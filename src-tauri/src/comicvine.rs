//! Comic Vine client: series ("volumes") and their issues, with the user's
//! own key. The service allows about 200 requests an hour for each kind of
//! data and blocks quick bursts, so requests are paced and answers kept
//! for the session.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use renome_core::comics::{Issue, Volume};
use serde_json::Value;

const API: &str = "https://comicvine.gamespot.com/api";
/// Issues per page, the most the service gives.
const PAGE: usize = 100;

/// Tests point this at a local server with `RENOME_CV_API`.
fn api_base() -> String {
    std::env::var("RENOME_CV_API").unwrap_or_else(|_| API.into())
}

pub struct Client {
    http: reqwest::Client,
    last: tokio::sync::Mutex<Option<Instant>>,
    /// Answers by request (without the key), for the session.
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
            last: tokio::sync::Mutex::new(None),
            cache: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_connect() {
        "cvOffline".into()
    } else if e.is_timeout() {
        "cvTimeout".into()
    } else {
        format!("cvNetwork: {}", e.without_url())
    }
}

/// A Comic Vine image, unless it is the blank one put where there is none.
fn image(v: &Value, size: &str) -> String {
    let url = v["image"][size].as_str().unwrap_or_default();
    match url.contains("-blank.") {
        true => String::new(),
        false => url.to_string(),
    }
}

fn volume(v: &Value) -> Volume {
    Volume {
        id: v["id"].as_u64().unwrap_or_default().to_string(),
        name: v["name"].as_str().unwrap_or_default().trim().to_string(),
        start_year: v["start_year"]
            .as_str()
            .and_then(|y| y.trim().parse().ok())
            .or_else(|| v["start_year"].as_u64().and_then(|y| u16::try_from(y).ok())),
        publisher: v["publisher"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        count: v["count_of_issues"]
            .as_u64()
            .and_then(|c| u32::try_from(c).ok())
            .unwrap_or_default(),
        cover: image(v, "small_url"),
        ..Default::default()
    }
}

fn issue(v: &Value) -> Issue {
    Issue {
        id: v["id"].as_u64().unwrap_or_default().to_string(),
        number: v["issue_number"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string(),
        name: v["name"].as_str().unwrap_or_default().trim().to_string(),
        cover_date: v["cover_date"].as_str().unwrap_or_default().to_string(),
        cover: image(v, "small_url"),
    }
}

const VOLUME_FIELDS: &str = "id,name,start_year,publisher,count_of_issues,image";

impl Client {
    /// A request at most every 1.1 seconds; a busy service is asked again
    /// a little later, once.
    async fn get(&self, key: &str, path: &str, query: &[(&str, &str)]) -> Result<Value, String> {
        let id = format!("{path}?{query:?}");
        if let Some(v) = self.cache.lock().await.get(&id) {
            return Ok(v.clone());
        }
        let url = format!("{}/{path}/", api_base());
        let mut params = vec![("api_key", key), ("format", "json")];
        params.extend_from_slice(query);
        for attempt in 0..2 {
            let mut last = self.last.lock().await;
            if let Some(t) = *last {
                tokio::time::sleep(Duration::from_millis(1100).saturating_sub(t.elapsed())).await;
            }
            let res = self.http.get(&url).query(&params).send().await;
            *last = Some(Instant::now());
            drop(last);
            let res = res.map_err(net_error)?;
            let status = res.status().as_u16();
            match status {
                200 => {}
                401 => return Err("cvBadKey".into()),
                404 => return Err("cvNotFound".into()),
                420 | 429 => return Err("cvBusy".into()),
                500..=599 if attempt == 0 => {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
                code => return Err(format!("cvNetwork: HTTP {code}")),
            }
            let v: Value = res.json().await.map_err(net_error)?;
            // The service's own codes: 1 is fine.
            match v["status_code"].as_u64() {
                Some(1) => {}
                Some(100) => return Err("cvBadKey".into()),
                Some(101) => return Err("cvNotFound".into()),
                Some(107) => return Err("cvBusy".into()),
                _ => return Err(format!("cvNetwork: {}", v["error"].as_str().unwrap_or("?"))),
            }
            self.cache.lock().await.insert(id, v.clone());
            return Ok(v);
        }
        Err("cvBusy".into())
    }

    /// Whether the key works.
    pub async fn check(&self, key: &str) -> Result<(), String> {
        self.get(key, "types", &[]).await.map(|_| ())
    }

    /// Series with a name, from `year` and by `publisher` when given,
    /// best first: the search finds them (more of them to pick from when
    /// filtered), a second request gives their publishers and issue
    /// counts (the search's are empty).
    pub async fn search(
        &self,
        key: &str,
        query: &str,
        year: Option<u16>,
        publisher: &str,
        lang: &str,
    ) -> Result<Vec<Volume>, String> {
        let filtered = year.is_some() || !publisher.trim().is_empty();
        let found = self
            .get(
                key,
                "search",
                &[
                    ("resources", "volume"),
                    ("query", query),
                    ("limit", if filtered { "50" } else { "20" }),
                    ("field_list", "id"),
                ],
            )
            .await?;
        let ids: Vec<String> = found["results"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r["id"].as_u64())
            .map(|id| id.to_string())
            .collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let filter = format!("id:{}", ids.join("|"));
        let full = self
            .get(
                key,
                "volumes",
                &[
                    ("filter", &filter),
                    ("field_list", VOLUME_FIELDS),
                    ("limit", "100"),
                ],
            )
            .await?;
        let by_id: HashMap<String, Volume> = full["results"]
            .as_array()
            .into_iter()
            .flatten()
            .map(volume)
            .map(|v| (v.id.clone(), v))
            .collect();
        let found = ids.iter().filter_map(|id| by_id.get(id).cloned()).collect();
        let found = renome_core::comics::filter_volumes(found, year, publisher);
        Ok(renome_core::comics::rank_volumes(query, found, lang))
    }

    /// A series and all its issues, in order.
    pub async fn issues(&self, key: &str, id: &str) -> Result<(Volume, Vec<Issue>), String> {
        let v = self
            .get(
                key,
                &format!("volume/4050-{id}"),
                &[("field_list", VOLUME_FIELDS)],
            )
            .await?;
        let vol = volume(&v["results"]);
        let filter = format!("volume:{id}");
        let mut issues = Vec::new();
        loop {
            let offset = issues.len().to_string();
            let page = self
                .get(
                    key,
                    "issues",
                    &[
                        ("filter", &filter),
                        ("field_list", "id,issue_number,name,cover_date,image"),
                        ("sort", "cover_date:asc"),
                        ("limit", "100"),
                        ("offset", &offset),
                    ],
                )
                .await?;
            let got: Vec<Issue> = page["results"]
                .as_array()
                .into_iter()
                .flatten()
                .map(issue)
                .collect();
            let total = page["number_of_total_results"].as_u64().unwrap_or_default() as usize;
            let short = got.len() < PAGE;
            issues.extend(got);
            if short || issues.len() >= total {
                break;
            }
        }
        Ok((vol, issues))
    }

    /// The series an issue belongs to (from a `ComicInfo.xml` link).
    pub async fn volume_of(&self, key: &str, issue: &str) -> Result<String, String> {
        let v = self
            .get(
                key,
                &format!("issue/4000-{issue}"),
                &[("field_list", "volume")],
            )
            .await?;
        v["results"]["volume"]["id"]
            .as_u64()
            .map(|id| id.to_string())
            .ok_or_else(|| "cvNotFound".into())
    }
}
