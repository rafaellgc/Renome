//! Discogs client, with the user's own personal token: about one request
//! a second, as the service allows 60 a minute. Releases fetched are kept
//! for the session (the review, the covers and the tags ask again).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use renome_core::discogs::{self as dg, DgMaster, DgRelease, DgSearch};
use renome_core::musicbrainz::{self as mb, AlbumQuery, Candidate, Release};
use serde::Deserialize;

const API: &str = "https://api.discogs.com";

/// Tests point this at a local server with `RENOME_DISCOGS_API`.
fn api_base() -> String {
    std::env::var("RENOME_DISCOGS_API").unwrap_or_else(|_| API.into())
}

pub struct Client {
    http: reqwest::Client,
    last: tokio::sync::Mutex<Option<Instant>>,
    /// The personal token from the options (empty: Discogs is not used).
    token: std::sync::Mutex<String>,
    /// Releases as fetched, with their master's year, by number.
    releases: tokio::sync::Mutex<HashMap<String, (DgRelease, Option<u32>)>>,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!(
                    "Renome/",
                    env!("CARGO_PKG_VERSION"),
                    " +https://github.com/rafaellgc/Renome"
                ))
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            last: tokio::sync::Mutex::new(None),
            token: std::sync::Mutex::new(String::new()),
            releases: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_connect() {
        "dgOffline".into()
    } else if e.is_timeout() {
        "dgTimeout".into()
    } else {
        format!("dgNetwork: {}", e.without_url())
    }
}

impl Client {
    pub fn set_token(&self, token: &str) {
        *self.token.lock().unwrap() = token.trim().to_string();
    }

    /// Whether the user gave a token.
    pub fn ready(&self) -> bool {
        !self.token.lock().unwrap().is_empty()
    }

    /// A request at most every 1.1 seconds; when the service says it is
    /// asked too often, it is asked again after a pause, twice at most.
    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, &str)],
        token: Option<&str>,
    ) -> Result<T, String> {
        // A token's check asks every time.
        let keep = token.is_none();
        let question = format!("{path} {query:?}");
        if keep && let Some(v) = crate::kept("discogs", &question) {
            return Ok(v);
        }
        let token = match token {
            Some(t) => t.trim().to_string(),
            None => self.token.lock().unwrap().clone(),
        };
        if token.is_empty() {
            return Err("dgNoKey".into());
        }
        for attempt in 0..3 {
            let mut last = self.last.lock().await;
            if let Some(t) = *last {
                tokio::time::sleep(Duration::from_millis(1100).saturating_sub(t.elapsed())).await;
            }
            let res = self
                .http
                .get(format!("{}{path}", api_base()))
                .header(
                    reqwest::header::AUTHORIZATION,
                    format!("Discogs token={token}"),
                )
                .query(query)
                .send()
                .await;
            *last = Some(Instant::now());
            drop(last);
            let res = res.map_err(net_error)?;
            match res.status().as_u16() {
                200 if keep => {
                    let body = res.bytes().await.map_err(net_error)?;
                    return crate::keep("discogs", &question, &body)
                        .map_err(|e| format!("dgNetwork: {e}"));
                }
                200 => return res.json().await.map_err(net_error),
                401 | 403 => return Err("dgBadKey".into()),
                404 => return Err("dgNotFound".into()),
                429 if attempt < 2 => tokio::time::sleep(Duration::from_secs(20)).await,
                429 => return Err("dgBusy".into()),
                code => return Err(format!("dgNetwork: HTTP {code}")),
            }
        }
        Err("dgBusy".into())
    }

    /// Checks a token (from the options) with a tiny search.
    pub async fn check(&self, token: &str) -> Result<(), String> {
        self.get::<DgSearch>(
            "/database/search",
            &[("q", "test"), ("type", "release"), ("per_page", "1")],
            Some(token),
        )
        .await
        .map(|_| ())
    }

    /// Releases for an album, ranked as MusicBrainz's are; when nothing is
    /// found by artist and title apart, by both as free text.
    pub async fn search(&self, q: &AlbumQuery) -> Result<Vec<Candidate>, String> {
        let mut params = vec![
            ("type", "release".to_string()),
            ("release_title", q.album.clone()),
            ("per_page", "25".to_string()),
        ];
        if !q.artist.is_empty() && q.artist != renome_core::music::VARIOUS_ARTISTS {
            params.push(("artist", q.artist.clone()));
        }
        let ask = |params: Vec<(&'static str, String)>| async move {
            let query: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
            self.get::<DgSearch>("/database/search", &query, None).await
        };
        let mut found = ask(params).await?;
        if found.results.is_empty() {
            let text = format!("{} {}", q.artist, q.album);
            found = ask(vec![
                ("type", "release".to_string()),
                ("q", text.trim().to_string()),
                ("per_page", "25".to_string()),
            ])
            .await?;
        }
        let thumbs: HashMap<String, String> = found
            .results
            .iter()
            .map(|r| (format!("{}{}", dg::PREFIX, r.id), r.thumb.clone()))
            .collect();
        let mut out = mb::rank(q, &dg::found(&found));
        for c in &mut out {
            c.image = thumbs.get(&c.id).cloned().unwrap_or_default();
        }
        Ok(out)
    }

    /// A release with its master's year, fetched once.
    async fn fetch(&self, id: &str) -> Result<(DgRelease, Option<u32>), String> {
        let n = dg::number(id).ok_or("dgNotFound")?.to_string();
        if let Some(r) = self.releases.lock().await.get(&n) {
            return Ok(r.clone());
        }
        let r: DgRelease = self.get(&format!("/releases/{n}"), &[], None).await?;
        let year = match r.master_id {
            Some(m) if m > 0 => self
                .get::<DgMaster>(&format!("/masters/{m}"), &[], None)
                .await
                .ok()
                .and_then(|m| m.year),
            _ => None,
        };
        let year = year.or(r.year);
        self.releases.lock().await.insert(n, (r.clone(), year));
        Ok((r, year))
    }

    /// A release, read as MusicBrainz's (with tracks).
    pub async fn release(&self, id: &str) -> Result<Release, String> {
        let (r, year) = self.fetch(id).await?;
        Ok(dg::to_release(&r, year))
    }

    /// The release's front cover, as Discogs has it (about 600 pixels).
    pub async fn front_cover(&self, id: &str) -> Result<Option<Vec<u8>>, String> {
        let (r, _) = self.fetch(id).await?;
        let Some(url) = r.cover() else {
            return Ok(None);
        };
        let res = self.http.get(url).send().await.map_err(net_error)?;
        if !res.status().is_success() {
            return Ok(None);
        }
        let bytes = res.bytes().await.map_err(net_error)?;
        Ok(Some(bytes.to_vec()))
    }
}
