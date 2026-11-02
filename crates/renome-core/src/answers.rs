//! Answers of the online catalogues kept on disk for a while, so looking
//! up the same files again asks nothing: one file per answer, under a
//! folder per service, named by a hash of the question (which never holds
//! keys or passwords). An answer's age is its file's modification time.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

const DAY: u64 = 24 * 60 * 60;

/// The longest a service's answers may be kept, whatever the user chose:
/// TMDB's terms allow six months; Discogs wants its data fresh.
fn cap(service: &str) -> Duration {
    match service {
        "tmdb" => Duration::from_secs(180 * DAY),
        "discogs" => Duration::from_secs(6 * 60 * 60),
        _ => Duration::MAX,
    }
}

pub struct Store {
    dir: PathBuf,
    /// How many days answers are kept; 0: none are.
    days: AtomicU32,
}

impl Store {
    pub fn new(dir: PathBuf, days: u32) -> Self {
        Self {
            dir,
            days: AtomicU32::new(days),
        }
    }

    pub fn set_days(&self, days: u32) {
        self.days.store(days, Ordering::Relaxed);
    }

    pub fn days(&self) -> u32 {
        self.days.load(Ordering::Relaxed)
    }

    fn max_age(&self, service: &str) -> Duration {
        Duration::from_secs(u64::from(self.days()) * DAY).min(cap(service))
    }

    fn path(&self, service: &str, question: &str) -> PathBuf {
        let hash = xxhash_rust::xxh3::xxh3_128(question.as_bytes());
        self.dir.join(service).join(format!("{hash:032x}"))
    }

    /// The answer kept for `question`, when there is one young enough.
    pub fn get(&self, service: &str, question: &str) -> Option<Vec<u8>> {
        if self.days() == 0 {
            return None;
        }
        let path = self.path(service, question);
        let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default();
        if age > self.max_age(service) {
            let _ = std::fs::remove_file(&path);
            return None;
        }
        std::fs::read(&path).ok()
    }

    /// Keeps an answer (written whole or not at all).
    pub fn put(&self, service: &str, question: &str, answer: &[u8]) {
        if self.days() == 0 {
            return;
        }
        let path = self.path(service, question);
        let Some(dir) = path.parent() else { return };
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let part = path.with_extension("part");
        if std::fs::write(&part, answer).is_ok() && std::fs::rename(&part, &path).is_err() {
            let _ = std::fs::remove_file(&part);
        }
    }

    /// The bytes the answers take on disk.
    pub fn size(&self) -> u64 {
        files(&self.dir)
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum()
    }

    /// Deletes every answer kept; returns the bytes freed.
    pub fn clear(&self) -> u64 {
        let mut freed = 0;
        for p in files(&self.dir) {
            let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            if std::fs::remove_file(&p).is_ok() {
                freed += len;
            }
        }
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for e in entries.flatten() {
                let _ = std::fs::remove_dir(e.path());
            }
        }
        freed
    }

    /// Deletes the answers too old to be used (all of them when none are
    /// kept), and pieces of answers a crash left.
    pub fn purge(&self) {
        let now = SystemTime::now();
        for p in files(&self.dir) {
            let service = p
                .parent()
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let old = std::fs::metadata(&p)
                .and_then(|m| m.modified())
                .map(|t| now.duration_since(t).unwrap_or_default() > self.max_age(&service))
                .unwrap_or(true);
            if old || p.extension().is_some_and(|e| e == "part") {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
}

/// The answer files: one level of service folders.
fn files(dir: &Path) -> Vec<PathBuf> {
    let Ok(services) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    services
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| std::fs::read_dir(e.path()).ok())
        .flat_map(|d| d.flatten().map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_and_forgets() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path().join("answers"), 7);
        assert_eq!(store.get("mb", "q"), None);
        store.put("mb", "q", b"answer");
        assert_eq!(store.get("mb", "q").as_deref(), Some(&b"answer"[..]));
        assert_eq!(store.get("mb", "other"), None);
        assert_eq!(store.get("ol", "q"), None);
        assert_eq!(store.size(), 6);

        // Too old: dropped.
        let path = store.path("mb", "q");
        let old = SystemTime::now() - Duration::from_secs(8 * DAY);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(old)
            .unwrap();
        assert_eq!(store.get("mb", "q"), None);
        assert!(!path.exists());

        // Discogs answers last hours, whatever the days.
        store.put("discogs", "q", b"x");
        let path = store.path("discogs", "q");
        let hours = SystemTime::now() - Duration::from_secs(7 * 60 * 60);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(hours)
            .unwrap();
        store.purge();
        assert!(!path.exists());

        // Off: nothing kept or given.
        store.put("mb", "a", b"1");
        store.set_days(0);
        assert_eq!(store.get("mb", "a"), None);
        store.put("mb", "b", b"2");
        store.set_days(7);
        assert_eq!(store.get("mb", "b"), None);
        assert_eq!(store.clear(), 1);
        assert_eq!(store.size(), 0);
    }
}
