//! How things are written: colours when the terminal shows them (never
//! with `NO_COLOR` or `--color never`), JSON for scripts, quiet runs.

use std::io::IsTerminal;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::ArgMatches;

static COLOR: AtomicBool = AtomicBool::new(false);
static JSON: AtomicBool = AtomicBool::new(false);
static QUIET: AtomicBool = AtomicBool::new(false);

pub fn setup(m: &ArgMatches) {
    let color = match m.get_one::<String>("color").map(String::as_str) {
        Some("always") => true,
        Some("never") => false,
        _ => std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal(),
    };
    COLOR.store(color, Ordering::Relaxed);
    JSON.store(m.get_flag("json"), Ordering::Relaxed);
    QUIET.store(m.get_flag("quiet"), Ordering::Relaxed);
}

pub fn json() -> bool {
    JSON.load(Ordering::Relaxed)
}

pub fn quiet() -> bool {
    QUIET.load(Ordering::Relaxed) || json()
}

fn paint(code: &str, s: &str) -> String {
    if COLOR.load(Ordering::Relaxed) {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn red(s: &str) -> String {
    paint("31", s)
}
pub fn green(s: &str) -> String {
    paint("32", s)
}
pub fn yellow(s: &str) -> String {
    paint("33", s)
}
pub fn dim(s: &str) -> String {
    paint("2", s)
}
pub fn bold(s: &str) -> String {
    paint("1", s)
}

/// An error, on stderr (as JSON with `--json`).
pub fn error(message: &str) {
    if json() {
        eprintln!("{}", serde_json::json!({ "error": message }));
    } else {
        eprintln!("{}: {message}", red(&crate::text::t("error", &[])));
    }
}

/// A notice for people (not in quiet or JSON runs), on stderr so the
/// output stays the list.
pub fn note(message: &str) {
    if !quiet() {
        eprintln!("{message}");
    }
}

/// A path as people read it: the home folder as `~`.
pub fn path(p: &Path) -> String {
    if let Some(home) = dirs_home()
        && let Ok(rest) = p.strip_prefix(&home)
    {
        return if rest.as_os_str().is_empty() {
            "~".into()
        } else {
            format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display())
        };
    }
    p.display().to_string()
}

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(Into::into)
}

/// Bytes as the list shows them.
pub fn size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < units.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", units[u])
    }
}
