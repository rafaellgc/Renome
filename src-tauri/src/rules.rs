//! The window's commands: rules.

use super::*;

/// Everything the window builds rules, their menus and its options from,
/// in this language (the command line's too): see `catalog::table` and
/// `options::table`, and the details windows' fields.
/// The "+ Rule" menu's groups in their order, by the rules' names as
/// the window says them (the shared order: `catalog::menu`, the command
/// line's manual too).
#[tauri::command]
pub(crate) fn rule_menu(names: HashMap<String, String>) -> (Vec<&'static str>, Vec<&'static str>) {
    renome_core::catalog::menu(|k| names.get(k).cloned().unwrap_or_default())
}

/// The manual's pages in order, by their ids (`catalog::manual_order`, the
/// command line's `manual` too): each page's place, and a group's title.
#[tauri::command]
pub(crate) fn manual_order(
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
pub(crate) fn rule_reader(rule: Value) -> renome_core::readers::Reader {
    renome_core::readers::reader(&rule)
}

#[tauri::command]
pub(crate) fn rule_table(lang: String) -> Value {
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

/// How a rule's card shows its options as the rule is set: those it
/// hides (`catalog::hidden`: under another that leaves them nothing to
/// do, and the inert ones) and their labels (`catalog::labels_of`).
#[tauri::command]
pub(crate) fn rule_view(rule: Value) -> Value {
    serde_json::json!({
        "hidden": renome_core::catalog::hidden(&rule),
        "labels": renome_core::catalog::labels_of(&rule),
    })
}

/// The list's optional columns that make sense with these rules.
#[tauri::command]
pub(crate) fn columns_for(steps: Vec<Value>) -> Vec<&'static str> {
    renome_core::catalog::columns_for(&steps)
}

/// Why each rule cannot join the rules there (`types`), when it cannot.
#[tauri::command]
pub(crate) fn rule_blocks(
    types: Vec<String>,
) -> HashMap<String, Option<renome_core::catalog::Blocked>> {
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
pub(crate) fn complete_steps(
    steps: Vec<Value>,
    lang: String,
    saved: bool,
) -> (Vec<Value>, Option<renome_services::flow::after::Line>) {
    let (steps, dropped) = renome_core::catalog::complete_steps(steps, &lang, saved);
    (steps, renome_services::flow::said::dropped(&dropped))
}

/// The rule sets with rules saved under a name (see `options::save_set`,
/// the command line's `sets` too); refused with `exists:<place>` when one
/// has that name and it is not to be replaced.
#[tauri::command]
pub(crate) fn sets_save(
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
/// The sets' places in the order the menu lists them (`options::set_order`).
#[tauri::command]
pub(crate) fn sets_order(sets: Vec<Value>) -> Vec<usize> {
    renome_services::options::set_order(&sets)
}

#[tauri::command]
pub(crate) fn sets_rename(
    mut sets: Vec<Value>,
    at: usize,
    name: String,
) -> Result<Vec<Value>, String> {
    renome_services::options::rename_set(&mut sets, at, &name)
        .map_err(|at| format!("exists:{at}"))?;
    Ok(sets)
}

#[tauri::command]
pub(crate) fn save_preset(path: PathBuf, mut steps: Vec<serde_json::Value>) -> Result<(), String> {
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
pub(crate) fn save_csv(path: PathBuf, rows: Vec<Vec<String>>, lang: String) -> Result<(), String> {
    std::fs::write(path, renome_core::text::csv(&rows, &lang)).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn load_preset(
    path: PathBuf,
    lang: String,
) -> Result<
    (
        Vec<serde_json::Value>,
        Option<renome_services::flow::after::Line>,
    ),
    String,
> {
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
    Ok((steps, renome_services::flow::said::dropped(&dropped)))
}
