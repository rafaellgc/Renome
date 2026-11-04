//! AniList client: manga series by title, with no key. Like MangaDex,
//! AniList knows series, not a publisher's volumes, so only the series is
//! looked up: the volume or chapter stays the one in the file. Its terms
//! allow free non-commercial use without mass collection, and at most 90
//! requests a minute (30 while the service is degraded), so requests are
//! paced and answers kept only for a while.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use renome_core::comics::Volume;
use serde_json::{Value, json};

const API: &str = "https://graphql.anilist.co";

/// Tests point this at a local server with `RENOME_ANILIST_API`.
fn api_base() -> String {
    std::env::var("RENOME_ANILIST_API").unwrap_or_else(|_| API.into())
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
        "alOffline".into()
    } else if e.is_timeout() {
        "alTimeout".into()
    } else {
        format!("alNetwork: {}", e.without_url())
    }
}

/// Words without accents or case, to compare titles.
pub(crate) fn fold(s: &str) -> String {
    renome_core::text::strip_accents(&s.to_lowercase())
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The title to name by: the one the file already has, when it is one of
/// the series' titles (a translation the reader knows), else the English
/// one, else the main one (often in romaji).
pub(crate) fn pick_title(query: &str, main: &[&str], others: &[&str]) -> String {
    let q = fold(query);
    main.iter()
        .chain(others)
        .find(|t| !q.is_empty() && fold(t) == q)
        .or_else(|| main.iter().find(|t| !t.trim().is_empty()))
        .map(|t| t.trim().to_string())
        .unwrap_or_default()
}

/// The search, `DATES` standing for the year's filter (AniList refuses an
/// empty one).
const MANGA: &str = "query ($search: String, $from: FuzzyDateInt, $to: FuzzyDateInt) {
  Page(perPage: 20) {
    media(search: $search, type: MANGA, format_not: NOVEL DATES, sort: SEARCH_MATCH) {
      id
      title { romaji english }
      synonyms
      startDate { year }
      volumes
      countryOfOrigin
      format
      coverImage { medium }
      staff(perPage: 8) { edges { role node { name { full } } } }
    }
  }
}";

fn volume(m: &Value, query: &str) -> Volume {
    fn s(v: &Value) -> &str {
        v.as_str().unwrap_or_default()
    }
    let others: Vec<&str> = m["synonyms"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    // Who wrote it: the story's author (or the author of story and art).
    let writer = m["staff"]["edges"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| {
            let role = s(&e["role"]).to_lowercase();
            role == "story" || role == "story & art" || role == "original creator"
        })
        .filter_map(|e| e["node"]["name"]["full"].as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Volume {
        id: m["id"].as_u64().unwrap_or_default().to_string(),
        name: pick_title(
            query,
            &[s(&m["title"]["english"]), s(&m["title"]["romaji"])],
            &others,
        ),
        start_year: m["startDate"]["year"]
            .as_u64()
            .and_then(|y| u16::try_from(y).ok()),
        count: m["volumes"]
            .as_u64()
            .and_then(|c| u32::try_from(c).ok())
            .unwrap_or_default(),
        cover: s(&m["coverImage"]["medium"]).to_string(),
        country: s(&m["countryOfOrigin"]).to_lowercase(),
        format: s(&m["format"]).to_lowercase().replace('_', " "),
        writer,
        ..Default::default()
    }
}

// ---------------------------------------------------------------- anime
//
// AniList keeps each season of a show (each cour) as an entry of its own,
// linked to the next by a SEQUEL relation. A show is built from its first
// season and those after it, in order: season 1, 2… Episode titles come
// from the streaming services' list AniList keeps (`Episode 3 - Title`),
// when there is one; air dates from its airing schedule.

/// Seasons followed at most.
const MAX_SEASONS: usize = 12;

const MEDIA_FIELDS: &str = "id title { romaji english native } synonyms format episodes status
  startDate { year month day } description(asHtml: false) genres countryOfOrigin
  coverImage { medium } nextAiringEpisode { episode } popularity
  relations { edges { relationType node { id type format } } }";

static ANIME_SEARCH: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!(
        "query ($search: String) {{ Page(perPage: 20) {{ media(search: $search, type: ANIME,
          format_in: [TV, TV_SHORT, ONA, OVA], sort: SEARCH_MATCH) {{ {MEDIA_FIELDS} }} }} }}"
    )
});

static ANIME_ONE: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!(
        "query ($id: Int) {{ Media(id: $id, type: ANIME) {{ {MEDIA_FIELDS}
          streamingEpisodes {{ title }} airingSchedule(perPage: 50) {{ nodes {{ episode airingAt }} }} }} }}"
    )
});

/// Formats that are a season of a show (not a movie or a special).
fn is_season(format: &str) -> bool {
    matches!(format, "TV" | "TV_SHORT" | "ONA")
}

/// The entry a relation of `kind` leads to that is a season.
fn related(m: &Value, kind: &str) -> Option<u64> {
    m["relations"]["edges"].as_array()?.iter().find(|e| {
        e["relationType"] == kind
            && e["node"]["type"] == "ANIME"
            && is_season(e["node"]["format"].as_str().unwrap_or_default())
    })?["node"]["id"]
        .as_u64()
}

fn sequel(m: &Value) -> Option<u64> {
    related(m, "SEQUEL")
}

/// A season after another one.
fn later_season(m: &Value) -> bool {
    is_season(m["format"].as_str().unwrap_or_default()) && related(m, "PREQUEL").is_some()
}

/// An AniList id typed: a number, or a link to an anime.
fn anime_id(typed: &str) -> Option<u64> {
    if let Some(rest) = typed.split("anilist.co/anime/").nth(1) {
        return rest
            .split(|c: char| !c.is_ascii_digit())
            .next()?
            .parse()
            .ok();
    }
    typed.parse().ok()
}

fn titles(m: &Value) -> (Vec<String>, Vec<String>) {
    let s = |v: &Value| v.as_str().unwrap_or_default().trim().to_string();
    let main = vec![s(&m["title"]["english"]), s(&m["title"]["romaji"])];
    let mut others: Vec<String> = m["synonyms"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    others.push(s(&m["title"]["native"]));
    (main, others)
}

/// A description without its markup (`<br>`, `<i>`).
fn plain(text: &str) -> String {
    let breaks = regex::Regex::new(r"(?i)<br\s*/?>").unwrap();
    let tags = regex::Regex::new(r"<[^>]*>").unwrap();
    let text = breaks.replace_all(text, " ");
    renome_core::text::collapse_spaces(&tags.replace_all(&text, ""))
}

fn date_of(d: &Value) -> Option<String> {
    let y = d["year"].as_u64()?;
    match (d["month"].as_u64(), d["day"].as_u64()) {
        (Some(m), Some(day)) => Some(format!("{y:04}-{m:02}-{day:02}")),
        _ => None,
    }
}

fn anime_result(m: &Value, query: &str) -> renome_core::tmdb::SearchResult {
    let (main, others) = titles(m);
    let main: Vec<&str> = main.iter().map(String::as_str).collect();
    let others: Vec<&str> = others.iter().map(String::as_str).collect();
    // Compared by the closest of all its titles (a synonym may be the
    // name the files use), shown with that one when it is the same.
    let closest = main
        .iter()
        .chain(&others)
        .filter(|t| !t.is_empty())
        .max_by(|a, b| {
            renome_core::tmdb::similarity(query, a)
                .total_cmp(&renome_core::tmdb::similarity(query, b))
        })
        .copied()
        .unwrap_or_default();
    // A show itself before its side stories (shorts, web extras).
    let side = m["format"].as_str() != Some("TV");
    renome_core::tmdb::SearchResult {
        id: m["id"].as_u64().unwrap_or_default(),
        title: pick_title(query, &main, &others),
        original_title: closest.to_string(),
        release_date: m["startDate"]["year"].as_u64().map(|y| format!("{y:04}")),
        overview: Some(plain(m["description"].as_str().unwrap_or_default())),
        poster_path: m["coverImage"]["medium"].as_str().map(str::to_string),
        popularity: if side {
            0.0
        } else {
            m["popularity"].as_f64().unwrap_or(0.0)
        },
    }
}

/// The number of episodes of a season: all of them, or those aired so far.
fn episode_count(m: &Value) -> u16 {
    let aired = m["nextAiringEpisode"]["episode"]
        .as_u64()
        .map(|n| n.saturating_sub(1));
    let total = m["episodes"].as_u64();
    let n = match m["status"].as_str() {
        Some("RELEASING") => aired.or(total),
        _ => total.or(aired),
    };
    u16::try_from(n.unwrap_or(0)).unwrap_or(u16::MAX)
}

/// Episode titles from the streaming list (`Episode 3 - Title`), by number.
fn episode_titles(m: &Value) -> HashMap<u16, String> {
    let re = regex::Regex::new(r"(?i)^\s*(?:episode|ep\.?)\s*(\d+)\s*[-–:]\s*(.+?)\s*$").unwrap();
    m["streamingEpisodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let c = re.captures(e["title"].as_str()?)?;
            Some((c[1].parse().ok()?, c[2].to_string()))
        })
        .collect()
}

fn air_dates(m: &Value) -> HashMap<u16, String> {
    m["airingSchedule"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| {
            let ep = u16::try_from(n["episode"].as_u64()?).ok()?;
            let at = chrono::DateTime::from_timestamp(n["airingAt"].as_i64()?, 0)?;
            Some((ep, at.format("%Y-%m-%d").to_string()))
        })
        .collect()
}

/// The show of a list of seasons, the first one first.
fn anime_show(seasons: &[Value], typed: &str) -> renome_core::tv::Show {
    let first = &seasons[0];
    let (main, others) = titles(first);
    let main_s: Vec<&str> = main.iter().map(String::as_str).collect();
    let others_s: Vec<&str> = others.iter().map(String::as_str).collect();
    // What was searched for, without the year after it (`Name (2020)`).
    let typed = regex::Regex::new(r"\s*\(\d{4}\)\s*$")
        .unwrap()
        .replace(typed.trim(), "");
    let name = pick_title(&typed, &main_s, &others_s);
    let mut episodes = Vec::new();
    for (i, s) in seasons.iter().enumerate() {
        let season = u16::try_from(i + 1).unwrap_or(u16::MAX);
        let names = episode_titles(s);
        let dates = air_dates(s);
        for n in 1..=episode_count(s) {
            episodes.push(renome_core::tv::Episode {
                season,
                number: n,
                name: names.get(&n).cloned().unwrap_or_default(),
                names: Vec::new(),
                air_date: dates
                    .get(&n)
                    .cloned()
                    .or_else(|| (n == 1).then(|| date_of(&s["startDate"])).flatten()),
                overview: String::new(),
                still_path: None,
            });
        }
    }
    let mut aliases: Vec<String> = Vec::new();
    for s in seasons {
        let (m, o) = titles(s);
        for t in m.into_iter().chain(o) {
            if !t.is_empty() && t != name && !aliases.contains(&t) {
                aliases.push(t);
            }
        }
    }
    let mut extra = renome_core::fields::Extra::new();
    let genres: Vec<&str> = first["genres"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    renome_core::fields::add(
        &mut extra,
        "genre",
        genres.first().copied().unwrap_or_default(),
    );
    renome_core::fields::add(
        &mut extra,
        "genres",
        &genres
            .iter()
            .take(3)
            .copied()
            .collect::<Vec<_>>()
            .join(", "),
    );
    renome_core::fields::add(
        &mut extra,
        "country",
        &first["countryOfOrigin"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase(),
    );
    renome_core::tv::Show {
        source: "anilist".into(),
        id: first["id"].as_u64().unwrap_or_default(),
        name,
        original_name: first["title"]["native"]
            .as_str()
            .or(first["title"]["romaji"].as_str())
            .unwrap_or_default()
            .to_string(),
        year: first["startDate"]["year"]
            .as_u64()
            .and_then(|y| u16::try_from(y).ok()),
        overview: plain(first["description"].as_str().unwrap_or_default()),
        poster_path: first["coverImage"]["medium"].as_str().map(str::to_string),
        episodes,
        aliases,
        extra,
    }
}

impl Client {
    /// Manga with a title, from `year` when given, the closest first.
    pub async fn search(&self, query: &str, year: Option<u16>) -> Result<Vec<Volume>, String> {
        let key = format!("{year:?}:{query}");
        if let Some(v) = self.cache.lock().await.get(&key) {
            return Ok(v.clone());
        }
        let question = format!("manga {year:?} {query}");
        let v = match crate::kept::<Value>("anilist", &question) {
            Some(v) => v,
            None => {
                // Dates are numbers like 20200000 (a year with no month).
                let vars = match year {
                    Some(y) => json!({
                        "search": query,
                        "from": u32::from(y) * 10000 - 1,
                        "to": (u32::from(y) + 1) * 10000,
                    }),
                    None => json!({ "search": query }),
                };
                let dates = match year {
                    Some(_) => ", startDate_greater: $from, startDate_lesser: $to",
                    None => "",
                };
                let gql = MANGA.replace(" DATES", dates);
                let gql = match year {
                    Some(_) => gql,
                    None => gql.replace(
                        "($search: String, $from: FuzzyDateInt, $to: FuzzyDateInt)",
                        "($search: String)",
                    ),
                };
                self.ask(&gql, vars, &question).await?
            }
        };
        let found: Vec<Volume> = v["data"]["Page"]["media"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|m| volume(m, query))
            .collect();
        let ranked = renome_core::comics::rank_volumes(query, found, "");
        self.cache.lock().await.insert(key, ranked.clone());
        Ok(ranked)
    }

    /// A question kept, else asked.
    async fn kept_or_ask(&self, query: &str, vars: Value, question: &str) -> Result<Value, String> {
        match crate::kept::<Value>("anilist", question) {
            Some(v) => Ok(v),
            None => self.ask(query, vars, question).await,
        }
    }

    /// Anime shows by the name typed (or an AniList id or link), or by the
    /// files' names when nothing was typed. Later seasons (entries with a
    /// season before them) are left out: a show is looked up from its
    /// first season.
    pub async fn anime_search(
        &self,
        typed: &str,
        items: &[crate::tmdb::TvItem],
    ) -> Result<crate::tmdb::ShowSearch, String> {
        let typed = typed.trim();
        let id = anime_id(typed);
        let guess = if typed.is_empty() {
            let names: Vec<(String, std::path::PathBuf)> = items
                .iter()
                .map(|i| (i.stem().to_string(), i.folder()))
                .collect();
            renome_core::tv::guess_show(&names)
        } else {
            renome_core::tmdb::Guess {
                title: typed.to_string(),
                year: None,
                id: None,
            }
        };
        if id.is_none() && guess.title.is_empty() {
            return Err("tvNoShow".into());
        }
        let media: Vec<Value> = match id {
            Some(id) => {
                let v = self
                    .kept_or_ask(&ANIME_ONE, json!({ "id": id }), &format!("anime one {id}"))
                    .await?;
                v["data"]["Media"]
                    .as_object()
                    .map(|_| v["data"]["Media"].clone())
                    .into_iter()
                    .collect()
            }
            None => {
                let v = self
                    .kept_or_ask(
                        &ANIME_SEARCH,
                        json!({ "search": guess.title }),
                        &format!("anime search {}", guess.title),
                    )
                    .await?;
                let all = v["data"]["Page"]["media"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let first: Vec<Value> = all.iter().filter(|m| !later_season(m)).cloned().collect();
                // Only later seasons found: offered as they are.
                if first.is_empty() { all } else { first }
            }
        };
        let results: Vec<renome_core::tmdb::SearchResult> = media
            .iter()
            .map(|m| anime_result(m, &guess.title))
            .collect();
        let (candidates, auto) = match id {
            Some(_) => (renome_core::tmdb::rank(&guess, results).0, true),
            None => renome_core::tmdb::rank(&guess, results),
        };
        let query = match (id, guess.year) {
            (Some(id), _) => id.to_string(),
            (None, Some(y)) => format!("{} ({y})", guess.title),
            (None, None) => guess.title.clone(),
        };
        Ok(crate::tmdb::ShowSearch {
            query,
            candidates,
            auto,
        })
    }

    /// A show from its first season and the seasons after it (each one an
    /// entry of its own on AniList), with each file's episode.
    pub async fn anime_lookup(
        &self,
        id: u64,
        items: &[crate::tmdb::TvItem],
        typed: &str,
    ) -> Result<crate::tmdb::TvLookup, String> {
        let mut seasons: Vec<Value> = Vec::new();
        let mut next = Some(id);
        while let Some(id) = next.take() {
            if seasons.len() >= MAX_SEASONS || seasons.iter().any(|s| s["id"].as_u64() == Some(id))
            {
                break;
            }
            let v = self
                .kept_or_ask(&ANIME_ONE, json!({ "id": id }), &format!("anime one {id}"))
                .await?;
            let m = v["data"]["Media"].clone();
            if !m.is_object() {
                return Err("tvNoShow".into());
            }
            next = sequel(&m);
            seasons.push(m);
        }
        let show = anime_show(&seasons, typed);
        Ok(crate::tmdb::files_of(show, items, typed))
    }

    /// Asks AniList a question, two seconds apart at least, keeping the
    /// answer.
    pub(crate) async fn ask(
        &self,
        query: &str,
        vars: Value,
        question: &str,
    ) -> Result<Value, String> {
        let mut last = self.last.lock().await;
        if let Some(t) = *last {
            tokio::time::sleep(Duration::from_millis(2000).saturating_sub(t.elapsed())).await;
        }
        let res = self
            .http
            .post(api_base())
            .json(&json!({ "query": query, "variables": vars }))
            .send()
            .await;
        *last = Some(Instant::now());
        drop(last);
        let res = res.map_err(net_error)?;
        match res.status().as_u16() {
            200 => {}
            429 => return Err("alBusy".into()),
            code => return Err(format!("alNetwork: HTTP {code}")),
        }
        let body = res.bytes().await.map_err(net_error)?;
        let v: Value = serde_json::from_slice(&body).map_err(|e| format!("alNetwork: {e}"))?;
        if let Some(e) = v["errors"].as_array().and_then(|e| e.first()) {
            return Err(format!(
                "alNetwork: {}",
                e["message"].as_str().unwrap_or("?")
            ));
        }
        crate::keep("anilist", question, &body).map_err(|e| format!("alNetwork: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_files_title_is_kept_when_it_is_one_of_the_series() {
        let m: Value = serde_json::from_str(
            r#"{"id":7,"title":{"romaji":"Hoshi no Tabi","english":"Star Journey"},
            "synonyms":["Jornada das Estrelas"],"startDate":{"year":2020},"volumes":12,
            "countryOfOrigin":"JP","format":"MANGA","coverImage":{"medium":"https://x/c.jpg"},
            "staff":{"edges":[{"role":"Story","node":{"name":{"full":"Ana Lima"}}},
            {"role":"Art","node":{"name":{"full":"Rui Sá"}}}]}}"#,
        )
        .unwrap();
        let v = volume(&m, "jornada das estrelas");
        assert_eq!(v.name, "Jornada das Estrelas");
        assert_eq!(
            (v.id.as_str(), v.start_year, v.count),
            ("7", Some(2020), 12)
        );
        assert_eq!(v.writer, "Ana Lima");
        assert_eq!(volume(&m, "Hoshi no Tabi").name, "Hoshi no Tabi");
        assert_eq!(volume(&m, "outra coisa").name, "Star Journey");
    }

    #[test]
    fn seasons_make_one_show() {
        let s1: Value = serde_json::from_str(
            r#"{"id":10,"title":{"romaji":"Hoshi no Tabi","english":"Star Journey","native":"星の旅"},
            "synonyms":["Jornada das Estrelas"],"format":"TV","episodes":3,"status":"FINISHED",
            "startDate":{"year":2023,"month":9,"day":29},"description":"Uma <i>viagem</i>.<br>Longa.",
            "genres":["Adventure","Drama"],"countryOfOrigin":"JP","coverImage":{"medium":"https://x/c.jpg"},
            "streamingEpisodes":[{"title":"Episode 2 - O Rio"},{"title":"Episode 1 - A Partida"}],
            "airingSchedule":{"nodes":[{"episode":2,"airingAt":1696600800}]},
            "relations":{"edges":[{"relationType":"SEQUEL","node":{"id":11,"type":"ANIME","format":"TV"}},
            {"relationType":"ADAPTATION","node":{"id":5,"type":"MANGA","format":"MANGA"}}]}}"#,
        )
        .unwrap();
        let s2: Value = serde_json::from_str(
            r#"{"id":11,"title":{"romaji":"Hoshi no Tabi 2nd Season","english":null,"native":null},
            "format":"TV","episodes":null,"status":"RELEASING","nextAiringEpisode":{"episode":3},
            "startDate":{"year":2026},
            "relations":{"edges":[{"relationType":"PREQUEL","node":{"id":10,"type":"ANIME","format":"TV"}}]}}"#,
        )
        .unwrap();
        assert_eq!(sequel(&s1), Some(11));
        assert!(!later_season(&s1) && later_season(&s2));
        let show = anime_show(&[s1, s2], "Jornada das Estrelas (2023)");
        assert_eq!(
            (show.source.as_str(), show.id, show.year),
            ("anilist", 10, Some(2023))
        );
        assert_eq!(show.name, "Jornada das Estrelas");
        assert_eq!(show.original_name, "星の旅");
        assert_eq!(show.overview, "Uma viagem. Longa.");
        let eps: Vec<(u16, u16, &str)> = show
            .episodes
            .iter()
            .map(|e| (e.season, e.number, e.name.as_str()))
            .collect();
        assert_eq!(
            eps,
            [
                (1, 1, "A Partida"),
                (1, 2, "O Rio"),
                (1, 3, ""),
                (2, 1, ""),
                (2, 2, "")
            ]
        );
        assert_eq!(show.episodes[0].air_date.as_deref(), Some("2023-09-29"));
        assert_eq!(show.episodes[1].air_date.as_deref(), Some("2023-10-06"));
        assert!(
            show.aliases
                .contains(&"Hoshi no Tabi 2nd Season".to_string())
        );
        assert_eq!(anime_id("https://anilist.co/anime/154587/x"), Some(154587));
        assert_eq!(anime_id("Nome"), None);
    }
}
