//! New versions: looked for in the latest release on GitHub (its
//! `latest.json`, signed with the project's key, whose public half is in
//! `tauri.conf.json`), downloaded and installed by the program where it
//! can replace itself (the AppImage, Windows and macOS). A `.deb` or `.rpm`
//! only hears of it, with the page to download it from; the Flatpak is
//! updated by Flatpak, so it never asks.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

/// How this copy of the program is updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Way {
    /// Downloaded and installed by the program.
    Itself,
    /// By the user, from the release page (a `.deb` or `.rpm`).
    Page,
    /// By the system (Flatpak).
    System,
}

pub fn way() -> Way {
    if cfg!(target_os = "linux") {
        if std::env::var_os("FLATPAK_ID").is_some() {
            Way::System
        } else if std::env::var_os("APPIMAGE").is_some() {
            Way::Itself
        } else {
            Way::Page
        }
    } else {
        Way::Itself
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub version: String,
    pub way: Way,
}

#[derive(Clone, Serialize)]
struct Progress {
    done: u64,
    total: Option<u64>,
}

fn error(e: &tauri_plugin_updater::Error) -> String {
    match e {
        tauri_plugin_updater::Error::Minisign(_)
        | tauri_plugin_updater::Error::SignatureUtf8(_) => "updateSignature".into(),
        _ => format!("updateFailed: {e}"),
    }
}

/// The version the last check found, kept for the install so it does not
/// have to ask GitHub again.
static FOUND: std::sync::Mutex<Option<tauri_plugin_updater::Update>> = std::sync::Mutex::new(None);

async fn ask(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    app.updater()
        .map_err(|e| error(&e))?
        .check()
        .await
        .map_err(|e| error(&e))
}

/// The newer version, if there is one.
pub async fn check(app: &AppHandle) -> Result<Option<Found>, String> {
    let way = way();
    if way == Way::System {
        return Ok(None);
    }
    let update = ask(app).await?;
    let found = update.as_ref().map(|u| Found {
        version: u.version.clone(),
        way,
    });
    *FOUND.lock().unwrap_or_else(|e| e.into_inner()) = update;
    Ok(found)
}

/// Downloads the newer version, checks its signature, installs it and
/// starts it; `update-progress` events as it downloads.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    if way() != Way::Itself {
        return Err("updateFailed".into());
    }
    let kept = FOUND.lock().unwrap_or_else(|e| e.into_inner()).take();
    let update = match kept {
        Some(u) => u,
        None => match ask(app).await? {
            Some(u) => u,
            None => return Ok(()),
        },
    };
    let mut done = 0u64;
    update
        .download_and_install(
            |chunk, total| {
                done += chunk as u64;
                let _ = app.emit("update-progress", Progress { done, total });
            },
            || {},
        )
        .await
        .map_err(|e| error(&e))?;
    app.restart();
}
