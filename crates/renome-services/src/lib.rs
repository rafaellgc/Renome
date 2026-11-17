//! What the window and the command line share beyond the core: the online
//! catalogues and services, the keys in the system's password store, the
//! answers kept on disk, and where the program keeps its files.
//!
//! The work that reports progress or can be stopped takes a [`Host`]:
//! the window turns its reports into events for the page, the command
//! line prints them.

pub mod acoustid;
pub mod anilist;
pub mod comicvine;
pub mod discogs;
pub mod gcd;
pub mod googlebooks;
pub mod mangadex;
pub mod mangaupdates;
pub mod mediaserver;
pub mod metron;
pub mod musicbrainz;
pub mod openlibrary;
pub mod secrets;
pub mod subtitles;
pub mod tmdb;

use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

/// Whoever runs the work: hears its progress and says when to stop.
pub trait Host: Send + Sync {
    /// A report, named as the page's events are (`tmdb-progress`, …).
    fn report(&self, event: &str, payload: Value);
    /// A check that turns true once the person asks to stop the work
    /// begun when it was made.
    fn stop_signal(&self) -> Arc<dyn Fn() -> bool + Send + Sync>;
}

/// Sends a report made of any serializable value.
pub fn report<T: Serialize>(host: &dyn Host, event: &str, payload: T) {
    host.report(event, serde_json::to_value(payload).unwrap_or(Value::Null));
}

/// A host that hears nothing and never stops (tests, quiet runs).
pub struct Quiet;

impl Host for Quiet {
    fn report(&self, _: &str, _: Value) {}
    fn stop_signal(&self) -> Arc<dyn Fn() -> bool + Send + Sync> {
        Arc::new(|| false)
    }
}

/// The options file (`settings.json`), shared by the window and the
/// command line.
pub fn settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("renome")
        .join("settings.json")
}

/// The options as saved, if they can be read, with the remembered rules
/// and the rule sets brought up to date (see [`renome_core::catalog`]).
pub fn read_settings_file() -> Option<Value> {
    let text = std::fs::read_to_string(settings_path()).ok()?;
    let mut settings: Value = serde_json::from_str(&text).ok()?;
    migrate_settings(&mut settings);
    Some(settings)
}

/// Rules saved with old option names take the new ones.
pub fn migrate_settings(settings: &mut Value) {
    for key in ["rules", "ruleSets"] {
        if let Some(v) = settings.get_mut(key) {
            renome_core::catalog::migrate_steps(v);
        }
    }
}

/// Saves the options whole.
pub fn write_settings_file(settings: &Value) -> Result<(), String> {
    let path = settings_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// Where the History is kept.
pub fn history_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("renome")
        .join("history")
}

/// The catalogues' answers kept on disk (see [`renome_core::answers`]);
/// how long comes from the settings (`cacheDays`, 7 by default).
pub static ANSWERS: std::sync::LazyLock<renome_core::answers::Store> =
    std::sync::LazyLock::new(|| {
        let dir = cache_root().join("answers");
        renome_core::answers::Store::new(dir, cache_days(read_settings_file().as_ref()))
    });

/// Where the program keeps what it can always fetch or work out again.
/// On Windows not `%LOCALAPPDATA%\renome`: the installer puts the program
/// itself in `%LOCALAPPDATA%\Renome`, the same folder there, so clearing
/// the cache would take the program with it. The app's own data folder is
/// used instead, which the uninstaller offers to delete.
pub fn cache_root() -> PathBuf {
    let base = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    if cfg!(windows) {
        base.join("io.github.rafaellgc.Renome").join("cache")
    } else {
        base.join("renome")
    }
}

/// Windows: the cache once kept beside the installed program goes. Only
/// the names the program wrote there, never the folder.
pub fn drop_old_cache() {
    if cfg!(windows)
        && let Some(old) = dirs::cache_dir().map(|d| d.join("renome"))
    {
        let _ = std::fs::remove_dir_all(old.join("answers"));
        let _ = std::fs::remove_file(old.join("pictures"));
    }
}

/// How many days answers are kept, from the options.
pub fn cache_days(settings: Option<&Value>) -> u32 {
    settings
        .and_then(|s| s.get("cacheDays")?.as_u64())
        .map_or(7, |d| d.min(365) as u32)
}

/// A kept answer, read as `T`.
pub fn kept<T: for<'de> serde::Deserialize<'de>>(service: &str, question: &str) -> Option<T> {
    serde_json::from_slice(&ANSWERS.get(service, question)?).ok()
}

/// Keeps an answer and reads it as `T`.
pub fn keep<T: for<'de> serde::Deserialize<'de>>(
    service: &str,
    question: &str,
    body: &[u8],
) -> Result<T, String> {
    let v = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    ANSWERS.put(service, question, body);
    Ok(v)
}
