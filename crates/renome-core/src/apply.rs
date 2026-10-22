//! Safe two-phase renaming, plus a history for undo.
//!
//! Phase 1 moves each source to a temporary name in the same folder; phase 2
//! moves each temporary to its target. This makes swaps (a↔b), cycles and
//! case-only changes work, and nothing is ever overwritten.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Operation {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Error kind (translated by the UI); `detail` holds the OS message.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    TempExists,
    TargetExists,
    /// The source no longer exists (moved or deleted).
    NotFound,
    PermissionDenied,
    /// Any other OS error; `detail` holds its message.
    Io,
    /// Failed and the original name could not be restored; `detail` is the temporary.
    RestoreFailed,
    /// Renamed, but the history could not be written.
    History,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpError {
    pub from: PathBuf,
    pub to: PathBuf,
    pub kind: ErrorKind,
    pub detail: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    pub done: Vec<Operation>,
    pub errors: Vec<OpError>,
    /// Id of the history entry created (if anything was renamed).
    pub history_id: Option<String>,
}

fn temp_path(from: &Path, i: usize) -> PathBuf {
    from.with_file_name(format!(".renome-{}-{i}.tmp", std::process::id()))
}

/// Runs the operations. Errors are per file: the others still go ahead.
pub fn execute(ops: &[Operation]) -> ApplyReport {
    let mut report = ApplyReport::default();
    let mut staged: Vec<(&Operation, PathBuf)> = Vec::with_capacity(ops.len());

    for (i, op) in ops.iter().enumerate() {
        let tmp = temp_path(&op.from, i);
        if fs::symlink_metadata(&tmp).is_ok() {
            report
                .errors
                .push(err(op, ErrorKind::TempExists, String::new()));
            continue;
        }
        match fs::rename(&op.from, &tmp) {
            Ok(()) => staged.push((op, tmp)),
            Err(e) => {
                let (kind, detail) = io_kind(&e);
                report.errors.push(err(op, kind, detail));
            }
        }
    }

    for (op, tmp) in staged {
        let result = if fs::symlink_metadata(&op.to).is_ok() {
            Err((ErrorKind::TargetExists, String::new()))
        } else {
            fs::rename(&tmp, &op.to).map_err(|e| io_kind(&e))
        };
        match result {
            Ok(()) => report.done.push(op.clone()),
            Err((kind, detail)) => {
                // Restore the original name.
                let (kind, detail) = match fs::rename(&tmp, &op.from) {
                    Ok(()) => (kind, detail),
                    Err(_) => (ErrorKind::RestoreFailed, tmp.display().to_string()),
                };
                report.errors.push(err(op, kind, detail));
            }
        }
    }
    report
}

/// Maps an OS error to a kind the UI can translate; only unknown errors keep
/// the OS message (without the "(os error N)" suffix).
fn io_kind(e: &std::io::Error) -> (ErrorKind, String) {
    match e.kind() {
        std::io::ErrorKind::NotFound => (ErrorKind::NotFound, String::new()),
        std::io::ErrorKind::PermissionDenied => (ErrorKind::PermissionDenied, String::new()),
        _ => {
            let text = e.to_string();
            let text = text.split(" (os error").next().unwrap_or(&text);
            (ErrorKind::Io, text.to_string())
        }
    }
}

fn err(op: &Operation, kind: ErrorKind, detail: String) -> OpError {
    OpError {
        from: op.from.clone(),
        to: op.to.clone(),
        kind,
        detail,
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Journal {
    created: DateTime<Local>,
    undone: bool,
    /// Operations not undone yet.
    ops: Vec<Operation>,
    /// How many were already undone (partial undo).
    #[serde(default)]
    reverted: usize,
    /// How many failed on the last undo attempt.
    #[serde(default)]
    failed: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub created: DateTime<Local>,
    pub count: usize,
    pub undone: bool,
    /// Undone so far, when an undo was only partial.
    pub reverted: usize,
    /// Failed on the last undo attempt (e.g. files moved or deleted).
    pub failed: usize,
}

/// Rename history, one JSON file per operation.
pub struct History {
    dir: PathBuf,
}

impl History {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err("invalidId".into());
        }
        Ok(self.dir.join(format!("{id}.json")))
    }

    fn read(&self, id: &str) -> Result<Journal, String> {
        let text = fs::read_to_string(self.path(id)?).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    fn write(&self, id: &str, j: &Journal) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(j).map_err(|e| e.to_string())?;
        fs::write(self.path(id)?, text).map_err(|e| e.to_string())
    }

    /// Runs and records in the history.
    pub fn apply(&self, ops: &[Operation]) -> ApplyReport {
        let mut report = execute(ops);
        if !report.done.is_empty() {
            let created = Local::now();
            let id = format!(
                "{}-{}",
                created.format("%Y%m%d-%H%M%S-%3f"),
                std::process::id()
            );
            let j = Journal {
                created,
                undone: false,
                ops: report.done.clone(),
                reverted: 0,
                failed: 0,
            };
            match self.write(&id, &j) {
                Ok(()) => report.history_id = Some(id),
                Err(e) => report.errors.push(OpError {
                    from: PathBuf::new(),
                    to: PathBuf::new(),
                    kind: ErrorKind::History,
                    detail: e,
                }),
            }
        }
        report
    }

    /// Lists entries, newest first.
    pub fn list(&self) -> Vec<HistoryEntry> {
        let Ok(rd) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out: Vec<HistoryEntry> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let id = name.strip_suffix(".json")?.to_string();
                let j = self.read(&id).ok()?;
                Some(HistoryEntry {
                    id,
                    created: j.created,
                    count: j.ops.len() + j.reverted,
                    undone: j.undone,
                    reverted: j.reverted,
                    failed: j.failed,
                })
            })
            .collect();
        out.sort_by_key(|e| std::cmp::Reverse(e.created));
        out
    }

    /// Deletes a history entry (the files are left as they are).
    pub fn delete(&self, id: &str) -> Result<(), String> {
        fs::remove_file(self.path(id)?).map_err(|e| e.to_string())
    }

    /// Undoes a history entry by renaming back. It is only marked as undone
    /// if everything goes back; whatever failed (e.g. a moved file) stays
    /// in the history so it can be retried.
    pub fn undo(&self, id: &str) -> Result<ApplyReport, String> {
        let mut j = self.read(id)?;
        if j.undone {
            return Err("alreadyUndone".into());
        }
        let reverse: Vec<Operation> = j
            .ops
            .iter()
            .map(|op| Operation {
                from: op.to.clone(),
                to: op.from.clone(),
            })
            .collect();
        let report = execute(&reverse);
        j.reverted += report.done.len();
        j.ops.retain(|op| {
            !report
                .done
                .iter()
                .any(|d| d.from == op.to && d.to == op.from)
        });
        j.failed = report.errors.len();
        j.undone = j.ops.is_empty();
        self.write(id, &j)?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(d: &Path, a: &str, b: &str) -> Operation {
        Operation {
            from: d.join(a),
            to: d.join(b),
        }
    }

    fn read(p: PathBuf) -> String {
        fs::read_to_string(p).unwrap()
    }

    #[test]
    fn swap_and_undo() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::write(p.join("a"), "A").unwrap();
        fs::write(p.join("b"), "B").unwrap();
        let h = History::new(p.join("hist"));

        let r = h.apply(&[op(p, "a", "b"), op(p, "b", "a")]);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(read(p.join("a")), "B");
        assert_eq!(read(p.join("b")), "A");

        let id = r.history_id.unwrap();
        assert_eq!(h.list().len(), 1);
        let u = h.undo(&id).unwrap();
        assert!(u.errors.is_empty());
        assert_eq!(read(p.join("a")), "A");
        assert!(h.list()[0].undone);
        assert!(h.undo(&id).is_err());
    }

    #[test]
    fn partial_undo_stays_pending() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::write(p.join("a"), "A").unwrap();
        fs::write(p.join("b"), "B").unwrap();
        let h = History::new(p.join("hist"));
        let id = h
            .apply(&[op(p, "a", "a2"), op(p, "b", "b2")])
            .history_id
            .unwrap();

        // The user moved one of the renamed files.
        fs::create_dir(p.join("sub")).unwrap();
        fs::rename(p.join("b2"), p.join("sub/b2")).unwrap();

        let u = h.undo(&id).unwrap();
        assert_eq!(u.done.len(), 1);
        assert_eq!(u.errors.len(), 1);
        let e = &h.list()[0];
        assert!(!e.undone);
        assert_eq!((e.count, e.reverted, e.failed), (2, 1, 1));
        assert_eq!(u.errors[0].kind, ErrorKind::NotFound);

        // Once it is back in place, the undo completes.
        fs::rename(p.join("sub/b2"), p.join("b2")).unwrap();
        let u = h.undo(&id).unwrap();
        assert!(u.errors.is_empty());
        assert_eq!(read(p.join("b")), "B");
        assert!(h.list()[0].undone);
        assert_eq!(h.list()[0].failed, 0);

        h.delete(&id).unwrap();
        assert!(h.list().is_empty());
    }

    #[test]
    fn never_overwrites() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::write(p.join("a"), "A").unwrap();
        fs::write(p.join("b"), "B").unwrap();
        let r = execute(&[op(p, "a", "b")]);
        assert_eq!(r.errors.len(), 1);
        assert_eq!(read(p.join("a")), "A");
        assert_eq!(read(p.join("b")), "B");
        // No leftover temporaries.
        assert_eq!(fs::read_dir(p).unwrap().count(), 2);
    }

    #[test]
    fn missing_source_reported() {
        let d = tempfile::tempdir().unwrap();
        let r = execute(&[op(d.path(), "nao-existe", "x")]);
        assert_eq!(r.errors.len(), 1);
        assert!(r.done.is_empty());
    }

    #[test]
    fn rejects_path_traversal_ids() {
        let d = tempfile::tempdir().unwrap();
        assert!(History::new(d.path()).undo("../x").is_err());
    }
}
