//! Safe two-phase renaming, plus a history for undo.
//!
//! Phase 1 moves each source to a temporary name in the same folder; phase 2
//! moves each temporary to its target. This makes swaps (a↔b), cycles and
//! case-only changes work, and nothing is ever overwritten.
//!
//! Folders can be renamed in the same batch as files inside them. Renames
//! run by depth: deepest first when applying (so a folder is renamed after
//! its contents), shallowest first when undoing (so the folder is back in
//! place before its contents are restored).

use std::collections::HashSet;
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
    /// Folders created to move files into, outermost first.
    #[serde(skip)]
    pub created_dirs: Vec<PathBuf>,
    /// Cue sheets and playlists updated to the new names (or rebuilt).
    pub lists: usize,
    /// Useless cue sheets and playlists deleted (none of their files exist).
    pub lists_removed: usize,
}

fn temp_path(from: &Path, i: usize) -> PathBuf {
    from.with_file_name(format!(".renome-{}-{i}.tmp", std::process::id()))
}

/// Order in which depth levels run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    DeepestFirst,
    ShallowestFirst,
}

/// Runs the operations, one depth level at a time. Errors are per file:
/// the others still go ahead. `done` lists the renames in the order they
/// happened.
pub fn execute(ops: &[Operation], order: Depth) -> ApplyReport {
    let depth = |op: &Operation| op.from.components().count();
    let mut levels: Vec<usize> = ops.iter().map(depth).collect();
    levels.sort_unstable();
    levels.dedup();
    if order == Depth::DeepestFirst {
        levels.reverse();
    }
    let mut report = ApplyReport::default();
    for level in levels {
        let batch: Vec<(usize, &Operation)> = ops
            .iter()
            .enumerate()
            .filter(|(_, op)| depth(op) == level)
            .collect();
        execute_level(&batch, &mut report);
    }
    report
}

/// Two-phase rename of operations at the same depth. `i` keeps the
/// temporary names unique across levels.
fn execute_level(ops: &[(usize, &Operation)], report: &mut ApplyReport) {
    let mut staged: Vec<(&Operation, PathBuf)> = Vec::with_capacity(ops.len());

    for &(i, op) in ops {
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
        let made = match make_parent(&op.to, report) {
            Ok(made) => made,
            Err(e) => {
                let (kind, detail) = io_kind(&e);
                let _ = fs::rename(&tmp, &op.from);
                report.errors.push(err(op, kind, detail));
                continue;
            }
        };
        let result = if fs::symlink_metadata(&op.to).is_ok() {
            Err((ErrorKind::TargetExists, String::new()))
        } else {
            fs::rename(&tmp, &op.to).map_err(|e| io_kind(&e))
        };
        match result {
            Ok(()) => report.done.push(op.clone()),
            Err((kind, detail)) => {
                // A folder made just for this file goes away again.
                for dir in made.iter().rev() {
                    if fs::remove_dir(dir).is_ok() {
                        report.created_dirs.retain(|d| d != dir);
                    }
                }
                // Restore the original name.
                let (kind, detail) = match fs::rename(&tmp, &op.from) {
                    Ok(()) => (kind, detail),
                    Err(_) => (ErrorKind::RestoreFailed, tmp.display().to_string()),
                };
                report.errors.push(err(op, kind, detail));
            }
        }
    }
}

/// Creates the target's missing parent folders (moving into a new folder)
/// and records them; returns the ones made now.
fn make_parent(to: &Path, report: &mut ApplyReport) -> std::io::Result<Vec<PathBuf>> {
    let Some(parent) = to.parent() else {
        return Ok(Vec::new());
    };
    let missing: Vec<PathBuf> = parent
        .ancestors()
        .take_while(|p| !p.as_os_str().is_empty() && fs::symlink_metadata(p).is_err())
        .map(Path::to_path_buf)
        .collect();
    if missing.is_empty() {
        return Ok(missing);
    }
    fs::create_dir_all(parent)?;
    // Outermost first.
    let made: Vec<PathBuf> = missing.into_iter().rev().collect();
    report.created_dirs.extend(made.iter().cloned());
    Ok(made)
}

/// Removes the folders files were moved out of when nothing is left in
/// them (`Show/Season 1` after the episodes went to `Show (2008)/Season
/// 01`), up to the folder the move happened within. Undo makes them again
/// when moving the files back.
fn remove_emptied(done: &[Operation]) {
    for op in done {
        let mut dir = op.from.parent();
        while let Some(d) = dir {
            if d.as_os_str().is_empty() || op.to.starts_with(d) || fs::remove_dir(d).is_err() {
                break;
            }
            dir = d.parent();
        }
    }
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
    /// Files created with the rename (`.nfo`, posters), deleted on undo.
    #[serde(default)]
    extra_files: Vec<PathBuf>,
    /// Folders the rename created, removed by undo when empty.
    #[serde(default)]
    dirs: Vec<PathBuf>,
    /// Cue sheets and playlists rewritten to follow the renames, with the
    /// file (in the entry's folder) that keeps their original content.
    #[serde(default)]
    lists: Vec<(PathBuf, String)>,
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
    /// Files created along with the rename (`.nfo`, posters).
    pub extra: usize,
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

    /// Where an entry keeps the original content of the lists it rewrote.
    fn backups(&self, id: &str) -> Result<PathBuf, String> {
        Ok(self.path(id)?.with_extension("files"))
    }

    /// Rewrites (or deletes, when useless) the cue sheets and playlists
    /// that refer to renamed files, keeping each original first; a list
    /// whose original cannot be kept is left alone. Returns the lists
    /// touched and how many were deleted.
    fn update_lists(&self, id: &str, done: &[Operation]) -> (Vec<(PathBuf, String)>, usize) {
        let changes = crate::playlists::changes(done);
        let Ok(dir) = self.backups(id) else {
            return (Vec::new(), 0);
        };
        if changes.is_empty() || fs::create_dir_all(&dir).is_err() {
            return (Vec::new(), 0);
        }
        let mut kept = Vec::new();
        let mut removed = 0;
        for (n, c) in changes.into_iter().enumerate() {
            let name = n.to_string();
            if fs::write(dir.join(&name), &c.old).is_err() {
                continue;
            }
            let done = match &c.new {
                Some(new) => fs::write(&c.path, new).is_ok(),
                None => fs::remove_file(&c.path).is_ok(),
            };
            if done {
                removed += usize::from(c.new.is_none());
                kept.push((c.path, name));
            }
        }
        (kept, removed)
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
        let mut report = execute(ops, Depth::DeepestFirst);
        if !report.done.is_empty() {
            let created = Local::now();
            let id = format!(
                "{}-{}",
                created.format("%Y%m%d-%H%M%S-%3f"),
                std::process::id()
            );
            let (lists, removed) = self.update_lists(&id, &report.done);
            report.lists = lists.len() - removed;
            report.lists_removed = removed;
            // After the lists: a deleted one may leave its folder empty.
            remove_emptied(&report.done);
            let j = Journal {
                created,
                undone: false,
                ops: report.done.clone(),
                reverted: 0,
                failed: 0,
                extra_files: Vec::new(),
                dirs: report.created_dirs.clone(),
                lists,
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
                    extra: j.extra_files.len(),
                })
            })
            .collect();
        out.sort_by_key(|e| std::cmp::Reverse(e.created));
        out
    }

    /// A history entry for files created without any rename (metadata for
    /// movies whose names were already right), so undo deletes them.
    pub fn record_files(&self, paths: &[PathBuf]) -> Result<String, String> {
        let created = Local::now();
        let id = format!(
            "{}-{}",
            created.format("%Y%m%d-%H%M%S-%3f"),
            std::process::id()
        );
        let j = Journal {
            created,
            undone: false,
            ops: Vec::new(),
            reverted: 0,
            failed: 0,
            extra_files: paths.to_vec(),
            dirs: Vec::new(),
            lists: Vec::new(),
        };
        self.write(&id, &j)?;
        Ok(id)
    }

    /// Records files created along with a rename, so undo deletes them.
    pub fn add_created(&self, id: &str, paths: &[PathBuf]) -> Result<(), String> {
        let mut j = self.read(id)?;
        j.extra_files.extend_from_slice(paths);
        self.write(id, &j)
    }

    /// Deletes a history entry (the files are left as they are).
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let _ = fs::remove_dir_all(self.backups(id)?);
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
        // Files the rename created go first (already gone is fine).
        j.extra_files.retain(|p| match fs::remove_file(p) {
            Ok(()) => false,
            Err(e) => e.kind() != std::io::ErrorKind::NotFound,
        });
        // Rewritten lists get their content back where they are now; the
        // renames below then take them back.
        let backups = self.backups(id)?;
        j.lists.retain(|(path, name)| {
            // A deleted list's folder may be gone too.
            let restored = fs::read(backups.join(name)).and_then(|old| {
                if let Some(dir) = path.parent() {
                    fs::create_dir_all(dir)?;
                }
                fs::write(path, old)
            });
            restored.is_err()
        });
        if j.lists.is_empty() {
            let _ = fs::remove_dir_all(&backups);
        }
        let reverse: Vec<Operation> = j
            .ops
            .iter()
            .map(|op| Operation {
                from: op.to.clone(),
                to: op.from.clone(),
            })
            .collect();
        let report = execute(&reverse, Depth::ShallowestFirst);
        j.reverted += report.done.len();
        let back: HashSet<(&Path, &Path)> = report
            .done
            .iter()
            .map(|d| (d.to.as_path(), d.from.as_path()))
            .collect();
        j.ops
            .retain(|op| !back.contains(&(op.from.as_path(), op.to.as_path())));
        j.failed = report.errors.len();
        if j.ops.is_empty() {
            // Innermost first; a folder that is not empty (the user put
            // something there) stays.
            for dir in j.dirs.iter().rev() {
                let _ = fs::remove_dir(dir);
            }
            j.dirs.clear();
        }
        j.undone = j.ops.is_empty() && j.extra_files.is_empty() && j.lists.is_empty();
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
    fn folder_and_contents_together() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::create_dir_all(p.join("old/inner")).unwrap();
        fs::write(p.join("old/inner/a"), "A").unwrap();
        let h = History::new(p.join("hist"));

        // Listed parent first: the order must not matter.
        let r = h.apply(&[
            op(p, "old", "new"),
            op(p, "old/inner", "old/deep"),
            op(p, "old/inner/a", "old/inner/b"),
        ]);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(read(p.join("new/deep/b")), "A");

        let u = h.undo(&r.history_id.unwrap()).unwrap();
        assert!(u.errors.is_empty(), "{:?}", u.errors);
        assert_eq!(read(p.join("old/inner/a")), "A");
    }

    #[test]
    fn moves_into_new_folder_and_back() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::write(p.join("m.mkv"), "M").unwrap();
        fs::write(p.join("m.srt"), "S").unwrap();
        let h = History::new(p.join("hist"));
        let r = h.apply(&[
            op(p, "m.mkv", "Movie/Movie.mkv"),
            op(p, "m.srt", "Movie/Movie.srt"),
        ]);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(r.created_dirs, [p.join("Movie")]);
        assert_eq!(read(p.join("Movie/Movie.mkv")), "M");
        let id = r.history_id.unwrap();
        fs::write(p.join("Movie/Movie.nfo"), "x").unwrap();
        h.add_created(&id, &[p.join("Movie/Movie.nfo")]).unwrap();
        h.undo(&id).unwrap();
        assert_eq!(read(p.join("m.mkv")), "M");
        assert!(!p.join("Movie").exists());
    }

    #[test]
    fn emptied_folders_go_and_come_back() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::create_dir_all(p.join("Show/Season 1")).unwrap();
        fs::create_dir_all(p.join("Other")).unwrap();
        fs::write(p.join("Show/Season 1/e1.mkv"), "1").unwrap();
        fs::write(p.join("Other/keep.txt"), "k").unwrap();
        fs::write(p.join("Other/e2.mkv"), "2").unwrap();
        let h = History::new(p.join("h"));
        let r = h.apply(&[
            op(p, "Show/Season 1/e1.mkv", "New/Season 01/e1.mkv"),
            op(p, "Other/e2.mkv", "New/Season 01/e2.mkv"),
        ]);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(!p.join("Show").exists());
        assert!(p.join("Other/keep.txt").exists());
        h.undo(&r.history_id.unwrap()).unwrap();
        assert_eq!(read(p.join("Show/Season 1/e1.mkv")), "1");
        assert_eq!(read(p.join("Other/e2.mkv")), "2");
        assert!(!p.join("New").exists());
    }

    #[test]
    fn lists_follow_and_come_back() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::write(p.join("a.flac"), "a").unwrap();
        fs::write(p.join("album.cue"), "FILE \"a.flac\" WAVE\n").unwrap();
        let h = History::new(p.join("h"));
        let r = h.apply(&[op(p, "a.flac", "01 - A.flac")]);
        assert_eq!(r.lists, 1);
        assert_eq!(read(p.join("album.cue")), "FILE \"01 - A.flac\" WAVE\n");
        h.undo(&r.history_id.unwrap()).unwrap();
        assert_eq!(read(p.join("album.cue")), "FILE \"a.flac\" WAVE\n");
        assert!(p.join("a.flac").exists());
        assert_eq!(fs::read_dir(p.join("h")).unwrap().count(), 1);
    }

    #[test]
    fn useless_list_deleted_and_restored() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::create_dir(p.join("old")).unwrap();
        fs::write(p.join("old/a.flac"), "a").unwrap();
        fs::write(p.join("old/list.m3u"), "missing.flac\n").unwrap();
        let h = History::new(p.join("h"));
        let r = h.apply(&[op(p, "old/a.flac", "a.flac")]);
        assert_eq!((r.lists, r.lists_removed), (0, 1));
        // The list was all that kept the folder.
        assert!(!p.join("old").exists());
        h.undo(&r.history_id.unwrap()).unwrap();
        assert_eq!(read(p.join("old/list.m3u")), "missing.flac\n");
        assert_eq!(read(p.join("old/a.flac")), "a");
    }

    #[test]
    fn undo_deletes_created_files() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::write(p.join("a"), "A").unwrap();
        let h = History::new(p.join("hist"));
        let id = h.apply(&[op(p, "a", "b")]).history_id.unwrap();
        fs::write(p.join("b.nfo"), "x").unwrap();
        h.add_created(&id, &[p.join("b.nfo"), p.join("gone.jpg")])
            .unwrap();
        assert_eq!(h.list()[0].extra, 2);
        h.undo(&id).unwrap();
        assert!(!p.join("b.nfo").exists());
        assert_eq!(read(p.join("a")), "A");
        assert!(h.list()[0].undone);

        // Metadata alone, without renames.
        fs::write(p.join("c.nfo"), "x").unwrap();
        let id = h.record_files(&[p.join("c.nfo")]).unwrap();
        h.undo(&id).unwrap();
        assert!(!p.join("c.nfo").exists());
    }

    #[test]
    fn never_overwrites() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::write(p.join("a"), "A").unwrap();
        fs::write(p.join("b"), "B").unwrap();
        let r = execute(&[op(p, "a", "b")], Depth::DeepestFirst);
        assert_eq!(r.errors.len(), 1);
        assert_eq!(read(p.join("a")), "A");
        assert_eq!(read(p.join("b")), "B");
        // No leftover temporaries.
        assert_eq!(fs::read_dir(p).unwrap().count(), 2);
    }

    #[test]
    fn missing_source_reported() {
        let d = tempfile::tempdir().unwrap();
        let r = execute(&[op(d.path(), "nao-existe", "x")], Depth::DeepestFirst);
        assert_eq!(r.errors.len(), 1);
        assert!(r.done.is_empty());
    }

    #[test]
    fn rejects_path_traversal_ids() {
        let d = tempfile::tempdir().unwrap();
        assert!(History::new(d.path()).undo("../x").is_err());
    }
}
