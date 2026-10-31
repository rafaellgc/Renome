use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::files::FileEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanOptions {
    /// Descends into subfolders.
    pub recursive: bool,
    /// Includes files and folders whose name starts with a dot.
    pub include_hidden: bool,
    /// Adds the given folders themselves (to rename them) instead of
    /// their contents.
    pub folders_as_items: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            include_hidden: false,
            folders_as_items: false,
        }
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub files: Vec<FileEntry>,
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, Serialize)]
pub struct Skipped {
    pub path: PathBuf,
    pub reason: String,
}

/// A name starting with a dot, or on Windows a file marked hidden or
/// system (`desktop.ini`, `Thumbs.db`) or a folder marked hidden (Windows
/// marks folders with a custom look as system: they count).
fn is_hidden(p: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const HIDDEN: u32 = 0x2;
        const SYSTEM: u32 = 0x4;
        if let Ok(m) = std::fs::symlink_metadata(p) {
            let mask = if m.is_dir() { HIDDEN } else { HIDDEN | SYSTEM };
            if m.file_attributes() & mask != 0 {
                return true;
            }
        }
    }
    p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

/// Collects files from the given paths, skipping those already in `known`.
/// Files passed directly are always included, even hidden ones.
pub fn scan(paths: &[PathBuf], opts: &ScanOptions, known: &HashSet<PathBuf>) -> ScanResult {
    let mut seen = known.clone();
    let mut out = ScanResult::default();

    // `base`: the folder added, for the files found in it.
    let mut push = |path: &Path, base: Option<&Path>, out: &mut ScanResult| {
        if !seen.insert(path.to_path_buf()) {
            return;
        }
        match std::fs::symlink_metadata(path) {
            Ok(meta) => match FileEntry::from_path(path, &meta) {
                Some(mut e) => {
                    if let Some(b) = base {
                        e.base = b.to_path_buf();
                    }
                    out.files.push(e)
                }
                None => out.skipped.push(Skipped {
                    path: path.into(),
                    reason: "invalidUtf8".into(),
                }),
            },
            Err(e) => out.skipped.push(Skipped {
                path: path.into(),
                reason: e.to_string(),
            }),
        }
    };

    for path in paths {
        // A broken symlink is still a file that can be renamed.
        let meta = match std::fs::metadata(path).or_else(|_| std::fs::symlink_metadata(path)) {
            Ok(m) => m,
            Err(e) => {
                out.skipped.push(Skipped {
                    path: path.clone(),
                    reason: e.to_string(),
                });
                continue;
            }
        };
        if !meta.is_dir() || opts.folders_as_items {
            push(path, None, &mut out);
            continue;
        }
        let walker = WalkDir::new(path)
            .min_depth(1)
            .max_depth(if opts.recursive { usize::MAX } else { 1 })
            .sort_by_file_name()
            .into_iter()
            // The folder added itself is never left out.
            .filter_entry(|e| opts.include_hidden || e.depth() == 0 || !is_hidden(e.path()));
        for entry in walker {
            match entry {
                Ok(e) if e.file_type().is_dir() => {}
                Ok(e) => push(e.path(), Some(path), &mut out),
                Err(e) => out.skipped.push(Skipped {
                    path: e
                        .path()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| path.clone()),
                    reason: e.to_string(),
                }),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scan_folder() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        fs::write(root.join("a.JPG"), "").unwrap();
        fs::write(root.join(".oculto.jpg"), "").unwrap();
        fs::create_dir_all(root.join("sub/.git")).unwrap();
        fs::write(root.join("sub/c.jpg"), "").unwrap();
        fs::write(root.join("sub/.git/config"), "").unwrap();

        let names = |opts: &ScanOptions| {
            let r = scan(&[root.to_path_buf()], opts, &HashSet::new());
            let mut v: Vec<String> = r.files.iter().map(|f| f.file_name()).collect();
            v.sort();
            v
        };

        let mut o = ScanOptions::default();
        assert_eq!(names(&o), ["a.JPG", "c.jpg"]);
        o.include_hidden = true;
        assert_eq!(names(&o), [".oculto.jpg", "a.JPG", "c.jpg", "config"]);
        o = ScanOptions {
            recursive: false,
            include_hidden: false,
            folders_as_items: false,
        };
        assert_eq!(names(&o), ["a.JPG"]);
        // A hidden folder added on its own shows what it holds.
        let r = scan(
            &[root.join("sub/.git")],
            &ScanOptions::default(),
            &HashSet::new(),
        );
        assert_eq!(r.files.len(), 1);

        let r = scan(
            &[root.join("sub")],
            &ScanOptions {
                folders_as_items: true,
                ..Default::default()
            },
            &HashSet::new(),
        );
        assert_eq!(r.files.len(), 1);
        assert!(r.files[0].is_dir);
        assert_eq!(
            (r.files[0].stem.as_str(), r.files[0].ext.as_str()),
            ("sub", "")
        );
    }

    #[test]
    fn skips_known() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("x.txt");
        fs::write(&f, "").unwrap();
        let known = HashSet::from([f.clone()]);
        let r = scan(&[f.clone(), f], &ScanOptions::default(), &known);
        assert!(r.files.is_empty());
    }
}
