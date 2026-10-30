use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

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
mod comicvine;
mod gcd;
mod mangadex;
mod musicbrainz;
mod openlibrary;
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
}

/// How many list edits can be undone.
const UNDO_LIMIT: usize = 100;

struct AppState {
    session: Mutex<Session>,
    history: History,
    hashes: Mutex<HashCache>,
    /// Bumped by each search for identical files; an older search that sees
    /// a newer number stops.
    identical_run: AtomicU64,
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
}

type Overrides = HashMap<PathBuf, String>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddResult {
    /// Paths actually added (so the UI can select them).
    added: Vec<PathBuf>,
    skipped: Vec<Skipped>,
}

/// Saves the current state before the UI changes the list.
#[tauri::command]
fn checkpoint(state: State<AppState>, overrides: Overrides) {
    let s = &mut *state.session.lock().unwrap();
    if s.undo.len() == UNDO_LIMIT {
        s.undo.remove(0);
    }
    s.undo.push((s.files.clone(), overrides));
}

/// Drops the last checkpoint (the edit changed nothing).
#[tauri::command]
fn drop_checkpoint(state: State<AppState>) {
    state.session.lock().unwrap().undo.pop();
}

/// Restores the list to the last checkpoint and returns its hand-edited
/// names; `None` when there is nothing to undo.
#[tauri::command]
fn undo_list(state: State<AppState>) -> Option<Overrides> {
    let s = &mut *state.session.lock().unwrap();
    let (files, overrides) = s.undo.pop()?;
    s.files = files;
    Some(overrides)
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
fn remove_files(state: State<AppState>, paths: Vec<PathBuf>) {
    let remove: HashSet<PathBuf> = paths.into_iter().collect();
    state
        .session
        .lock()
        .unwrap()
        .files
        .retain(|f| !remove.contains(&f.path));
}

#[tauri::command]
fn clear_files(state: State<AppState>) {
    state.session.lock().unwrap().files.clear();
}

/// Moves the selected files one position up (`-1`) or down (`1`).
#[tauri::command]
fn move_files(state: State<AppState>, paths: Vec<PathBuf>, delta: i32) {
    let selected: HashSet<PathBuf> = paths.into_iter().collect();
    let files = &mut state.session.lock().unwrap().files;
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

#[tauri::command]
fn sort_files(state: State<AppState>, key: SortKey, descending: bool) {
    renome_core::sort_files(&mut state.session.lock().unwrap().files, key, descending);
}

#[tauri::command]
async fn preview(
    app: AppHandle,
    steps: Vec<Step>,
    overrides: Overrides,
    mut view: View,
) -> Result<Preview, String> {
    in_background(app, move |state| {
        let s = state.session.lock().unwrap();
        view.identical = s.identical.clone();
        view.protected = protected_dirs();
        renome_core::preview(&s.files, &steps, &overrides, &view)
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

/// Searches the list for files with identical content, in the background.
/// Returns the groups, or `None` if a newer search replaced this one.
/// Emits `identical-progress` while reading whole files.
#[tauri::command]
async fn find_identical(app: AppHandle) -> Result<Option<Vec<Vec<PathBuf>>>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let run = state.identical_run.fetch_add(1, Ordering::SeqCst) + 1;
        let files = state.session.lock().unwrap().files.clone();
        let stale = || state.identical_run.load(Ordering::SeqCst) != run;
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
        state.session.lock().unwrap().identical = groups
            .iter()
            .enumerate()
            .flat_map(|(i, g)| g.iter().map(move |p| (p.clone(), i)))
            .collect();
        Some(groups)
    })
    .await
    .map_err(|e| e.to_string())
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
    in_background(app, move |state| {
        let mut s = state.session.lock().unwrap();
        view.identical = s.identical.clone();
        view.protected = protected_dirs();
        // Recompute the preview here to apply exactly what is visible and valid right now.
        let ops = renome_core::preview(&s.files, &steps, &overrides, &view).operations();
        let report = state.history.apply(&ops);
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

/// A search typed by the user in the review window.
#[tauri::command]
async fn tmdb_search(
    key: String,
    lang: String,
    title: String,
    year: Option<u16>,
) -> Result<Vec<renome_core::tmdb::Candidate>, String> {
    let guess = renome_core::tmdb::Guess {
        title,
        year,
        id: None,
    };
    Ok(tmdb::Client::new(&key, &lang).search(&guess).await?.0)
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
    tmdb::search_show(&tmdb::Client::new(&key, &lang), &query, &items).await
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
        acoustid: &state.acoustid,
        heard: &state.heard,
    };
    Ok(musicbrainz::lookup(&app, &services, albums, unnamed, listen).await)
}

/// A search typed in the album review.
#[tauri::command]
async fn mb_search(
    app: AppHandle,
    query: renome_core::musicbrainz::AlbumQuery,
) -> Result<Vec<renome_core::musicbrainz::Candidate>, String> {
    app.state::<AppState>().mb.search(&query).await
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
    musicbrainz::choose(&app.state::<AppState>().mb, &id, &songs).await
}

/// Saves the albums' covers and records them like `write_sidecars`.
#[tauri::command]
async fn write_covers(
    app: AppHandle,
    history_id: Option<String>,
    items: Vec<musicbrainz::CoverItem>,
) -> Result<tmdb::SidecarReport, String> {
    let mut report = musicbrainz::covers(&app, &app.state::<AppState>().mb, items).await;
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
        let image = if cover {
            if !covers.contains_key(&job.song.release) {
                let mut found = state.mb.front_cover(&job.song.release).await.ok().flatten();
                if found.is_none() {
                    found = job.path.parent().and_then(folder_cover);
                }
                covers.insert(job.song.release.clone(), found);
            }
            covers[&job.song.release].clone()
        } else {
            None
        };
        if !full.contains_key(&job.song.release) {
            let r = state.mb.release_full(&job.song.release).await.ok();
            full.insert(job.song.release.clone(), r);
        }
        let mut fields = tagwrite::fields_of(&job.song);
        if let Some(r) = &full[&job.song.release] {
            fields.extend(tagwrite::details(r, &job.song.track_id));
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
    let guess = renome_core::books::Guess {
        title,
        author,
        from_file: true,
        ..Default::default()
    };
    Ok(app.state::<AppState>().ol.find(&guess, &lang).await?.0)
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

/// Signs in to the Grand Comics Database for this session (kept in memory
/// only), checking the account.
#[tauri::command]
async fn gcd_sign_in(app: AppHandle, email: String, password: String) -> Result<(), String> {
    let gcd = &app.state::<AppState>().gcd;
    gcd.sign_in(email, password);
    let checked = gcd.check().await;
    if checked.is_err() {
        gcd.sign_out();
    }
    checked
}

#[tauri::command]
fn gcd_sign_out(app: AppHandle) {
    app.state::<AppState>().gcd.sign_out();
}

/// Series with a name in a catalogue (`gcd`, `comicvine`, `mangadex`),
/// from a year and by a publisher when given (MangaDex has no
/// publishers), best first.
#[tauri::command]
async fn comic_search(
    app: AppHandle,
    source: String,
    key: String,
    query: String,
    year: Option<u16>,
    publisher: String,
    lang: String,
) -> Result<Vec<renome_core::comics::Volume>, String> {
    let state = app.state::<AppState>();
    match source.as_str() {
        "comicvine" => state.cv.search(&key, &query, year, &publisher, &lang).await,
        "mangadex" => state.md.search(&query, year, &lang).await,
        _ => state.gcd.search(&query, year, &publisher, &lang).await,
    }
}

/// A series and its issues, and the issue each comic is (by its number).
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComicIssues {
    volume: renome_core::comics::Volume,
    issues: Vec<renome_core::comics::Issue>,
    /// The issue of each comic asked about, when its number says it.
    found: Vec<Option<String>>,
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
    in_background(app, move |state| {
        let report = state.history.undo(&id)?;
        let s = &mut *state.session.lock().unwrap();
        refresh_renamed(&mut s.files, &report);
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

/// Saved settings; `None` on first run. The format is defined by the UI.
#[tauri::command]
fn load_settings() -> Option<Value> {
    let text = std::fs::read_to_string(settings_path()).ok()?;
    serde_json::from_str(&text).ok()
}

#[tauri::command]
fn save_settings(settings: Value) -> Result<(), String> {
    let path = settings_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
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
    let history_dir = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("renome")
        .join("history");

    // Paths given on the command line go into the initial list.
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let mut session = Session::default();
    if !args.is_empty() {
        let include_hidden = load_settings()
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
            identical_run: AtomicU64::new(0),
            stop_run: AtomicU64::new(0),
            mb: musicbrainz::Client::default(),
            acoustid: acoustid::Client::default(),
            heard: Mutex::new(HashMap::new()),
            ol: openlibrary::Client::default(),
            cv: comicvine::Client::default(),
            gcd: gcd::Client::default(),
            md: mangadex::Client::default(),
        })
        .invoke_handler(tauri::generate_handler![
            checkpoint,
            drop_checkpoint,
            undo_list,
            add_paths,
            remove_files,
            stop_search,
            clear_files,
            move_files,
            sort_files,
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
            mb_lookup,
            mb_search,
            mb_choose,
            write_covers,
            write_playlists,
            write_tags,
            playlist_dirs,
            book_lookup,
            book_search,
            comic_guesses,
            comic_check,
            comic_search,
            comic_issues,
            comic_volume_of,
            gcd_sign_in,
            gcd_sign_out,
            save_text,
            tmdb_images,
            recover,
            write_book_sidecars,
            apply,
            history,
            undo,
            delete_history,
            clear_history,
            save_preset,
            load_preset,
            load_settings,
            save_settings,
            system_language,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Renome");
}
