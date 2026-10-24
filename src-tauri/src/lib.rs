use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use renome_core::music::AudioTags;
use renome_core::rules::song_tags;
use renome_core::scan::Skipped;
use renome_core::{
    ApplyReport, FileEntry, HashCache, History, HistoryEntry, Operation, Preview, ScanOptions,
    SortKey, Step, View,
};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

mod musicbrainz;
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
    /// MusicBrainz, paced for all lookups together.
    mb: musicbrainz::Client,
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

#[tauri::command]
fn add_paths(state: State<AppState>, paths: Vec<PathBuf>, options: ScanOptions) -> AddResult {
    let mut s = state.session.lock().unwrap();
    let known: HashSet<PathBuf> = s.files.iter().map(|f| f.path.clone()).collect();
    let result = renome_core::scan(&paths, &options, &known);
    let added = result.files.iter().map(|f| f.path.clone()).collect();
    s.files.extend(result.files);
    AddResult {
        added,
        skipped: result.skipped,
    }
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
fn preview(
    state: State<AppState>,
    steps: Vec<Step>,
    overrides: Overrides,
    mut view: View,
) -> Preview {
    let s = state.session.lock().unwrap();
    view.identical = s.identical.clone();
    renome_core::preview(&s.files, &steps, &overrides, &view)
}

#[derive(Clone, Serialize)]
struct IdenticalProgress {
    done: u64,
    total: u64,
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
fn apply(
    state: State<AppState>,
    steps: Vec<Step>,
    overrides: Overrides,
    mut view: View,
) -> ApplyReport {
    let mut s = state.session.lock().unwrap();
    view.identical = s.identical.clone();
    // Recompute the preview here to apply exactly what is visible and valid right now.
    let ops = renome_core::preview(&s.files, &steps, &overrides, &view).operations();
    let report = state.history.apply(&ops);
    refresh_renamed(&mut s.files, &report);
    // Earlier states point to the old names.
    s.undo.clear();
    report
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
) -> Result<tmdb::TvLookup, String> {
    tmdb::lookup_episodes(&tmdb::Client::new(&key, &lang), id, items).await
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

/// The tags of the given songs in the list (with the name filled in, as
/// the music rule does).
fn songs_of(app: &AppHandle, paths: &[PathBuf], from_name: bool) -> Vec<(PathBuf, AudioTags)> {
    let wanted: HashSet<&PathBuf> = paths.iter().collect();
    let files: Vec<FileEntry> = app
        .state::<AppState>()
        .session
        .lock()
        .unwrap()
        .files
        .iter()
        .filter(|f| wanted.contains(&f.path))
        .cloned()
        .collect();
    files
        .iter()
        .filter_map(|f| Some((f.path.clone(), song_tags(f, from_name)?)))
        .collect()
}

/// Groups the songs in albums and looks each one up on MusicBrainz.
#[tauri::command]
async fn mb_lookup(
    app: AppHandle,
    paths: Vec<PathBuf>,
    from_name: bool,
) -> Result<Vec<musicbrainz::AlbumLookup>, String> {
    let songs = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || songs_of(&app, &paths, from_name)
    })
    .await
    .map_err(|e| e.to_string())?;
    let albums = musicbrainz::albums(&songs);
    Ok(musicbrainz::lookup(&app, &app.state::<AppState>().mb, albums).await)
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
    let songs = songs_of(&app, &paths, from_name);
    let songs: Vec<_> = musicbrainz::albums(&songs).concat();
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

#[tauri::command]
fn history(state: State<AppState>) -> Vec<HistoryEntry> {
    state.history.list()
}

#[tauri::command]
fn undo(state: State<AppState>, id: String) -> Result<ApplyReport, String> {
    let report = state.history.undo(&id)?;
    let s = &mut *state.session.lock().unwrap();
    refresh_renamed(&mut s.files, &report);
    s.undo.clear();
    Ok(report)
}

#[tauri::command]
fn delete_history(state: State<AppState>, id: String) -> Result<(), String> {
    state.history.delete(&id)
}

#[tauri::command]
fn save_preset(path: PathBuf, steps: Vec<Step>) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&steps).map_err(|e| e.to_string())?;
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
            mb: musicbrainz::Client::default(),
        })
        .invoke_handler(tauri::generate_handler![
            checkpoint,
            drop_checkpoint,
            undo_list,
            add_paths,
            remove_files,
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
            write_sidecars,
            write_tv_sidecars,
            mb_lookup,
            mb_search,
            mb_choose,
            write_covers,
            apply,
            history,
            undo,
            delete_history,
            save_preset,
            load_preset,
            load_settings,
            save_settings,
            system_language,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Renome");
}
