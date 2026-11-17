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

/// Rule types that need a catalogue lookup not in the command line yet.
const LOOKUPS: [&str; 4] = ["series", "music", "book", "comic"];

pub fn preview_or_apply(
    m: &ArgMatches,
    settings: Option<&Value>,
    apply: bool,
) -> Result<Exit, String> {
    // The rules.
    let sources = crate::sources(m);
    if sources.is_empty() {
        return Err(t("noRules", &[]));
    }
    let steps_json = rulespec::steps(&sources, settings)?;
    rulespec::check_together(&steps_json)?;
    for step in &steps_json {
        let kind = rulespec::kind_of(step);
        if step.get("enabled") != Some(&Value::Bool(false)) && LOOKUPS.contains(&kind) {
            return Err(t(
                "lookupLater",
                &[("rule", &t(&format!("rule.{kind}"), &[]))],
            ));
        }
    }
    let steps: Vec<Step> = steps_json
        .iter()
        .map(|s| serde_json::from_value(s.clone()).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;

    // The list.
    let paths: Vec<PathBuf> = m
        .get_many::<PathBuf>("paths")
        .unwrap()
        .map(|p| std::path::absolute(p).unwrap_or_else(|_| p.clone()))
        .collect();
    let opts = ScanOptions {
        recursive: !m.get_flag("no-recursive"),
        include_hidden: m.get_flag("hidden") || setting_bool(settings, "includeHidden", false),
        folders_as_items: m.get_flag("folders"),
    };
    let found = renome_core::scan(&paths, &opts, &HashSet::new());
    let mut files = found.files;
    for s in &found.skipped {
        out::note(&format!("{}: {}", out::path(&s.path), s.reason));
    }
    if files.is_empty() {
        out::note(&t("noFiles", &[]));
        return Ok(Exit::Done);
    }
    let mut view = View {
        filter: m.get_one::<String>("filter").cloned().unwrap_or_default(),
        lang: lang().into(),
        leftover_suffix: t("leftoverSuffix", &[]),
        number_clashes: setting_bool(settings, "numberClashes", false),
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
            renome_core::sort_files(&mut files, key, view.descending);
        }
        None => {}
    }
    let overrides = names_by_hand(m)?;
    if steps
        .iter()
        .any(|s| s.enabled && matches!(s.rule, renome_core::Rule::Photo { .. }))
    {
        let list: Vec<&FileEntry> = files.iter().collect();
        renome_core::photos::read_all(&list, &|_, _| {});
    }

    // The lookup rule: searched on the list as the rules above it name it,
    // then the list made again with what was chosen.
    let movie_rule = steps_json
        .iter()
        .find(|s| rulespec::kind_of(s) == "movie" && s.get("enabled") != Some(&Value::Bool(false)))
        .map(|s| s["rule"].clone());
    let mut movies = None;
    if let Some(rule) = &movie_rule {
        view.trash_ads = rule
            .get("trashAds")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let first = renome_core::preview(&files, &steps, &overrides, &view);
        let ask = ask_of(m)?;
        let found = runtime()?.block_on(crate::lookup::movies(&first, settings, &ask))?;
        view.movies = found.chosen.clone();
        movies = Some(found);
    }
    let preview = renome_core::preview(&files, &steps, &overrides, &view);
    if let Some(found) = &movies {
        let doubtful = preview
            .items
            .iter()
            .filter(|i| {
                let ext = Path::new(&i.original)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                renome_core::tmdb::is_video(&ext)
                    && i.extra.is_none()
                    && !i.release_junk
                    && !found.chosen.contains_key(&i.path)
            })
            .count();
        if doubtful > 0 {
            out::note(&out::yellow(&t("leftDoubtful", &[("n", &doubtful)])));
        }
    }
    if let Some(e) = &preview.filter_error {
        return Err(format!("{}: {e}", t("invalidFilter", &[])));
    }
    if let Some(e) = preview.rule_errors.first() {
        return Err(serde_json::to_value(e)
            .map(|v| v.to_string())
            .unwrap_or_default());
    }
    if let Some(path) = m.get_one::<PathBuf>("csv") {
        write_csv(path, &preview)?;
        out::note(&t("csvSaved", &[]));
    }
    if !apply {
        show(&preview, m.get_flag("all"));
        return Ok(if preview.conflicts > 0 {
            Exit::Doubts
        } else {
            Exit::Done
        });
    }
    if !out::json() {
        show(&preview, m.get_flag("all"));
    }
    if preview.changed == 0 {
        if out::json() {
            println!(
                "{}",
                json!({ "renamed": 0, "historyId": null, "failed": [] })
            );
        }
        return Ok(if preview.conflicts > 0 {
            Exit::Doubts
        } else {
            Exit::Done
        });
    }
    if preview.conflicts > 0 && m.get_flag("strict") {
        out::note(&t("strictConflicts", &[("n", &preview.conflicts)]));
        return Ok(Exit::Doubts);
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
    let (exit, report) = rename(
        &preview,
        m.get_one::<PathBuf>("to").is_some(),
        m.get_flag("copy"),
    )?;
    if let (Some(rule), Some(found)) = (&movie_rule, &movies) {
        let history = renome_core::History::new(renome_services::history_dir());
        for note in runtime()?.block_on(crate::lookup::after_movies(
            rule, found, &preview, &report, &history,
        )) {
            out::note(&note);
        }
    }
    Ok(exit)
}

/// Runs the plan: the renames, recorded in the History, with progress on
/// stderr; what failed is listed.
fn rename(
    preview: &Preview,
    sent: bool,
    copying: bool,
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
        if !out::quiet() && total > 0 {
            eprint!("\r{}%", done * 100 / total);
            shown.set(true);
        }
    };
    let hooks = renome_core::apply::Hooks {
        progress: &progress,
        stop: &|| false,
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
        let key = match sent {
            true if copying => "copiedN",
            true => "movedN",
            false => "renamedN",
        };
        out::note(&out::green(&t(key, &[("n", &report.done.len())])));
        for e in &report.errors {
            out::error(&format!(
                "{} → {}: {}",
                out::path(&e.from),
                out::path(&e.to),
                e.detail
            ));
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
    Ok(crate::lookup::Ask { choose, picks })
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

fn setting_bool(settings: Option<&Value>, key: &str, default: bool) -> bool {
    settings
        .and_then(|s| s.get(key)?.as_bool())
        .unwrap_or(default)
}

/// Asks a yes/no question on the terminal.
pub fn ask(question: &str) -> bool {
    eprint!("{question}");
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(
        answer.trim().to_lowercase().as_str(),
        "s" | "sim" | "y" | "yes" | "sí" | "si"
    )
}

/// The list: one line per changed file (all with `all`), grouped by
/// folder, then the totals.
fn show(p: &Preview, all: bool) {
    if out::json() {
        let items: Vec<Value> = p
            .items
            .iter()
            .filter(|i| all || i.status != Status::Unchanged)
            .map(item_json)
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "items": items,
                "totals": { "files": p.total, "changed": p.changed, "conflicts": p.conflicts },
                "trash": p.trash,
                "trashFolders": p.trash_folders,
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
            Status::Unchanged if i.to_trash => println!(
                "  {}  {}",
                out::dim(&i.original),
                out::yellow(&format!("({})", t("junkRowTrash", &[])))
            ),
            Status::Unchanged => println!("  {}", out::dim(&i.original)),
            Status::Ok => println!("  {} → {}", i.original, out::green(&new)),
            Status::Conflict => {
                let why = i
                    .problem
                    .map(|pr| t(&format!("problem.{}", problem_key(pr)), &[]))
                    .unwrap_or_default();
                println!(
                    "  {} → {}  {}",
                    i.original,
                    out::red(&new),
                    out::yellow(&format!("({why})"))
                );
            }
        }
    }
    let mut totals = vec![
        t("files", &[("n", &p.total)]),
        t("changed", &[("n", &p.changed)]),
    ];
    if p.conflicts > 0 {
        totals.push(out::red(&t("conflicts", &[("n", &p.conflicts)])));
    }
    if unchanged > 0 && p.changed > 0 {
        totals.push(out::dim(&t("unchangedN", &[("n", &unchanged)])));
    }
    out::note(&totals.join(" · "));
}

/// A problem's name as the page's texts call it (`problem.<name>`).
fn problem_key(p: renome_core::Problem) -> String {
    serde_json::to_value(p)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn item_json(i: &PreviewItem) -> Value {
    json!({
        "path": i.path,
        "newPath": i.target(),
        "status": i.status,
        "problem": i.problem,
        "manual": i.manual,
        "isDir": i.is_dir,
    })
}

/// "Export list": the same CSV as the window's.
fn write_csv(path: &Path, p: &Preview) -> Result<(), String> {
    let sep = if lang() == "en" { "," } else { ";" };
    let cell = |v: &str| {
        if v.contains(['"', ',', ';', '\n', '\r']) {
            format!("\"{}\"", v.replace('"', "\"\""))
        } else {
            v.to_string()
        }
    };
    let mut rows = vec![
        [
            "csv.folder",
            "csv.original",
            "csv.new",
            "csv.dest",
            "csv.status",
            "csv.problem",
        ]
        .iter()
        .map(|k| cell(&t(k, &[])))
        .collect::<Vec<_>>()
        .join(sep),
    ];
    for i in &p.items {
        let status = serde_json::to_value(i.status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let problem = i
            .problem
            .map(|pr| t(&format!("problem.{}", problem_key(pr)), &[]))
            .unwrap_or_default();
        rows.push(
            [
                i.dir.display().to_string(),
                i.original.clone(),
                i.new_name.clone(),
                i.folder.clone().unwrap_or_default(),
                t(&format!("csv.{status}"), &[]),
                problem,
            ]
            .iter()
            .map(|v| cell(v))
            .collect::<Vec<_>>()
            .join(sep),
        );
    }
    std::fs::write(path, format!("\u{feff}{}\r\n", rows.join("\r\n"))).map_err(|e| e.to_string())
}
