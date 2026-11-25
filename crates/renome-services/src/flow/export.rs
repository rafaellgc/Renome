//! The lists saved as CSV (the window's "Export list" and "Export the
//! missing", the command line's `--csv` and `--missing-csv`): which
//! columns, which rows, in what order, where and under what name. Words
//! come as keys of the page's texts, which each side translates.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use renome_core::Preview;
use renome_core::details::Details;
use serde::Serialize;

use super::missing::Missing;

/// A CSV cell: text as it is, or a key of the page's texts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Cell {
    Text(String),
    Key { key: String },
}

fn key(k: impl Into<String>) -> Cell {
    Cell::Key { key: k.into() }
}

fn text(s: impl Into<String>) -> Cell {
    Cell::Text(s.into())
}

/// The key of an optional column's title.
pub fn column_label(column: &str) -> String {
    let mut c = column.chars();
    let cap = c
        .next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default();
    format!("col{cap}")
}

/// The list as shown: folder, name, new name, where it goes, status,
/// problem, and the optional columns chosen (`columns`) that make sense
/// with the rules there, as the list shows them.
pub fn list(
    p: &Preview,
    details: &HashMap<PathBuf, Details>,
    columns: &[&str],
    lang: &str,
) -> Vec<Vec<Cell>> {
    let mut head: Vec<Cell> = [
        "csv.folder",
        "csv.original",
        "csv.new",
        "csv.dest",
        "csv.status",
        "csv.problem",
    ]
    .into_iter()
    .map(key)
    .collect();
    head.extend(columns.iter().map(|c| key(column_label(c))));
    let mut rows = vec![head];
    for i in &p.items {
        let status = serde_json::to_value(i.status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let problem = i
            .problem
            .and_then(|pr| serde_json::to_value(pr).ok())
            .and_then(|v| v.as_str().map(|s| key(format!("problem.{s}"))))
            .unwrap_or_else(|| text(""));
        let mut row = vec![
            text(i.dir.display().to_string()),
            text(i.original.clone()),
            text(i.new_name.clone()),
            text(i.folder.clone().unwrap_or_default()),
            key(format!("csv.{status}")),
            problem,
        ];
        let d = details.get(&i.path);
        row.extend(
            columns
                .iter()
                .map(|c| text(d.map(|d| d.text(c, lang)).unwrap_or_default())),
        );
        rows.push(row);
    }
    rows
}

/// The missing items saved: where (the folder their files share, above
/// any season folder), under what name (the show's, with the kind's
/// default name: a key of the texts), and the rows, by show (or series,
/// album, collection), season (or disc) and number; each kind with its
/// own columns, mixed ones with those they all share.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingCsv {
    pub dir: Option<PathBuf>,
    /// The group in the name (one group only), made safe for a file name.
    pub group: Option<String>,
    /// Key of the name's text (`defaultMissingName`…).
    pub name_key: &'static str,
    pub rows: Vec<Vec<Cell>>,
}

/// How a kind of missing item makes its CSV row.
type Row = fn(&Missing) -> Vec<String>;

pub fn missing(list: &[Missing], dir_of: &dyn Fn(&Path) -> Option<PathBuf>) -> MissingCsv {
    let mut kinds: Vec<&str> = list.iter().map(|m| m.kind).collect();
    kinds.sort_unstable();
    kinds.dedup();
    let mut groups: Vec<&str> = list.iter().map(|m| m.group.as_str()).collect();
    groups.sort_unstable();
    groups.dedup();
    let one = (kinds.len() == 1).then(|| kinds[0]);
    let group = (groups.len() == 1).then(|| safe_name(groups[0]));
    let mut sorted: Vec<&Missing> = list.iter().collect();
    sorted.sort_by(|a, b| {
        a.group
            .to_lowercase()
            .cmp(&b.group.to_lowercase())
            .then(a.season.cmp(&b.season))
            .then(a.number.cmp(&b.number))
    });
    let (head, row) = columns(one);
    let mut rows = vec![head.into_iter().map(key).collect::<Vec<_>>()];
    rows.extend(
        sorted
            .iter()
            .map(|m| row(m).into_iter().map(Cell::Text).collect()),
    );
    MissingCsv {
        dir: shared_folder(list.iter().filter_map(|m| dir_of(&m.anchor))),
        group,
        name_key: name_key(one),
        rows,
    }
}

/// The key of the file's name for the one kind of what is missing (or
/// for a mix).
fn name_key(one: Option<&str>) -> &'static str {
    match one {
        Some("episode") => "defaultMissingName",
        Some("issue") => "defaultMissingIssuesName",
        Some("track") => "defaultMissingTracksName",
        Some("movie") => "defaultMissingMoviesName",
        Some("book") => "defaultMissingBooksName",
        _ => "defaultMissingItemsName",
    }
}

/// A group's name as a part of a file name.
fn safe_name(group: &str) -> String {
    group
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

/// A kind's columns (the keys of their titles) and how it makes a row.
fn columns(one: Option<&str>) -> (Vec<&'static str>, Row) {
    match one {
        Some("episode") => (
            vec![
                "csv.show",
                "csv.season",
                "csv.episode",
                "csv.code",
                "csv.title",
                "csv.airDate",
            ],
            |m| {
                vec![
                    m.group.clone(),
                    m.season.to_string(),
                    m.number.to_string(),
                    m.code.clone(),
                    m.title.clone(),
                    m.date.clone(),
                ]
            },
        ),
        Some("issue") => (
            vec!["csv.series", "csv.number", "csv.title", "csv.coverDate"],
            |m| {
                vec![
                    m.group.clone(),
                    m.code.trim_start_matches('#').to_string(),
                    m.title.clone(),
                    m.date.clone(),
                ]
            },
        ),
        Some("track") => (
            vec!["csv.album", "csv.disc", "csv.track", "csv.title"],
            |m| {
                vec![
                    m.group.clone(),
                    m.season.to_string(),
                    m.position.map(|p| p.to_string()).unwrap_or_default(),
                    m.title.clone(),
                ]
            },
        ),
        Some("movie") => (
            vec!["csv.collection", "csv.title", "csv.releaseDate"],
            |m| vec![m.group.clone(), m.title.clone(), m.date.clone()],
        ),
        Some("book") => (vec!["csv.series", "csv.volume"], |m| {
            vec![m.group.clone(), m.code.clone()]
        }),
        _ => (
            vec!["csv.group", "csv.code", "csv.title", "csv.date"],
            |m| {
                vec![
                    m.group.clone(),
                    m.code.clone(),
                    m.title.clone(),
                    m.date.clone(),
                ]
            },
        ),
    }
}

/// The folder some folders share, above a season folder (the show's).
fn shared_folder(dirs: impl Iterator<Item = PathBuf>) -> Option<PathBuf> {
    let mut common: Option<PathBuf> = None;
    for d in dirs {
        common = Some(match common {
            None => d,
            Some(c) => {
                let mut out = PathBuf::new();
                for (a, b) in c.components().zip(d.components()) {
                    if a != b {
                        break;
                    }
                    out.push(a);
                }
                out
            }
        });
    }
    let mut dir = common?;
    if dir.parent().is_some()
        && dir
            .file_name()
            .is_some_and(|n| renome_core::tv::folder_season(&n.to_string_lossy()).is_some())
    {
        dir.pop();
    }
    (!dir.as_os_str().is_empty()).then_some(dir)
}

/// The cells as text, each key translated by `t`.
pub fn texts(rows: &[Vec<Cell>], t: &dyn Fn(&str) -> String) -> Vec<Vec<String>> {
    rows.iter()
        .map(|r| {
            r.iter()
                .map(|c| match c {
                    Cell::Text(s) => s.clone(),
                    Cell::Key { key } => t(key),
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_in_the_show_folder() {
        let m = |s: u32, n: i64, anchor: &str| Missing {
            key: format!("{s}:{n}"),
            kind: "episode",
            group: "A Ilha (2001)".into(),
            season: s,
            number: n,
            position: None,
            code: format!("S{s:02}E{n:02}"),
            title: String::new(),
            date: String::new(),
            anchor: PathBuf::from(anchor),
            after: true,
        };
        let list = vec![
            m(2, 1, "/v/A/Season 2/x.mkv"),
            m(1, 3, "/v/A/Season 1/y.mkv"),
        ];
        let csv = missing(&list, &|p| p.parent().map(Path::to_path_buf));
        assert_eq!(csv.dir, Some(PathBuf::from("/v/A")));
        assert_eq!(csv.group.as_deref(), Some("A Ilha (2001)"));
        assert_eq!(csv.name_key, "defaultMissingName");
        // By season, then number.
        assert_eq!(csv.rows[1][1], Cell::Text("1".into()));
    }
}
