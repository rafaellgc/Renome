//! Grand Comics Database client (comics.org): series by name and the
//! numbers of their issues. Without an account the service allows about
//! 30 requests an hour, for testing; with one, about 25 a minute. The
//! account's email and password are kept in memory only, never written to
//! disk, and forgotten when the program closes.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use renome_core::comics::{Issue, Volume, issue_number};
use serde_json::Value;

const API: &str = "https://www.comics.org/api";

/// Tests point this at a local server with `RENOME_GCD_API`.
fn api_base() -> String {
    std::env::var("RENOME_GCD_API").unwrap_or_else(|_| API.into())
}

pub struct Client {
    http: reqwest::Client,
    last: tokio::sync::Mutex<Option<Instant>>,
    /// The account signed in for this session (email, password).
    login: std::sync::Mutex<Option<(String, String)>>,
    /// Answers by request, for the session.
    cache: tokio::sync::Mutex<HashMap<String, Value>>,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!(
                    "Renome/",
                    env!("CARGO_PKG_VERSION"),
                    " ( https://github.com/rafaellgc/Renome )"
                ))
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(40))
                .build()
                .unwrap_or_default(),
            last: tokio::sync::Mutex::new(None),
            login: std::sync::Mutex::new(None),
            cache: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_connect() {
        "gcdOffline".into()
    } else if e.is_timeout() {
        "gcdTimeout".into()
    } else {
        format!("gcdNetwork: {}", e.without_url())
    }
}

/// The id at the end of an API link (`…/series/194284/?format=json`).
fn id_of(url: &str) -> String {
    url.split('/')
        .rev()
        .find(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or_default()
        .to_string()
}

fn volume(v: &Value) -> Volume {
    let issues = v["active_issues"].as_array().map_or(0, Vec::len);
    Volume {
        id: id_of(v["api_url"].as_str().unwrap_or_default()),
        name: v["name"].as_str().unwrap_or_default().trim().to_string(),
        start_year: v["year_began"].as_u64().and_then(|y| u16::try_from(y).ok()),
        count: u32::try_from(issues).unwrap_or_default(),
        country: v["country"].as_str().unwrap_or_default().to_string(),
        language: v["language"].as_str().unwrap_or_default().to_string(),
        format: v["publishing_format"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string(),
        ..Default::default()
    }
}

impl Client {
    pub fn sign_in(&self, email: String, password: String) {
        *self.login.lock().unwrap() = Some((email, password));
    }

    pub fn sign_out(&self) {
        *self.login.lock().unwrap() = None;
    }

    /// A request at least 2.5 seconds after the last (25 a minute with an
    /// account); a busy service is not asked again: its limit is per hour.
    async fn get(&self, url: &str) -> Result<Value, String> {
        if let Some(v) = self.cache.lock().await.get(url) {
            return Ok(v.clone());
        }
        let login = self.login.lock().unwrap().clone();
        let mut last = self.last.lock().await;
        if let Some(t) = *last {
            tokio::time::sleep(Duration::from_millis(2500).saturating_sub(t.elapsed())).await;
        }
        let mut req = self.http.get(url);
        if let Some((email, password)) = &login {
            req = req.basic_auth(email, Some(password));
        }
        let res = req.send().await;
        *last = Some(Instant::now());
        drop(last);
        let res = res.map_err(net_error)?;
        match res.status().as_u16() {
            200 => {}
            401 | 403 if login.is_some() => return Err("gcdBadLogin".into()),
            404 => return Err("gcdNotFound".into()),
            429 if login.is_some() => return Err("gcdBusy".into()),
            429 => return Err("gcdBusyAnonymous".into()),
            code => return Err(format!("gcdNetwork: HTTP {code}")),
        }
        let v: Value = res.json().await.map_err(net_error)?;
        self.cache.lock().await.insert(url.to_string(), v.clone());
        Ok(v)
    }

    /// Whether the account signed in works: a request for one publisher
    /// (that exists) answers, or refuses the account.
    pub async fn check(&self) -> Result<(), String> {
        self.get(&format!("{}/publisher/4999/?format=json", api_base()))
            .await
            .map(|_| ())
    }

    /// Series whose names contain the words, best first (two pages at
    /// most: a name that common needs more words).
    pub async fn search(&self, query: &str, lang: &str) -> Result<Vec<Volume>, String> {
        let mut first = reqwest::Url::parse(&format!("{}/series/name/", api_base()))
            .map_err(|e| format!("gcdNetwork: {e}"))?;
        first
            .path_segments_mut()
            .map_err(|_| "gcdNetwork: URL".to_string())?
            .pop_if_empty()
            .push(query.trim())
            .push("");
        first.set_query(Some("format=json"));
        let mut url = first.to_string();
        let mut found = Vec::new();
        for _ in 0..2 {
            let page = self.get(&url).await?;
            found.extend(page["results"].as_array().into_iter().flatten().map(volume));
            match page["next"].as_str() {
                Some(next) if !next.is_empty() => url = next.to_string(),
                _ => break,
            }
        }
        Ok(renome_core::comics::rank_volumes(query, found, lang))
    }

    /// A series with its publisher, and its issues' numbers.
    pub async fn issues(&self, id: &str) -> Result<(Volume, Vec<Issue>), String> {
        let v = self
            .get(&format!("{}/series/{id}/?format=json", api_base()))
            .await?;
        let mut vol = volume(&v);
        if vol.id.is_empty() {
            vol.id = id.to_string();
        }
        if let Some(publisher) = v["publisher"].as_str().filter(|p| !p.is_empty())
            && let Ok(p) = self.get(publisher).await
        {
            vol.publisher = p["name"].as_str().unwrap_or_default().trim().to_string();
        }
        let urls = v["active_issues"].as_array().cloned().unwrap_or_default();
        let descriptors = v["issue_descriptors"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let issues = urls
            .iter()
            .enumerate()
            .map(|(i, u)| Issue {
                id: id_of(u.as_str().unwrap_or_default()),
                number: issue_number(
                    descriptors
                        .get(i)
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                ),
                ..Default::default()
            })
            .collect();
        Ok((vol, issues))
    }
}
