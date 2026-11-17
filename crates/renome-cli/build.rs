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
