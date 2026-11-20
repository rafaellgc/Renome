use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use renome_core::scan::Skipped;
use renome_core::{
    ApplyReport, FileEntry, HashCache, History, Preview, ScanOptions, SortKey, Step, View,
};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

mod update;

use renome_services::{
    ANSWERS, acoustid, anilist, cache_days, cache_root, comicvine, discogs, drop_old_cache, gcd,
    mangadex, mangaupdates, mediaserver, metron, musicbrainz, openlibrary, read_settings_file,
    secrets, subtitles, tmdb, write_settings_file,
};

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
    covers: renome_services::covers::Covers,
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
    /// The flows' questions waiting for the page's answer, by number.
    asks: Mutex<HashMap<u64, tokio::sync::oneshot::Sender<Option<Value>>>>,
    ask_seq: AtomicU64,
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
    /// The list as last shown, and as last renamed: what is written after
    /// the renames is planned on them (see `flow::after`).
    shown: Mutex<Option<renome_core::Preview>>,
    /// What is missing from the list as last shown.
    missing: Mutex<Vec<renome_services::flow::missing::Missing>>,
    renamed: Mutex<Option<renome_core::Preview>>,
}

type Overrides = HashMap<PathBuf, String>;

/// The bytes the kept answers take.
#[tauri::command]
async fn cache_size() -> (u64, String) {
    let n = tauri::async_runtime::spawn_blocking(|| ANSWERS.size())
        .await
        .unwrap_or(0);
    (n, renome_core::text::size(n))
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

use renome_services::protected_dirs;

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
    lang: String,
) -> Result<HashMap<PathBuf, HashMap<&'static str, String>>, String> {
    // As the list shows them (see `Details::text`: the CSV saves the same).
    in_background(app, move |state| {
        let want: HashSet<PathBuf> = paths.into_iter().collect();
        let s = state.session.lock().unwrap();
        s.files
            .iter()
            .filter(|f| want.contains(&f.path))
            .map(|f| (f.path.clone(), renome_core::details::of(f).texts(&lang)))
            .collect()
    })
    .await
}

/// The list as shown, as CSV cells (see `flow::export::list`, the command
/// line's `--csv` too): the optional columns asked for.
#[tauri::command]
async fn export_rows(
    app: AppHandle,
    columns: Vec<String>,
    lang: String,
) -> Result<Vec<Vec<renome_services::flow::export::Cell>>, String> {
    in_background(app, move |state| {
        let preview = state.shown.lock().unwrap().clone().unwrap_or_default();
        let cols: Vec<&str> = columns.iter().map(String::as_str).collect();
        let details: HashMap<PathBuf, renome_core::details::Details> = if cols.is_empty() {
            HashMap::new()
        } else {
            let s = state.session.lock().unwrap();
            s.files
                .iter()
                .map(|f| (f.path.clone(), renome_core::details::of(f)))
                .collect()
        };
        renome_services::flow::export::list(&preview, &details, &cols, &lang)
    })
    .await
}

/// The missing items as CSV, where they go and under what name (see
/// `flow::export::missing`, the command line's `--missing-csv` too).
#[tauri::command]
async fn export_missing(
    app: AppHandle,
) -> Result<renome_services::flow::export::MissingCsv, String> {
    in_background(app, move |state| {
        let missing = state.missing.lock().unwrap().clone();
        let preview = state.shown.lock().unwrap().clone().unwrap_or_default();
        let dirs: HashMap<&PathBuf, &PathBuf> =
            preview.items.iter().map(|i| (&i.path, &i.dir)).collect();
        renome_services::flow::export::missing(&missing, &|p| {
            dirs.get(&p.to_path_buf()).map(|d| (*d).clone())
        })
    })
    .await
}

/// The rules as the window sends them, read as the core reads them (see
/// `catalog::normalize_steps`, the command line's too); what is asked of
/// the release's adverts, from them.
fn rule_steps(mut steps: Vec<Value>, view: &mut View) -> Result<Vec<Step>, String> {
    renome_core::catalog::normalize_steps(&mut steps);
    view.trash_ads = renome_core::catalog::trash_ads(&steps);
    serde_json::from_value(Value::Array(steps)).map_err(|e| e.to_string())
}

#[tauri::command]
async fn preview(
    app: AppHandle,
    steps: Vec<Value>,
    overrides: Overrides,
    mut view: View,
) -> Result<Preview, String> {
    let steps = rule_steps(steps, &mut view)?;
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
        *state.shown.lock().unwrap() = Some(preview.clone());
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

/// The window's comic catalogues.
fn sources(state: &AppState) -> renome_services::comics::Sources<'_> {
    renome_services::comics::Sources {
        cv: &state.cv,
        mt: &state.mt,
        gcd: &state.gcd,
        md: &state.md,
        al: &state.al,
        mu: &state.mu,
    }
}

/// Files written: their tags are read again when next needed.
fn forget_tags_of(state: &AppState, written: Vec<PathBuf>) {
    let written: HashSet<PathBuf> = written.into_iter().collect();
    for f in state.session.lock().unwrap().files.iter_mut() {
        if written.contains(&f.path) {
            f.forget_tags();
        }
    }
}

/// The window as the host of the shared work: reports become events for
/// the page, Stop is the page's button.
#[derive(Clone)]
struct Gui(AppHandle);

impl renome_services::Host for Gui {
    fn report(&self, event: &str, payload: Value) {
        let _ = self.0.emit(event, payload);
    }
    fn stop_signal(&self) -> std::sync::Arc<dyn Fn() -> bool + Send + Sync> {
        std::sync::Arc::new(stop_signal(&self.0))
    }
    fn sender(&self) -> renome_services::Sender {
        let app = self.0.clone();
        std::sync::Arc::new(move |event, payload| {
            let _ = app.emit(event, payload);
        })
    }
    fn print_pdf(
        &self,
        html: PathBuf,
        pdf: PathBuf,
        page: renome_core::bookconv::print::Page,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move { print_to_pdf(&self.0, &html, &pdf, page).await })
    }
    /// The question goes to the page (`flow-ask`), which opens its picker
    /// and answers with `flow_answer`.
    fn ask(
        &self,
        question: renome_services::flow::Question,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<Value>> + Send + '_>> {
        Box::pin(async move {
            let state = self.0.state::<AppState>();
            let id = state.ask_seq.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = tokio::sync::oneshot::channel();
            state.asks.lock().unwrap().insert(id, tx);
            let _ = self.0.emit(
                "flow-ask",
                serde_json::json!({ "id": id, "question": question }),
            );
            rx.await.ok().flatten()
        })
    }
}

/// The page's answer to a flow's question (`None`: none fits).
#[tauri::command]
fn flow_answer(state: State<AppState>, id: u64, answer: Option<Value>) {
    if let Some(tx) = state.asks.lock().unwrap().remove(&id) {
        let _ = tx.send(answer);
    }
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
    /// The same movie, episode, comic or book in several files.
    works: Vec<renome_core::repeated::Group>,
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
        use renome_core::repeated::by_group;
        let identical = by_group(&groups);
        // The songs' tags are read on the copy, without holding the list
        // (the command line's `duplicates` finds them the same way).
        let (songs, works) = renome_core::repeated::repeated(&files, &identical, &stale)?;
        if stale() {
            return None;
        }
        let mut s = state.session.lock().unwrap();
        s.identical = identical;
        // One filter for both: songs' groups first, then the works'.
        let all: Vec<Vec<PathBuf>> = songs
            .iter()
            .cloned()
            .chain(
                works
                    .iter()
                    .map(|g| g.items.iter().map(|i| i.path.clone()).collect()),
            )
            .collect();
        s.alike = by_group(&all);
        let read: HashMap<&PathBuf, &FileEntry> = files.iter().map(|f| (&f.path, f)).collect();
        for f in s.files.iter_mut() {
            if let Some(other) = read.get(&f.path) {
                f.keep_read(other);
            }
        }
        Some(Copies {
            identical: groups,
            songs,
            works,
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
        state.session.lock().unwrap().similar = renome_core::repeated::by_group(&groups);
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
    /// The size as shown (`12 MB`).
    size_text: String,
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

/// A picture of a group as the comparison window knows it.
#[derive(serde::Deserialize)]
struct CardFacts {
    path: PathBuf,
    width: Option<u32>,
    height: Option<u32>,
    size: u64,
    taken: Option<String>,
    modified: Option<i64>,
}

/// Which pictures of a group stand out (see `similar::picks`: the
/// command line marks the same).
#[tauri::command]
fn picture_picks(cards: Vec<CardFacts>) -> renome_core::similar::Picks {
    let worth: Vec<renome_core::similar::Worth> = cards
        .iter()
        .map(|c| {
            let taken = c
                .taken
                .as_deref()
                .and_then(|t| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S").ok());
            renome_core::similar::Worth::of(
                &c.path,
                c.width.zip(c.height),
                c.size,
                taken,
                c.modified,
            )
        })
        .collect();
    renome_core::similar::picks(&worth)
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
                size_text: renome_core::text::size(entry.as_ref().map_or(0, |e| e.size)),
                modified: entry.as_ref().and_then(|e| e.modified),
                taken: entry
                    .as_ref()
                    .filter(|_| renome_core::similar::is_picture(&ext))
                    .and_then(|e| e.taken())
                    .map(|t| t.format("%Y-%m-%dT%H:%M:%S").to_string()),
                width: dims.map(|d| d.0),
                height: dims.map(|d| d.1),
                image: shown.map(|(jpeg, _)| {
                    format!("data:image/jpeg;base64,{}", renome_core::b64::encode(&jpeg))
                }),
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
        let (bytes, hash) = renome_services::covers::own_cover(&path)?;
        let (jpeg, _) = renome_core::similar::shown_copy(&bytes, "img", 600)?;
        Some(OwnCover {
            image: format!("data:image/jpeg;base64,{}", renome_core::b64::encode(&jpeg)),
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
    (bytes.len() <= 4 << 20)
        .then(|| format!("data:{kind};base64,{}", renome_core::b64::encode(&bytes)))
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
    // Only those that are the file's cover (see `covers::SAME_COVER`).
    let own = u64::from_str_radix(&hash, 16).map_err(|e| e.to_string())?;
    Ok(state.covers.same(own, &urls).await)
}

/// The series found, compared with the file's cover (see
/// `covers::series_by_cover`, the command line's chooser too); tells how
/// far it got as `cover-progress`.
#[tauri::command]
async fn series_by_cover(
    app: AppHandle,
    source: String,
    key: String,
    hash: String,
    comic: renome_core::comics::ComicMatch,
    candidates: Vec<renome_core::comics::Volume>,
) -> Result<renome_services::covers::SeriesByCover, String> {
    let own = u64::from_str_radix(&hash, 16).map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    let sources = sources(&state);
    let emitter = app.clone();
    Ok(renome_services::covers::series_by_cover(
        &state.covers,
        &sources,
        &source,
        &key,
        own,
        &comic,
        candidates,
        &move |i, n| {
            let _ = emitter.emit("cover-progress", serde_json::json!({ "i": i, "n": n }));
        },
    )
    .await)
}

/// A series' issues in the order the review shows them, those `typed`
/// names (see `covers::issue_order`, the command line's too).
#[tauri::command]
fn issue_order(
    issues: Vec<renome_core::comics::Issue>,
    own: renome_core::comics::ComicMatch,
    far: HashMap<String, Option<u32>>,
    typed: String,
) -> Vec<usize> {
    renome_services::covers::issue_order(&issues, &own, &far, &typed)
}

/// The covers a review compares, and whether at once (few) or when asked.
#[tauri::command]
fn issue_covers(
    issues: Vec<renome_core::comics::Issue>,
    volume_key: String,
) -> (Vec<String>, bool, Option<&'static str>) {
    let urls = renome_services::covers::issue_covers(&issues);
    let now = urls.len() <= renome_services::covers::COMPARE_AT_ONCE;
    // The GCD gives no covers to programs: said so.
    let note = volume_key.starts_with("gcd:").then_some("coverNoneGcd");
    (urls, now, note)
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

/// Updates the session entries after renaming (see `apply::Follow`: what
/// the window found per file follows them the same way), and tells the
/// window where each one went (`moved`).
fn refresh_renamed(files: &mut [FileEntry], report: &mut ApplyReport) {
    report.moved = followed(files, &report.done);
}

/// [`refresh_renamed`] for renames done: where each entry went.
fn followed(files: &mut [FileEntry], done: &[renome_core::Operation]) -> Vec<(PathBuf, PathBuf)> {
    if done.is_empty() {
        return Vec::new();
    }
    let follow = renome_core::apply::Follow::new(done);
    let mut moved = Vec::new();
    for f in files.iter_mut() {
        let path = follow.path(&f.path);
        if path != f.path {
            moved.push((f.path.clone(), path.clone()));
            if let Ok(meta) = std::fs::symlink_metadata(&path)
                && let Some(e) = FileEntry::from_path(&path, &meta)
            {
                *f = e;
            }
        }
    }
    moved
}

/// The renames done, with what is said of them (the shared lines, as the
/// command line says them: `flow::after::renamed`).
#[derive(Serialize)]
struct Applied {
    #[serde(flatten)]
    report: ApplyReport,
    said: Vec<renome_services::flow::after::Line>,
    /// Each file that failed, as said (`flow::said::failed`).
    failed: Vec<renome_services::flow::after::Line>,
}

impl Applied {
    fn new(report: ApplyReport, said: Vec<renome_services::flow::after::Line>) -> Self {
        let failed = report
            .errors
            .iter()
            .map(renome_services::flow::said::failed)
            .collect();
        Applied {
            report,
            said,
            failed,
        }
    }
}

#[tauri::command]
async fn apply(
    app: AppHandle,
    steps: Vec<Value>,
    overrides: Overrides,
    mut view: View,
) -> Result<Applied, String> {
    let steps = rule_steps(steps, &mut view)?;
    let sent = view.send_to.as_ref().map(|s| s.copy);
    let stop = stop_signal(&app);
    let emitter = app.clone();
    in_background(app, move |state| {
        let _disk = state.busy_disk.lock().unwrap();
        // Not while the command line renames.
        let _lock = renome_services::RenameLock::take(|| ()).ok();
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
        let mut report = state.history.apply_with(&ops, &hooks);
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
        refresh_renamed(&mut s.files, &mut report);
        // Earlier states point to the old names.
        s.undo.clear();
        let said = renome_services::flow::after::renamed(&plan, &report, sent);
        *state.renamed.lock().unwrap() = Some(plan);
        Applied::new(report, said)
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

/// The movie rule's search: what is sure is picked, the rest are doubts
/// (the flow the command line runs too).
#[tauri::command]
async fn movie_search(
    app: AppHandle,
    key: String,
    lang: String,
    items: Vec<renome_services::flow::Item>,
) -> Result<Vec<renome_services::flow::movies::Found>, String> {
    let client = tmdb::Client::new(&key, &lang);
    renome_services::flow::movies::search(&Gui(app.clone()), &client, &items).await
}

/// Each chosen movie as the rule names it, with its template fields (the
/// shared flow's: `flow::movies::named`).
#[tauri::command]
fn movie_named(
    files: Vec<(PathBuf, renome_core::tmdb::Candidate)>,
    extras: HashMap<u64, renome_core::fields::Extra>,
) -> HashMap<PathBuf, renome_core::tmdb::MovieMatch> {
    renome_services::flow::movies::named(files, &extras)
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
    // A link or id pasted: that movie (see `flow::typed`, the command
    // line's too).
    let client = tmdb::Client::new(&key, &lang);
    renome_services::flow::typed::movie(&client, &title, year).await
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
    renome_services::flow::typed::show(&client, None, &query, &items).await
}

/// Like `tv_search`, on AniList (anime, no key).
#[tauri::command]
async fn anime_search(
    app: AppHandle,
    query: String,
    items: Vec<tmdb::TvItem>,
) -> Result<tmdb::ShowSearch, String> {
    let state = app.state::<AppState>();
    let client = tmdb::Client::new("", "");
    renome_services::flow::typed::show(&client, Some(&state.al), &query, &items).await
}

/// The TV show rule's search (the flow the command line runs too): the
/// shows, asked with `flow-ask` when unsure, and each file's episode.
/// `settled`: [path, show id] of the files that keep their episode.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn series_search(
    app: AppHandle,
    key: String,
    lang: String,
    source: String,
    typed: String,
    items: Vec<renome_services::flow::Item>,
    settled: Vec<(PathBuf, u64)>,
    picked: HashMap<String, renome_core::tmdb::Candidate>,
) -> Result<renome_services::flow::series::Outcome, String> {
    use renome_services::flow::series::{self, Source};
    let client = tmdb::Client::new(&key, &lang);
    let state = app.state::<AppState>();
    let source = if source == "anilist" {
        Source::Anilist(&state.al)
    } else {
        Source::Tmdb(&client)
    };
    let settled: HashSet<(PathBuf, u64)> = settled.into_iter().collect();
    series::search(&Gui(app.clone()), source, &typed, &items, &settled, picked).await
}

/// Each file's episodes as the rule names them.
#[tauri::command]
fn series_named(
    files: Vec<renome_services::flow::series::Settled>,
    shows: Vec<renome_core::tv::Show>,
) -> HashMap<PathBuf, renome_core::tv::EpisodeMatch> {
    renome_services::flow::series::named(&files, &shows)
}

/// Everything the window builds rules, their menus and its options from,
/// in this language (the command line's too): see `catalog::table` and
/// `options::table`, and the details windows' fields.
/// The "+ Rule" menu's groups in their order, by the rules' names as
/// the window says them (the shared order: `catalog::menu`, the command
/// line's manual too).
#[tauri::command]
fn rule_menu(names: HashMap<String, String>) -> (Vec<&'static str>, Vec<&'static str>) {
    renome_core::catalog::menu(|k| names.get(k).cloned().unwrap_or_default())
}

/// The manual's pages in order, by their ids (`catalog::manual_order`, the
/// command line's `manual` too): each page's place, and a group's title.
#[tauri::command]
fn manual_order(
    ids: Vec<String>,
    names: HashMap<String, String>,
) -> Vec<(usize, Option<&'static str>)> {
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    renome_core::catalog::manual_order(&ids, |k| names.get(k).cloned().unwrap_or_default())
}

/// The reader a comic or book rule makes its e-book for, and the formats
/// it does not open (the command line warns of the same:
/// `readers::reader`).
#[tauri::command]
fn rule_reader(rule: Value) -> renome_core::readers::Reader {
    renome_core::readers::reader(&rule)
}

#[tauri::command]
fn rule_table(lang: String) -> Value {
    let mut t = renome_core::catalog::table(&lang);
    t["options"] = renome_services::options::table();
    t["editFields"] = renome_services::flow::edit::fields_table();
    // Each optional column's title (its key in the texts), as the CSV's.
    t["columnLabels"] = renome_core::catalog::COLUMNS
        .iter()
        .map(|c| {
            (
                c.to_string(),
                Value::from(renome_services::flow::export::column_label(c)),
            )
        })
        .collect::<serde_json::Map<String, Value>>()
        .into();
    // The comic catalogues of whole series (a file's volume and chapter stay).
    let sources = renome_core::catalog::rule("comic")
        .and_then(|r| r.option("source"))
        .map(|o| match o.kind {
            renome_core::catalog::Kind::Choice(list) => list.iter().map(|c| c.value).collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default();
    t["seriesOnly"] = sources
        .into_iter()
        .filter(|s| renome_services::flow::comics::series_only(s))
        .collect();
    t
}

/// The episodes what was typed names (see `flow::series::typed`, the
/// command line's chooser too).
#[tauri::command]
fn episodes_typed(
    show: renome_core::tv::Show,
    text: String,
) -> renome_services::flow::series::Typed {
    renome_services::flow::series::typed(&show, &text)
}

/// Where each picture of a group is, as the comparison window says it
/// (see `similar::places`).
#[tauri::command]
fn picture_places(group: Vec<PathBuf>) -> Vec<String> {
    renome_core::similar::places(&group)
}

/// Whether the GCD is searched without an account (see `options::gcd_few`).
#[tauri::command]
fn gcd_few(source: String, email: String, password: String) -> bool {
    renome_services::options::gcd_few(&source, &email, &password)
}

/// The options a rule's card hides as the rule is set (see
/// `catalog::hidden`: those under another that leaves them nothing to
/// do, and the inert ones).
#[tauri::command]
fn rule_inert(rule: Value) -> Vec<&'static str> {
    renome_core::catalog::hidden(&rule)
}

/// Episodes as file names write them (`S01E02-E03`: `flow::series::code`,
/// the command line's too), for each list.
#[tauri::command]
fn episode_codes(lists: Vec<Vec<(u16, u16)>>) -> Vec<String> {
    lists
        .iter()
        .map(|keys| renome_services::flow::series::code(keys))
        .collect()
}

/// A larger copy of a catalogue's cover, where there is one.
#[tauri::command]
fn bigger_cover(url: String) -> String {
    renome_services::mangadex::bigger(&url)
}

/// Where a new album search looks first (see `flow::typed`).
#[tauri::command]
fn album_on_discogs(first_found: Option<String>, token: bool) -> bool {
    renome_services::flow::typed::album_on_discogs(first_found.as_deref(), token)
}

/// The files that take an answer given for `path` (see `flow::same_answer`,
/// the command line's too).
#[tauri::command]
fn same_answer(
    path: PathBuf,
    files: Vec<(PathBuf, String, renome_services::flow::Status)>,
) -> Vec<PathBuf> {
    renome_services::flow::same_answer(
        &path,
        files.iter().map(|(p, g, s)| (p.as_path(), g.as_str(), *s)),
    )
}

/// The list's optional columns that make sense with these rules.
#[tauri::command]
fn columns_for(steps: Vec<Value>) -> Vec<&'static str> {
    renome_core::catalog::columns_for(&steps)
}

/// The languages the catalogues are asked in: TMDB's (the one chosen, else
/// the interface's) and the comic catalogues'.
#[tauri::command]
fn catalog_langs(tmdb_lang: String, lang: String) -> Value {
    let chosen = serde_json::json!({ "tmdbLang": tmdb_lang });
    serde_json::json!({
        "tmdb": renome_services::options::tmdb_lang(Some(&chosen), &lang),
        "comic": renome_services::options::comic_lang(&lang),
    })
}

/// Why each rule cannot join the rules there (`types`), when it cannot.
#[tauri::command]
fn rule_blocks(types: Vec<String>) -> HashMap<String, Option<renome_core::catalog::Blocked>> {
    let there: Vec<&str> = types.iter().map(String::as_str).collect();
    renome_core::catalog::RULES
        .iter()
        .map(|spec| {
            (
                spec.kind.to_string(),
                renome_core::catalog::blocked_by(&there, spec.kind),
            )
        })
        .collect()
}

/// Saved steps made ready to use (see `catalog::complete_steps`, the
/// command line's too), and the types left out.
#[tauri::command]
fn complete_steps(steps: Vec<Value>, lang: String, saved: bool) -> (Vec<Value>, Vec<String>) {
    renome_core::catalog::complete_steps(steps, &lang, saved)
}

/// The details typed in a details window over each file's (see
/// `flow::edit`, the command line's `--edit` too): `kind` is `song`,
/// `book` or `comic`.
#[tauri::command]
fn edit_details(
    kind: String,
    items: Vec<Value>,
    changed: renome_services::flow::edit::Changed,
) -> Result<Vec<Value>, String> {
    use renome_services::flow::edit;
    items
        .into_iter()
        .map(|v| {
            let out = match kind.as_str() {
                "song" => serde_json::to_value(edit::song(
                    serde_json::from_value(v).map_err(|e| e.to_string())?,
                    &changed,
                )),
                "book" => serde_json::to_value(edit::book(
                    serde_json::from_value(v).map_err(|e| e.to_string())?,
                    &changed,
                )),
                _ => serde_json::to_value(edit::comic(
                    serde_json::from_value(v).map_err(|e| e.to_string())?,
                    &changed,
                )),
            };
            out.map_err(|e| e.to_string())
        })
        .collect()
}

/// A value the window sent, read as `T` (unreadable: none).
fn current<T: serde::de::DeserializeOwned>(v: &Option<Value>) -> Option<T> {
    v.clone().and_then(|v| serde_json::from_value(v).ok())
}

/// What the window knows of a file for its details: what was chosen or
/// typed (`current`), what the lookup read of it (`guess`: a book's
/// guess, a comic's own details).
#[derive(serde::Deserialize, Default)]
struct Known {
    #[serde(default)]
    current: Option<Value>,
    #[serde(default)]
    guess: Option<Value>,
}

/// The rows a details window opens on for those chosen, and what each
/// starts from (see `flow::edit`: the command line's `--edit` starts from
/// the same): `kind` is the rule's (`music`, `book`, `comic`, `photo`).
// The command's arguments are what the window sends, each by name.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn edit_start(
    app: AppHandle,
    kind: String,
    chosen: Vec<PathBuf>,
    known: HashMap<PathBuf, Known>,
    parts: HashMap<PathBuf, renome_services::flow::books::Part>,
    dates: HashMap<PathBuf, String>,
    from_name: bool,
    by_folder: bool,
    audiobooks: bool,
) -> Result<
    (
        Vec<PathBuf>,
        Vec<Value>,
        Vec<renome_services::flow::edit::Changed>,
    ),
    String,
> {
    use renome_services::flow::edit;
    in_background(app, move |state| {
        let items = state
            .shown
            .lock()
            .unwrap()
            .as_ref()
            .map(|p| p.items.clone())
            .unwrap_or_default();
        let chosen: HashSet<PathBuf> = chosen.into_iter().collect();
        let paths = edit::rows(&kind, &items, &chosen, &parts, audiobooks);
        let s = state.session.lock().unwrap();
        let files: HashMap<&PathBuf, &FileEntry> = s.files.iter().map(|f| (&f.path, f)).collect();
        let by_path: HashMap<&PathBuf, &renome_core::PreviewItem> = items.iter().map(|i| (&i.path, i)).collect();
        let empty = Known::default();
        let values = paths
            .iter()
            .map(|p| {
                let k = known.get(p).unwrap_or(&empty);
                let item = by_path.get(p);
                let name = item
                    .map(|i| i.lookup_name.clone().unwrap_or_else(|| i.original.clone()))
                    .unwrap_or_default();
                let is_dir = item.is_some_and(|i| i.is_dir);
                match kind.as_str() {
                    "music" => serde_json::to_value(edit::song_start(current(&k.current), files.get(p).copied(), from_name)),
                    "book" => serde_json::to_value(edit::book_start(current(&k.current), current(&k.guess).as_ref(), p, &name)),
                    "comic" => serde_json::to_value(edit::comic_start(
                        current(&k.current),
                        current(&k.guess).as_ref(),
                        p,
                        &name,
                        is_dir,
                        by_folder,
                    )),
                    _ => {
                        let [year, month, day, hour, minute] =
                            edit::date_parts(dates.get(p).map_or("", String::as_str));
                        Ok(serde_json::json!({ "year": year, "month": month, "day": day, "hour": hour, "minute": minute }))
                    }
                }
                .unwrap_or_default()
            })
            .collect::<Vec<Value>>();
        // As the window shows them (the same text the fields are read back from).
        let rows = values.iter().map(|v| edit::as_text(&kind, v)).collect();
        (paths, values, rows)
    })
    .await
}

/// Photo dates typed over the ones they had (empty: what the file says);
/// `None` when the fields make no date.
#[tauri::command]
fn edit_dates(
    old: Vec<String>,
    changed: renome_services::flow::edit::Changed,
) -> Option<Vec<String>> {
    old.iter()
        .map(|o| renome_services::flow::edit::photo_date(o, &changed))
        .collect()
}

/// What the lookups say is missing from the list as shown (see
/// `flow::missing`): the faded rows, beside their files.
#[tauri::command]
fn missing_items(
    app: AppHandle,
    rules: Vec<Value>,
    known: renome_services::flow::missing::Known,
    descending: bool,
) -> Vec<renome_services::flow::missing::Missing> {
    use renome_services::flow::missing;
    let state = app.state::<AppState>();
    let shown = state.shown.lock().unwrap();
    let found = match shown.as_ref() {
        Some(p) => missing::of(&rules, &known, p, descending, &missing::today()),
        None => Vec::new(),
    };
    // Kept for "Export the missing".
    *state.missing.lock().unwrap() = found.clone();
    found
}

/// What renaming the list as shown would write besides the renames (the
/// files already named right only; subtitles for every file), for the
/// question before renaming and the Rename button.
#[tauri::command]
fn after_counts(
    app: AppHandle,
    rules: Vec<Value>,
    chosen: renome_services::flow::after::Chosen,
) -> renome_services::flow::after::Counts {
    let state = app.state::<AppState>();
    let shown = state.shown.lock().unwrap();
    match shown.as_ref() {
        Some(p) => renome_services::flow::after::counts(&rules, p, &chosen),
        None => Default::default(),
    }
}

/// What the question before renaming says (the shared lines, the
/// command line's too: `flow::after::ask`), with the work besides the
/// renames, on the list as shown.
#[tauri::command]
fn rename_ask(
    app: AppHandle,
    rules: Vec<Value>,
    chosen: renome_services::flow::after::Chosen,
    send_to: Option<renome_core::plan::SendTo>,
) -> (
    renome_services::flow::after::Counts,
    renome_services::flow::after::Ask,
) {
    use renome_services::flow::after;
    let state = app.state::<AppState>();
    let shown = state.shown.lock().unwrap();
    let Some(p) = shown.as_ref() else {
        return Default::default();
    };
    let counts = after::counts(&rules, p, &chosen);
    let ask = after::ask(
        p,
        &counts,
        send_to.as_ref().map(|s| (s.dir.as_path(), s.copy)),
    );
    (counts, ask)
}

/// Everything written once the list is renamed (see `flow::after`): on
/// the list as it was renamed, with what the lookups chose; the list
/// follows what it moved, trashed or retagged.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn after_rename(
    app: AppHandle,
    rules: Vec<Value>,
    chosen: renome_services::flow::after::Chosen,
    done: Vec<(PathBuf, PathBuf)>,
    history_id: Option<String>,
    tmdb_key: String,
    tmdb_lang: String,
    subs: subtitles::Auth,
) -> Result<renome_services::flow::after::Report, String> {
    use renome_services::flow::after;
    let state = app.state::<AppState>();
    // The list as renamed (as shown, when nothing was).
    let renamed = !done.is_empty();
    let preview = renamed
        .then(|| state.renamed.lock().unwrap().take())
        .flatten()
        .or_else(|| state.shown.lock().unwrap().clone())
        .unwrap_or_default();
    let moved: HashMap<PathBuf, PathBuf> = done.into_iter().collect();
    let jobs = after::plan(&rules, &preview, &chosen, &moved);
    let services = after::Services {
        tmdb_key,
        tmdb_lang,
        mb: &state.mb,
        dg: &state.dg,
        ol: &state.ol,
        subs,
    };
    let mut report = after::run(
        &Gui(app.clone()),
        &state.history,
        history_id,
        renamed,
        &rules,
        jobs,
        &services,
    )
    .await;
    {
        let s = &mut *state.session.lock().unwrap();
        if !report.trashed.is_empty() {
            s.files
                .retain(|f| !report.trashed.iter().any(|t| f.path.starts_with(t)));
        }
        report.followed = followed(&mut s.files, &report.moved);
    }
    forget_tags_of(&state, report.retagged.clone());
    Ok(report)
}

/// The videos whose subtitles are searched (the flow the command line
/// runs too).
#[tauri::command]
fn subs_targets(
    files: Vec<renome_services::flow::subtitles::Identified>,
    langs: String,
) -> Vec<Value> {
    use renome_services::flow::subtitles as flow;
    flow::targets(&files)
        .into_iter()
        .map(|item| {
            let key = flow::key(&item, &langs);
            let mut v = serde_json::to_value(&item).unwrap_or_default();
            v["key"] = Value::from(key);
            v
        })
        .collect()
}

/// Whether some subtitle service can be asked with these keys.
#[tauri::command]
fn subs_ready(auth: subtitles::Auth) -> bool {
    auth.ready()
}

/// The languages whose subtitles the rules on download, if any (see
/// `flow::subtitles::wanted`, the command line's too).
#[tauri::command]
fn subs_wanted(rules: Vec<Value>) -> Option<String> {
    renome_services::flow::subtitles::wanted(&rules)
}

/// Searches subtitles for the videos, in the languages not there yet,
/// with the one picked per language.
#[tauri::command]
async fn subs_search(
    app: AppHandle,
    auth: subtitles::Auth,
    langs: String,
    items: Vec<subtitles::Item>,
) -> renome_services::flow::subtitles::Outcome {
    let stop = stop_signal(&app);
    renome_services::flow::subtitles::search(&Gui(app.clone()), auth, &langs, items, stop).await
}

/// What is chosen across the videos' subtitles, and whether those from
/// OpenSubtitles pass the day's quota (see `flow::subtitles::tally`).
#[tauri::command]
fn subs_tally(
    videos: Vec<renome_services::flow::subtitles::Standing>,
    langs: Vec<String>,
    remaining: Option<i64>,
) -> renome_services::flow::subtitles::Tally {
    renome_services::flow::subtitles::tally(&videos, &langs, remaining)
}

#[tauri::command]
async fn os_check(auth: subtitles::Auth) -> Result<Option<i64>, String> {
    subtitles::check_os(auth).await
}

#[tauri::command]
async fn subdl_check(key: String) -> Result<(), String> {
    subtitles::check_subdl(key).await
}

#[tauri::command]
async fn update_check(app: AppHandle) -> Result<Option<update::Found>, String> {
    update::check(&app).await
}

/// How this copy is updated: by itself, from the release page, or by the
/// system (Flatpak), whose copies do not look for versions.
#[tauri::command]
fn update_way() -> update::Way {
    update::way()
}

#[tauri::command]
async fn update_install(app: AppHandle) -> Result<(), String> {
    update::install(&app).await
}

#[tauri::command]
async fn server_check(server: mediaserver::Server) -> Result<(), String> {
    server.check().await
}

#[tauri::command]
async fn server_read(server: Value, key: String) -> Result<(Value, usize, usize), String> {
    // Pairs kept, paths here paired with themselves (see `mediaserver::read`,
    // the command line's `servers` too).
    mediaserver::read(server, &key).await
}

/// The rule sets with rules saved under a name (see `options::save_set`,
/// the command line's `sets` too); refused with `exists:<place>` when one
/// has that name and it is not to be replaced.
#[tauri::command]
fn sets_save(
    mut sets: Vec<Value>,
    name: String,
    steps: Vec<Value>,
    replace: bool,
) -> Result<Vec<Value>, String> {
    renome_services::options::save_set(&mut sets, &name, steps, replace)
        .map_err(|at| format!("exists:{at}"))?;
    Ok(sets)
}

/// The rule sets with one renamed; refused when another has that name.
#[tauri::command]
fn sets_rename(mut sets: Vec<Value>, at: usize, name: String) -> Result<Vec<Value>, String> {
    renome_services::options::rename_set(&mut sets, at, &name)
        .map_err(|at| format!("exists:{at}"))?;
    Ok(sets)
}

/// A new media server for the options.
#[tauri::command]
fn server_new() -> Value {
    mediaserver::new_server("jellyfin", "")
}

/// The servers' keys (one secret) read, or with one server's set.
#[tauri::command]
fn server_keys(raw: String) -> serde_json::Map<String, Value> {
    mediaserver::keys_of(&raw)
}

#[tauri::command]
fn server_key_with(raw: String, id: String, key: String) -> String {
    mediaserver::with_key(&raw, &id, &key)
}

/// Tells the servers in the options (`servers`, with their keys,
/// `serverKeys`) about the folders the renamed files left and went to
/// (the choice of servers is the command line's too).
#[tauri::command]
async fn server_tell(
    servers: Value,
    keys: String,
    done: Vec<renome_core::Operation>,
) -> Vec<renome_services::flow::after::Line> {
    // What is said of it (`flow::said::told`, the command line's too).
    let answers = mediaserver::tell_all(mediaserver::callable(&servers, &keys), &done).await;
    renome_services::flow::said::told(&answers)
}

#[tauri::command]
async fn subsource_check(key: String) -> Result<(), String> {
    subtitles::check_subsource(key).await
}

/// The list's files at these paths.
fn session_files(app: &AppHandle, paths: &HashSet<PathBuf>) -> Vec<FileEntry> {
    let state = app.state::<AppState>();
    let session = state.session.lock().unwrap();
    session
        .files
        .iter()
        .filter(|f| paths.contains(&f.path))
        .cloned()
        .collect()
}

/// The music rule's search (the flow the command line runs too): the
/// songs grouped in albums and looked up; `settled`: songs whose album
/// keeps its release.
#[tauri::command]
async fn music_search(
    app: AppHandle,
    items: Vec<renome_services::flow::Item>,
    settled: Vec<PathBuf>,
    from_name: bool,
    listen: bool,
) -> Result<Vec<musicbrainz::AlbumLookup>, String> {
    use renome_services::flow::music;
    let settled: HashSet<PathBuf> = settled.into_iter().collect();
    let wanted: HashSet<PathBuf> = music::targets(&items, listen, &settled)
        .into_iter()
        .collect();
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let files = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || session_files(&app, &wanted)
    })
    .await
    .map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    let services = musicbrainz::Services {
        mb: &state.mb,
        dg: &state.dg,
        acoustid: &state.acoustid,
        heard: &state.heard,
    };
    let mut found = music::search(
        &Gui(app.clone()),
        &services,
        &files.iter().collect::<Vec<_>>(),
        from_name,
        listen,
    )
    .await?;
    for a in found.iter_mut() {
        musicbrainz::thumbs(a.candidates.iter_mut().chain(a.choice.as_mut()));
    }
    Ok(found)
}

/// A search typed in the album review, on MusicBrainz or Discogs.
#[tauri::command]
async fn mb_search(
    app: AppHandle,
    query: renome_core::musicbrainz::AlbumQuery,
    discogs: Option<bool>,
) -> Result<Vec<renome_core::musicbrainz::Candidate>, String> {
    let state = app.state::<AppState>();
    let mut found =
        renome_services::flow::typed::album(&state.mb, &state.dg, &query, discogs == Some(true))
            .await?;
    renome_services::musicbrainz::thumbs(found.iter_mut());
    Ok(found)
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
    let files = session_files(&app, &paths.into_iter().collect());
    let state = app.state::<AppState>();
    let services = musicbrainz::Services {
        mb: &state.mb,
        dg: &state.dg,
        acoustid: &state.acoustid,
        heard: &state.heard,
    };
    renome_services::flow::music::choose(
        &services,
        &id,
        &files.iter().collect::<Vec<_>>(),
        from_name,
    )
    .await
}

/// Whether this system's web view can print books to PDF.
#[tauri::command]
fn pdf_print_available() -> bool {
    cfg!(target_os = "linux")
}

/// Prints the page `html` to the PDF file `pdf` with a hidden web view.
async fn print_to_pdf(
    app: &AppHandle,
    html: &Path,
    pdf: &Path,
    page: renome_core::bookconv::print::Page,
) -> Result<(), String> {
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
    let started = print_webview(&win, out, page, dtx.clone());
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
fn print_webview(
    win: &tauri::WebviewWindow,
    out: PathBuf,
    page: renome_core::bookconv::print::Page,
    done: PrintDone,
) -> Result<(), String> {
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
        if page == renome_core::bookconv::print::Page::A5 {
            setup.set_paper_size(&gtk::PaperSize::new(Some("iso_a5")));
        } else {
            setup.set_paper_size(&gtk::PaperSize::new_custom(
                "renome-screen",
                "Reader",
                page.width,
                page.height,
                gtk::Unit::Mm,
            ));
        }
        setup.set_top_margin(page.top, gtk::Unit::Mm);
        setup.set_bottom_margin(page.bottom, gtk::Unit::Mm);
        setup.set_left_margin(page.side, gtk::Unit::Mm);
        setup.set_right_margin(page.side, gtk::Unit::Mm);
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
fn print_webview(
    _: &tauri::WebviewWindow,
    _: PathBuf,
    _: renome_core::bookconv::print::Page,
    _: PrintDone,
) -> Result<(), String> {
    Err("pdfPrint".into())
}

/// The book rule's search (the flow the command line runs too): books
/// and audiobooks not `settled`, sure ones picked.
#[tauri::command]
async fn books_search(
    app: AppHandle,
    lang: String,
    items: Vec<renome_services::flow::Item>,
    settled: Vec<PathBuf>,
    audiobooks: bool,
) -> Result<renome_services::flow::books::Outcome, String> {
    let ol = &app.state::<AppState>().ol;
    let settled: HashSet<PathBuf> = settled.into_iter().collect();
    let mut out = renome_services::flow::books::search(
        &Gui(app.clone()),
        ol,
        &lang,
        &items,
        &settled,
        audiobooks,
    )
    .await?;
    for f in out.found.iter_mut() {
        renome_services::openlibrary::thumbs(f.candidates.iter_mut().chain(f.choice.as_mut()));
    }
    Ok(out)
}

/// Each file's book as the rule names it, from the books chosen.
#[tauri::command]
fn book_matches(
    chosen: Vec<renome_services::flow::books::Chosen>,
    parts: HashMap<PathBuf, renome_services::flow::books::Part>,
    audiobooks: bool,
) -> HashMap<PathBuf, renome_core::books::BookMatch> {
    renome_services::flow::books::matches(&chosen, &parts, audiobooks)
}

/// A search typed in the book review.
#[tauri::command]
async fn book_search(
    app: AppHandle,
    lang: String,
    title: String,
    author: String,
) -> Result<Vec<renome_core::books::Candidate>, String> {
    let ol = &app.state::<AppState>().ol;
    let mut found = renome_services::flow::typed::book(ol, &lang, &title, &author).await?;
    renome_services::openlibrary::thumbs(found.iter_mut());
    Ok(found)
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
/// first (see `flow::typed::comic`). `key` is Comic Vine's key or
/// Metron's token.
// The command's arguments are what the window sends, each by name.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn comic_search(
    app: AppHandle,
    source: String,
    key: String,
    query: String,
    year: String,
    publisher: String,
    lang: String,
    skip: Option<String>,
    highest: f64,
) -> Result<Vec<renome_core::comics::Volume>, String> {
    let state = app.state::<AppState>();
    // As the command line's chooser searches.
    renome_services::flow::typed::comic(
        &sources(&state),
        &source,
        &key,
        &query,
        &year,
        &publisher,
        &lang,
        skip.as_deref().unwrap_or_default(),
        highest,
    )
    .await
}

/// The comic rule's search (the flow the command line runs too): a
/// series it is not sure of is asked with `flow-ask`.
#[tauri::command]
async fn comics_search(
    app: AppHandle,
    params: renome_services::flow::comics::Params,
    items: Vec<renome_services::flow::Item>,
    picked: HashMap<String, renome_core::comics::Volume>,
) -> Result<renome_services::flow::comics::Outcome, String> {
    let state = app.state::<AppState>();
    renome_services::flow::comics::search(
        &Gui(app.clone()),
        &sources(&state),
        &params,
        &items,
        &HashSet::new(),
        picked,
    )
    .await
}

/// Each comic as the rule names it, with its page in the catalogue.
#[tauri::command]
fn comic_named(
    files: Vec<renome_services::flow::comics::Settled>,
    volumes: HashMap<String, renome_services::flow::comics::Listed>,
) -> HashMap<PathBuf, renome_services::flow::comics::Named> {
    renome_services::flow::comics::named(&files, &volumes)
}

/// Checks a Metron token.
#[tauri::command]
async fn metron_check(app: AppHandle, token: String) -> Result<(), String> {
    app.state::<AppState>().mt.check(&token).await
}

#[tauri::command]
fn history(state: State<AppState>) -> Vec<Value> {
    // Each with what is said of it (the command line's `history` too).
    state
        .history
        .list()
        .iter()
        .map(|e| {
            let mut v = serde_json::to_value(e).unwrap_or_default();
            v["said"] =
                serde_json::to_value(renome_services::flow::said::entry(e)).unwrap_or_default();
            v
        })
        .collect()
}

#[tauri::command]
async fn undo(app: AppHandle, id: String) -> Result<Applied, String> {
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
        // Not while the command line renames.
        let _lock = renome_services::RenameLock::take(|| ()).ok();
        let mut report = state.history.undo_with(&id, &hooks, &tags)?;
        let s = &mut *state.session.lock().unwrap();
        refresh_renamed(&mut s.files, &mut report);
        // Songs may have their old tags back: read again.
        for f in s.files.iter_mut() {
            if renome_core::music::is_audio(&f.ext) {
                f.forget_tags();
            }
        }
        s.undo.clear();
        let said = renome_services::flow::said::undone(&report);
        Ok(Applied::new(report, said))
    })
    .await?
}

/// Puts back renames a stopped program left halfway (see
/// [`History::recover`]); called once at start.
#[tauri::command]
async fn recover(app: AppHandle) -> Result<Option<renome_services::flow::said::Recovery>, String> {
    in_background(app, |state| {
        renome_services::flow::said::recovered(&state.history.recover())
    })
    .await
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
fn save_preset(path: PathBuf, mut steps: Vec<serde_json::Value>) -> Result<(), String> {
    // As the window sends them: options only the window reads (a reader
    // model, comic options the core takes elsewhere) are kept.
    renome_core::catalog::normalize_steps(&mut steps);
    serde_json::from_value::<Vec<Step>>(serde_json::Value::Array(steps.clone()))
        .map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&steps).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// Writes a text file the user chose (the exported list).
/// Saves rows as a CSV file (see `text::csv`: the command line's too).
#[tauri::command]
fn save_csv(path: PathBuf, rows: Vec<Vec<String>>, lang: String) -> Result<(), String> {
    std::fs::write(path, renome_core::text::csv(&rows, &lang)).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_preset(
    path: PathBuf,
    lang: String,
) -> Result<(Vec<serde_json::Value>, Vec<String>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("invalidPreset: {e}"))?;
    // A list of steps, a set or a list of sets (the command line reads the
    // same), made ready as a saved set.
    let Value::Array(steps) = renome_core::catalog::steps_of(v) else {
        return Err("invalidPreset".into());
    };
    let (steps, dropped) = renome_core::catalog::complete_steps(steps, &lang, true);
    serde_json::from_value::<Vec<Step>>(serde_json::Value::Array(steps.clone()))
        .map_err(|e| format!("invalidPreset: {e}"))?;
    Ok((steps, dropped))
}

/// The settings file is read and written whole by the settings and the
/// secrets apart: one at a time.
use renome_services::SETTINGS_FILE;

/// Saved settings; `None` on first run. The format is defined by the UI.
/// Secrets come apart ([`load_secrets`]), unless the file still has them.
#[tauri::command]
fn load_settings(fallback_lang: String) -> Result<Value, String> {
    let _file = SETTINGS_FILE.lock().unwrap();
    match read_settings_file() {
        Some(saved) => Ok(renome_services::options::whole(Some(saved))),
        // The first run: the system's language, saved.
        None => {
            let mut first = renome_services::options::whole(None);
            let mut sys = renome_services::system_language();
            if sys.is_empty() {
                // The webview's language where the system says none.
                sys = fallback_lang
                    .chars()
                    .take(2)
                    .collect::<String>()
                    .to_lowercase();
            }
            first["lang"] = Value::from(match sys.as_str() {
                "pt" | "es" => sys.as_str(),
                _ => "en",
            });
            write_settings_file(&first)?;
            Ok(first)
        }
    }
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
            if let Some(Value::Array(rules)) = obj.get_mut("rules") {
                renome_core::catalog::normalize_steps(rules);
            }
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
    tauri::async_runtime::spawn_blocking(renome_services::load_secrets)
        .await
        .unwrap_or_else(|_| serde_json::json!({ "status": "refused" }))
}

/// Saves the secrets in the system's password store; in the settings
/// file when the store does not take them (or was not read: then what it
/// holds stays as it is). Whether the store took them.
#[tauri::command]
async fn save_secrets(secrets: serde_json::Map<String, Value>) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || renome_services::save_secrets(secrets))
        .await
        .map_err(|e| e.to_string())?
}

/// System language (`pt_BR.UTF-8` → `pt`), from the locale variables.
#[tauri::command]
fn system_language() -> String {
    renome_services::system_language()
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
    std::thread::spawn(|| {
        drop_old_cache();
        ANSWERS.purge();
    });

    let history_dir = renome_services::history_dir();

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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|_| {
            #[cfg(target_os = "linux")]
            sync_titlebar_buttons();
            Ok(())
        })
        .manage(AppState {
            session: Mutex::new(session),
            history: History::new(history_dir),
            hashes: Mutex::default(),
            covers: renome_services::covers::Covers::default(),
            pictures: Mutex::new(renome_core::similar::PictureCache::in_file(
                cache_root().join("pictures"),
            )),
            identical_run: AtomicU64::new(0),
            busy_disk: Mutex::new(()),
            stop_run: AtomicU64::new(0),
            asks: Mutex::default(),
            ask_seq: AtomicU64::new(0),
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
            shown: Mutex::default(),
            missing: Mutex::default(),
            renamed: Mutex::default(),
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
            picture_picks,
            own_cover,
            cover_distances,
            series_by_cover,
            issue_order,
            issue_covers,
            remote_image,
            trash_files,
            tmdb_collections,
            clear_files,
            move_files,
            sort_files,
            file_details,
            export_rows,
            export_missing,
            discogs_token,
            google_books_key,
            google_books_check,
            discogs_check,
            preview,
            find_identical,
            tmdb_check,
            movie_search,
            series_search,
            series_named,
            movie_named,
            episode_codes,
            rule_inert,
            subs_tally,
            gcd_few,
            picture_places,
            flow_answer,
            tmdb_search,
            tv_search,
            subs_search,
            subs_targets,
            subs_wanted,
            subs_ready,
            os_check,
            subdl_check,
            subsource_check,
            server_check,
            server_read,
            server_new,
            sets_save,
            sets_rename,
            server_keys,
            server_key_with,
            server_tell,
            update_check,
            update_install,
            update_way,
            music_search,
            mb_search,
            mb_choose,
            after_counts,
            rename_ask,
            rule_menu,
            manual_order,
            rule_reader,
            edit_details,
            edit_dates,
            edit_start,
            rule_table,
            rule_blocks,
            complete_steps,
            album_on_discogs,
            bigger_cover,
            episodes_typed,
            same_answer,
            columns_for,
            catalog_langs,
            missing_items,
            after_rename,
            books_search,
            book_matches,
            book_search,
            comic_check,
            comic_search,
            anime_search,
            comics_search,
            comic_named,
            metron_check,
            gcd_account,
            gcd_check,
            save_csv,
            tmdb_images,
            recover,
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
