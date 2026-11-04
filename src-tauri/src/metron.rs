//! Metron client: series and their issues, with the user's own token, and
//! issues by the barcode printed on them (UPC or ISBN). The service allows
//! about 20 requests a minute and a few thousand a day for each account,
//! so requests are paced and answers kept.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use renome_core::comics::{Issue, Volume};
use serde::Serialize;
use serde_json::Value;

const API: &str = "https://metron.cloud/api";

/// Tests point this at a local server with `RENOME_METRON_API`.
fn api_base() -> String {
    std::env::var("RENOME_METRON_API").unwrap_or_else(|_| API.into())
}

pub struct Client {
    http: reqwest::Client,
    last: tokio::sync::Mutex<Option<Instant>>,
    /// Answers by request (without the token), for the session.
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
        "mtOffline".into()
    } else if e.is_timeout() {
        "mtTimeout".into()
    } else {
        format!("mtNetwork: {}", e.without_url())
    }
}

/// A series' name without the year Metron shows after it
/// (`Name (2016)`), when that is its first year.
fn bare_name(name: &str, year: Option<u16>) -> String {
    let name = name.trim();
    if let Some(y) = year
        && let Some(rest) = name.strip_suffix(&format!("({y})"))
    {
        return rest.trim_end().to_string();
    }
    name.to_string()
}

fn volume(v: &Value) -> Volume {
    let s = |v: &Value| v.as_str().unwrap_or_default().trim().to_string();
    let start_year = v["year_began"].as_u64().and_then(|y| u16::try_from(y).ok());
    // The list says `series`, a series' own page `name`.
    let name = if v["name"].is_string() {
        s(&v["name"])
    } else {
        s(&v["series"])
    };
    Volume {
        id: v["id"].as_u64().unwrap_or_default().to_string(),
        name: bare_name(&name, start_year),
        start_year,
        publisher: s(&v["publisher"]["name"]),
        count: v["issue_count"]
            .as_u64()
            .and_then(|c| u32::try_from(c).ok())
            .unwrap_or_default(),
        language: s(&v["language"]),
        format: s(&v["series_type"]["name"]).to_lowercase(),
        ..Default::default()
    }
}

fn issue(v: &Value) -> Issue {
    let s = |v: &Value| v.as_str().unwrap_or_default().trim().to_string();
    Issue {
        id: v["id"].as_u64().unwrap_or_default().to_string(),
        number: s(&v["number"]),
        // The list says `issue` (series and number), a page `name`s.
        name: v["name"]
            .as_array()
            .and_then(|n| n.first())
            .map(s)
            .unwrap_or_default(),
        cover_date: s(&v["cover_date"]),
        cover: s(&v["image"]),
    }
}

/// An issue found by its barcode, with its series and the series' issues.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Coded {
    pub volume: Volume,
    pub issues: Vec<Issue>,
    /// The issue's id among them.
    pub issue: String,
}

impl Client {
    /// A request at most every three seconds; a busy service is asked
    /// again a little later, once.
    async fn get(&self, token: &str, path: &str, query: &[(&str, &str)]) -> Result<Value, String> {
        let id = format!("{path}?{query:?}");
        if let Some(v) = self.cache.lock().await.get(&id) {
            return Ok(v.clone());
        }
        // The token's check asks every time.
        let keep = path != "series_type/";
        if keep && let Some(v) = crate::kept::<Value>("metron", &id) {
            self.cache.lock().await.insert(id, v.clone());
            return Ok(v);
        }
        let url = format!("{}/{path}", api_base());
        for attempt in 0..2 {
            let mut last = self.last.lock().await;
            if let Some(t) = *last {
                tokio::time::sleep(Duration::from_millis(3000).saturating_sub(t.elapsed())).await;
            }
            let res = self
                .http
                .get(&url)
                .bearer_auth(token)
                .query(query)
                .send()
                .await;
            *last = Some(Instant::now());
            drop(last);
            let res = res.map_err(net_error)?;
            match res.status().as_u16() {
                200 => {}
                401 | 403 => return Err("mtBadKey".into()),
                404 => return Err("mtNotFound".into()),
                429 => return Err("mtBusy".into()),
                500..=599 if attempt == 0 => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                code => return Err(format!("mtNetwork: HTTP {code}")),
            }
            let body = res.bytes().await.map_err(net_error)?;
            let v: Value = serde_json::from_slice(&body).map_err(|e| format!("mtNetwork: {e}"))?;
            if keep {
                crate::ANSWERS.put("metron", &id, &body);
            }
            self.cache.lock().await.insert(id, v.clone());
            return Ok(v);
        }
        Err("mtBusy".into())
    }

    /// Whether the token works.
    pub async fn check(&self, token: &str) -> Result<(), String> {
        self.get(token, "series_type/", &[]).await.map(|_| ())
    }

    /// Series with a name, from `year` and by `publisher` when given, best
    /// first.
    pub async fn search(
        &self,
        token: &str,
        query: &str,
        year: Option<u16>,
        publisher: &str,
        lang: &str,
    ) -> Result<Vec<Volume>, String> {
        let year_s = year.map(|y| y.to_string());
        let mut params = vec![("name", query)];
        if let Some(y) = &year_s {
            params.push(("year_began", y));
        }
        let page = self.get(token, "series/", &params).await?;
        let found: Vec<Volume> = page["results"]
            .as_array()
            .into_iter()
            .flatten()
            .map(volume)
            .collect();
        // The list may leave the publisher out: kept when unknown.
        let typed = publisher.trim();
        let found = if typed.is_empty() || found.iter().all(|v| v.publisher.is_empty()) {
            renome_core::comics::filter_volumes(found, year, "")
        } else {
            renome_core::comics::filter_volumes(found, year, typed)
        };
        Ok(renome_core::comics::rank_volumes(query, found, lang))
    }

    /// A series and all its issues, in order.
    pub async fn issues(&self, token: &str, id: &str) -> Result<(Volume, Vec<Issue>), String> {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err("mtNotFound".into());
        }
        let v = self.get(token, &format!("series/{id}/"), &[]).await?;
        let vol = volume(&v);
        let mut issues = Vec::new();
        let mut page = 1u32;
        loop {
            let n = page.to_string();
            let got = self
                .get(token, &format!("series/{id}/issue_list/"), &[("page", &n)])
                .await?;
            issues.extend(got["results"].as_array().into_iter().flatten().map(issue));
            if got["next"].is_null() || page >= 50 {
                break;
            }
            page += 1;
        }
        Ok((vol, issues))
    }

    /// The issue a barcode belongs to: a UPC (12 digits, with its 5-digit
    /// add-on when printed) or an ISBN (10 or 13 digits). `None` when
    /// Metron does not know it.
    pub async fn by_code(&self, token: &str, code: &str) -> Result<Option<Coded>, String> {
        let mut digits: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        // A UPC read as an EAN-13 has a 0 before it.
        if digits.len() >= 13 && digits.starts_with('0') {
            digits.remove(0);
        }
        let field = match digits.len() {
            10 => "isbn",
            13 if digits.starts_with("978") || digits.starts_with("979") => "isbn",
            12..=17 => "upc",
            _ => return Ok(None),
        };
        let page = self
            .get(token, "issue/", &[(field, digits.as_str())])
            .await?;
        let mut found = page["results"].as_array().into_iter().flatten();
        let (Some(first), None) = (found.next(), found.next()) else {
            return Ok(None);
        };
        let series = first["series"]["id"]
            .as_u64()
            .unwrap_or_default()
            .to_string();
        let (volume, mut issues) = self.issues(token, &series).await?;
        let id = first["id"].as_u64().unwrap_or_default().to_string();
        if !issues.iter().any(|i| i.id == id) {
            issues.push(issue(first));
        }
        Ok(Some(Coded {
            volume,
            issues,
            issue: id,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_names_lose_their_year() {
        let v: Value = serde_json::from_str(
            r#"{"id":5,"series":"Estrela (2016)","year_began":2016,"issue_count":40,
            "publisher":{"id":1,"name":"Editora Um"},"series_type":{"id":2,"name":"Ongoing Series"}}"#,
        )
        .unwrap();
        let v = volume(&v);
        assert_eq!(v.name, "Estrela");
        assert_eq!(
            (v.start_year, v.count, v.publisher.as_str()),
            (Some(2016), 40, "Editora Um")
        );
        assert_eq!(bare_name("Estrela (2016)", Some(2017)), "Estrela (2016)");
        let i: Value = serde_json::from_str(
            r#"{"id":9,"number":"3","issue":"Estrela (2016) #3","cover_date":"2016-05-01",
            "image":"https://x/3.jpg"}"#,
        )
        .unwrap();
        let i = issue(&i);
        assert_eq!(
            (i.id.as_str(), i.number.as_str(), i.name.as_str()),
            ("9", "3", "")
        );
    }
}
