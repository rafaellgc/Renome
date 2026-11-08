use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use renome_core::links::{self, Link};
use renome_core::music::AudioTags;
use renome_core::rules::{any_song_tags, song_tags};
use renome_core::scan::Skipped;
use renome_core::{
    ApplyReport, FileEntry, HashCache, History, HistoryEntry, Operation, Preview, ScanOptions,
    SortKey, Step, View,
};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

mod acoustid;
mod anilist;
mod comicvine;
mod discogs;
mod gcd;
mod googlebooks;
mod mangadex;
mod mangaupdates;
mod metron;
mod musicbrainz;
mod openlibrary;
mod secrets;
mod subtitles;
mod tmdb;

/// The session's file list. It lives on the Rust side so the whole list
/// is not sent over on every preview update.
#[derive(Default)]
struct Session {
    files: Vec<FileEntry>,
    /// Earlier states of the list and of the hand-edited names, for Ctrl+Z.
    undo: Vec<(Vec<FileEntry>, Overrides)>,
    /// Group of each file with an identical copy in the list.
    identical: HashMap<PathBuf, usize>,
    /// Group of each song that is the same music as another in the list.
    alike: HashMap<PathBuf, usize>,
    /// Group of each picture that looks like another in the list.
    similar: HashMap<PathBuf, usize>,
}

/// How many list edits can be undone.
const UNDO_LIMIT: usize = 100;

struct AppState {
    session: Mutex<Session>,
    history: History,
    hashes: Mutex<HashCache>,
    /// Fingerprints of the catalogues' covers, by address (`None`: could
    /// not be read).
    covers: tokio::sync::Mutex<HashMap<String, Option<u64>>>,
    /// Fingerprints of pictures, kept between sessions.
    pictures: Mutex<renome_core::similar::PictureCache>,
    /// Bumped by each search for identical files; an older search that sees
    /// a newer number stops.
    identical_run: AtomicU64,
    /// Held while a preview reads the files or a renaming writes them: the
    /// searches for copies wait meanwhile (on a hard disk, two readers
    /// each go slower than one after the other).
    busy_disk: Mutex<()>,
    /// Bumped by Stop; a search that sees a newer number than it started
    /// with stops, keeping what it found.
    stop_run: AtomicU64,
    /// MusicBrainz, paced for all lookups together.
    mb: musicbrainz::Client,
    /// AcoustID, likewise.
    acoustid: acoustid::Client,
    /// The recordings each song was heard as (AcoustID), for the album
    /// review's choices.
    heard: Mutex<HashMap<PathBuf, Vec<String>>>,
    /// Open Library, likewise.
    ol: openlibrary::Client,
    /// Comic Vine, likewise, with its answers kept.
    cv: comicvine::Client,
    /// The Grand Comics Database, and the account signed in (in memory).
    gcd: gcd::Client,
    /// MangaDex.
    md: mangadex::Client,
    /// AniList (manga), with its answers kept.
    al: anilist::Client,
    /// MangaUpdates, likewise.
    mu: mangaupdates::Client,
    /// Metron, with the user's token, likewise.
    mt: metron::Client,
    /// Discogs, with the user's token.
    dg: discogs::Client,
}

type Overrides = HashMap<PathBuf, String>;

/// The catalogues' answers kept on disk (see [`renome_core::answers`]);
/// how long comes from the settings (`cacheDays`, 7 by default).
pub(crate) static ANSWERS: std::sync::LazyLock<renome_core::answers::Store> =
    std::sync::LazyLock::new(|| {
        let dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("renome")
            .join("answers");
        renome_core::answers::Store::new(dir, cache_days(read_settings_file().as_ref()))
    });

fn cache_days(settings: Option<&Value>) -> u32 {
    settings
        .and_then(|s| s.get("cacheDays")?.as_u64())
        .map_or(7, |d| d.min(365) as u32)
}

/// A kept answer, read as `T`.
pub(crate) fn kept<T: for<'de> serde::Deserialize<'de>>(
    service: &str,
    question: &str,
) -> Option<T> {
    serde_json::from_slice(&ANSWERS.get(service, question)?).ok()
}

/// Keeps an answer and reads it as `T`.
pub(crate) fn keep<T: for<'de> serde::Deserialize<'de>>(
    service: &str,
    question: &str,
    body: &[u8],
) -> Result<T, String> {
    let v = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    ANSWERS.put(service, question, body);
    Ok(v)
}

/// The bytes the kept answers take.
#[tauri::command]
async fn cache_size() -> u64 {
    tauri::async_runtime::spawn_blocking(|| ANSWERS.size())
        .await
        .unwrap_or(0)
}

/// Forgets the kept answers; returns the bytes freed.
#[tauri::command]
async fn clear_cache() -> u64 {
    tauri::async_runtime::spawn_blocking(|| ANSWERS.clear())
        .await
        .unwrap_or(0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddResult {
    /// Paths actually added (so the UI can select them).
    added: Vec<PathBuf>,
    skipped: Vec<Skipped>,
}

/// Saves the current state before the UI changes the list.
#[tauri::command]
async fn checkpoint(app: AppHandle, overrides: Overrides) -> Result<(), String> {
    in_background(app, move |state| {
        let s = &mut *state.session.lock().unwrap();
        if s.undo.len() == UNDO_LIMIT {
            s.undo.remove(0);
        }
        s.undo.push((s.files.clone(), overrides));
    })
    .await
}

/// Drops the last checkpoint (the edit changed nothing).
#[tauri::command]
async fn drop_checkpoint(app: AppHandle) -> Result<(), String> {
    in_background(app, |state| {
        state.session.lock().unwrap().undo.pop();
    })
    .await
}

/// Restores the list to the last checkpoint and returns its hand-edited
/// names; `None` when there is nothing to undo.
#[tauri::command]
async fn undo_list(app: AppHandle) -> Result<Option<Overrides>, String> {
    in_background(app, |state| {
        let s = &mut *state.session.lock().unwrap();
        let (files, overrides) = s.undo.pop()?;
        s.files = files;
        Some(overrides)
    })
    .await
}

/// The user's own folders (home, Downloads, Videos…), never taken for a
/// show's folder to rename.
fn protected_dirs() -> Vec<PathBuf> {
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

/// Runs `work` away from the window's thread (commands without `async`
/// run on it and freeze the window while they last).
async fn in_background<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&AppState) -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || work(&app.state::<AppState>()))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_paths(
    app: AppHandle,
    paths: Vec<PathBuf>,
    options: ScanOptions,
) -> Result<AddResult, String> {
    in_background(app, move |state| {
        let known: HashSet<PathBuf> = {
            let s = state.session.lock().unwrap();
            s.files.iter().map(|f| f.path.clone()).collect()
        };
        // Scanned without holding the list: a big folder takes a while.
        let result = renome_core::scan(&paths, &options, &known);
        let added = result.files.iter().map(|f| f.path.clone()).collect();
        state.session.lock().unwrap().files.extend(result.files);
        AddResult {
            added,
            skipped: result.skipped,
        }
    })
    .await
}

#[tauri::command]
async fn remove_files(app: AppHandle, paths: Vec<PathBuf>) -> Result<(), String> {
    in_background(app, move |state| {
        let remove: HashSet<PathBuf> = paths.into_iter().collect();
        state
            .session
            .lock()
            .unwrap()
            .files
            .retain(|f| !remove.contains(&f.path));
    })
    .await
}

#[tauri::command]
async fn clear_files(app: AppHandle) -> Result<(), String> {
    in_background(app, |state| state.session.lock().unwrap().files.clear()).await
}

/// Moves the selected files one position up (`-1`) or down (`1`).
#[tauri::command]
async fn move_files(app: AppHandle, paths: Vec<PathBuf>, delta: i32) -> Result<(), String> {
    in_background(app, move |state| {
        move_in(&mut state.session.lock().unwrap().files, &paths, delta)
    })
    .await
}

fn move_in(files: &mut [FileEntry], paths: &[PathBuf], delta: i32) {
    let selected: HashSet<&PathBuf> = paths.iter().collect();
    let n = files.len();
    let order: Vec<usize> = if delta < 0 {
        (1..n).collect()
    } else {
        (0..n.saturating_sub(1)).rev().collect()
    };
    for i in order {
        let j = if delta < 0 { i - 1 } else { i + 1 };
        if selected.contains(&files[i].path) && !selected.contains(&files[j].path) {
            files.swap(i, j);
        }
    }
}

/// Sorts the list; by a column of details it reads the files, so in the
/// background.
#[tauri::command]
async fn sort_files(app: AppHandle, key: SortKey, descending: bool) -> Result<(), String> {
    in_background(app, move |state| {
        renome_core::sort_files(&mut state.session.lock().unwrap().files, key, descending);
    })
    .await
}

/// What the optional columns show about these files (read once each).
#[tauri::command]
async fn file_details(
    app: AppHandle,
    paths: Vec<PathBuf>,
) -> Result<HashMap<PathBuf, renome_core::details::Details>, String> {
    in_background(app, move |state| {
        let want: HashSet<PathBuf> = paths.into_iter().collect();
        let s = state.session.lock().unwrap();
        s.files
            .iter()
            .filter(|f| want.contains(&f.path))
            .map(|f| (f.path.clone(), renome_core::details::of(f)))
            .collect()
    })
    .await
}

/// Each song's own tags (with what its name says, with `from_name`), as
/// a start for typing its details in; `None` for what is not a song.
#[tauri::command]
async fn song_own_tags(
    app: AppHandle,
    paths: Vec<PathBuf>,
    from_name: bool,
) -> Result<Vec<Option<renome_core::musicbrainz::SongMatch>>, String> {
    in_background(app, move |state| {
        let s = state.session.lock().unwrap();
        let by_path: HashMap<&PathBuf, &renome_core::FileEntry> =
            s.files.iter().map(|f| (&f.path, f)).collect();
        paths
            .iter()
            .map(|p| {
                let f = by_path.get(p)?;
                song_tags(f, from_name)
                    .or_else(|| any_song_tags(f))
                    .map(|t| renome_core::musicbrainz::SongMatch::from_tags(&t))
            })
            .collect()
    })
    .await
}

#[tauri::command]
async fn preview(
    app: AppHandle,
    steps: Vec<Step>,
    overrides: Overrides,
    mut view: View,
) -> Result<Preview, String> {
    let emitter = app.clone();
    in_background(app, move |state| {
        let _disk = state.busy_disk.lock().unwrap();
        // Read on a copy of the list: the list stays free for the window
        // (the first reading of thousands of photos takes a minute).
        let files = {
            let s = state.session.lock().unwrap();
            view.identical = s.identical.clone();
            view.alike = s.alike.clone();
            view.similar = s.similar.clone();
            s.files.clone()
        };
        view.protected = protected_dirs();
        if steps
            .iter()
            .any(|s| s.enabled && matches!(s.rule, renome_core::rules::Rule::Photo { .. }))
        {
            let list: Vec<&FileEntry> = files.iter().collect();
            let last = Mutex::new(Instant::now());
            renome_core::photos::read_all(&list, &|done, total| {
                let mut last = last.lock().unwrap();
                if last.elapsed() > Duration::from_millis(150) || done == total {
                    *last = Instant::now();
                    let _ = emitter.emit(
                        "photos-progress",
                        IdenticalProgress {
                            done: done as u64,
                            total: total as u64,
                        },
                    );
                }
            });
        }
        let preview = renome_core::preview(&files, &steps, &overrides, &view);
        // What was read stays known.
        let read: HashMap<&PathBuf, &FileEntry> = files.iter().map(|f| (&f.path, f)).collect();
        for f in state.session.lock().unwrap().files.iter_mut() {
            if let Some(other) = read.get(&f.path) {
                f.keep_read(other);
            }
        }
        preview
    })
    .await
}

#[derive(Clone, Serialize)]
struct IdenticalProgress {
    done: u64,
    total: u64,
}

/// Whether Stop was pressed since this was called.
fn stop_signal(app: &AppHandle) -> impl Fn() -> bool + Send + Sync + 'static {
    let app = app.clone();
    let run = app.state::<AppState>().stop_run.load(Ordering::SeqCst);
    move || app.state::<AppState>().stop_run.load(Ordering::SeqCst) != run
}

/// Stops the searches under way (see [`stop_signal`]).
#[tauri::command]
fn stop_search(state: State<AppState>) {
    state.stop_run.fetch_add(1, Ordering::SeqCst);
}

/// What the search for copies found: identical files, and songs that are
/// the same music in different files.
#[derive(Serialize)]
struct Copies {
    identical: Vec<Vec<PathBuf>>,
    songs: Vec<Vec<PathBuf>>,
}

/// Searches the list for files with identical content, then for songs
/// that are the same music, in the background. Returns the groups, or
/// `None` if a newer search replaced this one. Emits `identical-progress`
/// while reading whole files.
#[tauri::command]
async fn find_identical(app: AppHandle) -> Result<Option<Copies>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let run = state.identical_run.fetch_add(1, Ordering::SeqCst) + 1;
        let files = state.session.lock().unwrap().files.clone();
        let stale = || {
            drop(state.busy_disk.lock());
            state.identical_run.load(Ordering::SeqCst) != run
        };
        let mut hashes = state.hashes.lock().unwrap();
        let mut last = Instant::now();
        let groups = renome_core::find_identical(
            &files,
            &mut hashes,
            &mut |done, total| {
                if last.elapsed() > Duration::from_millis(150) || done == total {
                    last = Instant::now();
                    let _ = app.emit("identical-progress", IdenticalProgress { done, total });
                }
            },
            &stale,
        )?;
        if stale() {
            return None;
        }
        drop(hashes);
        let by_group = |groups: &[Vec<PathBuf>]| -> HashMap<PathBuf, usize> {
            groups
                .iter()
                .enumerate()
                .flat_map(|(i, g)| g.iter().map(move |p| (p.clone(), i)))
                .collect()
        };
        let identical = by_group(&groups);
        // The songs' tags are read on the copy, without holding the list.
        let songs = renome_core::alike::same_songs(&files, &identical, &stale)?;
        if stale() {
            return None;
        }
        let mut s = state.session.lock().unwrap();
        s.identical = identical;
        s.alike = by_group(&songs);
        let read: HashMap<&PathBuf, &FileEntry> = files.iter().map(|f| (&f.path, f)).collect();
        for f in s.files.iter_mut() {
            if let Some(other) = read.get(&f.path) {
                f.keep_read(other);
            }
        }
        Some(Copies {
            identical: groups,
            songs,
        })
    })
    .await
    .map_err(|e| e.to_string())
}

/// Searches the list for pictures that look alike, in the background,
/// after [`find_identical`] (a newer search for copies stops it). Returns
/// the groups, or `None` if it was stopped. Emits `similar-progress`
/// while reading pictures not seen before.
#[tauri::command]
async fn find_similar(app: AppHandle) -> Result<Option<Vec<Vec<PathBuf>>>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let run = state.identical_run.load(Ordering::SeqCst);
        // Asked before each picture: waits while a preview reads.
        let stale = || {
            drop(state.busy_disk.lock());
            state.identical_run.load(Ordering::SeqCst) != run
        };
        let (files, identical) = {
            let s = state.session.lock().unwrap();
            (s.files.clone(), s.identical.clone())
        };
        let mut cache = state.pictures.lock().unwrap();
        let mut last = Instant::now();
        let groups = renome_core::similar::similar_pictures(
            &files,
            &identical,
            &mut cache,
            &mut |done, total| {
                if last.elapsed() > Duration::from_millis(150) || done == total {
                    last = Instant::now();
                    let _ = app.emit("similar-progress", IdenticalProgress { done, total });
                }
            },
            &stale,
        );
        cache.save();
        drop(cache);
        let groups = groups.filter(|_| !stale())?;
        state.session.lock().unwrap().similar = groups
            .iter()
            .enumerate()
            .flat_map(|(i, g)| g.iter().map(move |p| (p.clone(), i)))
            .collect();
        Some(groups)
    })
    .await
    .map_err(|e| e.to_string())
}

/// A picture (or any file) as the comparison window shows it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PictureCard {
    path: PathBuf,
    size: u64,
    /// Unix seconds.
    modified: Option<i64>,
    /// When the photo was taken (EXIF), `YYYY-MM-DDTHH:MM:SS`.
    taken: Option<String>,
    /// Width and height as shown.
    width: Option<u32>,
    height: Option<u32>,
    /// A smaller copy, as a `data:` URL; `None` for what is not a picture
    /// or cannot be read.
    image: Option<String>,
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The files of a group as the comparison window shows them, pictures
/// with a copy whose longest side is at most `max` pixels.
#[tauri::command]
async fn picture_cards(paths: Vec<PathBuf>, max: u32) -> Vec<PictureCard> {
    tauri::async_runtime::spawn_blocking(move || {
        let card = |path: &PathBuf| {
            let meta = std::fs::symlink_metadata(path).ok();
            let entry = meta.as_ref().and_then(|m| FileEntry::from_path(path, m));
            let ext = entry
                .as_ref()
                .map(|e| e.ext.to_lowercase())
                .unwrap_or_default();
            let shown = (renome_core::similar::is_picture(&ext)
                && entry.as_ref().is_some_and(|e| !e.is_dir))
            .then(|| std::fs::read(path).ok())
            .flatten()
            .and_then(|b| renome_core::similar::shown_copy(&b, &ext, max.clamp(64, 4096)));
            let dims = shown.as_ref().and_then(|(_, d)| *d);
            PictureCard {
                path: path.clone(),
                size: entry.as_ref().map_or(0, |e| e.size),
                modified: entry.as_ref().and_then(|e| e.modified),
                taken: entry
                    .as_ref()
                    .filter(|_| renome_core::similar::is_picture(&ext))
                    .and_then(|e| e.taken())
                    .map(|t| t.format("%Y-%m-%dT%H:%M:%S").to_string()),
                width: dims.map(|d| d.0),
                height: dims.map(|d| d.1),
                image: shown.map(|(jpeg, _)| format!("data:image/jpeg;base64,{}", base64(&jpeg))),
            }
        };
        std::thread::scope(|scope| {
            let jobs: Vec<_> = paths
                .chunks(paths.len().div_ceil(4).max(1))
                .map(|part| scope.spawn(move || part.iter().map(card).collect::<Vec<_>>()))
                .collect();
            jobs.into_iter()
                .flat_map(|j| j.join().unwrap_or_default())
                .collect()
        })
    })
    .await
    .unwrap_or_default()
}

/// A comic's or book's own cover: a smaller copy to show and its
/// fingerprint (hex, to compare with the catalogues' covers).
#[derive(Serialize)]
struct OwnCover {
    image: String,
    hash: String,
}

/// The cover of a comic or book file (see [`renome_core::convert::cover`]).
#[tauri::command]
async fn own_cover(path: PathBuf) -> Option<OwnCover> {
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = renome_core::convert::cover(&path)?;
        let hash = renome_core::similar::fingerprint(&bytes, "img")?;
        let (jpeg, _) = renome_core::similar::shown_copy(&bytes, "img", 600)?;
        Some(OwnCover {
            image: format!("data:image/jpeg;base64,{}", base64(&jpeg)),
            hash: format!("{hash:016x}"),
        })
    })
    .await
    .ok()
    .flatten()
}

/// For catalogue covers, fetched by the program (some catalogues refuse
/// their pictures to a page that is not theirs).
static HTTP: std::sync::LazyLock<reqwest::Client> = std::sync::LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(concat!(
            "Renome/",
            env!("CARGO_PKG_VERSION"),
            " ( https://github.com/rafaellgc/Renome )"
        ))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_default()
});

/// A catalogue's cover that the page could not show by its address, as
/// a data address; `None` when it cannot be read either.
#[tauri::command]
async fn remote_image(url: String) -> Option<String> {
    if !url.starts_with("https://") {
        return None;
    }
    let res = HTTP.get(&url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    let kind = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("image/"))?
        .to_string();
    let bytes = res.bytes().await.ok()?;
    (bytes.len() <= 4 << 20).then(|| format!("data:{kind};base64,{}", base64(&bytes)))
}

/// How far each cover (by address) is from the fingerprint `hash`, in
/// bits; `None` for one that could not be read. Covers are fetched four at
/// a time and remembered for the session.
#[tauri::command]
async fn cover_distances(
    state: State<'_, AppState>,
    hash: String,
    urls: Vec<String>,
) -> Result<Vec<Option<u32>>, String> {
    use futures_util::StreamExt;
    let own = u64::from_str_radix(&hash, 16).map_err(|e| e.to_string())?;
    let missing: Vec<String> = {
        let known = state.covers.lock().await;
        let mut m: Vec<String> = urls
            .iter()
            .filter(|u| u.starts_with("http") && !known.contains_key(*u))
            .cloned()
            .collect();
        m.sort();
        m.dedup();
        m
    };
    let found: Vec<(String, Option<u64>)> = futures_util::stream::iter(missing)
        .map(|url| async move {
            let bytes = match HTTP.get(&url).send().await {
                Ok(r) if r.status().is_success() => r.bytes().await.ok(),
                _ => None,
            };
            let hash = match bytes {
                Some(b) => tauri::async_runtime::spawn_blocking(move || {
                    renome_core::similar::fingerprint(&b, "img")
                })
                .await
                .ok()
                .flatten(),
                None => None,
            };
            (url, hash)
        })
        .buffer_unordered(4)
        .collect()
        .await;
    let mut known = state.covers.lock().await;
    known.extend(found);
    Ok(urls
        .iter()
        .map(|u| {
            known
                .get(u)
                .copied()
                .flatten()
                .map(|h| renome_core::similar::distance(own, h))
        })
        .collect())
}

#[derive(Serialize)]
struct Trashed {
    trashed: Vec<PathBuf>,
    /// Path and why.
    failed: Vec<(PathBuf, String)>,
}

/// Sends files to the system's trash and takes them out of the list (and
/// of the earlier states Ctrl+Z goes back to).
#[tauri::command]
async fn trash_files(app: AppHandle, paths: Vec<PathBuf>) -> Result<Trashed, String> {
    in_background(app, move |state| {
        let mut done = Trashed {
            trashed: Vec::new(),
            failed: Vec::new(),
        };
        for p in paths {
            match renome_core::files::to_trash(&p) {
                Ok(()) => done.trashed.push(p),
                Err(e) => done.failed.push((p, e)),
            }
        }
        let gone: HashSet<&PathBuf> = done.trashed.iter().collect();
        let mut s = state.session.lock().unwrap();
        s.files.retain(|f| !gone.contains(&f.path));
        for (files, _) in s.undo.iter_mut() {
            files.retain(|f| !gone.contains(&f.path));
        }
        s.identical.retain(|p, _| !gone.contains(p));
        s.alike.retain(|p, _| !gone.contains(p));
        s.similar.retain(|p, _| !gone.contains(p));
        drop(s);
        done
    })
    .await
}

/// Renames as they happened, in order: plain renames are looked up at
/// once; a renamed folder also moves whatever was under it.
enum Renamed<'a> {
    Exact(HashMap<&'a Path, &'a Path>),
    Folder(&'a Operation),
}

/// Updates the session entries after renaming. Renames run in order, and a
/// renamed folder also moves the entries inside it.
fn refresh_renamed(files: &mut [FileEntry], report: &ApplyReport) {
    if report.done.is_empty() {
        return;
    }
    let mut renames: Vec<Renamed> = Vec::new();
    for op in &report.done {
        if op.to.is_dir() {
            renames.push(Renamed::Folder(op));
        } else {
            if !matches!(renames.last(), Some(Renamed::Exact(_))) {
                renames.push(Renamed::Exact(HashMap::new()));
            }
            if let Some(Renamed::Exact(map)) = renames.last_mut() {
                map.insert(&op.from, &op.to);
            }
        }
    }
    for f in files.iter_mut() {
        let mut path = f.path.clone();
        for step in &renames {
            match step {
                Renamed::Exact(map) => {
                    if let Some(to) = map.get(path.as_path()) {
                        path = to.to_path_buf();
                    }
                }
                Renamed::Folder(op) => {
                    if path == op.from {
                        path = op.to.clone();
                    } else if let Ok(rest) = path.strip_prefix(&op.from) {
                        path = op.to.join(rest);
                    }
                }
            }
        }
        if path != f.path
            && let Ok(meta) = std::fs::symlink_metadata(&path)
            && let Some(e) = FileEntry::from_path(&path, &meta)
        {
            *f = e;
        }
    }
}

#[tauri::command]
async fn apply(
    app: AppHandle,
    steps: Vec<Step>,
    overrides: Overrides,
    mut view: View,
) -> Result<ApplyReport, String> {
    let stop = stop_signal(&app);
    let emitter = app.clone();
    in_background(app, move |state| {
        let _disk = state.busy_disk.lock().unwrap();
        let mut s = state.session.lock().unwrap();
        view.identical = s.identical.clone();
        view.alike = s.alike.clone();
        view.similar = s.similar.clone();
        view.protected = protected_dirs();
        // Recompute the preview here to apply exactly what is visible and valid right now.
        let plan = renome_core::preview(&s.files, &steps, &overrides, &view);
        let ops = plan.operations();
        // The photo rule's "set the file date": each photo's date, by path.
        let touch: HashMap<PathBuf, i64> = plan
            .items
            .iter()
            .filter_map(|i| Some((i.path.clone(), i.touch?)))
            .collect();
        // Copies (and moves to other disks) tell how far they are.
        let last = std::cell::Cell::new(std::time::Instant::now());
        let progress = |done: u64, total: u64| {
            if last.get().elapsed().as_millis() >= 200 || done == total {
                last.set(std::time::Instant::now());
                let _ = emitter.emit(
                    "apply-progress",
                    serde_json::json!({ "done": done, "total": total }),
                );
            }
        };
        let hooks = renome_core::apply::Hooks {
            progress: &progress,
            stop: &stop,
        };
        let report = state.history.apply_with(&ops, &hooks);
        if let Some(id) = &report.history_id {
            let times: Vec<(PathBuf, i64)> = report
                .done
                .iter()
                .filter_map(|op| Some((op.to.clone(), *touch.get(&op.from)?)))
                .collect();
            if !times.is_empty() {
                let _ = state.history.set_times(id, &times);
            }
        }
        refresh_renamed(&mut s.files, &report);
        // Earlier states point to the old names.
        s.undo.clear();
        report
    })
    .await
}

/// Where TMDB images are shown from, without the size (`…/t/p`): tests
/// point it elsewhere with `RENOME_TMDB_IMAGES`.
#[tauri::command]
fn tmdb_images() -> String {
    std::env::var("RENOME_TMDB_IMAGES")
        .ok()
        .and_then(|b| b.strip_suffix("/original").map(str::to_string))
        .unwrap_or_else(|| "https://image.tmdb.org/t/p".into())
}

#[tauri::command]
async fn tmdb_check(key: String) -> Result<(), String> {
    tmdb::Client::new(&key, "en-US").check().await
}

/// Guesses title and year for each file and searches TMDB for them.
#[tauri::command]
async fn tmdb_lookup(
    app: AppHandle,
    key: String,
    lang: String,
    items: Vec<tmdb::LookupItem>,
) -> Result<Vec<tmdb::Lookup>, String> {
    let client = tmdb::Client::new(&key, &lang);
    Ok(tmdb::lookup(&app, &client, items).await)
}

/// The collections the chosen movies belong to, for those missing.
#[tauri::command]
async fn tmdb_collections(key: String, lang: String, ids: Vec<u64>, sets: bool) -> tmdb::MovieSets {
    tmdb::collections(&tmdb::Client::new(&key, &lang), ids, sets).await
}

/// A search typed by the user in the review window.
#[tauri::command]
async fn tmdb_search(
    key: String,
    lang: String,
    title: String,
    year: Option<u16>,
) -> Result<Vec<renome_core::tmdb::Candidate>, String> {
    let client = tmdb::Client::new(&key, &lang);
    // A link or id pasted: that movie.
    let id = match links::parse(&title) {
        Some(Link::Tmdb { tv: true, .. }) => return Err("linkOther".into()),
        Some(Link::Tmdb { id, .. }) => Some(id),
        Some(Link::Imdb(tt)) => Some(client.find_imdb(&tt, false).await.ok_or("linkNotFound")?),
        Some(_) => return Err("linkOther".into()),
        None => None,
    };
    let guess = renome_core::tmdb::Guess { title, year, id };
    Ok(client.search(&guess).await?.0)
}

/// Finds a show by what the user typed (a name, a TMDB id or link), or by
/// the files' names when nothing was typed.
#[tauri::command]
async fn tv_search(
    key: String,
    lang: String,
    query: String,
    items: Vec<tmdb::TvItem>,
) -> Result<tmdb::ShowSearch, String> {
    let client = tmdb::Client::new(&key, &lang);
    // An IMDb id pasted: the show it names.
    let query = match links::parse(&query) {
        Some(Link::Imdb(tt)) => client
            .find_imdb(&tt, true)
            .await
            .ok_or("linkNotFound")?
            .to_string(),
        Some(Link::Tmdb { tv: true, .. }) | None => query,
        Some(_) if query.contains("tmdbid") => query,
        Some(_) => return Err("linkOther".into()),
    };
    tmdb::search_show(&client, &query, &items).await
}

/// Fetches a show's episodes and finds each file's episode.
#[tauri::command]
async fn tv_lookup(
    key: String,
    lang: String,
    id: u64,
    items: Vec<tmdb::TvItem>,
    typed: String,
) -> Result<tmdb::TvLookup, String> {
    tmdb::lookup_episodes(&tmdb::Client::new(&key, &lang), id, items, &typed).await
}

/// Like `tv_search`, on AniList (anime, no key).
#[tauri::command]
async fn anime_search(
    app: AppHandle,
    query: String,
    items: Vec<tmdb::TvItem>,
) -> Result<tmdb::ShowSearch, String> {
    app.state::<AppState>()
        .al
        .anime_search(&query, &items)
        .await
}

/// Like `tv_lookup`, on AniList: the show from its first season and the
/// seasons after it.
#[tauri::command]
async fn anime_lookup(
    app: AppHandle,
    id: u64,
    items: Vec<tmdb::TvItem>,
    typed: String,
) -> Result<tmdb::TvLookup, String> {
    app.state::<AppState>()
        .al
        .anime_lookup(id, &items, &typed)
        .await
}

/// Groups the files by show, to look each one up.
#[tauri::command]
fn tv_groups(items: Vec<tmdb::TvItem>) -> tmdb::ShowGroups {
    tmdb::group_shows(&items)
}

/// Records created metadata files in the rename's history entry, so undo
/// deletes them; without a rename (names already right), in an entry of
/// their own.
fn record_created(app: &AppHandle, history_id: Option<String>, report: &mut tmdb::SidecarReport) {
    if report.created.is_empty() {
        return;
    }
    let history = &app.state::<AppState>().history;
    let saved = match history_id {
        Some(id) => history.add_created(&id, &report.created),
        None => history.record_files(&report.created).map(|_| ()),
    };
    if let Err(e) = saved {
        report
            .errors
            .push((String::new(), format!("err.history: {e}")));
    }
}

/// Writes `.nfo` and images next to renamed movies and records them in the
/// rename's history entry, so undo deletes them.
#[tauri::command]
async fn write_sidecars(
    app: AppHandle,
    key: String,
    lang: String,
    history_id: Option<String>,
    items: Vec<tmdb::SidecarItem>,
    fanart: bool,
    in_folder: bool,
) -> Result<tmdb::SidecarReport, String> {
    let client = tmdb::Client::new(&key, &lang);
    let mut report = tmdb::sidecars(&app, &client, items, fanart, in_folder).await;
    record_created(&app, history_id, &mut report);
    Ok(report)
}

/// Writes `.nfo` files and images for renamed episodes and their show, and
/// records them like `write_sidecars`.
#[tauri::command]
async fn write_tv_sidecars(
    app: AppHandle,
    key: String,
    lang: String,
    history_id: Option<String>,
    items: Vec<tmdb::TvSidecarItem>,
    fanart: bool,
    folders: bool,
) -> Result<tmdb::SidecarReport, String> {
    let client = tmdb::Client::new(&key, &lang);
    let mut report = tmdb::tv_sidecars(&app, &client, items, fanart, folders).await;
    record_created(&app, history_id, &mut report);
    Ok(report)
}

/// Searches subtitles for the videos, in the languages not there yet.
#[tauri::command]
async fn subs_search(
    app: AppHandle,
    auth: subtitles::Auth,
    langs: String,
    items: Vec<subtitles::Item>,
) -> subtitles::Search {
    let stop = stop_signal(&app);
    subtitles::search(&app, auth, &langs, items, stop).await
}

/// Downloads the subtitles chosen beside the renamed videos and records
/// them like `write_sidecars`.
#[tauri::command]
async fn write_subtitles(
    app: AppHandle,
    auth: subtitles::Auth,
    history_id: Option<String>,
    items: Vec<subtitles::Write>,
) -> subtitles::Written {
    let stop = stop_signal(&app);
    let mut out = subtitles::write(&app, auth, items, stop).await;
    let mut report = tmdb::SidecarReport {
        created: std::mem::take(&mut out.created),
        errors: std::mem::take(&mut out.errors),
        kept: 0,
    };
    record_created(&app, history_id, &mut report);
    out.created = report.created;
    out.errors = report.errors;
    out
}

#[tauri::command]
async fn os_check(auth: subtitles::Auth) -> Result<Option<i64>, String> {
    subtitles::check_os(auth).await
}

#[tauri::command]
async fn subdl_check(key: String) -> Result<(), String> {
    subtitles::check_subdl(key).await
}

type Songs = Vec<(PathBuf, AudioTags)>;

/// The tags of the given songs in the list (with the name filled in, as
/// the music rule does, and the recordings they were heard as): the songs
/// with a title in their tags, and those without (a title from the file
/// name is only a guess).
fn songs_of(app: &AppHandle, paths: &[PathBuf], from_name: bool) -> (Songs, Songs) {
    let state = app.state::<AppState>();
    let wanted: HashSet<&PathBuf> = paths.iter().collect();
    let files: Vec<FileEntry> = state
        .session
        .lock()
        .unwrap()
        .files
        .iter()
        .filter(|f| wanted.contains(&f.path))
        .cloned()
        .collect();
    let heard = state.heard.lock().unwrap();
    let (mut named, mut unnamed) = (Vec::new(), Vec::new());
    for f in &files {
        let Some(mut t) = song_tags(f, from_name).or_else(|| any_song_tags(f)) else {
            continue;
        };
        t.recordings = heard.get(&f.path).cloned().unwrap_or_default();
        if f.audio_tags().is_some_and(|t| !t.title.is_empty()) {
            named.push((f.path.clone(), t));
        } else {
            unnamed.push((f.path.clone(), t));
        }
    }
    (named, unnamed)
}

/// Groups the songs in albums and looks each one up on MusicBrainz.
#[tauri::command]
async fn mb_lookup(
    app: AppHandle,
    paths: Vec<PathBuf>,
    from_name: bool,
    listen: bool,
) -> Result<Vec<musicbrainz::AlbumLookup>, String> {
    let (mut named, mut unnamed) = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || songs_of(&app, &paths, from_name)
    })
    .await
    .map_err(|e| e.to_string())?;
    if !listen {
        // Without listening, a title from the file name is all there is.
        let (guessed, none): (Vec<_>, Vec<_>) =
            unnamed.into_iter().partition(|(_, t)| !t.title.is_empty());
        named.extend(guessed);
        unnamed = none;
    }
    let albums = musicbrainz::albums(&named);
    let state = app.state::<AppState>();
    let services = musicbrainz::Services {
        mb: &state.mb,
        dg: &state.dg,
        acoustid: &state.acoustid,
        heard: &state.heard,
    };
    Ok(musicbrainz::lookup(&app, &services, albums, unnamed, listen).await)
}

/// A search typed in the album review, on MusicBrainz or Discogs.
#[tauri::command]
async fn mb_search(
    app: AppHandle,
    query: renome_core::musicbrainz::AlbumQuery,
    discogs: Option<bool>,
) -> Result<Vec<renome_core::musicbrainz::Candidate>, String> {
    let state = app.state::<AppState>();
    // A link pasted: that release.
    match links::parse(&query.album) {
        Some(Link::MusicBrainz(id)) => {
            let r = state.mb.release(&id).await?;
            return Ok(vec![renome_core::musicbrainz::summary(&r, 1.0)]);
        }
        Some(Link::Discogs(id)) => {
            let id = format!("{}{id}", renome_core::discogs::PREFIX);
            let r = state.dg.release(&id).await?;
            return Ok(vec![renome_core::musicbrainz::summary(&r, 1.0)]);
        }
        Some(_) => return Err("linkOther".into()),
        None => {}
    }
    if discogs == Some(true) {
        state.dg.search(&query).await
    } else {
        state.mb.search(&query).await
    }
}

/// The Discogs token from the options (empty: Discogs is not used).
#[tauri::command]
fn discogs_token(state: State<AppState>, token: String) {
    state.dg.set_token(&token);
}

/// The Google Books key from the options (empty: Google Books is not used).
#[tauri::command]
fn google_books_key(state: State<AppState>, key: String) {
    state.ol.gb.set_key(&key);
}

/// Checks a Google Books key.
#[tauri::command]
async fn google_books_check(app: AppHandle, key: String) -> Result<(), String> {
    app.state::<AppState>().ol.gb.check(&key).await
}

/// Checks a Discogs token.
#[tauri::command]
async fn discogs_check(app: AppHandle, token: String) -> Result<(), String> {
    app.state::<AppState>().dg.check(&token).await
}

/// Matches an album's songs to the release the user chose.
#[tauri::command]
async fn mb_choose(
    app: AppHandle,
    id: String,
    paths: Vec<PathBuf>,
    from_name: bool,
) -> Result<musicbrainz::Chosen, String> {
    // Songs without a title in their tags match by the recording heard, or
    // by what their file name tells.
    let (mut songs, unnamed) = songs_of(&app, &paths, from_name);
    songs = musicbrainz::albums(&songs).concat();
    songs.extend(unnamed);
    let state = app.state::<AppState>();
    musicbrainz::choose(&state.mb, &state.dg, &id, &songs).await
}

/// Saves the albums' covers and records them like `write_sidecars`.
#[tauri::command]
async fn write_covers(
    app: AppHandle,
    history_id: Option<String>,
    items: Vec<musicbrainz::CoverItem>,
) -> Result<tmdb::SidecarReport, String> {
    let state = app.state::<AppState>();
    let mut report = musicbrainz::covers(&app, &state.mb, &state.dg, items).await;
    record_created(&app, history_id, &mut report);
    Ok(report)
}

#[derive(serde::Deserialize)]
struct TagJob {
    /// Where the song is now (after renaming).
    path: PathBuf,
    song: renome_core::musicbrainz::SongMatch,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct TagReport {
    written: usize,
    /// PDF pages drawn while converting comics (see `write_inside`).
    drawn: usize,
    /// File name and reason.
    errors: Vec<(String, String)>,
}

/// Writes each song's release details into its tags (and, with `cover`,
/// the release's front cover in place of its pictures; with `clean`, the
/// other tags go), keeping what they held in the rename's history entry (or
/// one of its own) for undo. Each release is looked up in full once, for
/// what its songs' tags take beyond the match (labels, genres, credits).
#[tauri::command]
async fn write_tags(
    app: AppHandle,
    history_id: Option<String>,
    items: Vec<TagJob>,
    cover: bool,
    clean: bool,
    cover_size: Option<u32>,
) -> Result<TagReport, String> {
    use renome_core::tagwrite;
    let state = app.state::<AppState>();
    let mut report = TagReport::default();
    let items: Vec<TagJob> = items
        .into_iter()
        .filter(|j| {
            let ok = tagwrite::writable(&j.path);
            if !ok {
                report
                    .errors
                    .push((j.path.display().to_string(), "tagsUnsupported".into()));
            }
            ok
        })
        .collect();
    if items.is_empty() {
        return Ok(report);
    }
    let own_entry = history_id.is_none();
    let id = match history_id {
        Some(id) => id,
        None => state.history.record_files(&[])?,
    };
    let store = state.history.files_dir(&id)?;
    // Each song noted before and after, for a stop halfway.
    let mut note = state.history.tagging(&id)?;
    // One cover per release: the archive's, else the album folder's own.
    let mut covers: HashMap<String, Option<Vec<u8>>> = HashMap::new();
    let mut full: HashMap<String, Option<renome_core::musicbrainz::Release>> = HashMap::new();
    let total = items.len();
    let _ = app.emit(
        "mb-progress",
        serde_json::json!({ "done": 0, "total": total, "kind": "tags" }),
    );
    let mut backups = Vec::new();
    let mut written = Vec::new();
    for (done, job) in items.into_iter().enumerate() {
        // Typed in: no release to ask (the folder's cover still counts).
        let typed = job.song.typed || job.song.release.is_empty();
        let image = if cover && typed {
            job.path.parent().and_then(folder_cover)
        } else if cover {
            if !covers.contains_key(&job.song.release) {
                let mut found = musicbrainz::front_cover(
                    &state.mb,
                    &state.dg,
                    &job.song.release,
                    cover_size.unwrap_or(1200),
                )
                .await
                .ok()
                .flatten();
                if found.is_none() {
                    found = job.path.parent().and_then(folder_cover);
                }
                covers.insert(job.song.release.clone(), found);
            }
            covers[&job.song.release].clone()
        } else {
            None
        };
        if !typed && !full.contains_key(&job.song.release) {
            let r = if renome_core::discogs::is_discogs(&job.song.release) {
                state.dg.release(&job.song.release).await.ok()
            } else {
                state.mb.release_full(&job.song.release).await.ok()
            };
            full.insert(job.song.release.clone(), r);
        }
        let mut fields = tagwrite::fields_of(&job.song);
        if let Some(Some(r)) = full.get(&job.song.release).filter(|_| !typed) {
            // A genre typed in counts over the release's.
            let typed_genre = !job.song.genre.is_empty();
            fields.extend(
                tagwrite::details(r, &job.song.track_id)
                    .into_iter()
                    .filter(|(f, _)| !(typed_genre && *f == tagwrite::Field::Genre)),
            );
        }
        let (path, store) = (job.path.clone(), store.clone());
        note.start(&path);
        let result = tauri::async_runtime::spawn_blocking(move || {
            tagwrite::write(&path, &fields, image.as_deref(), clean, &store)
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);
        match result {
            Ok(Some(b)) => {
                note.done(&b);
                backups.push(b);
                written.push(job.path);
            }
            Ok(None) => {}
            Err(e) => report.errors.push((job.path.display().to_string(), e)),
        }
        let _ = app.emit(
            "mb-progress",
            serde_json::json!({ "done": done + 1, "total": total, "kind": "tags" }),
        );
    }
    report.written = backups.len();
    let saved = if backups.is_empty() {
        if own_entry {
            let _ = state.history.delete(&id);
        }
        Ok(())
    } else {
        state.history.add_tags(&id, backups)
    };
    match saved {
        Ok(()) => note.finish(),
        // The note stays: the next start puts the songs in the entry.
        Err(e) => report
            .errors
            .push((String::new(), format!("err.history: {e}"))),
    }
    // The list reads the new tags.
    let written: HashSet<PathBuf> = written.into_iter().collect();
    for f in state.session.lock().unwrap().files.iter_mut() {
        if written.contains(&f.path) {
            f.forget_tags();
        }
    }
    Ok(report)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct InsideJob {
    /// Where the file is now (after renaming).
    path: PathBuf,
    #[serde(default)]
    comic: Option<renome_core::comics::ComicMatch>,
    /// The issue's page in the catalogue.
    #[serde(default)]
    web: String,
    #[serde(default)]
    manga: bool,
    #[serde(default)]
    book: Option<renome_core::books::BookMatch>,
    /// The book's language as Open Library gives it.
    #[serde(default)]
    language: String,
    /// The comic's `ComicInfo.xml` is cleared: written with only `comic`'s
    /// details, or (without `comic`) taken out.
    #[serde(default)]
    fresh: bool,
    /// Converts the comic to CBZ first (with `comic`'s details in it, when
    /// given), its original kept beside or sent to the trash.
    #[serde(default)]
    convert: Option<renome_core::convert::Original>,
    /// How PDFs are converted (whether pages may be drawn, at what size,
    /// whether their own title and author go in).
    #[serde(default)]
    pdf: renome_core::convert::PdfOptions,
    /// Converts the book to this format (with `book`'s details in it, when
    /// given), its original kept beside or sent to the trash (`convert`).
    #[serde(default)]
    book_to: Option<renome_core::bookconv::Target>,
}

/// Makes the book of `job` (renamed `.pdf`) into a PDF: its page is
/// written, printed by the system's web view, and takes the book's place.
async fn print_book(
    app: &AppHandle,
    job: InsideJob,
) -> Result<(Option<renome_core::tagwrite::Backup>, Option<String>), String> {
    use renome_core::bookconv::print;
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let work = std::env::temp_dir().join(format!("renome-print-{}-{n}", std::process::id()));
    let path = job.path.clone();
    let values = job
        .book
        .as_ref()
        .map(|b| renome_core::inside::book_values(b, &job.language));
    let original = job.convert.unwrap_or(renome_core::convert::Original::Keep);
    let prep = {
        let (path, work) = (path.clone(), work.clone());
        tauri::async_runtime::spawn_blocking(move || print::prepare(&path, values.as_ref(), &work))
            .await
            .map_err(|e| e.to_string())??
    };
    if let Err(e) = print_to_pdf(app, &prep.html, &prep.pdf).await {
        let _ = std::fs::remove_dir_all(&prep.dir);
        let _ = std::fs::remove_file(&prep.pdf);
        return Err(e);
    }
    let done = tauri::async_runtime::spawn_blocking(move || print::finish(&path, &prep, original))
        .await
        .map_err(|e| e.to_string())??;
    Ok((Some(done.backup), done.not_trashed))
}

/// Whether this system's web view can print books to PDF.
#[tauri::command]
fn pdf_print_available() -> bool {
    cfg!(target_os = "linux")
}

/// Prints the page `html` to the PDF file `pdf` with a hidden web view.
async fn print_to_pdf(app: &AppHandle, html: &Path, pdf: &Path) -> Result<(), String> {
    use tauri::webview::PageLoadEvent;
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let url = tauri::Url::from_file_path(html).map_err(|_| "pdfPrint".to_string())?;
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let tx = std::sync::Mutex::new(Some(tx));
    let label = format!(
        "print{}",
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .visible(false)
        .on_page_load(move |_, p| {
            if p.event() == PageLoadEvent::Finished
                && let Some(t) = tx.lock().unwrap().take()
            {
                let _ = t.send(());
            }
        })
        .build()
        .map_err(|e| e.to_string())?;
    let loaded = tokio::time::timeout(std::time::Duration::from_secs(120), rx).await;
    if !matches!(loaded, Ok(Ok(()))) {
        let _ = win.destroy();
        return Err("pdfPrint".into());
    }
    let (dtx, drx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let dtx = std::sync::Arc::new(std::sync::Mutex::new(Some(dtx)));
    let out = pdf.to_path_buf();
    let started = print_webview(&win, out, dtx.clone());
    if let Err(e) = started {
        let _ = win.destroy();
        return Err(e);
    }
    let r = tokio::time::timeout(std::time::Duration::from_secs(600), drx).await;
    let _ = win.destroy();
    match r {
        Ok(Ok(r)) => r,
        _ => Err("pdfPrint".into()),
    }
}

type PrintDone =
    std::sync::Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<Result<(), String>>>>>;

#[cfg(target_os = "linux")]
fn print_webview(win: &tauri::WebviewWindow, out: PathBuf, done: PrintDone) -> Result<(), String> {
    win.with_webview(move |pw| {
        use webkit2gtk::PrintOperationExt;
        let send = move |r: Result<(), String>| {
            if let Some(t) = done.lock().unwrap().take() {
                let _ = t.send(r);
            }
        };
        let Ok(uri) = tauri::Url::from_file_path(&out) else {
            send(Err("pdfPrint".into()));
            return;
        };
        let op = webkit2gtk::PrintOperation::new(&pw.inner());
        let settings = gtk::PrintSettings::new();
        // The printer that writes files has a name in the desktop's
        // language: GTK's own translation of it.
        let name = gtk::glib::dgettext(Some("gtk30"), "Print to File");
        settings.set_printer(name.as_str());
        settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT, Some("pdf"));
        settings.set(gtk::PRINT_SETTINGS_OUTPUT_URI, Some(uri.as_str()));
        op.set_print_settings(&settings);
        let setup = gtk::PageSetup::new();
        setup.set_paper_size(&gtk::PaperSize::new(Some("iso_a5")));
        for (side, mm) in [
            ("top", 16.0),
            ("bottom", 18.0),
            ("left", 14.0),
            ("right", 14.0),
        ] {
            match side {
                "top" => setup.set_top_margin(mm, gtk::Unit::Mm),
                "bottom" => setup.set_bottom_margin(mm, gtk::Unit::Mm),
                "left" => setup.set_left_margin(mm, gtk::Unit::Mm),
                _ => setup.set_right_margin(mm, gtk::Unit::Mm),
            }
        }
        op.set_page_setup(&setup);
        let send = std::rc::Rc::new(std::cell::RefCell::new(Some(send)));
        let s1 = send.clone();
        // The operation is kept until it ends.
        let keep = op.clone();
        op.connect_finished(move |_| {
            let _ = &keep;
            if let Some(f) = s1.borrow_mut().take() {
                f(Ok(()));
            }
        });
        let s2 = send.clone();
        op.connect_failed(move |_, e| {
            if let Some(f) = s2.borrow_mut().take() {
                f(Err(format!("pdfPrint: {e}")));
            }
        });
        op.print();
    })
    .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "linux"))]
fn print_webview(_: &tauri::WebviewWindow, _: PathBuf, _: PrintDone) -> Result<(), String> {
    Err("pdfPrint".into())
}

/// Writes each comic's details into its `ComicInfo.xml` (CBZ) and each
/// book's into its OPF (EPUB), keeping what they held in the rename's
/// history entry (or one of its own) for undo, like `write_tags`. Comics
/// to convert (CBR, CB7, CBT renamed `.cbz`) are converted, the details
/// written in the same pass.
#[tauri::command]
async fn write_inside(
    app: AppHandle,
    history_id: Option<String>,
    items: Vec<InsideJob>,
) -> Result<TagReport, String> {
    use renome_core::{convert, inside};
    let state = app.state::<AppState>();
    let mut report = TagReport::default();
    let items: Vec<InsideJob> = items
        .into_iter()
        .map(|mut j| {
            // Already a zip (or not an archive): only its details.
            if j.book_to.is_none() && j.convert.is_some() && !convert::convertible(&j.path) {
                j.convert = None;
            }
            j
        })
        .filter(|j| {
            let ok = match (&j.convert, &j.comic, &j.book) {
                _ if j.book_to.is_some() => true,
                (Some(_), _, _) => true,
                (_, Some(_), _) => inside::comic_writable(&j.path),
                (_, _, None) if j.fresh => inside::comic_writable(&j.path),
                (_, _, Some(_)) => inside::book_writable(&j.path),
                _ => false,
            };
            if !ok && (j.comic.is_some() || j.book.is_some() || j.fresh) {
                let why = if j.comic.is_some() {
                    "comicUnsupported"
                } else {
                    "bookUnsupported"
                };
                report
                    .errors
                    .push((j.path.display().to_string(), why.into()));
            }
            ok
        })
        .collect();
    if items.is_empty() {
        return Ok(report);
    }
    let own_entry = history_id.is_none();
    let id = match history_id {
        Some(id) => id,
        None => state.history.record_files(&[])?,
    };
    let store = state.history.files_dir(&id)?;
    let mut note = state.history.tagging(&id)?;
    let total = items.len();
    let _ = app.emit(
        "inside-progress",
        serde_json::json!({ "done": 0, "total": total }),
    );
    let mut backups = Vec::new();
    let mut written = Vec::new();
    let pages = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for (done, job) in items.into_iter().enumerate() {
        let (path, store) = (job.path.clone(), store.clone());
        let job_path = job.path.clone();
        let converting = job.convert.is_some() && job.book_to.is_none();
        let book_converting = job.book_to.is_some();
        let drawn = pages.clone();
        note.start(&path);
        // A book made into PDF: its page printed by the web view.
        let result = if job.book_to == Some(renome_core::bookconv::Target::Pdf) {
            print_book(&app, job).await
        } else {
            tauri::async_runtime::spawn_blocking(move || {
                let values = || {
                    job.comic
                        .as_ref()
                        .map(|c| inside::comic_values(c, &job.web, job.manga))
                        .unwrap_or_default()
                };
                if let Some(to) = job.book_to {
                    let values = job
                        .book
                        .as_ref()
                        .map(|b| inside::book_values(b, &job.language));
                    let original = job.convert.unwrap_or(convert::Original::Keep);
                    return renome_core::bookconv::convert(&path, to, original, values.as_ref())
                        .map(|d| (Some(d.backup), d.not_trashed));
                }
                match (job.convert, &job.comic, &job.book) {
                    (Some(original), _, _) => {
                        convert::convert_with(&path, &values(), original, job.pdf).map(|d| {
                            drawn.fetch_add(d.drawn_pages, std::sync::atomic::Ordering::Relaxed);
                            (Some(d.backup), d.not_trashed)
                        })
                    }
                    (_, Some(_), _) => {
                        inside::write_comic_with(&path, &values(), job.fresh, &store)
                            .map(|b| (b, None))
                    }
                    (_, _, Some(b)) => {
                        inside::write_book(&path, &inside::book_values(b, &job.language), &store)
                            .map(|b| (b, None))
                    }
                    _ if job.fresh => inside::remove_comic(&path, &store).map(|b| (b, None)),
                    _ => Ok((None, None)),
                }
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r)
        };
        let result = result.map(|(b, not_trashed)| {
            if let Some(why) = not_trashed {
                report.errors.push((job_path.display().to_string(), why));
            }
            b
        });
        match result {
            Ok(Some(b)) => {
                note.done(&b);
                written.push(b.path.clone());
                backups.push(b);
            }
            Ok(None) => {}
            Err(e) => {
                // Named `.cbz` (or `.epub`…) to be converted, it could not
                // be: its own extension back.
                let back = if book_converting {
                    renome_core::bookconv::beside(&job_path).filter(|b| *b != job_path)
                } else if converting {
                    convert::kind(&job_path)
                        .filter(|k| *k != convert::Kind::Zip)
                        .map(|k| convert::beside(&job_path, k))
                } else {
                    None
                };
                if let Some(back) = back
                    && std::fs::symlink_metadata(&back).is_err()
                    && state.history.retarget(&id, &job_path, &back) == Ok(true)
                {
                    {
                        let moved = renome_core::apply::ApplyReport {
                            done: vec![renome_core::apply::Operation {
                                from: job_path.clone(),
                                to: back,
                                copy: false,
                                within: None,
                            }],
                            ..Default::default()
                        };
                        refresh_renamed(&mut state.session.lock().unwrap().files, &moved);
                    }
                }
                report.errors.push((job_path.display().to_string(), e));
            }
        }
        let _ = app.emit(
            "inside-progress",
            serde_json::json!({ "done": done + 1, "total": total }),
        );
    }
    report.written = backups.len();
    report.drawn = pages.load(std::sync::atomic::Ordering::Relaxed);
    let saved = if backups.is_empty() {
        if own_entry {
            let _ = state.history.delete(&id);
        } else {
            // Comics that could not be converted may have taken back the
            // only renames.
            let _ = state.history.drop_if_empty(&id);
        }
        Ok(())
    } else {
        state.history.add_tags(&id, backups)
    };
    match saved {
        Ok(()) => note.finish(),
        Err(e) => report
            .errors
            .push((String::new(), format!("err.history: {e}"))),
    }
    let written: HashSet<PathBuf> = written.into_iter().collect();
    for f in state.session.lock().unwrap().files.iter_mut() {
        if written.contains(&f.path) {
            f.forget_tags();
        }
    }
    Ok(report)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubJob {
    /// Where the file is now (after renaming).
    path: PathBuf,
    /// A subtitle the tools change (encoding, cleaning, times, SubRip).
    #[serde(default)]
    fix: bool,
    /// A video whose text tracks are taken out.
    #[serde(default)]
    extract: bool,
}

#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SubToolsReport {
    /// Subtitles changed.
    written: usize,
    /// Subtitles taken out of videos.
    created: usize,
    /// File name and reason.
    errors: Vec<(String, String)>,
}

/// The subtitle rule's work after the rename: subtitles changed where they
/// are (their old content kept in the history entry for undo) or made
/// into SubRip (their original kept beside or sent to the trash), and the
/// text tracks of videos written beside them (deleted by undo). `stop`
/// ends it between files (and while a video is read).
#[tauri::command]
async fn subtitle_tools(
    app: AppHandle,
    history_id: Option<String>,
    items: Vec<SubJob>,
    tools: renome_core::subtext::Tools,
    original: renome_core::convert::Original,
    extract_langs: String,
) -> Result<SubToolsReport, String> {
    use renome_core::{mkvsubs, subtext};
    let state = app.state::<AppState>();
    let mut report = SubToolsReport::default();
    let items: Vec<SubJob> = items.into_iter().filter(|j| j.fix || j.extract).collect();
    if items.is_empty() {
        return Ok(report);
    }
    let own_entry = history_id.is_none();
    let id = match history_id {
        Some(id) => id,
        None => state.history.record_files(&[])?,
    };
    let store = state.history.files_dir(&id)?;
    let mut note = state.history.tagging(&id)?;
    let langs = std::sync::Arc::new(renome_core::subtitles::langs(&extract_langs));
    let stop = std::sync::Arc::new(stop_signal(&app));
    let total = items.len();
    let progress = |done: usize, part: f64| {
        let _ = app.emit(
            "subtools-progress",
            serde_json::json!({ "done": done, "total": total, "part": part }),
        );
    };
    progress(0, 0.0);
    let mut backups = Vec::new();
    let mut created = Vec::new();
    let mut changed = Vec::new();
    for (done, job) in items.into_iter().enumerate() {
        if stop() {
            break;
        }
        let name = job
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if job.fix {
            note.start(&job.path);
            let (path, store) = (job.path.clone(), store.clone());
            let r = tauri::async_runtime::spawn_blocking(move || {
                subtext::fix_file(&path, &tools, original, &store)
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r);
            match r {
                Ok(f) => {
                    if let Some(why) = f.not_trashed {
                        report.errors.push((name.clone(), why));
                    }
                    if let Some(b) = f.backup {
                        note.done(&b);
                        changed.push(b.path.clone());
                        backups.push(b);
                    }
                }
                Err(e) => report.errors.push((name.clone(), e)),
            }
        }
        if job.extract {
            let (path, langs, stop) = (job.path.clone(), langs.clone(), stop.clone());
            let app2 = app.clone();
            let r = tauri::async_runtime::spawn_blocking(move || {
                let tell = |at: u64, size: u64| {
                    let part = if size > 0 { at as f64 / size as f64 } else { 0.0 };
                    let _ = app2.emit(
                        "subtools-progress",
                        serde_json::json!({ "done": done, "total": total, "part": part }),
                    );
                };
                mkvsubs::extract(&path, &tools, &langs, &*stop, &tell)
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r);
            match r {
                Ok(x) => {
                    created.extend(x.created);
                    report.errors.extend(x.errors);
                }
                Err(e) if e == "stopped" => break,
                Err(e) => report.errors.push((name, e)),
            }
        }
        progress(done + 1, 0.0);
    }
    report.written = backups.len();
    report.created = created.len();
    let mut saved = Ok(());
    if !backups.is_empty() {
        saved = state.history.add_tags(&id, backups);
    }
    if saved.is_ok() && !created.is_empty() {
        saved = state.history.add_created(&id, &created);
    }
    if report.written == 0 && report.created == 0 && own_entry {
        let _ = state.history.delete(&id);
    }
    match saved {
        Ok(()) => note.finish(),
        Err(e) => report.errors.push((String::new(), format!("err.history: {e}"))),
    }
    let changed: HashSet<PathBuf> = changed.into_iter().collect();
    for f in state.session.lock().unwrap().files.iter_mut() {
        if changed.contains(&f.path) {
            f.forget_tags();
        }
    }
    Ok(report)
}

/// An album folder's own cover image (`cover`, `folder` or `front`).
fn folder_cover(dir: &Path) -> Option<Vec<u8>> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut found: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            ["cover.", "folder.", "front."]
                .iter()
                .any(|s| n.starts_with(s))
                && (n.ends_with(".jpg") || n.ends_with(".jpeg") || n.ends_with(".png"))
        })
        .collect();
    found.sort();
    std::fs::read(found.first()?).ok()
}

/// Reads each book's details and looks it up on Open Library.
#[tauri::command]
async fn book_lookup(
    app: AppHandle,
    lang: String,
    items: Vec<openlibrary::LookupItem>,
) -> Result<Vec<openlibrary::Lookup>, String> {
    let ol = &app.state::<AppState>().ol;
    Ok(openlibrary::lookup(&app, ol, &lang, items).await)
}

/// A search typed in the book review.
#[tauri::command]
async fn book_search(
    app: AppHandle,
    lang: String,
    title: String,
    author: String,
) -> Result<Vec<renome_core::books::Candidate>, String> {
    // An ISBN or a link pasted: that book.
    if let Some(link) = links::parse(&title) {
        return app.state::<AppState>().ol.by_link(&link, &lang).await;
    }
    let guess = renome_core::books::Guess {
        title,
        author,
        from_file: true,
        ..Default::default()
    };
    Ok(app.state::<AppState>().ol.find(&guess, &lang).await?.0)
}

/// What each book says about itself (its details and name), as a start
/// for typing them in; `names` are the names the rule reads (stems).
#[tauri::command]
async fn book_guesses(paths: Vec<PathBuf>, names: Vec<String>) -> Vec<renome_core::books::Guess> {
    tauri::async_runtime::spawn_blocking(move || {
        paths
            .iter()
            .zip(names)
            .map(|(p, n)| renome_core::books::guess(&n, &renome_core::ebook::read(p)))
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// What each comic says about itself (its `ComicInfo.xml` and name), for
/// grouping files by series and matching them to issues.
#[tauri::command]
async fn comic_guesses(
    paths: Vec<PathBuf>,
    names: Vec<Option<String>>,
    by_folder: bool,
) -> Vec<Option<renome_core::comics::ComicMatch>> {
    tauri::async_runtime::spawn_blocking(move || {
        paths
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let stem = names.get(i).cloned().flatten();
                renome_core::comics::from_path(p, stem.as_deref(), by_folder)
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn comic_check(app: AppHandle, key: String) -> Result<(), String> {
    app.state::<AppState>().cv.check(&key).await
}

/// The Grand Comics Database account from the options (empty: none).
#[tauri::command]
fn gcd_account(state: State<AppState>, email: String, password: String) {
    state.gcd.set_account(&email, &password);
}

/// Checks a Grand Comics Database account.
#[tauri::command]
async fn gcd_check(app: AppHandle, email: String, password: String) -> Result<(), String> {
    app.state::<AppState>().gcd.check(&email, &password).await
}

/// Series with a name in a catalogue (`gcd`, `comicvine`, `metron`,
/// `mangadex`, `anilist`, `mangaupdates`), from a year and by a publisher
/// when given (the manga catalogues are not searched by publisher), best
/// first. `key` is Comic Vine's key or Metron's token.
// The command's arguments are what the window sends, each by name.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn comic_search(
    app: AppHandle,
    source: String,
    key: String,
    query: String,
    year: Option<u16>,
    publisher: String,
    lang: String,
    skip: Option<String>,
) -> Result<Vec<renome_core::comics::Volume>, String> {
    let state = app.state::<AppState>();
    // A link pasted: that series, from the catalogue the rule uses.
    if let Some(link) = links::parse(&query) {
        let ours = if source.is_empty() {
            "gcd"
        } else {
            source.as_str()
        };
        if link.comic_source() != Some(ours) {
            return Err("linkOther".into());
        }
        let v = match link {
            Link::ComicVine { issue, id } => {
                let id = match issue {
                    true => state.cv.volume_of(&key, &id).await?,
                    false => id,
                };
                state.cv.issues(&key, &id).await?.0
            }
            Link::Metron(id) => state.mt.issues(&key, &id).await?.0,
            Link::Gcd(id) => state.gcd.issues(&id).await?.0,
            Link::MangaDex(id) => state.md.series(&id, &lang).await?,
            Link::AniList { id, .. } => state.al.manga(id).await?,
            Link::MangaUpdates(id) => state.mu.series(&id, "").await?,
            _ => return Err("linkOther".into()),
        };
        return Ok(vec![v]);
    }
    let found = match source.as_str() {
        "comicvine" => state.cv.search(&key, &query, year, &publisher, &lang).await,
        "metron" => state.mt.search(&key, &query, year, &publisher, &lang).await,
        "mangadex" => state.md.search(&query, year, &lang).await,
        "anilist" => state.al.search(&query, year).await,
        "mangaupdates" => state.mu.search(&query, year).await,
        _ => state.gcd.search(&query, year, &publisher, &lang).await,
    }?;
    // The publishers the rule leaves out.
    Ok(renome_core::comics::skip_publishers(
        found,
        skip.as_deref().unwrap_or_default(),
    ))
}

/// A series and its issues, and the issue each comic is (by its number).
/// A manga catalogue gives the series alone, with its details (MangaUpdates
/// tells its authors and volumes only there).
#[tauri::command]
async fn comic_issues(
    app: AppHandle,
    source: String,
    key: String,
    id: String,
    comics: Vec<renome_core::comics::ComicMatch>,
) -> Result<ComicIssues, String> {
    let state = app.state::<AppState>();
    let (volume, issues) = match source.as_str() {
        "comicvine" => state.cv.issues(&key, &id).await?,
        "metron" => state.mt.issues(&key, &id).await?,
        "mangaupdates" => {
            let query = comics
                .first()
                .map(|c| c.series.as_str())
                .unwrap_or_default();
            (state.mu.series(&id, query).await?, Vec::new())
        }
        _ => state.gcd.issues(&id).await?,
    };
    let found = comics
        .iter()
        .map(|m| renome_core::comics::find_issue(m, &issues).map(|i| i.id.clone()))
        .collect();
    Ok(ComicIssues {
        volume,
        issues,
        found,
    })
}

/// The covers, by address, of the comic's issue (or volume) in a series,
/// to tell which series is the file's. None from the Grand Comics
/// Database (its pictures are not for programs) nor from catalogues of
/// whole series (AniList, MangaUpdates: their one cover is the series').
#[tauri::command]
async fn series_covers(
    state: State<'_, AppState>,
    source: String,
    key: String,
    id: String,
    comic: renome_core::comics::ComicMatch,
) -> Result<Vec<String>, String> {
    let (_, issues) = match source.as_str() {
        "mangadex" => {
            return match comic.volume {
                Some(v) => state.md.volume_covers(&id, v).await,
                None => Ok(Vec::new()),
            };
        }
        "comicvine" => state.cv.issues(&key, &id).await?,
        "metron" => state.mt.issues(&key, &id).await?,
        _ => return Ok(Vec::new()),
    };
    Ok(renome_core::comics::find_issue(&comic, &issues)
        .map(|i| i.cover.clone())
        .filter(|c| !c.is_empty())
        .into_iter()
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComicIssues {
    volume: renome_core::comics::Volume,
    issues: Vec<renome_core::comics::Issue>,
    /// The issue of each comic asked about, when its number says it.
    found: Vec<Option<String>>,
}

/// The barcodes printed on comics (read in their first and last pages),
/// a few at a time.
#[tauri::command]
async fn comic_barcodes(paths: Vec<PathBuf>) -> Vec<Option<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        let workers = std::thread::available_parallelism().map_or(2, |n| n.get().min(6));
        let next = std::sync::atomic::AtomicUsize::new(0);
        let out: Mutex<Vec<Option<String>>> = Mutex::new(vec![None; paths.len()]);
        std::thread::scope(|s| {
            for _ in 0..workers {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(p) = paths.get(i) else { break };
                        let code = renome_core::barcode::of_comic(p);
                        if let Ok(mut o) = out.lock() {
                            o[i] = code;
                        }
                    }
                });
            }
        });
        out.into_inner().unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

/// The issue a barcode belongs to, on Metron.
#[tauri::command]
async fn comic_by_code(
    app: AppHandle,
    key: String,
    code: String,
) -> Result<Option<metron::Coded>, String> {
    app.state::<AppState>().mt.by_code(&key, &code).await
}

/// Checks a Metron token.
#[tauri::command]
async fn metron_check(app: AppHandle, token: String) -> Result<(), String> {
    app.state::<AppState>().mt.check(&token).await
}

/// The Comic Vine series an issue belongs to.
#[tauri::command]
async fn comic_volume_of(app: AppHandle, key: String, issue: String) -> Result<String, String> {
    app.state::<AppState>().cv.volume_of(&key, &issue).await
}

/// Saves the books' covers and `metadata.opf` files and records them like
/// `write_sidecars`.
#[tauri::command]
async fn write_book_sidecars(
    app: AppHandle,
    history_id: Option<String>,
    items: Vec<openlibrary::SidecarItem>,
    cover: bool,
    opf: bool,
    in_folder: bool,
) -> Result<tmdb::SidecarReport, String> {
    let ol = &app.state::<AppState>().ol;
    let mut report = openlibrary::sidecars(&app, ol, items, cover, opf, in_folder).await;
    record_created(&app, history_id, &mut report);
    Ok(report)
}

/// The album folders among (or above) `dirs` that would get a playlist.
/// The album folders among `dirs` without a cover image yet: those the
/// album cover would be saved in.
#[tauri::command]
async fn cover_dirs(dirs: Vec<PathBuf>) -> Vec<PathBuf> {
    tauri::async_runtime::spawn_blocking(move || {
        dirs.into_iter()
            .filter(|d| !musicbrainz::has_cover(d))
            .collect()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn playlist_dirs(dirs: Vec<PathBuf>) -> Vec<PathBuf> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut out: Vec<PathBuf> = Vec::new();
        for d in dirs
            .iter()
            .filter_map(|d| renome_core::playlists::playlist_dir(d))
        {
            if !out.contains(&d) {
                out.push(d);
            }
        }
        out
    })
    .await
    .unwrap_or_default()
}

/// Writes a playlist for each album in `dirs` (see
/// `playlists::album_playlist`) and records them like `write_sidecars`.
#[tauri::command]
async fn write_playlists(
    app: AppHandle,
    history_id: Option<String>,
    dirs: Vec<PathBuf>,
) -> Result<tmdb::SidecarReport, String> {
    let mut report = tauri::async_runtime::spawn_blocking(move || {
        let mut report = tmdb::SidecarReport::default();
        let mut seen = HashSet::new();
        let mut kept = HashSet::new();
        for dir in dirs {
            let Some((path, text)) = renome_core::playlists::album_playlist(&dir) else {
                if renome_core::playlists::has_playlist(&dir) {
                    kept.insert(renome_core::music::album_dir(&dir).to_path_buf());
                }
                continue;
            };
            if !seen.insert(path.clone()) {
                continue;
            }
            match tmdb::write_new(&path, text.as_bytes()) {
                Ok(()) => report.created.push(path),
                Err(e) => report.errors.push((path.display().to_string(), e)),
            }
        }
        report.kept = kept.len();
        report
    })
    .await
    .map_err(|e| e.to_string())?;
    record_created(&app, history_id, &mut report);
    Ok(report)
}

#[tauri::command]
fn history(state: State<AppState>) -> Vec<HistoryEntry> {
    state.history.list()
}

#[tauri::command]
async fn undo(app: AppHandle, id: String) -> Result<ApplyReport, String> {
    let stop = stop_signal(&app);
    let emitter = app.clone();
    in_background(app, move |state| {
        // How far: songs whose tags are back, then bytes moved back from
        // another disk (renames on the same disk take no time).
        let last = std::cell::Cell::new(std::time::Instant::now());
        let tell = |kind: &str, done: u64, total: u64| {
            if last.get().elapsed().as_millis() >= 200 || done == total {
                last.set(std::time::Instant::now());
                let _ = emitter.emit(
                    "undo-progress",
                    serde_json::json!({ "done": done, "total": total, "kind": kind }),
                );
            }
        };
        let progress = |done: u64, total: u64| tell("files", done, total);
        let hooks = renome_core::apply::Hooks {
            progress: &progress,
            stop: &stop,
        };
        let tags = |done: usize, total: usize| tell("tags", done as u64, total as u64);
        let report = state.history.undo_with(&id, &hooks, &tags)?;
        let s = &mut *state.session.lock().unwrap();
        refresh_renamed(&mut s.files, &report);
        // Songs may have their old tags back: read again.
        for f in s.files.iter_mut() {
            if renome_core::music::is_audio(&f.ext) {
                f.forget_tags();
            }
        }
        s.undo.clear();
        Ok(report)
    })
    .await?
}

/// Puts back renames a stopped program left halfway (see
/// [`History::recover`]); called once at start.
#[tauri::command]
async fn recover(app: AppHandle) -> Result<renome_core::apply::Recovered, String> {
    in_background(app, |state| state.history.recover()).await
}

#[tauri::command]
fn delete_history(state: State<AppState>, id: String) -> Result<(), String> {
    state.history.delete(&id)
}

#[tauri::command]
fn clear_history(state: State<AppState>) -> Result<usize, String> {
    state.history.clear()
}

#[tauri::command]
fn save_preset(path: PathBuf, steps: Vec<Step>) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&steps).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// Writes a text file the user chose (the exported list).
#[tauri::command]
fn save_text(path: PathBuf, text: String) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_preset(path: PathBuf) -> Result<Vec<Step>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("invalidPreset: {e}"))
}

fn settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("renome")
        .join("settings.json")
}

fn read_settings_file() -> Option<Value> {
    let text = std::fs::read_to_string(settings_path()).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_settings_file(settings: &Value) -> Result<(), String> {
    let path = settings_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// The settings file is read and written whole by the settings and the
/// secrets apart: one at a time.
static SETTINGS_FILE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Saved settings; `None` on first run. The format is defined by the UI.
/// Secrets come apart ([`load_secrets`]), unless the file still has them.
#[tauri::command]
fn load_settings() -> Option<Value> {
    read_settings_file()
}

/// Saves the settings but their secrets, which [`save_secrets`] saves:
/// those in the file are kept as they are.
#[tauri::command]
async fn save_settings(mut settings: Value) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _file = SETTINGS_FILE.lock().unwrap();
        let old = read_settings_file()
            .and_then(|v| v.as_object().map(secrets::of))
            .unwrap_or_default();
        if let Some(obj) = settings.as_object_mut() {
            for k in secrets::FIELDS {
                obj.remove(k);
            }
            obj.extend(old);
        }
        ANSWERS.set_days(cache_days(Some(&settings)));
        write_settings_file(&settings)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The secrets from the system's password store, which may wait for the
/// user to unlock it or allow it, and how that went: `stored` (then
/// `secrets` are those), `refused` (the store did not answer: what it holds
/// stays there, untouched) or `missing` (no store: those in the file
/// hold). Secrets still in the file (from before the store was used, or
/// saved there while it did not answer, so the newest) move to the store.
#[tauri::command]
async fn load_secrets() -> serde_json::Value {
    tauri::async_runtime::spawn_blocking(|| {
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
    })
    .await
    .unwrap_or_else(|_| serde_json::json!({ "status": "refused" }))
}

/// Saves the secrets in the system's password store; in the settings
/// file when the store does not take them (or was not read: then what it
/// holds stays as it is). Whether the store took them.
#[tauri::command]
async fn save_secrets(secrets: serde_json::Map<String, Value>) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
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
    })
    .await
    .map_err(|e| e.to_string())?
}

/// System language (`pt_BR.UTF-8` → `pt`), from the locale variables.
#[tauri::command]
fn system_language() -> String {
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

/// Inside the AppImage, GTK reads the GNOME schemas bundled with it, whose
/// default title bar layout is "close" only; distros that change that
/// default (e.g. Bazzite, with minimize/maximize) would be ignored. Ask the
/// system's `gsettings`, without the AppImage's variables.
#[cfg(target_os = "linux")]
fn sync_titlebar_buttons() {
    use gtk::prelude::GtkSettingsExt;

    if std::env::var_os("APPIMAGE").is_none() {
        return;
    }
    let Ok(out) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.wm.preferences", "button-layout"])
        .env_remove("GSETTINGS_SCHEMA_DIR")
        .env_remove("GIO_MODULE_DIR")
        .env_remove("XDG_DATA_DIRS")
        .env_remove("LD_LIBRARY_PATH")
        .output()
    else {
        return;
    };
    let layout = String::from_utf8_lossy(&out.stdout);
    let layout = layout.trim().trim_matches('\'');
    if out.status.success()
        && layout.contains(':')
        && let Some(s) = gtk::Settings::default()
    {
        s.set_gtk_decoration_layout(Some(layout));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Answers too old to use go, out of the way of the start.
    std::thread::spawn(|| ANSWERS.purge());

    let history_dir = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("renome")
        .join("history");

    // Paths given on the command line go into the initial list.
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let mut session = Session::default();
    if !args.is_empty() {
        let include_hidden = read_settings_file()
            .and_then(|s| s.get("includeHidden")?.as_bool())
            .unwrap_or(false);
        let opts = ScanOptions {
            include_hidden,
            ..Default::default()
        };
        session.files = renome_core::scan(&args, &opts, &HashSet::new()).files;
        renome_core::sort_files(&mut session.files, SortKey::Name, false);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|_| {
            #[cfg(target_os = "linux")]
            sync_titlebar_buttons();
            Ok(())
        })
        .manage(AppState {
            session: Mutex::new(session),
            history: History::new(history_dir),
            hashes: Mutex::default(),
            covers: tokio::sync::Mutex::new(HashMap::new()),
            pictures: Mutex::new(renome_core::similar::PictureCache::in_file(
                dirs::cache_dir()
                    .unwrap_or_else(std::env::temp_dir)
                    .join("renome")
                    .join("pictures"),
            )),
            identical_run: AtomicU64::new(0),
            busy_disk: Mutex::new(()),
            stop_run: AtomicU64::new(0),
            mb: musicbrainz::Client::default(),
            acoustid: acoustid::Client::default(),
            heard: Mutex::new(HashMap::new()),
            ol: openlibrary::Client::default(),
            cv: comicvine::Client::default(),
            gcd: gcd::Client::default(),
            md: mangadex::Client::default(),
            al: anilist::Client::default(),
            mu: mangaupdates::Client::default(),
            mt: metron::Client::default(),
            dg: discogs::Client::default(),
        })
        .invoke_handler(tauri::generate_handler![
            pdf_print_available,
            checkpoint,
            drop_checkpoint,
            undo_list,
            add_paths,
            remove_files,
            stop_search,
            find_similar,
            picture_cards,
            own_cover,
            cover_distances,
            series_covers,
            remote_image,
            trash_files,
            tmdb_collections,
            clear_files,
            move_files,
            sort_files,
            file_details,
            discogs_token,
            google_books_key,
            google_books_check,
            discogs_check,
            preview,
            find_identical,
            tmdb_check,
            tmdb_lookup,
            tmdb_search,
            tv_search,
            tv_lookup,
            tv_groups,
            write_sidecars,
            write_tv_sidecars,
            subs_search,
            write_subtitles,
            subtitle_tools,
            os_check,
            subdl_check,
            mb_lookup,
            mb_search,
            mb_choose,
            write_covers,
            write_playlists,
            write_tags,
            playlist_dirs,
            cover_dirs,
            book_lookup,
            book_search,
            comic_guesses,
            book_guesses,
            song_own_tags,
            comic_check,
            comic_search,
            comic_issues,
            comic_volume_of,
            anime_search,
            anime_lookup,
            comic_barcodes,
            comic_by_code,
            metron_check,
            gcd_account,
            gcd_check,
            save_text,
            tmdb_images,
            recover,
            write_book_sidecars,
            write_inside,
            apply,
            history,
            undo,
            delete_history,
            clear_history,
            cache_size,
            clear_cache,
            save_preset,
            load_preset,
            load_settings,
            save_settings,
            load_secrets,
            save_secrets,
            system_language,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Renome");
}
