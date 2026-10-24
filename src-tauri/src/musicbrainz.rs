//! MusicBrainz client: at most one request per second, as the service
//! asks, with the program identified; and the album lookups built on it.
//! Covers come from the Cover Art Archive.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use renome_core::music::{self, AudioTags};
use renome_core::musicbrainz::{
    self as mb, AlbumQuery, Candidate, Release, Search, Song, SongMatch,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

const API: &str = "https://musicbrainz.org/ws/2";
const COVERS: &str = "https://coverartarchive.org";

/// Tests point these at a local server with `RENOME_MB_API` and
/// `RENOME_MB_COVERS`.
fn api_base() -> String {
    std::env::var("RENOME_MB_API").unwrap_or_else(|_| API.into())
}

fn covers_base() -> String {
    std::env::var("RENOME_MB_COVERS").unwrap_or_else(|_| COVERS.into())
}

/// Shared by all lookups, so the pace holds across them.
pub struct Client {
    http: reqwest::Client,
    last: tokio::sync::Mutex<Option<Instant>>,
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_connect() {
        "mbOffline".into()
    } else if e.is_timeout() {
        "mbTimeout".into()
    } else {
        format!("mbNetwork: {}", e.without_url())
    }
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
        }
    }
}

impl Client {
    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, String> {
        for attempt in 0..4 {
            // One request every 2 seconds, well within the service's limit (one
            // per second on average); the lock keeps others waiting.
            let mut last = self.last.lock().await;
            if let Some(t) = *last {
                let wait = Duration::from_secs(2).saturating_sub(t.elapsed());
                tokio::time::sleep(wait).await;
            }
            let res = self
                .http
                .get(format!("{}{path}", api_base()))
                .query(query)
                .query(&[("fmt", "json")])
                .send()
                .await;
            *last = Some(Instant::now());
            drop(last);
            let res = res.map_err(net_error)?;
            match res.status().as_u16() {
                200 => return res.json().await.map_err(net_error),
                404 => return Err("mbNotFound".into()),
                // Busy (it sheds load now and then, whatever the pace) or
                // too fast: wait as long as it asks, at least 3 seconds.
                503 if attempt < 3 => {
                    let asked = res
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok()?.trim().parse::<u64>().ok())
                        .unwrap_or(0);
                    tokio::time::sleep(Duration::from_secs(asked.clamp(3, 60))).await
                }
                503 => return Err("mbBusy".into()),
                code => return Err(format!("mbNetwork: HTTP {code}")),
            }
        }
        Err("mbBusy".into())
    }

    pub async fn search(&self, q: &AlbumQuery) -> Result<Vec<Candidate>, String> {
        let s: Search = self
            .get("/release/", &[("query", &mb::query(q)), ("limit", "25")])
            .await?;
        Ok(mb::rank(q, &s.releases))
    }

    /// A release with its tracks.
    pub async fn release(&self, id: &str) -> Result<Release, String> {
        self.get(
            &format!("/release/{id}"),
            &[(
                "inc",
                "recordings artist-credits release-groups labels media",
            )],
        )
        .await
    }

    /// The front cover (1200 pixels), or `None` if the release has none.
    async fn cover(&self, id: &str) -> Result<Option<Vec<u8>>, String> {
        let res = self
            .http
            .get(format!("{}/release/{id}/front-1200", covers_base()))
            .send()
            .await
            .map_err(net_error)?;
        match res.status().as_u16() {
            200 => Ok(Some(res.bytes().await.map_err(net_error)?.to_vec())),
            404 => Ok(None),
            code => Err(format!("mbNetwork: HTTP {code}")),
        }
    }
}

/// An album of the list and what was found for it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumLookup {
    files: Vec<PathBuf>,
    /// The folder the album's songs are in (shown in the review).
    dir: PathBuf,
    query: AlbumQuery,
    candidates: Vec<Candidate>,
    /// Picked with confidence.
    choice: Option<Candidate>,
    matches: Vec<(PathBuf, SongMatch)>,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
}

/// The songs among `files` grouped in albums, with album-wide details agreed.
pub fn albums(files: &[(PathBuf, AudioTags)]) -> Vec<Vec<Song>> {
    let mut songs: Vec<(PathBuf, AudioTags)> = files
        .iter()
        .map(|(p, t)| {
            (
                p.parent().map(Path::to_path_buf).unwrap_or_default(),
                t.clone(),
            )
        })
        .collect();
    music::fill_albums(&mut songs);
    music::album_groups(&songs)
        .into_iter()
        .map(|g| {
            g.into_iter()
                .map(|i| (files[i].0.clone(), songs[i].1.clone()))
                .collect()
        })
        .collect()
}

fn query_of(songs: &[Song]) -> AlbumQuery {
    let t = &songs[0].1;
    AlbumQuery {
        artist: t.album_artist().to_string(),
        album: t.album.clone(),
        year: t.year,
        tracks: songs.len() as u32,
    }
}

/// Looks up each album: by the MusicBrainz release in the tags, or by
/// searching; the best release found is checked against the songs (track
/// count and durations) and picked only if it fits without doubt.
pub async fn lookup(app: &AppHandle, client: &Client, albums: Vec<Vec<Song>>) -> Vec<AlbumLookup> {
    let total = albums.len();
    let _ = app.emit("mb-progress", Progress { done: 0, total });
    let mut out = Vec::new();
    for (done, songs) in albums.into_iter().enumerate() {
        let query = query_of(&songs);
        let mut found = AlbumLookup {
            files: songs.iter().map(|(p, _)| p.clone()).collect(),
            dir: songs[0]
                .0
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            query: query.clone(),
            candidates: Vec::new(),
            choice: None,
            matches: Vec::new(),
            error: None,
        };
        let tagged = &songs[0].1.mb_release;
        let same_release = !tagged.is_empty() && songs.iter().all(|(_, t)| t.mb_release == *tagged);
        let result = async {
            let release = if same_release {
                // Tagged with its release already: no search needed.
                let r = client.release(tagged).await?;
                found.candidates = mb::rank(&query, std::slice::from_ref(&r));
                Some(r)
            } else {
                found.candidates = client.search(&query).await?;
                match found.candidates.first() {
                    Some(best) => Some(client.release(&best.id).await?),
                    None => None,
                }
            };
            // The release in the tags is taken when it has every song (bonus
            // tracks from elsewhere call for a review).
            if let Some(r) = release {
                let matches = mb::match_tracks(&r, &songs).0;
                if (same_release && matches.len() == songs.len()) || mb::sure(&r, &songs) {
                    found.choice = found.candidates.iter().find(|c| c.id == r.id).cloned();
                    found.matches = matches;
                }
            }
            Ok::<(), String>(())
        }
        .await;
        found.error = result.err();
        found.candidates.truncate(12);
        out.push(found);
        let _ = app.emit(
            "mb-progress",
            Progress {
                done: done + 1,
                total,
            },
        );
    }
    out
}

/// The release the user chose for an album, matched to its songs.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chosen {
    matches: Vec<(PathBuf, SongMatch)>,
    /// How many songs agree in duration with their track.
    agree: usize,
}

pub async fn choose(client: &Client, id: &str, songs: &[Song]) -> Result<Chosen, String> {
    let r = client.release(id).await?;
    let (matches, agree) = mb::match_tracks(&r, songs);
    Ok(Chosen { matches, agree })
}

#[derive(Deserialize)]
pub struct CoverItem {
    /// The album's folder after renaming.
    dir: PathBuf,
    release: String,
}

/// Saves each album's front cover as `cover.jpg` in its folder, unless the
/// folder already has a cover (`cover`, `folder` or `front` image).
pub async fn covers(
    app: &AppHandle,
    client: &Client,
    items: Vec<CoverItem>,
) -> crate::tmdb::SidecarReport {
    let mut report = crate::tmdb::SidecarReport::default();
    let total = items.len();
    let _ = app.emit("mb-progress", Progress { done: 0, total });
    for (done, item) in items.into_iter().enumerate() {
        let has_cover = std::fs::read_dir(&item.dir)
            .map(|rd| {
                rd.flatten().any(|e| {
                    let n = e.file_name().to_string_lossy().to_lowercase();
                    ["cover.", "folder.", "front."]
                        .iter()
                        .any(|p| n.starts_with(p))
                })
            })
            .unwrap_or(true);
        if !has_cover {
            let path = item.dir.join("cover.jpg");
            match client.cover(&item.release).await {
                Ok(Some(bytes)) => match crate::tmdb::write_new(&path, &bytes) {
                    Ok(()) => report.created.push(path),
                    Err(e) => report.errors.push(("cover.jpg".into(), e)),
                },
                Ok(None) => {}
                Err(e) => report.errors.push((item.dir.display().to_string(), e)),
            }
        }
        let _ = app.emit(
            "mb-progress",
            Progress {
                done: done + 1,
                total,
            },
        );
    }
    report
}
