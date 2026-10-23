//! TMDB client: movie and show search, details and images. The key is either a v3
//! API key (sent as `api_key`) or a v4 read access token (sent as a bearer
//! token); TMDB gives both on the same settings page.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use futures_util::{StreamExt, stream};
use renome_core::files::split_name;
use renome_core::tmdb::{self, Candidate, Details, Guess, SearchResult};
use renome_core::tv::{
    self, ApiEpisode, EpisodeDetails, EpisodeLookup, EpisodeMatch, SeasonFull, Show, ShowDetails,
    ShowQuery, TvDetails, TvResult,
};
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
        self.get_in(&self.lang, path, query).await
    }

    /// Like `get`, with titles in another language.
    async fn get_in<T: for<'de> Deserialize<'de>>(
        &self,
        lang: &str,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, String> {
        let mut req = self
            .http
            .get(format!("{}{path}", api_base()))
            .query(query)
            .query(&[("language", lang)]);
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

    /// Finds a show by id, link or name; the flag says the first one is a
    /// safe automatic choice.
    pub async fn search_tv(&self, query: &ShowQuery) -> Result<(Vec<Candidate>, bool), String> {
        let guess = match query {
            ShowQuery::Id(id) => {
                let d: TvDetails = self.get(&format!("/tv/{id}"), &[]).await?;
                let c = Candidate {
                    id: d.id,
                    year: tmdb::year_of(&d.first_air_date),
                    title: d.name,
                    original_title: d.original_name,
                    overview: d.overview.unwrap_or_default(),
                    poster_path: d.poster_path,
                    score: 1.0,
                };
                return Ok((vec![c], true));
            }
            ShowQuery::Name(g) => g,
        };
        let base = [("query", guess.title.as_str()), ("include_adult", "false")];
        let mut results: Vec<SearchResult> = Vec::new();
        if let Some(y) = guess.year {
            let y = y.to_string();
            let mut q = base.to_vec();
            q.push(("first_air_date_year", &y));
            let page: Page<TvResult> = self.get("/search/tv", &q).await?;
            results.extend(page.results.into_iter().map(Into::into));
        }
        let page: Page<TvResult> = self.get("/search/tv", &base).await?;
        results.extend(page.results.into_iter().map(Into::into));
        Ok(tmdb::rank(guess, results))
    }

    /// A show with all its episodes. Titles come in the user's language,
    /// with English and the original language for untranslated ones (and
    /// for matching file names in those languages).
    pub async fn show(&self, id: u64) -> Result<Show, String> {
        let path = format!("/tv/{id}");
        let base: TvDetails = self.get(&path, &[]).await?;
        let mut langs = vec![self.lang.clone()];
        if self.lang2() != "en" {
            langs.push("en-US".into());
        }
        let original = base.original_language.as_str();
        if !original.is_empty() && original != self.lang2() && original != "en" {
            langs.push(original.into());
        }
        // Up to 20 seasons per request.
        let seasons: Vec<String> = base
            .seasons
            .iter()
            .map(|s| format!("season/{}", s.season_number))
            .collect();
        let requests: Vec<(usize, String)> = langs
            .iter()
            .enumerate()
            .flat_map(|(i, _)| seasons.chunks(20).map(move |c| (i, c.join(","))))
            .collect();
        let mut by_lang: Vec<Vec<ApiEpisode>> = vec![Vec::new(); langs.len()];
        let mut tasks = stream::iter(requests)
            .map(|(i, append)| {
                let (path, lang) = (&path, &langs[i]);
                async move {
                    let d: Result<TvDetails, String> = self
                        .get_in(lang, path, &[("append_to_response", &append)])
                        .await;
                    (i, d)
                }
            })
            .buffer_unordered(4);
        while let Some((i, d)) = tasks.next().await {
            match d {
                Ok(d) => by_lang[i].extend(d.appended_episodes()),
                // The fallback languages are a bonus.
                Err(e) if i == 0 => return Err(e),
                Err(_) => {}
            }
        }
        Ok(tv::build_show(&base, &by_lang))
    }

    /// A show's details for its `.nfo`.
    pub async fn show_details(&self, id: u64) -> Result<ShowDetails, String> {
        self.get(
            &format!("/tv/{id}"),
            &[
                (
                    "append_to_response",
                    "aggregate_credits,content_ratings,keywords,external_ids,videos,images",
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

    /// A season with its episodes' details.
    pub async fn season(&self, id: u64, season: u16) -> Result<SeasonFull, String> {
        self.get(&format!("/tv/{id}/season/{season}"), &[]).await
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
#[serde(rename_all = "camelCase")]
pub struct TvItem {
    path: PathBuf,
    /// File name, with extension.
    name: String,
    is_dir: bool,
}

impl TvItem {
    /// The name to read numbers from: no extension, no subtitle language.
    fn stem(&self) -> &str {
        if self.is_dir {
            return &self.name;
        }
        let (stem, ext) = split_name(&self.name);
        &stem[..stem.len() - tmdb::subtitle_suffix(stem, ext).len()]
    }

    fn folder(&self) -> PathBuf {
        self.path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowSearch {
    /// What was searched for (guessed from the files when nothing was typed).
    query: String,
    candidates: Vec<Candidate>,
    auto: bool,
}

/// Searches for the show typed, or for the one the files are named after.
pub async fn search_show(
    client: &Client,
    query: &str,
    items: &[TvItem],
) -> Result<ShowSearch, String> {
    let q = if query.trim().is_empty() {
        let names: Vec<(String, PathBuf)> = items
            .iter()
            .map(|i| (i.stem().to_string(), i.folder()))
            .collect();
        ShowQuery::Name(tv::guess_show(&names))
    } else {
        tv::show_query(query)
    };
    let text = match &q {
        ShowQuery::Id(id) => id.to_string(),
        ShowQuery::Name(g) if g.title.is_empty() => return Err("tvNoShow".into()),
        ShowQuery::Name(g) => match g.year {
            Some(y) => format!("{} ({y})", g.title),
            None => g.title.clone(),
        },
    };
    let (candidates, auto) = client.search_tv(&q).await?;
    Ok(ShowSearch {
        query: text,
        candidates,
        auto,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TvFile {
    path: PathBuf,
    #[serde(flatten)]
    found: EpisodeLookup,
}

#[derive(Serialize)]
pub struct TvLookup {
    show: Show,
    files: Vec<TvFile>,
}

/// Fetches the show and finds each file's episode.
pub async fn lookup_episodes(
    client: &Client,
    id: u64,
    items: Vec<TvItem>,
) -> Result<TvLookup, String> {
    let show = client.show(id).await?;
    let files = items
        .iter()
        .map(|i| {
            let folder = i.folder();
            let folder = folder.file_name().unwrap_or_default().to_string_lossy();
            TvFile {
                path: i.path.clone(),
                found: tv::find(&show, &tv::parse(i.stem(), &folder)),
            }
        })
        .collect();
    Ok(TvLookup { show, files })
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

#[derive(Deserialize)]
pub struct TvSidecarItem {
    /// The renamed video.
    path: PathBuf,
    /// The show.
    id: u64,
    season: u16,
    episodes: Vec<u16>,
    /// The titles used in the name.
    titles: Vec<String>,
}

/// What goes into a metadata file.
enum Source {
    Text(String),
    Image(String),
}

/// Writes, for each renamed episode, its `.nfo` and thumbnail; and in the
/// show's folder (when there is one), `tvshow.nfo`, poster, the season's
/// poster and optionally backdrop and logo; `season.nfo` in a season
/// folder. `folders`: the files were just moved into `Show/Season NN`.
/// Existing files are never overwritten.
pub async fn tv_sidecars(
    app: &AppHandle,
    client: &Client,
    items: Vec<TvSidecarItem>,
    fanart: bool,
    folders: bool,
) -> SidecarReport {
    let mut report = SidecarReport::default();
    let name = |p: &Path| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };

    // Everything to fetch, once.
    let mut ids: Vec<u64> = items.iter().map(|i| i.id).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut seasons: Vec<(u64, u16)> = items.iter().map(|i| (i.id, i.season)).collect();
    seasons.sort_unstable();
    seasons.dedup();
    let _ = app.emit(
        "tmdb-progress",
        Progress {
            done: 0,
            total: items.len(),
        },
    );
    let shows: HashMap<u64, Result<ShowDetails, String>> = stream::iter(ids)
        .map(|id| async move { (id, client.show_details(id).await) })
        .buffer_unordered(4)
        .collect()
        .await;
    let fetched: HashMap<(u64, u16), Result<SeasonFull, String>> = stream::iter(seasons)
        .map(|(id, n)| async move { ((id, n), client.season(id, n).await) })
        .buffer_unordered(4)
        .collect()
        .await;

    let mut jobs: Vec<(PathBuf, Source)> = Vec::new();
    let mut add = |path: PathBuf, source: Source| {
        if std::fs::symlink_metadata(&path).is_err() && !jobs.iter().any(|(p, _)| *p == path) {
            jobs.push((path, source));
        }
    };
    for item in &items {
        let show = match shows.get(&item.id) {
            Some(Ok(d)) => d,
            Some(Err(e)) => {
                report.errors.push((name(&item.path), e.clone()));
                continue;
            }
            None => continue,
        };
        let season = match fetched.get(&(item.id, item.season)) {
            Some(Ok(s)) => s,
            Some(Err(e)) => {
                report.errors.push((name(&item.path), e.clone()));
                continue;
            }
            None => continue,
        };
        let episodes: Vec<&EpisodeDetails> = item
            .episodes
            .iter()
            .filter_map(|&n| season.episodes.iter().find(|e| e.episode_number == n))
            .collect();
        if !episodes.is_empty() {
            add(
                sibling(&item.path, ".nfo"),
                Source::Text(tv::episode_nfo(&show.name, &episodes, &item.titles)),
            );
            if let Some(p) = &episodes[0].still_path {
                add(sibling(&item.path, "-thumb.jpg"), Source::Image(p.clone()));
            }
        }

        let dir = item.path.parent().unwrap_or(Path::new(""));
        let in_season = tv::folder_season(&name(dir)) == Some(item.season);
        let m = EpisodeMatch {
            id: show.id,
            show: show.name.clone(),
            original_show: show.original_name.clone(),
            year: None,
            season: item.season,
            episodes: item.episodes.clone(),
            titles: Vec::new(),
        };
        let show_dir = if folders {
            dir.parent().map(Path::to_path_buf)
        } else {
            tv::show_dir(dir, &m)
        };
        let summary = show.seasons.iter().find(|s| s.season_number == item.season);
        if in_season && let Some(s) = summary {
            add(dir.join("season.nfo"), Source::Text(tv::season_nfo(s)));
        }
        let Some(show_dir) = show_dir else { continue };
        add(
            show_dir.join("tvshow.nfo"),
            Source::Text(tv::show_nfo(show, &client.lang)),
        );
        if let Some(p) = &show.poster_path {
            add(show_dir.join("poster.jpg"), Source::Image(p.clone()));
        }
        if let Some(p) = summary
            .and_then(|s| s.poster_path.clone())
            .or(season.poster_path.clone())
        {
            add(
                show_dir.join(tv::season_poster(item.season)),
                Source::Image(p),
            );
        }
        if fanart {
            if let Some(p) = &show.backdrop_path {
                add(show_dir.join("fanart.jpg"), Source::Image(p.clone()));
            }
            if let Some(p) = show.images.logo(client.lang2()) {
                add(show_dir.join("logo.png"), Source::Image(p.to_string()));
            }
        }
    }

    let total = jobs.len();
    let _ = app.emit("tmdb-progress", Progress { done: 0, total });
    let mut tasks = stream::iter(jobs)
        .map(|(path, source)| async move {
            let data = match source {
                Source::Text(t) => Ok(t.into_bytes()),
                Source::Image(p) => client.image(&p).await,
            };
            let r = data.and_then(|bytes| write_new(&path, &bytes));
            (path, r)
        })
        .buffer_unordered(4);
    let mut done = 0;
    while let Some((path, r)) = tasks.next().await {
        match r {
            Ok(()) => report.created.push(path),
            Err(e) => report.errors.push((name(&path), e)),
        }
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
