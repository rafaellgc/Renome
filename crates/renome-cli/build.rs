//! The page's texts (`src/lib/i18n.ts`) made into tables, so the command
//! line says things as the window does, in the same three languages,
//! without a second copy to keep in step.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let i18n = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/lib/i18n.ts");
    println!("cargo::rerun-if-changed={}", i18n.display());
    let text = std::fs::read_to_string(&i18n).expect("src/lib/i18n.ts");
    let mut out = String::new();
    for (lang, start) in [
        ("PT", "const pt = {"),
        ("EN", "const en: Dict = {"),
        ("ES", "const es: Dict = {"),
    ] {
        let from = text
            .find(start)
            .unwrap_or_else(|| panic!("{start} in i18n.ts"));
        let body = &text[from + start.len()..];
        let body = &body[..body.find("\n};").expect("end of a dictionary")];
        let mut pairs: Vec<(String, String)> = Vec::new();
        for line in body.lines() {
            let Some((key, value)) = entry(line) else {
                continue;
            };
            pairs.retain(|(k, _)| *k != key);
            pairs.push((key, value));
        }
        pairs.sort();
        writeln!(out, "pub static {lang}: &[(&str, &str)] = &[").unwrap();
        for (k, v) in &pairs {
            writeln!(out, "    ({k:?}, {v:?}),").unwrap();
        }
        writeln!(out, "];").unwrap();
    }
    // The public key updates are checked against: the window's.
    let conf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/tauri.conf.json");
    println!("cargo::rerun-if-changed={}", conf.display());
    let conf: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&conf).expect("tauri.conf.json")).unwrap();
    let key = conf["plugins"]["updater"]["pubkey"]
        .as_str()
        .expect("updater pubkey");
    let endpoint = conf["plugins"]["updater"]["endpoints"][0]
        .as_str()
        .expect("updater endpoint");
    let update = format!(
        "pub static UPDATE_KEY: &str = {key:?};\npub static UPDATE_URL: &str = {endpoint:?};\n"
    );
    std::fs::write(
        Path::new(&std::env::var("OUT_DIR").unwrap()).join("update.rs"),
        update,
    )
    .unwrap();
    manual();
    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join("texts.rs");
    std::fs::write(dest, out).unwrap();
}

/// `  key: "text",` or `  "dotted.key": "text",` read as key and text.
fn entry(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    let (key, rest) = line.split_once(": ")?;
    let key = key.trim_matches('"');
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return None;
    }
    let literal = rest.strip_suffix(',').unwrap_or(rest);
    if !literal.starts_with('"') {
        return None;
    }
    let value: String = serde_json::from_str(literal).ok()?;
    Some((key.to_string(), value))
}

/// The window's manual (`src/lib/help/*.ts`, HTML in template strings)
/// as plain text: one logical line per paragraph, item or table row,
/// wrapped to the terminal when shown.
fn manual() {
    let mut out = String::new();
    for lang in ["pt", "en", "es"] {
        let file =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../src/lib/help/{lang}.ts"));
        println!("cargo::rerun-if-changed={}", file.display());
        let src = std::fs::read_to_string(&file).expect("manual");
        writeln!(
            out,
            "pub static MANUAL_{}: &[(&str, &str, bool, &str)] = &[",
            lang.to_uppercase()
        )
        .unwrap();
        let mut rest = src.as_str();
        while let Some(at) = rest.find("    id: \"") {
            rest = &rest[at + 9..];
            let id = &rest[..rest.find('"').unwrap()];
            let title_at = rest.find("title: ").unwrap();
            let (title, after) = js_string(&rest[title_at + 7..]);
            let html_at = after.find("html: `").unwrap();
            let sub = after[..html_at].contains("sub: true");
            let (html, after) = template(&after[html_at + 7..]);
            writeln!(out, "    ({id:?}, {title:?}, {sub}, {:?}),", to_text(&html)).unwrap();
            rest = after;
        }
        writeln!(out, "];").unwrap();
    }
    std::fs::write(
        Path::new(&std::env::var("OUT_DIR").unwrap()).join("manual.rs"),
        out,
    )
    .unwrap();
}

/// A "…" string at the start: its value and what follows.
fn js_string(s: &str) -> (String, &str) {
    let quote = s.chars().next().unwrap();
    let mut value = String::new();
    let mut chars = s.char_indices().skip(1);
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => value.push(chars.next().unwrap().1),
            c if c == quote => return (value, &s[i + 1..]),
            c => value.push(c),
        }
    }
    panic!("unterminated string");
}

/// A `…` template at the start (opening backtick already taken): its
/// value with `${ex(a, b)}` written out, and what follows.
fn template(s: &str) -> (String, &str) {
    let mut value = String::new();
    let mut i = 0;
    let b = s.as_bytes();
    while i < b.len() {
        match b[i] {
            b'\\' => {
                let c = s[i + 1..].chars().next().unwrap();
                value.push(c);
                i += 1 + c.len_utf8();
            }
            b'`' => return (value, &s[i + 1..]),
            b'$' if s[i..].starts_with("${ex(") => {
                let (from, rest) = js_string(&s[i + 5..]);
                let rest = rest.trim_start().strip_prefix(',').unwrap().trim_start();
                let (to, rest) = js_string(rest);
                let rest = rest.trim_start().strip_prefix(")}").unwrap();
                write!(value, "<ex>{}\u{1}{}</ex>", from, to).unwrap();
                i = s.len() - rest.len();
            }
            _ => {
                let c = s[i..].chars().next().unwrap();
                value.push(c);
                i += c.len_utf8();
            }
        }
    }
    panic!("unterminated template");
}

/// HTML to lines: headings marked with `#`, items with `•` or numbers,
/// rows as cells joined by ` — `, examples as `old → new`.
fn to_text(html: &str) -> String {
    let mut out = String::new();
    let mut line = String::new();
    let mut lists: Vec<Option<usize>> = Vec::new();
    let mut cells = 0;
    let flush = |out: &mut String, line: &mut String| {
        let l = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !l.is_empty() {
            out.push_str(&l);
            out.push('\n');
        }
        line.clear();
    };
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        line.push_str(&rest[..open]);
        let close = rest[open..].find('>').unwrap() + open;
        let tag = &rest[open + 1..close];
        rest = &rest[close + 1..];
        let name = tag
            .trim_start_matches('/')
            .split([' ', '/'])
            .next()
            .unwrap_or_default();
        let closing = tag.starts_with('/');
        match (name, closing) {
            ("p" | "div" | "table" | "tr", false) => {
                flush(&mut out, &mut line);
                cells = 0;
            }
            ("p" | "table", true) => {
                flush(&mut out, &mut line);
                out.push('\n');
            }
            ("div" | "h4" | "tr" | "li", true) => flush(&mut out, &mut line),
            ("h4", false) => {
                flush(&mut out, &mut line);
                if !out.ends_with("\n\n") {
                    out.push('\n');
                }
                line.push_str("# ");
            }
            ("ul", false) => lists.push(None),
            ("ol", false) => lists.push(Some(0)),
            ("ul" | "ol", true) => {
                flush(&mut out, &mut line);
                lists.pop();
                if lists.is_empty() {
                    out.push('\n');
                }
            }
            ("li", false) => {
                flush(&mut out, &mut line);
                let depth = "  ".repeat(lists.len().saturating_sub(1));
                match lists.last_mut() {
                    Some(Some(n)) => {
                        *n += 1;
                        line.push_str(&format!("{depth}{n}. "));
                    }
                    _ => line.push_str(&format!("{depth}• ")),
                }
            }
            ("td" | "th", false) => {
                if cells > 0 {
                    line.push_str(" — ");
                }
                cells += 1;
            }
            ("ex", false) => {
                flush(&mut out, &mut line);
                line.push_str("    ");
            }
            ("ex", true) => {
                let l = line.replace('\u{1}', " → ");
                line = l;
                // Spaces inside an example are part of the names.
                out.push_str(&line);
                out.push('\n');
                line.clear();
            }
            ("br", _) => flush(&mut out, &mut line),
            _ => {}
        }
    }
    line.push_str(rest);
    flush(&mut out, &mut line);
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out.trim()
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
}
