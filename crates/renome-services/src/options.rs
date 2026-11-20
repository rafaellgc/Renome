//! The program's options (the window's Options, `renome-cli options`):
//! each one's kind, default and label, and the defaults of all that the
//! settings file holds. The window and the command line read them here.

use renome_core::catalog::{Choice, Default, Kind};
use serde_json::{Map, Value};

/// One option.
pub struct Setting {
    /// Its name in the settings file.
    pub name: &'static str,
    pub kind: Kind,
    pub default: Default,
    /// Key of its label in the page's texts.
    pub label: &'static str,
    /// Only the window uses it.
    pub window_only: bool,
}

const fn c(value: &'static str, label: &'static str) -> Choice {
    Choice {
        value,
        word: "",
        label,
    }
}

pub const SETTINGS: &[Setting] = &[
    Setting {
        name: "lang",
        kind: Kind::Choice(&[
            c("pt", "language"),
            c("en", "language"),
            c("es", "language"),
        ]),
        default: Default::Text("en"),
        label: "language",
        window_only: false,
    },
    Setting {
        name: "includeHidden",
        kind: Kind::Bool,
        default: Default::Bool(false),
        label: "includeHidden",
        window_only: false,
    },
    Setting {
        name: "warnWindows",
        kind: Kind::Bool,
        default: Default::Bool(true),
        label: "warnWindows",
        window_only: false,
    },
    Setting {
        name: "numberClashes",
        kind: Kind::Bool,
        default: Default::Bool(false),
        label: "numberClashes",
        window_only: false,
    },
    Setting {
        name: "tmdbLang",
        kind: Kind::Choice(&[
            Choice {
                value: "",
                word: "auto",
                label: "tmdbLangAuto",
            },
            c("pt-BR", "tmdbLangLabel"),
            c("pt-PT", "tmdbLangLabel"),
            c("en-US", "tmdbLangLabel"),
            c("en-GB", "tmdbLangLabel"),
            c("es-ES", "tmdbLangLabel"),
            c("es-MX", "tmdbLangLabel"),
            c("fr-FR", "tmdbLangLabel"),
            c("it-IT", "tmdbLangLabel"),
            c("de-DE", "tmdbLangLabel"),
            c("ja-JP", "tmdbLangLabel"),
        ]),
        default: Default::Text(""),
        label: "tmdbLangLabel",
        window_only: false,
    },
    Setting {
        name: "cacheDays",
        kind: Kind::IntChoice(&[0, 1, 7, 30, 180]),
        default: Default::Int(7),
        label: "cacheDays",
        window_only: false,
    },
    Setting {
        name: "checkUpdates",
        kind: Kind::Bool,
        default: Default::Bool(true),
        label: "checkUpdates",
        window_only: false,
    },
    Setting {
        name: "theme",
        kind: Kind::Choice(&[
            c("light", "themeLight"),
            c("dark", "themeDark"),
            c("system", "themeSystem"),
        ]),
        default: Default::Text("system"),
        label: "theme",
        window_only: true,
    },
    Setting {
        name: "textSize",
        kind: Kind::Choice(&[
            c("normal", "textSizeNormal"),
            c("large", "textSizeLarge"),
            c("larger", "textSizeLarger"),
        ]),
        default: Default::Text("normal"),
        label: "textSize",
        window_only: true,
    },
    Setting {
        name: "confirmApply",
        kind: Kind::Bool,
        default: Default::Bool(true),
        label: "confirmApply",
        window_only: true,
    },
    Setting {
        name: "disableRulesAfterApply",
        kind: Kind::Bool,
        default: Default::Bool(true),
        label: "disableAfterApply",
        window_only: true,
    },
    Setting {
        name: "rememberRules",
        kind: Kind::Bool,
        default: Default::Bool(true),
        label: "rememberRules",
        window_only: true,
    },
    Setting {
        name: "showMissing",
        kind: Kind::Bool,
        default: Default::Bool(true),
        label: "showMissingHint",
        window_only: true,
    },
];

/// The option called so (camelCase or kebab-case, case ignored).
pub fn setting(name: &str) -> Option<&'static Setting> {
    let want = renome_core::catalog::squash(name);
    SETTINGS
        .iter()
        .find(|s| renome_core::catalog::squash(s.name) == want)
}

/// An option's value when not saved.
pub fn default_of(s: &Setting) -> Value {
    match s.default {
        Default::Text(t) => Value::from(t),
        Default::Lang(by) => Value::from(by[1]),
        Default::Bool(b) => Value::from(b),
        Default::Int(n) => Value::from(n),
        Default::Float(f) => Value::from(f),
    }
}

/// Everything the settings file holds, at its default: the options, the
/// keys and accounts (empty: not set up), the media servers, the rules
/// remembered, the rule sets and the list's columns.
pub fn defaults() -> Map<String, Value> {
    let mut out: Map<String, Value> = SETTINGS
        .iter()
        .map(|s| (s.name.to_string(), default_of(s)))
        .collect();
    for k in crate::secrets::FIELDS {
        out.insert(k.to_string(), Value::from(""));
    }
    for k in ["servers", "rules", "ruleSets", "columns"] {
        out.insert(k.to_string(), Value::Array(Vec::new()));
    }
    out
}

/// The settings as saved, whole: each one missing at its default, the
/// remembered rules made ready to use (see
/// [`renome_core::catalog::complete_steps`]).
pub fn whole(saved: Option<Value>) -> Value {
    let mut out = defaults();
    if let Some(Value::Object(saved)) = saved {
        out.extend(saved);
    }
    let lang = out["lang"].as_str().unwrap_or("en").to_string();
    if let Some(Value::Array(rules)) = out.remove("rules") {
        let (steps, _) = renome_core::catalog::complete_steps(rules, &lang, false);
        out.insert("rules".into(), Value::Array(steps));
    }
    Value::Object(out)
}

/// The language TMDB titles are asked in: the one chosen, else the
/// interface's.
pub fn tmdb_lang(settings: Option<&Value>, ui: &str) -> String {
    settings
        .and_then(|s| s.get("tmdbLang")?.as_str())
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            match ui {
                "pt" => "pt-BR",
                "es" => "es-ES",
                _ => "en-US",
            }
            .to_string()
        })
}

/// The language the comic catalogues are asked in (`pt-br`, `en`, `es`).
pub fn comic_lang(ui: &str) -> String {
    if ui == "pt" {
        "pt-br".into()
    } else {
        ui.to_string()
    }
}

/// The key a comic catalogue is asked with: Comic Vine's, Metron's token.
pub fn comic_key_name(source: &str) -> &'static str {
    if source == "metron" {
        "metronToken"
    } else {
        "comicVineKey"
    }
}

/// Everything the window reads its options from: the options, the
/// fields kept in the password store, each menu's choices (words with
/// their labels' keys, numbers as they are; the TMDB languages, empty
/// following the interface).
pub fn table() -> Value {
    let menus: Map<String, Value> = SETTINGS
        .iter()
        .filter_map(|s| match s.kind {
            Kind::Choice(list) => Some((
                s.name.to_string(),
                list.iter()
                    .map(|c| serde_json::json!({ "value": c.value, "label": c.label }))
                    .collect(),
            )),
            Kind::IntChoice(list) => Some((s.name.to_string(), Value::from(list.to_vec()))),
            _ => None,
        })
        .collect();
    serde_json::json!({
        "defaults": defaults(),
        "secrets": crate::secrets::FIELDS,
        "menus": menus,
        "serverKinds": crate::mediaserver::KINDS,
        "comicKeys": { "comicvine": comic_key_name("comicvine"), "metron": comic_key_name("metron") },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_rules_come_whole() {
        let saved = serde_json::json!({
            "lang": "pt",
            "rules": [{ "id": "a", "enabled": true, "rule": { "type": "number", "template": "{n}", "start": 1, "step": 1, "digits": 2 } }],
        });
        let w = whole(Some(saved));
        let r = &w["rules"][0]["rule"];
        // Saved before pairs: counted apart.
        assert_eq!(r["pairs"], false);
        assert_eq!(w["includeHidden"], false);
        assert_eq!(w["servers"], serde_json::json!([]));
    }

    #[test]
    fn languages() {
        assert_eq!(tmdb_lang(None, "pt"), "pt-BR");
        assert_eq!(
            tmdb_lang(Some(&serde_json::json!({"tmdbLang": "fr-FR"})), "pt"),
            "fr-FR"
        );
        assert_eq!(comic_lang("pt"), "pt-br");
    }

    #[test]
    fn the_window_gets_each_menu() {
        let t = table();
        assert_eq!(
            t["menus"]["cacheDays"],
            serde_json::json!([0, 1, 7, 30, 180])
        );
        assert_eq!(t["menus"]["tmdbLang"][0]["value"], "");
        assert_eq!(t["menus"]["theme"][2]["label"], "themeSystem");
    }
}

/// The rule sets saved by name (`ruleSets`: `[{name, steps}]`), case
/// ignored: the place of the one called so, other than `except`.
pub fn set_index(sets: &[Value], name: &str, except: Option<usize>) -> Option<usize> {
    let want = name.trim().to_lowercase();
    sets.iter().enumerate().position(|(i, s)| {
        Some(i) != except && s["name"].as_str().is_some_and(|n| n.to_lowercase() == want)
    })
}

/// Saves rules as a set; one of that name is replaced when asked (it
/// keeps its name as it was written), else its place is returned.
pub fn save_set(
    sets: &mut Vec<Value>,
    name: &str,
    steps: Vec<Value>,
    replace: bool,
) -> Result<(), usize> {
    let name = name.trim();
    match set_index(sets, name, None) {
        Some(at) if !replace => Err(at),
        Some(at) => {
            let kept = sets[at]["name"].clone();
            sets[at] = serde_json::json!({ "name": kept, "steps": steps });
            Ok(())
        }
        None => {
            sets.push(serde_json::json!({ "name": name, "steps": steps }));
            Ok(())
        }
    }
}

/// Renames a set; refused when another has that name (its place).
pub fn rename_set(sets: &mut [Value], at: usize, name: &str) -> Result<(), usize> {
    if let Some(other) = set_index(sets, name, Some(at)) {
        return Err(other);
    }
    if let Some(s) = sets.get_mut(at) {
        s["name"] = Value::from(name.trim());
    }
    Ok(())
}

#[cfg(test)]
mod set_tests {
    use super::*;

    #[test]
    fn sets_by_name() {
        let mut sets = vec![];
        save_set(&mut sets, "Filmes", vec![], false).unwrap();
        assert_eq!(save_set(&mut sets, "filmes", vec![], false), Err(0));
        save_set(&mut sets, "filmes", vec![Value::Null], true).unwrap();
        assert_eq!(sets[0]["name"], "Filmes");
        save_set(&mut sets, "Séries", vec![], false).unwrap();
        assert_eq!(rename_set(&mut sets, 1, "FILMES"), Err(0));
        rename_set(&mut sets, 0, "filmes").unwrap();
        assert_eq!(sets[0]["name"], "filmes");
    }
}
