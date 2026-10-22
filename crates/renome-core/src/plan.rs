//! Preview: applies the rules to the visible files and flags conflicts.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::apply::Operation;
use crate::files::{FileEntry, join_name, natural_cmp};
use crate::filter::Filter;
use crate::rules::{Ctx, Pipeline, RuleError, Step};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Unchanged,
    Ok,
    Conflict,
}

/// Reason for a conflict (translated by the UI).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Problem {
    /// Empty name, `.` or `..`.
    Empty,
    /// Contains `/` or a null character.
    Slash,
    /// Longer than 255 bytes.
    TooLong,
    /// On Windows: contains `\ : * ? " < > |` or ends in a dot or space.
    WindowsChars,
    /// Another file in the batch would get the same name.
    Duplicate,
    /// A file with that name already exists outside the batch.
    Exists,
}

/// How the list is shown: which files are included and in what order.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct View {
    pub filter: String,
    /// Sorts the preview by new name (otherwise keeps the list order).
    pub sort_by_new_name: bool,
    pub descending: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewItem {
    pub path: PathBuf,
    pub dir: PathBuf,
    pub size: u64,
    pub modified: Option<i64>,
    pub original: String,
    pub new_name: String,
    pub status: Status,
    pub problem: Option<Problem>,
    /// Non-blocking warning: contains characters Windows does not accept.
    pub windows_unsafe: bool,
    pub manual: bool,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub items: Vec<PreviewItem>,
    pub rule_errors: Vec<RuleError>,
    pub filter_error: Option<String>,
    /// Total files in the list, including those hidden by the filter.
    pub total: usize,
    pub changed: usize,
    pub conflicts: usize,
}

impl Preview {
    /// Operations to run (only changed rows without conflicts).
    pub fn operations(&self) -> Vec<Operation> {
        self.items
            .iter()
            .filter(|i| i.status == Status::Ok)
            .map(|i| Operation {
                from: i.path.clone(),
                to: i.path.with_file_name(&i.new_name),
            })
            .collect()
    }
}

const WINDOWS_UNSAFE: &[char] = &['\\', ':', '*', '?', '"', '<', '>', '|'];

fn validate(name: &str) -> Option<Problem> {
    if name.is_empty() || name == "." || name == ".." {
        Some(Problem::Empty)
    } else if name.contains(['/', '\0']) {
        Some(Problem::Slash)
    } else if cfg!(windows)
        && (name.contains(WINDOWS_UNSAFE) || name.ends_with(['.', ' ']))
    {
        Some(Problem::WindowsChars)
    } else if name.len() > 255 {
        Some(Problem::TooLong)
    } else {
        None
    }
}

#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::symlink_metadata(a), std::fs::symlink_metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
        _ => false,
    }
}

/// Key for comparing paths: the default file systems on Windows and macOS
/// are case-insensitive, so `A.txt` and `a.txt` are the same target.
fn path_key(p: &Path) -> PathBuf {
    if cfg!(any(windows, target_os = "macos")) {
        PathBuf::from(p.to_string_lossy().to_lowercase())
    } else {
        p.to_path_buf()
    }
}

/// Without inodes (Windows), compares canonical paths, which there carry
/// the real on-disk casing.
#[cfg(not(unix))]
fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Computes the preview. `overrides` maps path → hand-edited name.
/// Files hidden by the filter are left out (and will not be renamed);
/// numbering only counts the visible ones.
pub fn preview(
    files: &[FileEntry],
    steps: &[Step],
    overrides: &HashMap<PathBuf, String>,
    view: &View,
) -> Preview {
    let pipeline = Pipeline::new(steps);
    let now = Local::now();
    let (filter, filter_error) = match Filter::new(&view.filter) {
        Ok(f) => (f, None),
        Err(e) => (Filter::default(), Some(e)),
    };

    let mut items: Vec<PreviewItem> = files
        .iter()
        .filter(|f| filter.matches(&f.file_name()))
        .enumerate()
        .map(|(index, file)| {
            let original = file.file_name();
            let (new_name, manual) = match overrides.get(&file.path) {
                Some(name) => (name.clone(), true),
                None => {
                    let (stem, ext) = pipeline.run(&Ctx { index, file, now });
                    (join_name(&stem, &ext), false)
                }
            };
            let problem = validate(&new_name);
            let status = if problem.is_some() {
                Status::Conflict
            } else if new_name == original {
                Status::Unchanged
            } else {
                Status::Ok
            };
            let windows_unsafe = new_name.contains(WINDOWS_UNSAFE);
            PreviewItem {
                path: file.path.clone(),
                dir: file.dir.clone(),
                size: file.size,
                modified: file.modified,
                original,
                new_name,
                status,
                problem,
                windows_unsafe,
                manual,
            }
        })
        .collect();

    // Repeated targets within the batch.
    let mut targets: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for (i, item) in items.iter().enumerate() {
        if item.status != Status::Conflict {
            targets
                .entry(path_key(&item.path.with_file_name(&item.new_name)))
                .or_default()
                .push(i);
        }
    }
    for idx in targets.values().filter(|v| v.len() > 1) {
        for &i in idx {
            items[i].status = Status::Conflict;
            items[i].problem = Some(Problem::Duplicate);
        }
    }

    // Target already exists on disk and is not moving away. Repeat until
    // stable, since a conflict keeps that file from being moved.
    loop {
        let moving: HashSet<PathBuf> = items
            .iter()
            .filter(|i| i.status == Status::Ok)
            .map(|i| path_key(&i.path))
            .collect();
        let mut blocked = Vec::new();
        for (i, item) in items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.status == Status::Ok)
        {
            let target = item.path.with_file_name(&item.new_name);
            let exists = std::fs::symlink_metadata(&target).is_ok();
            // Case-only change on a case-insensitive file system:
            // the "target" is the file itself.
            if exists && !moving.contains(&path_key(&target)) && !same_file(&target, &item.path) {
                blocked.push(i);
            }
        }
        if blocked.is_empty() {
            break;
        }
        for i in blocked {
            items[i].status = Status::Conflict;
            items[i].problem = Some(Problem::Exists);
        }
    }

    if view.sort_by_new_name {
        items.sort_by(|a, b| {
            let ord = natural_cmp(&a.new_name, &b.new_name);
            if view.descending { ord.reverse() } else { ord }
        });
    }

    let changed = items.iter().filter(|i| i.status == Status::Ok).count();
    let conflicts = items
        .iter()
        .filter(|i| i.status == Status::Conflict)
        .count();
    Preview {
        items,
        rule_errors: pipeline.errors,
        filter_error,
        total: files.len(),
        changed,
        conflicts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Rule, Target};
    use std::fs;

    fn entries(dir: &Path, names: &[&str]) -> Vec<FileEntry> {
        names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                fs::write(&p, n).unwrap();
                FileEntry::from_path(&p, &fs::metadata(&p).unwrap()).unwrap()
            })
            .collect()
    }

    fn replace(find: &str, with: &str) -> Vec<Step> {
        vec![Step {
            enabled: true,
            rule: Rule::Replace {
                find: find.into(),
                replace: with.into(),
                case_sensitive: true,
                first_only: false,
                target: Target::Name,
            },
        }]
    }

    #[test]
    fn duplicates_conflict() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["a1.txt", "a2.txt", "b.txt"]);
        let p = preview(&files, &replace_digits(), &HashMap::new(), &View::default());
        assert_eq!(p.items[0].status, Status::Conflict);
        assert_eq!(p.items[1].status, Status::Conflict);
        assert_eq!(p.items[2].status, Status::Unchanged);
    }

    fn replace_digits() -> Vec<Step> {
        vec![Step {
            enabled: true,
            rule: Rule::Regex {
                pattern: r"\d".into(),
                replace: "".into(),
                case_sensitive: true,
                first_only: false,
                target: Target::Name,
            },
        }]
    }

    #[test]
    fn existing_file_conflict_unless_moving() {
        let d = tempfile::tempdir().unwrap();
        // "x" -> "y" blocked: y.txt exists and is not in the batch.
        let files = entries(d.path(), &["x.txt"]);
        fs::write(d.path().join("y.txt"), "").unwrap();
        let p = preview(
            &files,
            &replace("x", "y"),
            &HashMap::new(),
            &View::default(),
        );
        assert_eq!(p.items[0].status, Status::Conflict);

        // Swapping a <-> b within the batch is allowed.
        let files = entries(d.path(), &["a.md", "b.md"]);
        let mut ov = HashMap::new();
        ov.insert(files[0].path.clone(), "b.md".to_string());
        ov.insert(files[1].path.clone(), "a.md".to_string());
        let p = preview(&files, &[], &ov, &View::default());
        assert!(
            p.items.iter().all(|i| i.status == Status::Ok),
            "{:?}",
            p.items
        );
    }

    #[test]
    fn invalid_names() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["a.txt"]);
        let p = preview(
            &files,
            &replace("a", "x/y"),
            &HashMap::new(),
            &View::default(),
        );
        assert_eq!(p.items[0].status, Status::Conflict);
        let p = preview(
            &files,
            &replace("a", "a:b"),
            &HashMap::new(),
            &View::default(),
        );
        assert_eq!(p.items[0].status, Status::Ok);
        assert!(p.items[0].windows_unsafe);
    }

    #[test]
    fn filter_limits_batch_and_numbering() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["a.jpg", "b.txt", "c.jpg"]);
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Number {
                template: "{n}".into(),
                start: 1,
                step: 1,
                digits: 1,
            },
        }];
        let view = View {
            filter: "*.jpg".into(),
            ..Default::default()
        };
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert_eq!(p.total, 3);
        let names: Vec<_> = p.items.iter().map(|i| i.new_name.as_str()).collect();
        assert_eq!(names, ["1.jpg", "2.jpg"]);
        assert_eq!(p.operations().len(), 2);
    }

    #[test]
    fn sort_by_new_name() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["b.txt", "a.txt"]);
        let mut ov = HashMap::new();
        ov.insert(files[1].path.clone(), "z.txt".to_string());
        let view = View {
            sort_by_new_name: true,
            ..Default::default()
        };
        let p = preview(&files, &[], &ov, &view);
        assert_eq!(p.items[0].new_name, "b.txt");
        assert_eq!(p.items[1].new_name, "z.txt");
    }
}
