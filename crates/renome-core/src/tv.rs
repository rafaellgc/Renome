//! TV shows from TMDB, without the network part: reading season and
//! episode numbers from file names, matching files to the episodes of a
//! show and building the new name.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::text;
use crate::tmdb::{
    self, Cast, Crew, Guess, IMAGE_BASE, Images, Named, SearchResult, TAGS, Videos, before_tags,
    clean, crew, number, person, rating_for, safe_title, similarity, tag, trailer_url, xml,
    year_of,
};

/// Season, episode and title read from a file name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeGuess {
    /// Text before the numbers: usually the show's name.
    pub show: String,
    pub season: Option<u16>,
    /// One, or more for a file with several episodes (`S01E01E02`).
    pub episodes: Vec<u16>,
    /// A bare number (`Naruto - 125`, `Show 101`): absolute numbering, or
    /// season and episode run together.
    pub number: Option<u32>,
    /// Air date (`2024-03-15`), for daily shows.
    pub date: Option<String>,
    /// Text after the numbers: maybe the episode's title.
    pub title: String,
}

/// S01E02, s1e2, S01 E02, T1E2, S01E01E02, S01E01-02, S2015E05.
static SXE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[st](\d{1,4}) ?-? ?ep? ?(\d{1,4})((?: ?-? ?e\d{1,4}\b|-\d{1,4}\b)*)")
        .unwrap()
});
/// 6ª Temporada Ep. 2, 2a temporada episódio 3.
static ORDINAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(\d{1,2})[ªºa°]? ?temporada\W{1,4}(?:episódio|episodio|ep|e|capítulo|capitulo|cap)[. ]{0,2}(\d{1,4})\b",
    )
    .unwrap()
});
/// 1x02.
static NXN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(\d{1,2})x(\d{2,3})\b((?:-\d{2,3}\b)*)").unwrap());
/// Temporada 1 Episódio 2, Season 1, Episode 2, Temp. 1 Ep. 2.
static LONG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:season|temporada|temp|saison|staffel)[. ]{0,2}(\d{1,2})\W{1,4}(?:episode|episódio|episodio|épisode|ep|e|capítulo|capitulo|cap|folge)[. ]{0,2}(\d{1,3})\b",
    )
    .unwrap()
});
/// 2024.03.15 (dots are spaces by then).
static DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b((?:19|20)\d{2})[ -](\d{2})[ -](\d{2})\b").unwrap());
/// Episódio 5, Ep 05, E05 (season from the folder).
static EP_ONLY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:(?:episode|episódio|episodio|épisode|ep|capítulo|capitulo|cap|chapter|folge|part|parte)[. ]{0,2}|e)(\d{1,4})\b",
    )
    .unwrap()
});
static BARE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d{1,4}\b").unwrap());
static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());
/// Season 2, Temporada 02, S02.
static SEASON_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:season|temporada|temp|saison|staffel)[ ._-]*(\d{1,3})\b|^[st](\d{1,3})$")
        .unwrap()
});
static SPECIALS_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:specials?|especiais|especiales|extras)$").unwrap());

/// The season a folder stands for (`Season 2`, `Temporada 02`,
/// `Specials` = 0).
pub fn folder_season(folder: &str) -> Option<u16> {
    if SPECIALS_DIR.is_match(folder.trim()) {
        return Some(0);
    }
    let c = SEASON_DIR.captures(folder.trim())?;
    c.get(1).or_else(|| c.get(2))?.as_str().parse().ok()
}

/// Reads season, episode and title from a file name without extension;
/// `folder` (the name of the folder it is in) can give the season.
pub fn parse(name: &str, folder: &str) -> EpisodeGuess {
    let spaced = name.replace(['.', '_'], " ");
    let num = |s: &str| s.parse::<u16>().ok();
    let extra = |s: &str| {
        DIGITS
            .find_iter(s)
            .filter_map(|m| num(m.as_str()))
            .collect::<Vec<_>>()
    };
    let mut g = EpisodeGuess::default();
    let range = if let Some(c) = SXE.captures(&spaced) {
        g.season = num(&c[1]);
        g.episodes = num(&c[2]).into_iter().chain(extra(&c[3])).collect();
        c.get(0).map(|m| m.range())
    } else if let Some(c) = NXN.captures(&spaced) {
        g.season = num(&c[1]);
        g.episodes = num(&c[2]).into_iter().chain(extra(&c[3])).collect();
        c.get(0).map(|m| m.range())
    } else if let Some(c) = LONG.captures(&spaced).or_else(|| ORDINAL.captures(&spaced)) {
        g.season = num(&c[1]);
        g.episodes = num(&c[2]).into_iter().collect();
        c.get(0).map(|m| m.range())
    } else if let Some(c) = DATE.captures(&spaced) {
        g.date = Some(format!("{}-{}-{}", &c[1], &c[2], &c[3]));
        c.get(0).map(|m| m.range())
    } else if let Some(c) = EP_ONLY.captures(&spaced) {
        g.episodes = num(&c[1]).into_iter().collect();
        c.get(0).map(|m| m.range())
    } else {
        // A bare number, before any release tag (1080p, x264…), that is
        // not a year.
        let cut = TAGS.find(&spaced).map_or(spaced.len(), |m| m.start());
        BARE.find_iter(&spaced[..cut])
            .find(|m| {
                let s = m.as_str();
                !(s.len() == 4 && (s.starts_with("19") || s.starts_with("20")))
            })
            .map(|m| {
                g.number = m.as_str().parse().ok();
                m.range()
            })
    };
    let Some(range) = range else {
        g.title = clean(&before_tags(&spaced));
        return g;
    };
    g.show = clean(&spaced[..range.start]);
    let after = &spaced[range.end..];
    g.title = clean(&after[..TAGS.find(after).map_or(after.len(), |m| m.start())]);
    if g.season.is_none() {
        let season = folder_season(folder);
        match (g.number, season) {
            // `Season 2/205.mkv`: season and episode run together.
            (Some(n), Some(s)) if n >= 100 && n / 100 == u32::from(s) => {
                g.episodes = vec![(n % 100) as u16];
                g.number = None;
            }
            // `Season 2/05.mkv`: episode 5 of season 2.
            (Some(n), Some(_)) if n < 1000 => {
                g.episodes = vec![n as u16];
                g.number = None;
            }
            _ => {}
        }
        if !g.episodes.is_empty() {
            g.season = season;
        }
    }
    g
}

/// The show to search for when the user typed nothing: the name most of
/// the files start with, else the folder (the one above, for a season
/// folder). Items are (file name without extension, folder path).
pub fn guess_show(items: &[(String, std::path::PathBuf)]) -> Guess {
    let mut count: HashMap<String, (usize, String)> = HashMap::new();
    for (name, dir) in items {
        let folder = dir.file_name().unwrap_or_default().to_string_lossy();
        let show = parse(name, &folder).show;
        if !show.is_empty() {
            let e = count.entry(show.to_lowercase()).or_insert((0, show));
            e.0 += 1;
        }
    }
    let text = match count.into_values().max_by_key(|(n, _)| *n) {
        Some((_, show)) => show,
        None => items
            .first()
            .map(|(_, dir)| {
                let name = |p: &std::path::Path| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                };
                match dir.parent() {
                    Some(up) if folder_season(&name(dir)).is_some() => name(up),
                    _ => name(dir),
                }
            })
            .unwrap_or_default(),
    };
    tmdb::guess(&text)
}

/// What the user typed to find a show: a TMDB id, a TMDB link or a name.
pub enum ShowQuery {
    Id(u64),
    Name(Guess),
}

static TV_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)themoviedb\.org/tv/(\d+)|tmdbid[-=](\d+)").unwrap());

pub fn show_query(q: &str) -> ShowQuery {
    let q = q.trim();
    if let Ok(id) = q.parse() {
        return ShowQuery::Id(id);
    }
    if let Some(id) = TV_LINK
        .captures(q)
        .and_then(|c| c.get(1).or_else(|| c.get(2))?.as_str().parse().ok())
    {
        return ShowQuery::Id(id);
    }
    ShowQuery::Name(tmdb::guess(q))
}

/// A show search result as the TMDB API returns it.
#[derive(Debug, Clone, Deserialize)]
pub struct TvResult {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub original_name: String,
    #[serde(default)]
    pub first_air_date: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub popularity: f64,
}

impl From<TvResult> for SearchResult {
    fn from(r: TvResult) -> Self {
        SearchResult {
            id: r.id,
            title: r.name,
            original_title: r.original_name,
            release_date: r.first_air_date,
            overview: r.overview,
            poster_path: r.poster_path,
            popularity: r.popularity,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeasonInfo {
    pub season_number: u16,
}

/// Show details (`/tv/{id}`); seasons asked for with `append_to_response`
/// come as `season/N` keys.
#[derive(Debug, Clone, Deserialize)]
pub struct TvDetails {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub original_name: String,
    #[serde(default)]
    pub original_language: String,
    #[serde(default)]
    pub first_air_date: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub seasons: Vec<SeasonInfo>,
    #[serde(flatten)]
    pub appended: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiEpisode {
    pub season_number: u16,
    pub episode_number: u16,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub air_date: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub still_path: Option<String>,
}

#[derive(Deserialize)]
struct SeasonDetails {
    #[serde(default)]
    episodes: Vec<ApiEpisode>,
}

impl TvDetails {
    /// Episodes of the seasons appended to this response.
    pub fn appended_episodes(&self) -> Vec<ApiEpisode> {
        self.appended
            .iter()
            .filter(|(k, _)| k.starts_with("season/"))
            .filter_map(|(_, v)| serde_json::from_value::<SeasonDetails>(v.clone()).ok())
            .flat_map(|s| s.episodes)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub season: u16,
    pub number: u16,
    pub name: String,
    /// The title in the other languages fetched, for matching file names.
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub air_date: Option<String>,
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub still_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Show {
    pub id: u64,
    pub name: String,
    pub original_name: String,
    pub year: Option<u16>,
    pub overview: String,
    pub poster_path: Option<String>,
    /// Every episode, specials (season 0) included, in order.
    pub episodes: Vec<Episode>,
}

static GENERIC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s*(?:(?:episode|episódio|episodio|épisode|folge|aflevering|odcinek|capítulo|capitulo|ep)\.?\s*)?第?\s*\d+\s*話?\s*$",
    )
    .unwrap()
});

/// TMDB fills untranslated titles with "Episode 3" in each language.
fn is_generic(name: &str) -> bool {
    name.trim().is_empty() || GENERIC.is_match(name)
}

/// Builds the show from its details and its episodes in several
/// languages: the user's first, then fallbacks (English, the original).
/// Each episode is named in the first language that has a real title.
pub fn build_show(d: &TvDetails, by_lang: &[Vec<ApiEpisode>]) -> Show {
    let mut titles: HashMap<(u16, u16), Vec<String>> = HashMap::new();
    for list in by_lang {
        for e in list {
            let name = e.name.clone().unwrap_or_default();
            let t = titles
                .entry((e.season_number, e.episode_number))
                .or_default();
            if !is_generic(&name) && !t.contains(&name) {
                t.push(name);
            }
        }
    }
    let mut episodes: Vec<Episode> = by_lang
        .first()
        .map(|list| {
            list.iter()
                .map(|e| {
                    let k = (e.season_number, e.episode_number);
                    let mut names = titles.remove(&k).unwrap_or_default();
                    let name = if names.is_empty() {
                        e.name.clone().unwrap_or_default()
                    } else {
                        names.remove(0)
                    };
                    Episode {
                        season: k.0,
                        number: k.1,
                        name,
                        names,
                        air_date: e.air_date.clone().filter(|s| !s.is_empty()),
                        overview: e.overview.clone().unwrap_or_default(),
                        still_path: e.still_path.clone(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    episodes.sort_by_key(|e| (e.season == 0, e.season, e.number));
    episodes.dedup_by_key(|e| (e.season, e.number));
    Show {
        id: d.id,
        name: d.name.clone(),
        original_name: d.original_name.clone(),
        year: year_of(&d.first_air_date),
        overview: d.overview.clone().unwrap_or_default(),
        poster_path: d.poster_path.clone(),
        episodes,
    }
}

/// Season and episode.
pub type Key = (u16, u16);

/// The episode found for a file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeLookup {
    pub guess: EpisodeGuess,
    /// Picked with confidence (several for a multi-episode file); empty
    /// when the user has to choose.
    pub choice: Vec<Key>,
    /// The numbers in the name point to another episode than its title.
    pub conflict: bool,
    /// Likely episodes, best first (the first one is the suggestion).
    pub alternatives: Vec<Vec<Key>>,
}

fn words(s: &str) -> Vec<String> {
    text::strip_accents(&s.to_lowercase())
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// The title without the show's name in front ("Breaking Bad Pilot").
fn without_show(title: &str, show: &Show) -> String {
    let t = words(title);
    for name in [&show.name, &show.original_name] {
        let n = words(name);
        if !n.is_empty() && t.len() > n.len() && t.starts_with(&n) {
            return t[n.len()..].join(" ");
        }
    }
    title.to_string()
}

/// Finds the episode for a file, crossing the numbers in its name with
/// its title: when the title clearly names another episode than the
/// numbers, the user decides (suggesting the title's).
pub fn find(show: &Show, g: &EpisodeGuess) -> EpisodeLookup {
    let has = |k: Key| show.episodes.iter().any(|e| (e.season, e.number) == k);
    let regular: Vec<Key> = show
        .episodes
        .iter()
        .filter(|e| e.season > 0)
        .map(|e| (e.season, e.number))
        .collect();
    let absolute = |n: u32| regular.get((n as usize).checked_sub(1)?).copied();
    let mut seasons: Vec<u16> = regular.iter().map(|k| k.0).collect();
    seasons.dedup();

    let mut by_number: Vec<Vec<Key>> = Vec::new();
    let mut push = |v: Option<Vec<Key>>| {
        if let Some(v) = v
            && !v.is_empty()
            && v.iter().all(|&k| has(k))
            && !by_number.contains(&v)
        {
            by_number.push(v);
        }
    };
    if !g.episodes.is_empty() {
        push(match g.season {
            Some(s) => Some(g.episodes.iter().map(|&e| (s, e)).collect()),
            // No season: the only one, or the absolute order.
            None if seasons.len() <= 1 => Some(g.episodes.iter().map(|&e| (1, e)).collect()),
            None => g.episodes.iter().map(|&e| absolute(e.into())).collect(),
        });
    }
    if let Some(n) = g.number {
        let run_together = (100..10_000)
            .contains(&n)
            .then(|| vec![((n / 100) as u16, (n % 100) as u16)]);
        // Long seasons (anime) count episodes straight through.
        let long = show.episodes.iter().any(|e| e.number > 99);
        if long {
            match absolute(n) {
                Some(k) => push(Some(vec![k])),
                None => push(run_together),
            }
        } else {
            push(run_together);
            push(absolute(n).map(|k| vec![k]));
        }
    }
    if let Some(d) = &g.date {
        for e in show
            .episodes
            .iter()
            .filter(|e| e.air_date.as_ref() == Some(d))
        {
            push(Some(vec![(e.season, e.number)]));
        }
    }

    let title = without_show(&g.title, show);
    let sim = |k: Key| {
        show.episodes
            .iter()
            .find(|e| (e.season, e.number) == k)
            .map_or(0.0, |e| {
                std::iter::once(&e.name)
                    .chain(&e.names)
                    .map(|n| similarity(&title, n))
                    .fold(0.0, f64::max)
            })
    };
    let mut scored: Vec<(Key, f64)> = if title.is_empty() {
        Vec::new()
    } else {
        show.episodes
            .iter()
            .map(|e| ((e.season, e.number), sim((e.season, e.number))))
            .collect()
    };
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    let top = scored.first().copied();
    let group_sim = |grp: &Vec<Key>| grp.iter().map(|&k| sim(k)).fold(0.0, f64::max);

    let found = |choice: Vec<Key>, alternatives: Vec<Vec<Key>>| EpisodeLookup {
        guess: g.clone(),
        choice,
        conflict: false,
        alternatives,
    };
    if !by_number.is_empty() {
        if title.is_empty() {
            return if by_number.len() == 1 {
                found(by_number[0].clone(), by_number)
            } else {
                found(Vec::new(), by_number)
            };
        }
        by_number.sort_by(|a, b| group_sim(b).total_cmp(&group_sim(a)));
        let best = group_sim(&by_number[0]);
        if let Some((k, s)) = top
            && s >= 0.7
            && s > best + 0.2
            && !by_number.iter().any(|grp| grp.contains(&k))
        {
            let mut alternatives = vec![vec![k]];
            alternatives.extend(by_number);
            return EpisodeLookup {
                guess: g.clone(),
                choice: Vec::new(),
                conflict: true,
                alternatives,
            };
        }
        let clear = by_number
            .get(1)
            .is_none_or(|second| best >= 0.6 && group_sim(second) < best - 0.1);
        return if clear {
            found(by_number[0].clone(), by_number)
        } else {
            found(Vec::new(), by_number)
        };
    }
    let alternatives: Vec<Vec<Key>> = scored
        .iter()
        .filter(|(_, s)| *s >= 0.4)
        .take(5)
        .map(|(k, _)| vec![*k])
        .collect();
    match (scored.first(), scored.get(1)) {
        (Some((k, s)), second) if *s >= 0.85 && second.is_none_or(|x| x.1 < s - 0.1) => {
            found(vec![*k], alternatives)
        }
        _ => found(Vec::new(), alternatives),
    }
}

/// The episode chosen for a file, as the rule needs it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeMatch {
    pub id: u64,
    pub show: String,
    #[serde(default)]
    pub original_show: String,
    #[serde(default)]
    pub year: Option<u16>,
    pub season: u16,
    pub episodes: Vec<u16>,
    #[serde(default)]
    pub titles: Vec<String>,
}

/// The new name from the rule's template: `{show}`, `{original}`,
/// `{year}`, `{season}`, `{episode}`, `{title}`, `{id}` (also in
/// Portuguese and Spanish). Numbers have two digits; several episodes
/// become `01-E02` (so `S{season}E{episode}` gives `S01E01-E02`).
pub fn episode_name(template: &str, m: &EpisodeMatch, colon: &str) -> String {
    let show = safe_title(&m.show, colon);
    let original = safe_title(&m.original_show, colon);
    let year = m.year.map(|y| y.to_string()).unwrap_or_default();
    let season = format!("{:02}", m.season);
    let episode = m
        .episodes
        .iter()
        .map(|e| format!("{e:02}"))
        .collect::<Vec<_>>()
        .join("-E");
    let mut titles: Vec<&str> = Vec::new();
    for t in &m.titles {
        if !t.is_empty() && !titles.contains(&t.as_str()) {
            titles.push(t);
        }
    }
    let title = safe_title(&titles.join(" & "), colon);
    let id = m.id.to_string();
    let out = crate::rules::expand(template, |key| match key {
        "show" | "série" | "serie" | "series" => Some(show.as_str()),
        "original" => Some(original.as_str()),
        "year" | "ano" | "año" => Some(year.as_str()),
        "season" | "temporada" => Some(season.as_str()),
        "episode" | "episódio" | "episodio" => Some(episode.as_str()),
        "title" | "título" | "titulo" => Some(title.as_str()),
        "id" | "tmdbid" => Some(id.as_str()),
        _ => None,
    });
    let out = out.replace("()", "").replace("[]", "");
    // A missing title leaves a dangling " - ".
    text::collapse_spaces(&out)
        .trim()
        .trim_end_matches([' ', '-', '–', ':'])
        .trim_start_matches([' ', '-', '–', ':'])
        .to_string()
}

/// The season's folder, as Jellyfin and Kodi expect it: `Season 01`.
pub fn season_folder(season: u16) -> String {
    format!("Season {season:02}")
}

/// Whether a folder is named after the show: its TMDB id, or (almost) its
/// name or original name, with or without the year.
fn names_show(folder: &Path, m: &EpisodeMatch) -> bool {
    let name = folder.file_name().unwrap_or_default().to_string_lossy();
    if name.contains(&format!("tmdbid-{}", m.id)) {
        return true;
    }
    let title = tmdb::guess(&name).title;
    let w = words(&title);
    [&m.show, &m.original_show].iter().any(|show| {
        let s = words(show);
        !s.is_empty() && (similarity(&title, show) >= 0.8 || w.starts_with(&s))
    })
}

/// The show's folder a file is already in: its own folder, or the one
/// above a season folder.
pub fn show_dir(dir: &Path, m: &EpisodeMatch) -> Option<PathBuf> {
    let name = dir.file_name().unwrap_or_default().to_string_lossy();
    if folder_season(&name).is_some() {
        dir.parent()
            .filter(|up| names_show(up, m))
            .map(Path::to_path_buf)
    } else {
        names_show(dir, m).then(|| dir.to_path_buf())
    }
}

/// Where the show's folder goes: next to the show's current folder (so
/// `Show/Season 1/x.mkv` becomes `Show (2008)/Season 01/…`, not a folder
/// inside it), next to a loose season folder, or else in the file's folder.
pub fn library_root(dir: &Path, m: &EpisodeMatch) -> PathBuf {
    let name = dir.file_name().unwrap_or_default().to_string_lossy();
    let up = match show_dir(dir, m) {
        Some(show) => show.parent().map(Path::to_path_buf),
        None if folder_season(&name).is_some() => dir.parent().map(Path::to_path_buf),
        None => None,
    };
    up.unwrap_or_else(|| dir.to_path_buf())
}

#[derive(Debug, Default, Deserialize)]
pub struct Role {
    #[serde(default)]
    pub character: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ShowCast {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub roles: Vec<Role>,
    #[serde(default)]
    pub profile_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ShowCredits {
    #[serde(default)]
    pub cast: Vec<ShowCast>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ContentRating {
    #[serde(default)]
    pub iso_3166_1: String,
    #[serde(default)]
    pub rating: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ContentRatings {
    #[serde(default)]
    pub results: Vec<ContentRating>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ShowKeywords {
    #[serde(default)]
    pub results: Vec<Named>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ExternalIds {
    #[serde(default)]
    pub imdb_id: Option<String>,
    #[serde(default)]
    pub tvdb_id: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct SeasonSummary {
    pub season_number: u16,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub air_date: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
}

/// Show details for the metadata (`/tv/{id}` with aggregate credits,
/// content ratings, keywords, external ids, videos and images).
#[derive(Debug, Default, Deserialize)]
pub struct ShowDetails {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub original_name: String,
    #[serde(default)]
    pub first_air_date: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub tagline: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub genres: Vec<Named>,
    #[serde(default)]
    pub networks: Vec<Named>,
    #[serde(default)]
    pub production_companies: Vec<Named>,
    #[serde(default)]
    pub vote_average: Option<f64>,
    #[serde(default)]
    pub vote_count: Option<u64>,
    #[serde(default)]
    pub episode_run_time: Vec<u32>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub created_by: Vec<Named>,
    #[serde(default)]
    pub external_ids: ExternalIds,
    #[serde(default)]
    pub content_ratings: ContentRatings,
    #[serde(default)]
    pub keywords: ShowKeywords,
    #[serde(default)]
    pub aggregate_credits: ShowCredits,
    #[serde(default)]
    pub videos: Videos,
    #[serde(default)]
    pub images: Images,
    #[serde(default)]
    pub seasons: Vec<SeasonSummary>,
}

/// An episode with what its `.nfo` needs (`/tv/{id}/season/{n}`).
#[derive(Debug, Default, Deserialize)]
pub struct EpisodeDetails {
    #[serde(default)]
    pub id: u64,
    pub season_number: u16,
    pub episode_number: u16,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub air_date: Option<String>,
    #[serde(default)]
    pub runtime: Option<u32>,
    #[serde(default)]
    pub vote_average: Option<f64>,
    #[serde(default)]
    pub vote_count: Option<u64>,
    #[serde(default)]
    pub still_path: Option<String>,
    #[serde(default)]
    pub crew: Vec<Crew>,
    #[serde(default)]
    pub guest_stars: Vec<Cast>,
}

/// A season with its episodes (`/tv/{id}/season/{n}`).
#[derive(Debug, Default, Deserialize)]
pub struct SeasonFull {
    pub season_number: u16,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub air_date: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub episodes: Vec<EpisodeDetails>,
}

const XML_HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// `tvshow.nfo` (Kodi format, read by Jellyfin, Kodi and Emby), with the
/// fields Jellyfin's reader takes that TMDB has. `lang` (`pt-BR`): its
/// country picks the age rating, its language the trailer.
pub fn show_nfo(d: &ShowDetails, lang: &str) -> String {
    let (lang2, region) = lang.split_once('-').unwrap_or((lang, "US"));
    let mut o = format!("{XML_HEAD}<tvshow>\n");
    let premiered = d.first_air_date.as_deref().unwrap_or_default();
    tag(&mut o, "title", &d.name);
    tag(&mut o, "originaltitle", &d.original_name);
    tag(&mut o, "showtitle", &d.name);
    tag(
        &mut o,
        "year",
        &year_of(&d.first_air_date)
            .map(|y| y.to_string())
            .unwrap_or_default(),
    );
    tag(&mut o, "premiered", premiered);
    tag(&mut o, "plot", d.overview.as_deref().unwrap_or_default());
    tag(&mut o, "tagline", d.tagline.as_deref().unwrap_or_default());
    tag(
        &mut o,
        "runtime",
        &number(d.episode_run_time.first().copied()),
    );
    tag(
        &mut o,
        "rating",
        &d.vote_average
            .filter(|&v| v > 0.0)
            .map(|v| format!("{v:.1}"))
            .unwrap_or_default(),
    );
    tag(&mut o, "votes", &number(d.vote_count));
    let cert = d
        .content_ratings
        .results
        .iter()
        .find(|c| c.iso_3166_1.eq_ignore_ascii_case(region))
        .map(|c| rating_for(region, &c.rating))
        .unwrap_or_default();
    tag(&mut o, "mpaa", &cert);
    // Jellyfin's series states.
    let status = match d.status.as_deref() {
        Some("Ended" | "Canceled") => "Ended",
        Some("Returning Series" | "In Production") => "Continuing",
        Some("Planned") => "Unreleased",
        _ => "",
    };
    tag(&mut o, "status", status);
    for g in &d.genres {
        tag(&mut o, "genre", &g.name);
    }
    for k in &d.keywords.results {
        tag(&mut o, "tag", &k.name);
    }
    for s in d.networks.iter().chain(&d.production_companies) {
        tag(&mut o, "studio", &s.name);
    }
    for c in &d.created_by {
        tag(&mut o, "credits", &c.name);
    }
    if let Some(key) = d.videos.trailer(lang2) {
        tag(&mut o, "trailer", &trailer_url(key));
    }
    let imdb = d.external_ids.imdb_id.as_deref().unwrap_or_default();
    let tvdb = d
        .external_ids
        .tvdb_id
        .map(|i| i.to_string())
        .unwrap_or_default();
    tag(&mut o, "tmdbid", &d.id.to_string());
    tag(&mut o, "imdb_id", imdb);
    tag(&mut o, "tvdbid", &tvdb);
    o.push_str(&format!(
        "  <uniqueid type=\"tmdb\" default=\"true\">{}</uniqueid>\n",
        d.id
    ));
    if !imdb.is_empty() {
        o.push_str(&format!(
            "  <uniqueid type=\"imdb\">{}</uniqueid>\n",
            xml(imdb)
        ));
    }
    if !tvdb.is_empty() {
        o.push_str(&format!("  <uniqueid type=\"tvdb\">{tvdb}</uniqueid>\n"));
    }
    // Kodi 20's form, which Jellyfin also reads.
    let mut guide = format!("{{\"tmdb\":\"{}\"", d.id);
    if !imdb.is_empty() {
        guide.push_str(&format!(",\"imdb\":\"{imdb}\""));
    }
    if !tvdb.is_empty() {
        guide.push_str(&format!(",\"tvdb\":\"{tvdb}\""));
    }
    guide.push('}');
    tag(&mut o, "episodeguide", &guide);
    for (i, a) in d.aggregate_credits.cast.iter().take(20).enumerate() {
        let role = a
            .roles
            .first()
            .map(|r| r.character.as_str())
            .unwrap_or_default();
        person(&mut o, &a.name, role, "Actor", i, &a.profile_path);
    }
    o.push_str("</tvshow>\n");
    o
}

/// `season.nfo`, inside the season's folder.
pub fn season_nfo(s: &SeasonSummary) -> String {
    let mut o = format!("{XML_HEAD}<season>\n");
    tag(&mut o, "title", &s.name);
    tag(&mut o, "plot", s.overview.as_deref().unwrap_or_default());
    tag(
        &mut o,
        "premiered",
        s.air_date.as_deref().unwrap_or_default(),
    );
    tag(
        &mut o,
        "releasedate",
        s.air_date.as_deref().unwrap_or_default(),
    );
    tag(&mut o, "seasonnumber", &s.season_number.to_string());
    o.push_str("</season>\n");
    o
}

/// The episode's `.nfo`: one `<episodedetails>` per episode in the file.
/// `titles` are the names used for the file (with the fallback language
/// for untranslated ones).
pub fn episode_nfo(show: &str, episodes: &[&EpisodeDetails], titles: &[String]) -> String {
    let mut o = String::from(XML_HEAD);
    for (i, e) in episodes.iter().enumerate() {
        o.push_str("<episodedetails>\n");
        let title = titles
            .get(i)
            .filter(|t| !t.is_empty())
            .cloned()
            .or_else(|| e.name.clone())
            .unwrap_or_default();
        let aired = e.air_date.as_deref().unwrap_or_default();
        tag(&mut o, "title", &title);
        tag(&mut o, "showtitle", show);
        tag(&mut o, "season", &e.season_number.to_string());
        tag(&mut o, "episode", &e.episode_number.to_string());
        tag(&mut o, "aired", aired);
        tag(&mut o, "premiered", aired);
        tag(&mut o, "plot", e.overview.as_deref().unwrap_or_default());
        tag(&mut o, "runtime", &number(e.runtime));
        tag(
            &mut o,
            "rating",
            &e.vote_average
                .filter(|&v| v > 0.0)
                .map(|v| format!("{v:.1}"))
                .unwrap_or_default(),
        );
        tag(&mut o, "votes", &number(e.vote_count));
        if e.id > 0 {
            o.push_str(&format!(
                "  <uniqueid type=\"tmdb\" default=\"true\">{}</uniqueid>\n",
                e.id
            ));
        }
        for c in crew(&e.crew, |c| c.job == "Director") {
            tag(&mut o, "director", &c.name);
        }
        for c in crew(&e.crew, |c| c.department == "Writing") {
            tag(&mut o, "writer", &c.name);
        }
        if let Some(p) = &e.still_path {
            tag(&mut o, "thumb", &format!("{IMAGE_BASE}{p}"));
        }
        for (i, a) in e.guest_stars.iter().take(20).enumerate() {
            person(
                &mut o,
                &a.name,
                &a.character,
                "GuestStar",
                i,
                &a.profile_path,
            );
        }
        o.push_str("</episodedetails>\n");
    }
    o
}

/// The season's poster in the show's folder, as Jellyfin and Kodi name it.
pub fn season_poster(season: u16) -> String {
    if season == 0 {
        "season-specials-poster.jpg".into()
    } else {
        format!("season{season:02}-poster.jpg")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str) -> (Option<u16>, Vec<u16>, Option<u32>, String, String) {
        let g = parse(name, "");
        (g.season, g.episodes, g.number, g.show, g.title)
    }

    #[test]
    fn numbers() {
        let s = |x: &str| x.to_string();
        assert_eq!(
            p("Breaking.Bad.S01E03.And.the.Bags.in.the.River.720p.WEB-DL"),
            (
                Some(1),
                vec![3],
                None,
                s("Breaking Bad"),
                s("And the Bags in the River")
            )
        );
        assert_eq!(p("Show s1e2").1, vec![2]);
        assert_eq!(p("Show T2 E8 - Nome").0, Some(2));
        assert_eq!(p("Show T2 E8 - Nome").4, "Nome");
        assert_eq!(p("Show S01E01E02").1, vec![1, 2]);
        assert_eq!(p("Show S01E01-02 Title").1, vec![1, 2]);
        assert_eq!(p("Show S01E01-E02").1, vec![1, 2]);
        assert_eq!(p("Show 1x02 Title").0, Some(1));
        assert_eq!(p("Show 1x02 Title").1, vec![2]);
        assert_eq!(
            p("Show - Temporada 1 Episódio 2 - Título"),
            (Some(1), vec![2], None, s("Show"), s("Título"))
        );
        assert_eq!(p("Show Season 3, Episode 10").1, vec![10]);
        assert_eq!(p("Show Ep 05").1, vec![5]);
        assert_eq!(p("Show Ep 05").0, None);
        assert_eq!(
            p("[Grupo] Naruto - 125 [720p]"),
            (None, vec![], Some(125), s("Naruto"), s(""))
        );
        assert_eq!(p("01 - Pilot").2, Some(1));
        assert_eq!(p("01 - Pilot").4, "Pilot");
        assert_eq!(p("Show S01E05-720p").1, vec![5]);
        assert_eq!(p("Pokemon.S01E1000").1, vec![1000]);
        assert_eq!(p("Show.S2015E05").0, Some(2015));
        assert_eq!(p("Peaky Blinders 6ª Temporada Ep. 2").0, Some(6));
        assert_eq!(p("Peaky Blinders 6ª Temporada Ep. 2").3, "Peaky Blinders");
        assert_eq!(p("Chernobyl.Part.2").3, "Chernobyl");
        assert_eq!(p("House.of.the.Dragon.S02E01.HDR.2160p").4, "");
        assert_eq!(parse("Show 205", "Season 2").episodes, vec![5]);
        assert_eq!(p("Show Temp. 2 Ep. 3").1, vec![3]);
        let g = parse("The Daily Show 2024.03.15 Guest", "");
        assert_eq!(g.date.as_deref(), Some("2024-03-15"));
        // Season from the folder.
        let g = parse("05 - Title", "Season 2");
        assert_eq!((g.season, g.episodes, g.number), (Some(2), vec![5], None));
        assert_eq!(parse("Ep 3", "Temporada 04").season, Some(4));
        assert_eq!(parse("Ep 3", "Specials").season, Some(0));
        // No numbers at all: the whole name may be the title.
        assert_eq!(p("Pilot").4, "Pilot");
    }

    fn ep(season: u16, number: u16, name: &str) -> Episode {
        Episode {
            season,
            number,
            name: name.into(),
            names: vec![],
            air_date: None,
            overview: String::new(),
            still_path: None,
        }
    }

    fn show() -> Show {
        Show {
            id: 1,
            name: "Breaking Bad".into(),
            original_name: "Breaking Bad".into(),
            year: Some(2008),
            overview: String::new(),
            poster_path: None,
            episodes: vec![
                ep(1, 1, "Pilot"),
                ep(1, 2, "Cat's in the Bag..."),
                ep(1, 3, "...And the Bag's in the River"),
                ep(2, 1, "Seven Thirty-Seven"),
                ep(2, 8, "Better Call Saul"),
                ep(0, 1, "Good Cop Bad Cop"),
            ],
        }
    }

    #[test]
    fn matching() {
        let s = show();
        let f = |name: &str| find(&s, &parse(name, ""));
        // Numbers alone.
        assert_eq!(f("BB S02E08").choice, vec![(2, 8)]);
        // Numbers and title agree.
        assert_eq!(
            f("BB S01E03 And the Bags in the River").choice,
            vec![(1, 3)]
        );
        // A misspelled title still agrees.
        assert_eq!(f("BB S01E02 Cats in teh Bag").choice, vec![(1, 2)]);
        // The title names another episode: the user decides.
        let x = f("BB S02E08 And the Bags in the River");
        assert!(x.conflict && x.choice.is_empty());
        assert_eq!(x.alternatives[0], vec![(1, 3)]);
        assert_eq!(x.alternatives[1], vec![(2, 8)]);
        // Title alone.
        assert_eq!(f("Breaking Bad - Better Call Saul").choice, vec![(2, 8)]);
        assert_eq!(f("Pilot").choice, vec![(1, 1)]);
        // Junk after the numbers does not get in the way.
        assert_eq!(f("BB S01E01 PROPER GRP").choice, vec![(1, 1)]);
        // Numbers the show does not have, no title: nothing.
        let x = f("BB S05E20");
        assert!(x.choice.is_empty() && x.alternatives.is_empty());
        // Absolute numbering across seasons: 4th regular episode.
        assert_eq!(f("BB Ep 4").choice, vec![(2, 1)]);
        // "201": season 2 episode 1, or the 201st (does not exist).
        assert_eq!(f("BB 201").choice, vec![(2, 1)]);
        // Multi-episode file.
        assert_eq!(f("BB S01E01E02").choice, vec![(1, 1), (1, 2)]);
        // Specials.
        assert_eq!(f("BB S00E01").choice, vec![(0, 1)]);
    }

    #[test]
    fn show_from_api() {
        let d: TvDetails = serde_json::from_str(
            r#"{"id":5,"name":"Série","original_name":"Show","original_language":"en","first_air_date":"2010-01-01",
                "seasons":[{"season_number":1}],
                "season/1":{"episodes":[{"season_number":1,"episode_number":1,"name":"Episódio 1"},
                                        {"season_number":1,"episode_number":2,"name":"Título"}]}}"#,
        )
        .unwrap();
        let pt = d.appended_episodes();
        let en: Vec<ApiEpisode> = serde_json::from_str(
            r#"[{"season_number":1,"episode_number":1,"name":"Pilot"},
                {"season_number":1,"episode_number":2,"name":"Title"}]"#,
        )
        .unwrap();
        let s = build_show(&d, &[pt, en]);
        assert_eq!(s.year, Some(2010));
        assert_eq!(s.episodes[0].name, "Pilot");
        assert_eq!(s.episodes[1].name, "Título");
        assert_eq!(s.episodes[1].names, vec!["Title".to_string()]);
    }

    #[test]
    fn names() {
        let m = EpisodeMatch {
            id: 1396,
            show: "Breaking Bad".into(),
            original_show: String::new(),
            year: Some(2008),
            season: 1,
            episodes: vec![1, 2],
            titles: vec!["Piloto".into(), "O Gato: Parte 2".into()],
        };
        assert_eq!(
            episode_name(
                "{série} ({ano}) - S{temporada}E{episódio} - {título}",
                &m,
                " - "
            ),
            "Breaking Bad (2008) - S01E01-E02 - Piloto & O Gato - Parte 2"
        );
        let m = EpisodeMatch {
            titles: vec![],
            year: None,
            episodes: vec![3],
            ..m
        };
        assert_eq!(
            episode_name("{show} ({year}) - S{season}E{episode} - {title}", &m, " - "),
            "Breaking Bad - S01E03"
        );
    }

    #[test]
    fn folders() {
        let m = EpisodeMatch {
            id: 1396,
            show: "Breaking Bad: A Química do Mal".into(),
            original_show: "Breaking Bad".into(),
            year: Some(2008),
            season: 1,
            episodes: vec![1],
            titles: vec![],
        };
        let root = |d: &str| library_root(Path::new(d), &m);
        assert_eq!(root("/tv/Breaking Bad/Season 1"), Path::new("/tv"));
        assert_eq!(root("/tv/Breaking.Bad.2008"), Path::new("/tv"));
        assert_eq!(
            root("/tv/Breaking Bad (2008) [tmdbid-1396]/Season 01"),
            Path::new("/tv")
        );
        assert_eq!(root("/dl/Season 1"), Path::new("/dl"));
        assert_eq!(root("/dl/stuff"), Path::new("/dl/stuff"));
        assert_eq!(show_dir(Path::new("/dl/Season 1"), &m), None);
        assert_eq!(season_folder(0), "Season 00");
    }

    #[test]
    fn nfo_files() {
        let d: ShowDetails = serde_json::from_str(
            r#"{"id":1396,"name":"Breaking Bad & Co","original_name":"Breaking Bad","first_air_date":"2008-01-20",
                "status":"Ended","episode_run_time":[47],"genres":[{"name":"Drama"}],"networks":[{"name":"AMC"}],
                "external_ids":{"imdb_id":"tt0903747","tvdb_id":81189},
                "content_ratings":{"results":[{"iso_3166_1":"BR","rating":"16"}]},
                "aggregate_credits":{"cast":[{"name":"Bryan Cranston","roles":[{"character":"Walter White"}]}]}}"#,
        )
        .unwrap();
        let x = show_nfo(&d, "pt-BR");
        for want in [
            "<tvshow>",
            "<title>Breaking Bad &amp; Co</title>",
            "<year>2008</year>",
            "<status>Ended</status>",
            "<mpaa>BR-16</mpaa>",
            "<studio>AMC</studio>",
            "<runtime>47</runtime>",
            "<uniqueid type=\"tvdb\">81189</uniqueid>",
            "<episodeguide>{&quot;tmdb&quot;:&quot;1396&quot;,&quot;imdb&quot;:&quot;tt0903747&quot;,&quot;tvdb&quot;:&quot;81189&quot;}</episodeguide>",
            "<role>Walter White</role>",
        ] {
            assert!(x.contains(want), "missing {want} in\n{x}");
        }
        let e: EpisodeDetails = serde_json::from_str(
            r#"{"id":62085,"season_number":1,"episode_number":1,"name":"Episódio 1","air_date":"2008-01-20",
                "crew":[{"name":"Vince Gilligan","job":"Director","department":"Directing"}],
                "guest_stars":[{"name":"X","character":"Y"}]}"#,
        )
        .unwrap();
        let x = episode_nfo("Breaking Bad", &[&e, &e], &["Piloto".into()]);
        assert_eq!(x.matches("<episodedetails>").count(), 2);
        assert!(x.contains("<title>Piloto</title>"));
        assert!(x.contains("<title>Episódio 1</title>"));
        assert!(x.contains("<director>Vince Gilligan</director>"));
        assert!(x.contains("<type>GuestStar</type>"));
        assert_eq!(season_poster(2), "season02-poster.jpg");
    }

    #[test]
    fn queries() {
        assert!(matches!(show_query("1396"), ShowQuery::Id(1396)));
        assert!(matches!(
            show_query("https://www.themoviedb.org/tv/1396-breaking-bad?language=pt-BR"),
            ShowQuery::Id(1396)
        ));
        match show_query("Doctor Who (2005)") {
            ShowQuery::Name(g) => {
                assert_eq!((g.title.as_str(), g.year), ("Doctor Who", Some(2005)))
            }
            ShowQuery::Id(_) => panic!(),
        }
        let items = vec![
            ("Breaking.Bad.S01E01".to_string(), "/tv/x/Season 1".into()),
            ("Breaking Bad S01E02".to_string(), "/tv/x/Season 1".into()),
        ];
        assert_eq!(guess_show(&items).title, "Breaking Bad");
        let items = vec![("01 - Pilot".to_string(), "/tv/Lost (2004)/Season 1".into())];
        let g = guess_show(&items);
        assert_eq!((g.title.as_str(), g.year), ("Lost", Some(2004)));
    }
}
