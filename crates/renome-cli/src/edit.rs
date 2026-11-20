//! `--edit PATH:KEY=VALUE,…`: the window's details editors (song tags,
//! book, comic, photo date). The fields typed replace the file's match,
//! or what the file says about itself when nothing was chosen, with the
//! same rules as the window: a book or comic retyped loses its catalogue
//! id, a song moved to another album loses its release's.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use renome_core::FileEntry;
use renome_core::musicbrainz::SongMatch;

use crate::text::t;

/// Each file's typed fields, in the order given.
pub type Edits = HashMap<PathBuf, BTreeMap<String, String>>;

use renome_services::flow::edit::Field;

/// The fields of the editor for a rule (`music`, `book`, `comic`,
/// `photo`; the window's), and the names `--edit` takes: the photo's
/// date also whole (`date=`).
pub fn fields_of(kind: &str) -> Vec<Field> {
    let mut out = renome_services::flow::edit::fields_of(kind).to_vec();
    if kind == "photo" {
        out.insert(
            0,
            Field {
                key: "date",
                numeric: false,
                wide: false,
                max_length: None,
            },
        );
    }
    out
}

/// The names of an editor's fields, for the help.
pub fn names(kind: &str) -> String {
    fields_of(kind)
        .iter()
        .map(|f| f.key)
        .collect::<Vec<_>>()
        .join(", ")
}

/// `PATH:KEY=VALUE,KEY=VALUE`: the path ends at the last `:` followed by
/// a field and `=` (so `C:\…` stays whole); a comma starts a new field
/// only before `KEY=` (so values keep their commas).
pub fn parse(specs: &[String], fields: &[Field]) -> Result<Edits, String> {
    let keys: Vec<&str> = fields.iter().map(|f| f.key).collect();
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
        let typed = out.entry(path).or_default();
        for (n, &from) in starts.iter().enumerate() {
            let to = starts.get(n + 1).map_or(rest.len(), |next| next - 1);
            let (key, value) = rest[from..to].split_once('=').unwrap();
            let value = value.trim();
            // Checked as the window's details window takes it.
            if let Some(f) = fields.iter().find(|f| f.key == key)
                && !edit::fits(f, value)
            {
                let n = f.max_length.unwrap_or_default();
                return Err(t(
                    "badEditNumber",
                    &[("key", &key), ("value", &value), ("n", &n)],
                ));
            }
            typed.insert(key.to_string(), value.to_string());
        }
    }
    Ok(out)
}

use renome_services::flow::edit;

/// The files of `edits` a details window edits (`flow::edit::rows`, as
/// the window's), in the list's order.
fn rows(kind: &str, edits: &Edits, items: &[renome_core::PreviewItem]) -> Vec<PathBuf> {
    let chosen = edits.keys().cloned().collect();
    edit::rows(kind, items, &chosen, &HashMap::new(), false)
}

pub fn songs(
    edits: &Edits,
    chosen: &mut HashMap<PathBuf, SongMatch>,
    files: &HashMap<&PathBuf, &FileEntry>,
    items: &[renome_core::PreviewItem],
    from_name: bool,
) {
    // What each starts from, as the window's details window (see
    // `flow::edit`).
    for path in rows("music", edits, items) {
        let changed = &edits[&path];
        let path = &path;
        let m = edit::song_start(
            chosen.get(path).cloned(),
            files.get(path).copied(),
            from_name,
        );
        chosen.insert(path.clone(), edit::song(m, changed));
    }
}

pub fn books(
    edits: &Edits,
    found: &mut crate::lookup::Books,
    items: &[renome_core::PreviewItem],
    names: &HashMap<&PathBuf, String>,
    audiobooks: bool,
) {
    // An audiobook's files are edited together (as in the window).
    for (path, changed) in edits {
        let one: std::collections::HashSet<PathBuf> = [path.clone()].into();
        for p in edit::rows("book", items, &one, &found.parts, audiobooks) {
            let name = names.get(&p).cloned().unwrap_or_default();
            let m = edit::book_start(
                found.chosen.get(&p).cloned(),
                found.guesses.get(&p),
                &p,
                &name,
            );
            found.chosen.insert(p, edit::book(m, changed));
        }
    }
}

pub fn comics(
    edits: &Edits,
    found: &mut crate::lookup::Comics,
    items: &[renome_core::PreviewItem],
    names: &HashMap<&PathBuf, String>,
    by_folder: bool,
) {
    for path in rows("comic", edits, items) {
        let changed = &edits[&path];
        let path = &path;
        let own = found
            .settled
            .iter()
            .find(|s| &s.path == path)
            .map(|s| s.own.clone());
        let name = names.get(path).cloned().unwrap_or_default();
        let is_dir = items.iter().any(|i| &i.path == path && i.is_dir);
        let m = edit::comic_start(
            found.chosen.get(path).cloned(),
            own.as_ref(),
            path,
            &name,
            is_dir,
            by_folder,
        );
        found.chosen.insert(path.clone(), edit::comic(m, changed));
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
        let e = parse(
            &["C:/a:b/x.mp3:title=Hello, world,year=1999".into()],
            &fields_of("music"),
        )
        .unwrap();
        let (p, f) = e.iter().next().unwrap();
        assert!(p.ends_with("C:/a:b/x.mp3"));
        assert_eq!(f["title"], "Hello, world");
        assert_eq!(f["year"], "1999");
        assert!(parse(&["x.mp3:year=abc".into()], &fields_of("music")).is_err());
        assert!(parse(&["x.mp3:year=19999".into()], &fields_of("music")).is_err());
        assert!(parse(&["x.mp3:nope=1".into()], &fields_of("music")).is_err());
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
        let m = renome_core::books::BookMatch {
            work: "OL1W".into(),
            title: "A".into(),
            edition_year: Some(2001),
            ..Default::default()
        };
        let changed = BTreeMap::from([
            ("title".to_string(), "B".to_string()),
            ("year".to_string(), "1990".to_string()),
        ]);
        let b = edit::book(edit::book_start(Some(m), None, &p, "b.epub"), &changed);
        assert_eq!(b.title, "B");
        assert_eq!(b.work, "");
        assert_eq!((b.year, b.edition_year), (Some(1990), None));
    }
}
