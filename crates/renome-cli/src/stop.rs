//! Ctrl+C: the first stops the searches (what was found stays chosen) or
//! the renames (what was renamed stays in the History); a second, or one
//! while a question waits for an answer, ends the program (exit 130).

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static PRESSED: AtomicU8 = AtomicU8::new(0);
static ASKING: AtomicBool = AtomicBool::new(false);

pub fn install() {
    let _ = ctrlc::set_handler(|| {
        let n = PRESSED.fetch_add(1, Ordering::SeqCst) + 1;
        if n >= 2 || ASKING.load(Ordering::SeqCst) {
            eprintln!();
            std::process::exit(130);
        }
        eprintln!("\n{}", crate::text::t("stopping", &[]));
    });
}

pub fn requested() -> bool {
    PRESSED.load(Ordering::SeqCst) > 0
}

/// Going on after a stop was answered.
pub fn reset() {
    PRESSED.store(0, Ordering::SeqCst);
}

/// While a question waits: Ctrl+C ends the program at once.
pub fn asking<T>(f: impl FnOnce() -> T) -> T {
    ASKING.store(true, Ordering::SeqCst);
    let out = f();
    ASKING.store(false, Ordering::SeqCst);
    out
}
