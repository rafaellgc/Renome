//! Albums from MusicBrainz, without the network part: the search query,
//! ranking the releases found, checking them against the files' track
//! counts and durations, and matching files to tracks.

use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::music::AudioTags;
use crate::tmdb::similarity;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Named {
    #[serde(default)]
    pub name: String,
}

/// One artist of an artist credit (`Korn feat. Nas` is two).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Credit {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub joinphrase: String,
}

/// MusicBrainz's typographic punctuation in its plain form, for file
/// names: `a‐ha` (U+2010) → `a-ha`, `’` → `'`, `“…”` → `"…"`, `…` → `...`.
/// Dashes (`–`, `—`) are kept: titles use them on purpose.
pub fn plain(s: &str) -> String {
    s.replace(['\u{2010}', '\u{2011}'], "-")
        .replace(['\u{2018}', '\u{2019}', '\u{2032}'], "'")
        .replace(['\u{201c}', '\u{201d}'], "\"")
        .replace('\u{2026}', "...")
}

/// An artist credit written out: `Korn feat. Nas`.
pub fn credit(c: &[Credit]) -> String {
    c.iter()
        .map(|a| format!("{}{}", a.name, a.joinphrase))
        .collect::<String>()
        .trim()
        .to_string()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReleaseGroup {
    #[serde(default, rename = "primary-type")]
    pub primary_type: Option<String>,
    #[serde(default, rename = "first-release-date")]
    pub first_release_date: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LabelInfo {
    #[serde(default)]
    pub label: Option<Named>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Recording {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub length: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Track {
    #[serde(default)]
    pub position: u32,
    #[serde(default)]
    pub title: String,
    /// Milliseconds.
    #[serde(default)]
    pub length: Option<u64>,
    #[serde(default, rename = "artist-credit")]
    pub artist_credit: Vec<Credit>,
    #[serde(default)]
    pub recording: Option<Recording>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Medium {
    #[serde(default)]
    pub position: u32,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default, rename = "track-count")]
    pub track_count: u32,
    #[serde(default)]
    pub tracks: Vec<Track>,
}

/// A release as the search (summary) or the lookup (with tracks) returns it.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Release {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub score: Option<u32>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, rename = "artist-credit")]
    pub artist_credit: Vec<Credit>,
    #[serde(default, rename = "release-group")]
    pub release_group: Option<ReleaseGroup>,
    #[serde(default, rename = "label-info")]
    pub label_info: Vec<LabelInfo>,
    #[serde(default, rename = "track-count")]
    pub track_count: Option<u32>,
    #[serde(default)]
    pub media: Vec<Medium>,
}

impl Release {
    pub fn tracks(&self) -> u32 {
        self.track_count
            .unwrap_or_else(|| self.media.iter().map(|m| m.track_count).sum())
    }
}

#[derive(Debug, Deserialize)]
pub struct Search {
    #[serde(default)]
    pub releases: Vec<Release>,
}

/// What is known of an album from its files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumQuery {
    pub artist: String,
    pub album: String,
    pub year: Option<u16>,
    /// How many songs the album has in the list.
    pub tracks: u32,
}

/// Escapes Lucene's special characters for a quoted phrase.
fn phrase(s: &str) -> String {
    let escaped: String = s
        .chars()
        .flat_map(|c| {
            let special = "\\\"".contains(c);
            special
                .then_some('\\')
                .into_iter()
                .chain(std::iter::once(c))
        })
        .collect();
    format!("\"{escaped}\"")
}

/// The search for an album: by title and artist (any of the artist's
/// spellings, as MusicBrainz knows them).
pub fn query(q: &AlbumQuery) -> String {
    let mut parts = vec![format!("release:{}", phrase(&q.album))];
    if !q.artist.is_empty() && q.artist != crate::music::VARIOUS_ARTISTS {
        parts.push(format!("artist:{}", phrase(&q.artist)));
    }
    parts.join(" AND ")
}

/// A release offered for an album, best first.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub date: String,
    pub country: String,
    pub format: String,
    pub label: String,
    pub tracks: u32,
    pub kind: String,
    pub score: f64,
}

fn year_of(date: &Option<String>) -> Option<u16> {
    date.as_deref()?.get(..4)?.parse().ok()
}

fn summary(r: &Release, score: f64) -> Candidate {
    let mut formats: Vec<String> = r.media.iter().filter_map(|m| m.format.clone()).collect();
    formats.dedup();
    let format = match formats.as_slice() {
        [] => String::new(),
        [one] if r.media.len() > 1 => format!("{}×{one}", r.media.len()),
        _ => formats.join(" + "),
    };
    Candidate {
        id: r.id.clone(),
        title: r.title.clone(),
        artist: credit(&r.artist_credit),
        date: r.date.clone().unwrap_or_default(),
        country: r.country.clone().unwrap_or_default(),
        format,
        label: r
            .label_info
            .iter()
            .find_map(|l| l.label.as_ref().map(|n| n.name.clone()))
            .unwrap_or_default(),
        tracks: r.tracks(),
        kind: r
            .release_group
            .as_ref()
            .and_then(|g| g.primary_type.clone())
            .unwrap_or_default(),
        score,
    }
}

/// Ranks the releases found: title and artist alike, the same number of
/// tracks as the files, the same year, official releases and MusicBrainz's
/// own score. A release with as many tracks as the files counts most:
/// that is what tells editions apart.
pub fn rank(q: &AlbumQuery, releases: &[Release]) -> Vec<Candidate> {
    let mut seen = HashSet::new();
    let mut out: Vec<Candidate> = releases
        .iter()
        .filter(|r| seen.insert(r.id.clone()))
        .map(|r| {
            let title = similarity(&q.album, &r.title);
            let artist = if q.artist.is_empty() {
                0.5
            } else {
                similarity(&q.artist, &credit(&r.artist_credit))
            };
            let tracks = match r.tracks() {
                n if n == q.tracks => 0.3,
                n if n > q.tracks => 0.1, // bonus tracks missing from the files
                _ => -0.2,
            };
            let year = match (q.year, year_of(&r.date)) {
                (Some(a), Some(b)) if a == b => 0.1,
                _ => 0.0,
            };
            let official = if r.status.as_deref() == Some("Official") {
                0.05
            } else {
                0.0
            };
            let mb = f64::from(r.score.unwrap_or(0)) / 2000.0;
            summary(r, title + artist * 0.6 + tracks + year + official + mb)
        })
        .collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out
}

/// A song's details from its release, which replace those in its tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongMatch {
    pub release: String,
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub year: Option<u16>,
    pub track: u32,
    pub track_total: u32,
    pub disc: u32,
    pub disc_total: u32,
}

impl SongMatch {
    /// The tags with the release's details in place.
    pub fn apply(&self, t: &mut AudioTags) {
        t.title = self.title.clone();
        t.artist = self.artist.clone();
        t.album_artist = self.album_artist.clone();
        t.album = self.album.clone();
        t.year = self.year.or(t.year);
        t.track = Some(self.track);
        t.track_total = Some(self.track_total);
        t.disc = Some(self.disc);
        t.disc_total = Some(self.disc_total);
    }
}

/// A song in the list: its path and tags (with duration).
pub type Song = (PathBuf, AudioTags);

/// Titles alike, also without what is in brackets ("Celice (2026
/// Remaster)" and "Celice").
fn same_title(a: &str, b: &str) -> bool {
    let bare = |s: &str| crate::tmdb::BRACKETS.replace_all(s, " ").into_owned();
    a.trim().is_empty() || similarity(a, b) >= 0.6 || similarity(&bare(a), &bare(b)) >= 0.8
}

fn length_of(tr: &Track) -> Option<u64> {
    tr.length.or(tr.recording.as_ref().and_then(|r| r.length))
}

/// Within 5 seconds of the track's length.
fn same_length(tr: &Track, song: &AudioTags) -> bool {
    matches!((length_of(tr), song.duration), (Some(ms), Some(secs)) if (ms / 1000).abs_diff(u64::from(secs)) <= 5)
}

/// Matches the songs of an album to a release's tracks: by disc and track
/// number when the track has the song's title or length, else by title;
/// each track at most once. A song that fits no track is left out (it keeps
/// its tags). Returns the matches and how many agree in length.
pub fn match_tracks(release: &Release, songs: &[Song]) -> (Vec<(PathBuf, SongMatch)>, usize) {
    let album_artist = plain(&credit(&release.artist_credit));
    let discs = release.media.len().max(1) as u32;
    let year = year_of(&release.date);
    let mut used = HashSet::new();
    let mut out = Vec::new();
    let mut agree = 0;
    // Songs without a disc number belong to the only disc, or the first.
    let tracks: Vec<(&Medium, &Track)> = release
        .media
        .iter()
        .flat_map(|m| m.tracks.iter().map(move |t| (m, t)))
        .collect();
    for (path, t) in songs {
        let disc = t.disc.unwrap_or(1);
        // Same number, but another song (another edition's track list).
        let by_number = tracks.iter().position(|(m, tr)| {
            Some(tr.position) == t.track
                && m.position.max(1) == disc
                && !used.contains(&(m.position, tr.position))
                && (same_title(&t.title, &tr.title) || same_length(tr, t))
        });
        let by_title = || {
            tracks
                .iter()
                .enumerate()
                .filter(|(_, (m, tr))| !used.contains(&(m.position, tr.position)))
                .map(|(i, (_, tr))| (i, similarity(&t.title, &tr.title)))
                .filter(|(_, s)| *s >= 0.8)
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
        };
        let Some(i) = by_number.or_else(by_title) else {
            continue;
        };
        let (m, tr) = tracks[i];
        used.insert((m.position, tr.position));
        if same_length(tr, t) {
            agree += 1;
        }
        let artist = if tr.artist_credit.is_empty() {
            album_artist.clone()
        } else {
            plain(&credit(&tr.artist_credit))
        };
        out.push((
            path.clone(),
            SongMatch {
                release: release.id.clone(),
                title: plain(&tr.title),
                artist,
                album_artist: album_artist.clone(),
                album: plain(&release.title),
                year,
                track: tr.position,
                track_total: m.track_count.max(m.tracks.len() as u32),
                disc: m.position.max(1),
                disc_total: discs,
            },
        ));
    }
    (out, agree)
}

/// A release fits the songs without doubt: every song found its track (by
/// number with its title or length, or by title) and (nearly) all lengths
/// agree.
pub fn sure(release: &Release, songs: &[Song]) -> bool {
    let (matched, agree) = match_tracks(release, songs);
    let with_duration = songs.iter().filter(|(_, t)| t.duration.is_some()).count();
    matched.len() == songs.len() && with_duration > 0 && agree * 10 >= with_duration * 9
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(json: &str) -> Release {
        serde_json::from_str(json).unwrap()
    }

    fn song(title: &str, track: u32, secs: u32) -> Song {
        (
            PathBuf::from(format!("/m/{track}.flac")),
            AudioTags {
                title: title.into(),
                artist: "A-Ha".into(),
                album: "Hunting High and Low".into(),
                track: Some(track),
                duration: Some(secs),
                ..Default::default()
            },
        )
    }

    const HHL: &str = r#"{"id":"r1","title":"Hunting High and Low","date":"1985-06-01","status":"Official",
        "artist-credit":[{"name":"a‐ha","joinphrase":""}],
        "media":[{"position":1,"format":"CD","track-count":2,"tracks":[
            {"position":1,"title":"Take On Me","length":228000},
            {"position":2,"title":"Train of Thought","length":254000,
             "artist-credit":[{"name":"a‐ha","joinphrase":" feat. "},{"name":"X","joinphrase":""}]}]}]}"#;

    #[test]
    fn matches_and_checks() {
        let r = release(HHL);
        let songs = vec![song("Take On Me", 1, 227), song("Train Of Thought", 2, 255)];
        let (m, agree) = match_tracks(&r, &songs);
        assert_eq!(agree, 2);
        assert_eq!(m[0].1.album_artist, "a-ha");
        assert_eq!(m[1].1.artist, "a-ha feat. X");
        assert_eq!(plain("Don’t – “Stop”…"), "Don't – \"Stop\"...");
        assert_eq!(
            (m[1].1.track, m[1].1.track_total, m[1].1.year),
            (2, 2, Some(1985))
        );
        assert!(sure(&r, &songs));
        // A different edition: durations disagree.
        let songs = vec![song("Take On Me", 1, 300), song("Train Of Thought", 2, 400)];
        assert!(!sure(&r, &songs));
        // A song the release does not have.
        let songs = vec![song("Take On Me", 1, 227), song("Bonus", 3, 100)];
        assert!(!sure(&r, &songs));
        // Same number, another song and length: not that track.
        let songs = vec![song("Take On Me", 1, 227), song("Sing Sorrow", 2, 400)];
        let (m, _) = match_tracks(&r, &songs);
        assert_eq!(m.len(), 1);
        assert!(!sure(&r, &songs));
        // A remaster's title with the right length still fits.
        let songs = vec![
            song("Take On Me (2026 Remaster)", 1, 228),
            song("Train of Thought", 2, 254),
        ];
        assert!(sure(&r, &songs));
    }

    #[test]
    fn ranking() {
        let q = AlbumQuery {
            artist: "A-Ha".into(),
            album: "Hunting High And Low".into(),
            year: Some(1985),
            tracks: 2,
        };
        let deluxe = release(
            r#"{"id":"r2","title":"Hunting High and Low","date":"2010-07-26","track-count":40,"score":100,
                "artist-credit":[{"name":"a‐ha"}]}"#,
        );
        let mut first = release(HHL);
        first.score = Some(90);
        let c = rank(&q, &[deluxe, first]);
        assert_eq!(c[0].id, "r1");
        assert_eq!(c[0].format, "CD");
        assert_eq!(
            query(&q),
            r#"release:"Hunting High And Low" AND artist:"A-Ha""#
        );
    }
}
