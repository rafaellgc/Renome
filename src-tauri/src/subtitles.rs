//! Subtitles from OpenSubtitles.com (the user's own API key, and account
//! for more downloads a day) and SubDL (the user's own key): searched by
//! the TMDB number of the movie or show (season and episode for an
//! episode), and on OpenSubtitles also by the video's fingerprint, which
//! finds those timed for that very file. Searching costs nothing;
//! downloads count against a daily quota, so they happen only when the
//! files are renamed, for the subtitles chosen.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use renome_core::subtitles::{self as subs, Candidate, Lang, Source};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};

const OS_API: &str = "https://api.opensubtitles.com/api/v1";
const SUBDL_API: &str = "https://api.subdl.com/api/v1";
const SUBDL_DL: &str = "https://dl.subdl.com";

/// Tests point these at a local server.
fn os_api() -> Option<String> {
    std::env::var("RENOME_OPENSUBTITLES_API").ok()
}
fn subdl_api() -> String {
    std::env::var("RENOME_SUBDL_API").unwrap_or_else(|_| SUBDL_API.into())
}
fn subdl_dl() -> String {
    std::env::var("RENOME_SUBDL_DL").unwrap_or_else(|_| SUBDL_DL.into())
}

/// The keys and account from the options.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auth {
    #[serde(default)]
    pub os_key: String,
    #[serde(default)]
    pub os_user: String,
    #[serde(default)]
    pub os_password: String,
    #[serde(default)]
    pub subdl_key: String,
}

impl Auth {
    fn os(&self) -> bool {
        !self.os_key.trim().is_empty()
    }
    fn subdl(&self) -> bool {
        !self.subdl_key.trim().is_empty()
    }
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        // OpenSubtitles asks for the program's name and version.
        .user_agent(concat!("Renome v", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

fn net_error(prefix: &str, e: &reqwest::Error) -> String {
    if e.is_connect() {
        format!("{prefix}Offline")
    } else if e.is_timeout() {
        format!("{prefix}Timeout")
    } else {
        format!(
            "{prefix}Network: {}",
            e.to_string().split(" for url").next().unwrap_or("")
        )
    }
}

/// A login, kept for the session (tokens last a day): the account it is
/// for, the token and the server to ask from then on.
struct Login {
    who: (String, String),
    token: String,
    base: String,
}

static LOGIN: std::sync::Mutex<Option<Login>> = std::sync::Mutex::new(None);
/// OpenSubtitles allows about five requests a second.
static LAST_OS: tokio::sync::Mutex<Option<Instant>> = tokio::sync::Mutex::const_new(None);

async fn os_pace() {
    let mut last = LAST_OS.lock().await;
    if let Some(t) = *last {
        tokio::time::sleep(Duration::from_millis(250).saturating_sub(t.elapsed())).await;
    }
    *last = Some(Instant::now());
}

/// What OpenSubtitles' answers mean, as error codes the window translates.
fn os_status(code: u16, body: &str) -> String {
    match code {
        401 => "osBadLogin".into(),
        403 => "osBadKey".into(),
        406 => "osQuota".into(),
        429 => "osBusy".into(),
        _ => {
            let msg = serde_json::from_str::<Value>(body)
                .ok()
                .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_string))
                .unwrap_or_default();
            format!("osNetwork: HTTP {code} {msg}").trim().to_string()
        }
    }
}

struct Os {
    http: reqwest::Client,
    auth: Auth,
}

impl Os {
    /// The server to ask and the token, logging in first when the options
    /// have an account (without one, downloads count against the
    /// computer's address: 5 a day).
    async fn session(&self, again: bool) -> Result<(String, Option<String>), String> {
        let base = os_api().unwrap_or_else(|| OS_API.into());
        let who = (
            self.auth.os_user.trim().to_string(),
            self.auth.os_password.clone(),
        );
        if who.0.is_empty() {
            return Ok((base, None));
        }
        if !again
            && let Some(l) = LOGIN.lock().unwrap().as_ref()
            && l.who == who
        {
            return Ok((l.base.clone(), Some(l.token.clone())));
        }
        os_pace().await;
        let res = self
            .http
            .post(format!("{base}/login"))
            .header("Api-Key", self.auth.os_key.trim())
            .header("Accept", "application/json")
            .json(&serde_json::json!({ "username": who.0, "password": who.1 }))
            .send()
            .await
            .map_err(|e| net_error("os", &e))?;
        let code = res.status().as_u16();
        let body = res.text().await.map_err(|e| net_error("os", &e))?;
        if code != 200 {
            return Err(os_status(code, &body));
        }
        let v: Value = serde_json::from_str(&body).map_err(|e| format!("osNetwork: {e}"))?;
        let token = v["token"].as_str().unwrap_or_default().to_string();
        // A VIP account is sent to another server.
        let base = match (os_api(), v["base_url"].as_str()) {
            (None, Some(host)) if !host.is_empty() => {
                format!("https://{}/api/v1", host.trim_end_matches('/'))
            }
            _ => base,
        };
        *LOGIN.lock().unwrap() = Some(Login {
            who,
            token: token.clone(),
            base: base.clone(),
        });
        Ok((base, Some(token)))
    }

    /// A request, with the key and the token; asked again after a pause
    /// when the service is busy, and after a new login when the token
    /// expired.
    async fn send(
        &self,
        get: bool,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Value, String> {
        let mut again = false;
        for attempt in 0..4 {
            let (base, token) = self.session(again).await?;
            os_pace().await;
            let mut req = if get {
                self.http.get(format!("{base}{path}")).query(query)
            } else {
                self.http
                    .post(format!("{base}{path}"))
                    .json(&body.clone().unwrap_or_default())
            }
            .header("Api-Key", self.auth.os_key.trim())
            .header("Accept", "application/json");
            if let Some(t) = &token {
                req = req.bearer_auth(t);
            }
            let res = req.send().await.map_err(|e| net_error("os", &e))?;
            let code = res.status().as_u16();
            let text = res.text().await.map_err(|e| net_error("os", &e))?;
            match code {
                200 => return serde_json::from_str(&text).map_err(|e| format!("osNetwork: {e}")),
                401 if token.is_some() && !again => again = true,
                429 | 503 if attempt < 3 => {
                    tokio::time::sleep(Duration::from_secs(1 + attempt)).await
                }
                _ => return Err(os_status(code, &text)),
            }
        }
        Err("osBusy".into())
    }

    /// The downloads left today, when the account says.
    async fn remaining(&self) -> Option<i64> {
        if self.auth.os_user.trim().is_empty() {
            return None;
        }
        let v = self.send(true, "/infos/user", &[], None).await.ok()?;
        v["data"]["remaining_downloads"].as_i64()
    }

    async fn search(
        &self,
        item: &Item,
        langs: &[Lang],
        hash: Option<&str>,
    ) -> Result<Vec<Candidate>, String> {
        let codes: Vec<&str> = {
            let mut c: Vec<&str> = langs.iter().map(|l| l.opensubtitles.as_str()).collect();
            c.sort_unstable();
            c
        };
        // In alphabetical order, as the service asks.
        let mut q: Vec<(&str, String)> = vec![("ai_translated", "exclude".into())];
        if let Some(e) = item.episode {
            q.push(("episode_number", e.to_string()));
        }
        q.push(("foreign_parts_only", "exclude".into()));
        q.push(("languages", codes.join(",")));
        q.push(("machine_translated", "exclude".into()));
        if let Some(h) = hash {
            q.push(("moviehash", h.to_string()));
        }
        match (item.season, item.episode) {
            (Some(s), Some(_)) => {
                q.push(("parent_tmdb_id", item.tmdb.to_string()));
                q.push(("season_number", s.to_string()));
            }
            _ => q.push(("tmdb_id", item.tmdb.to_string())),
        }
        let v = self.send(true, "/subtitles", &q, None).await?;
        let mut out = Vec::new();
        for d in v["data"].as_array().into_iter().flatten() {
            let a = &d["attributes"];
            let Some(lang) = langs.iter().find(|l| {
                a["language"]
                    .as_str()
                    .is_some_and(|x| x.eq_ignore_ascii_case(&l.opensubtitles))
            }) else {
                continue;
            };
            // An episode's must be that episode.
            if let (Some(s), Some(e)) = (item.season, item.episode) {
                let f = &a["feature_details"];
                if f["season_number"]
                    .as_u64()
                    .is_some_and(|x| x != u64::from(s))
                    || f["episode_number"]
                        .as_u64()
                        .is_some_and(|x| x != u64::from(e))
                {
                    continue;
                }
            }
            // One file: a subtitle in parts (CD1, CD2) is for a video in parts.
            let files = a["files"].as_array().map(Vec::as_slice).unwrap_or_default();
            let [file] = files else { continue };
            let Some(id) = file["file_id"].as_u64() else {
                continue;
            };
            out.push(Candidate {
                source: Source::OpenSubtitles,
                id: id.to_string(),
                lang: lang.tag.clone(),
                release: a["release"].as_str().unwrap_or_default().to_string(),
                downloads: a["download_count"].as_u64().unwrap_or_default(),
                exact: a["moviehash_match"].as_bool().unwrap_or(false),
                hearing_impaired: a["hearing_impaired"].as_bool().unwrap_or(false),
                trusted: a["from_trusted"].as_bool().unwrap_or(false),
                url: a["url"].as_str().unwrap_or_default().to_string(),
            });
        }
        Ok(out)
    }

    /// The subtitle's bytes and the downloads left today.
    async fn download(&self, c: &Candidate) -> Result<(Vec<u8>, String, Option<i64>), String> {
        let id: u64 = c.id.parse().map_err(|_| "osNetwork: id".to_string())?;
        let v = self
            .send(
                false,
                "/download",
                &[],
                Some(serde_json::json!({ "file_id": id })),
            )
            .await?;
        let link = v["link"].as_str().ok_or("osQuota")?;
        let name = v["file_name"].as_str().unwrap_or("x.srt");
        let ext = name.rsplit_once('.').map_or("srt", |(_, e)| e).to_string();
        let bytes = self
            .http
            .get(link)
            .send()
            .await
            .map_err(|e| net_error("os", &e))?;
        if !bytes.status().is_success() {
            return Err(os_status(bytes.status().as_u16(), ""));
        }
        let bytes = bytes.bytes().await.map_err(|e| net_error("os", &e))?;
        Ok((bytes.to_vec(), ext, v["remaining"].as_i64()))
    }
}

struct Subdl {
    http: reqwest::Client,
    key: String,
}

fn subdl_status(code: u16, body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or_default();
    let msg = v["error"]
        .as_str()
        .or(v["message"].as_str())
        .unwrap_or_default()
        .to_lowercase();
    match code {
        _ if msg.contains("api key") || msg.contains("api_key") || msg.contains("invalid key") => {
            "subdlBadKey".into()
        }
        401 | 403 => "subdlBadKey".into(),
        429 if msg.contains("limit") => "subdlQuota".into(),
        429 => "subdlBusy".into(),
        _ => format!("subdlNetwork: HTTP {code} {msg}")
            .trim()
            .to_string(),
    }
}

impl Subdl {
    async fn search(&self, item: &Item, langs: &[Lang]) -> Result<Vec<Candidate>, String> {
        let codes: Vec<&str> = langs.iter().map(|l| l.subdl.as_str()).collect();
        let mut q: Vec<(&str, String)> = vec![
            ("api_key", self.key.trim().to_string()),
            ("tmdb_id", item.tmdb.to_string()),
            ("languages", codes.join(",")),
            ("subs_per_page", "30".into()),
            ("releases", "1".into()),
            ("hi", "1".into()),
        ];
        match (item.season, item.episode) {
            (Some(s), Some(e)) => {
                q.push(("type", "tv".into()));
                q.push(("season_number", s.to_string()));
                q.push(("episode_number", e.to_string()));
            }
            _ => q.push(("type", "movie".into())),
        }
        let mut res = None;
        for attempt in 0..3u64 {
            let r = self
                .http
                .get(format!("{}/subtitles", subdl_api()))
                .query(&q)
                .send()
                .await
                .map_err(|e| net_error("subdl", &e))?;
            let code = r.status().as_u16();
            let text = r.text().await.map_err(|e| net_error("subdl", &e))?;
            if code == 429 && attempt < 2 && !text.to_lowercase().contains("limit") {
                tokio::time::sleep(Duration::from_secs(5 * (attempt + 1))).await;
                continue;
            }
            res = Some((code, text));
            break;
        }
        let (code, text) = res.ok_or("subdlBusy")?;
        if code != 200 {
            return Err(subdl_status(code, &text));
        }
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("subdlNetwork: {e}"))?;
        if v["status"] == false || v["success"] == false {
            let msg = v["error"]
                .as_str()
                .or(v["message"].as_str())
                .unwrap_or_default()
                .to_lowercase();
            // Nothing for it: not an error.
            if msg.contains("find") || msg.contains("not found") || msg.is_empty() {
                return Ok(Vec::new());
            }
            return Err(subdl_status(200, &text));
        }
        let mut out = Vec::new();
        for s in v["subtitles"].as_array().into_iter().flatten() {
            let code = s["language"].as_str().unwrap_or_default();
            let Some(lang) = langs.iter().find(|l| l.subdl.eq_ignore_ascii_case(code)) else {
                continue;
            };
            let Some(url) = s["url"].as_str().filter(|u| u.starts_with('/')) else {
                continue;
            };
            if let (Some(se), Some(ep)) = (item.season, item.episode)
                && (s["season"].as_u64().is_some_and(|x| x != u64::from(se))
                    || (s["full_season"] != true
                        && s["episode"].as_u64().is_some_and(|x| x != u64::from(ep))))
            {
                continue;
            }
            let release = s["releases"]
                .as_array()
                .and_then(|r| r.first())
                .and_then(Value::as_str)
                .or(s["release_name"].as_str())
                .or(s["name"].as_str())
                .unwrap_or_default();
            out.push(Candidate {
                source: Source::Subdl,
                id: url.to_string(),
                lang: lang.tag.clone(),
                release: release.to_string(),
                downloads: 0,
                exact: false,
                hearing_impaired: s["hi"].as_bool().unwrap_or(false),
                trusted: false,
                url: s["subtitlePage"]
                    .as_str()
                    .map(|p| format!("https://subdl.com{p}"))
                    .unwrap_or_default(),
            });
        }
        Ok(out)
    }

    async fn download(&self, c: &Candidate) -> Result<Vec<u8>, String> {
        let r = self
            .http
            .get(format!("{}{}", subdl_dl(), c.id))
            .send()
            .await
            .map_err(|e| net_error("subdl", &e))?;
        let code = r.status().as_u16();
        if code != 200 {
            let text = r.text().await.unwrap_or_default();
            return Err(subdl_status(code, &text));
        }
        Ok(r.bytes()
            .await
            .map_err(|e| net_error("subdl", &e))?
            .to_vec())
    }
}

/// A video to find subtitles for: its movie's (or show's) TMDB number,
/// and the season and episode of an episode.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub path: PathBuf,
    pub tmdb: u64,
    #[serde(default)]
    pub season: Option<u32>,
    #[serde(default)]
    pub episode: Option<u32>,
}

/// What was found for a video.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub path: PathBuf,
    /// The languages asked for that it has already (beside or inside).
    pub have: Vec<String>,
    /// Best first, every language.
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Search {
    /// The languages asked for, as understood (`pt-BR`, `en`).
    pub langs: Vec<String>,
    pub found: Vec<Found>,
    /// What failed, once per source (a wrong key, the service down).
    pub errors: Vec<String>,
    /// OpenSubtitles downloads left today, when known.
    pub remaining: Option<i64>,
}

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
}

/// Searches each video on the sources with a key, for the languages not
/// there yet. Stops between videos when asked.
pub async fn search(
    app: &AppHandle,
    auth: Auth,
    langs: &str,
    items: Vec<Item>,
    stop: impl Fn() -> bool,
) -> Search {
    let langs = subs::langs(langs);
    let http = http();
    let os = auth.os().then(|| Os {
        http: http.clone(),
        auth: auth.clone(),
    });
    let subdl = auth.subdl().then(|| Subdl {
        http: http.clone(),
        key: auth.subdl_key.clone(),
    });
    let (mut os_ok, mut subdl_ok) = (true, true);
    let mut out = Search {
        langs: langs.iter().map(|l| l.tag.clone()).collect(),
        ..Search::default()
    };
    let total = items.len();
    let _ = app.emit("subs-progress", Progress { done: 0, total });
    for (i, item) in items.into_iter().enumerate() {
        if stop() {
            break;
        }
        let video = item.path.clone();
        let (present, hash) =
            tokio::task::spawn_blocking(move || (subs::present(&video), subs::hash(&video)))
                .await
                .unwrap_or_default();
        let have: Vec<String> = langs
            .iter()
            .filter(|l| present.iter().any(|p| subs::same_lang(p, l)))
            .map(|l| l.tag.clone())
            .collect();
        let missing: Vec<Lang> = langs
            .iter()
            .filter(|l| !have.contains(&l.tag))
            .cloned()
            .collect();
        let mut candidates = Vec::new();
        if !missing.is_empty() {
            if let Some(os) = os.as_ref().filter(|_| os_ok) {
                match os.search(&item, &missing, hash.as_deref()).await {
                    Ok(c) => candidates.extend(c),
                    Err(e) => {
                        os_ok = !(e == "osBadKey" || e == "osBadLogin" || e.ends_with("Offline"));
                        if !out.errors.contains(&e) {
                            out.errors.push(e);
                        }
                    }
                }
            }
            if let Some(sd) = subdl.as_ref().filter(|_| subdl_ok) {
                match sd.search(&item, &missing).await {
                    Ok(c) => candidates.extend(c),
                    Err(e) => {
                        subdl_ok =
                            !(e == "subdlBadKey" || e == "subdlQuota" || e.ends_with("Offline"));
                        if !out.errors.contains(&e) {
                            out.errors.push(e);
                        }
                    }
                }
            }
        }
        let name = item
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        subs::rank(&name, &mut candidates);
        out.found.push(Found {
            path: item.path,
            have,
            candidates,
        });
        let _ = app.emit("subs-progress", Progress { done: i + 1, total });
    }
    if let Some(os) = os.filter(|_| os_ok) {
        out.remaining = os.remaining().await;
    }
    out
}

/// A renamed video and its subtitles to download: for each language, the
/// one chosen first, then the others to try when it cannot be had.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Write {
    pub path: PathBuf,
    #[serde(default)]
    pub season: Option<u32>,
    #[serde(default)]
    pub episode: Option<u32>,
    pub langs: Vec<Vec<Candidate>>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Written {
    pub created: Vec<PathBuf>,
    /// File name and reason.
    pub errors: Vec<(String, String)>,
    pub remaining: Option<i64>,
}

/// How many to try per language before giving up on it.
const TRIES: usize = 3;

/// Downloads the subtitles beside the renamed videos (a name taken is
/// never replaced). When OpenSubtitles' quota runs out, SubDL's are tried.
pub async fn write(
    app: &AppHandle,
    auth: Auth,
    items: Vec<Write>,
    stop: impl Fn() -> bool,
) -> Written {
    let http = http();
    let os = auth.os().then(|| Os {
        http: http.clone(),
        auth: auth.clone(),
    });
    let subdl = auth.subdl().then(|| Subdl {
        http: http.clone(),
        key: auth.subdl_key.clone(),
    });
    let (mut os_ok, mut subdl_ok) = (true, true);
    let mut out = Written::default();
    let total = items.iter().map(|i| i.langs.len()).sum();
    let mut done = 0;
    let _ = app.emit("subs-progress", Progress { done, total });
    'items: for item in items {
        let name = item
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        for list in &item.langs {
            if stop() {
                break 'items;
            }
            let mut last_err = None;
            let mut tried = 0;
            for c in list {
                if tried == TRIES {
                    break;
                }
                let got = match c.source {
                    Source::OpenSubtitles => match os.as_ref().filter(|_| os_ok) {
                        Some(os) => os.download(c).await.map(|(b, ext, left)| {
                            if left.is_some() {
                                out.remaining = left;
                            }
                            (b, ext)
                        }),
                        None => continue,
                    },
                    Source::Subdl => match subdl.as_ref().filter(|_| subdl_ok) {
                        Some(sd) => sd
                            .download(c)
                            .await
                            .and_then(|b| {
                                let ep = item.season.zip(item.episode);
                                subs::from_archive(&b, ep).ok_or_else(|| "subsEmpty".to_string())
                            })
                            .map(|(ext, b)| (b, ext)),
                        None => continue,
                    },
                };
                tried += 1;
                match got {
                    Ok((bytes, ext)) => {
                        let to = subs::target(&item.path, &c.lang, c.hearing_impaired, &ext);
                        let text = ["srt", "ass", "ssa", "vtt", "sub"]
                            .contains(&ext.to_lowercase().as_str());
                        let bytes = if text {
                            renome_core::subtext::to_utf8(&bytes, Some(&c.lang))
                        } else {
                            bytes
                        };
                        match write_new(&to, &bytes) {
                            Ok(()) => out.created.push(to),
                            Err(e) => {
                                let file = to
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default();
                                out.errors.push((file, e));
                            }
                        }
                        last_err = None;
                        break;
                    }
                    Err(e) => {
                        // Out of downloads or a wrong key: that source is
                        // done for the rest.
                        match e.as_str() {
                            "osQuota" | "osBadKey" | "osBadLogin" => os_ok = false,
                            "subdlQuota" | "subdlBadKey" => subdl_ok = false,
                            _ => {}
                        }
                        if e == "osQuota" {
                            out.remaining = Some(0);
                        }
                        last_err = Some(e);
                    }
                }
            }
            if let Some(e) = last_err {
                out.errors.push((
                    format!(
                        "{name} ({})",
                        list.first().map(|c| c.lang.as_str()).unwrap_or("")
                    ),
                    e,
                ));
            }
            done += 1;
            let _ = app.emit("subs-progress", Progress { done, total });
        }
    }
    out
}

/// Writes a file that must not be there yet.
fn write_new(to: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write as _;
    let mut f = std::fs::File::options()
        .write(true)
        .create_new(true)
        .open(to)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                "err.targetExists".to_string()
            } else {
                format!("err.io: {e}")
            }
        })?;
    f.write_all(bytes).map_err(|e| format!("err.io: {e}"))?;
    f.sync_all().map_err(|e| format!("err.io: {e}"))
}

/// Checks OpenSubtitles' key (and account, when given): the downloads
/// left today, when the account says.
pub async fn check_os(auth: Auth) -> Result<Option<i64>, String> {
    if !auth.os() {
        return Err("osBadKey".into());
    }
    *LOGIN.lock().unwrap() = None;
    let os = Os { http: http(), auth };
    // A search costs nothing and needs the key.
    os.send(true, "/infos/formats", &[], None).await?;
    if os.auth.os_user.trim().is_empty() {
        return Ok(None);
    }
    os.session(true).await?;
    Ok(os.remaining().await)
}

/// Checks SubDL's key with a search.
pub async fn check_subdl(key: String) -> Result<(), String> {
    let sd = Subdl { http: http(), key };
    let item = Item {
        path: PathBuf::new(),
        tmdb: 0,
        season: None,
        episode: None,
    };
    sd.search(&item, &subs::langs("en")).await.map(|_| ())
}
