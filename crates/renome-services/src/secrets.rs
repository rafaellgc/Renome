//! The keys, tokens and the GCD account from the options, kept in the
//! system's password store (the Secret Service on Linux, the Credential
//! Manager on Windows, the Keychain on macOS), encrypted there, instead of
//! in the settings file. When the system has no such store, they stay in
//! the settings file as before, and the options say so.
//!
//! The store may take long to answer (a locked store asks the user for
//! its password), so the settings load without it and the secrets come
//! apart, when it answers.

use std::sync::Mutex;

use serde_json::{Map, Value};

/// The settings that are secrets.
pub const FIELDS: [&str; 13] = [
    "tmdbKey",
    "osKey",
    "osUser",
    "osPassword",
    "subdlKey",
    "subsourceKey",
    "comicVineKey",
    "discogsToken",
    "metronToken",
    "googleBooksKey",
    "gcdEmail",
    "gcdPassword",
    "serverKeys",
];

/// Tests use a store of their own with `RENOME_SECRETS_SERVICE`.
fn service() -> String {
    std::env::var("RENOME_SECRETS_SERVICE").unwrap_or_else(|_| "io.github.rafaellgc.Renome".into())
}

const ACCOUNT: &str = "credentials";

/// What was last read from or written to the store, so unchanged secrets
/// are not written again. `None` until the store was read: a store that
/// could not be read (a refused or locked one) is never written to, so
/// what it holds is not lost.
static STORED: Mutex<Option<String>> = Mutex::new(None);

/// How reading the store went.
pub enum Read {
    /// The secrets (none, when it has no entry yet).
    Found(Map<String, Value>),
    /// There is a store, but it did not answer: refused (macOS asks the
    /// user after each new version), locked, or failing.
    Refused,
    /// The system has no store.
    Missing,
}

fn entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(&service(), ACCOUNT)
}

/// The secret fields of a JSON object.
pub fn of(settings: &Map<String, Value>) -> Map<String, Value> {
    FIELDS
        .iter()
        .filter_map(|&k| Some((k.to_string(), settings.get(k)?.clone())))
        .collect()
}

fn is_empty(secrets: &Map<String, Value>) -> bool {
    secrets
        .values()
        .all(|v| v.as_str().is_none_or(str::is_empty))
}

/// The secrets in the store.
pub fn read() -> Read {
    let Ok(entry) = entry() else {
        return Read::Missing;
    };
    let found = entry.get_password();
    let mut stored = STORED.lock().unwrap();
    match found {
        Ok(text) => match parse(&text) {
            Some(map) => {
                *stored = Some(text);
                Read::Found(map)
            }
            // Not ours to understand: never written over.
            None => Read::Refused,
        },
        Err(keyring::Error::NoEntry) => {
            *stored = Some(String::new());
            Read::Found(Map::new())
        }
        Err(keyring::Error::NoDefaultStore) => Read::Missing,
        Err(_) => Read::Refused,
    }
}

fn parse(text: &str) -> Option<Map<String, Value>> {
    if text.is_empty() {
        return Some(Map::new());
    }
    serde_json::from_str(text).ok()
}

fn text_of(map: &Map<String, Value>) -> String {
    if is_empty(map) {
        String::new()
    } else {
        Value::Object(map.clone()).to_string()
    }
}

/// Writes the secrets this program changed; whether the store took them.
/// A store not read in this session is left alone.
///
/// All the secrets are one entry, shared by every copy of the program
/// (the window, renome-cli, a Flatpak and an AppImage, an older version).
/// So the entry is read again first, and only the fields that differ from
/// what this program last saw there are changed: a field another copy
/// saved meanwhile, or one this version does not know, stays as it is.
pub fn write(secrets: &Map<String, Value>) -> bool {
    let Some(last) = STORED.lock().unwrap().clone() else {
        return false;
    };
    let Ok(entry) = entry() else {
        return false;
    };
    let now_text = match entry.get_password() {
        Ok(t) => t,
        Err(keyring::Error::NoEntry) => String::new(),
        Err(_) => return false,
    };
    let (Some(now), Some(seen)) = (parse(&now_text), parse(&last)) else {
        return false;
    };
    let text = text_of(&merged(&now, &seen, secrets));
    if text == now_text {
        *STORED.lock().unwrap() = Some(text);
        return true;
    }
    let done = if text.is_empty() {
        match entry.delete_credential() {
            Err(keyring::Error::NoEntry) => Ok(()),
            other => other,
        }
    } else {
        entry.set_password(&text)
    };
    if done.is_ok() {
        *STORED.lock().unwrap() = Some(text);
    }
    done.is_ok()
}

/// What the store holds `now`, with the fields of `mine` that differ from
/// what was `seen` there (empty ones removed).
fn merged(
    now: &Map<String, Value>,
    seen: &Map<String, Value>,
    mine: &Map<String, Value>,
) -> Map<String, Value> {
    let text = |m: &Map<String, Value>, k: &str| {
        m.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let mut out = now.clone();
    for (k, v) in mine {
        let v = v.as_str().unwrap_or_default();
        if v == text(seen, k) {
            continue;
        }
        if v.is_empty() {
            out.remove(k);
        } else {
            out.insert(k.clone(), Value::from(v));
        }
    }
    out.retain(|_, v| v.as_str().is_none_or(|s| !s.is_empty()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> Map<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Value::from(*v)))
            .collect()
    }

    #[test]
    fn fields_saved_by_another_copy_or_unknown_here_stay() {
        // Seen at start: two keys. Meanwhile another copy (or a newer
        // version) added two more. This one changes only the TMDB key.
        let seen = map(&[("tmdbKey", "a"), ("gcdEmail", "e")]);
        let now = map(&[
            ("tmdbKey", "a"),
            ("gcdEmail", "e"),
            ("osKey", "o"),
            ("futureKey", "f"),
        ]);
        let mine = map(&[("tmdbKey", "b"), ("gcdEmail", "e"), ("osKey", "")]);
        let out = merged(&now, &seen, &mine);
        assert_eq!(
            out,
            map(&[
                ("tmdbKey", "b"),
                ("gcdEmail", "e"),
                ("osKey", "o"),
                ("futureKey", "f")
            ])
        );
        // A key cleared here goes.
        let mine = map(&[("tmdbKey", ""), ("gcdEmail", "e")]);
        assert_eq!(
            merged(&now, &seen, &mine),
            map(&[("gcdEmail", "e"), ("osKey", "o"), ("futureKey", "f")])
        );
    }

    #[test]
    fn an_entry_that_is_not_ours_is_not_read_as_empty() {
        assert_eq!(parse(""), Some(Map::new()));
        assert_eq!(parse("not json"), None);
    }

    /// Run by the CI on Windows and macOS, where the store is there.
    #[test]
    #[ignore = "uses the system's password store"]
    fn round_trip() {
        // SAFETY: the only test in this binary that sets variables.
        unsafe { std::env::set_var("RENOME_SECRETS_SERVICE", "io.github.rafaellgc.Renome.test") };
        let mut secrets = Map::new();
        secrets.insert("tmdbKey".into(), "fake-key".into());
        secrets.insert("gcdPassword".into(), "fake password".into());
        // Never read: the store is left alone.
        assert!(!write(&secrets));
        let found = |r: Read| match r {
            Read::Found(m) => m,
            _ => panic!("the store did not answer"),
        };
        found(read());
        assert!(write(&secrets));
        *STORED.lock().unwrap() = None;
        assert_eq!(found(read()), secrets);
        let empty: Map<String, Value> = FIELDS.iter().map(|k| (k.to_string(), "".into())).collect();
        assert!(write(&empty));
        *STORED.lock().unwrap() = None;
        assert_eq!(found(read()), Map::new());
    }
}
