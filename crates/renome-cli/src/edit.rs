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

use renome_services::flow::edit;

pub fn songs(
    edits: &Edits,
    chosen: &mut HashMap<PathBuf, SongMatch>,
    files: &HashMap<&PathBuf, &FileEntry>,
    from_name: bool,
) {
    for (path, changed) in edits {
        let m = chosen.get(path).cloned().unwrap_or_else(|| {
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
        chosen.insert(path.clone(), edit::song(m, changed));
    }
}

pub fn books(
    edits: &Edits,
    chosen: &mut HashMap<PathBuf, BookMatch>,
    names: &HashMap<&PathBuf, String>,
) {
    for (path, changed) in edits {
        let m = chosen.get(path).cloned().unwrap_or_else(|| {
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
            edit::book_from_guess(&renome_core::books::guess(
                &stem,
                &renome_core::ebook::read(path),
            ))
        });
        chosen.insert(path.clone(), edit::book(m, changed));
    }
}

pub fn comics(
    edits: &Edits,
    chosen: &mut HashMap<PathBuf, ComicMatch>,
    names: &HashMap<&PathBuf, String>,
    by_folder: bool,
) {
    for (path, changed) in edits {
        let m = chosen.get(path).cloned().unwrap_or_else(|| {
            let stem = names.get(path).map(|n| {
                Path::new(n)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
            renome_core::comics::from_path(path, stem.as_deref(), by_folder).unwrap_or_default()
        });
        chosen.insert(path.clone(), edit::comic(m, changed));
    }
}

/// Photo dates as the photo rule takes them (`2018-03-27`, `… 14:30`;
/// empty: back to what the file says): `date=` whole, or its parts.
pub fn photo_dates(edits: &Edits, dates: &mut HashMap<PathBuf, String>) -> Result<(), String> {
    for (path, changed) in edits {
        let old = dates.get(path).cloned().unwrap_or_default();
        match edit::photo_date(&old, changed) {
            None => return Err(t("photoEditInvalid", &[])),
            Some(text) if text.is_empty() => {
                dates.remove(path);
            }
            Some(text) => {
                dates.insert(path.clone(), text);
            }
        }
    }
    Ok(())
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
