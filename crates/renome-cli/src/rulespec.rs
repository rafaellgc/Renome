//! Rules from the command line: `TYPE[:option=value,…]` written by hand,
//! saved sets, JSON files and the window's remembered rules, made into the
//! steps the window would make (every option at its default unless given,
//! checked against [`renome_core::catalog`]).

use renome_core::catalog::{self, Kind, RuleSpec};
use serde_json::{Map, Value};

use crate::text::{lang, t};

/// Where a list of steps comes from, in the order given.
pub enum Source {
    Spec(String),
    Set(String),
    File(String),
    Saved,
}

/// The steps from all the sources, in order, brought up to date and with
/// the options they leave out at their defaults.
pub fn steps(sources: &[Source], settings: Option<&Value>) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for source in sources {
        match source {
            Source::Spec(spec) => out.push(parse(spec)?),
            Source::Set(name) => {
                let set = find_set(settings, name).ok_or_else(|| t("noSet", &[("name", name)]))?;
                out.extend(complete(
                    set.get("steps").cloned().unwrap_or(Value::Array(vec![])),
                )?);
            }
            Source::File(path) => {
                let text = if path == "-" {
                    let mut s = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)
                        .map_err(|e| e.to_string())?;
                    s
                } else {
                    std::fs::read_to_string(path)
                        .map_err(|e| t("badFile", &[("file", path), ("error", &e)]))?
                };
                let v: Value = serde_json::from_str(&text)
                    .map_err(|e| t("badFile", &[("file", path), ("error", &e)]))?;
                out.extend(complete(steps_of(v))?);
            }
            Source::Saved => {
                let saved = settings
                    .and_then(|s| s.get("rules"))
                    .cloned()
                    .unwrap_or(Value::Array(vec![]));
                if saved.as_array().is_none_or(Vec::is_empty) {
                    return Err(t("noSaved", &[]));
                }
                out.extend(complete(saved)?);
            }
        }
    }
    Ok(out)
}

/// A set saved in the options, by name (case ignored).
pub fn find_set<'a>(settings: Option<&'a Value>, name: &str) -> Option<&'a Value> {
    settings?.get("ruleSets")?.as_array()?.iter().find(|s| {
        s.get("name")
            .and_then(Value::as_str)
            .is_some_and(|n| n.to_lowercase() == name.to_lowercase())
    })
}

/// The steps in a JSON file: a list of steps, a set (`{name, steps}`), or
/// a list of sets (the first).
pub fn steps_of(v: Value) -> Value {
    match v {
        Value::Object(mut o) => o.remove("steps").unwrap_or(Value::Array(vec![])),
        Value::Array(items) if items.first().is_some_and(|i| i.get("steps").is_some()) => items
            .into_iter()
            .next()
            .and_then(|mut s| s.get_mut("steps").map(Value::take))
            .unwrap_or_default(),
        other => other,
    }
}

/// Saved steps with old option names updated and the options they leave
/// out at their defaults (as the window's `newStep`).
fn complete(mut steps: Value) -> Result<Vec<Value>, String> {
    catalog::migrate_steps(&mut steps);
    let Value::Array(items) = steps else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    for mut step in items {
        let Some(rule) = step.get_mut("rule").and_then(Value::as_object_mut) else {
            continue;
        };
        let kind = rule
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let spec = catalog::rule(&kind).ok_or_else(|| t("unknownRule", &[("name", &kind)]))?;
        let mut full = catalog::new_rule(spec, lang());
        full.extend(std::mem::take(rule));
        *rule = full;
        out.push(step);
    }
    Ok(out)
}

/// `TYPE[:option=value,…]` made into a step.
pub fn parse(spec: &str) -> Result<Value, String> {
    let (kind, rest) = spec.split_once(':').unwrap_or((spec, ""));
    let rule_spec =
        catalog::rule(kind.trim()).ok_or_else(|| t("unknownRule", &[("name", &kind.trim())]))?;
    let mut rule = catalog::new_rule(rule_spec, lang());
    let mut enabled = true;
    let mut only = String::new();
    for (key, value) in pairs(rest).map_err(|_| t("badSpec", &[("spec", &spec)]))? {
        match catalog::squash(&key).as_str() {
            "only" => only = value.unwrap_or_default(),
            "off" => enabled = value.as_deref().map(boolean).unwrap_or(Some(true)) != Some(true),
            _ => {
                let opt = rule_spec.option(&key).ok_or_else(|| {
                    t(
                        "unknownOption",
                        &[("rule", &rule_spec.kind), ("name", &key)],
                    )
                })?;
                rule.insert(opt.name.into(), value_of(rule_spec, opt, value)?);
            }
        }
    }
    let mut step = Map::new();
    step.insert("id".into(), Value::from(format!("cli{}", rand_id())));
    step.insert("enabled".into(), Value::from(enabled));
    step.insert("rule".into(), Value::Object(rule));
    if !only.is_empty() {
        step.insert("only".into(), Value::from(only));
    }
    Ok(Value::Object(step))
}

/// A short id, different for each step of a run.
fn rand_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{:x}{}", now.as_millis(), N.fetch_add(1, Ordering::Relaxed))
}

fn boolean(s: &str) -> Option<bool> {
    match s.trim().to_lowercase().as_str() {
        "yes" | "y" | "true" | "on" | "1" | "sim" | "s" | "sí" | "si" => Some(true),
        "no" | "n" | "false" | "off" | "0" | "não" | "nao" => Some(false),
        _ => None,
    }
}

/// A value typed for an option, checked against its kind.
pub fn parse_value(rule: &RuleSpec, opt: &catalog::Opt, raw: &str) -> Result<Value, String> {
    value_of(rule, opt, Some(raw.to_string()))
}

/// What the person typed for an option, as the rule saves it.
fn value_of(rule: &RuleSpec, opt: &catalog::Opt, value: Option<String>) -> Result<Value, String> {
    let bad = |v: &str| {
        t(
            "badValue",
            &[
                ("name", &format!("{}.{}", rule.kind, opt.name)),
                ("value", &v),
                ("kind", &kind_text(&opt.kind)),
            ],
        )
    };
    match opt.kind {
        Kind::Bool => match value {
            None => Ok(Value::Bool(true)),
            Some(v) => boolean(&v).map(Value::Bool).ok_or_else(|| bad(&v)),
        },
        Kind::Text | Kind::Template => Ok(Value::from(value.unwrap_or_default())),
        Kind::Lines => Ok(Value::from(value.unwrap_or_default().replace("\\n", "\n"))),
        Kind::Int { min, max } => {
            let v = value.unwrap_or_default();
            match v.trim().parse::<i64>() {
                Ok(n) if n >= min && n <= max => Ok(Value::from(n)),
                _ => Err(bad(&v)),
            }
        }
        Kind::IntChoice(list) => {
            let v = value.unwrap_or_default();
            match v.trim().parse::<i64>() {
                Ok(n) if list.contains(&n) => Ok(Value::from(n)),
                _ => Err(bad(&v)),
            }
        }
        Kind::Float => {
            let v = value.unwrap_or_default();
            match v.trim().replace(',', ".").parse::<f64>() {
                Ok(n) if n >= 0.0 && n.is_finite() => Ok(Value::from(n)),
                _ => Err(bad(&v)),
            }
        }
        Kind::Choice(list) => {
            let v = value.unwrap_or_default();
            let want = catalog::squash(&v);
            list.iter()
                .find(|c| {
                    catalog::squash(c.word()) == want
                        || (!c.value.is_empty() && catalog::squash(c.value) == want)
                })
                .map(|c| Value::from(c.value))
                .ok_or_else(|| bad(&v))
        }
    }
}

/// What an option takes, in words.
pub fn kind_text(kind: &Kind) -> String {
    match kind {
        Kind::Text => t("kindText", &[]),
        Kind::Template => t("kindTemplate", &[]),
        Kind::Lines => t("kindLines", &[]),
        Kind::Bool => t("kindBool", &[]),
        Kind::Int { min, max } if *min > i64::MIN / 2 => {
            format!("{} {min}…{max}", t("kindInt", &[]))
        }
        Kind::Int { .. } => t("kindInt", &[]),
        Kind::Float => t("kindFloat", &[]),
        Kind::IntChoice(list) => {
            format!(
                "{} {}",
                t("kindChoice", &[]),
                list.iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Kind::Choice(list) => format!(
            "{} {}",
            t("kindChoice", &[]),
            list.iter().map(|c| c.word()).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// `key=value,key="a, b",flag` as pairs; a key alone has no value.
fn pairs(text: &str) -> Result<Vec<(String, Option<String>)>, ()> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| *c == ',' || c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            return Ok(out);
        }
        let mut key = String::new();
        while let Some(&c) = chars.peek() {
            if c == '=' || c == ',' {
                break;
            }
            key.push(c);
            chars.next();
        }
        let key = key.trim().to_string();
        if key.is_empty() {
            return Err(());
        }
        if chars.peek() == Some(&'=') {
            chars.next();
            let mut value = String::new();
            let mut quote: Option<char> = None;
            let mut quoted = false;
            while let Some(c) = chars.next() {
                match (quote, c) {
                    (None, '"' | '\'') if value.is_empty() && !quoted => {
                        quote = Some(c);
                        quoted = true;
                    }
                    (Some(q), c) if c == q => quote = None,
                    (Some(_), '\\') => {
                        if let Some(n) = chars.next() {
                            value.push(n);
                        }
                    }
                    (None, ',') => break,
                    (_, c) => value.push(c),
                }
            }
            if quote.is_some() {
                return Err(());
            }
            out.push((key, Some(value)));
        } else {
            out.push((key, None));
        }
    }
}

/// The type of the rule a step holds.
pub fn kind_of(step: &Value) -> &str {
    step.get("rule")
        .and_then(|r| r.get("type"))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// Checks what the window checks when rules are put together: one rule
/// that makes the whole name at a time.
pub fn check_together(steps: &[Value]) -> Result<(), String> {
    let wholes: Vec<&str> = steps
        .iter()
        .map(kind_of)
        .filter(|k| catalog::rule(k).is_some_and(|r| r.whole_name))
        .collect();
    if wholes.len() > 1 {
        let name = |k: &str| t(&format!("rule.{k}"), &[]);
        return Err(t(
            "twoWhole",
            &[("a", &name(wholes[0])), ("b", &name(wholes[1]))],
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_rules_become_steps() {
        let _turn = crate::text::TEST_LANG.lock().unwrap();
        crate::text::set_lang("en");
        let s = parse(r#"replace:find=_,replace=" ",first-only"#).unwrap();
        let r = &s["rule"];
        assert_eq!(r["type"], "replace");
        assert_eq!(r["find"], "_");
        assert_eq!(r["replace"], " ");
        assert_eq!(r["firstOnly"], true);
        assert_eq!(r["target"], "name");
        assert_eq!(s["enabled"], true);

        let s =
            parse("movie:sidecar=yes,colon=' - ',sub-langs=\"pt-BR, en\",only=*.mkv,off").unwrap();
        assert_eq!(s["rule"]["nfo"], true);
        assert_eq!(s["rule"]["colonAs"], " - ");
        assert_eq!(s["rule"]["subLangs"], "pt-BR, en");
        assert_eq!(s["rule"]["template"], "{title} ({year}) [tmdbid-{id}]");
        assert_eq!(s["only"], "*.mkv");
        assert_eq!(s["enabled"], false);

        assert_eq!(
            parse("convert:web-safe=dash").unwrap()["rule"]["webSafe"],
            "-"
        );
        assert_eq!(parse("list:names=a\\nb").unwrap()["rule"]["names"], "a\nb");
        assert!(parse("case:mode=shouting").is_err());
        assert!(parse("number:digits=x").is_err());
        assert!(parse("nothing").is_err());
        assert!(parse("replace:nope=1").is_err());
        assert!(parse("replace:find=\"open").is_err());
        // What the core reads.
        let step: renome_core::Step =
            serde_json::from_value(parse("case:mode=upper").unwrap()).unwrap();
        assert!(step.enabled);
    }
}
