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
//!
//! A move to another disk, and a copy, writes the file under a passing
//! name beside its target, waits until it is on the disk, checks its size
//! and only then gives it its name; a moved file's original goes after
//! that. Undo deletes a copy only while it is as it was left.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Operation {
    pub from: PathBuf,
    pub to: PathBuf,
    /// A copy is made at `to`; `from` stays.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub copy: bool,
    /// Sent to another folder: the folders it leaves empty are deleted
    /// only below this one (the folder it was added with), not it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within: Option<PathBuf>,
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
    /// The target is on another disk (a move would be a copy).
    OtherDisk,
    /// Any other OS error; `detail` holds its message.
    Io,
    /// Failed and the original name could not be restored; `detail` is the temporary.
    RestoreFailed,
    /// Renamed, but the history could not be written.
    History,
    /// A song's old tags could not be put back; `detail` says why.
    Tags,
    /// A folder moved to another disk, whose original could not all be
    /// deleted: what is left of it stays under its old name.
    PartlyLeft,
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
    /// Copies undo left because they changed after they were made.
    pub kept: Vec<PathBuf>,
    /// Copies undo deleted.
    pub copies_removed: usize,
    /// Stopped before the end: the files not reached stay as they were.
    pub stopped: bool,
}

/// What the caller is told while files are copied, and asked whether to
/// stop.
pub struct Hooks<'a> {
    /// Bytes done and in all, for the copies (and moves to other disks).
    pub progress: &'a dyn Fn(u64, u64),
    pub stop: &'a dyn Fn() -> bool,
}

impl Hooks<'_> {
    pub fn none() -> Hooks<'static> {
        Hooks {
            progress: &|_, _| {},
            stop: &|| false,
        }
    }
}

/// What a copy was like when it was left: undo deletes it only if it still
/// is. A folder's: its files' count and total size, and the latest date.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Stamp {
    pub size: u64,
    pub modified: i128,
    pub files: u64,
}

fn modified_ns(m: &fs::Metadata) -> i128 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i128)
        .unwrap_or_default()
}

pub fn stamp(path: &Path) -> Option<Stamp> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_dir() {
        return Some(Stamp {
            size: meta.len(),
            modified: modified_ns(&meta),
            files: 1,
        });
    }
    let mut s = Stamp::default();
    for e in walkdir::WalkDir::new(path).min_depth(1) {
        let m = e.ok()?.metadata().ok()?;
        if !m.is_dir() {
            s.files += 1;
            s.size += m.len();
            s.modified = s.modified.max(modified_ns(&m));
        }
    }
    Some(s)
}

/// The bytes a copy of `path` writes (a folder's files).
fn size_of(path: &Path) -> u64 {
    stamp(path).map(|s| s.size).unwrap_or_default()
}

/// Copies a file with its date, waits until it is on the disk and checks
/// its size; a folder with everything in it (links as links where they
/// can be).
fn copy_any(from: &Path, to: &Path, done: &mut dyn FnMut(u64)) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(from)?;
    if meta.is_symlink() {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(fs::read_link(from)?, to);
    }
    if !meta.is_dir() {
        let n = fs::copy(from, to)?;
        let f = File::options().write(true).open(to)?;
        if let Ok(t) = meta.modified() {
            f.set_modified(t)?;
        }
        f.sync_all()?;
        if n != meta.len() || f.metadata()?.len() != meta.len() {
            return Err(std::io::Error::other("copy incomplete"));
        }
        done(n);
        return Ok(());
    }
    fs::create_dir(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        copy_any(&e.path(), &to.join(e.file_name()), done)?;
    }
    if let Ok(t) = meta.modified() {
        let _ = File::open(to).and_then(|d| d.set_modified(t));
    }
    Ok(())
}

fn remove_any(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Where a copy is written before it gets its name (see [`copy_new`]).
fn part_path(to: &Path, i: usize) -> PathBuf {
    to.with_file_name(format!(".renome-{}-{i}.part", std::process::id()))
}

/// Copies `from` to `to` (which must not exist) through `part`, and gives
/// it its name only when whole; nothing is left behind on failure.
fn copy_new(
    from: &Path,
    to: &Path,
    part: &Path,
    done: &mut dyn FnMut(u64),
) -> Result<(), (ErrorKind, String)> {
    if fs::symlink_metadata(to).is_ok() {
        return Err((ErrorKind::TargetExists, String::new()));
    }
    let _ = remove_any(part);
    if let Err(e) = copy_any(from, part, done) {
        let _ = remove_any(part);
        return Err(io_kind(&e));
    }
    sync_dir(part);
    if let Err(e) = rename_new(part, to) {
        let _ = remove_any(part);
        return Err(e);
    }
    sync_dir(to);
    Ok(())
}

/// Renames `from` to `to` only if `to` does not exist. The check and the
/// rename are one step, so nothing can appear in between: `renameat2` on
/// Linux, `renamex_np` on macOS, `MoveFileExW` on Windows. Where the file
/// system cannot do that (or a Windows path is too long for it), it checks
/// first.
fn rename_new(from: &Path, to: &Path) -> Result<(), (ErrorKind, String)> {
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        if let (Ok(f), Ok(t)) = (
            CString::new(from.as_os_str().as_bytes()),
            CString::new(to.as_os_str().as_bytes()),
        ) {
            // SAFETY: both are valid C strings for the call's duration.
            let r = unsafe {
                libc::renameat2(
                    libc::AT_FDCWD,
                    f.as_ptr(),
                    libc::AT_FDCWD,
                    t.as_ptr(),
                    libc::RENAME_NOREPLACE,
                )
            };
            if r == 0 {
                return Ok(());
            }
            let e = std::io::Error::last_os_error();
            match e.raw_os_error() {
                Some(libc::EEXIST) => return Err((ErrorKind::TargetExists, String::new())),
                // Not supported here: check, then rename.
                Some(libc::EINVAL | libc::ENOSYS | libc::ENOTSUP) => {}
                _ => return Err(io_kind(&e)),
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        if let (Ok(f), Ok(t)) = (
            CString::new(from.as_os_str().as_bytes()),
            CString::new(to.as_os_str().as_bytes()),
        ) {
            // SAFETY: both are valid C strings for the call's duration.
            let r = unsafe { libc::renamex_np(f.as_ptr(), t.as_ptr(), libc::RENAME_EXCL) };
            if r == 0 {
                return Ok(());
            }
            let e = std::io::Error::last_os_error();
            match e.raw_os_error() {
                Some(libc::EEXIST) => return Err((ErrorKind::TargetExists, String::new())),
                // Not supported here: check, then rename.
                Some(libc::EINVAL | libc::ENOSYS | libc::ENOTSUP) => {}
                _ => return Err(io_kind(&e)),
            }
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
        }
        const ERROR_FILE_EXISTS: i32 = 80;
        const ERROR_ALREADY_EXISTS: i32 = 183;
        let wide = |p: &Path| -> Vec<u16> {
            p.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        };
        let (f, t) = (wide(from), wide(to));
        // Longer paths need a prefix the standard rename knows how to add.
        if f.len() < 260 && t.len() < 260 {
            // No flags: an existing target is refused, not replaced.
            // SAFETY: both are null-terminated wide strings for the call's
            // duration.
            if unsafe { MoveFileExW(f.as_ptr(), t.as_ptr(), 0) } != 0 {
                return Ok(());
            }
            let e = std::io::Error::last_os_error();
            return match e.raw_os_error() {
                Some(ERROR_FILE_EXISTS | ERROR_ALREADY_EXISTS) => {
                    Err((ErrorKind::TargetExists, String::new()))
                }
                _ => Err(io_kind(&e)),
            };
        }
    }
    if fs::symlink_metadata(to).is_ok() {
        return Err((ErrorKind::TargetExists, String::new()));
    }
    fs::rename(from, to).map_err(|e| io_kind(&e))
}

/// Writes a file through a copy that takes its place, and waits until it
/// is on the disk: a power cut does not leave it empty or half written
/// while what comes next (removing the [`Pending`] note) is saved.
fn write_synced(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.tmp"));
    let mut f = File::create(&tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    fs::rename(&tmp, path)?;
    sync_dir(path);
    Ok(())
}

/// Saves a folder's list of names to the disk (where a folder can be
/// opened as a file; elsewhere the system does it on its own).
fn sync_dir(path: &Path) {
    if let Some(dir) = path.parent()
        && let Ok(d) = File::open(dir)
    {
        let _ = d.sync_all();
    }
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
    execute_with(ops, order, &Hooks::none())
}

pub fn execute_with(ops: &[Operation], order: Depth, hooks: &Hooks) -> ApplyReport {
    // What is copied: the copies, and moves that may cross disks (only
    // known when they fail to rename; counted when they do).
    let total: u64 = ops
        .iter()
        .filter(|op| op.copy)
        .map(|op| size_of(&op.from))
        .sum();
    let bytes = std::cell::Cell::new(0u64);
    let total = std::cell::Cell::new(total);
    let progress = |n: u64| {
        bytes.set(bytes.get() + n);
        (hooks.progress)(bytes.get(), total.get().max(bytes.get()));
    };
    let ctx = Copying {
        progress: &progress,
        grow: &|n| total.set(total.get() + n),
        stop: hooks.stop,
    };
    let depth = |op: &Operation| op.from.components().count();
    let mut levels: Vec<usize> = ops.iter().map(depth).collect();
    levels.sort_unstable();
    levels.dedup();
    if order == Depth::DeepestFirst {
        levels.reverse();
    }
    let mut report = ApplyReport::default();
    for level in levels {
        if (ctx.stop)() {
            report.stopped = true;
            break;
        }
        let batch: Vec<(usize, &Operation)> = ops
            .iter()
            .enumerate()
            .filter(|(_, op)| depth(op) == level)
            .collect();
        execute_level(&batch, &mut report, &ctx);
    }
    report
}

/// Progress and stop, while running.
struct Copying<'a> {
    progress: &'a dyn Fn(u64),
    /// A move found to cross disks: its bytes join the total.
    grow: &'a dyn Fn(u64),
    stop: &'a dyn Fn() -> bool,
}

/// Two-phase rename of operations at the same depth. `i` keeps the
/// temporary names unique across levels. Copies skip the first phase:
/// their files stay.
fn execute_level(ops: &[(usize, &Operation)], report: &mut ApplyReport, ctx: &Copying) {
    let mut staged: Vec<(usize, &Operation, PathBuf)> = Vec::with_capacity(ops.len());

    for &(i, op) in ops {
        if op.copy {
            staged.push((i, op, op.from.clone()));
            continue;
        }
        let tmp = temp_path(&op.from, i);
        if fs::symlink_metadata(&tmp).is_ok() {
            report
                .errors
                .push(err(op, ErrorKind::TempExists, String::new()));
            continue;
        }
        match fs::rename(&op.from, &tmp) {
            Ok(()) => staged.push((i, op, tmp)),
            Err(e) => {
                let (kind, detail) = io_kind(&e);
                report.errors.push(err(op, kind, detail));
            }
        }
    }

    for (i, op, tmp) in staged {
        // Stopped: those not reached get their names back.
        if report.stopped || (ctx.stop)() {
            report.stopped = true;
            if !op.copy {
                let _ = fs::rename(&tmp, &op.from);
            }
            continue;
        }
        let made = match make_parent(&op.to, report) {
            Ok(made) => made,
            Err(e) => {
                let (kind, detail) = io_kind(&e);
                if !op.copy {
                    let _ = fs::rename(&tmp, &op.from);
                }
                report.errors.push(err(op, kind, detail));
                continue;
            }
        };
        let mut add = |n| (ctx.progress)(n);
        let result = if op.copy {
            copy_new(&op.from, &op.to, &part_path(&op.to, i), &mut add)
        } else {
            match rename_new(&tmp, &op.to) {
                // Another disk: copied, then the original goes.
                Err((ErrorKind::OtherDisk, _)) => {
                    (ctx.grow)(size_of(&tmp));
                    let copied = copy_new(&tmp, &op.to, &part_path(&op.to, i), &mut add);
                    match copied.map(|()| (tmp.is_dir(), remove_any(&tmp))) {
                        Ok((_, Ok(()))) => Ok(()),
                        // A file is whole when it cannot go: its copy goes.
                        Ok((false, Err(e))) => {
                            let _ = remove_any(&op.to);
                            Err(io_kind(&e))
                        }
                        // A folder may be partly gone: its copy is the
                        // whole one, so it stays and the move counts; what
                        // is left of the original is told, under its name.
                        Ok((true, Err(e))) => {
                            let _ = fs::rename(&tmp, &op.from);
                            report
                                .errors
                                .push(err(op, ErrorKind::PartlyLeft, io_kind(&e).1));
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
                r => r,
            }
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
                let (kind, detail) = if op.copy {
                    (kind, detail)
                } else {
                    match fs::rename(&tmp, &op.from) {
                        Ok(()) => (kind, detail),
                        Err(_) => (ErrorKind::RestoreFailed, tmp.display().to_string()),
                    }
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
/// 01`), up to the folder the move happened within; inside a folder that
/// was renamed afterwards, under its new name. Undo makes them again
/// when moving the files back.
fn remove_emptied(done: &[Operation]) {
    let folders: Vec<&Operation> = done
        .iter()
        .filter(|op| !op.copy && op.to.is_dir())
        .collect();
    for op in done.iter().filter(|op| !op.copy) {
        let from = folders
            .iter()
            .find_map(|f| {
                op.from
                    .strip_prefix(&f.from)
                    .ok()
                    .filter(|rest| !rest.as_os_str().is_empty())
                    .map(|rest| f.to.join(rest))
            })
            .unwrap_or_else(|| op.from.clone());
        let mut dir = from.parent();
        while let Some(d) = dir {
            let above = op
                .within
                .as_ref()
                .is_some_and(|w| !d.starts_with(w) || d == w);
            if d.as_os_str().is_empty()
                || above
                || op.to.starts_with(d)
                || fs::remove_dir(d).is_err()
            {
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
        std::io::ErrorKind::CrossesDevices => (ErrorKind::OtherDisk, String::new()),
        _ => {
            let text = e.to_string();
            let text = text.split(" (os error").next().unwrap_or(&text);
            (ErrorKind::Io, text.to_string())
        }
    }
}

/// The copies among `done`, as they are now.
fn copy_stamps(done: &[Operation]) -> Vec<(PathBuf, Stamp)> {
    done.iter()
        .filter(|op| op.copy)
        .filter_map(|op| Some((op.to.clone(), stamp(&op.to)?)))
        .collect()
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
    /// Songs whose tags were written, with what they held (pictures in the
    /// entry's folder); undo puts it back.
    #[serde(default)]
    tags: Vec<crate::tagwrite::Backup>,
    /// How many songs had their tags written (for the list).
    #[serde(default)]
    tagged: usize,
    /// Each copy made as it was left (after its lists and tags), for undo.
    #[serde(default)]
    stamps: Vec<(PathBuf, Stamp)>,
}

/// A rename in progress: written before it starts and removed when the
/// history entry is written, so that, if the program stops in between,
/// the files left with temporary names can be put back.
#[derive(Debug, Serialize, Deserialize)]
struct Pending {
    created: DateTime<Local>,
    ops: Vec<Operation>,
    /// The temporary name of each operation (see [`temp_path`]).
    temps: Vec<PathBuf>,
    /// An undo of this entry, not a rename.
    #[serde(default)]
    undoing: bool,
    /// Where each copy (and move to another disk) is written before it
    /// gets its name (see [`part_path`]).
    #[serde(default)]
    parts: Vec<PathBuf>,
}

/// What [`History::recover`] did with renames left halfway.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recovered {
    /// Files put back under their old names.
    pub restored: usize,
    /// Files that had been renamed: kept, in a history entry to undo.
    pub renamed: usize,
    /// Files an undo had already taken back: kept so, out of their entry.
    pub undone: usize,
    /// Temporary files that could not be put back (their old name is
    /// taken), with the name each should have.
    pub stuck: Vec<Operation>,
    /// Songs whose tags had been written: kept so, in their entry to undo.
    pub tagged: usize,
}

/// One line of a [`Tagging`] note.
#[derive(Deserialize)]
enum TagStep {
    /// A song is about to be written.
    Start(PathBuf),
    /// It was: what it held, for undo.
    Done(crate::tagwrite::Backup),
}

/// Songs whose tags are being written for a history entry. Each is noted
/// in a file next to the entry before (so the copy it is written in can
/// be removed) and after (so undo keeps what it held); the end removes the
/// file. A stop halfway is recovered from it (see [`History::recover`]).
pub struct Tagging {
    path: PathBuf,
    file: File,
}

impl Tagging {
    pub fn start(&mut self, song: &Path) {
        self.note(&serde_json::json!({ "Start": song }));
    }

    pub fn done(&mut self, backup: &crate::tagwrite::Backup) {
        self.note(&serde_json::json!({ "Done": backup }));
    }

    fn note(&mut self, step: &serde_json::Value) {
        let _ = writeln!(self.file, "{step}");
        let _ = self.file.sync_data();
    }

    /// The songs are in their entry: the note goes.
    pub fn finish(self) {
        let _ = fs::remove_file(&self.path);
    }
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
    /// Songs whose tags were written.
    pub tagged: usize,
}

/// Comics converted to CBZ whose original went to the trash: undo gives
/// them their old name back, as the CBZ they now are (a CBZ named `.cbr`
/// would be wrong).
fn trashed_originals(j: &mut Journal, backups: &[crate::tagwrite::Backup]) {
    for b in backups {
        if !b.converted.as_ref().is_some_and(|c| c.original.is_none()) {
            continue;
        }
        for op in j.ops.iter_mut().filter(|op| !op.copy && op.to == b.path) {
            let cbz = op
                .from
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("cbz"));
            if !cbz {
                op.from = op.from.with_extension("cbz");
            }
        }
    }
}

/// Rename history, one JSON file per operation.
pub struct History {
    dir: PathBuf,
}

impl History {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// An id from the time, unused yet (two entries in the same
    /// millisecond get `-2`, `-3`…).
    fn new_id(&self, created: &DateTime<Local>) -> String {
        let base = format!(
            "{}-{}",
            created.format("%Y%m%d-%H%M%S-%3f"),
            std::process::id()
        );
        let mut id = base.clone();
        let mut n = 1;
        while self.dir.join(format!("{id}.json")).exists() {
            n += 1;
            id = format!("{base}-{n}");
        }
        id
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

    fn pending_path(&self, id: &str) -> Result<PathBuf, String> {
        Ok(self.path(id)?.with_extension("pending"))
    }

    /// Puts back renames (and undos) a stopped program left halfway: files
    /// still under a temporary name get their name from before back; those
    /// already renamed stay so, in a history entry that can undo them (with
    /// the folders made for them); those an undo had already taken back are
    /// taken out of their entry.
    pub fn recover(&self) -> Recovered {
        let mut out = Recovered::default();
        let Ok(rd) = fs::read_dir(&self.dir) else {
            return out;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(id) = name.strip_suffix(".tagging") {
                out.tagged += self.recover_tags(id, &e.path());
                continue;
            }
            let Some(id) = name.strip_suffix(".pending") else {
                continue;
            };
            let Ok(p) = fs::read_to_string(e.path())
                .map_err(|e| e.to_string())
                .and_then(|t| serde_json::from_str::<Pending>(&t).map_err(|e| e.to_string()))
            else {
                continue;
            };
            let stuck = out.stuck.len();
            let mut done = Vec::new();
            for (n, (op, tmp)) in p.ops.iter().zip(&p.temps).enumerate() {
                let here = |p: &Path| fs::symlink_metadata(p).is_ok();
                // A copy cut short.
                if let Some(part) = p.parts.get(n) {
                    let _ = remove_any(part);
                }
                // A copy gets its name only when whole.
                if op.copy {
                    if here(&op.to) {
                        done.push(op.clone());
                    }
                    continue;
                }
                if here(tmp) {
                    // Copied whole to another disk, the original not gone
                    // yet: the copy goes, the same as the original.
                    if here(&op.to) && stamp(&op.to) == stamp(tmp) {
                        let _ = remove_any(&op.to);
                    }
                    match rename_new(tmp, &op.from) {
                        Ok(()) => out.restored += 1,
                        Err(_) => out.stuck.push(Operation {
                            from: tmp.clone(),
                            to: op.from.clone(),
                            copy: false,
                            within: None,
                        }),
                    }
                } else if !here(&op.from) && here(&op.to) {
                    done.push(op.clone());
                }
            }
            let kept = if p.undoing {
                out.undone += done.len();
                done.is_empty() || self.settle_undo(id, &done).is_ok()
            } else {
                out.renamed += done.len();
                // The folders the renamed files went into, as far as they
                // were not there for the old names; undo removes them when
                // they are left empty.
                let mut dirs: Vec<PathBuf> = Vec::new();
                for op in &done {
                    for d in op.to.ancestors().skip(1) {
                        if op.from.starts_with(d) {
                            break;
                        }
                        if !dirs.iter().any(|x| x == d) {
                            dirs.push(d.to_path_buf());
                        }
                    }
                }
                // Outermost first.
                dirs.sort_by_key(|d| d.components().count());
                let stamps = copy_stamps(&done);
                done.is_empty()
                    || self
                        .write(
                            id,
                            &Journal {
                                created: p.created,
                                undone: false,
                                ops: done,
                                reverted: 0,
                                failed: 0,
                                extra_files: Vec::new(),
                                dirs,
                                lists: Vec::new(),
                                tags: Vec::new(),
                                tagged: 0,
                                stamps,
                            },
                        )
                        .is_ok()
            };
            if kept && out.stuck.len() == stuck {
                let _ = fs::remove_file(e.path());
            }
        }
        out
    }

    /// Tags left halfway (see [`Tagging`]): the copies left beside songs
    /// are removed, and the songs written join entry `id`, for undo (an
    /// entry left with nothing goes). Returns how many joined.
    fn recover_tags(&self, id: &str, note: &Path) -> usize {
        let Ok(text) = fs::read_to_string(note) else {
            return 0;
        };
        let mut done = Vec::new();
        // A line cut short by the stop is skipped.
        for step in text.lines().filter_map(|l| serde_json::from_str(l).ok()) {
            match step {
                TagStep::Start(song) => {
                    let _ = fs::remove_file(crate::tagwrite::temp_of(&song));
                }
                TagStep::Done(b) => done.push(b),
            }
        }
        let Ok(mut j) = self.read(id) else {
            let _ = fs::remove_file(note);
            return 0;
        };
        // Already there when the stop came after the entry was written.
        done.retain(|b| !j.tags.iter().any(|t| t.path == b.path));
        let joined = done.len();
        j.tagged += joined;
        trashed_originals(&mut j, &done);
        j.tags.extend(done);
        let empty =
            j.ops.is_empty() && j.extra_files.is_empty() && j.lists.is_empty() && j.tags.is_empty();
        let kept = if empty {
            self.delete(id).is_ok()
        } else {
            self.write(id, &j).is_ok()
        };
        if kept {
            let _ = fs::remove_file(note);
        }
        joined
    }

    /// Takes out of entry `id` the renames an undo took back (`back` are
    /// the undo's operations), as the undo would have.
    fn settle_undo(&self, id: &str, back: &[Operation]) -> Result<(), String> {
        let mut j = self.read(id)?;
        let back: HashSet<(&Path, &Path)> = back
            .iter()
            .map(|b| (b.to.as_path(), b.from.as_path()))
            .collect();
        let before = j.ops.len();
        j.ops
            .retain(|op| !back.contains(&(op.from.as_path(), op.to.as_path())));
        j.reverted += before - j.ops.len();
        j.undone =
            j.ops.is_empty() && j.extra_files.is_empty() && j.lists.is_empty() && j.tags.is_empty();
        self.write(id, &j)
    }

    fn read(&self, id: &str) -> Result<Journal, String> {
        let text = fs::read_to_string(self.path(id)?).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    fn write(&self, id: &str, j: &Journal) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(j).map_err(|e| e.to_string())?;
        write_synced(&self.path(id)?, text.as_bytes()).map_err(|e| e.to_string())
    }

    /// Runs and records in the history. What is about to happen is written
    /// first (see [`Pending`]), so a stop halfway can be recovered.
    pub fn apply(&self, ops: &[Operation]) -> ApplyReport {
        self.apply_with(ops, &Hooks::none())
    }

    pub fn apply_with(&self, ops: &[Operation], hooks: &Hooks) -> ApplyReport {
        let created = Local::now();
        let id = self.new_id(&created);
        let pending = self.pending_path(&id).ok().filter(|path| {
            let p = Pending {
                created,
                ops: ops.to_vec(),
                temps: ops
                    .iter()
                    .enumerate()
                    .map(|(i, op)| temp_path(&op.from, i))
                    .collect(),
                undoing: false,
                parts: ops
                    .iter()
                    .enumerate()
                    .map(|(i, op)| part_path(&op.to, i))
                    .collect(),
            };
            fs::create_dir_all(&self.dir).is_ok()
                && serde_json::to_vec(&p).is_ok_and(|t| write_synced(path, &t).is_ok())
        });
        let mut report = execute_with(ops, Depth::DeepestFirst, hooks);
        if !report.done.is_empty() {
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
                tags: Vec::new(),
                tagged: 0,
                stamps: copy_stamps(&report.done),
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
        if let Some(p) = pending {
            let _ = fs::remove_file(p);
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
                    tagged: j.tagged,
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
        let id = self.new_id(&created);
        let j = Journal {
            created,
            undone: false,
            ops: Vec::new(),
            reverted: 0,
            failed: 0,
            extra_files: paths.to_vec(),
            dirs: Vec::new(),
            lists: Vec::new(),
            tags: Vec::new(),
            tagged: 0,
            stamps: Vec::new(),
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

    /// The folder an entry keeps originals in (lists, pictures taken out of
    /// songs).
    pub fn files_dir(&self, id: &str) -> Result<PathBuf, String> {
        self.backups(id)
    }

    /// Records songs whose tags were written along with a rename, so undo
    /// puts their old tags back.
    pub fn add_tags(&self, id: &str, backups: Vec<crate::tagwrite::Backup>) -> Result<(), String> {
        let mut j = self.read(id)?;
        // Copies written to are left so.
        for (path, st) in &mut j.stamps {
            if backups.iter().any(|b| b.path == *path)
                && let Some(now) = stamp(path)
            {
                *st = now;
            }
        }
        j.tagged += backups.len();
        trashed_originals(&mut j, &backups);
        j.tags.extend(backups);
        self.write(id, &j)
    }

    /// Starts writing tags for entry `id` (see [`Tagging`]).
    pub fn tagging(&self, id: &str) -> Result<Tagging, String> {
        let path = self.path(id)?.with_extension("tagging");
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        sync_dir(&path);
        Ok(Tagging { path, file })
    }

    /// Deletes a history entry (the files are left as they are).
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let _ = fs::remove_dir_all(self.backups(id)?);
        fs::remove_file(self.path(id)?).map_err(|e| e.to_string())
    }

    /// Deletes every entry; returns how many there were.
    pub fn clear(&self) -> Result<usize, String> {
        let ids: Vec<String> = self.list().into_iter().map(|e| e.id).collect();
        for id in &ids {
            self.delete(id)?;
        }
        Ok(ids.len())
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
        // Copies go while they are as they were left; a changed one stays
        // (and leaves the entry), with its lists and tags as they are.
        let mut kept = Vec::new();
        let mut removed = 0;
        let mut copies = Vec::new();
        let stamps = std::mem::take(&mut j.stamps);
        let before = j.ops.len();
        j.ops.retain(|op| {
            if !op.copy {
                return true;
            }
            let left = stamps.iter().find(|(p, _)| *p == op.to).map(|(_, s)| *s);
            match stamp(&op.to) {
                None => {}
                Some(now) if Some(now) == left => {
                    if remove_any(&op.to).is_err() {
                        return true;
                    }
                    removed += 1;
                }
                Some(_) => kept.push(op.to.clone()),
            }
            copies.push(op.clone());
            false
        });
        j.reverted += before - j.ops.len();
        j.stamps = stamps
            .into_iter()
            .filter(|(p, _)| j.ops.iter().any(|op| op.to == *p))
            .collect();
        let in_copy = |p: &Path| copies.iter().any(|c| p.starts_with(&c.to));
        j.tags.retain(|b| !in_copy(&b.path));
        j.lists.retain(|(p, _)| !in_copy(p));
        let backups = self.backups(id)?;
        // Songs get their old tags back where they are now, before the
        // renames take them back.
        let mut errors = Vec::new();
        // Noted like writing them, for the copies a stop would leave.
        let mut note = (!j.tags.is_empty())
            .then(|| self.tagging(id).ok())
            .flatten();
        j.tags.retain(|b| {
            if let Some(n) = note.as_mut() {
                n.start(&b.path);
            }
            match crate::tagwrite::restore(b, &backups) {
                Ok(()) => false,
                Err(e) => {
                    errors.push(OpError {
                        from: b.path.clone(),
                        to: b.path.clone(),
                        kind: ErrorKind::Tags,
                        detail: e,
                    });
                    true
                }
            }
        });
        if let Some(n) = note {
            n.finish();
        }
        // Rewritten lists get their content back where they are now; the
        // renames below then take them back.
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
        if j.lists.is_empty() && j.tags.is_empty() {
            let _ = fs::remove_dir_all(&backups);
        }
        let reverse: Vec<Operation> = j
            .ops
            .iter()
            .map(|op| Operation {
                from: op.to.clone(),
                to: op.from.clone(),
                copy: false,
                within: None,
            })
            .collect();
        // As for a rename: written first, so a stop halfway is recovered.
        let pending = self.pending_path(id).ok().filter(|path| {
            let p = Pending {
                created: j.created,
                ops: reverse.clone(),
                temps: reverse
                    .iter()
                    .enumerate()
                    .map(|(i, op)| temp_path(&op.from, i))
                    .collect(),
                undoing: true,
                parts: reverse
                    .iter()
                    .enumerate()
                    .map(|(i, op)| part_path(&op.to, i))
                    .collect(),
            };
            serde_json::to_vec(&p).is_ok_and(|t| write_synced(path, &t).is_ok())
        });
        let mut report = execute(&reverse, Depth::ShallowestFirst);
        report.kept = kept;
        report.copies_removed = removed;
        // The list goes back to the originals.
        report.done.extend(copies.into_iter().map(|op| Operation {
            from: op.to,
            to: op.from,
            copy: true,
            within: None,
        }));
        j.reverted += report.done.len();
        let back: HashSet<(&Path, &Path)> = report
            .done
            .iter()
            .map(|d| (d.to.as_path(), d.from.as_path()))
            .collect();
        j.ops
            .retain(|op| !back.contains(&(op.from.as_path(), op.to.as_path())));
        report.errors.extend(errors);
        j.failed = report.errors.len();
        if j.ops.is_empty() {
            // Innermost first; a folder that is not empty (the user put
            // something there) stays.
            for dir in j.dirs.iter().rev() {
                let _ = fs::remove_dir(dir);
            }
            j.dirs.clear();
        }
        j.undone =
            j.ops.is_empty() && j.extra_files.is_empty() && j.lists.is_empty() && j.tags.is_empty();
        self.write(id, &j)?;
        if let Some(p) = pending {
            let _ = fs::remove_file(p);
        }
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
            copy: false,
            within: None,
        }
    }

    fn copy(d: &Path, a: &str, b: &str) -> Operation {
        Operation {
            copy: true,
            ..op(d, a, b)
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
    fn undo_puts_tags_back() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::copy("tests/fixtures/song.mp3", p.join("a.mp3")).unwrap();
        let h = History::new(p.join("hist"));
        let id = h
            .apply(&[op(p, "a.mp3", "01 - Take On Me.mp3")])
            .history_id
            .unwrap();
        let m = crate::musicbrainz::SongMatch {
            title: "Take On Me".into(),
            track: 1,
            ..Default::default()
        };
        let song = p.join("01 - Take On Me.mp3");
        let b = crate::tagwrite::write(
            &song,
            &crate::tagwrite::fields_of(&m),
            Some(b"\xFF\xD8 cover"),
            false,
            &h.files_dir(&id).unwrap(),
        )
        .unwrap()
        .unwrap();
        h.add_tags(&id, vec![b]).unwrap();
        assert_eq!(h.list()[0].tagged, 1);
        assert_eq!(crate::music::read(&song).unwrap().title, "Take On Me");

        let u = h.undo(&id).unwrap();
        assert!(u.errors.is_empty(), "{:?}", u.errors);
        let t = crate::music::read(&p.join("a.mp3")).unwrap();
        assert_eq!(t.title, "Old");
        assert!(h.list()[0].undone);
        // The pictures kept for undo are gone with it.
        assert!(!h.files_dir(&id).unwrap().exists());
    }

    #[test]
    fn undo_takes_a_conversion_back() {
        use crate::convert::{Converted, Original, convert};
        let rar = include_bytes!("../../renome-archive/tests/data/rar4.rar");
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::write(p.join("old.cbr"), rar).unwrap();
        let h = History::new(p.join("hist"));
        // Renamed with the CBZ's extension, then converted.
        let id = h
            .apply(&[op(p, "old.cbr", "Hero 001.cbz")])
            .history_id
            .unwrap();
        let cbz = p.join("Hero 001.cbz");
        let done = convert(&cbz, &[], Original::Keep).unwrap();
        assert!(p.join("Hero 001.cbr").exists());
        h.add_tags(&id, vec![done.backup]).unwrap();
        let u = h.undo(&id).unwrap();
        assert!(u.errors.is_empty(), "{:?}", u.errors);
        assert_eq!(fs::read(p.join("old.cbr")).unwrap(), rar);
        assert!(!cbz.exists() && !p.join("Hero 001.cbr").exists());
        assert!(h.list()[0].undone);

        // The original gone to the trash (as told here, not done): the CBZ
        // takes the old name back, as a CBZ.
        let id = h
            .apply(&[op(p, "old.cbr", "Hero 001.cbz")])
            .history_id
            .unwrap();
        let mut done = convert(&cbz, &[], Original::Keep).unwrap();
        fs::remove_file(p.join("Hero 001.cbr")).unwrap();
        done.backup.converted = Some(Converted { original: None });
        h.add_tags(&id, vec![done.backup]).unwrap();
        let u = h.undo(&id).unwrap();
        assert!(u.errors.is_empty(), "{:?}", u.errors);
        assert_eq!(
            crate::convert::kind(&p.join("old.cbz")),
            Some(crate::convert::Kind::Zip)
        );
        assert!(!p.join("old.cbr").exists() && !cbz.exists());
    }

    #[test]
    fn tags_left_halfway_are_recovered() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::copy("tests/fixtures/song.mp3", p.join("a.mp3")).unwrap();
        fs::copy("tests/fixtures/song.mp3", p.join("b.mp3")).unwrap();
        let h = History::new(p.join("hist"));
        let id = h.apply(&[op(p, "a.mp3", "a2.mp3")]).history_id.unwrap();
        let m = crate::musicbrainz::SongMatch {
            title: "Take On Me".into(),
            track: 1,
            ..Default::default()
        };
        // One song written, the next stopped while in its copy.
        let mut note = h.tagging(&id).unwrap();
        let song = p.join("a2.mp3");
        note.start(&song);
        let fields = crate::tagwrite::fields_of(&m);
        let b = crate::tagwrite::write(&song, &fields, None, false, &h.files_dir(&id).unwrap())
            .unwrap()
            .unwrap();
        note.done(&b);
        let other = p.join("b.mp3");
        note.start(&other);
        let copy = crate::tagwrite::temp_of(&other);
        fs::copy(&other, &copy).unwrap();
        drop(note);

        let r = h.recover();
        assert_eq!(r.tagged, 1);
        assert!(!copy.exists());
        assert!(!p.join(format!("hist/{id}.tagging")).exists());
        assert_eq!(h.list()[0].tagged, 1);
        // Recovering again adds nothing.
        assert_eq!(h.recover().tagged, 0);
        let u = h.undo(&id).unwrap();
        assert!(u.errors.is_empty(), "{:?}", u.errors);
        assert_eq!(crate::music::read(&p.join("a.mp3")).unwrap().title, "Old");
    }

    #[test]
    fn clear_keeps_the_files() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::write(p.join("a"), "A").unwrap();
        fs::write(p.join("b"), "B").unwrap();
        let h = History::new(p.join("hist"));
        h.apply(&[op(p, "a", "a2")]);
        h.apply(&[op(p, "b", "b2")]);
        assert_eq!(h.clear().unwrap(), 2);
        assert!(h.list().is_empty());
        assert_eq!(
            (read(p.join("a2")), read(p.join("b2"))),
            ("A".into(), "B".into())
        );
        assert_eq!(h.clear().unwrap(), 0);
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
    fn recovers_a_stop_halfway() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        for n in ["a", "b", "c"] {
            fs::write(p.join(n), n).unwrap();
        }
        let h = History::new(p.join("hist"));
        // As if the program stopped: `a` is under its temporary name, `b`
        // was renamed, `c` was not touched.
        let ops = [op(p, "a", "a2"), op(p, "b", "b2"), op(p, "c", "c2")];
        let temps: Vec<PathBuf> = ops
            .iter()
            .enumerate()
            .map(|(i, o)| temp_path(&o.from, i))
            .collect();
        fs::rename(p.join("a"), &temps[0]).unwrap();
        fs::rename(p.join("b"), p.join("b2")).unwrap();
        fs::create_dir_all(p.join("hist")).unwrap();
        let pending = Pending {
            created: Local::now(),
            ops: ops.to_vec(),
            temps,
            undoing: false,
            parts: Vec::new(),
        };
        fs::write(
            p.join("hist/x-1.pending"),
            serde_json::to_string(&pending).unwrap(),
        )
        .unwrap();

        let r = h.recover();
        assert_eq!((r.restored, r.renamed, r.stuck.len()), (1, 1, 0));
        assert_eq!(read(p.join("a")), "a");
        assert_eq!(read(p.join("b2")), "b");
        assert!(!p.join("hist/x-1.pending").exists());
        // The rename that went through can be undone.
        assert_eq!(h.list().len(), 1);
        h.undo("x-1").unwrap();
        assert_eq!(read(p.join("b")), "b");
        // Nothing left to recover; a normal apply leaves nothing pending.
        h.apply(&[op(p, "c", "c3")]);
        assert_eq!(h.recover().restored, 0);
        assert_eq!(
            fs::read_dir(p.join("hist"))
                .unwrap()
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().ends_with(".pending"))
                .count(),
            0
        );
    }

    #[test]
    fn recovers_an_undo_stopped_halfway() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::write(p.join("a"), "a").unwrap();
        fs::write(p.join("b"), "b").unwrap();
        let h = History::new(p.join("hist"));
        let id = h
            .apply(&[op(p, "a", "New/a2"), op(p, "b", "New/b2")])
            .history_id
            .unwrap();
        // As if an undo stopped: `a` went back, `b` is under its temporary
        // name.
        let back = [op(p, "New/a2", "a"), op(p, "New/b2", "b")];
        let temps: Vec<PathBuf> = back
            .iter()
            .enumerate()
            .map(|(i, o)| temp_path(&o.from, i))
            .collect();
        fs::rename(p.join("New/a2"), p.join("a")).unwrap();
        fs::rename(p.join("New/b2"), &temps[1]).unwrap();
        let pending = Pending {
            created: Local::now(),
            ops: back.to_vec(),
            temps,
            undoing: true,
            parts: Vec::new(),
        };
        fs::write(
            h.pending_path(&id).unwrap(),
            serde_json::to_string(&pending).unwrap(),
        )
        .unwrap();
        let r = h.recover();
        assert_eq!((r.restored, r.undone), (1, 1));
        assert_eq!(read(p.join("New/b2")), "b");
        // The entry keeps only what is left to undo, and finishes it.
        assert_eq!(h.list()[0].reverted, 1);
        h.undo(&id).unwrap();
        assert_eq!(read(p.join("b")), "b");
        assert!(h.list()[0].undone);
        assert!(!p.join("New").exists());
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
    fn copies_and_undo() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::create_dir_all(p.join("in/Album")).unwrap();
        fs::write(p.join("in/a.txt"), "A").unwrap();
        fs::write(p.join("in/b.txt"), "B").unwrap();
        fs::write(p.join("in/Album/01.flac"), "song").unwrap();
        // The original's date goes with the copy.
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        File::options()
            .write(true)
            .open(p.join("in/a.txt"))
            .unwrap()
            .set_modified(old)
            .unwrap();
        let h = History::new(p.join("hist"));
        let r = h.apply(&[
            copy(p, "in/a.txt", "out/x/A.txt"),
            copy(p, "in/b.txt", "out/B.txt"),
            copy(p, "in/Album", "out/Album (1999)"),
        ]);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(read(p.join("in/a.txt")), "A");
        assert_eq!(read(p.join("out/x/A.txt")), "A");
        assert_eq!(read(p.join("out/Album (1999)/01.flac")), "song");
        assert_eq!(
            fs::metadata(p.join("out/x/A.txt"))
                .unwrap()
                .modified()
                .unwrap(),
            old
        );
        // Nothing passing is left.
        let names: Vec<String> = walkdir::WalkDir::new(p.join("out"))
            .into_iter()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|n| !n.starts_with(".renome")), "{names:?}");
        // One copy changed afterwards: it stays.
        fs::write(p.join("out/B.txt"), "edited").unwrap();
        let u = h.undo(r.history_id.as_deref().unwrap()).unwrap();
        assert!(u.errors.is_empty(), "{:?}", u.errors);
        assert_eq!(u.kept, [p.join("out/B.txt")]);
        // The list is told to go back to the originals.
        assert!(u.done.contains(&Operation {
            from: p.join("out/x/A.txt"),
            to: p.join("in/a.txt"),
            copy: true,
            within: None,
        }));
        assert!(!p.join("out/x").exists());
        assert!(!p.join("out/Album (1999)").exists());
        assert_eq!(read(p.join("out/B.txt")), "edited");
        // The originals as they were.
        assert_eq!(read(p.join("in/b.txt")), "B");
        assert_eq!(read(p.join("in/Album/01.flac")), "song");
        assert!(h.list()[0].undone);
        // A copy onto a name taken is refused, and nothing is touched.
        let r = h.apply(&[copy(p, "in/a.txt", "out/B.txt")]);
        assert_eq!(r.errors[0].kind, ErrorKind::TargetExists);
        assert_eq!(read(p.join("out/B.txt")), "edited");
    }

    #[test]
    fn copied_lists_follow_their_copies() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::create_dir_all(p.join("in")).unwrap();
        fs::write(p.join("in/01.mp3"), "x").unwrap();
        fs::write(p.join("in/list.m3u"), "01.mp3\n").unwrap();
        fs::write(p.join("in/other.m3u"), "01.mp3\n").unwrap();
        let h = History::new(p.join("hist"));
        let r = h.apply(&[
            copy(p, "in/01.mp3", "out/01 - Song.mp3"),
            copy(p, "in/list.m3u", "out/list.m3u"),
        ]);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(read(p.join("out/list.m3u")), "01 - Song.mp3\n");
        // The originals' lists still name the originals.
        assert_eq!(read(p.join("in/list.m3u")), "01.mp3\n");
        assert_eq!(read(p.join("in/other.m3u")), "01.mp3\n");
        // Undo: the copies were left so, and go.
        h.undo(r.history_id.as_deref().unwrap()).unwrap();
        assert!(!p.join("out").exists());
    }

    #[test]
    fn stops_between_files() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        for n in ["a", "b", "c"] {
            fs::write(p.join(n), n).unwrap();
        }
        let h = History::new(p.join("hist"));
        let seen = std::cell::Cell::new(0);
        let stop = || {
            seen.set(seen.get() + 1);
            seen.get() > 2
        };
        let r = h.apply_with(
            &[copy(p, "a", "a2"), copy(p, "b", "b2"), op(p, "c", "c2")],
            &Hooks {
                progress: &|_, _| {},
                stop: &stop,
            },
        );
        assert!(r.stopped);
        assert_eq!(r.done.len(), 1);
        // The others stay as they were, under their names.
        assert!(p.join("b").exists() && !p.join("b2").exists());
        assert!(p.join("c").exists() && !p.join("c2").exists());
        assert!(h.recover().restored == 0);
    }

    #[test]
    fn recovers_copies_stopped_halfway() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        for n in ["a", "b", "c"] {
            fs::write(p.join(n), n).unwrap();
        }
        fs::create_dir_all(p.join("out")).unwrap();
        let h = History::new(p.join("hist"));
        // `a` was copied whole; `b`'s copy was cut short; `c`, moved to
        // another disk, was copied whole but its original not deleted yet.
        let ops = [
            copy(p, "a", "out/a"),
            copy(p, "b", "out/b"),
            op(p, "c", "out/c"),
        ];
        let temps: Vec<PathBuf> = ops
            .iter()
            .enumerate()
            .map(|(i, o)| temp_path(&o.from, i))
            .collect();
        let parts: Vec<PathBuf> = ops
            .iter()
            .enumerate()
            .map(|(i, o)| part_path(&o.to, i))
            .collect();
        fs::copy(p.join("a"), p.join("out/a")).unwrap();
        fs::write(&parts[1], "half").unwrap();
        fs::rename(p.join("c"), &temps[2]).unwrap();
        fs::copy(&temps[2], p.join("out/c")).unwrap();
        let t = fs::metadata(&temps[2]).unwrap().modified().unwrap();
        File::options()
            .write(true)
            .open(p.join("out/c"))
            .unwrap()
            .set_modified(t)
            .unwrap();
        fs::create_dir_all(p.join("hist")).unwrap();
        let pending = Pending {
            created: Local::now(),
            ops: ops.to_vec(),
            temps,
            undoing: false,
            parts: parts.clone(),
        };
        fs::write(
            p.join("hist/x-1.pending"),
            serde_json::to_string(&pending).unwrap(),
        )
        .unwrap();
        let r = h.recover();
        assert_eq!((r.restored, r.renamed, r.stuck.len()), (1, 1, 0));
        assert!(!parts[1].exists());
        // `c` is back, its extra copy gone.
        assert_eq!(read(p.join("c")), "c");
        assert!(!p.join("out/c").exists());
        // The whole copy is in an entry that undo deletes.
        h.undo("x-1").unwrap();
        assert!(!p.join("out/a").exists());
        assert_eq!(read(p.join("a")), "a");
    }

    /// Two folders on different file systems, when this machine has them
    /// (`/dev/shm` beside the build folder, say).
    fn two_disks() -> Option<(tempfile::TempDir, tempfile::TempDir)> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let a = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).ok()?;
            for other in ["/dev/shm", "/tmp"] {
                if let Ok(b) = tempfile::tempdir_in(other)
                    && fs::metadata(a.path()).ok()?.dev() != fs::metadata(b.path()).ok()?.dev()
                {
                    return Some((a, b));
                }
            }
        }
        None
    }

    #[test]
    fn moves_to_another_disk() {
        let Some((a, b)) = two_disks() else {
            return;
        };
        fs::create_dir_all(a.path().join("Album")).unwrap();
        fs::write(a.path().join("song.mp3"), "s").unwrap();
        fs::write(a.path().join("Album/01.flac"), "1").unwrap();
        let h = History::new(a.path().join("hist"));
        let ops = [
            Operation {
                from: a.path().join("song.mp3"),
                to: b.path().join("x/Song.mp3"),
                copy: false,
                within: None,
            },
            Operation {
                from: a.path().join("Album"),
                to: b.path().join("Album (2001)"),
                copy: false,
                within: None,
            },
        ];
        let r = h.apply(&ops);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(!a.path().join("song.mp3").exists());
        assert!(!a.path().join("Album").exists());
        assert_eq!(read(b.path().join("x/Song.mp3")), "s");
        assert_eq!(read(b.path().join("Album (2001)/01.flac")), "1");
        h.undo(r.history_id.as_deref().unwrap()).unwrap();
        assert_eq!(read(a.path().join("song.mp3")), "s");
        assert_eq!(read(a.path().join("Album/01.flac")), "1");
        assert!(!b.path().join("x").exists());
    }

    #[test]
    fn sent_files_leave_the_added_folder() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::create_dir_all(p.join("home/Downloads/Album/CD1")).unwrap();
        fs::write(p.join("home/Downloads/Album/CD1/01.flac"), "1").unwrap();
        let h = History::new(p.join("hist"));
        let r = h.apply(&[Operation {
            within: Some(p.join("home/Downloads")),
            ..op(p, "home/Downloads/Album/CD1/01.flac", "lib/Album/01.flac")
        }]);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        // The folders below the one added go; it and those above stay.
        assert!(!p.join("home/Downloads/Album").exists());
        assert!(p.join("home/Downloads").is_dir());
        // Undo makes them again.
        h.undo(r.history_id.as_deref().unwrap()).unwrap();
        assert_eq!(read(p.join("home/Downloads/Album/CD1/01.flac")), "1");
    }

    #[test]
    fn rejects_path_traversal_ids() {
        let d = tempfile::tempdir().unwrap();
        assert!(History::new(d.path()).undo("../x").is_err());
    }
}
