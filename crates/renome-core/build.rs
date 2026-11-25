// A build script that cannot do its work should stop the build.
#![allow(clippy::unwrap_used)]

//! The readers' list (`src/lib/readers.ts`, the window's) made into
//! tables: the screens the comic converter fits pages to (size, ink,
//! pixels per inch), the models with the formats each opens as it comes,
//! and the readers the comic rule offers, with no second copy of the list
//! to keep in step. The window and the command line both read these.

use std::fmt::Write as _;
use std::path::Path;

/// The quoted strings of a text, in order (`"a", "b, c"` → `a`, `b, c`).
fn quoted(s: &str) -> Vec<&str> {
    s.split('"').skip(1).step_by(2).collect()
}

/// A list of formats: `["epub", "pdf"]`, or `APPS`.
fn formats<'a>(s: &'a str, apps: &[&'a str]) -> Option<Vec<&'a str>> {
    let s = s.trim().trim_end_matches(',').trim_end_matches(')').trim();
    if s == "APPS" {
        Some(apps.to_vec())
    } else if s.starts_with('[') {
        Some(quoted(s))
    } else {
        None
    }
}

fn main() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/lib/readers.ts");
    println!("cargo::rerun-if-changed={}", src.display());
    let text = std::fs::read_to_string(&src).expect("src/lib/readers.ts");
    let mut out = String::new();
    // `  "kindle-pw": s(1236, 1648),` / `  kindle: s(1072, 1448, "grey", 212),`
    writeln!(
        out,
        "pub static SCREENS: &[(&str, u32, u32, &str, u32)] = &["
    )
    .unwrap();
    let screens = &text[text.find("export const SCREENS").expect("SCREENS")..];
    let screens = &screens[..screens.find("\n};").expect("end of SCREENS")];
    let mut names = Vec::new();
    for line in screens.lines().skip(1) {
        let Some((name, rest)) = line.trim().split_once(": s(") else {
            continue;
        };
        let args: Vec<&str> = rest
            .trim_end_matches("),")
            .split(',')
            .map(str::trim)
            .collect();
        let num = |i: usize, d: u32| args.get(i).and_then(|a| a.parse().ok()).unwrap_or(d);
        let ink = args.get(2).map(|a| a.trim_matches('"')).unwrap_or("grey");
        let name = name.trim().trim_matches('"').to_string();
        writeln!(
            out,
            "    ({name:?}, {}, {}, {ink:?}, {}),",
            num(0, 0),
            num(1, 0),
            num(3, 300)
        )
        .unwrap();
        names.push(name);
    }
    writeln!(out, "];").unwrap();
    // `export const NO_PNG: ComicDevice[] = ["kindle-scribe", …];`
    let line = text
        .lines()
        .find(|l| l.starts_with("export const NO_PNG"))
        .expect("NO_PNG");
    writeln!(out, "pub static NO_PNG: &[&str] = &{:?};", quoted(line)).unwrap();
    // The models, by brand: `"comicDevices_kobo",` then the brand's
    // formats (`["kepub", "epub"],` or `APPS,`), then
    // `r("kobo-sage", "Kobo Sage (2021)", "kobo-sage"),`, a model's own
    // formats after its screen when they are not its brand's.
    let apps_line = text
        .lines()
        .find(|l| l.starts_with("const APPS"))
        .expect("APPS");
    let apps = quoted(apps_line);
    let readers = &text[text.find("export const READERS").expect("READERS")..];
    let readers = &readers[..readers.find("\n];").expect("end of READERS")];
    writeln!(
        out,
        "pub static MODELS: &[(&str, &str, &str, &[&str])] = &["
    )
    .unwrap();
    let mut brand: Option<Vec<&str>> = None;
    let mut expect_formats = false;
    for line in readers.lines().skip(1) {
        let line = line.trim();
        if line.starts_with("//") || line.is_empty() {
            continue;
        }
        if line.starts_with("\"comicDevices_") {
            expect_formats = true;
            continue;
        }
        if expect_formats {
            brand = Some(formats(line, &apps).expect("a brand's formats"));
            expect_formats = false;
            continue;
        }
        let Some(rest) = line.strip_prefix("r(") else {
            continue;
        };
        let q = quoted(rest);
        let (id, name, device) = (q[0], q[1], q[2]);
        // What follows the screen's quote: `)` or `, [..])` or `, APPS)`.
        let after = rest
            .match_indices('"')
            .nth(5)
            .map(|(i, _)| &rest[i + 1..])
            .unwrap_or("");
        let own = after
            .trim_start_matches(',')
            .trim()
            .strip_suffix("),")
            .and_then(|f| formats(f, &apps));
        let list = own.or_else(|| brand.clone()).expect("formats");
        assert!(names.iter().any(|n| n == device), "{id}: {device}");
        writeln!(out, "    ({id:?}, {name:?}, {device:?}, &{list:?}),").unwrap();
    }
    writeln!(out, "];").unwrap();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    // The rule's choice of reader: the pages as they are, each screen in
    // the list's order, a custom size (`OTHER_DEVICES`).
    let mut devices = String::from("&[\n    c(\"original\", \"comicDevice_original\"),\n");
    for name in &names {
        writeln!(devices, "    c({name:?}, \"comicDevice_{name}\"),").unwrap();
    }
    devices.push_str("    c(\"custom\", \"comicDevice_custom\"),\n]");
    std::fs::write(Path::new(&out_dir).join("devices.rs"), devices).unwrap();
    std::fs::write(Path::new(&out_dir).join("screens.rs"), out).unwrap();
}
