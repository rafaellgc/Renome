//! AcoustID answers, without the network part: the recordings a song's
//! fingerprint was heard as, the releases they are on, and how the songs of
//! a folder are split among those releases.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::musicbrainz::{Candidate, Credit, credit, plain};

/// Answers below this score are other songs that sound alike.
const MIN_SCORE: f64 = 0.5;

#[derive(Debug, Default, Deserialize)]
pub struct Response {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub results: Vec<Result>,
    #[serde(default)]
    pub error: Option<ApiError>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ApiError {
    #[serde(default)]
    pub code: u32,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct Result {
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub recordings: Vec<Recording>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Recording {
    pub id: String,
    /// How many submissions linked the fingerprint to this recording.
    #[serde(default)]
    pub sources: u32,
    #[serde(default)]
    pub releasegroups: Vec<Group>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Group {
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub secondarytypes: Vec<String>,
    #[serde(default)]
    pub releases: Vec<Release>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Date {
    pub year: Option<u16>,
    pub month: Option<u8>,
    pub day: Option<u8>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Release {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artists: Vec<Credit>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub date: Option<Date>,
    #[serde(default)]
    pub track_count: Option<u32>,
}

/// A release a song is on.
#[derive(Debug, Clone, PartialEq)]
pub struct OnRelease {
    pub id: String,
    pub title: String,
    pub artist: String,
    /// `2005-01-24`, as much of it as is known.
    pub date: String,
    pub country: String,
    pub tracks: u32,
    pub kind: String,
    /// An album of the artist's own: not a compilation, live album,
    /// soundtrack or remix.
    pub studio: bool,
}

/// What AcoustID knows of a song.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Heard {
    /// MusicBrainz recordings, the likeliest first.
    pub recordings: Vec<String>,
    pub releases: Vec<OnRelease>,
}

fn date_of(d: &Option<Date>) -> String {
    let Some(d) = d else { return String::new() };
    match (d.year, d.month, d.day) {
        (Some(y), Some(m), Some(d)) => format!("{y:04}-{m:02}-{d:02}"),
        (Some(y), Some(m), None) => format!("{y:04}-{m:02}"),
        (Some(y), _, _) => format!("{y:04}"),
        _ => String::new(),
    }
}

/// The recordings and releases of an answer: those heard with confidence
/// and confirmed by at least half as many submissions as the most
/// confirmed one (a fingerprint is sometimes linked to other songs too).
pub fn heard(r: &Response) -> Heard {
    let mut out = Heard::default();
    let mut recordings: Vec<&Recording> = r
        .results
        .iter()
        .filter(|x| x.score >= MIN_SCORE)
        .flat_map(|x| &x.recordings)
        .collect();
    let most = recordings.iter().map(|x| x.sources).max().unwrap_or(0);
    recordings.retain(|x| x.sources * 2 >= most);
    recordings.sort_by_key(|x| std::cmp::Reverse(x.sources));
    for rec in recordings {
        if !out.recordings.contains(&rec.id) {
            out.recordings.push(rec.id.clone());
        }
        for g in &rec.releasegroups {
            for rel in &g.releases {
                if out.releases.iter().any(|x| x.id == rel.id) {
                    continue;
                }
                out.releases.push(OnRelease {
                    id: rel.id.clone(),
                    title: plain(&rel.title),
                    artist: plain(&credit(&rel.artists)),
                    date: date_of(&rel.date),
                    country: rel.country.clone().unwrap_or_default(),
                    tracks: rel.track_count.unwrap_or(0),
                    kind: g.kind.clone().unwrap_or_default(),
                    studio: g.kind.as_deref() == Some("Album") && g.secondarytypes.is_empty(),
                });
            }
        }
    }
    out
}

/// How a release is preferred for some songs of a folder of `n` songs:
/// one with at least half of them (the folder is that album), then an album
/// of the artist's own, then one with more of them, then one with as many
/// tracks as the folder has songs (not a deluxe edition), then the first
/// one out.
fn preference(r: &OnRelease, count: usize, n: usize) -> impl Ord {
    let earliest = std::cmp::Reverse(if r.date.is_empty() {
        "9999".to_string()
    } else {
        r.date.clone()
    });
    (
        count * 2 >= n && count > 1,
        r.studio,
        count,
        std::cmp::Reverse((r.tracks as usize).abs_diff(n)),
        earliest,
    )
}

/// An album found for some songs of a folder: its songs and the releases
/// that have all of them, best first (none when nothing was heard).
#[derive(Debug, Clone)]
pub struct Found {
    pub songs: Vec<PathBuf>,
    pub candidates: Vec<Candidate>,
}

fn summary(r: &OnRelease, count: usize) -> Candidate {
    Candidate {
        id: r.id.clone(),
        title: r.title.clone(),
        artist: r.artist.clone(),
        date: r.date.clone(),
        country: r.country.clone(),
        format: String::new(),
        label: String::new(),
        tracks: r.tracks,
        kind: r.kind.clone(),
        score: count as f64,
    }
}

/// Splits the songs of one folder among releases: again and again, the
/// release preferred for the songs still without one takes those of them it
/// has. Songs nothing was heard for end up together, without candidates.
pub fn albums(songs: &[(PathBuf, Heard)]) -> Vec<Found> {
    let n = songs.len();
    let mut left: Vec<usize> = (0..n).collect();
    let mut out = Vec::new();
    loop {
        // In the order AcoustID gave them, which settles ties the same way
        // every time.
        let mut on: Vec<(&OnRelease, Vec<usize>)> = Vec::new();
        let mut at: HashMap<&str, usize> = HashMap::new();
        for &i in &left {
            for r in &songs[i].1.releases {
                let k = *at.entry(&r.id).or_insert_with(|| {
                    on.push((r, Vec::new()));
                    on.len() - 1
                });
                on[k].1.push(i);
            }
        }
        // `max_by_key` keeps the last of equals: look from the end.
        let Some((_, best)) = on
            .iter()
            .rev()
            .max_by_key(|(r, s)| preference(r, s.len(), n))
        else {
            break;
        };
        let best = best.clone();
        let mut candidates: Vec<(&OnRelease, usize)> = on
            .iter()
            .filter(|(_, s)| best.iter().all(|i| s.contains(i)))
            .map(|(r, s)| (*r, s.len()))
            .collect();
        candidates.sort_by_key(|(r, c)| std::cmp::Reverse(preference(r, *c, n)));
        out.push(Found {
            songs: best.iter().map(|&i| songs[i].0.clone()).collect(),
            candidates: candidates.iter().map(|(r, c)| summary(r, *c)).collect(),
        });
        left.retain(|i| !best.contains(i));
    }
    if !left.is_empty() {
        out.push(Found {
            songs: left.iter().map(|&i| songs[i].0.clone()).collect(),
            candidates: Vec::new(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(id: &str, tracks: u32, date: &str, studio: bool) -> OnRelease {
        OnRelease {
            id: id.into(),
            title: id.into(),
            artist: "X".into(),
            date: date.into(),
            country: String::new(),
            tracks,
            kind: "Album".into(),
            studio,
        }
    }

    fn song(name: &str, releases: Vec<OnRelease>) -> (PathBuf, Heard) {
        (
            PathBuf::from(name),
            Heard {
                recordings: vec![format!("rec-{name}")],
                releases,
            },
        )
    }

    #[test]
    fn reads_an_answer() {
        let json = r#"{"status":"ok","results":[
            {"score":0.97,"recordings":[{"id":"rec1","sources":10,"releasegroups":[
                {"type":"Album","secondarytypes":["Compilation"],"releases":[
                    {"id":"comp","title":"Hits’","artists":[{"name":"Various Artists"}],
                     "date":{"year":2008,"month":7},"track_count":23}]},
                {"type":"Album","releases":[
                    {"id":"orig","title":"Dawn","artists":[{"name":"M83"}],"country":"FR",
                     "date":{"year":2005,"month":1,"day":24},"track_count":15}]}]}]},
            {"score":0.97,"recordings":[{"id":"linked","sources":4,"releasegroups":[
                {"type":"Album","releases":[{"id":"wrong","title":"Other"}]}]}]},
            {"score":0.2,"recordings":[{"id":"other","sources":50}]}]}"#;
        let r: Response = serde_json::from_str(json).unwrap();
        let h = heard(&r);
        assert_eq!(h.recordings, ["rec1"]);
        assert_eq!(h.releases.len(), 2);
        assert_eq!(h.releases[0].title, "Hits'");
        assert_eq!(h.releases[0].date, "2008-07");
        assert!(!h.releases[0].studio);
        assert_eq!(
            (h.releases[1].date.as_str(), h.releases[1].studio),
            ("2005-01-24", true)
        );
        let e: Response = serde_json::from_str(
            r#"{"status":"error","error":{"code":4,"message":"invalid API key"}}"#,
        )
        .unwrap();
        assert_eq!(e.error.unwrap().code, 4);
    }

    #[test]
    fn splits_a_folder() {
        // An album folder: the original edition beats the deluxe one and the
        // later reissue; the bonus song is only on the deluxe.
        let orig = rel("orig", 3, "1985", true);
        let deluxe = rel("deluxe", 6, "2010", true);
        let reissue = rel("reissue", 3, "1998", true);
        let songs = vec![
            song("1", vec![deluxe.clone(), orig.clone(), reissue.clone()]),
            song("2", vec![orig.clone(), deluxe.clone(), reissue.clone()]),
            song("3", vec![orig.clone(), reissue.clone(), deluxe.clone()]),
            song("4", vec![deluxe.clone()]),
            song("5", vec![]),
        ];
        let a = albums(&songs);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].songs.len(), 4);
        assert_eq!(a[0].candidates[0].id, "deluxe");
        assert!(a[1].candidates.is_empty());
        // Editions alike: the first one given, every time.
        let a1 = rel("a1", 3, "1985", true);
        let a2 = rel("a2", 3, "1985", true);
        let tie = vec![
            song("1", vec![a1.clone(), a2.clone()]),
            song("2", vec![a1, a2]),
        ];
        for _ in 0..20 {
            assert_eq!(albums(&tie)[0].candidates[0].id, "a1");
        }
        // Without the bonus song, the original edition.
        let a = albums(&songs[..3]);
        assert_eq!(a.len(), 1);
        let ids: Vec<&str> = a[0].candidates.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["orig", "reissue", "deluxe"]);
        // Nothing heard: no candidates.
        let a = albums(&songs[4..]);
        assert!(a[0].candidates.is_empty());

        // Loose songs: two of ten on a compilation still go to their own
        // albums.
        let hits = OnRelease {
            studio: false,
            ..rel("hits", 20, "2000", false)
        };
        let mut songs = vec![
            song("a", vec![hits.clone(), rel("albumA", 10, "1990", true)]),
            song("b", vec![hits.clone(), rel("albumB", 10, "1992", true)]),
        ];
        for i in 0..8 {
            songs.push(song(
                &format!("x{i}"),
                vec![rel(&format!("x{i}"), 10, "1995", true)],
            ));
        }
        let a = albums(&songs);
        assert_eq!(a.len(), 10);
        assert!(a.iter().all(|f| f.songs.len() == 1));
        assert!(a.iter().any(|f| f.candidates[0].id == "albumA"));
        // A compilation folder stays the compilation.
        let a = albums(&songs[..2]);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].candidates[0].id, "hits");
    }
}
