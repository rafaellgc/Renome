//! What the lookups say is missing from the list, the same for the window
//! (its faded rows) and the command line (`--missing`): aired episodes of
//! the seasons the videos cover, published issues of the comics' series
//! (the gaps between the files' own numbers for a catalogue without a
//! list of issues), tracks of the albums, released movies of the
//! collections, volumes between a book series' files. Only what is out by
//! `day`, and not what a file renamed by hand now is.

use super::after::Opts;
use super::comics::{Listed, Settled};
use crate::tmdb::MovieSet;
use renome_core::books::Guess;
use renome_core::comics::{ComicMatch, Issue};
use renome_core::musicbrainz::{ListedTrack, SongMatch};
use renome_core::tv::{EpisodeMatch, Show};
use renome_core::{Preview, PreviewItem};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// An album chosen, with its release's tracks and its songs.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumTracks {
    /// The folder its songs are in.
    pub dir: PathBuf,
    pub release: String,
    pub tracks: Vec<ListedTrack>,
    pub files: Vec<PathBuf>,
}

/// What the lookups found, by each file's path.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Known {
    /// The shows fetched, with their episodes.
    pub shows: Vec<Show>,
    pub episodes: HashMap<PathBuf, EpisodeMatch>,
    /// Each comic's series and issue, and the series with their issues.
    pub comics: Vec<Settled>,
    pub volumes: HashMap<String, Listed>,
    pub albums: Vec<AlbumTracks>,
    pub songs: HashMap<PathBuf, SongMatch>,
    /// Each movie's TMDB id, the collection each movie is in, and the
    /// collections.
    pub movies: HashMap<PathBuf, u64>,
    pub set_of: HashMap<u64, Option<u64>>,
    pub sets: Vec<MovieSet>,
    /// What each book was taken for (its series and number).
    pub book_guesses: HashMap<PathBuf, Guess>,
}

/// A missing item, and the file it sits next to in the list.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Missing {
    pub key: String,
    /// `episode`, `issue`, `track`, `movie` or `book`.
    pub kind: &'static str,
    /// The show, series, album or collection, with its year.
    pub group: String,
    /// An episode's season, a track's disc; 0 for the others.
    pub season: u32,
    /// An episode's number; an issue's, track's or movie's place in its
    /// series, album or collection; a book's volume.
    pub number: i64,
    /// A track's number on its disc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<u32>,
    /// As in file names: S01E16, #7, 1-05, 1999, 3.
    pub code: String,
    pub title: String,
    /// Air, cover or release date (YYYY-MM-DD), when known.
    pub date: String,
    /// The file it sits next to, and on which side.
    pub anchor: PathBuf,
    pub after: bool,
}

/// Today as YYYY-MM-DD, in local time (air dates have no hour).
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// A file holding the places `from`..=`to` of a series.
struct Span<'a> {
    path: &'a PathBuf,
    from: f64,
    to: f64,
}

/// Where a missing `n` goes among the files: before the nearest later
/// file, else after the nearest earlier one (swapped when the list runs
/// backwards).
fn place(n: f64, files: &[Span], descending: bool) -> (PathBuf, bool) {
    let mut later: Option<&Span> = None;
    let mut earlier: Option<&Span> = None;
    for f in files {
        if f.from > n && later.is_none_or(|l| f.from < l.from) {
            later = Some(f);
        }
        if f.to < n && earlier.is_none_or(|e| f.to > e.to) {
            earlier = Some(f);
        }
    }
    let anchor = later
        .or(earlier)
        .map(|f| f.path.clone())
        .unwrap_or_default();
    (anchor, later.is_none() != descending)
}

fn named(name: &str, year: Option<u16>) -> String {
    year.map_or_else(|| name.to_string(), |y| format!("{name} ({y})"))
}

fn is_video(name: &str) -> bool {
    let ext = Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    renome_core::tmdb::is_video(&ext)
}

/// A number written at the start (`12`, `12.5`, `12a` as 12), as a page
/// reads it; none: not a number.
fn leading_number(s: &str) -> Option<f64> {
    let s = s.trim();
    let end = s
        .char_indices()
        .take_while(|(i, c)| c.is_ascii_digit() || (*c == '.' && *i > 0) || (*c == '-' && *i == 0))
        .map(|(i, c)| i + c.len_utf8())
        .last()?;
    s[..end].trim_end_matches('.').parse().ok()
}

/// Everything missing, for the rules that are on (each with its
/// "missing" option): `items`, the list; `descending`, which way it runs.
pub fn of(
    rules: &[Value],
    known: &Known,
    preview: &Preview,
    descending: bool,
    day: &str,
) -> Vec<Missing> {
    let items = &preview.items;
    let on = |kind| Opts::of(rules, kind).bool("missing");
    let mut out = Vec::new();
    if on("series") {
        out.extend(episodes(known, items, descending, day));
    }
    if on("comic") {
        out.extend(issues(known, items, descending, day));
    }
    if on("music") {
        out.extend(tracks(known, items, descending));
    }
    if on("movie") {
        out.extend(movies(known, items, descending, day));
    }
    if on("book") {
        out.extend(books(known, items, descending));
    }
    let mut seen = HashSet::new();
    out.retain(|m| seen.insert(m.key.clone()));
    not_typed_in(out, items)
}

/// The aired episodes missing from the seasons the listed videos cover
/// (seasons with no video at all are left out: a whole show would flood
/// the list).
fn episodes(known: &Known, items: &[PreviewItem], descending: bool, day: &str) -> Vec<Missing> {
    type Season<'a> = (&'a Show, u16, Vec<(&'a PathBuf, &'a [u16])>);
    let mut seasons: Vec<Season> = Vec::new();
    for i in items {
        let Some(m) = known.episodes.get(&i.path) else {
            continue;
        };
        let Some(show) = known
            .shows
            .iter()
            .find(|s| s.id == m.id && s.source == m.source)
        else {
            continue;
        };
        if m.episodes.is_empty() || !is_video(&i.original) {
            continue;
        }
        match seasons
            .iter_mut()
            .find(|(s, n, _)| std::ptr::eq(*s, show) && *n == m.season)
        {
            Some(s) => s.2.push((&i.path, &m.episodes)),
            None => seasons.push((show, m.season, vec![(&i.path, &m.episodes)])),
        }
    }
    let mut out = Vec::new();
    for (show, season, files) in seasons {
        let have: HashSet<u16> = files.iter().flat_map(|(_, e)| e.iter().copied()).collect();
        let spans: Vec<Span> = files
            .iter()
            .map(|(p, e)| Span {
                path: p,
                from: f64::from(*e.iter().min().unwrap_or(&0)),
                to: f64::from(*e.iter().max().unwrap_or(&0)),
            })
            .collect();
        for e in &show.episodes {
            let Some(date) = e.air_date.as_deref().filter(|d| !d.is_empty() && *d <= day) else {
                continue;
            };
            if e.season != season || have.contains(&e.number) {
                continue;
            }
            let (anchor, after) = place(f64::from(e.number), &spans, descending);
            out.push(Missing {
                key: format!("ghost:{}{}:{season}:{}", show.source, show.id, e.number),
                kind: "episode",
                group: named(&show.name, show.year),
                season: u32::from(season),
                number: i64::from(e.number),
                position: None,
                code: format!("S{season:02}E{:02}", e.number),
                title: e.name.clone(),
                date: date.to_string(),
                anchor,
                after,
            });
        }
    }
    out
}

/// A series' issues in reading order: by number, then as listed.
fn in_order(issues: &[Issue]) -> Vec<&Issue> {
    let mut order: Vec<&Issue> = issues.iter().collect();
    let n = |i: &Issue| leading_number(&i.number).unwrap_or(f64::INFINITY);
    order.sort_by(|a, b| n(a).total_cmp(&n(b)));
    order
}

/// The numbers missing between a series' own files, from 1 to the
/// highest they have, for a catalogue without a list of issues
/// (MangaDex): by volume when every file has one, else by issue number,
/// else by chapter.
fn gaps(
    name: &str,
    vkey: &str,
    files: &[(&PathBuf, &ComicMatch)],
    descending: bool,
) -> Vec<Missing> {
    type Of = fn(&ComicMatch) -> Option<f64>;
    type Code = fn(i64) -> String;
    let by: [(Of, Code); 3] = [
        (|o| o.volume.map(f64::from), |n| format!("v{n:02}")),
        (|o| leading_number(&o.number), |n| format!("#{n}")),
        (|o| leading_number(&o.chapter), |n| format!("c{n:03}")),
    ];
    let Some((of, code)) = by
        .into_iter()
        .find(|(of, _)| files.iter().all(|(_, own)| of(own).is_some()))
    else {
        return Vec::new();
    };
    let spans: Vec<Span> = files
        .iter()
        .map(|(p, own)| {
            let n = of(own).unwrap_or_default();
            Span {
                path: p,
                from: n,
                to: n,
            }
        })
        .collect();
    let top = spans.iter().map(|s| s.from).fold(f64::MIN, f64::max);
    let mut out = Vec::new();
    let mut n = 1;
    while (n as f64) < top {
        if !spans.iter().any(|s| s.from == n as f64) {
            let (anchor, after) = place(n as f64, &spans, descending);
            out.push(Missing {
                key: format!("ghost:{vkey}:{n}"),
                kind: "issue",
                group: name.to_string(),
                season: 0,
                number: n,
                position: None,
                code: code(n),
                title: String::new(),
                date: String::new(),
                anchor,
                after,
            });
        }
        n += 1;
    }
    out
}

/// The issues missing from each series some listed comic has (those with
/// a cover date still to come are left out); for a series without a list
/// of issues, the gaps between its files.
fn issues(known: &Known, items: &[PreviewItem], descending: bool, day: &str) -> Vec<Missing> {
    let listed: HashSet<&PathBuf> = items.iter().map(|i| &i.path).collect();
    let mut unlisted: Vec<(&String, Vec<(&PathBuf, &ComicMatch)>)> = Vec::new();
    let mut series: Vec<(&String, Vec<&Issue>, Vec<Span>)> = Vec::new();
    // In the list's order.
    let by_path: HashMap<&PathBuf, &Settled> = known.comics.iter().map(|c| (&c.path, c)).collect();
    for i in items {
        let Some(c) = by_path.get(&i.path).filter(|c| listed.contains(&c.path)) else {
            continue;
        };
        let Some(v) = known.volumes.get(&c.volume_key) else {
            continue;
        };
        if c.choice.as_deref() == Some("") && v.issues.is_empty() {
            match unlisted.iter_mut().find(|(k, _)| **k == c.volume_key) {
                Some(u) => u.1.push((&c.path, &c.own)),
                None => unlisted.push((&c.volume_key, vec![(&c.path, &c.own)])),
            }
            continue;
        }
        let Some(choice) = c.choice.as_deref().filter(|x| !x.is_empty()) else {
            continue;
        };
        if v.issues.is_empty() {
            continue;
        }
        let at = match series.iter().position(|(k, ..)| **k == c.volume_key) {
            Some(at) => at,
            None => {
                series.push((&c.volume_key, in_order(&v.issues), Vec::new()));
                series.len() - 1
            }
        };
        let s = &mut series[at];
        if let Some(k) = s.1.iter().position(|x| x.id == choice) {
            s.2.push(Span {
                path: &c.path,
                from: k as f64,
                to: k as f64,
            });
        }
    }
    let mut out = Vec::new();
    for (vkey, files) in unlisted {
        let v = &known.volumes[vkey].volume;
        out.extend(gaps(
            &named(&v.name, v.start_year),
            vkey,
            &files,
            descending,
        ));
    }
    for (vkey, order, files) in series {
        if files.is_empty() {
            continue;
        }
        let v = &known.volumes[vkey].volume;
        let have: HashSet<i64> = files.iter().map(|f| f.from as i64).collect();
        for (at, issue) in order.iter().enumerate() {
            if have.contains(&(at as i64))
                || (!issue.cover_date.is_empty() && issue.cover_date.as_str() > day)
            {
                continue;
            }
            let (anchor, after) = place(at as f64, &files, descending);
            out.push(Missing {
                key: format!("ghost:{vkey}:{}", issue.id),
                kind: "issue",
                group: named(&v.name, v.start_year),
                season: 0,
                number: at as i64,
                position: None,
                code: if issue.number.starts_with(|c: char| c.is_ascii_digit()) {
                    format!("#{}", issue.number)
                } else {
                    issue.number.clone()
                },
                title: issue.name.clone(),
                date: issue.cover_date.clone(),
                anchor,
                after,
            });
        }
    }
    out
}

/// The tracks missing from each album with a release whose songs are
/// listed. A folder whose songs were heard on more than one release (a
/// song from another edition) shows only those of the release most of
/// them are on, and a track whose disc and number a song of the folder
/// already has is not missing.
fn tracks(known: &Known, items: &[PreviewItem], descending: bool) -> Vec<Missing> {
    let listed: HashSet<&PathBuf> = items.iter().map(|i| &i.path).collect();
    let found: Vec<(&AlbumTracks, Vec<Span>)> = known
        .albums
        .iter()
        .filter(|a| !a.tracks.is_empty())
        .filter_map(|a| {
            let files: Vec<Span> = a
                .files
                .iter()
                .filter(|p| listed.contains(p))
                .filter_map(|p| {
                    let id = &known.songs.get(p)?.track_id;
                    let at = a.tracks.iter().position(|t| &t.id == id)? as f64;
                    Some(Span {
                        path: p,
                        from: at,
                        to: at,
                    })
                })
                .collect();
            (!files.is_empty()).then_some((a, files))
        })
        .collect();
    let mut most: HashMap<&PathBuf, usize> = HashMap::new();
    let mut taken: HashMap<&PathBuf, HashSet<(u32, u32)>> = HashMap::new();
    for (a, files) in &found {
        let m = most.entry(&a.dir).or_default();
        *m = (*m).max(files.len());
        let t = taken.entry(&a.dir).or_default();
        for f in files {
            let tr = &a.tracks[f.from as usize];
            t.insert((tr.disc, tr.position));
        }
    }
    let mut out = Vec::new();
    for (a, files) in &found {
        if files.len() < most[&a.dir] {
            continue;
        }
        let Some(m) = known.songs.get(files[0].path) else {
            continue;
        };
        let group = format!("{} - {}", m.album_artist, named(&m.album, m.year));
        let discs: HashSet<u32> = a.tracks.iter().map(|t| t.disc).collect();
        let have: HashSet<usize> = files.iter().map(|f| f.from as usize).collect();
        for (at, tr) in a.tracks.iter().enumerate() {
            if have.contains(&at) || taken[&a.dir].contains(&(tr.disc, tr.position)) {
                continue;
            }
            let n = format!("{:02}", tr.position);
            let (anchor, after) = place(at as f64, files, descending);
            out.push(Missing {
                // The same release in two folders (an album as FLAC and as
                // MP3) shows its missing tracks in each.
                key: format!(
                    "ghost:{}:{}:{}",
                    a.dir.display(),
                    a.release,
                    if tr.id.is_empty() {
                        at.to_string()
                    } else {
                        tr.id.clone()
                    }
                ),
                kind: "track",
                group: group.clone(),
                season: tr.disc,
                number: at as i64,
                position: Some(tr.position),
                code: if discs.len() > 1 {
                    format!("{}-{n}", tr.disc)
                } else {
                    n
                },
                title: tr.title.clone(),
                date: String::new(),
                anchor,
                after,
            });
        }
    }
    out
}

/// The released movies missing from each collection a listed movie is
/// in, in order of release.
fn movies(known: &Known, items: &[PreviewItem], descending: bool, day: &str) -> Vec<Missing> {
    let mut by_set: Vec<(u64, Vec<(&PathBuf, u64)>)> = Vec::new();
    for i in items {
        let Some(id) = known.movies.get(&i.path) else {
            continue;
        };
        let Some(set) = known.set_of.get(id).copied().flatten() else {
            continue;
        };
        if !known.sets.iter().any(|s| s.id == set) || !is_video(&i.original) {
            continue;
        }
        match by_set.iter_mut().find(|(s, _)| *s == set) {
            Some(b) => b.1.push((&i.path, *id)),
            None => by_set.push((set, vec![(&i.path, *id)])),
        }
    }
    let mut out = Vec::new();
    for (id, listed) in by_set {
        let Some(set) = known.sets.iter().find(|s| s.id == id) else {
            continue;
        };
        // Unknown dates last, as TMDB lists them.
        let mut order: Vec<_> = set.parts.iter().collect();
        let date = |d: &str| {
            if d.is_empty() {
                "9".to_string()
            } else {
                d.to_string()
            }
        };
        order.sort_by_key(|m| date(&m.release_date));
        let files: Vec<Span> = listed
            .iter()
            .filter_map(|(p, mid)| {
                let k = order.iter().position(|m| m.id == *mid)? as f64;
                Some(Span {
                    path: p,
                    from: k,
                    to: k,
                })
            })
            .collect();
        if files.is_empty() {
            continue;
        }
        let have: HashSet<usize> = files.iter().map(|f| f.from as usize).collect();
        for (k, m) in order.iter().enumerate() {
            if have.contains(&k) || m.release_date.is_empty() || m.release_date.as_str() > day {
                continue;
            }
            let (anchor, after) = place(k as f64, &files, descending);
            out.push(Missing {
                key: format!("ghost:set:{id}:{}", m.id),
                kind: "movie",
                group: set.name.clone(),
                season: 0,
                number: k as i64,
                position: None,
                code: m.release_date.get(..4).unwrap_or_default().to_string(),
                title: m.title.clone(),
                date: m.release_date.clone(),
                anchor,
                after,
            });
        }
    }
    out
}

/// The volumes missing between a book series' own files (its number in
/// the books' details or names), from 1 to the highest: no catalogue has
/// a reliable list of a series' books.
fn books(known: &Known, items: &[PreviewItem], descending: bool) -> Vec<Missing> {
    let fold = |s: &str| {
        renome_core::text::strip_accents(s)
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut series: Vec<(String, String, Vec<Span>)> = Vec::new();
    for i in items {
        let Some(g) = known.book_guesses.get(&i.path) else {
            continue;
        };
        let Some(n) = leading_number(&g.series_index).filter(|n| n.is_finite()) else {
            continue;
        };
        if g.series.trim().is_empty() {
            continue;
        }
        let k = fold(&g.series);
        let span = Span {
            path: &i.path,
            from: n,
            to: n,
        };
        match series.iter_mut().find(|(key, ..)| *key == k) {
            Some(s) => s.2.push(span),
            None => series.push((k, g.series.trim().to_string(), vec![span])),
        }
    }
    let mut out = Vec::new();
    for (k, name, files) in series {
        // Half volumes (2.5) are extras: they neither fill nor make a gap.
        let have: HashSet<i64> = files
            .iter()
            .filter(|f| f.from.fract() == 0.0)
            .map(|f| f.from as i64)
            .collect();
        let top = have.iter().max().copied().unwrap_or(0);
        for n in (1..top).filter(|n| !have.contains(n)) {
            let (anchor, after) = place(n as f64, &files, descending);
            out.push(Missing {
                key: format!("ghost:book:{k}:{n}"),
                kind: "book",
                group: name.clone(),
                season: 0,
                number: n,
                position: None,
                code: n.to_string(),
                title: String::new(),
                date: String::new(),
                anchor,
                after,
            });
        }
    }
    out
}

/// Lower case, without accents or punctuation (but `#`), for comparing names.
fn plain(s: &str) -> String {
    renome_core::text::strip_accents(&renome_core::text::plain(s))
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '#')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a name typed by hand is that missing item's: its code (S01E05
/// or 1x05, #7, v07, a track's number at the start) or, for a movie or a
/// book, its title.
fn names(m: &Missing, name: &str) -> bool {
    let n = plain(name);
    let num = format!("0*{}(?:[^0-9]|$)", m.number);
    let has_title = || {
        let t = plain(&m.title);
        t.chars().count() > 2 && n.contains(&t)
    };
    let re = |p: String| regex::Regex::new(&p).is_ok_and(|r| r.is_match(&n));
    match m.kind {
        "episode" => re(format!(
            "(?:^|[^a-z0-9])(?:s0*{s} ?e{num}|{s}x{num})",
            s = m.season
        )),
        "issue" => re(format!(r"(?:#|\bv|\bc|\bvol |\bn ?){num}")) || has_title(),
        "track" => re(format!("^(?:{} |{}-)?{num}", m.season, m.season)) || has_title(),
        _ => has_title(),
    }
}

/// The missing items a file renamed by hand now is: left out (the file
/// has to sit in the same folder as the item's place).
fn not_typed_in(missing: Vec<Missing>, items: &[PreviewItem]) -> Vec<Missing> {
    let typed: Vec<&PreviewItem> = items.iter().filter(|i| i.manual).collect();
    if typed.is_empty() {
        return missing;
    }
    missing
        .into_iter()
        .filter(|m| {
            let dir = m.anchor.parent();
            !typed
                .iter()
                .any(|i| i.path.parent() == dir && names(m, &i.new_name))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_as_a_page_does() {
        assert_eq!(leading_number("12"), Some(12.0));
        assert_eq!(leading_number("12.5"), Some(12.5));
        assert_eq!(leading_number("12a"), Some(12.0));
        assert_eq!(leading_number("½"), None);
        assert_eq!(leading_number(""), None);
    }

    #[test]
    fn a_gap_sits_before_the_next_file() {
        let a = PathBuf::from("/s/1");
        let c = PathBuf::from("/s/3");
        let spans = [
            Span {
                path: &a,
                from: 1.0,
                to: 1.0,
            },
            Span {
                path: &c,
                from: 3.0,
                to: 3.0,
            },
        ];
        assert_eq!(place(2.0, &spans, false), (c.clone(), false));
        // After the last one.
        assert_eq!(place(4.0, &spans, false), (c, true));
    }

    #[test]
    fn a_name_typed_by_hand_takes_its_place() {
        let m = Missing {
            key: String::new(),
            kind: "episode",
            group: String::new(),
            season: 1,
            number: 5,
            position: None,
            code: "S01E05".into(),
            title: String::new(),
            date: String::new(),
            anchor: PathBuf::new(),
            after: false,
        };
        assert!(names(&m, "Show - S01E05 - Title.mkv"));
        assert!(names(&m, "show 1x05.mkv"));
        assert!(!names(&m, "Show - S01E15.mkv"));
    }
}
