//! MangaDex client: manga series by title, with no key. MangaDex lists the
//! chapters its users upload, not a publisher's volumes, so only the
//! series is looked up: the volume or chapter stays the one in the file.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use renome_core::comics::Volume;
use serde_json::Value;

const API: &str = "https://api.mangadex.org";
const COVERS: &str = "https://uploads.mangadex.org/covers";

/// Tests point this at a local server with `RENOME_MANGADEX_API`.
fn api_base() -> String {
    std::env::var("RENOME_MANGADEX_API").unwrap_or_else(|_| API.into())
}

pub struct Client {
    http: reqwest::Client,
    last: tokio::sync::Mutex<Option<Instant>>,
    cache: tokio::sync::Mutex<HashMap<String, Vec<Volume>>>,
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
            last: tokio::sync::Mutex::new(None),
            cache: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_connect() {
        "mdOffline".into()
    } else if e.is_timeout() {
        "mdTimeout".into()
    } else {
        format!("mdNetwork: {}", e.without_url())
    }
}

/// The title to show and name by: the one in the reader's language, else
/// in English, else the main one (often in romaji).
fn title(a: &Value, lang: &str) -> String {
    let want = lang.to_lowercase();
    let short = want.split('-').next().unwrap_or("").to_string();
    let mut titles: Vec<(String, String)> = Vec::new();
    for t in [&a["title"]]
        .into_iter()
        .chain(a["altTitles"].as_array().into_iter().flatten())
    {
        if let Some(o) = t.as_object() {
            for (l, v) in o {
                if let Some(v) = v.as_str() {
                    titles.push((l.to_lowercase(), v.trim().to_string()));
                }
            }
        }
    }
    let pick = |f: &dyn Fn(&str) -> bool| titles.iter().find(|(l, _)| f(l)).map(|(_, t)| t.clone());
    pick(&|l| l == want)
        .or_else(|| pick(&|l| l.split('-').next() == Some(short.as_str())))
        .or_else(|| pick(&|l| l == "en"))
        .or_else(|| {
            a["title"]
                .as_object()?
                .values()
                .next()?
                .as_str()
                .map(str::to_string)
        })
        .unwrap_or_default()
}

fn volume(m: &Value, lang: &str) -> Volume {
    let a = &m["attributes"];
    let id = m["id"].as_str().unwrap_or_default().to_string();
    let rel = |kind: &str| -> Vec<&Value> {
        m["relationships"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| r["type"] == kind)
            .collect()
    };
    let cover = rel("cover_art")
        .first()
        .and_then(|r| r["attributes"]["fileName"].as_str())
        .map(|f| format!("{COVERS}/{id}/{f}.256.jpg"))
        .unwrap_or_default();
    let writer = rel("author")
        .iter()
        .filter_map(|r| r["attributes"]["name"].as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Volume {
        name: title(a, lang),
        start_year: a["year"].as_u64().and_then(|y| u16::try_from(y).ok()),
        count: a["lastVolume"]
            .as_str()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or_default(),
        cover,
        language: a["originalLanguage"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        format: a["publicationDemographic"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        writer,
        id,
        ..Default::default()
    }
}

impl Client {
    /// Manga with a title, the closest first.
    pub async fn search(&self, query: &str, lang: &str) -> Result<Vec<Volume>, String> {
        let key = format!("{lang}:{query}");
        if let Some(v) = self.cache.lock().await.get(&key) {
            return Ok(v.clone());
        }
        let mut last = self.last.lock().await;
        if let Some(t) = *last {
            tokio::time::sleep(Duration::from_millis(500).saturating_sub(t.elapsed())).await;
        }
        let res = self
            .http
            .get(format!("{}/manga", api_base()))
            .query(&[
                ("title", query),
                ("limit", "20"),
                ("order[relevance]", "desc"),
                ("includes[]", "cover_art"),
                ("includes[]", "author"),
                ("contentRating[]", "safe"),
                ("contentRating[]", "suggestive"),
                ("contentRating[]", "erotica"),
            ])
            .send()
            .await;
        *last = Some(Instant::now());
        drop(last);
        let res = res.map_err(net_error)?;
        match res.status().as_u16() {
            200 => {}
            429 => return Err("mdBusy".into()),
            code => return Err(format!("mdNetwork: HTTP {code}")),
        }
        let v: Value = res.json().await.map_err(net_error)?;
        // Fan works (doujinshi) are not the series.
        let fan = |m: &&Value| {
            m["attributes"]["tags"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|t| {
                    t["attributes"]["name"]["en"]
                        .as_str()
                        .is_some_and(|n| n.eq_ignore_ascii_case("doujinshi"))
                })
        };
        let found: Vec<Volume> = v["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| !fan(m))
            .map(|m| volume(m, lang))
            .collect();
        let ranked = renome_core::comics::rank_volumes(query, found, "");
        self.cache.lock().await.insert(key, ranked.clone());
        Ok(ranked)
    }
}
