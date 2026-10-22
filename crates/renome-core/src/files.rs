use std::cmp::Ordering;
use std::fs::Metadata;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A file in the list, with its name already split into stem and extension.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: PathBuf,
    pub dir: PathBuf,
    /// Name without the extension (`photo.tar` for `photo.tar.gz`; `.bashrc` for `.bashrc`).
    pub stem: String,
    /// Extension without the dot; empty when there is none.
    pub ext: String,
    pub size: u64,
    /// Modification time in Unix seconds.
    pub modified: Option<i64>,
}

impl FileEntry {
    /// Builds the entry from a path; `None` if the name is not valid UTF-8.
    pub fn from_path(path: &Path, meta: &Metadata) -> Option<Self> {
        let name = path.file_name()?.to_str()?;
        let (stem, ext) = split_name(name);
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        Some(Self {
            path: path.to_path_buf(),
            dir: path.parent().map(Path::to_path_buf).unwrap_or_default(),
            stem: stem.to_string(),
            ext: ext.to_string(),
            size: meta.len(),
            modified,
        })
    }

    pub fn file_name(&self) -> String {
        join_name(&self.stem, &self.ext)
    }
}

/// Splits `name.ext` at the last dot. Hidden files (`.bashrc`) and names
/// ending in a dot have no extension.
pub fn split_name(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => (&name[..i], &name[i + 1..]),
        _ => (name, ""),
    }
}

pub fn join_name(stem: &str, ext: &str) -> String {
    if ext.is_empty() {
        stem.to_string()
    } else {
        format!("{stem}.{ext}")
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SortKey {
    Name,
    Ext,
    Dir,
    Modified,
    Size,
}

pub fn sort_files(files: &mut [FileEntry], key: SortKey, descending: bool) {
    // sort_by is stable: ties keep their previous order.
    files.sort_by(|a, b| {
        let ord = match key {
            SortKey::Name => natural_cmp(&a.file_name(), &b.file_name()),
            SortKey::Ext => natural_cmp(&a.ext, &b.ext),
            SortKey::Dir => natural_cmp(&a.dir.to_string_lossy(), &b.dir.to_string_lossy()),
            SortKey::Modified => a.modified.cmp(&b.modified),
            SortKey::Size => a.size.cmp(&b.size),
        };
        if descending { ord.reverse() } else { ord }
    });
}

/// "Natural" comparison: runs of digits compare by value (`photo2 < photo10`)
/// and letters compare case-insensitively.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na = take_digits(&mut ai);
                let nb = take_digits(&mut bi);
                let ta = na.trim_start_matches('0');
                let tb = nb.trim_start_matches('0');
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                ai.next();
                bi.next();
            }
        }
    }
}

fn take_digits(it: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut s = String::new();
    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
        s.push(c);
        it.next();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split() {
        assert_eq!(split_name("a.tar.gz"), ("a.tar", "gz"));
        assert_eq!(split_name(".bashrc"), (".bashrc", ""));
        assert_eq!(split_name("semext"), ("semext", ""));
        assert_eq!(split_name("ponto."), ("ponto.", ""));
    }

    #[test]
    fn natural() {
        let mut v = vec!["foto10", "Foto2", "foto1", "foto02b", "abc"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["abc", "foto1", "Foto2", "foto02b", "foto10"]);
    }
}
