//! TMDB client: movie search, details and images. The key is either a v3
//! API key (sent as `api_key`) or a v4 read access token (sent as a bearer
//! token); TMDB gives both on the same settings page.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use futures_util::{StreamExt, stream};
use renome_core::tmdb::{self, Candidate, Details, Guess, SearchResult};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

const API: &str = "https://api.themoviedb.org/3";

/// The API and image addresses; tests point them at a local server with
/// `RENOME_TMDB_API` and `RENOME_TMDB_IMAGES`.
fn api_base() -> String {
    std::env::var("RENOME_TMDB_API").unwrap_or_else(|_| API.into())
}

fn image_base() -> String {
    std::env::var("RENOME_TMDB_IMAGES").unwrap_or_else(|_| tmdb::IMAGE_BASE.into())
}

/// Cheap to clone: the HTTP client is shared.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    key: String,
    lang: String,
}

#[derive(Deserialize)]
struct Page<T> {
    results: Vec<T>,
}

impl Client {
    pub fn new(key: &str, lang: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!("Renome/", env!("CARGO_PKG_VERSION")))
                .build()
                .unwrap_or_default(),
            key: key.trim().to_string(),
            lang: lang.to_string(),
        }
    }

    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, String> {
        let mut req = self
            .http
            .get(format!("{}{path}", api_base()))
            .query(query)
            .query(&[("language", &self.lang)]);
        // v4 tokens are JWTs; v3 keys are short hex strings.
        req = if self.key.starts_with("eyJ") {
            req.bearer_auth(&self.key)
        } else {
            req.query(&[("api_key", &self.key)])
        };
        let res = req.send().await.map_err(|e| format!("tmdbNetwork: {e}"))?;
        match res.status().as_u16() {
            200 => res.json().await.map_err(|e| format!("tmdbNetwork: {e}")),
            401 => Err("tmdbInvalidKey".into()),
            404 => Err("tmdbNotFound".into()),
            429 => Err("tmdbBusy".into()),
            code => Err(format!("tmdbNetwork: HTTP {code}")),
        }
    }

    /// Checks the key with a cheap request.
    pub async fn check(&self) -> Result<(), String> {
        self.get::<serde_json::Value>("/configuration", &[])
            .await
            .map(|_| ())
    }

    /// Searches by title; with a year, also within that year, since a plain
    /// search may bury an older or less popular movie with the same name.
    pub async fn search(&self, guess: &Guess) -> Result<(Vec<Candidate>, bool), String> {
        // The name already says which movie it is.
        if let Some(id) = guess.id {
            let d = self.details(id).await?;
            let year = d
                .release_date
                .as_deref()
                .and_then(|s| s.get(..4)?.parse().ok());
            let c = Candidate {
                id: d.id,
                title: d.title,
                original_title: d.original_title,
                year,
                overview: d.overview.unwrap_or_default(),
                poster_path: d.poster_path,
                score: 1.0,
            };
            return Ok((vec![c], true));
        }
        let base = [("query", guess.title.as_str()), ("include_adult", "false")];
        let mut results: Vec<SearchResult> = Vec::new();
        if let Some(y) = guess.year {
            let y = y.to_string();
            let mut q = base.to_vec();
            q.push(("year", &y));
            results.extend(
                self.get::<Page<SearchResult>>("/search/movie", &q)
                    .await?
                    .results,
            );
        }
        results.extend(
            self.get::<Page<SearchResult>>("/search/movie", &base)
                .await?
                .results,
        );
        Ok(tmdb::rank(guess, results))
    }

    pub async fn details(&self, id: u64) -> Result<Details, String> {
        self.get(
            &format!("/movie/{id}"),
            &[
                (
                    "append_to_response",
                    "credits,release_dates,keywords,videos,images",
                ),
                ("include_video_language", &format!("{},en", self.lang2())),
                (
                    "include_image_language",
                    &format!("{},en,null", self.lang2()),
                ),
            ],
        )
        .await
    }

    async fn image(&self, path: &str) -> Result<Vec<u8>, String> {
        let res = self
            .http
            .get(format!("{}{path}", image_base()))
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("tmdbNetwork: {e}"))?;
        Ok(res
            .bytes()
            .await
            .map_err(|e| format!("tmdbNetwork: {e}"))?
            .to_vec())
    }

    /// `BR` from `pt-BR`, for the age rating.
    /// `pt` from `pt-BR`.
    fn lang2(&self) -> &str {
        self.lang.split('-').next().unwrap_or("en")
    }
}

#[derive(Deserialize)]
pub struct LookupItem {
    path: PathBuf,
    /// File name without extension.
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lookup {
    path: PathBuf,
    guess: Guess,
    candidates: Vec<Candidate>,
    /// The first candidate is a safe automatic choice.
    auto: bool,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
}

/// Looks up many files, a few at a time. Files with the same guessed title
/// and year (a movie and its subtitles) share one search.
pub async fn lookup(app: &AppHandle, client: &Client, items: Vec<LookupItem>) -> Vec<Lookup> {
    let guesses: Vec<(PathBuf, Guess)> = items
        .into_iter()
        .map(|i| (i.path, tmdb::guess(&i.name)))
        .collect();
    let mut unique: Vec<Guess> = Vec::new();
    for (_, g) in &guesses {
        if !g.title.is_empty() && !unique.contains(g) {
            unique.push(g.clone());
        }
    }
    let total = unique.len();
    let _ = app.emit("tmdb-progress", Progress { done: 0, total });
    let mut done = 0;
    let mut found: HashMap<usize, Result<(Vec<Candidate>, bool), String>> = HashMap::new();
    let mut tasks = stream::iter(unique.clone().into_iter().enumerate())
        .map(|(i, g)| {
            let client = client.clone();
            async move { (i, client.search(&g).await) }
        })
        .buffer_unordered(6);
    while let Some((i, r)) = tasks.next().await {
        found.insert(i, r);
        done += 1;
        let _ = app.emit("tmdb-progress", Progress { done, total });
    }
    guesses
        .into_iter()
        .map(|(path, guess)| {
            let hit = unique
                .iter()
                .position(|g| *g == guess)
                .and_then(|i| found.get(&i));
            let (candidates, auto, error) = match hit {
                Some(Ok((c, auto))) => (c.clone(), *auto, None),
                Some(Err(e)) => (Vec::new(), false, Some(e.clone())),
                None => (Vec::new(), false, None),
            };
            Lookup {
                path,
                guess,
                candidates,
                auto,
                error,
            }
        })
        .collect()
}

#[derive(Deserialize)]
pub struct SidecarItem {
    /// The renamed video.
    path: PathBuf,
    id: u64,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SidecarReport {
    pub created: Vec<PathBuf>,
    /// File name and reason.
    pub errors: Vec<(String, String)>,
}

fn sibling(video: &Path, suffix: &str) -> PathBuf {
    let stem = video.file_stem().unwrap_or_default().to_string_lossy();
    video.with_file_name(format!("{stem}{suffix}"))
}

/// Writes `.nfo`, poster and (optionally) backdrop next to each video.
/// Existing files are never overwritten.
pub async fn sidecars(
    app: &AppHandle,
    client: &Client,
    items: Vec<SidecarItem>,
    fanart: bool,
    in_folder: bool,
) -> SidecarReport {
    let total = items.len();
    let mut report = SidecarReport::default();
    let _ = app.emit("tmdb-progress", Progress { done: 0, total });
    let mut tasks = stream::iter(items)
        .map(|item| {
            let client = client.clone();
            async move {
                let mut created = Vec::new();
                let mut errors: Vec<(String, String)> = Vec::new();
                let name = |p: &Path| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                };
                match client.details(item.id).await {
                    Err(e) => errors.push((name(&item.path), e)),
                    Ok(d) => {
                        // Files already there (an earlier run) are kept and
                        // not downloaded again.
                        let mut files: Vec<(PathBuf, Option<&str>)> =
                            vec![(sibling(&item.path, ".nfo"), None)];
                        // In the movie's own folder, Jellyfin's plain names
                        // (poster.jpg); next to other movies, prefixed ones.
                        let image = |kind: &str, ext: &str| {
                            if in_folder {
                                item.path.with_file_name(format!("{kind}.{ext}"))
                            } else {
                                sibling(&item.path, &format!("-{kind}.{ext}"))
                            }
                        };
                        if let Some(p) = &d.poster_path {
                            files.push((image("poster", "jpg"), Some(p)));
                        }
                        if fanart {
                            if let Some(p) = &d.backdrop_path {
                                files.push((image("fanart", "jpg"), Some(p)));
                            }
                            if let Some(p) = d.logo(client.lang2()) {
                                files.push((image("logo", "png"), Some(p)));
                            }
                        }
                        for (path, source) in files {
                            if std::fs::symlink_metadata(&path).is_ok() {
                                continue;
                            }
                            let data = match source {
                                None => Ok(tmdb::nfo(&d, &client.lang).into_bytes()),
                                Some(p) => client.image(p).await,
                            };
                            match data.and_then(|bytes| write_new(&path, &bytes)) {
                                Ok(()) => created.push(path),
                                Err(e) => errors.push((name(&path), e)),
                            }
                        }
                    }
                }
                (created, errors)
            }
        })
        .buffer_unordered(4);
    let mut done = 0;
    while let Some((created, errors)) = tasks.next().await {
        report.created.extend(created);
        report.errors.extend(errors);
        done += 1;
        let _ = app.emit("tmdb-progress", Progress { done, total });
    }
    report
}

/// Creates the file only if it does not exist yet.
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => "alreadyExists".to_string(),
            _ => e.to_string(),
        })?;
    f.write_all(bytes).map_err(|e| e.to_string())
}
