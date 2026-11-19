//! The readers' screens and models (`src/lib/readers.ts`, the window's
//! list) made into tables, so a book printed to PDF for a reader gets the
//! same page size from the window and the command line, without a second
//! copy of the list to keep in step.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/lib/readers.ts");
    println!("cargo::rerun-if-changed={}", src.display());
    let text = std::fs::read_to_string(&src).expect("src/lib/readers.ts");
    let quoted = |s: &str| s.trim().trim_matches('"').to_string();
    let mut out = String::new();
    // `  "kindle-pw": s(1236, 1648),` / `  kindle: s(1072, 1448, "grey", 212),`
    writeln!(out, "pub static SCREENS: &[(&str, u32, u32, u32)] = &[").unwrap();
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
        let num = |i: usize, d: u32| args.get(i).and_then(|a| a.parse().ok()).unwrap_or(d);
        writeln!(
            out,
            "    ({:?}, {}, {}, {}),",
            quoted(name),
            num(0, 0),
            num(1, 0),
            num(3, 300)
        )
        .unwrap();
    }
    writeln!(out, "];").unwrap();
    // `      r("boox-go-10", "Boox Go 10.3 (2024)", "eink1860"),`
    writeln!(out, "pub static MODELS: &[(&str, &str)] = &[").unwrap();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("r(\"") else {
            continue;
        };
        let parts: Vec<&str> = rest.split("\", \"").collect();
        if parts.len() >= 3 {
            let device = parts[2].split('"').next().unwrap_or_default();
            writeln!(out, "    ({:?}, {:?}),", parts[0], device).unwrap();
        }
    }
    writeln!(out, "];").unwrap();
    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join("readers.rs");
    std::fs::write(dest, out).unwrap();
}
