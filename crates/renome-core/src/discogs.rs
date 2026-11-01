//! Albums from Discogs, without the network part. Its releases are read as
//! MusicBrainz's ([`Release`]), so the same ranking, matching and naming
//! work for both; their ids start with `discogs:`, which tells the app
//! where to fetch them and keeps them out of the MusicBrainz tags.

use std::collections::BTreeMap;

use regex::Regex;
use serde::{Deserialize, Deserializer};
use std::sync::LazyLock;

use crate::musicbrainz::{Credit, Genre, LabelInfo, Medium, Named, Release, ReleaseGroup, Track};

pub const PREFIX: &str = "discogs:";

/// Whether the release (or track) id is a Discogs one.
pub fn is_discogs(id: &str) -> bool {
    id.starts_with(PREFIX)
}

/// The release's number on Discogs, from its id here.
pub fn number(id: &str) -> Option<&str> {
    id.strip_prefix(PREFIX)
        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DgArtist {
    #[serde(default)]
    pub name: String,
    /// The name as credited on this release, when it differs.
    #[serde(default)]
    pub anv: String,
    /// What joins it to the next artist (`,`, `&`, `Feat.`).
    #[serde(default)]
    pub join: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DgLabel {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub catno: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DgFormat {
    #[serde(default)]
    pub name: String,
    #[serde(default, deserialize_with = "loose_number")]
    pub qty: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DgTrack {
    #[serde(default)]
    pub position: String,
    /// `track`, `heading` (a title between tracks) or `index` (a work
    /// whose parts are its `sub_tracks`).
    #[serde(default, rename = "type_")]
    pub kind: String,
    #[serde(default)]
    pub title: String,
    /// `3:45`, `1:02:03`; empty when unknown.
    #[serde(default)]
    pub duration: String,
    #[serde(default)]
    pub artists: Vec<DgArtist>,
    #[serde(default)]
    pub sub_tracks: Vec<DgTrack>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DgImage {
    /// `primary` (the front cover, usually) or `secondary`.
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub uri: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DgIdentifier {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub value: String,
}

/// A release as Discogs gives it.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DgRelease {
    pub id: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default, deserialize_with = "loose_number")]
    pub year: Option<u32>,
    /// `1987-07-00`: unknown parts are zeros.
    #[serde(default)]
    pub released: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub artists: Vec<DgArtist>,
    #[serde(default)]
    pub labels: Vec<DgLabel>,
    #[serde(default)]
    pub formats: Vec<DgFormat>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub tracklist: Vec<DgTrack>,
    #[serde(default)]
    pub images: Vec<DgImage>,
    #[serde(default)]
    pub identifiers: Vec<DgIdentifier>,
    #[serde(default)]
    pub master_id: Option<u64>,
}

impl DgRelease {
    /// The front cover's address: the primary image, else the first.
    pub fn cover(&self) -> Option<&str> {
        self.images
            .iter()
            .find(|i| i.kind == "primary")
            .or(self.images.first())
            .map(|i| i.uri.as_str())
            .filter(|u| !u.is_empty())
    }
}

/// A master release: what all its editions share, the first year among them.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DgMaster {
    #[serde(default, deserialize_with = "loose_number")]
    pub year: Option<u32>,
}

/// A release found by a search.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DgResult {
    pub id: u64,
    /// `Artist - Title`.
    #[serde(default)]
    pub title: String,
    #[serde(default, deserialize_with = "loose_number")]
    pub year: Option<u32>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub format: Vec<String>,
    #[serde(default)]
    pub label: Vec<String>,
    #[serde(default)]
    pub thumb: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DgSearch {
    #[serde(default)]
    pub results: Vec<DgResult>,
}

/// A number given as a number or as text (`"1987"`, `"2"`); none for 0 or
/// anything else.
fn loose_number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
    .filter(|&n| n > 0))
}

/// An artist's name without the number Discogs adds to tell artists of the
/// same name apart (`Korn (2)` → `Korn`).
pub fn clean_name(name: &str) -> String {
    static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+\(\d+\)$").unwrap());
    let name = NUMBER.replace(name.trim(), "").to_string();
    if name == "Various" {
        crate::music::VARIOUS_ARTISTS.to_string()
    } else {
        name
    }
}

/// The artists as a MusicBrainz credit: each as credited, joined as
/// Discogs joins them (`A, B & C`, `A Feat. B`).
fn credits(artists: &[DgArtist]) -> Vec<Credit> {
    let n = artists.len();
    artists
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let name = clean_name(if a.anv.trim().is_empty() {
                &a.name
            } else {
                &a.anv
            });
            let join = a.join.trim();
            let joinphrase = if i + 1 == n || join.is_empty() {
                if i + 1 == n {
                    String::new()
                } else {
                    ", ".into()
                }
            } else if join == "," {
                ", ".into()
            } else {
                format!(" {join} ")
            };
            Credit {
                name,
                joinphrase,
                artist: None,
            }
        })
        .collect()
}

/// `3:45` → 225000 ms.
fn millis(duration: &str) -> Option<u64> {
    let parts: Vec<u64> = duration
        .trim()
        .split(':')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    let secs = match parts.as_slice() {
        [m, s] => m * 60 + s,
        [h, m, s] => h * 3600 + m * 60 + s,
        _ => return None,
    };
    (secs > 0).then_some(secs * 1000)
}

/// The disc a track's position names (`2-05`, `CD2-5`, `CD2.5`); 1 for
/// the rest (`5`, vinyl sides `A1`, `B2`, which number on as one disc, and
/// a track's parts `12.1`, `12.2`).
fn disc_of(position: &str) -> u32 {
    static DISC: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?:(?:cd|dvd|disc|disk|lp)\s*(\d+)\s*[-.]|(\d+)\s*-)\s*\d+").unwrap()
    });
    DISC.captures(position.trim())
        .and_then(|c| c.get(1).or(c.get(2))?.as_str().parse().ok())
        .filter(|&d| d > 0)
        .unwrap_or(1)
}

/// The track a part's position belongs to (`12.1` and `3a` → `12`, `3`;
/// `2-3b` → `2-3`); `None` for a whole track (and `CD2.5`, a disc's).
fn part_of(position: &str) -> Option<&str> {
    static PART: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^([0-9][0-9-]*?)(?:\.[0-9]+|[a-z])$").unwrap());
    PART.captures(position.trim())
        .map(|c| c.get(1).unwrap().as_str())
}

/// Parts of one track listed as tracks (`12.1 Song`, `12.2 [hidden]`):
/// one track, as the file holds them, titled with both.
fn join_parts(tracks: Vec<DgTrack>) -> Vec<DgTrack> {
    let mut out: Vec<DgTrack> = Vec::new();
    let mut last_part: Option<String> = None;
    for t in tracks {
        let part = part_of(&t.position).map(str::to_string);
        match (&part, &last_part, out.last_mut()) {
            (Some(p), Some(l), Some(prev)) if p == l => {
                prev.title = format!("{} / {}", prev.title.trim(), t.title.trim());
                let total = millis(&prev.duration)
                    .zip(millis(&t.duration))
                    .map(|(a, b)| a + b);
                prev.duration = total
                    .map(|ms| format!("{}:{:02}", ms / 60000, ms / 1000 % 60))
                    .unwrap_or_default();
            }
            _ => {
                let mut t = t;
                if let Some(p) = &part {
                    t.position = p.clone();
                }
                out.push(t);
            }
        }
        last_part = part;
    }
    out
}

/// The tracks as the files have them: headings left out; a work's parts
/// as tracks of their own (`Sonata: I. Allegro`), unless they are parts of
/// one track (`3a`, `3b`).
fn flat(tracklist: &[DgTrack]) -> Vec<DgTrack> {
    let mut out = Vec::new();
    for t in tracklist {
        match t.kind.as_str() {
            "heading" => {}
            "index" => {
                let own = !t.sub_tracks.is_empty()
                    && t.sub_tracks.iter().all(|s| {
                        let p = s.position.trim();
                        !p.is_empty() && !p.ends_with(|c: char| c.is_ascii_alphabetic())
                    });
                if own {
                    for s in &t.sub_tracks {
                        out.push(DgTrack {
                            title: format!("{}: {}", t.title.trim(), s.title.trim()),
                            artists: if s.artists.is_empty() {
                                t.artists.clone()
                            } else {
                                s.artists.clone()
                            },
                            ..s.clone()
                        });
                    }
                } else {
                    let position = t
                        .sub_tracks
                        .first()
                        .map(|s| {
                            s.position
                                .trim_end_matches(|c: char| c.is_ascii_alphabetic())
                                .to_string()
                        })
                        .filter(|_| t.position.trim().is_empty())
                        .unwrap_or_else(|| t.position.clone());
                    let duration = if t.duration.trim().is_empty() {
                        let total: Option<u64> =
                            t.sub_tracks.iter().map(|s| millis(&s.duration)).sum();
                        total
                            .map(|ms| format!("{}:{:02}", ms / 60000, ms / 1000 % 60))
                            .unwrap_or_default()
                    } else {
                        t.duration.clone()
                    };
                    out.push(DgTrack {
                        position,
                        kind: "track".into(),
                        duration,
                        sub_tracks: Vec::new(),
                        ..t.clone()
                    });
                }
            }
            _ => out.push(t.clone()),
        }
    }
    join_parts(out)
}

/// `1987-07-00` → `1987-07`; the year alone when that is all there is.
fn date_of(r: &DgRelease) -> Option<String> {
    let full = r.released.as_deref().unwrap_or("").trim();
    let parts: Vec<&str> = full
        .split('-')
        .take_while(|p| !p.is_empty() && p.bytes().any(|b| b != b'0'))
        .collect();
    match parts.first() {
        Some(y) if y.len() == 4 => Some(parts.join("-")),
        _ => r.year.map(|y| y.to_string()),
    }
}

/// The release as MusicBrainz would give it. `original_year` is the
/// master release's (the first edition's) year.
pub fn to_release(r: &DgRelease, original_year: Option<u32>) -> Release {
    let id = format!("{PREFIX}{}", r.id);
    let mut discs: BTreeMap<u32, Vec<Track>> = BTreeMap::new();
    for t in flat(&r.tracklist) {
        let disc = disc_of(&t.position);
        let list = discs.entry(disc).or_default();
        let position = list.len() as u32 + 1;
        list.push(Track {
            id: format!("{id}:{disc}-{position}"),
            position,
            title: t.title.trim().to_string(),
            length: millis(&t.duration),
            artist_credit: credits(&t.artists),
            recording: None,
        });
    }
    let format = r.formats.first().map(|f| f.name.clone());
    let media: Vec<Medium> = discs
        .into_iter()
        .map(|(position, tracks)| Medium {
            position,
            format: format.clone(),
            title: String::new(),
            track_count: tracks.len() as u32,
            tracks,
        })
        .collect();
    // Styles are the more precise (`Nu Metal` within `Rock`).
    let genres: Vec<Genre> = r
        .styles
        .iter()
        .map(|s| Genre {
            name: s.clone(),
            count: 2,
        })
        .chain(r.genres.iter().map(|g| Genre {
            name: g.clone(),
            count: 1,
        }))
        .collect();
    Release {
        id,
        title: r.title.trim().to_string(),
        score: None,
        date: date_of(r),
        // Discogs names countries ("Europe", "Brazil"); the tags take codes.
        country: None,
        status: Some("Official".into()),
        artist_credit: credits(&r.artists),
        release_group: Some(ReleaseGroup {
            first_release_date: original_year.map(|y| y.to_string()),
            ..Default::default()
        }),
        label_info: r
            .labels
            .iter()
            .map(|l| LabelInfo {
                label: Some(Named {
                    name: clean_name(&l.name),
                }),
                catalog_number: Some(l.catno.trim().to_string())
                    .filter(|c| !c.is_empty() && !c.eq_ignore_ascii_case("none")),
            })
            .collect(),
        track_count: Some(media.iter().map(|m| m.track_count).sum()),
        media,
        barcode: r
            .identifiers
            .iter()
            .find(|i| i.kind == "Barcode")
            .map(|i| {
                i.value
                    .chars()
                    .filter(char::is_ascii_digit)
                    .collect::<String>()
            })
            .filter(|b| !b.is_empty()),
        text_representation: None,
        genres,
    }
}

/// The releases a search found, as MusicBrainz search results (no tracks:
/// the search does not give them).
pub fn found(search: &DgSearch) -> Vec<Release> {
    search
        .results
        .iter()
        .map(|r| {
            let (artist, title) = match r.title.split_once(" - ") {
                Some((a, t)) => (clean_name(a), t.trim().to_string()),
                None => (String::new(), r.title.trim().to_string()),
            };
            Release {
                id: format!("{PREFIX}{}", r.id),
                title,
                date: r.year.map(|y| y.to_string()),
                country: r.country.clone().filter(|c| !c.is_empty()),
                status: Some("Official".into()),
                artist_credit: vec![Credit {
                    name: artist,
                    ..Default::default()
                }],
                label_info: r
                    .label
                    .first()
                    .map(|l| LabelInfo {
                        label: Some(Named {
                            name: clean_name(l),
                        }),
                        catalog_number: None,
                    })
                    .into_iter()
                    .collect(),
                media: r
                    .format
                    .first()
                    .map(|f| Medium {
                        format: Some(f.clone()),
                        ..Default::default()
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::AudioTags;
    use crate::musicbrainz::{match_tracks, sure, tracklist};
    use std::path::PathBuf;

    const RELEASE: &str = r#"{"id":42,"title":"Lua Cheia","year":1999,"released":"1999-07-00",
        "country":"Brazil",
        "artists":[{"name":"Banda Azul (3)","anv":"","join":"Feat."},{"name":"Rui","anv":"Rui S.","join":""}],
        "labels":[{"name":"Selo (2)","catno":"SL 01"}],
        "formats":[{"name":"CD","qty":"2"}],
        "genres":["Rock"],"styles":["Pop Rock"],
        "identifiers":[{"type":"Barcode","value":"7 891234 567890"}],
        "images":[{"type":"secondary","uri":"https://i/2.jpg"},{"type":"primary","uri":"https://i/1.jpg"}],
        "tracklist":[
            {"position":"","type_":"heading","title":"Parte Um"},
            {"position":"1-1","type_":"track","title":"Abertura","duration":"3:45"},
            {"position":"1-2","type_":"track","title":"Maré","duration":"4:10",
             "artists":[{"name":"Outra","anv":"","join":""}]},
            {"position":"","type_":"index","title":"Suíte","sub_tracks":[
                {"position":"2-1","type_":"track","title":"I. Lento","duration":"2:00"},
                {"position":"2-2","type_":"track","title":"II. Rápido","duration":"1:30"}]},
            {"position":"","type_":"index","title":"Medley","sub_tracks":[
                {"position":"2-3a","type_":"track","title":"Uma","duration":"1:00"},
                {"position":"2-3b","type_":"track","title":"Outra","duration":"0:30"}]}
        ]}"#;

    fn song(title: &str, disc: u32, track: u32, secs: u32) -> (PathBuf, AudioTags) {
        (
            PathBuf::from(format!("/m/{disc}-{track}.flac")),
            AudioTags {
                title: title.into(),
                track: Some(track),
                disc: Some(disc),
                duration: Some(secs),
                ..Default::default()
            },
        )
    }

    #[test]
    fn parts_of_a_track() {
        assert_eq!(disc_of("12.1"), 1);
        assert_eq!(disc_of("2-05"), 2);
        assert_eq!(disc_of("CD2.5"), 2);
        assert_eq!(disc_of("A1"), 1);
        assert_eq!(part_of("12.1"), Some("12"));
        assert_eq!(part_of("2-3b"), Some("2-3"));
        assert_eq!(part_of("CD2.5"), None);
        assert_eq!(part_of("A1"), None);
        let dg: DgRelease = serde_json::from_str(
            r#"{"id":7,"title":"T","tracklist":[
            {"position":"11","type_":"track","title":"Onze","duration":"4:02"},
            {"position":"12.1","type_":"track","title":"Doze","duration":"9:00"},
            {"position":"12.2","type_":"track","title":"Oculta","duration":"0:32"}]}"#,
        )
        .unwrap();
        let r = to_release(&dg, None);
        assert_eq!(r.media.len(), 1);
        let t = &r.media[0].tracks;
        assert_eq!(
            (t.len(), t[1].title.as_str(), t[1].length),
            (2, "Doze / Oculta", Some(572_000))
        );
    }

    #[test]
    fn reads_a_release_as_musicbrainz() {
        let dg: DgRelease = serde_json::from_str(RELEASE).unwrap();
        assert_eq!(dg.cover(), Some("https://i/1.jpg"));
        let r = to_release(&dg, Some(1998));
        assert_eq!(r.id, "discogs:42");
        assert_eq!(r.date.as_deref(), Some("1999-07"));
        assert_eq!(
            crate::musicbrainz::credit(&r.artist_credit),
            "Banda Azul Feat. Rui S."
        );
        assert_eq!(r.barcode.as_deref(), Some("7891234567890"));
        let list = tracklist(&r);
        let shown: Vec<(u32, u32, &str, Option<u64>)> = list
            .iter()
            .map(|t| (t.disc, t.position, t.title.as_str(), t.length))
            .collect();
        assert_eq!(
            shown,
            [
                (1, 1, "Abertura", Some(225)),
                (1, 2, "Maré", Some(250)),
                (2, 1, "Suíte: I. Lento", Some(120)),
                (2, 2, "Suíte: II. Rápido", Some(90)),
                (2, 3, "Medley", Some(90)),
            ]
        );
        let songs = vec![
            song("Abertura", 1, 1, 226),
            song("Maré", 1, 2, 250),
            song("I. Lento", 2, 1, 120),
            song("II. Rápido", 2, 2, 91),
            song("Medley", 2, 3, 90),
        ];
        let (m, agree) = match_tracks(&r, &songs);
        assert_eq!((m.len(), agree), (5, 5));
        assert!(sure(&r, &songs));
        let first = &m[0].1;
        assert_eq!(
            (
                first.album_artist.as_str(),
                first.label.as_str(),
                first.catalog.as_str()
            ),
            ("Banda Azul Feat. Rui S.", "Selo", "SL 01")
        );
        assert_eq!((first.disc_total, first.original_year), (2, Some(1998)));
        assert_eq!(m[1].1.artist, "Outra");
    }

    #[test]
    fn reads_search_results() {
        let s: DgSearch = serde_json::from_str(
            r#"{"results":[{"id":7,"title":"Korn (2) - Korn","year":"1994","country":"US",
                "format":["CD","Album"],"label":["Immortal Records","Epic"],"thumb":"t"}]}"#,
        )
        .unwrap();
        let r = &found(&s)[0];
        assert_eq!((r.id.as_str(), r.title.as_str()), ("discogs:7", "Korn"));
        assert_eq!(crate::musicbrainz::credit(&r.artist_credit), "Korn");
        assert_eq!(r.date.as_deref(), Some("1994"));
    }

    #[test]
    fn small_parts() {
        assert_eq!(disc_of("2-05"), 2);
        assert_eq!(disc_of("CD3-1"), 3);
        assert_eq!(disc_of("B2"), 1);
        assert_eq!(disc_of("7"), 1);
        assert_eq!(millis("1:02:03"), Some(3_723_000));
        assert_eq!(millis(""), None);
        assert_eq!(clean_name("Various"), "Various Artists");
        assert_eq!(number("discogs:12"), Some("12"));
        assert_eq!(number("abc"), None);
    }
}
