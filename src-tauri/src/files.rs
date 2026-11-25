//! The window's commands: files.

use super::*;

/// The bytes the kept answers take.
#[tauri::command]
pub(crate) async fn cache_size() -> (u64, String) {
    let n = tauri::async_runtime::spawn_blocking(|| ANSWERS.size())
        .await
        .unwrap_or(0);
    (n, renome_core::text::size(n))
}

/// Forgets the kept answers; returns the bytes freed.
#[tauri::command]
pub(crate) async fn clear_cache() -> u64 {
    tauri::async_runtime::spawn_blocking(|| ANSWERS.clear())
        .await
        .unwrap_or(0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AddResult {
    /// Paths actually added (so the UI can select them).
    pub(crate) added: Vec<PathBuf>,
    pub(crate) skipped: Vec<Skipped>,
}

/// Saves the current state before the UI changes the list.
#[tauri::command]
pub(crate) async fn checkpoint(app: AppHandle, overrides: Overrides) -> Result<(), String> {
    in_background(app, move |state| {
        let s = &mut *state.session.locked();
        if s.undo.len() == UNDO_LIMIT {
            s.undo.remove(0);
        }
        s.undo.push((s.files.clone(), overrides));
    })
    .await
}

/// Drops the last checkpoint (the edit changed nothing).
#[tauri::command]
pub(crate) async fn drop_checkpoint(app: AppHandle) -> Result<(), String> {
    in_background(app, |state| {
        state.session.locked().undo.pop();
    })
    .await
}

/// Restores the list to the last checkpoint and returns its hand-edited
/// names; `None` when there is nothing to undo.
#[tauri::command]
pub(crate) async fn undo_list(app: AppHandle) -> Result<Option<Overrides>, String> {
    in_background(app, |state| {
        let s = &mut *state.session.locked();
        let (files, overrides) = s.undo.pop()?;
        s.files = files;
        Some(overrides)
    })
    .await
}

/// Runs `work` away from the window's thread (commands without `async`
/// run on it and freeze the window while they last).
pub(crate) async fn in_background<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&AppState) -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || work(&app.state::<AppState>()))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn add_paths(
    app: AppHandle,
    paths: Vec<PathBuf>,
    options: ScanOptions,
) -> Result<AddResult, String> {
    in_background(app, move |state| {
        let known: HashSet<PathBuf> = {
            let s = state.session.locked();
            s.files.iter().map(|f| f.path.clone()).collect()
        };
        // Scanned without holding the list: a big folder takes a while.
        let result = renome_core::scan(&paths, &options, &known);
        let added = result.files.iter().map(|f| f.path.clone()).collect();
        state.session.locked().files.extend(result.files);
        AddResult {
            added,
            skipped: result.skipped,
        }
    })
    .await
}

#[tauri::command]
pub(crate) async fn remove_files(app: AppHandle, paths: Vec<PathBuf>) -> Result<(), String> {
    in_background(app, move |state| {
        let remove: HashSet<PathBuf> = paths.into_iter().collect();
        state
            .session
            .locked()
            .files
            .retain(|f| !remove.contains(&f.path));
    })
    .await
}

#[tauri::command]
pub(crate) async fn clear_files(app: AppHandle) -> Result<(), String> {
    in_background(app, |state| state.session.locked().files.clear()).await
}

/// Moves the selected files one position up (`-1`) or down (`1`).
#[tauri::command]
pub(crate) async fn move_files(
    app: AppHandle,
    paths: Vec<PathBuf>,
    delta: i32,
) -> Result<(), String> {
    in_background(app, move |state| {
        move_in(&mut state.session.locked().files, &paths, delta)
    })
    .await
}

pub(crate) fn move_in(files: &mut [FileEntry], paths: &[PathBuf], delta: i32) {
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
pub(crate) async fn sort_files(
    app: AppHandle,
    key: SortKey,
    descending: bool,
) -> Result<(), String> {
    in_background(app, move |state| {
        renome_core::sort_files(&mut state.session.locked().files, key, descending);
    })
    .await
}

/// What the optional columns show about these files (read once each).
#[tauri::command]
pub(crate) async fn file_details(
    app: AppHandle,
    paths: Vec<PathBuf>,
    lang: String,
) -> Result<HashMap<PathBuf, HashMap<&'static str, String>>, String> {
    // As the list shows them (see `Details::text`: the CSV saves the same).
    in_background(app, move |state| {
        let want: HashSet<PathBuf> = paths.into_iter().collect();
        let s = state.session.locked();
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
pub(crate) async fn export_rows(
    app: AppHandle,
    columns: Vec<String>,
    lang: String,
) -> Result<Vec<Vec<renome_services::flow::export::Cell>>, String> {
    in_background(app, move |state| {
        let preview = state.shown.locked().clone().unwrap_or_default();
        let cols: Vec<&str> = columns.iter().map(String::as_str).collect();
        let details: HashMap<PathBuf, renome_core::details::Details> = if cols.is_empty() {
            HashMap::new()
        } else {
            let s = state.session.locked();
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
pub(crate) async fn export_missing(
    app: AppHandle,
) -> Result<renome_services::flow::export::MissingCsv, String> {
    in_background(app, move |state| {
        let missing = state.missing.locked().clone();
        let preview = state.shown.locked().clone().unwrap_or_default();
        let dirs: HashMap<&PathBuf, &PathBuf> =
            preview.items.iter().map(|i| (&i.path, &i.dir)).collect();
        renome_services::flow::export::missing(&missing, &|p| {
            dirs.get(&p.to_path_buf()).map(|d| (*d).clone())
        })
    })
    .await
}

#[derive(Serialize)]
pub(crate) struct Trashed {
    pub(crate) trashed: Vec<PathBuf>,
    /// Path and why.
    pub(crate) failed: Vec<(PathBuf, String)>,
}

/// Sends files to the system's trash and takes them out of the list (and
/// of the earlier states Ctrl+Z goes back to).
#[tauri::command]
pub(crate) async fn trash_files(app: AppHandle, paths: Vec<PathBuf>) -> Result<Trashed, String> {
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
        let mut s = state.session.locked();
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

/// The list's files at these paths.
pub(crate) fn session_files(app: &AppHandle, paths: &HashSet<PathBuf>) -> Vec<FileEntry> {
    let state = app.state::<AppState>();
    let session = state.session.locked();
    session
        .files
        .iter()
        .filter(|f| paths.contains(&f.path))
        .cloned()
        .collect()
}
