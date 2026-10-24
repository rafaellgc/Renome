//! Cue sheets and playlists (`.cue`, `.m3u`, `.m3u8`) list songs by file
//! name. After a rename, the references to renamed files are updated so
//! they keep working.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::apply::Operation;

fn is_list(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_lowercase().as_str(), "cue" | "m3u" | "m3u8"))
}

/// `a/./b/../c` → `a/c`, without touching the disk.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// `to` written relative to the folder `from` (`../Other/x.flac`).
fn relative(to: &Path, from: &Path) -> PathBuf {
    let to: Vec<_> = to.components().collect();
    let from: Vec<_> = from.components().collect();
    let common = to.iter().zip(&from).take_while(|(a, b)| a == b).count();
    let mut out = PathBuf::new();
    for _ in common..from.len() {
        out.push("..");
    }
    for c in &to[common..] {
        out.push(c);
    }
    out
}

static CUE_FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(\s*FILE\s+)(?:"([^"]+)"|(\S+))(.*)$"#).unwrap());

/// The new form of a reference to a renamed file, or `None` to keep it.
/// `was` is the folder the list was in, `now` the one it is in.
fn new_reference(
    reference: &str,
    was: &Path,
    now: &Path,
    moved: &HashMap<PathBuf, PathBuf>,
) -> Option<String> {
    if reference.contains("://") {
        return None;
    }
    let windows = reference.contains('\\');
    let path = Path::new(&*reference.replace('\\', "/")).to_path_buf();
    let target = normalize(&was.join(&path));
    let renamed = match moved.get(&target) {
        Some(to) => to.clone(),
        // The file stayed, but the list moved: the path changes.
        None if was != now && !path.is_absolute() => target,
        None => return None,
    };
    let out = if path.is_absolute() {
        renamed
    } else {
        relative(&renamed, now)
    };
    let out = out.to_string_lossy().into_owned();
    let out = if windows { out.replace('/', "\\") } else { out };
    (out != reference).then_some(out)
}

/// Rewrites the references in a list's text; `None` if nothing changed.
fn rewrite(
    text: &str,
    cue: bool,
    was: &Path,
    now: &Path,
    moved: &HashMap<PathBuf, PathBuf>,
) -> Option<String> {
    let mut changed = false;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let end = &line[body.len()..];
        let new = if cue {
            CUE_FILE.captures(body).and_then(|c| {
                let reference = c.get(2).or_else(|| c.get(3))?.as_str();
                let r = new_reference(reference, was, now, moved)?;
                Some(format!("{}\"{r}\"{}", &c[1], &c[4]))
            })
        } else {
            let t = body.trim();
            (!t.is_empty() && !t.starts_with('#'))
                .then(|| new_reference(t, was, now, moved))
                .flatten()
        };
        match new {
            Some(n) => {
                changed = true;
                out.push_str(&n);
            }
            None => out.push_str(body),
        }
        out.push_str(end);
    }
    changed.then_some(out)
}

/// Text in UTF-8, or in Latin-1 (common in older cue sheets), and how to
/// write it back.
fn decode(bytes: &[u8]) -> (String, bool) {
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_string(), true),
        Err(_) => (bytes.iter().map(|&b| b as char).collect(), false),
    }
}

fn encode(text: &str, utf8: bool) -> Option<Vec<u8>> {
    if utf8 {
        return Some(text.as_bytes().to_vec());
    }
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).ok())
        .collect()
}

/// The file names a list refers to (URLs left out).
fn references(text: &str, cue: bool) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            if cue {
                let c = CUE_FILE.captures(line)?;
                Some(c.get(2).or_else(|| c.get(3))?.as_str().to_string())
            } else {
                let t = line.trim();
                (!t.is_empty() && !t.starts_with('#') && !t.contains("://")).then(|| t.to_string())
            }
        })
        .collect()
}

fn resolves(dir: &Path, reference: &str) -> bool {
    dir.join(reference.replace('\\', "/")).exists()
}

static CUE_TRACK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*TRACK\s+(\d+)\s+\S+").unwrap());
static CUE_INDEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*INDEX\s+(\d+)\s").unwrap());
static CUE_DISC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*REM\s+DISCNUMBER\s+(\d+)").unwrap());

/// Rebuilds a cue sheet whose files are gone (a whole-disc image later
/// split into tracks) for the songs in its folder: each `TRACK n` gets the
/// song tagged with track `n` (and the cue's disc, if it says), in a
/// `FILE` of its own starting at `00:00:00`. `None` when a track has no
/// song, or more than one.
fn rebuild_cue(text: &str, dir: &Path) -> Option<String> {
    let disc = text
        .lines()
        .find_map(|l| CUE_DISC.captures(l)?[1].parse::<u32>().ok());
    let mut by_track: HashMap<u32, Vec<String>> = HashMap::new();
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        if !crate::music::is_audio(ext) {
            continue;
        }
        let Some(tags) = crate::music::read(&path) else {
            continue;
        };
        if disc.is_some_and(|d| tags.disc.unwrap_or(1) != d) {
            continue;
        }
        if let Some(n) = tags.track {
            by_track.entry(n).or_default().push(name);
        }
    }
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out = String::new();
    let mut tracks = 0;
    for line in text.lines() {
        let t = line.trim_start();
        if CUE_FILE.is_match(line)
            || t.to_uppercase().starts_with("PREGAP")
            || t.to_uppercase().starts_with("POSTGAP")
        {
            continue;
        }
        if let Some(c) = CUE_TRACK.captures(line) {
            let n: u32 = c[1].parse().ok()?;
            let [file] = by_track.get(&n)?.as_slice() else {
                return None;
            };
            let kind = if file.to_lowercase().ends_with(".mp3") {
                "MP3"
            } else {
                "WAVE"
            };
            out.push_str(&format!(
                "FILE \"{file}\" {kind}{eol}  TRACK {n:02} AUDIO{eol}"
            ));
            tracks += 1;
        } else if let Some(c) = CUE_INDEX.captures(line) {
            // Each file starts at its track; gaps went with the files.
            if c[1].parse::<u32>().ok() == Some(1) {
                out.push_str(&format!("    INDEX 01 00:00:00{eol}"));
            }
        } else {
            out.push_str(line);
            out.push_str(eol);
        }
    }
    (tracks > 0).then_some(out)
}

/// What to do with a list: write a new content, or delete it.
pub struct ListChange {
    pub path: PathBuf,
    pub old: Vec<u8>,
    /// `None`: the list is useless and goes away.
    pub new: Option<Vec<u8>>,
}

/// The lists affected by the renames in `done` (lists in the folders files
/// left, and lists that moved themselves) and what to do with them:
/// references to renamed files are updated; a cue sheet none of whose
/// files exist is rebuilt for the songs beside it, or deleted when that is
/// not possible; a playlist none of whose (relative) entries exist is
/// deleted. Nothing is written.
pub fn changes(done: &[Operation]) -> Vec<ListChange> {
    let moved: HashMap<PathBuf, PathBuf> = done
        .iter()
        .map(|op| (normalize(&op.from), op.to.clone()))
        .collect();
    // (where the list was, where it is now)
    let mut lists: Vec<(PathBuf, PathBuf)> = done
        .iter()
        .filter(|op| is_list(&op.to))
        .map(|op| (op.from.clone(), op.to.clone()))
        .collect();
    let moved_lists: HashSet<PathBuf> = lists.iter().map(|(from, _)| from.clone()).collect();
    let dirs: HashSet<&Path> = done.iter().filter_map(|op| op.from.parent()).collect();
    for dir in dirs {
        let Ok(rd) = fs::read_dir(dir) else { continue };
        for p in rd.flatten().map(|e| e.path()) {
            if is_list(&p) && !moved_lists.contains(&p) {
                lists.push((p.clone(), p));
            }
        }
    }
    let mut out = Vec::new();
    for (was, now) in lists {
        let (Some(was_dir), Some(now_dir)) = (was.parent(), now.parent()) else {
            continue;
        };
        let Ok(bytes) = fs::read(&now) else { continue };
        let (text, utf8) = decode(&bytes);
        let cue = now
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("cue"));
        let rewritten = rewrite(&text, cue, was_dir, now_dir, &moved);
        let current = rewritten.as_deref().unwrap_or(&text);
        let refs = references(current, cue);
        let all_missing = !refs.is_empty() && refs.iter().all(|r| !resolves(now_dir, r));
        let new: Option<String> = if !all_missing {
            match rewritten {
                Some(t) => Some(t),
                None => continue,
            }
        } else if cue {
            rebuild_cue(current, now_dir)
        } else if refs
            .iter()
            .any(|r| Path::new(&r.replace('\\', "/")).is_absolute())
        {
            // Absolute paths may be on a drive that is not there now.
            match rewritten {
                Some(t) => Some(t),
                None => continue,
            }
        } else {
            None
        };
        let new = match new {
            // A name the old encoding cannot hold: leave the list alone.
            Some(t) => match encode(&t, utf8) {
                Some(b) => Some(b),
                None => continue,
            },
            None => None,
        };
        out.push(ListChange {
            path: now,
            old: bytes,
            new,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Applies the changes; returns how many lists were rewritten and
    /// deleted.
    fn update(done: &[Operation]) -> (usize, usize) {
        let mut n = (0, 0);
        for l in changes(done) {
            match l.new {
                Some(b) => {
                    fs::write(&l.path, b).unwrap();
                    n.0 += 1;
                }
                None => {
                    fs::remove_file(&l.path).unwrap();
                    n.1 += 1;
                }
            }
        }
        n
    }

    #[test]
    fn rewrites_lists() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join("old");
        let new = d.path().join("A/B");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(new.join("01 - Song.flac"), "a").unwrap();
        fs::write(new.join("2.flac"), "b").unwrap();
        fs::write(old.join("keep.flac"), "c").unwrap();
        // The cue moved with the songs; the m3u stayed behind.
        fs::write(
            new.join("album.cue"),
            "TITLE \"X\"\r\nFILE \"01 Song.flac\" WAVE\r\n  TRACK 01 AUDIO\r\nFILE 2.flac WAVE\r\n",
        )
        .unwrap();
        fs::write(
            old.join("list.m3u"),
            "#EXTM3U\n01 Song.flac\nkeep.flac\nhttp://x/y.mp3\n",
        )
        .unwrap();
        let done = vec![
            Operation {
                from: old.join("01 Song.flac"),
                to: new.join("01 - Song.flac"),
            },
            Operation {
                from: old.join("2.flac"),
                to: new.join("2.flac"),
            },
            Operation {
                from: old.join("album.cue"),
                to: new.join("album.cue"),
            },
        ];
        assert_eq!(update(&done), (2, 0));
        assert_eq!(
            fs::read_to_string(new.join("album.cue")).unwrap(),
            "TITLE \"X\"\r\nFILE \"01 - Song.flac\" WAVE\r\n  TRACK 01 AUDIO\r\nFILE 2.flac WAVE\r\n"
        );
        assert_eq!(
            fs::read_to_string(old.join("list.m3u")).unwrap(),
            "#EXTM3U\n../A/B/01 - Song.flac\nkeep.flac\nhttp://x/y.mp3\n"
        );
    }

    #[test]
    fn latin1_and_windows_paths() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        fs::write(p.join("list.m3u"), b"Can\xe7\xe3o.mp3\r\nsub\\x.mp3\r\n").unwrap();
        fs::create_dir(p.join("sub")).unwrap();
        fs::write(p.join("01 - Can\u{e7}\u{e3}o.mp3"), "a").unwrap();
        fs::write(p.join("sub/y.mp3"), "b").unwrap();
        let done = vec![
            Operation {
                from: p.join("Canção.mp3"),
                to: p.join("01 - Canção.mp3"),
            },
            Operation {
                from: p.join("sub/x.mp3"),
                to: p.join("sub/y.mp3"),
            },
        ];
        update(&done);
        assert_eq!(
            fs::read(p.join("list.m3u")).unwrap(),
            b"01 - Can\xe7\xe3o.mp3\r\nsub\\y.mp3\r\n"
        );
    }

    #[test]
    fn useless_lists_rebuilt_or_deleted() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        // A whole-disc cue whose image was split into tracks.
        let image = "REM GENRE Rock\nPERFORMER \"Korn\"\nTITLE \"Untouchables\"\nFILE \"Korn - Untouchables.flac\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"Here To Stay\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"Make Believe\"\n    INDEX 00 04:30:10\n    INDEX 01 04:31:00\n";
        fs::write(p.join("album.cue"), image).unwrap();
        fs::write(p.join("gone.m3u"), "nothing.mp3\n").unwrap();
        for (n, name) in [(1, "01 - Here To Stay.flac"), (2, "02 - Make Believe.flac")] {
            crate::music::flac(
                &p.join(name),
                &[("TITLE", "x"), ("TRACKNUMBER", &n.to_string())],
            );
        }
        fs::write(p.join("x.txt"), "x").unwrap();
        let done = vec![Operation {
            from: p.join("x.txt"),
            to: p.join("y.txt"),
        }];
        assert_eq!(update(&done), (1, 1));
        assert_eq!(
            fs::read_to_string(p.join("album.cue")).unwrap(),
            "REM GENRE Rock\nPERFORMER \"Korn\"\nTITLE \"Untouchables\"\nFILE \"01 - Here To Stay.flac\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"Here To Stay\"\n    INDEX 01 00:00:00\nFILE \"02 - Make Believe.flac\" WAVE\n  TRACK 02 AUDIO\n    TITLE \"Make Believe\"\n    INDEX 01 00:00:00\n"
        );
        assert!(!p.join("gone.m3u").exists());

        // A track without its song: the cue cannot be rebuilt and goes.
        fs::write(p.join("album.cue"), image.replace("TRACK 02", "TRACK 03")).unwrap();
        assert_eq!(update(&done), (0, 1));
    }
}
