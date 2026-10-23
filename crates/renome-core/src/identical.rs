//! Finds files in the list with identical content. Each stage rules out
//! most candidates before the next, more expensive one:
//!
//! 1. size (already known; files of different sizes cannot be identical);
//! 2. same physical file (hard links), without reading anything;
//! 3. a hash of the first and last few KB;
//! 4. a hash of the whole file, only for what is left.
//!
//! Hashes are cached per path while the file's size and modification time
//! on disk stay the same, so checking again only reads what changed.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use xxhash_rust::xxh3::{Xxh3, xxh3_128};

use crate::files::{FileEntry, natural_cmp};

/// Bytes read from each end in stage 3. Files up to twice this size are
/// read whole there, so that hash is final.
const EDGE: u64 = 4096;
const CHUNK: usize = 1 << 20;

/// What identifies a file's current content on disk.
#[derive(Clone, Copy, PartialEq)]
struct Stamp {
    size: u64,
    modified: Option<SystemTime>,
}

#[derive(Clone, Copy)]
struct Cached {
    stamp: Stamp,
    edges: Option<u128>,
    full: Option<u128>,
}

#[derive(Default)]
pub struct HashCache {
    entries: HashMap<PathBuf, Cached>,
}

impl HashCache {
    fn get(&mut self, path: &Path, stamp: Stamp) -> &mut Cached {
        let fresh = Cached {
            stamp,
            edges: None,
            full: None,
        };
        let c = self.entries.entry(path.to_path_buf()).or_insert(fresh);
        if c.stamp != stamp {
            *c = fresh;
        }
        c
    }
}

/// Reports progress of stage 4: bytes read and bytes to read.
pub type Progress<'a> = &'a mut dyn FnMut(u64, u64);

/// Groups of identical files (two or more each, sorted by path). `None`
/// when `cancelled` returned true. Folders, symlinks and empty files are
/// left out; files that cannot be read are treated as unique.
pub fn find_identical(
    files: &[FileEntry],
    cache: &mut HashCache,
    progress: Progress,
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<Vec<PathBuf>>> {
    // 1. Same size.
    let mut by_size: HashMap<u64, Vec<&FileEntry>> = HashMap::new();
    for f in files.iter().filter(|f| !f.is_dir && f.size > 0) {
        by_size.entry(f.size).or_default().push(f);
    }
    let mut groups: Vec<Vec<&FileEntry>> = by_size.into_values().filter(|g| g.len() > 1).collect();

    // 2. Hard links: keep one file per physical file for reading; the
    //    others follow it.
    let mut links: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    let mut stamps: HashMap<PathBuf, Stamp> = HashMap::new();
    for g in &mut groups {
        let mut seen: HashMap<(u64, u64), PathBuf> = HashMap::new();
        g.retain(|f| match on_disk(&f.path) {
            None => false,
            Some((stamp, id)) => {
                stamps.insert(f.path.clone(), stamp);
                match id {
                    Some(id) => match seen.get(&id) {
                        Some(first) => {
                            links.entry(first.clone()).or_default().push(f.path.clone());
                            false
                        }
                        None => {
                            seen.insert(id, f.path.clone());
                            true
                        }
                    },
                    None => true,
                }
            }
        });
    }

    // 3. Start and end.
    let mut next = Vec::new();
    for g in groups {
        let mut by_edges: HashMap<u128, Vec<&FileEntry>> = HashMap::new();
        for f in g {
            if cancelled() {
                return None;
            }
            let c = cache.get(&f.path, stamps[&f.path]);
            if c.edges.is_none() {
                c.edges = hash_edges(&f.path, c.stamp.size);
            }
            match c.edges {
                Some(h) => by_edges.entry(h).or_default().push(f),
                None => {
                    // Unreadable, but its hard links are still identical.
                    if links.contains_key(&f.path) {
                        next.push(vec![f]);
                    }
                }
            }
        }
        next.extend(by_edges.into_values());
    }

    // 4. Whole file, only where the edges did not already cover it.
    let total: u64 = next
        .iter()
        .filter(|g| g.len() > 1)
        .flatten()
        .filter(|f| f.size > 2 * EDGE && cache.get(&f.path, stamps[&f.path]).full.is_none())
        .map(|f| f.size)
        .sum();
    let mut done = 0;
    let mut result: Vec<Vec<PathBuf>> = Vec::new();
    for g in next {
        let mut by_full: HashMap<u128, Vec<&FileEntry>> = HashMap::new();
        if g.len() > 1 && g[0].size > 2 * EDGE {
            for f in g {
                let stamp = stamps[&f.path];
                let c = cache.get(&f.path, stamp);
                if c.full.is_none() {
                    c.full = hash_full(&f.path, &mut |n| {
                        done += n;
                        progress(done, total);
                        !cancelled()
                    });
                    if cancelled() {
                        return None;
                    }
                }
                if let Some(h) = cache.get(&f.path, stamp).full {
                    by_full.entry(h).or_default().push(f);
                }
            }
        } else {
            by_full.insert(0, g);
        }
        for g in by_full.into_values() {
            let mut paths: Vec<PathBuf> = Vec::new();
            for f in g {
                paths.push(f.path.clone());
                paths.extend(links.get(&f.path).into_iter().flatten().cloned());
            }
            if paths.len() > 1 {
                paths.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
                result.push(paths);
            }
        }
    }
    result.sort_by(|a, b| natural_cmp(&a[0].to_string_lossy(), &b[0].to_string_lossy()));
    Some(result)
}

/// The file's stamp and physical id (`None` where the system has no file
/// ids). `None` for symlinks and missing files, which are left out.
fn on_disk(path: &Path) -> Option<(Stamp, Option<(u64, u64)>)> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let stamp = Stamp {
        size: meta.len(),
        modified: meta.modified().ok(),
    };
    #[cfg(unix)]
    let id = {
        use std::os::unix::fs::MetadataExt;
        Some((meta.dev(), meta.ino()))
    };
    #[cfg(not(unix))]
    let id = None;
    Some((stamp, id))
}

fn hash_edges(path: &Path, size: u64) -> Option<u128> {
    let mut file = File::open(path).ok()?;
    let mut buf = Vec::with_capacity((2 * EDGE).min(size) as usize);
    if size <= 2 * EDGE {
        file.read_to_end(&mut buf).ok()?;
    } else {
        (&mut file).take(EDGE).read_to_end(&mut buf).ok()?;
        file.seek(SeekFrom::End(-(EDGE as i64))).ok()?;
        file.read_to_end(&mut buf).ok()?;
    }
    Some(xxh3_128(&buf))
}

/// `step` gets the bytes read so far in each chunk and returns false to stop.
fn hash_full(path: &Path, step: &mut dyn FnMut(u64) -> bool) -> Option<u128> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Xxh3::new();
    let mut buf = vec![0; CHUNK];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            return Some(hasher.digest128());
        }
        hasher.update(&buf[..n]);
        if !step(n as u64) {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn entries(paths: &[PathBuf]) -> Vec<FileEntry> {
        paths
            .iter()
            .map(|p| FileEntry::from_path(p, &fs::symlink_metadata(p).unwrap()).unwrap())
            .collect()
    }

    fn run(files: &[FileEntry], cache: &mut HashCache) -> Vec<Vec<String>> {
        find_identical(files, cache, &mut |_, _| {}, &|| false)
            .unwrap()
            .into_iter()
            .map(|g| {
                g.iter()
                    .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn finds_copies() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        let big: Vec<u8> = (0..50_000u32).map(|i| (i % 251) as u8).collect();
        let mut middle = big.clone();
        middle[25_000] ^= 1; // same size, start and end; differs in the middle
        fs::write(p.join("a.bin"), &big).unwrap();
        fs::write(p.join("b copy.bin"), &big).unwrap();
        fs::write(p.join("c.bin"), &middle).unwrap();
        fs::write(p.join("s1.txt"), "small").unwrap();
        fs::write(p.join("s2.txt"), "small").unwrap();
        fs::write(p.join("s3.txt"), "SMALL").unwrap();
        fs::write(p.join("e1"), "").unwrap();
        fs::write(p.join("e2"), "").unwrap();
        fs::hard_link(p.join("s3.txt"), p.join("s3 link.txt")).unwrap();
        std::os::unix::fs::symlink(p.join("a.bin"), p.join("link.bin")).unwrap();
        let names = [
            "a.bin",
            "b copy.bin",
            "c.bin",
            "s1.txt",
            "s2.txt",
            "s3.txt",
            "s3 link.txt",
            "e1",
            "e2",
            "link.bin",
        ];
        let files = entries(&names.map(|n| p.join(n)));
        let mut cache = HashCache::default();
        let groups = run(&files, &mut cache);
        assert_eq!(
            groups,
            [
                vec!["a.bin", "b copy.bin"],
                vec!["s1.txt", "s2.txt"],
                vec!["s3 link.txt", "s3.txt"]
            ]
        );
        // The cache answers again without changes; a changed file is reread.
        assert_eq!(run(&files, &mut cache), groups);
        fs::write(p.join("s2.txt"), "other").unwrap();
        let files = entries(&names.map(|n| p.join(n)));
        assert_eq!(run(&files, &mut cache).len(), 2);
    }

    #[test]
    fn cancels() {
        let d = tempfile::tempdir().unwrap();
        let data = vec![7u8; 100_000];
        fs::write(d.path().join("a"), &data).unwrap();
        fs::write(d.path().join("b"), &data).unwrap();
        let files = entries(&[d.path().join("a"), d.path().join("b")]);
        let r = find_identical(&files, &mut HashCache::default(), &mut |_, _| {}, &|| true);
        assert!(r.is_none());
    }
}
