//! `preview` and `apply`: the list made from the paths, the rules run on
//! it as in the window, then (for `apply`) the renames, recorded in the
//! History.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use clap::ArgMatches;
use renome_core::{FileEntry, Preview, PreviewItem, ScanOptions, Status, Step, View};
use serde_json::{Value, json};

use crate::Exit;
use crate::out;
use crate::rulespec;
use crate::text::{lang, t};

pub fn preview_or_apply(
    m: &ArgMatches,
    settings: Option<&Value>,
    apply: bool,
) -> Result<Exit, String> {
    let (steps_json, steps) = build_steps(m, settings)?;
    let Some(mut files) = scan_list(m, settings) else {
        return Ok(Exit::Done);
    };
    let mut view = build_view(m, settings, &mut files);
    let overrides = names_by_hand(m)?;
    read_photo_dates(&steps, &files);
    let lookup_rule = lookup_rule_of(&steps_json);
    let edits = parse_edits(m, &steps, lookup_rule.as_ref(), &mut view)?;
    view.trash_ads = renome_core::catalog::trash_ads(&steps_json);
    let found = match &lookup_rule {
        Some(rule) => Some(search(
            rule, m, settings, &files, &steps, &overrides, &edits, &mut view,
        )?),
        None => None,
    };
    // Stopped during the searches: on with what was found, if wanted.
    if crate::stop::requested() {
        if !(std::io::stdin().is_terminal() && ask(&format!("{} [s/N] ", t("goOnAfterStop", &[]))))
        {
            return Ok(Exit::Stopped);
        }
        crate::stop::reset();
    }
    let mut preview = renome_core::preview(&files, &steps, &overrides, &view);
    // Identical copies heading for one name: the first goes, the others
    // stay where they are (the window finds them in the background).
    let copies = renome_core::identical::copies_among(&files, &preview);
    if !copies.is_empty() {
        view.identical = copies;
        preview = renome_core::preview(&files, &steps, &overrides, &view);
    }
    let doubtful = count_doubtful(&found, &preview);
    // The rules that are on, as saved: what is missing and what is written
    // after the renames read their options from them.
    let rules_on: Vec<Value> = steps_json
        .iter()
        .filter(|s| s.get("enabled") != Some(&Value::Bool(false)))
        .map(|s| s["rule"].clone())
        .collect();
    if !out::json() {
        warn_reader_limits(&rules_on);
    }
    let missing = match &found {
        Some(f) if m.get_flag("missing") || m.contains_id("missing-csv") => {
            crate::missing::of(&rules_on, f, &preview, m.get_flag("desc"))
        }
        _ => Vec::new(),
    };
    check_errors(&preview, &steps_json)?;
    export_csvs(m, settings, &files, &preview, &steps_json, &missing)?;
    finish(
        m, settings, apply, &steps, &preview, &rules_on, &found, &edits, &missing, doubtful,
    )
}

/// The rules of the command line and the settings, checked, as the plain
/// JSON of each and as the steps the core runs.
fn build_steps(
    m: &ArgMatches,
    settings: Option<&Value>,
) -> Result<(Vec<Value>, Vec<Step>), String> {
    let sources = crate::sources(m);
    if sources.is_empty() {
        return Err(t("noRules", &[]));
    }
    let steps_json = rulespec::steps(&sources, settings)?;
    rulespec::check_together(&steps_json)?;
    let steps: Vec<Step> = steps_json
        .iter()
        .map(|s| serde_json::from_value(s.clone()).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;

    Ok((steps_json, steps))
}

/// The files under the paths; `None` (said) when there are none.
fn scan_list(m: &ArgMatches, settings: Option<&Value>) -> Option<Vec<FileEntry>> {
    let paths: Vec<PathBuf> = m
        .get_many::<PathBuf>("paths")
        .unwrap()
        .map(|p| std::path::absolute(p).unwrap_or_else(|_| p.clone()))
        .collect();
    let opts = ScanOptions {
        recursive: !m.get_flag("no-recursive"),
        include_hidden: m.get_flag("hidden") || setting_bool(settings, "includeHidden"),
        folders_as_items: m.get_flag("folders"),
    };
    let found = renome_core::scan(&paths, &opts, &HashSet::new());
    let files = found.files;
    for s in &found.skipped {
        out::note(&format!("{}: {}", out::path(&s.path), s.reason));
    }
    if files.is_empty() {
        out::note(&t("noFiles", &[]));
        return None;
    }
    Some(files)
}

/// What the list is seen with (filter, language, destination, sort order);
/// sorts `files` as asked.
fn build_view(m: &ArgMatches, settings: Option<&Value>, files: &mut [FileEntry]) -> View {
    let mut view = View {
        filter: m.get_one::<String>("filter").cloned().unwrap_or_default(),
        lang: lang().into(),
        leftover_suffix: t("leftoverSuffix", &[]),
        number_clashes: setting_bool(settings, "numberClashes"),
        protected: renome_services::protected_dirs(),
        send_to: m
            .get_one::<PathBuf>("to")
            .map(|dir| renome_core::plan::SendTo {
                dir: std::path::absolute(dir).unwrap_or_else(|_| dir.clone()),
                copy: m.get_flag("copy"),
            }),
        descending: m.get_flag("desc"),
        ..View::default()
    };
    match m.get_one::<String>("sort").map(String::as_str) {
        Some("new") => view.sort_by_new_name = true,
        Some(key) => {
            let key = match key {
                "ext" => renome_core::SortKey::Ext,
                "dir" => renome_core::SortKey::Dir,
                "size" => renome_core::SortKey::Size,
                "modified" => renome_core::SortKey::Modified,
                _ => renome_core::SortKey::Name,
            };
            renome_core::sort_files(files, key, view.descending);
        }
        None => {}
    }
    view
}

/// The photos' dates read from the files, when a photo rule is on.
fn read_photo_dates(steps: &[Step], files: &[FileEntry]) {
    if steps
        .iter()
        .any(|s| s.enabled && matches!(s.rule, renome_core::Rule::Photo { .. }))
    {
        let list: Vec<&FileEntry> = files.iter().collect();
        renome_core::photos::read_all(&list, &|_, _| {});
    }
}

/// The first rule that is on and needs a catalogue lookup, as saved.
fn lookup_rule_of(steps_json: &[Value]) -> Option<Value> {
    steps_json
        .iter()
        .find(|s| {
            renome_core::catalog::LOOKUPS.contains(&rulespec::kind_of(s))
                && s.get("enabled") != Some(&Value::Bool(false))
        })
        .map(|s| s["rule"].clone())
}

/// The details typed by hand (`--edit`), for the lookup rule's editor or the
/// photo rule's dates.
fn parse_edits(
    m: &ArgMatches,
    steps: &[Step],
    lookup_rule: Option<&Value>,
    view: &mut View,
) -> Result<crate::edit::Edits, String> {
    let edit_specs: Vec<String> = m
        .get_many::<String>("edit")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();
    let has_photo = steps
        .iter()
        .any(|s| s.enabled && matches!(s.rule, renome_core::Rule::Photo { .. }));
    let edit_kind = match lookup_rule.and_then(|r| r["type"].as_str()) {
        Some(k @ ("music" | "book" | "comic")) => k,
        _ if has_photo => "photo",
        _ => "",
    };
    if !edit_specs.is_empty() && edit_kind.is_empty() {
        return Err(t("noEditor", &[]));
    }
    let edits = crate::edit::parse(&edit_specs, &crate::edit::fields_of(edit_kind))?;
    if edit_kind == "photo" {
        crate::edit::photo_dates(&edits, &mut view.dates)?;
    }
    Ok(edits)
}

/// Searches the catalogue for what the lookup rule needs, on the list as
/// the rules above it name it, and puts what was chosen in the view.
#[allow(clippy::too_many_arguments)]
fn search(
    rule: &Value,
    m: &ArgMatches,
    settings: Option<&Value>,
    files: &[FileEntry],
    steps: &[Step],
    overrides: &HashMap<PathBuf, String>,
    edits: &crate::edit::Edits,
    view: &mut View,
) -> Result<crate::lookup::Found, String> {
    let first = renome_core::preview(files, steps, overrides, view);
    let names: HashMap<&PathBuf, String> = first
        .items
        .iter()
        .map(|i| {
            (
                &i.path,
                i.lookup_name.clone().unwrap_or_else(|| {
                    i.path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                }),
            )
        })
        .collect();
    let by_path: HashMap<&PathBuf, &FileEntry> = files.iter().map(|f| (&f.path, f)).collect();
    let ask = ask_of(m)?;
    let rt = runtime()?;
    Ok(match rule["type"].as_str() {
        Some("movie") => {
            let f = rt.block_on(crate::lookup::movies(&first, settings, &ask))?;
            view.movies = f.chosen.clone();
            crate::lookup::Found::Movies(f)
        }
        Some("comic") => {
            let mut f = rt.block_on(crate::lookup::comics(&first, rule, settings, &ask))?;
            crate::edit::comics(
                edits,
                &mut f,
                &first.items,
                &names,
                rule.get("seriesByFolder")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            );
            view.comics = f.chosen.clone();
            crate::lookup::Found::Comics(Box::new(f))
        }
        Some("book") => {
            let mut f = rt.block_on(crate::lookup::books(&first, rule, settings, &ask))?;
            let audiobooks = rule.get("audiobooks").and_then(Value::as_bool) != Some(false);
            crate::edit::books(edits, &mut f, &first.items, &names, audiobooks);
            view.books = f.chosen.clone();
            crate::lookup::Found::Books(Box::new(f))
        }
        Some("music") => {
            let mut f = rt.block_on(crate::lookup::music(files, &first, rule, &ask))?;
            crate::edit::songs(
                edits,
                &mut f.chosen,
                &by_path,
                &first.items,
                rule.get("fromName")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            );
            view.songs = f.chosen.clone();
            crate::lookup::Found::Music(Box::new(f))
        }
        _ => {
            let f = rt.block_on(crate::lookup::series(&first, rule, settings, &ask))?;
            view.episodes = f.chosen.clone();
            if f.loose > 0 {
                out::note(&t("tvLoose", &[("n", &f.loose)]));
            }
            crate::lookup::Found::Series(f)
        }
    })
}

/// How many files the search left without an answer (said, in yellow).
fn count_doubtful(found: &Option<crate::lookup::Found>, preview: &Preview) -> usize {
    let Some(found) = found else {
        return 0;
    };
    let items: Vec<renome_services::flow::Item> = preview
        .items
        .iter()
        .map(renome_services::flow::Item::from)
        .collect();
    let doubtful = renome_services::flow::left_unanswered(found.rule(), &items, &|p| found.has(p));
    if doubtful > 0 {
        out::note(&out::yellow(&t("leftDoubtful", &[("n", &doubtful)])));
    }
    doubtful
}

/// A comic's e-book or a converted book its reader does not open as it
/// comes: said as the window's rule card says it.
fn warn_reader_limits(rules_on: &[Value]) {
    if !out::json() {
        for rule in rules_on {
            if let Some(n) = renome_core::readers::reader(rule).not_opens {
                let name = n
                    .name
                    .strip_prefix('@')
                    .map_or_else(|| n.name.to_string(), |k| t(k, &[]));
                out::note(&out::yellow(&t(
                    "readerNotOpens",
                    &[
                        ("reader", &name),
                        ("format", &n.format.to_uppercase()),
                        ("best", &n.best.to_uppercase()),
                    ],
                )));
            }
        }
    }
}

/// The filter or a rule that does not work, said as the window's rule card
/// says it: the rule, what is wrong, why.
fn check_errors(preview: &Preview, steps_json: &[Value]) -> Result<(), String> {
    if let Some(e) = &preview.filter_error {
        return Err(format!("{}: {e}", t("invalidFilter", &[])));
    }
    if let Some(e) = preview.rule_errors.first() {
        let rule = steps_json
            .get(e.index)
            .and_then(|s| s["rule"].get("type"))
            .and_then(Value::as_str)
            .map(|ty| t(&format!("rule.{ty}"), &[]))
            .unwrap_or_default();
        let what = t(e.code, &[]);
        return Err(if e.detail.is_empty() {
            format!("{rule}: {what}")
        } else {
            format!("{rule}: {what}: {}", e.detail)
        });
    }
    Ok(())
}

/// `--csv` and `--missing-csv`: the list and what is missing, as the
/// window's list has them.
fn export_csvs(
    m: &ArgMatches,
    settings: Option<&Value>,
    files: &[FileEntry],
    preview: &Preview,
    steps_json: &[Value],
    missing: &[crate::missing::Missing],
) -> Result<(), String> {
    if let Some(path) = m.get_one::<PathBuf>("csv") {
        // The optional columns chosen in the options that make sense with
        // these rules, as the window's list has them.
        let chosen: Vec<String> = settings
            .and_then(|s| s.get("columns")?.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        let available = renome_core::catalog::columns_for(steps_json);
        let columns: Vec<&str> = renome_core::catalog::COLUMNS
            .iter()
            .copied()
            .filter(|c| available.contains(c) && chosen.iter().any(|x| x == c))
            .collect();
        let details: HashMap<PathBuf, renome_core::details::Details> = if columns.is_empty() {
            HashMap::new()
        } else {
            files
                .iter()
                .map(|f| (f.path.clone(), renome_core::details::of(f)))
                .collect()
        };
        let rows = renome_services::flow::export::list(preview, &details, &columns, lang());
        write_cells(path, &rows)?;
        out::note(&t("csvSaved", &[]));
    }
    if let Some(path) = m.get_one::<PathBuf>("missing-csv") {
        let dirs: HashMap<&PathBuf, &PathBuf> =
            preview.items.iter().map(|i| (&i.path, &i.dir)).collect();
        let csv = renome_services::flow::export::missing(missing, &|p| {
            dirs.get(&p.to_path_buf()).map(|d| (*d).clone())
        });
        write_cells(path, &csv.rows)?;
        out::note(&t("missingSaved", &[]));
    }
    Ok(())
}

/// The list as the terminal shows it, with the options of the command line.
fn show_list(
    m: &ArgMatches,
    settings: Option<&Value>,
    steps: &[Step],
    preview: &Preview,
    missing: &[crate::missing::Missing],
) {
    show(
        preview,
        steps,
        m.get_flag("all"),
        missing,
        setting_bool(settings, "warnWindows"),
    );
}

/// The exit when nothing is wrong, or when conflicts or doubts are left.
fn doubts_or_done(preview: &Preview, doubtful: usize) -> Exit {
    if preview.real_conflicts() > 0 || doubtful > 0 {
        Exit::Doubts
    } else {
        Exit::Done
    }
}

/// The preview ends here, or the renames are asked for, done, and what is
/// written besides them (the shared flow of the window) follows.
#[allow(clippy::too_many_arguments)]
fn finish(
    m: &ArgMatches,
    settings: Option<&Value>,
    apply: bool,
    steps: &[Step],
    preview: &Preview,
    rules_on: &[Value],
    found: &Option<crate::lookup::Found>,
    edits: &crate::edit::Edits,
    missing: &[crate::missing::Missing],
    doubtful: usize,
) -> Result<Exit, String> {
    if !apply {
        show_list(m, settings, steps, preview, missing);
        return Ok(doubts_or_done(preview, doubtful));
    }
    if !out::json() {
        show_list(m, settings, steps, preview, missing);
    }
    // What is written besides the renames is decided by the shared flow,
    // as in the window: from the rules that are on and what was chosen
    // (the subtitles searched now, before the videos move).
    let typed: std::collections::HashSet<PathBuf> = edits.keys().cloned().collect();
    let mut chosen = crate::lookup::chosen(found.as_ref(), &typed);
    let auth = crate::lookup::subs_auth(settings);
    if let Some(crate::lookup::Found::Movies(_) | crate::lookup::Found::Series(_)) = &found {
        runtime()?.block_on(crate::lookup::subtitles(
            rules_on,
            preview,
            &auth,
            &mut chosen,
        ));
    }
    let counts = renome_services::flow::after::counts(rules_on, preview, &chosen);
    if preview.changed == 0 && counts.besides == 0 {
        if out::json() {
            println!(
                "{}",
                json!({ "renamed": 0, "historyId": null, "failed": [] })
            );
        }
        return Ok(doubts_or_done(preview, doubtful));
    }
    if preview.real_conflicts() > 0 && m.get_flag("strict") {
        out::note(&t("strictConflicts", &[("n", &preview.real_conflicts())]));
        return Ok(Exit::Doubts);
    }
    // What the window's question says (the shared lines): the work besides
    // the renames, conflicts, adverts to the trash and old folders left
    // behind, these last told even with --yes.
    let to = m.get_one::<PathBuf>("to");
    let said = renome_services::flow::after::ask(
        preview,
        &counts,
        to.map(|d| (d.as_path(), m.get_flag("copy"))),
    );
    if !out::json() && (!m.get_flag("yes") || said.always) {
        for line in crate::lookup::planned(&said) {
            out::note(&line);
        }
    }
    if !m.get_flag("yes") {
        if !std::io::stdin().is_terminal() {
            return Err(t("needYes", &[]));
        }
        if !ask(&t("askRename", &[])) {
            out::note(&t("cancelled", &[]));
            return Ok(Exit::Done);
        }
    }
    crate::manage::recover();
    let (exit, report) = if preview.changed == 0 {
        (Exit::Done, renome_core::ApplyReport::default())
    } else {
        rename(preview, to.map(|_| m.get_flag("copy")))?
    };
    if !m.get_flag("no-server") {
        for note in runtime()?.block_on(crate::servers::tell(settings, &report.done)) {
            out::note(&note);
        }
    }
    // Stopped while renaming: what was renamed stays (and can be undone);
    // nothing else is done.
    if crate::stop::requested() {
        out::note(&t("stoppedRenaming", &[]));
        return Ok(Exit::Stopped);
    }
    let history = renome_core::History::new(renome_services::history_dir());
    for note in runtime()?.block_on(crate::lookup::after(
        rules_on,
        found.as_ref(),
        preview,
        &chosen,
        &report,
        &history,
        settings,
        auth,
    )) {
        out::note(&note);
    }
    // Renamed what was certain; conflicts or doubts left say so.
    if matches!(exit, Exit::Done) && (preview.real_conflicts() > 0 || doubtful > 0) {
        return Ok(Exit::Doubts);
    }
    Ok(exit)
}

/// Runs the plan: the renames, recorded in the History, with progress on
/// stderr; what failed is listed.
fn rename(
    preview: &Preview,
    sent: Option<bool>,
) -> Result<(Exit, renome_core::ApplyReport), String> {
    let _lock = renome_services::RenameLock::take(|| out::note(&t("waitingWindow", &[])))?;
    let history = renome_core::History::new(renome_services::history_dir());
    let ops = preview.operations();
    let touch: HashMap<PathBuf, i64> = preview
        .items
        .iter()
        .filter_map(|i| Some((i.path.clone(), i.touch?)))
        .collect();
    let shown = std::cell::Cell::new(false);
    let progress = |done: u64, total: u64| {
        if out::live() && total > 0 {
            eprint!("\r{}%", done * 100 / total);
            shown.set(true);
        }
    };
    let hooks = renome_core::apply::Hooks {
        progress: &progress,
        stop: &crate::stop::requested,
    };
    let report = history.apply_with(&ops, &hooks);
    if shown.get() {
        eprintln!();
    }
    if let Some(id) = &report.history_id {
        let times: Vec<(PathBuf, i64)> = report
            .done
            .iter()
            .filter_map(|op| Some((op.to.clone(), *touch.get(&op.from)?)))
            .collect();
        if !times.is_empty() {
            let _ = history.set_times(id, &times);
        }
    }
    let failed: Vec<Value> = report
        .errors
        .iter()
        .map(|e| json!({ "path": e.from, "to": e.to, "error": e.detail }))
        .collect();
    if out::json() {
        println!(
            "{}",
            json!({ "renamed": report.done.len(), "historyId": report.history_id, "failed": failed })
        );
    } else {
        // As the window says it (the shared lines).
        let lines = crate::lookup::said(&renome_services::flow::after::renamed(
            preview, &report, sent,
        ));
        for (i, line) in lines.iter().enumerate() {
            out::note(&if i == 0 {
                out::green(line)
            } else {
                line.clone()
            });
        }
        // As the window says them (`flow::said::failed`).
        let failed: Vec<_> = report
            .errors
            .iter()
            .map(renome_services::flow::said::failed)
            .collect();
        for line in crate::lookup::said(&failed) {
            out::error(&line);
        }
    }
    let exit = if report.errors.is_empty() {
        Exit::Done
    } else {
        Exit::Failed
    };
    Ok((exit, report))
}

/// A runtime for the online work.
fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| e.to_string())
}

/// `--choose` and `--pick`: ask in the terminal by default, when there is
/// one.
fn ask_of(m: &ArgMatches) -> Result<crate::lookup::Ask, String> {
    use crate::lookup::Choose;
    let choose = match m.get_one::<String>("choose").map(String::as_str) {
        Some("auto") => Choose::Auto,
        Some("skip") => Choose::Skip,
        Some(_) => Choose::Ask,
        None if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() => Choose::Ask,
        None => Choose::Auto,
    };
    let mut picks = HashMap::new();
    for spec in m.get_many::<String>("pick").into_iter().flatten() {
        let (path, id) = spec
            .rsplit_once('=')
            .ok_or_else(|| t("badPick", &[("spec", spec)]))?;
        let path = Path::new(path);
        picks.insert(
            std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
            id.to_string(),
        );
    }
    Ok(crate::lookup::Ask {
        choose,
        picks,
        missing: m.get_flag("missing"),
    })
}

/// `--name PATH=NAME`, by the file's full path.
fn names_by_hand(m: &ArgMatches) -> Result<HashMap<PathBuf, String>, String> {
    let mut out = HashMap::new();
    for spec in m.get_many::<String>("name").into_iter().flatten() {
        let (path, name) = spec
            .rsplit_once('=')
            .ok_or_else(|| t("badName", &[("spec", spec)]))?;
        let path = Path::new(path);
        out.insert(
            std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
            name.to_string(),
        );
    }
    Ok(out)
}

/// An option that is yes or no: as saved, else its default (see
/// `renome_services::options`).
fn setting_bool(settings: Option<&Value>, key: &str) -> bool {
    settings
        .and_then(|s| s.get(key)?.as_bool())
        .or_else(|| {
            renome_services::options::setting(key)
                .and_then(|s| renome_services::options::default_of(s).as_bool())
        })
        .unwrap_or(false)
}

/// Asks a yes/no question on the terminal.
pub fn ask(question: &str) -> bool {
    eprint!("{question}");
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    if crate::stop::asking(|| std::io::stdin().lock().read_line(&mut answer)).is_err() {
        return false;
    }
    matches!(
        answer.trim().to_lowercase().as_str(),
        "s" | "sim" | "y" | "yes" | "sí" | "si"
    )
}

/// The list: one line per changed file (all with `all`), grouped by
/// folder, then the totals.
fn show(
    p: &Preview,
    steps: &[Step],
    all: bool,
    missing: &[crate::missing::Missing],
    warn_windows: bool,
) {
    // What the window's rule cards say under them (`said::summaries`).
    let notes: Vec<String> = crate::lookup::said(
        &renome_services::flow::said::summaries(p, steps)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
    );
    if out::json() {
        let items: Vec<Value> = p
            .items
            .iter()
            .filter(|i| all || i.status != Status::Unchanged)
            .map(|i| item_json(i, warn_windows))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "items": items,
                "totals": { "files": p.total, "changed": p.changed, "conflicts": p.real_conflicts(), "sameThere": p.same_there, "leftFolders": p.left_folders.len() },
                "trash": p.trash,
                "trashFolders": p.trash_folders,
                "notes": notes,
                "missing": missing,
            }))
            .unwrap_or_default()
        );
        return;
    }
    let mut last_dir: Option<&Path> = None;
    let mut unchanged = 0;
    for i in &p.items {
        if i.status == Status::Unchanged && !all && !i.to_trash {
            unchanged += 1;
            continue;
        }
        if last_dir != Some(i.dir.as_path()) {
            println!("{}", out::bold(&out::path(&i.dir)));
            last_dir = Some(i.dir.as_path());
        }
        // Into a folder of the rule's: as the Folder column shows it; sent
        // elsewhere: the whole path.
        let new = match (&i.folder, &i.dest) {
            (Some(folder), _) if !folder.is_empty() => {
                format!("{folder}{}{}", std::path::MAIN_SEPARATOR, i.new_name)
            }
            (_, Some(dest)) if dest != &i.dir => out::path(&dest.join(&i.new_name)),
            _ => i.new_name.clone(),
        };
        match i.status {
            Status::Unchanged => println!("  {}{}", out::dim(&i.original), note(i, warn_windows)),
            Status::Ok => println!(
                "  {} → {}{}",
                i.original,
                out::green(&new),
                note(i, warn_windows)
            ),
            Status::Conflict => {
                let why = i
                    .problem
                    .map(|pr| t(&format!("problem.{}", problem_key(pr)), &[]))
                    .unwrap_or_default();
                println!(
                    "  {} → {}  {}{}",
                    i.original,
                    out::red(&new),
                    out::yellow(&format!("({why})")),
                    note(i, warn_windows)
                );
            }
        }
    }
    // The list's totals, as under the window's list (`said::totals`).
    let mut totals: Vec<String> = renome_services::flow::said::totals(p)
        .iter()
        .map(|l| {
            let text = crate::lookup::said(std::slice::from_ref(l)).concat();
            match l.key {
                "conflicts" => out::red(&text),
                "leftoverCount" => out::yellow(&text),
                "sameThereN" => out::dim(&text),
                _ => text,
            }
        })
        .collect();
    if unchanged > 0 && p.changed > 0 {
        totals.push(out::dim(&t("unchangedN", &[("n", &unchanged)])));
    }
    out::note(&totals.join(" · "));
    for l in &notes {
        out::note(&out::dim(l));
    }
    if !missing.is_empty() {
        println!();
        println!(
            "{}",
            out::bold(&format!("{} ({})", t("missingTitle", &[]), missing.len()))
        );
        let mut group = "";
        for x in missing {
            if x.group != group {
                println!("  {}", out::bold(&x.group));
                group = &x.group;
            }
            let title = if x.title.is_empty() {
                String::new()
            } else {
                format!(" · {}", x.title)
            };
            let date = if x.date.is_empty() {
                String::new()
            } else {
                out::dim(&format!(" ({})", x.date))
            };
            println!("    {}{title}{date}", out::yellow(&x.code));
        }
    }
}

/// What the window tags a row with besides its problem (`said::tags`):
/// left alone by the lookup rule, pages drawn, a release's advert, what
/// the subtitle rule does, a name Windows does not take (when asked to
/// warn)…
fn note(i: &PreviewItem, warn_windows: bool) -> String {
    let tags: Vec<String> = renome_services::flow::said::tags(i, warn_windows)
        .iter()
        .map(|g| crate::lookup::said(&g.text).join(" · "))
        .collect();
    if tags.is_empty() {
        String::new()
    } else {
        format!("  {}", out::yellow(&format!("({})", tags.join(" · "))))
    }
}

fn problem_key(p: renome_core::Problem) -> String {
    serde_json::to_value(p)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn item_json(i: &PreviewItem, warn_windows: bool) -> Value {
    let tags: Vec<Value> = renome_services::flow::said::tags(i, warn_windows)
        .iter()
        .map(|g| {
            json!({
                "at": g.at,
                "text": crate::lookup::said(&g.text).join(" · "),
                "tip": crate::lookup::said(&g.tip).join("\n"),
            })
        })
        .collect();
    json!({
        "tags": tags,
        "path": i.path,
        "newPath": i.target(),
        "status": i.status,
        "problem": i.problem,
        "manual": i.manual,
        "isDir": i.is_dir,
    })
}

/// Saves CSV cells (see `flow::export`), each key in the interface's
/// language.
fn write_cells(
    path: &Path,
    rows: &[Vec<renome_services::flow::export::Cell>],
) -> Result<(), String> {
    let rows = renome_services::flow::export::texts(rows, &|k| t(k, &[]));
    std::fs::write(path, renome_core::text::csv(&rows, lang())).map_err(|e| e.to_string())
}
