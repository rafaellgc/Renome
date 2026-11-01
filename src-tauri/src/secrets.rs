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
pub const FIELDS: [&str; 5] = [
    "tmdbKey",
    "comicVineKey",
    "discogsToken",
    "gcdEmail",
    "gcdPassword",
];

/// Tests use a store of their own with `RENOME_SECRETS_SERVICE`.
fn service() -> String {
    std::env::var("RENOME_SECRETS_SERVICE").unwrap_or_else(|_| "io.github.rafaellgc.Renome".into())
}

const ACCOUNT: &str = "credentials";

/// What was last read from or written to the store, so unchanged secrets
/// are not written again.
static STORED: Mutex<Option<String>> = Mutex::new(None);

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

/// The secrets in the store; `None` when the store cannot be reached.
pub fn read() -> Option<Map<String, Value>> {
    let found = entry().and_then(|e| e.get_password());
    let mut stored = STORED.lock().unwrap();
    match found {
        Ok(text) => {
            *stored = Some(text.clone());
            Some(serde_json::from_str(&text).unwrap_or_default())
        }
        Err(keyring::Error::NoEntry) => {
            *stored = Some(String::new());
            Some(Map::new())
        }
        Err(_) => None,
    }
}

/// Writes the secrets (removes them when all are empty); whether the
/// store took them.
pub fn write(secrets: &Map<String, Value>) -> bool {
    let empty = is_empty(secrets);
    let text = if empty {
        String::new()
    } else {
        Value::Object(secrets.clone()).to_string()
    };
    if STORED.lock().unwrap().as_deref() == Some(text.as_str()) {
        return true;
    }
    let done = entry().and_then(|e| {
        if empty {
            match e.delete_credential() {
                Err(keyring::Error::NoEntry) => Ok(()),
                other => other,
            }
        } else {
            e.set_password(&text)
        }
    });
    if done.is_ok() {
        *STORED.lock().unwrap() = Some(text);
    }
    done.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run by the CI on Windows and macOS, where the store is there.
    #[test]
    #[ignore = "uses the system's password store"]
    fn round_trip() {
        // SAFETY: the only test in this binary that sets variables.
        unsafe { std::env::set_var("RENOME_SECRETS_SERVICE", "io.github.rafaellgc.Renome.test") };
        let mut secrets = Map::new();
        secrets.insert("tmdbKey".into(), "fake-key".into());
        secrets.insert("gcdPassword".into(), "fake password".into());
        assert!(write(&secrets));
        *STORED.lock().unwrap() = None;
        assert_eq!(read(), Some(secrets.clone()));
        let empty: Map<String, Value> = FIELDS.iter().map(|k| (k.to_string(), "".into())).collect();
        assert!(write(&empty));
        *STORED.lock().unwrap() = None;
        assert_eq!(read(), Some(Map::new()));
    }
}
