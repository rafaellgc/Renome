//! `update`: the newest release on GitHub, and (where the program can
//! replace itself: the AppImage, Windows, macOS) its download, checked
//! against the project's signature (the window's public key) before it
//! is installed. `.deb`/`.rpm`: the release page; Flatpak: Flatpak's job.

use std::path::{Path, PathBuf};

use clap::ArgMatches;
use serde_json::Value;

use crate::Exit;
use crate::out;
use crate::text::t;

mod keys {
    include!(concat!(env!("OUT_DIR"), "/update.rs"));
}

enum Way {
    AppImage(PathBuf),
    /// The Windows installer's folder.
    Windows,
    /// The macOS app bundle.
    MacApp(PathBuf),
    Page,
    Flatpak,
}

fn way() -> Way {
    if std::env::var_os("FLATPAK_ID").is_some() {
        return Way::Flatpak;
    }
    if let Some(a) = std::env::var_os("APPIMAGE") {
        return Way::AppImage(a.into());
    }
    let exe = std::env::current_exe().unwrap_or_default();
    if cfg!(windows)
        && exe
            .parent()
            .is_some_and(|d| d.join("uninstall.exe").exists())
    {
        return Way::Windows;
    }
    if cfg!(target_os = "macos")
        && let Some(app) = exe
            .ancestors()
            .find(|a| a.extension().is_some_and(|e| e == "app"))
    {
        return Way::MacApp(app.to_path_buf());
    }
    Way::Page
}

/// The platform's entry in `latest.json`, as the window's updater picks it.
fn platform_keys(way: &Way) -> Vec<String> {
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    };
    match way {
        Way::AppImage(_) => vec![format!("linux-{arch}-appimage"), format!("linux-{arch}")],
        Way::Windows => vec![format!("windows-{arch}-nsis"), format!("windows-{arch}")],
        Way::MacApp(_) => vec![format!("darwin-{arch}-app"), format!("darwin-{arch}")],
        _ => vec![],
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("Renome/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())
}

/// The newest release's `latest.json`, when it is newer than this one.
async fn newer() -> Result<Option<Value>, String> {
    let url = std::env::var("RENOME_UPDATE_URL").unwrap_or_else(|_| keys::UPDATE_URL.to_string());
    let latest: Value = client()?
        .get(url)
        .send()
        .await
        .map_err(|e| format!("updateFailed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("updateFailed: {e}"))?;
    let version = latest
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim_start_matches('v');
    let (Ok(theirs), Ok(ours)) = (
        semver::Version::parse(version),
        semver::Version::parse(env!("CARGO_PKG_VERSION")),
    ) else {
        return Ok(None);
    };
    Ok((theirs > ours).then_some(latest))
}

pub fn command(m: &ArgMatches) -> Result<Exit, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let found = rt
        .block_on(newer())
        .map_err(|e| crate::text::error_text(&e))?;
    let Some(latest) = found else {
        out::note(&t("upToDate", &[("v", &env!("CARGO_PKG_VERSION"))]));
        return Ok(Exit::Done);
    };
    let version = latest["version"]
        .as_str()
        .unwrap_or_default()
        .trim_start_matches('v')
        .to_string();
    let page = format!("https://github.com/rafaellgc/Renome/releases/tag/v{version}");
    if out::json() && !matches!(m.subcommand(), Some(("install", _))) {
        println!(
            "{}",
            serde_json::json!({ "version": version, "page": page })
        );
        return Ok(Exit::Done);
    }
    out::note(&t("updateAvailable", &[("v", &version)]));
    let way = way();
    if !matches!(m.subcommand(), Some(("install", _))) {
        match way {
            Way::Flatpak => out::note(&t("updateByFlatpak", &[])),
            Way::Page => out::note(&page),
            _ => out::note(&t("updateHowCli", &[])),
        }
        return Ok(Exit::Done);
    }
    match &way {
        Way::Flatpak => {
            out::note(&t("updateByFlatpak", &[]));
            return Ok(Exit::Done);
        }
        Way::Page => {
            out::note(&page);
            return Ok(Exit::Done);
        }
        _ => {}
    }
    let entry = platform_keys(&way)
        .iter()
        .find_map(|k| latest["platforms"].get(k).cloned())
        .ok_or_else(|| t("updateFailed", &[]))?;
    let url = entry["url"].as_str().unwrap_or_default().to_string();
    let signature = entry["signature"].as_str().unwrap_or_default().to_string();
    out::note(&t("updateStarting", &[]));
    let bytes = rt
        .block_on(async {
            let res = client()?
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("updateFailed: {e}"))?;
            res.bytes()
                .await
                .map(|b| b.to_vec())
                .map_err(|e| format!("updateFailed: {e}"))
        })
        .map_err(|e| crate::text::error_text(&e))?;
    verify(&bytes, &signature)?;
    match way {
        Way::AppImage(path) => {
            let tmp = path.with_extension("renome-update");
            std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
                    .map_err(|e| e.to_string())?;
            }
            std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        }
        Way::Windows => {
            // The installer replaces this program too: it runs on its own
            // once this one has ended.
            let setup = std::env::temp_dir().join(format!("Renome_{version}_setup.exe"));
            std::fs::write(&setup, &bytes).map_err(|e| e.to_string())?;
            std::process::Command::new(&setup)
                .arg("/P")
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        Way::MacApp(app) => install_mac(&bytes, &app)?,
        _ => {}
    }
    out::note(&out::green(&t("updateInstalled", &[("v", &version)])));
    Ok(Exit::Done)
}

/// Checks the download against the project's signature (minisign).
fn verify(bytes: &[u8], signature: &str) -> Result<(), String> {
    use base64::Engine;
    let decode = |s: &str| -> Result<String, String> {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(s.trim())
            .map_err(|_| t("updateSignature", &[]))?;
        String::from_utf8(raw).map_err(|_| t("updateSignature", &[]))
    };
    let key = minisign_verify::PublicKey::decode(&decode(keys::UPDATE_KEY)?)
        .map_err(|_| t("updateSignature", &[]))?;
    let sig = minisign_verify::Signature::decode(&decode(signature)?)
        .map_err(|_| t("updateSignature", &[]))?;
    key.verify(bytes, &sig, true)
        .map_err(|_| t("updateSignature", &[]))
}

/// The new `.app` (a `.tar.gz` holding it) put in place of the old one,
/// kept until the new one is there.
fn install_mac(bytes: &[u8], app: &Path) -> Result<(), String> {
    let parent = app.parent().ok_or("updateFailed")?;
    let work = parent.join(".renome-update");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    tar.unpack(&work).map_err(|e| e.to_string())?;
    let new = std::fs::read_dir(&work)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or("updateFailed")?;
    let old = work.join("old.app");
    std::fs::rename(app, &old).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&new, app) {
        let _ = std::fs::rename(&old, app);
        return Err(e.to_string());
    }
    let _ = std::fs::remove_dir_all(&work);
    Ok(())
}
