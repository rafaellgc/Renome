//! Google Books client: editions by ISBN and books by title and author,
//! with the user's own key (the shared quota without one runs out early
//! each day; with one, about 1,000 requests a day). Google's terms allow
//! no keeping of its answers beyond what its replies say, and they say
//! none, so answers are kept only while the program is open.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use renome_core::books::GoogleBook;
use serde_json::Value;

const API: &str = "https://www.googleapis.com/books/v1";

/// Tests point this at a local server with `RENOME_GB_API`.
fn api_base() -> String {
    std::env::var("RENOME_GB_API").unwrap_or_else(|_| API.into())
}

pub struct Client {
    http: reqwest::Client,
    key: Mutex<String>,
    last: tokio::sync::Mutex<Option<Instant>>,
    /// Answers by question (without the key), for the session only.
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
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            key: Mutex::new(String::new()),
            last: tokio::sync::Mutex::new(None),
            cache: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_connect() {
        "gbOffline".into()
    } else if e.is_timeout() {
        "gbTimeout".into()
    } else {
        format!("gbNetwork: {}", e.without_url())
    }
}

/// Words only: quotes and the search's own signs would change the search.
fn words(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '\'' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

impl Client {
    /// The key from the options; empty: Google Books is not used.
    pub fn set_key(&self, key: &str) {
        if let Ok(mut k) = self.key.lock() {
            *k = key.trim().to_string();
        }
    }

    fn key(&self) -> String {
        self.key.lock().map(|k| k.clone()).unwrap_or_default()
    }

    pub fn enabled(&self) -> bool {
        !self.key().is_empty()
    }

    /// The books for a search (`q`), half a second apart at least.
    async fn volumes(&self, key: &str, q: &str, lang: &str) -> Result<Value, String> {
        let id = format!("{lang} {q}");
        if let Some(v) = self.cache.lock().await.get(&id) {
            return Ok(v.clone());
        }
        let mut query = vec![
            ("q", q),
            ("maxResults", "10"),
            ("printType", "books"),
            ("key", key),
        ];
        if !lang.is_empty() {
            query.push(("langRestrict", lang));
        }
        let mut last = self.last.lock().await;
        if let Some(t) = *last {
            tokio::time::sleep(Duration::from_millis(500).saturating_sub(t.elapsed())).await;
        }
        let res = self
            .http
            .get(format!("{}/volumes", api_base()))
            .query(&query)
            .send()
            .await;
        *last = Some(Instant::now());
        drop(last);
        let res = res.map_err(net_error)?;
        let status = res.status().as_u16();
        let body: Value = res.json().await.map_err(net_error)?;
        let reason = body["error"]["errors"][0]["reason"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        match status {
            200 => {}
            400 if reason == "keyInvalid" || reason == "badRequest" => {
                return Err("gbBadKey".into());
            }
            403 if reason.contains("Exceeded") || reason.contains("Limit") => {
                return Err("gbBusy".into());
            }
            400 | 401 | 403 => return Err("gbBadKey".into()),
            429 => return Err("gbBusy".into()),
            code => return Err(format!("gbNetwork: HTTP {code}")),
        }
        self.cache.lock().await.insert(id, body.clone());
        Ok(body)
    }

    fn books(v: &Value) -> Vec<GoogleBook> {
        v["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|i| serde_json::from_value(i["volumeInfo"].clone()).ok())
            .collect()
    }

    /// The edition of an ISBN; `None` when Google does not know it.
    pub async fn by_isbn(&self, isbn: &str) -> Result<Option<GoogleBook>, String> {
        let key = self.key();
        if key.is_empty() {
            return Ok(None);
        }
        let v = self.volumes(&key, &format!("isbn:{isbn}"), "").await?;
        Ok(Self::books(&v).into_iter().next())
    }

    /// Books with a title (and by an author, by surname), editions in the
    /// reader's language first: asked in it, then in any.
    pub async fn search(
        &self,
        title: &str,
        author: &str,
        lang: &str,
    ) -> Result<Vec<GoogleBook>, String> {
        let key = self.key();
        let title = words(title);
        if key.is_empty() || title.is_empty() {
            return Ok(Vec::new());
        }
        let surname = renome_core::books::surname(author);
        let q = match surname.is_empty() {
            true => format!("intitle:{title}"),
            false => format!("intitle:{title} inauthor:{surname}"),
        };
        let lang = lang.split('-').next().unwrap_or("");
        let mut found = Self::books(&self.volumes(&key, &q, lang).await?);
        if found.is_empty() && !lang.is_empty() {
            found = Self::books(&self.volumes(&key, &q, "").await?);
        }
        Ok(found)
    }

    /// Whether a key works.
    pub async fn check(&self, key: &str) -> Result<(), String> {
        self.cache.lock().await.clear();
        self.volumes(key.trim(), "isbn:9780306406157", "")
            .await
            .map(|_| ())
    }

    /// A cover's picture.
    pub async fn cover(&self, url: &str) -> Result<Vec<u8>, String> {
        let res = self.http.get(url).send().await.map_err(net_error)?;
        if !res.status().is_success() {
            return Err(format!("gbNetwork: HTTP {}", res.status().as_u16()));
        }
        let kind = res
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        if !kind.starts_with("image/") {
            return Err("gbNetwork: no picture".into());
        }
        Ok(res.bytes().await.map_err(net_error)?.to_vec())
    }
}
