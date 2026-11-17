//! What the window and the command line share beyond the core: the online
//! catalogues and services, the keys in the system's password store, the
//! answers kept on disk, and where the program keeps its files.
//!
//! The work that reports progress or can be stopped takes a [`Host`]:
//! the window turns its reports into events for the page, the command
//! line prints them.

pub mod acoustid;
pub mod after;
pub mod anilist;
pub mod comics;
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

/// A report sender that can go to another thread.
pub type Sender = Arc<dyn Fn(&str, Value) + Send + Sync>;

/// Whoever runs the work: hears its progress and says when to stop.
pub trait Host: Send + Sync {
    /// A report, named as the page's events are (`tmdb-progress`, …).
    fn report(&self, event: &str, payload: Value);
    /// A check that turns true once the person asks to stop the work
    /// begun when it was made.
    fn stop_signal(&self) -> Arc<dyn Fn() -> bool + Send + Sync>;
    /// [`Host::report`] that can be taken along into work on another
    /// thread.
    fn sender(&self) -> Sender;
    /// Prints a book's page (`html`) to `pdf`, on pages of this size.
    /// Only the window can (its web view prints); elsewhere, `pdfPrint`.
    fn print_pdf(
        &self,
        html: PathBuf,
        pdf: PathBuf,
        page: renome_core::bookconv::print::Page,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let _ = (html, pdf, page);
        Box::pin(async { Err("pdfPrint".to_string()) })
    }
}

/// Sends a report made of any serializable value.
pub fn report<T: Serialize>(host: &dyn Host, event: &str, payload: T) {
    host.report(event, serde_json::to_value(payload).unwrap_or(Value::Null));
}

/// A host that hears nothing and never stops (tests, quiet runs).
pub struct Quiet;

impl Host for Quiet {
    fn report(&self, _: &str, _: Value) {}
    fn sender(&self) -> Sender {
        Arc::new(|_, _| {})
    }
    fn stop_signal(&self) -> Arc<dyn Fn() -> bool + Send + Sync> {
        Arc::new(|| false)
    }
}

/// The options file (`settings.json`), shared by the window and the
/// command line.
pub fn settings_path() -> PathBuf {
    if let Some(dir) = own_dir() {
        return dir.join("config").join("settings.json");
    }
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("renome")
        .join("settings.json")
}

/// `RENOME_DIR`: options, History and cache all inside this folder
/// instead of the system's places (tests; a profile kept apart).
fn own_dir() -> Option<PathBuf> {
    std::env::var_os("RENOME_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
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
    // Whole or not at all: a crash while writing leaves the old file (and
    // the keys kept there when the system has no password store).
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// Where the History is kept.
pub fn history_dir() -> PathBuf {
    if let Some(dir) = own_dir() {
        return dir.join("data").join("history");
    }
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
    if let Some(dir) = own_dir() {
        return dir.join("cache");
    }
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

/// The settings file is read and written whole by the settings and the
/// secrets apart: one at a time.
pub static SETTINGS_FILE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The secrets (keys, tokens, accounts): from the system's password store,
/// with those still in the settings file moved there. `{status:
/// "stored", secrets}`, or `{status: "refused" | "missing"}` when the
/// store did not answer or there is none (then they are in the file).
pub fn load_secrets() -> Value {
    let mut stored = match secrets::read() {
        secrets::Read::Found(m) => m,
        secrets::Read::Refused => return serde_json::json!({ "status": "refused" }),
        secrets::Read::Missing => return serde_json::json!({ "status": "missing" }),
    };
    let _file = SETTINGS_FILE.lock().unwrap();
    let mut file = read_settings_file();
    let old = file
        .as_ref()
        .and_then(Value::as_object)
        .map(secrets::of)
        .unwrap_or_default();
    if !old.is_empty() {
        for (k, v) in old {
            if v.as_str().is_some_and(|s| !s.is_empty()) {
                stored.insert(k, v);
            }
        }
        if secrets::write(&stored)
            && let Some(obj) = file.as_mut().and_then(Value::as_object_mut)
        {
            for k in secrets::FIELDS {
                obj.remove(k);
            }
            if let Some(f) = &file {
                write_settings_file(f).ok();
            }
        }
    }
    serde_json::json!({ "status": "stored", "secrets": stored })
}

/// Saves the secrets in the system's password store; in the settings
/// file when the store does not take them (or was not read: then what it
/// holds stays as it is). Whether the store took them.
pub fn save_secrets(secrets: serde_json::Map<String, Value>) -> Result<bool, String> {
    let secrets = secrets::of(&secrets);
    let stored = secrets::write(&secrets);
    let _file = SETTINGS_FILE.lock().unwrap();
    let mut file = read_settings_file().unwrap_or_else(|| Value::Object(Default::default()));
    if let Some(obj) = file.as_object_mut() {
        for k in secrets::FIELDS {
            obj.remove(k);
        }
        if !stored {
            obj.extend(
                secrets
                    .into_iter()
                    .filter(|(_, v)| v.as_str().is_some_and(|s| !s.is_empty())),
            );
        }
    }
    write_settings_file(&file)?;
    Ok(stored)
}

/// The user's own folders (home, Downloads, Desktop…): never renamed or
/// moved, even when added to the list.
pub fn protected_dirs() -> Vec<PathBuf> {
    [
        dirs::home_dir(),
        dirs::download_dir(),
        dirs::desktop_dir(),
        dirs::document_dir(),
        dirs::video_dir(),
        dirs::audio_dir(),
        dirs::picture_dir(),
        dirs::public_dir(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Held while files are renamed or put back, by the window or the command
/// line, so the two never do it at the same time (`renome.lock` beside
/// the History). Released when dropped.
pub struct RenameLock(#[allow(dead_code)] std::fs::File);

impl RenameLock {
    /// Waits for the other to finish, telling `waiting` once if it has to.
    pub fn take(waiting: impl FnOnce()) -> Result<Self, String> {
        let path = history_dir().with_file_name("renome.lock");
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        match file.try_lock() {
            Ok(()) => Ok(Self(file)),
            Err(std::fs::TryLockError::WouldBlock) => {
                waiting();
                file.lock().map_err(|e| e.to_string())?;
                Ok(Self(file))
            }
            Err(std::fs::TryLockError::Error(e)) => Err(e.to_string()),
        }
    }
}

/// System language (`pt_BR.UTF-8` → `pt`), from the locale variables.
pub fn system_language() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
        .map(|v| {
            v.chars()
                .take_while(|c| c.is_ascii_alphabetic())
                .collect::<String>()
                .to_lowercase()
        })
        .unwrap_or_default()
}
