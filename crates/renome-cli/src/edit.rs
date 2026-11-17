//! `--edit PATH:KEY=VALUE,…`: the window's details editors (song tags,
//! book, comic, photo date). The fields typed replace the file's match,
//! or what the file says about itself when nothing was chosen, with the
//! same rules as the window: a book or comic retyped loses its catalogue
//! id, a song moved to another album loses its release's.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use renome_core::FileEntry;
use renome_core::books::BookMatch;
use renome_core::comics::ComicMatch;
use renome_core::musicbrainz::SongMatch;

use crate::text::t;

pub const SONG: &[&str] = &[
    "title",
    "artist",
    "albumArtist",
    "genre",
    "album",
    "year",
    "track",
    "trackTotal",
    "disc",
    "discTotal",
];
pub const BOOK: &[&str] = &[
    "title",
    "authors",
    "year",
    "publisher",
    "language",
    "series",
    "seriesIndex",
    "isbn",
];
pub const COMIC: &[&str] = &[
    "series",
    "seriesYear",
    "volume",
    "number",
    "title",
    "year",
    "publisher",
    "writer",
    "artist",
];
pub const PHOTO: &[&str] = &["date", "day", "month", "year", "hour", "minute"];
const NUMERIC: &[&str] = &[
    "year",
    "track",
    "trackTotal",
    "disc",
    "discTotal",
    "seriesYear",
    "volume",
    "day",
    "month",
    "hour",
    "minute",
];

/// Each file's typed fields, in the order given.
pub type Edits = HashMap<PathBuf, BTreeMap<String, String>>;

/// The fields of the editor for a rule (`music`, `book`, `comic`, `photo`).
pub fn fields_of(kind: &str) -> &'static [&'static str] {
    match kind {
        "music" => SONG,
        "book" => BOOK,
        "comic" => COMIC,
        "photo" => PHOTO,
        _ => &[],
    }
}

/// `PATH:KEY=VALUE,KEY=VALUE`: the path ends at the last `:` followed by
/// a field and `=` (so `C:\…` stays whole); a comma starts a new field
/// only before `KEY=` (so values keep their commas).
pub fn parse(specs: &[String], keys: &[&str]) -> Result<Edits, String> {
    let starts_field = |s: &str| {
        keys.iter()
            .any(|k| s.strip_prefix(k).is_some_and(|r| r.starts_with('=')))
    };
    let mut out: Edits = HashMap::new();
    for spec in specs {
        let cut = spec
            .char_indices()
            .rfind(|(i, c)| *c == ':' && starts_field(&spec[i + 1..]))
            .map(|(i, _)| i)
            .ok_or_else(|| t("badEdit", &[("spec", spec), ("keys", &keys.join(", "))]))?;
        let path = std::path::absolute(&spec[..cut]).map_err(|e| e.to_string())?;
        let rest = &spec[cut + 1..];
        let mut starts: Vec<usize> = vec![0];
        starts.extend(
            rest.char_indices()
                .filter(|(i, c)| *c == ',' && starts_field(&rest[i + 1..]))
                .map(|(i, _)| i + 1),
        );
        let fields = out.entry(path).or_default();
        for (n, &from) in starts.iter().enumerate() {
            let to = starts.get(n + 1).map_or(rest.len(), |next| next - 1);
            let (key, value) = rest[from..to].split_once('=').unwrap();
            let value = value.trim();
            if NUMERIC.contains(&key) && !value.chars().all(|c| c.is_ascii_digit()) {
                return Err(t("badEditNumber", &[("key", &key), ("value", &value)]));
            }
            fields.insert(key.to_string(), value.to_string());
        }
    }
    Ok(out)
}

fn int<T: std::str::FromStr>(v: &str) -> Option<T> {
    v.parse().ok()
}

pub fn songs(
    edits: &Edits,
    chosen: &mut HashMap<PathBuf, SongMatch>,
    files: &HashMap<&PathBuf, &FileEntry>,
    from_name: bool,
) {
    for (path, changed) in edits {
        let mut m = chosen.get(path).cloned().unwrap_or_else(|| {
            files
                .get(path)
                .and_then(|f| {
                    renome_core::rules::song_tags(f, from_name)
                        .or_else(|| renome_core::rules::any_song_tags(f))
                })
                .map(|t| SongMatch::from_tags(&t))
                .unwrap_or_else(|| SongMatch {
                    typed: true,
                    ..SongMatch::default()
                })
        });
        for (k, v) in changed {
            match k.as_str() {
                "year" => m.year = int(v),
                "track" => m.track = int(v).unwrap_or(0),
                "trackTotal" => m.track_total = int(v).unwrap_or(0),
                "disc" => m.disc = int(v).unwrap_or(0),
                "discTotal" => m.disc_total = int(v).unwrap_or(0),
                "title" => m.title = v.clone(),
                "artist" => m.artist = v.clone(),
                "albumArtist" => m.album_artist = v.clone(),
                "album" => m.album = v.clone(),
                "genre" => m.genre = v.clone(),
                _ => {}
            }
        }
        // Another album: the release's ids no longer tell it.
        if m.typed
            || m.release.is_empty()
            || changed.contains_key("album")
            || changed.contains_key("albumArtist")
        {
            m.typed = true;
            m.release.clear();
            m.recording.clear();
            m.track_id.clear();
            m.release_group.clear();
            m.artist_ids.clear();
            m.album_artist_ids.clear();
        }
        chosen.insert(path.clone(), m);
    }
}

/// What a book says about itself, as a start for typing (no catalogue).
fn from_guess(g: &renome_core::books::Guess) -> BookMatch {
    let mut extra = renome_core::fields::Extra::new();
    if !g.language.is_empty() {
        extra.insert("language".into(), g.language.clone());
    }
    BookMatch {
        title: g.title.clone(),
        authors: g
            .author
            .split([';', '&'])
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .map(String::from)
            .collect(),
        year: g.year,
        publisher: g.publisher.clone(),
        isbn: g.isbns.first().cloned().unwrap_or_default(),
        series: g.series.clone(),
        series_index: g.series_index.clone(),
        extra,
        ..BookMatch::default()
    }
}

pub fn books(
    edits: &Edits,
    chosen: &mut HashMap<PathBuf, BookMatch>,
    names: &HashMap<&PathBuf, String>,
) {
    for (path, changed) in edits {
        let mut m = chosen.get(path).cloned().unwrap_or_else(|| {
            let stem = names
                .get(path)
                .map(|n| {
                    Path::new(n)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .unwrap_or_default();
            from_guess(&renome_core::books::guess(
                &stem,
                &renome_core::ebook::read(path),
            ))
        });
        for (k, v) in changed {
            match k.as_str() {
                "authors" => {
                    m.authors = v
                        .split(';')
                        .map(str::trim)
                        .filter(|a| !a.is_empty())
                        .map(String::from)
                        .collect()
                }
                "year" => {
                    m.year = int(v);
                    // One year typed: the edition's goes.
                    m.edition_year = None;
                }
                "language" => {
                    if v.is_empty() {
                        m.extra.remove("language");
                    } else {
                        m.extra.insert("language".into(), v.clone());
                    }
                }
                "title" => m.title = v.clone(),
                "publisher" => m.publisher = v.clone(),
                "series" => m.series = v.clone(),
                "seriesIndex" => m.series_index = v.clone(),
                "isbn" => m.isbn = v.clone(),
                _ => {}
            }
        }
        // Another book: the catalogue's work (and its cover) no longer tell it.
        if changed.contains_key("title") || changed.contains_key("authors") {
            m.work.clear();
        }
        chosen.insert(path.clone(), m);
    }
}

pub fn comics(
    edits: &Edits,
    chosen: &mut HashMap<PathBuf, ComicMatch>,
    names: &HashMap<&PathBuf, String>,
    by_folder: bool,
) {
    for (path, changed) in edits {
        let mut m = chosen.get(path).cloned().unwrap_or_else(|| {
            let stem = names.get(path).map(|n| {
                Path::new(n)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
            renome_core::comics::from_path(path, stem.as_deref(), by_folder).unwrap_or_default()
        });
        for (k, v) in changed {
            match k.as_str() {
                "seriesYear" => m.series_year = int(v),
                "volume" => m.volume = int(v),
                "year" => m.year = int(v),
                "series" => m.series = v.clone(),
                "number" => m.number = v.clone(),
                "title" => m.title = v.clone(),
                "publisher" => m.publisher = v.clone(),
                "writer" => m.writer = v.clone(),
                "artist" => m.artist = v.clone(),
                _ => {}
            }
        }
        // The catalogue's issue no longer tells it.
        if changed.contains_key("series") || changed.contains_key("number") {
            m.id.clear();
        }
        chosen.insert(path.clone(), m);
    }
}

/// Photo dates as the photo rule takes them (`2018-03-27`, `… 14:30`;
/// empty: back to what the file says): `date=` whole, or its parts.
pub fn photo_dates(edits: &Edits, dates: &mut HashMap<PathBuf, String>) -> Result<(), String> {
    for (path, changed) in edits {
        let text = match changed.get("date") {
            Some(d) => d.trim().to_string(),
            None => {
                let old = dates.get(path).cloned().unwrap_or_default();
                let (y, mo, d, h, mi) = parts(&old);
                let get = |k: &str, old: String| changed.get(k).cloned().unwrap_or(old);
                joined(
                    &get("year", y),
                    &get("month", mo),
                    &get("day", d),
                    &get("hour", h),
                    &get("minute", mi),
                )
                .ok_or_else(|| t("photoEditInvalid", &[]))?
            }
        };
        if text.is_empty() {
            dates.remove(path);
            continue;
        }
        let (y, mo, d, h, mi) = parts(&text);
        let valid = joined(&y, &mo, &d, &h, &mi).filter(|j| *j == text && !y.is_empty());
        if valid.is_none() {
            return Err(t("photoEditInvalid", &[]));
        }
        dates.insert(path.clone(), text);
    }
    Ok(())
}

fn parts(text: &str) -> (String, String, String, String, String) {
    let (day, time) = text.split_once(' ').unwrap_or((text, ""));
    let d: Vec<&str> = day.split('-').collect();
    let tm: Vec<&str> = time.split(':').collect();
    if d.len() != 3 {
        return Default::default();
    }
    let s = |v: Option<&&str>| v.map(|x| x.to_string()).unwrap_or_default();
    (
        d[0].into(),
        d[1].into(),
        d[2].into(),
        s(tm.first()).trim().to_string(),
        s(tm.get(1)),
    )
}

/// The parts as one date; `""` when all are empty; `None` when they make
/// no date (as the window checks them).
fn joined(y: &str, mo: &str, d: &str, h: &str, mi: &str) -> Option<String> {
    if [y, mo, d, h, mi].iter().all(|v| v.is_empty()) {
        return Some(String::new());
    }
    let (yy, mm, dd): (i32, u32, u32) = (y.parse().ok()?, mo.parse().ok()?, d.parse().ok()?);
    if y.len() != 4 || yy < 1900 {
        return None;
    }
    chrono::NaiveDate::from_ymd_opt(yy, mm, dd)?;
    let day = format!("{yy}-{mm:02}-{dd:02}");
    if h.is_empty() && mi.is_empty() {
        return Some(day);
    }
    let (hh, mn): (u32, u32) = (
        if h.is_empty() { 0 } else { h.parse().ok()? },
        if mi.is_empty() { 0 } else { mi.parse().ok()? },
    );
    (hh <= 23 && mn <= 59).then(|| format!("{day} {hh:02}:{mn:02}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_keep_colons_and_values_keep_commas() {
        let e = parse(&["C:/a:b/x.mp3:title=Hello, world,year=1999".into()], SONG).unwrap();
        let (p, f) = e.iter().next().unwrap();
        assert!(p.ends_with("C:/a:b/x.mp3"));
        assert_eq!(f["title"], "Hello, world");
        assert_eq!(f["year"], "1999");
        assert!(parse(&["x.mp3:year=abc".into()], SONG).is_err());
        assert!(parse(&["x.mp3:nope=1".into()], SONG).is_err());
    }

    #[test]
    fn photo_dates_are_checked() {
        let p = PathBuf::from("/a.jpg");
        let mut dates = HashMap::new();
        let edit = |k: &str, v: &str| {
            HashMap::from([(p.clone(), BTreeMap::from([(k.to_string(), v.to_string())]))])
        };
        photo_dates(&edit("date", "2018-03-27 14:30"), &mut dates).unwrap();
        assert_eq!(dates[&p], "2018-03-27 14:30");
        photo_dates(&edit("day", "28"), &mut dates).unwrap();
        assert_eq!(dates[&p], "2018-03-28 14:30");
        assert!(photo_dates(&edit("date", "2018-02-30"), &mut dates).is_err());
        photo_dates(&edit("date", ""), &mut dates).unwrap();
        assert!(dates.is_empty());
    }

    #[test]
    fn a_retyped_book_loses_its_work() {
        let p = PathBuf::from("/b.epub");
        let mut chosen = HashMap::from([(
            p.clone(),
            BookMatch {
                work: "OL1W".into(),
                title: "A".into(),
                edition_year: Some(2001),
                ..Default::default()
            },
        )]);
        let edits = HashMap::from([(
            p.clone(),
            BTreeMap::from([
                ("title".to_string(), "B".to_string()),
                ("year".to_string(), "1990".to_string()),
            ]),
        )]);
        books(&edits, &mut chosen, &HashMap::new());
        assert_eq!(chosen[&p].title, "B");
        assert_eq!(chosen[&p].work, "");
        assert_eq!(
            (chosen[&p].year, chosen[&p].edition_year),
            (Some(1990), None)
        );
    }
}
