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
const LOOKUPS: [&str; 0] = [];

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
        include_hidden: m.get_flag("hidden") || setting_bool(settings, "includeHidden"),
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
    let lookup_rule = steps_json
        .iter()
        .find(|s| {
            renome_core::catalog::LOOKUPS.contains(&rulespec::kind_of(s))
                && s.get("enabled") != Some(&Value::Bool(false))
        })
        .map(|s| s["rule"].clone());
    // The details typed by hand (--edit), for the lookup rule's editor or
    // the photo rule's dates.
    let edit_specs: Vec<String> = m
        .get_many::<String>("edit")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();
    let has_photo = steps
        .iter()
        .any(|s| s.enabled && matches!(s.rule, renome_core::Rule::Photo { .. }));
    let edit_kind = match lookup_rule.as_ref().and_then(|r| r["type"].as_str()) {
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
    let mut found = None;
    view.trash_ads = renome_core::catalog::trash_ads(&steps_json);
    if let Some(rule) = &lookup_rule {
        let first = renome_core::preview(&files, &steps, &overrides, &view);
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
        found = Some(match rule["type"].as_str() {
            Some("movie") => {
                let f = rt.block_on(crate::lookup::movies(&first, settings, &ask))?;
                view.movies = f.chosen.clone();
                crate::lookup::Found::Movies(f)
            }
            Some("comic") => {
                let mut f = rt.block_on(crate::lookup::comics(&first, rule, settings, &ask))?;
                crate::edit::comics(
                    &edits,
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
                crate::edit::books(&edits, &mut f, &first.items, &names, audiobooks);
                view.books = f.chosen.clone();
                crate::lookup::Found::Books(Box::new(f))
            }
            Some("music") => {
                let mut f = rt.block_on(crate::lookup::music(&files, &first, rule, &ask))?;
                crate::edit::songs(
                    &edits,
                    &mut f.chosen,
                    &by_path,
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
        });
    }
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
    let mut doubtful = 0;
    if let Some(found) = &found {
        doubtful = preview
            .items
            .iter()
            .filter(|i| {
                let ext = Path::new(&i.original)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                found.wanted(&ext) && i.extra.is_none() && !i.release_junk && !found.has(&i.path)
            })
            .count();
        if doubtful > 0 {
            out::note(&out::yellow(&t("leftDoubtful", &[("n", &doubtful)])));
        }
    }
    // The rules that are on, as saved: what is missing and what is written
    // after the renames read their options from them.
    let rules_on: Vec<Value> = steps_json
        .iter()
        .filter(|s| s.get("enabled") != Some(&Value::Bool(false)))
        .map(|s| s["rule"].clone())
        .collect();
    let missing = match &found {
        Some(f) if m.get_flag("missing") || m.contains_id("missing-csv") => {
            crate::missing::of(&rules_on, f, &preview, m.get_flag("desc"))
        }
        _ => Vec::new(),
    };
    if let Some(e) = &preview.filter_error {
        return Err(format!("{}: {e}", t("invalidFilter", &[])));
    }
    if let Some(e) = preview.rule_errors.first() {
        return Err(serde_json::to_value(e)
            .map(|v| v.to_string())
            .unwrap_or_default());
    }
    if let Some(path) = m.get_one::<PathBuf>("csv") {
        // The optional columns chosen in the options that make sense with
        // these rules, as the window's list has them.
        let chosen: Vec<String> = settings
            .and_then(|s| s.get("columns")?.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        let available = renome_core::catalog::columns_for(&steps_json);
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
        let rows = renome_services::flow::export::list(&preview, &details, &columns, lang());
        write_cells(path, &rows)?;
        out::note(&t("csvSaved", &[]));
    }
    if let Some(path) = m.get_one::<PathBuf>("missing-csv") {
        let dirs: HashMap<&PathBuf, &PathBuf> =
            preview.items.iter().map(|i| (&i.path, &i.dir)).collect();
        let csv = renome_services::flow::export::missing(&missing, &|p| {
            dirs.get(&p.to_path_buf()).map(|d| (*d).clone())
        });
        write_cells(path, &csv.rows)?;
        out::note(&t("missingSaved", &[]));
    }
    if !apply {
        show(
            &preview,
            m.get_flag("all"),
            &missing,
            setting_bool(settings, "warnWindows"),
        );
        return Ok(if real_conflicts(&preview) > 0 || doubtful > 0 {
            Exit::Doubts
        } else {
            Exit::Done
        });
    }
    if !out::json() {
        show(
            &preview,
            m.get_flag("all"),
            &missing,
            setting_bool(settings, "warnWindows"),
        );
    }
    // What is written besides the renames is decided by the shared flow,
    // as in the window: from the rules that are on and what was chosen
    // (the subtitles searched now, before the videos move).
    let typed: std::collections::HashSet<PathBuf> = edits.keys().cloned().collect();
    let mut chosen = crate::lookup::chosen(found.as_ref(), &typed);
    let auth = crate::lookup::subs_auth(settings);
    if let Some(crate::lookup::Found::Movies(_) | crate::lookup::Found::Series(_)) = &found {
        runtime()?.block_on(crate::lookup::subtitles(
            &rules_on,
            &preview,
            &auth,
            &mut chosen,
        ));
    }
    let counts = renome_services::flow::after::counts(&rules_on, &preview, &chosen);
    if preview.changed == 0 && counts.besides == 0 {
        if out::json() {
            println!(
                "{}",
                json!({ "renamed": 0, "historyId": null, "failed": [] })
            );
        }
        return Ok(if real_conflicts(&preview) > 0 || doubtful > 0 {
            Exit::Doubts
        } else {
            Exit::Done
        });
    }
    if real_conflicts(&preview) > 0 && m.get_flag("strict") {
        out::note(&t("strictConflicts", &[("n", &real_conflicts(&preview))]));
        return Ok(Exit::Doubts);
    }
    if !m.get_flag("yes") {
        if !std::io::stdin().is_terminal() {
            return Err(t("needYes", &[]));
        }
        for line in crate::lookup::planned(&counts) {
            out::note(&line);
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
        rename(
            &preview,
            m.get_one::<PathBuf>("to").is_some(),
            m.get_flag("copy"),
        )?
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
        &rules_on,
        found.as_ref(),
        &preview,
        &chosen,
        &report,
        &history,
        settings,
        auth,
    )) {
        out::note(&note);
    }
    // Renamed what was certain; conflicts or doubts left say so.
    if matches!(exit, Exit::Done) && (real_conflicts(&preview) > 0 || doubtful > 0) {
        return Ok(Exit::Doubts);
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
fn show(p: &Preview, all: bool, missing: &[crate::missing::Missing], warn_windows: bool) {
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
            Status::Unchanged if i.to_trash => println!(
                "  {}  {}",
                out::dim(&i.original),
                out::yellow(&format!("({})", t("junkRowTrash", &[])))
            ),
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
    if real_conflicts(p) > 0 {
        totals.push(out::red(&t("conflicts", &[("n", &real_conflicts(p))])));
    }
    let same = p.conflicts - real_conflicts(p);
    if same > 0 {
        totals.push(out::dim(&t("sameThereN", &[("n", &same)])));
    }
    if unchanged > 0 && p.changed > 0 {
        totals.push(out::dim(&t("unchangedN", &[("n", &unchanged)])));
    }
    out::note(&totals.join(" · "));
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

/// The conflicts, without the files already where they go (the same
/// file is there): those stay, and nothing is wrong.
fn real_conflicts(p: &Preview) -> usize {
    p.conflicts
        - p.items
            .iter()
            .filter(|i| i.problem == Some(renome_core::Problem::SameThere))
            .count()
}

/// A problem's name as the page's texts call it (`problem.<name>`).
/// What the window tags a row with besides its problem: a name Windows
/// does not take (when asked to warn), a file the lookup rule leaves alone.
fn note(i: &PreviewItem, warn_windows: bool) -> String {
    let mut notes = Vec::new();
    if i.windows_unsafe && warn_windows {
        notes.push(t("windowsUnsafe", &[]));
    }
    if let Some(kind) = i.outside {
        let key = serde_json::to_value(kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let mut k = key.chars();
        let cap = k
            .next()
            .map(|c| c.to_uppercase().collect::<String>() + k.as_str())
            .unwrap_or_default();
        notes.push(t(&format!("outside{cap}"), &[]));
    }
    if notes.is_empty() {
        String::new()
    } else {
        format!("  {}", out::yellow(&format!("({})", notes.join(" · "))))
    }
}

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

/// Saves CSV cells (see `flow::export`), each key in the interface's
/// language.
fn write_cells(
    path: &Path,
    rows: &[Vec<renome_services::flow::export::Cell>],
) -> Result<(), String> {
    let rows = renome_services::flow::export::texts(rows, &|k| t(k, &[]));
    std::fs::write(path, renome_core::text::csv(&rows, lang())).map_err(|e| e.to_string())
}
