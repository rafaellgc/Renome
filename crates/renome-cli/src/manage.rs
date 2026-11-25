//! The commands that look after things rather than rename: undo and the
//! History, rule sets, the rules' options, the Options, keys and the
//! answers cache.

use renome_core::sync::MutexExt;
use std::path::Path;

use clap::ArgMatches;
use renome_core::catalog::{self, Kind};
use serde_json::{Map, Value, json};

use crate::Exit;
use crate::out;
use crate::rulespec;
use crate::text::{lang, t};

fn the_history() -> renome_core::History {
    renome_core::History::new(renome_services::history_dir())
}

/// What a rename left halfway is put right first, as the window does when
/// it opens.
pub fn recover() {
    // Said as the window says it (`flow::said::recovered`).
    let Some(said) = renome_services::flow::said::recovered(&the_history().recover()) else {
        return;
    };
    out::note(&out::bold(&t(said.title, &[])));
    for line in crate::lookup::said(&said.lines) {
        out::note(&line);
    }
    for (from, to) in &said.stuck {
        out::error(&format!(
            "{} → {}",
            out::path(Path::new(from)),
            out::path(Path::new(to))
        ));
    }
}

pub fn undo(m: &ArgMatches) -> Result<Exit, String> {
    recover();
    let h = the_history();
    let id = match m.get_one::<String>("id").map(String::as_str) {
        Some(id) if id != "last" => id.to_string(),
        _ => match h.list().into_iter().find(|e| !e.undone) {
            Some(e) => e.id,
            None => {
                out::note(&t("nothingToUndo", &[]));
                return Ok(Exit::Done);
            }
        },
    };
    if !h.list().iter().any(|e| e.id == id) {
        return Err(t("noEntry", &[("id", &id)]));
    }
    let _lock = renome_services::RenameLock::take(|| out::note(&t("waitingWindow", &[])))?;
    let progress = |done: u64, total: u64| {
        if out::live() && total > 0 {
            eprint!("\r{}%", done * 100 / total);
        }
    };
    let hooks = renome_core::apply::Hooks {
        progress: &progress,
        stop: &|| false,
    };
    let report = h.undo_with(&id, &hooks, &|_, _| {})?;
    let server_notes = if m.get_flag("no-server") {
        Vec::new()
    } else {
        tokio::runtime::Runtime::new()
            .map(|rt| {
                rt.block_on(crate::servers::tell(
                    renome_services::read_settings_file().as_ref(),
                    &report.done,
                ))
            })
            .unwrap_or_default()
    };
    if out::json() {
        let failed: Vec<Value> = report
            .errors
            .iter()
            .map(|e| json!({ "path": e.from, "to": e.to, "error": e.detail }))
            .collect();
        println!(
            "{}",
            json!({ "restored": report.done.len(), "failed": failed })
        );
    } else {
        // As the window says it (`flow::said::undone`).
        let lines = crate::lookup::said(&renome_services::flow::said::undone(&report));
        out::note(&out::green(&lines.join(" · ")));
        if !report.kept.is_empty() {
            out::note(&t("copiesKeptText", &[]));
            for p in &report.kept {
                out::note(&format!("  {}", out::path(p)));
            }
        }
        for note in server_notes {
            out::note(&note);
        }
        let failed: Vec<_> = report
            .errors
            .iter()
            .map(renome_services::flow::said::failed)
            .collect();
        for line in crate::lookup::said(&failed) {
            out::error(&line);
        }
    }
    Ok(if report.errors.is_empty() {
        Exit::Done
    } else {
        Exit::Failed
    })
}

pub fn history(m: &ArgMatches) -> Result<Exit, String> {
    recover();
    let h = the_history();
    match m.subcommand() {
        Some(("show", s)) => {
            let id = s
                .get_one::<String>("id")
                .expect("required by the argument parser");
            let ops = h.operations(id).map_err(|_| t("noEntry", &[("id", id)]))?;
            if ops.is_empty() && !out::json() {
                out::note(&t("undone", &[]));
            }
            if out::json() {
                println!("{}", serde_json::to_string_pretty(&ops).unwrap_or_default());
            } else {
                // By folder, as the list shows them; the whole new path
                // when the file went to another folder.
                let mut last: Option<std::path::PathBuf> = None;
                for op in ops {
                    let dir = op.from.parent().map(Path::to_path_buf).unwrap_or_default();
                    if last.as_ref() != Some(&dir) {
                        println!("{}", out::bold(&out::path(&dir)));
                        last = Some(dir.clone());
                    }
                    let name = |p: &Path| {
                        p.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    };
                    let to = if op.to.parent() == Some(dir.as_path()) {
                        name(&op.to)
                    } else {
                        out::path(&op.to)
                    };
                    println!("  {} → {}", name(&op.from), out::green(&to));
                }
            }
        }
        Some(("delete", s)) => {
            let id = s
                .get_one::<String>("id")
                .expect("required by the argument parser");
            h.delete(id).map_err(|_| t("noEntry", &[("id", id)]))?;
            out::note(&t("entryDeleted", &[("id", id)]));
        }
        Some(("clear", _)) => {
            let n = h.clear()?;
            out::note(&t("historyCleared", &[("n", &n)]));
        }
        _ => {
            let list = h.list();
            if out::json() {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&list).unwrap_or_default()
                );
            } else if list.is_empty() {
                out::note(&t("historyEmpty", &[]));
            } else {
                for e in list {
                    // As the window's History says it (`flow::said::entry`).
                    let said = renome_services::flow::said::entry(&e);
                    let state = said
                        .state
                        .map(|l| crate::lookup::said(&[l]).join(""))
                        .map(|s| if e.undone { out::dim(&s) } else { s })
                        .unwrap_or_default();
                    println!(
                        "{}  {}  {}  {}",
                        out::bold(&e.id),
                        e.created.format("%Y-%m-%d %H:%M:%S"),
                        crate::lookup::said(&said.counts).join(" · "),
                        state
                    );
                }
            }
        }
    }
    Ok(Exit::Done)
}

/// The options file as it is now, to change and save.
pub(crate) fn settings_now() -> Value {
    renome_services::read_settings_file().unwrap_or_else(|| Value::Object(Map::new()))
}

pub(crate) fn save_settings(settings: &Value) -> Result<(), String> {
    let _file = renome_services::SETTINGS_FILE.locked();
    renome_services::write_settings_file(settings)
}

fn sets_of(settings: &mut Value) -> &mut Vec<Value> {
    list_of(settings, "ruleSets")
}

/// An option holding a list (`ruleSets`, `servers`), made one if it is not.
pub(crate) fn list_of<'a>(settings: &'a mut Value, key: &str) -> &'a mut Vec<Value> {
    let obj = settings.as_object_mut().expect("settings object");
    let list = obj.entry(key).or_insert_with(|| Value::Array(vec![]));
    if !list.is_array() {
        *list = Value::Array(vec![]);
    }
    list.as_array_mut()
        .expect("the list was just made an array")
}

pub fn sets(m: &ArgMatches, settings: Option<Value>) -> Result<Exit, String> {
    match m.subcommand() {
        Some(("show", s)) => {
            let name = s
                .get_one::<String>("name")
                .expect("required by the argument parser");
            let set = rulespec::find_set(settings.as_ref(), name)
                .ok_or_else(|| t("noSet", &[("name", name)]))?;
            if out::json() {
                println!("{}", serde_json::to_string_pretty(set).unwrap_or_default());
            } else {
                let steps =
                    rulespec::steps(&[rulespec::Source::Set(name.clone())], settings.as_ref())?;
                for step in &steps {
                    println!("-r {}", shell_quote(&spec_of(step)));
                }
            }
        }
        Some(("save", s)) => {
            let name = s
                .get_one::<String>("name")
                .expect("required by the argument parser")
                .trim()
                .to_string();
            let sources = crate::sources(s);
            if sources.is_empty() {
                return Err(t("noRules", &[]));
            }
            let steps = rulespec::steps(&sources, settings.as_ref())?;
            rulespec::check_together(&steps)?;
            put_set(&name, steps, s.get_flag("replace"))?;
        }
        Some(("export", s)) => {
            let name = s
                .get_one::<String>("name")
                .expect("required by the argument parser");
            let set = rulespec::find_set(settings.as_ref(), name)
                .ok_or_else(|| t("noSet", &[("name", name)]))?;
            // As the window saves a preset: the list of steps.
            let text =
                serde_json::to_string_pretty(set.get("steps").unwrap_or(&Value::Array(vec![])))
                    .unwrap_or_default();
            match s.get_one::<String>("file").map(String::as_str) {
                Some("-") | None => println!("{text}"),
                Some(file) => std::fs::write(file, text).map_err(|e| e.to_string())?,
            }
        }
        Some(("import", s)) => {
            let file = s
                .get_one::<String>("file")
                .expect("required by the argument parser");
            let name = s.get_one::<String>("name").cloned().unwrap_or_else(|| {
                Path::new(file)
                    .file_stem()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
            let steps =
                rulespec::steps(&[rulespec::Source::File(file.clone())], settings.as_ref())?;
            rulespec::check_together(&steps)?;
            put_set(name.trim(), steps, s.get_flag("replace"))?;
        }
        Some(("delete", s)) => {
            let name = s
                .get_one::<String>("name")
                .expect("required by the argument parser");
            let mut now = settings_now();
            let sets = sets_of(&mut now);
            let at = renome_services::options::set_index(sets, name, None)
                .ok_or_else(|| t("noSet", &[("name", name)]))?;
            sets.remove(at);
            save_settings(&now)?;
            out::note(&t("setDeleted", &[("name", name)]));
        }
        Some(("rename", s)) => {
            let from = s
                .get_one::<String>("name")
                .expect("required by the argument parser");
            let to = s
                .get_one::<String>("new")
                .expect("required by the argument parser")
                .trim()
                .to_string();
            let mut now = settings_now();
            let sets = sets_of(&mut now);
            let at = renome_services::options::set_index(sets, from, None)
                .ok_or_else(|| t("noSet", &[("name", from)]))?;
            renome_services::options::rename_set(sets, at, &to)
                .map_err(|_| t("setExists", &[("name", &to)]))?;
            save_settings(&now)?;
            out::note(&t("setRenamed", &[("from", from), ("to", &to)]));
        }
        _ => {
            let sets = settings
                .as_ref()
                .and_then(|s| s.get("ruleSets")?.as_array().cloned())
                .unwrap_or_default();
            // In the window's menu order (`options::set_order`).
            let sets: Vec<&Value> = renome_services::options::set_order(&sets)
                .into_iter()
                .map(|i| &sets[i])
                .collect();
            if out::json() {
                let list: Vec<Value> = sets
                    .iter()
                    .map(|s| json!({ "name": s["name"], "rules": s["steps"].as_array().map_or(0, Vec::len) }))
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&list).unwrap_or_default()
                );
            } else {
                for s in sets {
                    let n = s["steps"].as_array().map_or(0, Vec::len);
                    println!(
                        "{}  {}",
                        out::bold(s["name"].as_str().unwrap_or_default()),
                        out::dim(&t("ruleSetCount", &[("n", &n)]))
                    );
                }
            }
        }
    }
    Ok(Exit::Done)
}

fn put_set(name: &str, steps: Vec<Value>, replace: bool) -> Result<(), String> {
    let mut now = settings_now();
    let sets = sets_of(&mut now);
    let n = steps.len();
    renome_services::options::save_set(sets, name, steps, replace)
        .map_err(|_| t("setExists", &[("name", &name)]))?;
    save_settings(&now)?;
    out::note(&t("setSaved", &[("name", &name), ("n", &n)]));
    Ok(())
}

/// A step written back as `TYPE:option=value,…`, with only the options
/// that differ from their defaults.
fn spec_of(step: &Value) -> String {
    let rule = step
        .get("rule")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let kind = rule.get("type").and_then(Value::as_str).unwrap_or_default();
    let Some(spec) = catalog::rule(kind) else {
        return kind.to_string();
    };
    let mut parts = Vec::new();
    for opt in spec.options {
        let Some(v) = rule.get(opt.name) else {
            continue;
        };
        if *v == catalog::default_value(opt, lang()) {
            continue;
        }
        let text = match (v, opt.kind) {
            (Value::Bool(true), _) => {
                parts.push(opt.name.to_string());
                continue;
            }
            (Value::Bool(false), _) => "no".to_string(),
            (Value::String(s), Kind::Choice(list)) => list
                .iter()
                .find(|c| c.value == s)
                .map_or(s.clone(), |c| c.word().to_string()),
            (Value::String(s), _) => s.replace('\n', "\\n"),
            (other, _) => other.to_string(),
        };
        parts.push(format!("{}={}", opt.name, quote_value(&text)));
    }
    if let Some(only) = step
        .get("only")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        parts.push(format!("only={}", quote_value(only)));
    }
    if step.get("enabled") == Some(&Value::Bool(false)) {
        parts.push("off".into());
    }
    if parts.is_empty() {
        kind.to_string()
    } else {
        format!("{kind}:{}", parts.join(","))
    }
}

fn quote_value(v: &str) -> String {
    if v.is_empty() || v.contains([',', '"', '\'']) || v.starts_with(' ') || v.ends_with(' ') {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        v.to_string()
    }
}

fn shell_quote(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_alphanumeric() || "-_:=,./".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

pub fn rules(m: &ArgMatches) -> Result<Exit, String> {
    let Some(kind) = m.get_one::<String>("type") else {
        if out::json() {
            let list: Vec<Value> = catalog::RULES
                .iter()
                .map(|r| json!({ "type": r.kind, "name": t(&r.label(), &[]), "options": r.options.iter().map(|o| o.name).collect::<Vec<_>>() }))
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&list).unwrap_or_default()
            );
            return Ok(Exit::Done);
        }
        println!("{}", t("rulesTitle", &[]));
        for r in catalog::RULES {
            println!("  {:<12} {}", out::bold(r.kind), t(&r.label(), &[]));
        }
        return Ok(Exit::Done);
    };
    let spec = catalog::rule(kind).ok_or_else(|| t("unknownRule", &[("name", kind)]))?;
    if out::json() {
        let options: Vec<Value> = spec
            .options
            .iter()
            .map(|o| {
                json!({
                    "name": o.name,
                    "old": o.old,
                    "kind": rulespec::kind_text(&o.kind),
                    "default": catalog::default_value(o, lang()),
                    "label": t(o.label, &[]),
                    "hint": o.hint.map(|h| t(h, &[])),
                    "whenOn": catalog::LABELS_WHEN
                        .iter()
                        .filter(|w| w.0 == spec.kind && w.1 == o.name)
                        .map(|w| json!({ "option": w.2, "label": t(w.3, &[]), "hint": w.4.map(|h| t(h, &[])) }))
                        .collect::<Vec<_>>(),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({ "type": spec.kind, "name": t(&spec.label(), &[]), "options": options })
            )
            .unwrap_or_default()
        );
        return Ok(Exit::Done);
    }
    println!("{} ({})", out::bold(&t(&spec.label(), &[])), spec.kind);
    if let Some(h) = spec.hint {
        println!("{}", t(h, &[]));
    }
    println!();
    println!("{}:", t("ruleOptions", &[]));
    for o in spec.options {
        let def = catalog::default_value(o, lang());
        let def = match def {
            Value::String(s) => match o.kind {
                Kind::Choice(list) => list
                    .iter()
                    .find(|c| c.value == s)
                    .map_or(s.clone(), |c| c.word().to_string()),
                _ if s.is_empty() || s.starts_with(' ') || s.ends_with(' ') => format!("\"{s}\""),
                _ => s,
            },
            Value::Bool(b) => (if b { "yes" } else { "no" }).to_string(),
            other => other.to_string(),
        };
        let was = o
            .old
            .map(|old| format!(" ({} {old})", t("wasNamed", &[])))
            .unwrap_or_default();
        println!("  {}{}", out::bold(o.name), out::dim(&was));
        println!("      {}", t(o.label, &[]));
        if let Some(h) = o.hint {
            println!("      {}", out::dim(&t(h, &[])));
        }
        for w in catalog::LABELS_WHEN
            .iter()
            .filter(|w| w.0 == spec.kind && w.1 == o.name)
        {
            let label = t(w.3, &[]);
            println!(
                "      {}",
                out::dim(&t("labelWhenOn", &[("option", &w.2), ("label", &label)]))
            );
        }
        println!(
            "      {} · {}: {}",
            rulespec::kind_text(&o.kind),
            t("defaultIs", &[]),
            def
        );
    }
    println!();
    println!("{}", t("everyRule", &[]));
    Ok(Exit::Done)
}

pub fn options(m: &ArgMatches, settings: Option<Value>) -> Result<Exit, String> {
    use renome_services::options::{self as opts, SETTINGS};
    let find =
        |name: &str| opts::setting(name).ok_or_else(|| t("unknownSetting", &[("name", &name)]));
    let value_of = |s: &opts::Setting| -> Value {
        settings
            .as_ref()
            .and_then(|v| v.get(s.name).cloned())
            .unwrap_or_else(|| opts::default_of(s))
    };
    match m.subcommand() {
        Some(("get", s)) => {
            let set = find(
                s.get_one::<String>("option")
                    .expect("required by the argument parser"),
            )?;
            let v = value_of(set);
            println!(
                "{}",
                if out::json() {
                    v.to_string()
                } else {
                    v.as_str().map_or(v.to_string(), str::to_string)
                }
            );
        }
        Some(("set", s)) => {
            let set = find(
                s.get_one::<String>("option")
                    .expect("required by the argument parser"),
            )?;
            let (key, kind) = (&set.name, &set.kind);
            let raw = s
                .get_one::<String>("value")
                .expect("required by the argument parser");
            let spec = catalog::RuleSpec {
                kind: "options",
                hint: None,
                whole_name: false,
                options: &[],
            };
            let opt = catalog::Opt {
                name: key,
                old: None,
                kind: *kind,
                default: catalog::Default::Text(""),
                label: "",
                hint: None,
            };
            let v = typed(&spec, &opt, raw)?;
            let mut now = settings_now();
            now[*key] = v.clone();
            save_settings(&now)?;
            out::note(&t("settingSaved", &[("name", key), ("value", &v)]));
        }
        Some(("reset", s)) => {
            let key = &find(
                s.get_one::<String>("option")
                    .expect("required by the argument parser"),
            )?
            .name;
            let mut now = settings_now();
            if let Some(obj) = now.as_object_mut() {
                obj.remove(*key);
            }
            save_settings(&now)?;
            out::note(&t("settingSaved", &[("name", key), ("value", &"—")]));
        }
        _ => {
            let mut list = Vec::new();
            for set in SETTINGS {
                let (key, kind, label, window) =
                    (&set.name, &set.kind, &set.label, &set.window_only);
                let v = value_of(set);
                if out::json() {
                    list.push(json!({ "name": key, "value": v, "windowOnly": window, "label": t(label, &[]) }));
                } else {
                    let tag = if *window {
                        out::dim(&format!(" ({})", t("windowOnly", &[])))
                    } else {
                        String::new()
                    };
                    println!(
                        "{} = {}{}",
                        out::bold(key),
                        v.as_str().map_or(v.to_string(), str::to_string),
                        tag
                    );
                    println!(
                        "    {} · {}",
                        out::dim(&t(label, &[])),
                        out::dim(&rulespec::kind_text(kind))
                    );
                }
            }
            if out::json() {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&list).unwrap_or_default()
                );
            }
        }
    }
    Ok(Exit::Done)
}

/// A value typed for an option, checked against its kind.
fn typed(rule: &catalog::RuleSpec, opt: &catalog::Opt, raw: &str) -> Result<Value, String> {
    let step = rulespec::parse_value(rule, opt, raw)?;
    Ok(step)
}

/// The keys the command line names, and the option each is saved as.
const KEYS: &[(&str, &str)] = &[
    ("tmdb", "tmdbKey"),
    ("google-books", "googleBooksKey"),
    ("comicvine", "comicVineKey"),
    ("metron", "metronToken"),
    ("discogs", "discogsToken"),
    ("opensubtitles-key", "osKey"),
    ("opensubtitles-user", "osUser"),
    ("opensubtitles-password", "osPassword"),
    ("subdl", "subdlKey"),
    ("subsource", "subsourceKey"),
    ("gcd-email", "gcdEmail"),
    ("gcd-password", "gcdPassword"),
];

/// What the Options' Test button of a service says (`checks::test`):
/// its answer, or the error as the window words it.
async fn test_key(name: &str, get: &(dyn Fn(&str) -> String + Sync)) -> Result<String, String> {
    use renome_services::checks;
    match checks::test(name, get).await {
        Ok(line) => Ok(crate::lookup::said(&[line]).concat()),
        Err(e) if e == checks::UNSET => Err(t("keyUnsetTest", &[("name", &name)])),
        Err(e) => Err(crate::text::error_text(&e)),
    }
}

/// The secrets and whether they come from the system's store.
pub fn secrets() -> Result<(Map<String, Value>, bool), String> {
    let loaded = renome_services::load_secrets();
    match loaded.get("status").and_then(Value::as_str) {
        Some("stored") => Ok((
            loaded
                .get("secrets")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default(),
            true,
        )),
        Some("missing") => Ok((
            renome_services::read_settings_file()
                .and_then(|s| s.as_object().map(renome_services::secrets::of))
                .unwrap_or_default(),
            false,
        )),
        _ => Err(t("keyStoreRefused", &[])),
    }
}

/// A key by the name typed (its own or its field's, any case).
fn find_key(name: &str) -> Result<&'static (&'static str, &'static str), String> {
    let want = catalog::squash(name);
    KEYS.iter()
        .find(|(k, field)| catalog::squash(k) == want || catalog::squash(field) == want)
        .ok_or_else(|| t("unknownKey", &[("name", &name)]))
}

pub fn keys(m: &ArgMatches) -> Result<Exit, String> {
    let key_arg = |s: &ArgMatches| {
        find_key(
            s.get_one::<String>("key")
                .expect("required by the argument parser"),
        )
    };
    match m.subcommand() {
        Some(("set", s)) => {
            let (name, field) = key_arg(s)?;
            set_key(name, field, s.get_flag("stdin"))?;
        }
        Some(("test", s)) => test_one_key(key_arg(s)?.0)?,
        Some(("delete", s)) => {
            let (name, field) = key_arg(s)?;
            let (mut all, _) = secrets()?;
            all.insert((*field).into(), Value::from(""));
            renome_services::save_secrets(all)?;
            out::note(&t("keyDeleted", &[("name", name)]));
        }
        _ => list_keys()?,
    }
    Ok(Exit::Done)
}

/// Keeps a key typed (or piped), and says where it went.
fn set_key(name: &str, field: &str, stdin: bool) -> Result<(), String> {
    let value = if stdin {
        let mut v = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut v).map_err(|e| e.to_string())?;
        v.trim().to_string()
    } else {
        rpassword::prompt_password(t("keyPrompt", &[("name", &name)]))
            .map_err(|e| e.to_string())?
            .trim()
            .to_string()
    };
    let (mut all, had_store) = secrets()?;
    all.insert(field.into(), Value::from(value));
    let stored = renome_services::save_secrets(all)?;
    out::note(&t(
        match (stored, had_store) {
            (true, _) => "keyStored",
            // The store answered when read but refused the write (macOS
            // over SSH, a locked keychain).
            (false, true) => "keyStoreWouldNot",
            (false, false) => "keyInFile",
        },
        &[("name", &name)],
    ));
    Ok(())
}

/// Asks the service whether the key works.
fn test_one_key(name: &str) -> Result<(), String> {
    let (all, _) = secrets()?;
    let get = |f: &str| {
        all.get(f)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let ok = rt.block_on(test_key(name, &get))?;
    out::note(&out::green(&ok));
    Ok(())
}

/// Each key: set or not, and where it is kept (never its value).
fn list_keys() -> Result<(), String> {
    let (all, store) = secrets()?;
    // Keys the store did not take stay in the settings file.
    let in_file = renome_services::read_settings_file()
        .and_then(|s| s.as_object().map(renome_services::secrets::of))
        .unwrap_or_default();
    let filled = |m: &Map<String, Value>, field: &str| {
        m.get(field)
            .and_then(Value::as_str)
            .is_some_and(|v| !v.is_empty())
    };
    // (name, set, in the store)
    let mut rows: Vec<(String, bool, bool)> = KEYS
        .iter()
        .map(|(name, field)| {
            (
                (*name).to_string(),
                filled(&all, field),
                store && !filled(&in_file, field),
            )
        })
        .collect();
    // Each media server's key (kept together in `serverKeys`).
    let server_keys: Map<String, Value> = all
        .get("serverKeys")
        .and_then(Value::as_str)
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or_default();
    let servers = renome_services::read_settings_file()
        .and_then(|s| s.get("servers")?.as_array().cloned())
        .unwrap_or_default();
    for srv in servers {
        let id = srv.get("id").and_then(Value::as_str).unwrap_or_default();
        let url = srv.get("url").and_then(Value::as_str).unwrap_or_default();
        let set = server_keys
            .get(id)
            .and_then(Value::as_str)
            .is_some_and(|v| !v.trim().is_empty());
        rows.push((
            format!("server {}", if url.is_empty() { id } else { url }),
            set,
            store && !filled(&in_file, "serverKeys"),
        ));
    }
    let mut list = Vec::new();
    for (name, set, in_store) in rows {
        let where_ = t(
            if in_store {
                "keyWhereStore"
            } else {
                "keyWhereFile"
            },
            &[],
        );
        if out::json() {
            list.push(json!({ "name": name, "set": set, "store": set && in_store }));
        } else if set {
            println!(
                "{:<24} {} {}",
                name,
                out::green(&t("keySetHere", &[])),
                out::dim(&where_)
            );
        } else {
            println!("{:<24} {}", name, out::dim(&t("keyUnset", &[])));
        }
    }
    if out::json() {
        println!(
            "{}",
            serde_json::to_string_pretty(&list).unwrap_or_default()
        );
    }
    Ok(())
}

pub fn cache(m: &ArgMatches) -> Result<Exit, String> {
    match m.subcommand() {
        Some(("clear", _)) => {
            renome_services::ANSWERS.clear();
            out::note(&t("cacheCleared", &[]));
        }
        _ => {
            let size = renome_services::ANSWERS.size();
            if out::json() {
                println!("{}", json!({ "bytes": size }));
            } else {
                println!("{}", t("cacheSizeIs", &[("size", &out::size(size))]));
            }
        }
    }
    Ok(Exit::Done)
}
