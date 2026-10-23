//! TV shows from TMDB, without the network part: reading season and
//! episode numbers from file names, matching files to the episodes of a
//! show and building the new name.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::text;
use crate::tmdb::{self, Guess, SearchResult, TAGS, clean, safe_title, similarity, year_of};

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

/// S01E02, s1e2, S01 E02, T1E2, S01E01E02, S01E01-02.
static SXE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[st](\d{1,2}) ?-? ?ep? ?(\d{1,3})((?: ?-? ?e\d{1,3}\b|-\d{1,3}\b)*)")
        .unwrap()
});
/// 1x02.
static NXN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(\d{1,2})x(\d{2,3})\b((?:-\d{2,3}\b)*)").unwrap());
/// Temporada 1 Episódio 2, Season 1, Episode 2, Temp. 1 Ep. 2.
static LONG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:season|temporada|temp|saison|staffel)\.? ?(\d{1,2})\W{1,4}(?:episode|episódio|episodio|épisode|ep|e|capítulo|capitulo|cap|folge)\.? ?(\d{1,3})\b",
    )
    .unwrap()
});
/// 2024.03.15 (dots are spaces by then).
static DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b((?:19|20)\d{2})[ -](\d{2})[ -](\d{2})\b").unwrap());
/// Episódio 5, Ep 05, E05 (season from the folder).
static EP_ONLY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:(?:episode|episódio|episodio|épisode|ep|capítulo|capitulo|cap|chapter|folge)\.? ?|e)(\d{1,4})\b",
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
    } else if let Some(c) = LONG.captures(&spaced) {
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
        match g.number {
            // `Season 2/05.mkv`: episode 5 of season 2.
            Some(n) if season.is_some() && n < 1000 => {
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

/// The text before the first release tag that has something before it.
fn before_tags(s: &str) -> String {
    let cut = TAGS
        .find_iter(s)
        .map(|m| m.start())
        .find(|&i| !clean(&s[..i]).is_empty())
        .unwrap_or(s.len());
    s[..cut].to_string()
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
            push(absolute(n).map(|k| vec![k]));
            push(run_together);
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
