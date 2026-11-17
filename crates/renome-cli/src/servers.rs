//! Media servers (Jellyfin/Emby, Plex): kept in the options as the
//! window keeps them, their keys in the password store (`serverKeys`);
//! told about the folders that changed after renames and undos.

use std::path::PathBuf;

use clap::ArgMatches;
use renome_services::mediaserver;
use serde_json::{Map, Value, json};

use crate::Exit;
use crate::out;
use crate::text::t;

fn keys() -> Map<String, Value> {
    let loaded = renome_services::load_secrets();
    let raw = loaded
        .get("secrets")
        .and_then(|s| s.get("serverKeys"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            renome_services::read_settings_file()
                .and_then(|s| s.get("serverKeys")?.as_str().map(str::to_string))
        })
        .unwrap_or_else(|| "{}".into());
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_key(id: &str, key: Option<String>) -> Result<(), String> {
    let loaded = renome_services::load_secrets();
    let mut all: Map<String, Value> = loaded
        .get("secrets")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut k = keys();
    match key {
        Some(v) => k.insert(id.into(), Value::from(v)),
        None => k.remove(id),
    };
    all.insert(
        "serverKeys".into(),
        Value::from(Value::Object(k).to_string()),
    );
    renome_services::save_secrets(all).map(|_| ())
}

/// The servers to tell: with an address, a key and a folder on.
fn callable(settings: Option<&Value>) -> Vec<(String, mediaserver::Server)> {
    let keys = keys();
    let list = settings
        .and_then(|s| s.get("servers")?.as_array().cloned())
        .unwrap_or_default();
    list.into_iter()
        .filter_map(|s| {
            let id = s.get("id")?.as_str()?.to_string();
            let key = keys.get(&id)?.as_str()?.trim().to_string();
            let url = s.get("url")?.as_str()?.trim().to_string();
            if key.is_empty() || url.is_empty() {
                return None;
            }
            let server: mediaserver::Server = serde_json::from_value(json!({
                "kind": s.get("kind").cloned().unwrap_or(Value::from("jellyfin")),
                "url": url,
                "key": key,
                "folders": s.get("folders").cloned().unwrap_or(Value::Array(vec![])),
            }))
            .ok()?;
            server
                .folders
                .iter()
                .any(|f| f.on && !f.local.is_empty())
                .then_some((name_of(&s), server))
        })
        .collect()
}

fn name_of(s: &Value) -> String {
    if s.get("kind").and_then(Value::as_str) == Some("plex") {
        "Plex".into()
    } else {
        "Jellyfin".into()
    }
}

/// After renames or an undo: each server told about the folders the files
/// left and went to (those in its libraries); what was told, as notices.
pub async fn tell(settings: Option<&Value>, ops: &[renome_core::Operation]) -> Vec<String> {
    let mut notes = Vec::new();
    if ops.is_empty() {
        return notes;
    }
    let changed: Vec<PathBuf> = ops
        .iter()
        .flat_map(|op| [op.from.clone(), op.to.clone()])
        .collect();
    for (name, server) in callable(settings) {
        match server.tell(&changed).await {
            Ok(0) => {}
            Ok(n) => notes.push(t("serverTold", &[("name", &name), ("n", &n)])),
            Err(e) => notes.push(format!(
                "{}: {}",
                t("serverNotTold", &[("name", &name)]),
                crate::text::error_text(&e)
            )),
        }
    }
    notes
}

fn settings_now() -> Value {
    renome_services::read_settings_file().unwrap_or_else(|| Value::Object(Map::new()))
}

fn save(settings: &Value) -> Result<(), String> {
    let _file = renome_services::SETTINGS_FILE.lock().unwrap();
    renome_services::write_settings_file(settings)
}

fn servers_of(settings: &mut Value) -> &mut Vec<Value> {
    let obj = settings.as_object_mut().expect("settings object");
    let list = obj.entry("servers").or_insert_with(|| Value::Array(vec![]));
    if !list.is_array() {
        *list = Value::Array(vec![]);
    }
    list.as_array_mut().unwrap()
}

fn find<'a>(list: &'a mut [Value], id: &str) -> Result<&'a mut Value, String> {
    list.iter_mut()
        .find(|s| s.get("id").and_then(Value::as_str) == Some(id))
        .ok_or_else(|| t("noServer", &[("id", &id)]))
}

fn server_of(s: &Value, key: &str) -> Result<mediaserver::Server, String> {
    serde_json::from_value(json!({
        "kind": s.get("kind").cloned().unwrap_or(Value::from("jellyfin")),
        "url": s.get("url").and_then(Value::as_str).unwrap_or_default().trim(),
        "key": key,
        "folders": s.get("folders").cloned().unwrap_or(Value::Array(vec![])),
    }))
    .map_err(|e| e.to_string())
}

/// Reads a server's libraries and pairs their folders: kept pairs stay,
/// a path that is a folder here is paired with itself.
async fn read(server: &mut Value, key: &str) -> Result<(usize, usize), String> {
    let s = server_of(server, key)?;
    let libs = s
        .libraries()
        .await
        .map_err(|e| crate::text::error_text(&e))?;
    let old: Vec<Value> = server
        .get("folders")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut folders = Vec::new();
    for lib in &libs {
        for p in &lib.paths {
            let kept = old
                .iter()
                .find(|f| f["libraryId"] == lib.id.as_str() && f["server"] == p.path.as_str());
            folders.push(match kept {
                Some(f) => {
                    let mut f = f.clone();
                    f["library"] = Value::from(lib.name.clone());
                    f
                }
                None => json!({ "libraryId": lib.id, "library": lib.name, "server": p.path, "local": if p.here { p.path.clone() } else { String::new() }, "on": true }),
            });
        }
    }
    let n = folders.len();
    server["folders"] = Value::Array(folders);
    Ok((libs.len(), n))
}

pub fn command(m: &ArgMatches, rt: &tokio::runtime::Runtime) -> Result<Exit, String> {
    let keys = keys();
    match m.subcommand() {
        Some(("add", s)) => {
            let kind = s.get_one::<String>("kind").unwrap().clone();
            let url = s.get_one::<String>("url").unwrap().trim().to_string();
            let key = read_key(s)?;
            let id = format!(
                "srv{:x}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            );
            let mut server = json!({ "id": id, "kind": kind, "url": url, "folders": [] });
            let (libs, folders) = rt.block_on(read(&mut server, &key))?;
            save_key(&id, Some(key))?;
            let mut now = settings_now();
            servers_of(&mut now).push(server);
            save(&now)?;
            out::note(&format!(
                "{id}: {}",
                t("serverRead", &[("libs", &libs), ("folders", &folders)])
            ));
        }
        Some(("remove", s)) => {
            let id = s.get_one::<String>("id").unwrap();
            let mut now = settings_now();
            let list = servers_of(&mut now);
            let before = list.len();
            list.retain(|x| x.get("id").and_then(Value::as_str) != Some(id));
            if list.len() == before {
                return Err(t("noServer", &[("id", id)]));
            }
            save(&now)?;
            save_key(id, None)?;
            out::note(&t("serverRemoved", &[("id", id)]));
        }
        Some(("libraries", s)) => {
            let id = s.get_one::<String>("id").unwrap();
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
        }
        Some(("pair", s)) => {
            let id = s.get_one::<String>("id").unwrap();
            let server_path = s.get_one::<String>("server").unwrap();
            let local = std::path::absolute(s.get_one::<String>("local").unwrap())
                .map_err(|e| e.to_string())?;
            let mut now = settings_now();
            let server = find(servers_of(&mut now), id)?;
            let folder = folder_of(server, server_path)?;
            folder["local"] = Value::from(local.display().to_string());
            save(&now)?;
        }
        Some((on @ ("on" | "off"), s)) => {
            let id = s.get_one::<String>("id").unwrap();
            let server_path = s.get_one::<String>("server").unwrap();
            let mut now = settings_now();
            let server = find(servers_of(&mut now), id)?;
            folder_of(server, server_path)?["on"] = Value::from(on == "on");
            save(&now)?;
        }
        Some(("test", s)) => {
            let id = s.get_one::<String>("id").unwrap();
            let mut now = settings_now();
            let server = find(servers_of(&mut now), id)?;
            let key = keys.get(id).and_then(Value::as_str).unwrap_or_default();
            rt.block_on(server_of(server, key)?.check())
                .map_err(|e| crate::text::error_text(&e))?;
            out::note(&t("serverOk", &[]));
        }
        _ => {
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
