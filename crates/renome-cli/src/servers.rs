//! Media servers (Jellyfin/Emby, Plex): kept in the options as the
//! window keeps them, their keys in the password store (`serverKeys`);
//! told about the folders that changed after renames and undos.

use clap::ArgMatches;
use renome_services::mediaserver;
use serde_json::{Map, Value};

use crate::Exit;
use crate::out;
use crate::text::t;

/// The servers' keys (`serverKeys`, a JSON text among the secrets).
fn keys() -> Map<String, Value> {
    mediaserver::keys_of(&raw_keys())
}

fn raw_keys() -> String {
    crate::manage::secrets()
        .ok()
        .and_then(|(all, _)| all.get("serverKeys")?.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Sets (or removes) one server's key, keeping every other secret: from
/// the store, or from the settings file when the system has none.
fn save_key(id: &str, key: Option<String>) -> Result<(), String> {
    let (mut all, _) = crate::manage::secrets()?;
    let raw = mediaserver::with_key(&raw_keys(), id, key.as_deref().unwrap_or_default());
    all.insert("serverKeys".into(), Value::from(raw));
    renome_services::save_secrets(all).map(|_| ())
}

fn name_of(s: &Value) -> String {
    mediaserver::name_of(s).into()
}

/// After renames or an undo: each server told about the folders the files
/// left and went to (those in its libraries); what was told, as notices.
pub async fn tell(settings: Option<&Value>, ops: &[renome_core::Operation]) -> Vec<String> {
    let servers = settings
        .and_then(|s| s.get("servers"))
        .cloned()
        .unwrap_or(Value::Null);
    let keys = Value::Object(keys()).to_string();
    let answers = mediaserver::tell_all(mediaserver::callable(&servers, &keys), ops).await;
    // As the window says it (`flow::said::told`).
    crate::lookup::said(&renome_services::flow::said::told(&answers))
}

use crate::manage::{save_settings as save, settings_now};

fn servers_of(settings: &mut Value) -> &mut Vec<Value> {
    crate::manage::list_of(settings, "servers")
}

fn find<'a>(list: &'a mut [Value], id: &str) -> Result<&'a mut Value, String> {
    list.iter_mut()
        .find(|s| s.get("id").and_then(Value::as_str) == Some(id))
        .ok_or_else(|| t("noServer", &[("id", &id)]))
}

fn server_of(s: &Value, key: &str) -> Result<mediaserver::Server, String> {
    mediaserver::server_of(s, key)
}

/// Reads a server's libraries and pairs their folders (see
/// `mediaserver::read`, the window's too).
async fn read(server: &mut Value, key: &str) -> Result<(usize, usize), String> {
    let (read, libs, folders) = mediaserver::read(server.clone(), key)
        .await
        .map_err(|e| crate::text::error_text(&e))?;
    *server = read;
    Ok((libs, folders))
}

/// A required argument (the parser has checked it is there).
fn arg<'a>(s: &'a ArgMatches, name: &str) -> &'a String {
    s.get_one::<String>(name)
        .expect("required by the argument parser")
}

pub fn command(m: &ArgMatches, rt: &tokio::runtime::Runtime) -> Result<Exit, String> {
    let keys = keys();
    match m.subcommand() {
        Some(("add", s)) => add(s, rt)?,
        Some(("remove", s)) => remove(arg(s, "id"))?,
        Some(("libraries", s)) => libraries(arg(s, "id"), &keys, rt)?,
        Some(("pair", s)) => pair(s)?,
        Some((on @ ("on" | "off"), s)) => turn(arg(s, "id"), arg(s, "server"), on == "on")?,
        Some(("test", s)) => test(arg(s, "id"), &keys, rt)?,
        _ => return list(&keys),
    }
    Ok(Exit::Done)
}

/// Adds a server: its libraries are read and its key kept.
fn add(s: &ArgMatches, rt: &tokio::runtime::Runtime) -> Result<(), String> {
    let kind = arg(s, "kind").clone();
    let url = arg(s, "url").trim().to_string();
    let key = read_key(s)?;
    let mut server = mediaserver::new_server(&kind, &url);
    let id = server["id"].as_str().unwrap_or_default().to_string();
    let (libs, folders) = rt.block_on(read(&mut server, &key))?;
    save_key(&id, Some(key))?;
    let mut now = settings_now();
    servers_of(&mut now).push(server);
    save(&now)?;
    out::note(&format!(
        "{id}: {}",
        t("serverRead", &[("libs", &libs), ("folders", &folders)])
    ));
    Ok(())
}

fn remove(id: &str) -> Result<(), String> {
    let mut now = settings_now();
    let list = servers_of(&mut now);
    let before = list.len();
    list.retain(|x| x.get("id").and_then(Value::as_str) != Some(id));
    if list.len() == before {
        return Err(t("noServer", &[("id", &id)]));
    }
    save(&now)?;
    save_key(id, None)?;
    out::note(&t("serverRemoved", &[("id", &id)]));
    Ok(())
}

/// Reads the server's libraries again.
fn libraries(
    id: &str,
    keys: &Map<String, Value>,
    rt: &tokio::runtime::Runtime,
) -> Result<(), String> {
    let key = keys
        .get(id)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut now = settings_now();
    let server = find(servers_of(&mut now), id)?;
    let (libs, folders) = rt.block_on(read(server, &key))?;
    save(&now)?;
    out::note(&t("serverRead", &[("libs", &libs), ("folders", &folders)]));
    Ok(())
}

/// Pairs a server's folder with a folder here.
fn pair(s: &ArgMatches) -> Result<(), String> {
    let (id, server_path) = (arg(s, "id"), arg(s, "server"));
    let local = std::path::absolute(arg(s, "local")).map_err(|e| e.to_string())?;
    let mut now = settings_now();
    let server = find(servers_of(&mut now), id)?;
    let folder = folder_of(server, server_path)?;
    folder["local"] = Value::from(local.display().to_string());
    save(&now)
}

/// Turns a server's folder on or off.
fn turn(id: &str, server_path: &str, on: bool) -> Result<(), String> {
    let mut now = settings_now();
    let server = find(servers_of(&mut now), id)?;
    folder_of(server, server_path)?["on"] = Value::from(on);
    save(&now)
}

fn test(id: &str, keys: &Map<String, Value>, rt: &tokio::runtime::Runtime) -> Result<(), String> {
    let mut now = settings_now();
    let server = find(servers_of(&mut now), id)?;
    let key = keys.get(id).and_then(Value::as_str).unwrap_or_default();
    rt.block_on(server_of(server, key)?.check())
        .map_err(|e| crate::text::error_text(&e))?;
    out::note(&t("serverOk", &[]));
    Ok(())
}

/// The servers, as JSON or as text (a key not set is said).
fn list(keys: &Map<String, Value>) -> Result<Exit, String> {
    let now = settings_now();
    let list = now
        .get("servers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if out::json() {
        let shown: Vec<Value> = list
            .iter()
            .map(|s| {
                let mut s = s.clone();
                let id = s["id"].as_str().unwrap_or_default().to_string();
                s["key"] = Value::from(keys.get(&id).is_some());
                s
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&shown).unwrap_or_default()
        );
        return Ok(Exit::Done);
    }
    for s in &list {
        let id = s["id"].as_str().unwrap_or_default();
        let key = if keys.contains_key(id) {
            String::new()
        } else {
            out::red(&format!(" ({})", t("keyUnset", &[])))
        };
        println!(
            "{}  {}  {}{}",
            out::bold(id),
            name_of(s),
            s["url"].as_str().unwrap_or_default(),
            key
        );
        for f in s["folders"].as_array().into_iter().flatten() {
            let on = if f["on"].as_bool().unwrap_or(false) {
                "on "
            } else {
                "off"
            };
            let local = f["local"].as_str().unwrap_or_default();
            let local = if local.is_empty() {
                out::yellow(&t("srvUnpaired", &[]))
            } else {
                local.to_string()
            };
            println!(
                "    {on}  {}: {} → {local}",
                f["library"].as_str().unwrap_or_default(),
                f["server"].as_str().unwrap_or_default()
            );
        }
    }
    Ok(Exit::Done)
}

fn folder_of<'a>(server: &'a mut Value, path: &str) -> Result<&'a mut Value, String> {
    server["folders"]
        .as_array_mut()
        .and_then(|l| l.iter_mut().find(|f| f["server"].as_str() == Some(path)))
        .ok_or_else(|| t("noServerFolder", &[("path", &path)]))
}

fn read_key(s: &ArgMatches) -> Result<String, String> {
    if s.get_flag("stdin") {
        let mut v = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut v).map_err(|e| e.to_string())?;
        Ok(v.trim().to_string())
    } else {
        Ok(
            rpassword::prompt_password(t("keyPrompt", &[("name", &"server")]))
                .map_err(|e| e.to_string())?
                .trim()
                .to_string(),
        )
    }
}
