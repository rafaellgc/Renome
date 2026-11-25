//! The window's commands: rename.

use super::*;

/// The rules as the window sends them, read as the core reads them (see
/// `catalog::normalize_steps`, the command line's too); what is asked of
/// the release's adverts, from them.
pub(crate) fn rule_steps(mut steps: Vec<Value>, view: &mut View) -> Result<Vec<Step>, String> {
    renome_core::catalog::normalize_steps(&mut steps);
    view.trash_ads = renome_core::catalog::trash_ads(&steps);
    serde_json::from_value(Value::Array(steps)).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn preview(
    app: AppHandle,
    steps: Vec<Value>,
    overrides: Overrides,
    mut view: View,
    warn_windows: bool,
) -> Result<Shown, String> {
    let steps = rule_steps(steps, &mut view)?;
    let emitter = app.clone();
    in_background(app, move |state| {
        let _disk = state.busy_disk.locked();
        // Read on a copy of the list: the list stays free for the window
        // (the first reading of thousands of photos takes a minute).
        let files = {
            let s = state.session.locked();
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
                let mut last = last.locked();
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
        *state.shown.locked() = Some(preview.clone());
        // What was read stays known.
        let read: HashMap<&PathBuf, &FileEntry> = files.iter().map(|f| (&f.path, f)).collect();
        for f in state.session.locked().files.iter_mut() {
            if let Some(other) = read.get(&f.path) {
                f.keep_read(other);
            }
        }
        Shown::of(preview, &steps, warn_windows)
    })
    .await
}

/// Updates the session entries after renaming (see `apply::Follow`: what
/// the window found per file follows them the same way), and tells the
/// window where each one went (`moved`).
pub(crate) fn refresh_renamed(files: &mut [FileEntry], report: &mut ApplyReport) {
    report.moved = followed(files, &report.done);
}

/// [`refresh_renamed`] for renames done: where each entry went.
pub(crate) fn followed(
    files: &mut [FileEntry],
    done: &[renome_core::Operation],
) -> Vec<(PathBuf, PathBuf)> {
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
pub(crate) struct Applied {
    #[serde(flatten)]
    pub(crate) report: ApplyReport,
    pub(crate) said: Vec<renome_services::flow::after::Line>,
    /// Each file that failed, as said (`flow::said::failed`).
    pub(crate) failed: Vec<renome_services::flow::after::Line>,
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
pub(crate) async fn apply(
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
        let _disk = state.busy_disk.locked();
        // Not while the command line renames.
        let _lock = renome_services::RenameLock::take(|| ()).ok();
        let mut s = state.session.locked();
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
        *state.renamed.locked() = Some(plan);
        Applied::new(report, said)
    })
    .await
}

/// What renaming the list as shown would write besides the renames (the
/// files already named right only; subtitles for every file), for the
/// question before renaming and the Rename button.
#[tauri::command]
pub(crate) fn after_counts(
    app: AppHandle,
    rules: Vec<Value>,
    chosen: renome_services::flow::after::Chosen,
) -> renome_services::flow::after::Counts {
    let state = app.state::<AppState>();
    let shown = state.shown.locked();
    match shown.as_ref() {
        Some(p) => renome_services::flow::after::counts(&rules, p, &chosen),
        None => Default::default(),
    }
}

/// What the question before renaming says (the shared lines, the
/// command line's too: `flow::after::ask`), with the work besides the
/// renames, on the list as shown.
#[tauri::command]
pub(crate) fn rename_ask(
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
    let shown = state.shown.locked();
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
pub(crate) async fn after_rename(
    app: AppHandle,
    rules: Vec<Value>,
    chosen: renome_services::flow::after::Chosen,
    done: Vec<(PathBuf, PathBuf)>,
    history_id: Option<String>,
    tmdb_key: String,
    tmdb_lang: String,
    subs: subtitles::Auth,
) -> Result<Value, String> {
    use renome_services::flow::after;
    let state = app.state::<AppState>();
    // The list as renamed (as shown, when nothing was).
    let renamed = !done.is_empty();
    let preview = renamed
        .then(|| state.renamed.locked().take())
        .flatten()
        .or_else(|| state.shown.locked().clone())
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
        let s = &mut *state.session.locked();
        if !report.trashed.is_empty() {
            s.files
                .retain(|f| !report.trashed.iter().any(|t| f.path.starts_with(t)));
        }
        report.followed = followed(&mut s.files, &report.moved);
    }
    forget_tags_of(&state, report.retagged.clone());
    // With what is said of each stage that went wrong (the command line
    // says the same of each file).
    let told = renome_services::flow::said::troubles(&report);
    let mut v = serde_json::to_value(&report).map_err(|e| e.to_string())?;
    v["told"] = serde_json::to_value(told).map_err(|e| e.to_string())?;
    Ok(v)
}

#[tauri::command]
pub(crate) fn history(state: State<AppState>) -> Vec<Value> {
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
pub(crate) async fn undo(app: AppHandle, id: String) -> Result<Applied, String> {
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
        let s = &mut *state.session.locked();
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
pub(crate) async fn recover(
    app: AppHandle,
) -> Result<Option<renome_services::flow::said::Recovery>, String> {
    in_background(app, |state| {
        renome_services::flow::said::recovered(&state.history.recover())
    })
    .await
}

#[tauri::command]
pub(crate) fn delete_history(state: State<AppState>, id: String) -> Result<(), String> {
    state.history.delete(&id)
}

#[tauri::command]
pub(crate) fn clear_history(state: State<AppState>) -> Result<usize, String> {
    state.history.clear()
}
