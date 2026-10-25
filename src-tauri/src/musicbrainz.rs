//! MusicBrainz client: at most one request per second, as the service
//! asks, with the program identified; and the album lookups built on it.
//! Covers come from the Cover Art Archive.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use renome_core::acoustid::Heard;
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

    /// The front cover (1200 pixels): the release's, else its release
    /// group's (an edition without artwork, a cassette say, shares the
    /// album's); `None` if neither has one.
    async fn cover(&self, id: &str) -> Result<Option<Vec<u8>>, String> {
        if let Some(bytes) = self.front(&format!("release/{id}")).await? {
            return Ok(Some(bytes));
        }
        let group = self
            .release(id)
            .await
            .ok()
            .and_then(|r| r.release_group)
            .map(|g| g.id)
            .filter(|g| !g.is_empty());
        match group {
            Some(g) => self.front(&format!("release-group/{g}")).await,
            None => Ok(None),
        }
    }

    /// The front image of `what` (`release/<id>`, `release-group/<id>`).
    /// The archive fails now and then (HTTP 5xx): it is asked once more.
    async fn front(&self, what: &str) -> Result<Option<Vec<u8>>, String> {
        let url = format!("{}/{what}/front-1200", covers_base());
        let mut retried = false;
        loop {
            let res = self.http.get(&url).send().await.map_err(net_error)?;
            match res.status().as_u16() {
                200 => return Ok(Some(res.bytes().await.map_err(net_error)?.to_vec())),
                404 => return Ok(None),
                500..=599 if !retried => {
                    retried = true;
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
                code => return Err(format!("coverNetwork: HTTP {code}")),
            }
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
    /// Identified by the songs' sound (AcoustID), not their tags.
    heard: bool,
}

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
    /// `listen` while songs are fingerprinted and heard on AcoustID.
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<&'static str>,
}

fn progress(app: &AppHandle, done: usize, total: usize, kind: Option<&'static str>) {
    let _ = app.emit("mb-progress", Progress { done, total, kind });
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

/// What a lookup works with: MusicBrainz, AcoustID and the recordings each
/// song was heard as (kept for the review's choices).
pub struct Services<'a> {
    pub mb: &'a Client,
    pub acoustid: &'a crate::acoustid::Client,
    pub heard: &'a Mutex<HashMap<PathBuf, Vec<String>>>,
}

/// Looks up each album: by the MusicBrainz release in the tags, or by
/// searching; the best release found is checked against the songs (track
/// count and durations) and picked only if it fits without doubt. With
/// `listen`, the songs without a title in their tags (`unnamed`) and those
/// of albums without a release picked are identified by their sound
/// (AcoustID); for such an album, what was heard counts when it picks a
/// release for some of its songs, else the search's candidates stay.
pub async fn lookup(
    app: &AppHandle,
    s: &Services<'_>,
    albums: Vec<Vec<Song>>,
    unnamed: Vec<Song>,
    listen: bool,
) -> Vec<AlbumLookup> {
    let looked = by_tags(app, s.mb, albums).await;
    if !listen {
        return looked.into_iter().map(|(a, _)| a).collect();
    }
    let mut out = Vec::new();
    let mut pending: Vec<(Option<AlbumLookup>, Vec<Song>)> = Vec::new();
    for (a, songs) in looked {
        if a.choice.is_none() && a.error.is_none() {
            pending.push((Some(a), songs));
        } else {
            out.push(a);
        }
    }
    if !unnamed.is_empty() {
        pending.push((None, unnamed));
    }
    if pending.is_empty() {
        return out;
    }
    let mut all: Vec<Song> = pending.iter().flat_map(|(_, s)| s.clone()).collect();
    let (heard, error) = hear(app, s, &mut all).await;
    let recordings: HashMap<PathBuf, Vec<String>> =
        all.into_iter().map(|(p, t)| (p, t.recordings)).collect();
    let mut releases = HashMap::new();
    for (by_name, mut songs) in pending {
        for (p, t) in &mut songs {
            t.recordings = recordings.get(p).cloned().unwrap_or_default();
        }
        let groups = by_sound(app, s, &songs, &heard, &error, &mut releases).await;
        match by_name {
            Some(a) if !a.candidates.is_empty() && groups.iter().all(|g| g.choice.is_none()) => {
                out.push(a)
            }
            _ => out.extend(groups),
        }
    }
    out
}

/// The lookups by tags, each with its songs.
async fn by_tags(
    app: &AppHandle,
    client: &Client,
    albums: Vec<Vec<Song>>,
) -> Vec<(AlbumLookup, Vec<Song>)> {
    let total = albums.len();
    progress(app, 0, total, None);
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
            heard: false,
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
                // Nothing by the full name: without what is in brackets.
                if found.candidates.is_empty()
                    && let Some(q) = mb::simpler(&query)
                {
                    found.candidates = client.search(&q).await?;
                }
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
        out.push((found, songs));
        progress(app, done + 1, total, None);
    }
    out
}

/// Fingerprints the songs and asks AcoustID what they are; each song gets
/// the recordings it was heard as. Also returns the error that stopped the
/// listening, if any.
async fn hear(
    app: &AppHandle,
    s: &Services<'_>,
    songs: &mut [Song],
) -> (HashMap<PathBuf, Heard>, Option<String>) {
    let total = songs.len();
    progress(app, 0, total, Some("listen"));
    let mut heard = HashMap::new();
    let mut error = None;
    for (done, (path, tags)) in songs.iter_mut().enumerate() {
        let print = match &error {
            Some(_) => None,
            None => {
                let p = path.clone();
                tauri::async_runtime::spawn_blocking(move || renome_core::fingerprint::of(&p))
                    .await
                    .ok()
                    .flatten()
            }
        };
        let h = match print {
            Some(print) => match s.acoustid.lookup(&print).await {
                Ok(h) => h,
                // The key, the connection: the next ones would fail too.
                Err(e) => {
                    error = Some(e);
                    Default::default()
                }
            },
            None => Default::default(),
        };
        tags.recordings = h.recordings.clone();
        if !h.recordings.is_empty() {
            s.heard
                .lock()
                .unwrap()
                .insert(path.clone(), h.recordings.clone());
        }
        heard.insert(path.clone(), h);
        progress(app, done + 1, total, Some("listen"));
    }
    (heard, error)
}

/// Splits each folder's songs among the releases they were heard on and
/// checks each album against its release, as [`lookup`] does. `releases`
/// keeps those fetched.
async fn by_sound(
    app: &AppHandle,
    s: &Services<'_>,
    songs: &[Song],
    heard: &HashMap<PathBuf, Heard>,
    error: &Option<String>,
    releases: &mut HashMap<String, Result<Release, String>>,
) -> Vec<AlbumLookup> {
    // Each folder (with its disc folders) on its own.
    let mut folders: Vec<(PathBuf, Vec<usize>)> = Vec::new();
    for (i, (p, _)) in songs.iter().enumerate() {
        let dir = music::album_dir(p.parent().unwrap_or(Path::new(""))).to_path_buf();
        match folders.iter_mut().find(|(d, _)| *d == dir) {
            Some((_, v)) => v.push(i),
            None => folders.push((dir, vec![i])),
        }
    }
    let by_path: HashMap<&Path, &Song> = songs.iter().map(|s| (s.0.as_path(), s)).collect();
    let mut found = Vec::new();
    for (_, idx) in &folders {
        let part: Vec<_> = idx
            .iter()
            .map(|&i| {
                let p = &songs[i].0;
                (p.clone(), heard.get(p).cloned().unwrap_or_default())
            })
            .collect();
        let mut groups = renome_core::acoustid::albums(&part);
        // A folder mostly on one release is that album: its other songs
        // (a remaster heard as the original recording, say) join it when
        // their tags fit one of its tracks.
        if let Some(best) = groups.first().and_then(|g| g.candidates.first())
            && part.len() > 1
            && groups[0].songs.len() * 2 >= part.len()
        {
            let id = best.id.clone();
            if !releases.contains_key(&id) {
                releases.insert(id.clone(), s.mb.release(&id).await);
            }
            if let Some(Ok(r)) = releases.get(&id) {
                let rest: Vec<Song> = groups[1..]
                    .iter()
                    .flat_map(|g| &g.songs)
                    .map(|p| by_path[p.as_path()].clone())
                    .collect();
                let fit: Vec<PathBuf> = mb::match_tracks(r, &rest)
                    .0
                    .into_iter()
                    .map(|(p, _)| p)
                    .collect();
                for g in &mut groups[1..] {
                    g.songs.retain(|p| !fit.contains(p));
                }
                groups[0].songs.extend(fit);
                groups.retain(|g| !g.songs.is_empty());
            }
        }
        found.extend(groups);
    }
    let total = found.len();
    progress(app, 0, total, None);
    let mut out = Vec::new();
    for (done, f) in found.into_iter().enumerate() {
        let album: Vec<Song> = f
            .songs
            .iter()
            .map(|p| by_path[p.as_path()].clone())
            .collect();
        let mut query = query_of(&album);
        if let Some(best) = f.candidates.first() {
            query.artist = best.artist.clone();
            query.album = best.title.clone();
            query.year = best.date.get(..4).and_then(|y| y.parse().ok());
        }
        // Nothing heard because AcoustID failed: say why.
        let failed = if f.candidates.is_empty() {
            error.clone()
        } else {
            None
        };
        let mut lookup = AlbumLookup {
            files: f.songs.clone(),
            dir: f.songs[0]
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            query,
            candidates: f.candidates,
            choice: None,
            matches: Vec::new(),
            error: failed,
            heard: true,
        };
        if let Some(best) = lookup.candidates.first() {
            if !releases.contains_key(&best.id) {
                releases.insert(best.id.clone(), s.mb.release(&best.id).await);
            }
            match &releases[&best.id] {
                Ok(r) => {
                    let matches = mb::match_tracks(r, &album).0;
                    if mb::sure(r, &album) {
                        // The release's own details (format, label) for the review.
                        let mut c = mb::rank(&lookup.query, std::slice::from_ref(r)).remove(0);
                        c.score = best.score;
                        lookup.candidates[0] = c.clone();
                        lookup.choice = Some(c);
                        lookup.matches = matches;
                    }
                }
                Err(e) => lookup.error = Some(e.clone()),
            }
        }
        lookup.candidates.truncate(12);
        out.push(lookup);
        progress(app, done + 1, total, None);
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
    progress(app, 0, total, None);
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
        progress(app, done + 1, total, None);
    }
    report
}
