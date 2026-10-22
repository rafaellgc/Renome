use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use renome_core::scan::Skipped;
use renome_core::{
    ApplyReport, FileEntry, History, HistoryEntry, Preview, ScanOptions, SortKey, Step, View,
};
use serde::Serialize;
use serde_json::Value;
use tauri::State;

/// The session's file list. It lives on the Rust side so the whole list
/// is not sent over on every preview update.
#[derive(Default)]
struct Session {
    files: Vec<FileEntry>,
    /// Earlier states of the list and of the hand-edited names, for Ctrl+Z.
    undo: Vec<(Vec<FileEntry>, Overrides)>,
}

/// How many list edits can be undone.
const UNDO_LIMIT: usize = 100;

struct AppState {
    session: Mutex<Session>,
    history: History,
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
fn preview(state: State<AppState>, steps: Vec<Step>, overrides: Overrides, view: View) -> Preview {
    renome_core::preview(
        &state.session.lock().unwrap().files,
        &steps,
        &overrides,
        &view,
    )
}

/// Updates the session entries after renaming. Renames run in order, and a
/// renamed folder also moves the entries inside it.
fn refresh_renamed(files: &mut [FileEntry], report: &ApplyReport) {
    if report.done.is_empty() {
        return;
    }
    for f in files.iter_mut() {
        let mut path = f.path.clone();
        for op in &report.done {
            if path == op.from {
                path = op.to.clone();
            } else if let Ok(rest) = path.strip_prefix(&op.from) {
                path = op.to.join(rest);
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
    view: View,
) -> ApplyReport {
    let mut s = state.session.lock().unwrap();
    // Recompute the preview here to apply exactly what is visible and valid right now.
    let ops = renome_core::preview(&s.files, &steps, &overrides, &view).operations();
    let report = state.history.apply(&ops);
    refresh_renamed(&mut s.files, &report);
    // Earlier states point to the old names.
    s.undo.clear();
    report
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
        .setup(|_| {
            #[cfg(target_os = "linux")]
            sync_titlebar_buttons();
            Ok(())
        })
        .manage(AppState {
            session: Mutex::new(session),
            history: History::new(history_dir),
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
