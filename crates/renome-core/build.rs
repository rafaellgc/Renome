//! The readers' screens (`src/lib/readers.ts`, the window's list) made
//! into a table: the size and kind of ink the comic converter fits pages
//! to, and the readers the comic rule offers, with no second copy of
//! the list to keep in step.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/lib/readers.ts");
    println!("cargo::rerun-if-changed={}", src.display());
    let text = std::fs::read_to_string(&src).expect("src/lib/readers.ts");
    let mut out = String::new();
    // `  "kindle-pw": s(1236, 1648),` / `  kindle: s(1072, 1448, "grey", 212),`
    writeln!(out, "pub static SCREENS: &[(&str, u32, u32, &str)] = &[").unwrap();
    let screens = &text[text.find("export const SCREENS").expect("SCREENS")..];
    let screens = &screens[..screens.find("\n};").expect("end of SCREENS")];
    for line in screens.lines().skip(1) {
        let Some((name, rest)) = line.trim().split_once(": s(") else {
            continue;
        };
        let args: Vec<&str> = rest
            .trim_end_matches("),")
            .split(',')
            .map(str::trim)
            .collect();
        let num = |i: usize| args.get(i).and_then(|a| a.parse::<u32>().ok()).unwrap_or(0);
        let ink = args.get(2).map(|a| a.trim_matches('"')).unwrap_or("grey");
        writeln!(
            out,
            "    ({:?}, {}, {}, {:?}),",
            name.trim().trim_matches('"'),
            num(0),
            num(1),
            ink
        )
        .unwrap();
    }
    writeln!(out, "];").unwrap();
    // `export const NO_PNG: ComicDevice[] = ["kindle-scribe", …];`
    let line = text
        .lines()
        .find(|l| l.starts_with("export const NO_PNG"))
        .expect("NO_PNG");
    let list = &line[line.find('[').expect("[") + 1..line.rfind(']').expect("]")];
    let names: Vec<String> = list
        .split(',')
        .map(|n| n.trim().trim_matches('"').to_string())
        .filter(|n| !n.is_empty())
        .collect();
    writeln!(out, "pub static NO_PNG: &[&str] = &{names:?};").unwrap();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    // The rule's choice of reader: the pages as they are, each screen in
    // the list's order, a custom size (`OTHER_DEVICES`).
    let mut devices = String::from("&[\n    c(\"original\", \"comicDevice_original\"),\n");
    for line in out.lines().filter(|l| l.starts_with("    (\"")) {
        let name = line.trim().trim_start_matches("(\"");
        let name = &name[..name.find('"').unwrap()];
        writeln!(devices, "    c({name:?}, \"comicDevice_{name}\"),").unwrap();
    }
    devices.push_str("    c(\"custom\", \"comicDevice_custom\"),\n]");
    std::fs::write(Path::new(&out_dir).join("devices.rs"), devices).unwrap();
    std::fs::write(Path::new(&out_dir).join("screens.rs"), out).unwrap();
}
