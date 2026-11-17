//! The lookup rules on the command line: the catalogues searched as the
//! window's search buttons do, the doubtful files asked about in the
//! terminal (or left, or forced with `--pick`), and what runs after the
//! renames.

use std::collections::HashMap;
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use renome_core::tmdb::{Candidate, Guess, MovieMatch};
use renome_core::{Preview, PreviewItem};
use renome_services::{Host, tmdb};
use serde_json::Value;

use crate::out;
use crate::text::{lang, t};

/// How doubtful files are settled (SPEC §8.6).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Choose {
    Ask,
    Auto,
    Skip,
}

/// What the person asked for the lookups.
pub struct Ask {
    pub choose: Choose,
    /// `--pick PATH=ID`, by full path.
    pub picks: HashMap<PathBuf, String>,
}

/// The command line as the host of the shared work: progress on stderr.
pub struct Cli;

impl Host for Cli {
    fn report(&self, event: &str, payload: Value) {
        if out::quiet() {
            return;
        }
        let (Some(done), Some(total)) = (
            payload.get("done").and_then(Value::as_u64),
            payload.get("total").and_then(Value::as_u64),
        ) else {
            return;
        };
        let kind = payload
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let key = match (event, kind) {
            ("tmdb-progress", "save" | "saveTv") => "sidecarsWriting",
            ("tmdb-progress", _) => "tmdbSearching",
            ("mb-progress", "listen") => "mbListening",
            ("mb-progress", "tags") => "tagsWriting",
            ("mb-progress", "save") => "coversWriting",
            ("mb-progress", _) => "mbSearching",
            ("book-progress", _) => "olSearching",
            ("subs-progress", _) => "subsSearching",
            ("inside-progress", _) => "insideWriting",
            ("subtools-progress", _) => "subToolsWorking",
            _ => "",
        };
        let what = crate::text::t(key, &[("done", &done), ("total", &total)]);
        let what = if what.contains(&total.to_string()) {
            what
        } else {
            format!("{what} {done}/{total}")
        };
        eprint!("\r{what}   ");
        if done >= total {
            eprintln!();
        }
        let _ = std::io::stderr().flush();
    }
    fn stop_signal(&self) -> Arc<dyn Fn() -> bool + Send + Sync> {
        Arc::new(|| false)
    }
    fn sender(&self) -> renome_services::Sender {
        Arc::new(|event, payload| Cli.report(event, payload))
    }
}

/// The TMDB key and the language of its titles, from the options.
fn tmdb_access(settings: Option<&Value>) -> Result<(String, String), String> {
    let secrets = renome_services::load_secrets();
    let key = secrets
        .get("secrets")
        .and_then(|s| s.get("tmdbKey"))
        .or_else(|| settings.and_then(|s| s.get("tmdbKey")))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if key.is_empty() {
        return Err(t("tmdbNeedsKey", &[]));
    }
    let tmdb_lang = settings
        .and_then(|s| s.get("tmdbLang")?.as_str())
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| match lang() {
            "pt" => "pt-BR".into(),
            "es" => "es-ES".into(),
            _ => "en-US".into(),
        });
    Ok((key, tmdb_lang))
}

/// What the movie rule found and chose, kept for after the renames.
pub struct Movies {
    pub key: String,
    pub lang: String,
    pub chosen: HashMap<PathBuf, MovieMatch>,
}

/// Looks up the movies of the list (videos, their subtitles and folders,
/// not extras or a release's adverts) and settles each one.
pub async fn movies(
    preview: &Preview,
    settings: Option<&Value>,
    ask: &Ask,
) -> Result<Movies, String> {
    let (key, tmdb_lang) = tmdb_access(settings)?;
    let client = tmdb::Client::new(&key, &tmdb_lang);
    let items: Vec<&PreviewItem> = preview
        .items
        .iter()
        .filter(|i| {
            let ext = Path::new(&i.original)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            (i.is_dir || renome_core::tmdb::is_screen(&ext)) && i.extra.is_none() && !i.release_junk
        })
        .collect();
    let mut chosen: HashMap<PathBuf, Candidate> = HashMap::new();
    // Forced first: no search for them.
    for i in &items {
        if let Some(pick) = ask.picks.get(&i.path) {
            let found = typed_search(&client, pick).await?;
            match found.into_iter().next() {
                Some(c) => {
                    chosen.insert(i.path.clone(), c);
                }
                None => return Err(t("linkNotFound", &[])),
            }
        }
    }
    let to_search: Vec<tmdb::LookupItem> = items
        .iter()
        .filter(|i| !chosen.contains_key(&i.path))
        .map(|i| {
            let name = i.lookup_name.clone().unwrap_or_else(|| i.original.clone());
            let name = if i.is_dir {
                name
            } else {
                Path::new(&name)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or(name)
            };
            tmdb::LookupItem {
                path: i.path.clone(),
                name,
            }
        })
        .collect();
    if !to_search.is_empty() {
        let found = tmdb::lookup(&Cli, &client, to_search).await;
        if let Some(failed) = found.iter().find_map(|f| f.error.clone())
            && found
                .iter()
                .all(|f| f.error.is_some() || f.guess.title.is_empty())
        {
            return Err(crate::text::error_text(&failed));
        }
        // Each guess asked once: a movie's subtitles share its answer.
        let mut answered: HashMap<(String, Option<u16>), Option<Candidate>> = HashMap::new();
        let mut stop = false;
        for f in found {
            if f.auto
                && let Some(c) = f.candidates.first()
            {
                chosen.insert(f.path, c.clone());
                continue;
            }
            if stop || ask.choose != Choose::Ask {
                continue;
            }
            let same = (f.guess.title.clone(), f.guess.year);
            let pick = match answered.get(&same) {
                Some(p) => p.clone(),
                None => {
                    let p = match choose_movie(&client, &f.path, &f.guess, f.candidates).await? {
                        Answer::Pick(c) => Some(c),
                        Answer::Skip => None,
                        Answer::Stop => {
                            stop = true;
                            None
                        }
                    };
                    answered.insert(same, p.clone());
                    p
                }
            };
            if let Some(c) = pick {
                chosen.insert(f.path, c);
            }
        }
        if stop {
            return Err(t("searchStopped", &[]));
        }
    }
    // The template fields of each movie (genre, director…).
    let ids: Vec<u64> = {
        let mut v: Vec<u64> = chosen.values().map(|c| c.id).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let sets = tmdb::collections(&client, ids, false).await;
    let extras: HashMap<u64, renome_core::fields::Extra> = sets.extras.into_iter().collect();
    let chosen = chosen
        .into_iter()
        .map(|(p, c)| {
            let extra = extras.get(&c.id).cloned().unwrap_or_default();
            (
                p,
                MovieMatch {
                    id: c.id,
                    title: c.title,
                    original_title: c.original_title,
                    year: c.year,
                    extra,
                },
            )
        })
        .collect();
    Ok(Movies {
        key,
        lang: tmdb_lang,
        chosen,
    })
}

/// A search typed by the person: a TMDB or IMDb link or id, else a title
/// with an optional year at the end.
async fn typed_search(client: &tmdb::Client, text: &str) -> Result<Vec<Candidate>, String> {
    use renome_core::links::{self, Link};
    let text = text.trim().trim_start_matches("tmdb:");
    let (id, title, year) = match links::parse(text) {
        Some(Link::Tmdb { tv: false, id }) => (Some(id), String::new(), None),
        Some(Link::Imdb(tt)) => (
            Some(
                client
                    .find_imdb(&tt, false)
                    .await
                    .ok_or_else(|| t("linkNotFound", &[]))?,
            ),
            String::new(),
            None,
        ),
        Some(_) => return Err(t("linkOther", &[])),
        None => match text.parse::<u64>() {
            Ok(id) => (Some(id), String::new(), None),
            Err(_) => {
                let (title, year) = match text.rsplit_once(' ') {
                    Some((t, y)) if y.len() == 4 && y.parse::<u16>().is_ok() => {
                        (t.to_string(), y.parse().ok())
                    }
                    _ => (text.to_string(), None),
                };
                (None, title, year)
            }
        },
    };
    Ok(client.search(&Guess { title, year, id }).await?.0)
}

enum Answer<T> {
    Pick(T),
    Skip,
    Stop,
}

/// The review window's choice, in the terminal.
async fn choose_movie(
    client: &tmdb::Client,
    path: &Path,
    guess: &Guess,
    mut candidates: Vec<Candidate>,
) -> Result<Answer<Candidate>, String> {
    if !std::io::stdin().is_terminal() {
        return Ok(Answer::Skip);
    }
    loop {
        eprintln!();
        eprintln!(
            "{}",
            out::bold(
                &path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            )
        );
        let looked = match guess.year {
            Some(y) => format!("{} ({y})", guess.title),
            None => guess.title.clone(),
        };
        eprintln!("  {} {}", t("reviewGuess", &[]), looked);
        if candidates.is_empty() {
            eprintln!("  {}", out::dim(&t("reviewNoResults", &[])));
        }
        for (n, c) in candidates.iter().enumerate().take(9) {
            let year = c.year.map(|y| format!(" ({y})")).unwrap_or_default();
            let original = if !c.original_title.is_empty() && c.original_title != c.title {
                out::dim(&format!(" — {}", c.original_title))
            } else {
                String::new()
            };
            eprintln!("  {}) {}{}{}", n + 1, c.title, year, original);
        }
        eprint!("{} ", t("chooseKeys", &[]));
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        if std::io::stdin().lock().read_line(&mut line).unwrap_or(0) == 0 {
            return Ok(Answer::Stop);
        }
        let line = line.trim();
        match line {
            "" if !candidates.is_empty() => return Ok(Answer::Pick(candidates.remove(0))),
            "" | "s" => return Ok(Answer::Skip),
            "q" => return Ok(Answer::Stop),
            "?" => eprintln!("{}", t("chooseHelp", &[])),
            _ => {
                if let Ok(n) = line.parse::<usize>()
                    && (1..=candidates.len().min(9)).contains(&n)
                {
                    return Ok(Answer::Pick(candidates.remove(n - 1)));
                }
                let query = line
                    .strip_prefix('/')
                    .or_else(|| line.strip_prefix("l "))
                    .unwrap_or(line);
                match typed_search(client, query).await {
                    Ok(found) => candidates = found,
                    Err(e) => out::error(&e),
                }
            }
        }
    }
}

/// After the renames: the adverts to the Trash, the `.nfo` and images,
/// the details inside the videos. What was done, as notices.
pub async fn after_movies(
    rule: &Value,
    movies: &Movies,
    preview: &Preview,
    report: &renome_core::ApplyReport,
    history: &renome_core::History,
) -> Vec<String> {
    let mut notes = Vec::new();
    let on = |k: &str| rule.get(k).and_then(Value::as_bool).unwrap_or(false);
    let recorded = report.done.is_empty() || report.history_id.is_some();
    if on("trashAds")
        && !report.done.is_empty()
        && (!preview.trash.is_empty() || !preview.trash_folders.is_empty())
    {
        let (trashed, failed) =
            renome_core::plan::trash_leftovers(&preview.trash, &preview.trash_folders);
        let ads = preview.items.iter().filter(|i| i.to_trash).count();
        if failed.is_empty() {
            notes.push(t("adsTrashedN", &[("n", &ads)]));
        } else {
            notes.push(t("adsTrashedN", &[("n", &trashed.len())]));
            notes.push(t("adsFailedN", &[("n", &failed.len())]));
        }
    }
    let video = |p: &Path| {
        p.extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .is_some_and(|e| renome_core::tmdb::is_video(&e))
    };
    // Renamed movies, and those whose names were already right.
    let mut pairs: Vec<(PathBuf, PathBuf)> = report
        .done
        .iter()
        .map(|op| (op.from.clone(), op.to.clone()))
        .collect();
    pairs.extend(
        preview
            .items
            .iter()
            .filter(|i| i.status == renome_core::Status::Unchanged)
            .map(|i| (i.path.clone(), i.path.clone())),
    );
    let ids: Vec<(PathBuf, u64)> = pairs
        .iter()
        .filter(|(from, to)| video(to) && movies.chosen.contains_key(from))
        .map(|(from, to)| (to.clone(), movies.chosen[from].id))
        .collect();
    if recorded && on("nfo") && !ids.is_empty() {
        let items = ids
            .iter()
            .map(|(path, id)| tmdb::SidecarItem {
                path: path.clone(),
                id: *id,
            })
            .collect();
        let r = renome_services::after::movie_sidecars(
            &Cli,
            history,
            &movies.key,
            &movies.lang,
            report.history_id.clone(),
            items,
            on("backdrop"),
            on("folder"),
        )
        .await;
        if !r.created.is_empty() {
            notes.push(t("sidecarsN", &[("n", &r.created.len())]));
        }
        for (file, why) in r.errors {
            out::error(&format!("{file}: {}", crate::text::error_text(&why)));
        }
    }
    if recorded && on("videoTags") && !ids.is_empty() {
        let items = ids
            .iter()
            .filter(|(p, _)| renome_core::videotags::writable(p))
            .map(|(path, id)| tmdb::VideoTagItem {
                path: path.clone(),
                movie: Some(*id),
                show: None,
                season: 0,
                episodes: vec![],
            })
            .collect();
        let options = renome_services::after::VideoTagOptions {
            cover: true,
            clean: on("videoClean"),
            tracks: on("videoTracks"),
        };
        match renome_services::after::video_tags(
            &Cli,
            history,
            &movies.key,
            &movies.lang,
            report.history_id.clone(),
            items,
            options,
        )
        .await
        {
            Ok(r) => {
                if r.written > 0 {
                    notes.push(t("insideN", &[("n", &r.written)]));
                }
                for (file, why) in r.errors {
                    out::error(&format!("{file}: {}", crate::text::error_text(&why)));
                }
            }
            Err(e) => out::error(&crate::text::error_text(&e)),
        }
    }
    notes
}

/// What the person answered to a list of choices.
enum Pick {
    Number(usize),
    Skip,
    Stop,
    Search(String),
}

/// A list of choices for one file, in the terminal: its name, what was
/// looked for, the options (at most nine), and the keys.
fn ask_one(title: &str, looked: &str, options: &[String]) -> Pick {
    loop {
        eprintln!();
        eprintln!("{}", out::bold(title));
        if !looked.is_empty() {
            eprintln!("  {} {}", t("reviewGuess", &[]), looked);
        }
        if options.is_empty() {
            eprintln!("  {}", out::dim(&t("reviewNoResults", &[])));
        }
        for (n, o) in options.iter().enumerate().take(9) {
            eprintln!("  {}) {}", n + 1, o);
        }
        eprint!("{} ", t("chooseKeys", &[]));
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        if std::io::stdin().lock().read_line(&mut line).unwrap_or(0) == 0 {
            return Pick::Stop;
        }
        match line.trim() {
            "" if !options.is_empty() => return Pick::Number(0),
            "" | "s" => return Pick::Skip,
            "q" => return Pick::Stop,
            "?" => eprintln!("{}", t("chooseHelp", &[])),
            other => {
                if let Ok(n) = other.parse::<usize>()
                    && (1..=options.len().min(9)).contains(&n)
                {
                    return Pick::Number(n - 1);
                }
                let query = other
                    .strip_prefix('/')
                    .or_else(|| other.strip_prefix("l "))
                    .unwrap_or(other);
                return Pick::Search(query.trim().to_string());
            }
        }
    }
}

fn candidate_line(c: &Candidate) -> String {
    let year = c.year.map(|y| format!(" ({y})")).unwrap_or_default();
    let original = if !c.original_title.is_empty() && c.original_title != c.title {
        out::dim(&format!(" — {}", c.original_title))
    } else {
        String::new()
    };
    format!("{}{year}{original}", c.title)
}

/// What the series rule found and chose.
pub struct Series {
    pub key: String,
    pub lang: String,
    pub anime: bool,
    pub chosen: HashMap<PathBuf, renome_core::tv::EpisodeMatch>,
    /// Files whose show could not be told.
    pub loose: usize,
}

/// An episode's template fields: the show's, its air date and its number
/// counted from the first episode (specials apart).
fn episode_extra(
    show: &renome_core::tv::Show,
    season: u16,
    number: u16,
) -> renome_core::fields::Extra {
    let mut extra = show.extra.clone();
    let ep = show
        .episodes
        .iter()
        .find(|e| e.season == season && e.number == number);
    if let Some(date) = ep.and_then(|e| e.air_date.clone()) {
        extra.insert("date".into(), date);
    }
    if season > 0 && ep.is_some() {
        let n = show
            .episodes
            .iter()
            .filter(|e| {
                e.season > 0 && (e.season < season || (e.season == season && e.number <= number))
            })
            .count();
        extra.insert("absolute".into(), n.to_string());
    }
    extra
}

fn episode_match(
    show: &renome_core::tv::Show,
    keys: &[renome_core::tv::Key],
) -> renome_core::tv::EpisodeMatch {
    renome_core::tv::EpisodeMatch {
        source: show.source.clone(),
        id: show.id,
        show: show.name.clone(),
        original_show: show.original_name.clone(),
        year: show.year,
        season: keys[0].0,
        episodes: keys.iter().map(|k| k.1).collect(),
        titles: keys
            .iter()
            .map(|k| {
                show.episodes
                    .iter()
                    .find(|e| e.season == k.0 && e.number == k.1)
                    .map(|e| e.name.clone())
                    .unwrap_or_default()
            })
            .collect(),
        extra: episode_extra(show, keys[0].0, keys[0].1),
    }
}

/// `S01E05`, `1x05`, `S01E05-E06` read as episode keys of the show.
fn typed_episodes(text: &str, show: &renome_core::tv::Show) -> Option<Vec<renome_core::tv::Key>> {
    let re = regex_lite(text)?;
    let (season, first, last) = re;
    let keys: Vec<renome_core::tv::Key> = (first..=last).map(|n| (season, n)).collect();
    keys.iter()
        .all(|k| {
            show.episodes
                .iter()
                .any(|e| e.season == k.0 && e.number == k.1)
        })
        .then_some(keys)
}

/// Season and episode range from `S01E05`, `S1E5-E6`, `1x05`, `1x05-06`.
fn regex_lite(text: &str) -> Option<(u16, u16, u16)> {
    let t = text.trim().to_lowercase();
    let (season, rest) = if let Some(r) = t.strip_prefix('s') {
        let (s, r) = r.split_once('e')?;
        (s.parse().ok()?, r.to_string())
    } else {
        let (s, r) = t.split_once('x')?;
        (s.parse().ok()?, r.to_string())
    };
    let (a, b) = match rest.split_once('-') {
        Some((a, b)) => (a.parse().ok()?, b.trim_start_matches('e').parse().ok()?),
        None => {
            let a: u16 = rest.parse().ok()?;
            (a, a)
        }
    };
    (b >= a).then_some((season, a, b))
}

/// Looks up the episodes of the list's videos and subtitles: the show
/// typed in the rule (or each show the names and folders tell), asked
/// when unsure, then each file's episode, the doubtful ones asked.
pub async fn series(
    preview: &Preview,
    rule: &Value,
    settings: Option<&Value>,
    ask: &Ask,
) -> Result<Series, String> {
    let anime = rule.get("source").and_then(Value::as_str) == Some("anilist");
    let (key, tmdb_lang) = if anime {
        (String::new(), String::new())
    } else {
        tmdb_access(settings)?
    };
    let client = tmdb::Client::new(&key, &tmdb_lang);
    let anilist = renome_services::anilist::Client::default();
    let typed = rule
        .get("show")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let all: Vec<tmdb::TvItem> = preview
        .items
        .iter()
        .filter(|i| {
            let ext = Path::new(&i.original)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            !i.is_dir && renome_core::tmdb::is_screen(&ext)
        })
        .map(|i| tmdb::TvItem {
            path: i.path.clone(),
            name: i.lookup_name.clone().unwrap_or_else(|| i.original.clone()),
            is_dir: false,
        })
        .collect();
    let mut out_series = Series {
        key: key.clone(),
        lang: tmdb_lang.clone(),
        anime,
        chosen: HashMap::new(),
        loose: 0,
    };
    if all.is_empty() {
        return Ok(out_series);
    }
    let by_path: HashMap<PathBuf, &tmdb::TvItem> =
        all.iter().map(|i| (i.path.clone(), i)).collect();
    let mut groups: Vec<Vec<tmdb::TvItem>> = vec![all.clone()];
    if typed.is_empty() {
        let g = tmdb::group_shows(&all);
        out_series.loose = g.loose.len();
        if !g.groups.is_empty() {
            groups = g
                .groups
                .into_iter()
                .map(|list| {
                    list.into_iter()
                        .filter_map(|p| by_path.get(&p).map(|i| (*i).clone()))
                        .collect()
                })
                .collect();
        }
    }
    let mut picked_shows: HashMap<String, Candidate> = HashMap::new();
    for group in groups {
        if let Some(first) = group.first()
            && let Some(pick) = ask.picks.get(&first.path)
        {
            // A show forced for the first file of the group: the whole group.
            let _ = pick;
        }
        let search = if anime {
            anilist.anime_search(&typed, &group).await
        } else {
            tmdb::search_show(&client, &typed, &group).await
        };
        let res = match search {
            Ok(r) => r,
            Err(e) if e == "tvNoShow" && typed.is_empty() => continue,
            Err(e) => return Err(crate::text::error_text(&e)),
        };
        let show = match picked_shows.get(&res.query) {
            Some(c) => Some(c.clone()),
            None => {
                let c = if res.auto {
                    res.candidates.first().cloned()
                } else {
                    choose_show(&client, &anilist, anime, &res.query, res.candidates, ask).await?
                };
                if let Some(c) = &c {
                    picked_shows.insert(res.query.clone(), c.clone());
                }
                c
            }
        };
        let Some(show) = show else { continue };
        let typed_or = if typed.is_empty() {
            res.query.clone()
        } else {
            typed.clone()
        };
        let look = if anime {
            anilist.anime_lookup(show.id, &group, &typed_or).await
        } else {
            tmdb::lookup_episodes(&client, show.id, group, &typed_or).await
        }
        .map_err(|e| crate::text::error_text(&e))?;
        let mut answered: HashMap<String, Option<Vec<renome_core::tv::Key>>> = HashMap::new();
        for f in look.files {
            let found = f.found;
            let keys = if let Some(pick) = ask.picks.get(&f.path) {
                typed_episodes(pick, &look.show)
            } else if !found.choice.is_empty() {
                Some(found.choice.clone())
            } else if ask.choose == Choose::Ask && std::io::stdin().is_terminal() {
                let guess = serde_json::to_string(&found.guess).unwrap_or_default();
                match answered.get(&guess) {
                    Some(k) => k.clone(),
                    None => {
                        let k = choose_episode(&f.path, &look.show, &found)?;
                        answered.insert(guess, k.clone());
                        k
                    }
                }
            } else {
                None
            };
            if let Some(keys) = keys.filter(|k| !k.is_empty()) {
                out_series
                    .chosen
                    .insert(f.path, episode_match(&look.show, &keys));
            }
        }
    }
    Ok(out_series)
}

/// The show picker, in the terminal.
async fn choose_show(
    client: &tmdb::Client,
    anilist: &renome_services::anilist::Client,
    anime: bool,
    query: &str,
    mut candidates: Vec<Candidate>,
    ask: &Ask,
) -> Result<Option<Candidate>, String> {
    if ask.choose != Choose::Ask || !std::io::stdin().is_terminal() {
        return Ok(None);
    }
    loop {
        let lines: Vec<String> = candidates.iter().map(candidate_line).collect();
        match ask_one(&t("showPickTitle", &[]), query, &lines) {
            Pick::Number(n) => return Ok(Some(candidates.remove(n))),
            Pick::Skip => return Ok(None),
            Pick::Stop => return Err(t("searchStopped", &[])),
            Pick::Search(q) => {
                let found = if anime {
                    anilist.anime_search(&q, &[]).await
                } else {
                    tmdb::search_show(client, &q, &[]).await
                };
                match found {
                    Ok(r) => candidates = r.candidates,
                    Err(e) => out::error(&crate::text::error_text(&e)),
                }
            }
        }
    }
}

/// A file's episode, in the terminal: the likely ones, or one typed.
fn choose_episode(
    path: &Path,
    show: &renome_core::tv::Show,
    found: &renome_core::tv::EpisodeLookup,
) -> Result<Option<Vec<renome_core::tv::Key>>, String> {
    let label = |keys: &Vec<renome_core::tv::Key>| {
        keys.iter()
            .map(|k| {
                let name = show
                    .episodes
                    .iter()
                    .find(|e| e.season == k.0 && e.number == k.1)
                    .map(|e| e.name.clone())
                    .unwrap_or_default();
                format!("S{:02}E{:02} · {name}", k.0, k.1)
            })
            .collect::<Vec<_>>()
            .join(" + ")
    };
    let options: Vec<String> = found.alternatives.iter().map(label).collect();
    let title = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let looked = format!("{} — {}", show.name, t("epTypeHint", &[]));
    loop {
        match ask_one(&title, &looked, &options) {
            Pick::Number(n) => return Ok(Some(found.alternatives[n].clone())),
            Pick::Skip => return Ok(None),
            Pick::Stop => return Err(t("searchStopped", &[])),
            Pick::Search(text) => match typed_episodes(&text, show) {
                Some(keys) => return Ok(Some(keys)),
                None => out::error(&t("epTypeBad", &[("text", &text)])),
            },
        }
    }
}

/// After the renames: each episode's `.nfo` and images (TMDB shows), and
/// the details inside the videos.
pub async fn after_series(
    rule: &Value,
    series: &Series,
    preview: &Preview,
    report: &renome_core::ApplyReport,
    history: &renome_core::History,
) -> Vec<String> {
    let mut notes = Vec::new();
    let on = |k: &str| rule.get(k).and_then(Value::as_bool).unwrap_or(false);
    let recorded = report.done.is_empty() || report.history_id.is_some();
    if series.anime || !recorded {
        return notes;
    }
    let video = |p: &Path| {
        p.extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .is_some_and(|e| renome_core::tmdb::is_video(&e))
    };
    let mut pairs: Vec<(PathBuf, PathBuf)> = report
        .done
        .iter()
        .map(|op| (op.from.clone(), op.to.clone()))
        .collect();
    pairs.extend(
        preview
            .items
            .iter()
            .filter(|i| i.status == renome_core::Status::Unchanged)
            .map(|i| (i.path.clone(), i.path.clone())),
    );
    let found: Vec<(PathBuf, &renome_core::tv::EpisodeMatch)> = pairs
        .iter()
        .filter(|(from, to)| video(to) && series.chosen.contains_key(from))
        .map(|(from, to)| (to.clone(), &series.chosen[from]))
        .collect();
    if on("nfo") && !found.is_empty() {
        let items = found
            .iter()
            .map(|(path, m)| tmdb::TvSidecarItem {
                path: path.clone(),
                id: m.id,
                season: m.season,
                episodes: m.episodes.clone(),
                titles: m.titles.clone(),
            })
            .collect();
        let client = tmdb::Client::new(&series.key, &series.lang);
        let mut r = tmdb::tv_sidecars(&Cli, &client, items, on("backdrop"), on("folders")).await;
        renome_services::after::record_created(history, report.history_id.clone(), &mut r);
        if !r.created.is_empty() {
            notes.push(t("sidecarsN", &[("n", &r.created.len())]));
        }
        for (file, why) in r.errors {
            out::error(&format!("{file}: {}", crate::text::error_text(&why)));
        }
    }
    if on("videoTags") && !found.is_empty() {
        let items = found
            .iter()
            .filter(|(p, _)| renome_core::videotags::writable(p))
            .map(|(path, m)| tmdb::VideoTagItem {
                path: path.clone(),
                movie: None,
                show: Some(m.id),
                season: m.season,
                episodes: m.episodes.clone(),
            })
            .collect();
        let options = renome_services::after::VideoTagOptions {
            cover: true,
            clean: on("videoClean"),
            tracks: on("videoTracks"),
        };
        match renome_services::after::video_tags(
            &Cli,
            history,
            &series.key,
            &series.lang,
            report.history_id.clone(),
            items,
            options,
        )
        .await
        {
            Ok(r) => {
                if r.written > 0 {
                    notes.push(t("insideN", &[("n", &r.written)]));
                }
                for (file, why) in r.errors {
                    out::error(&format!("{file}: {}", crate::text::error_text(&why)));
                }
            }
            Err(e) => out::error(&crate::text::error_text(&e)),
        }
    }
    notes
}

/// What a lookup rule found and chose.
pub enum Found {
    Movies(Movies),
    Series(Series),
    Music(Box<Music>),
    Books(Box<Books>),
}

impl Found {
    /// Whether this file was matched.
    pub fn has(&self, path: &Path) -> bool {
        match self {
            Found::Movies(m) => m.chosen.contains_key(path),
            Found::Series(s) => s.chosen.contains_key(path),
            Found::Music(m) => m.chosen.contains_key(path),
            Found::Books(b) => b.chosen.contains_key(path),
        }
    }

    /// Whether a file of this extension is one the rule looks up.
    pub fn wanted(&self, ext: &str) -> bool {
        match self {
            Found::Music(_) => renome_core::music::is_audio(ext),
            Found::Books(_) => renome_core::ebook::is_book(ext),
            _ => renome_core::tmdb::is_video(ext),
        }
    }
}

/// What the music rule found and chose.
pub struct Music {
    pub mb: renome_services::musicbrainz::Client,
    pub dg: renome_services::discogs::Client,
    /// Each song's release and place in it.
    pub chosen: HashMap<PathBuf, renome_core::musicbrainz::SongMatch>,
    /// Each album chosen: its folder (before renaming) and release.
    pub albums: Vec<(PathBuf, String)>,
}

fn mb_line(c: &renome_core::musicbrainz::Candidate) -> String {
    let mut details: Vec<String> = Vec::new();
    for v in [&c.date, &c.country, &c.format, &c.label] {
        if !v.is_empty() {
            details.push(v.clone());
        }
    }
    if c.tracks > 0 {
        details.push(t("albumTracks", &[("n", &c.tracks)]));
    }
    format!(
        "{} — {}{}",
        c.title,
        c.artist,
        out::dim(&format!(" ({})", details.join(", ")))
    )
}

/// Looks up the songs' albums on MusicBrainz (Discogs when it finds
/// nothing and a token is set; by sound with `identifyBySound`), the
/// doubtful albums asked in the terminal.
pub async fn music(
    files: &[renome_core::FileEntry],
    preview: &Preview,
    rule: &Value,
    ask: &Ask,
) -> Result<Music, String> {
    use renome_services::musicbrainz as mbs;
    let from_name = rule
        .get("fromName")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let listen = rule
        .get("identifyBySound")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let mb = mbs::Client::default();
    let dg = renome_services::discogs::Client::default();
    if let Some(token) = renome_services::load_secrets()
        .get("secrets")
        .and_then(|s| s.get("discogsToken"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        dg.set_token(token);
    }
    let acoustid = renome_services::acoustid::Client::default();
    let heard = std::sync::Mutex::new(HashMap::new());
    let wanted: std::collections::HashSet<&PathBuf> = preview
        .items
        .iter()
        .filter(|i| {
            let ext = Path::new(&i.original)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            !i.is_dir && (!i.untagged || listen) && renome_core::music::is_audio(&ext)
        })
        .map(|i| &i.path)
        .collect();
    let list: Vec<&renome_core::FileEntry> =
        files.iter().filter(|f| wanted.contains(&f.path)).collect();
    let (mut named, mut unnamed) = mbs::songs_of(&list, &HashMap::new(), from_name);
    if !listen {
        let (guessed, none): (Vec<_>, Vec<_>) =
            unnamed.into_iter().partition(|(_, t)| !t.title.is_empty());
        named.extend(guessed);
        unnamed = none;
    }
    let albums = mbs::albums(&named);
    let services = mbs::Services {
        mb: &mb,
        dg: &dg,
        acoustid: &acoustid,
        heard: &heard,
    };
    let found = mbs::lookup(&Cli, &services, albums, unnamed, listen).await;
    if let Some(failed) = found.iter().find_map(|f| f.error.clone())
        && found.iter().all(|f| f.error.is_some())
    {
        return Err(crate::text::error_text(&failed));
    }
    let mut out_music = Music {
        chosen: HashMap::new(),
        albums: Vec::new(),
        mb: mbs::Client::default(),
        dg: renome_services::discogs::Client::default(),
    };
    for album in found {
        let forced = album.files.iter().find_map(|p| ask.picks.get(p)).cloned();
        let id = if let Some(pick) = forced {
            Some(pick.trim().trim_start_matches("mb:").to_string())
        } else if let Some(c) = &album.choice {
            for (p, m) in &album.matches {
                out_music.chosen.insert(p.clone(), m.clone());
            }
            out_music.albums.push((album.dir.clone(), c.id.clone()));
            continue;
        } else if ask.choose == Choose::Ask && std::io::stdin().is_terminal() {
            choose_album(&mb, &dg, &album).await?
        } else {
            None
        };
        let Some(id) = id else { continue };
        // The songs as the window gives them to the chosen release.
        let list: Vec<&renome_core::FileEntry> = files
            .iter()
            .filter(|f| album.files.contains(&f.path))
            .collect();
        let (songs, unnamed) = mbs::songs_of(&list, &heard.lock().unwrap(), from_name);
        let mut songs = mbs::albums(&songs).concat();
        songs.extend(unnamed);
        let chosen = mbs::choose(&mb, &dg, &id, &songs)
            .await
            .map_err(|e| crate::text::error_text(&e))?;
        for (p, m) in chosen.matches {
            out_music.chosen.insert(p, m);
        }
        out_music.albums.push((album.dir.clone(), id));
    }
    out_music.mb = mb;
    out_music.dg = dg;
    Ok(out_music)
}

/// An album's release, in the terminal; a new search by album name (or a
/// MusicBrainz/Discogs link).
async fn choose_album(
    mb: &renome_services::musicbrainz::Client,
    dg: &renome_services::discogs::Client,
    album: &renome_services::musicbrainz::AlbumLookup,
) -> Result<Option<String>, String> {
    use renome_core::links::{self, Link};
    let mut candidates = album.candidates.clone();
    let title = album
        .dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let looked = format!("{} — {}", album.query.album, album.query.artist);
    loop {
        let lines: Vec<String> = candidates.iter().map(mb_line).collect();
        match ask_one(
            &format!(
                "{title} ({})",
                t("albumFiles", &[("n", &album.files.len())])
            ),
            &looked,
            &lines,
        ) {
            Pick::Number(n) => return Ok(Some(candidates[n].id.clone())),
            Pick::Skip => return Ok(None),
            Pick::Stop => return Err(t("searchStopped", &[])),
            Pick::Search(text) => {
                let found = match links::parse(&text) {
                    Some(Link::MusicBrainz(id)) => mb
                        .release(&id)
                        .await
                        .map(|r| vec![renome_core::musicbrainz::summary(&r, 1.0)]),
                    Some(Link::Discogs(id)) => dg
                        .release(&format!("{}{id}", renome_core::discogs::PREFIX))
                        .await
                        .map(|r| vec![renome_core::musicbrainz::summary(&r, 1.0)]),
                    Some(_) => Err("linkOther".into()),
                    None => {
                        mb.search(&renome_core::musicbrainz::AlbumQuery {
                            album: text.clone(),
                            ..album.query.clone()
                        })
                        .await
                    }
                };
                match found {
                    Ok(c) => candidates = c,
                    Err(e) => out::error(&crate::text::error_text(&e)),
                }
            }
        }
    }
}

/// After the renames: the album covers, the playlists and the tags.
pub async fn after_music(
    rule: &Value,
    music: &Music,
    preview: &Preview,
    report: &renome_core::ApplyReport,
    history: &renome_core::History,
) -> Vec<String> {
    let mut notes = Vec::new();
    let on = |k: &str| rule.get(k).and_then(Value::as_bool).unwrap_or(false);
    let recorded = report.done.is_empty() || report.history_id.is_some();
    if !recorded {
        return notes;
    }
    let to: HashMap<&PathBuf, &PathBuf> = report.done.iter().map(|op| (&op.from, &op.to)).collect();
    let now = |p: &PathBuf| to.get(p).map_or(p.clone(), |t| (*t).clone());
    if on("cover") {
        let mut seen = std::collections::HashSet::new();
        let items: Vec<renome_services::musicbrainz::CoverItem> = music
            .albums
            .iter()
            .filter_map(|(_, release)| {
                let first = music.chosen.iter().find(|(_, m)| &m.release == release)?.0;
                let dir = now(first).parent()?.to_path_buf();
                seen.insert(dir.clone())
                    .then(|| renome_services::musicbrainz::CoverItem {
                        dir,
                        release: release.clone(),
                    })
            })
            .collect();
        if !items.is_empty() {
            let mut r =
                renome_services::musicbrainz::covers(&Cli, &music.mb, &music.dg, items).await;
            renome_services::after::record_created(history, report.history_id.clone(), &mut r);
            if !r.created.is_empty() {
                notes.push(t("sidecarsN", &[("n", &r.created.len())]));
            }
        }
    }
    if on("playlist") {
        let mut dirs: Vec<PathBuf> = music
            .chosen
            .keys()
            .filter_map(|p| now(p).parent().map(Path::to_path_buf))
            .collect();
        dirs.sort();
        dirs.dedup();
        let r = renome_services::after::playlists(history, report.history_id.clone(), dirs);
        if !r.created.is_empty() {
            notes.push(t("sidecarsN", &[("n", &r.created.len())]));
        }
        if r.kept > 0 {
            notes.push(t("playlistsKeptN", &[("n", &r.kept)]));
        }
    }
    if on("writeTags") {
        let clean = rule
            .get("cleanTags")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let unchanged: std::collections::HashSet<&PathBuf> = preview
            .items
            .iter()
            .filter(|i| {
                i.status == renome_core::Status::Unchanged && (i.retag || (clean && i.junk))
            })
            .map(|i| &i.path)
            .collect();
        let items: Vec<renome_services::after::TagJob> = music
            .chosen
            .iter()
            .filter(|(p, _)| to.contains_key(p) || unchanged.contains(p))
            .map(|(p, m)| renome_services::after::TagJob {
                path: now(p),
                song: m.clone(),
            })
            .collect();
        if !items.is_empty() {
            let embed = rule
                .get("embedCover")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let size = rule
                .get("coverSize")
                .and_then(Value::as_u64)
                .map(|n| n as u32);
            match renome_services::after::song_tags(
                &Cli,
                history,
                &music.mb,
                &music.dg,
                report.history_id.clone(),
                items,
                embed,
                clean,
                size,
            )
            .await
            {
                Ok((r, _)) => {
                    notes.push(t("tagsN", &[("n", &r.written)]));
                    for (file, why) in r.errors {
                        out::error(&format!("{file}: {}", crate::text::error_text(&why)));
                    }
                }
                Err(e) => out::error(&crate::text::error_text(&e)),
            }
        }
    }
    notes
}

/// One file of an audiobook: its book's first file and its place.
#[derive(Clone)]
struct Part {
    leader: PathBuf,
    part: u32,
    parts: u32,
    narrator: String,
    series: String,
    series_index: String,
}

/// What the book rule found and chose.
pub struct Books {
    pub ol: renome_services::openlibrary::Client,
    /// What each book (and audiobook's first file) was guessed as, and
    /// the one chosen.
    found: HashMap<PathBuf, (renome_core::books::Guess, renome_core::books::Candidate)>,
    parts: HashMap<PathBuf, Part>,
    /// Each file's book, as the rule names it.
    pub chosen: HashMap<PathBuf, renome_core::books::BookMatch>,
}

fn book_match(
    g: &renome_core::books::Guess,
    c: &renome_core::books::Candidate,
) -> renome_core::books::BookMatch {
    let mut extra = renome_core::fields::Extra::new();
    let language = if c.exact || g.language.is_empty() {
        c.language.clone()
    } else {
        g.language.clone()
    };
    if !language.is_empty() {
        extra.insert("language".into(), language);
    }
    if let Some(p) = c.pages {
        extra.insert("pages".into(), p.to_string());
    }
    renome_core::books::BookMatch {
        work: c.work.clone(),
        title: c.title.clone(),
        original_title: c.original_title.clone(),
        authors: c.authors.clone(),
        year: c.year,
        edition_year: if c.exact { c.edition_year } else { None },
        publisher: if c.exact {
            c.publisher.clone()
        } else {
            g.publisher.clone()
        },
        isbn: if c.exact {
            c.isbn.clone()
        } else {
            g.isbns.first().cloned().unwrap_or_default()
        },
        series: g.series.clone(),
        series_index: g.series_index.clone(),
        extra,
        ..Default::default()
    }
}

fn book_line(c: &renome_core::books::Candidate) -> String {
    let mut details = Vec::new();
    if let Some(y) = c.year {
        details.push(y.to_string());
    }
    if !c.publisher.is_empty() {
        details.push(c.publisher.clone());
    }
    if c.editions > 0 {
        details.push(t("bookEditions", &[("n", &c.editions)]));
    }
    format!(
        "{} — {}{}",
        c.title,
        c.authors.join(", "),
        out::dim(&format!(" ({})", details.join(", ")))
    )
}

/// Looks up the books (and, with `audiobooks`, the audio files grouped
/// into books) on Open Library and Google Books, the doubtful ones asked.
pub async fn books(
    preview: &Preview,
    rule: &Value,
    settings: Option<&Value>,
    ask: &Ask,
) -> Result<Books, String> {
    use renome_services::openlibrary as ol;
    let lang = settings
        .and_then(|s| s.get("tmdbLang")?.as_str())
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| match lang() {
            "pt" => "pt-BR".into(),
            "es" => "es-ES".into(),
            _ => "en-US".into(),
        });
    let client = ol::Client::default();
    if let Some(key) = renome_services::load_secrets()
        .get("secrets")
        .and_then(|s| s.get("googleBooksKey"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        client.gb.set_key(key);
    }
    let ext_of = |name: &str| {
        Path::new(name)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    };
    let mut items: Vec<ol::LookupItem> = preview
        .items
        .iter()
        .filter(|i| !i.is_dir && renome_core::ebook::is_book(&ext_of(&i.original)))
        .map(|i| {
            let name = i.lookup_name.clone().unwrap_or_else(|| i.original.clone());
            let name = Path::new(&name)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(name);
            ol::LookupItem {
                path: i.path.clone(),
                name,
            }
        })
        .collect();
    let mut parts = HashMap::new();
    if rule
        .get("audiobooks")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        let audio: Vec<PathBuf> = preview
            .items
            .iter()
            .filter(|i| !i.is_dir && renome_core::music::is_audio(&ext_of(&i.original)))
            .map(|i| i.path.clone())
            .collect();
        if !audio.is_empty() {
            let groups = tokio::task::spawn_blocking(move || renome_core::audiobook::group(&audio))
                .await
                .unwrap_or_default();
            for b in groups {
                let leader = b.paths[0].clone();
                let count = b.paths.len() as u32;
                for (n, p) in b.paths.iter().enumerate() {
                    parts.insert(
                        p.clone(),
                        Part {
                            leader: leader.clone(),
                            part: n as u32 + 1,
                            parts: if count > 1 { count } else { 0 },
                            narrator: b.narrator.clone(),
                            series: b.series.clone(),
                            series_index: b.series_index.clone(),
                        },
                    );
                }
                if !b.title.is_empty() || !b.author.is_empty() {
                    let author = if b.author.is_empty() {
                        String::new()
                    } else {
                        format!("{} - ", b.author)
                    };
                    let year = b.year.map(|y| format!(" ({y})")).unwrap_or_default();
                    items.push(ol::LookupItem {
                        path: leader,
                        name: format!("{author}{}{year}", b.title),
                    });
                }
            }
        }
    }
    let mut out_books = Books {
        ol: ol::Client::default(),
        found: HashMap::new(),
        parts,
        chosen: HashMap::new(),
    };
    if items.is_empty() {
        return Ok(out_books);
    }
    let found = ol::lookup(&Cli, &client, &lang, items).await;
    if let Some(failed) = found.iter().find_map(|f| f.error.clone())
        && found
            .iter()
            .all(|f| f.error.is_some() || (f.guess.title.is_empty() && f.guess.isbns.is_empty()))
    {
        return Err(crate::text::error_text(&failed));
    }
    let mut answered: HashMap<(String, String), Option<renome_core::books::Candidate>> =
        HashMap::new();
    for f in found {
        let pick = if let Some(text) = ask.picks.get(&f.path) {
            book_search(&client, &lang, text).await?.into_iter().next()
        } else if f.auto {
            f.candidates.first().cloned()
        } else if ask.choose == Choose::Ask && std::io::stdin().is_terminal() {
            let key = (f.guess.title.clone(), f.guess.author.clone());
            match answered.get(&key) {
                Some(p) => p.clone(),
                None => {
                    let p = choose_book(&client, &lang, &f.path, &f.guess, f.candidates).await?;
                    answered.insert(key, p.clone());
                    p
                }
            }
        } else {
            None
        };
        if let Some(c) = pick {
            out_books.found.insert(f.path, (f.guess, c));
        }
    }
    // One spelling per author across the list.
    let raw: Vec<(PathBuf, renome_core::books::BookMatch)> = out_books
        .found
        .iter()
        .map(|(p, (g, c))| (p.clone(), book_match(g, c)))
        .collect();
    let names: Vec<String> = raw.iter().flat_map(|(_, m)| m.authors.clone()).collect();
    let spelling = renome_core::books::author_spellings(&names);
    for (p, mut m) in raw {
        m.title = renome_core::text::nfc(&m.title);
        let mut authors: Vec<String> = Vec::new();
        for a in &m.authors {
            let a = renome_core::text::nfc(a);
            let a = spelling.get(&a).cloned().unwrap_or(a);
            if !authors.contains(&a) {
                authors.push(a);
            }
        }
        m.authors = authors;
        out_books.chosen.insert(p, m);
    }
    // An audiobook's files take its first file's book, with their part.
    for (p, g) in out_books.parts.clone() {
        if let Some(m) = out_books.chosen.get(&g.leader).cloned() {
            let mut m = m;
            m.narrator = g.narrator.clone();
            m.part = g.part;
            m.parts = g.parts;
            if m.series.is_empty() {
                m.series = g.series.clone();
            }
            if m.series_index.is_empty() {
                m.series_index = g.series_index.clone();
            }
            out_books.chosen.insert(p, m);
        }
    }
    out_books.ol = client;
    Ok(out_books)
}

/// A book searched by the person: an ISBN or a link, else `title / author`.
async fn book_search(
    client: &renome_services::openlibrary::Client,
    lang: &str,
    text: &str,
) -> Result<Vec<renome_core::books::Candidate>, String> {
    if let Some(link) = renome_core::links::parse(text.trim()) {
        return client
            .by_link(&link, lang)
            .await
            .map_err(|e| crate::text::error_text(&e));
    }
    let (title, author) = text.split_once(" / ").unwrap_or((text, ""));
    let guess = renome_core::books::Guess {
        title: title.trim().into(),
        author: author.trim().into(),
        from_file: true,
        ..Default::default()
    };
    Ok(client
        .find(&guess, lang)
        .await
        .map_err(|e| crate::text::error_text(&e))?
        .0)
}

async fn choose_book(
    client: &renome_services::openlibrary::Client,
    lang: &str,
    path: &Path,
    guess: &renome_core::books::Guess,
    mut candidates: Vec<renome_core::books::Candidate>,
) -> Result<Option<renome_core::books::Candidate>, String> {
    let title = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let looked = if guess.author.is_empty() {
        guess.title.clone()
    } else {
        format!("{} / {}", guess.title, guess.author)
    };
    loop {
        let lines: Vec<String> = candidates.iter().map(book_line).collect();
        match ask_one(&title, &looked, &lines) {
            Pick::Number(n) => return Ok(Some(candidates.remove(n))),
            Pick::Skip => return Ok(None),
            Pick::Stop => return Err(t("searchStopped", &[])),
            Pick::Search(text) => match book_search(client, lang, &text).await {
                Ok(c) => candidates = c,
                Err(e) => out::error(&e),
            },
        }
    }
}

/// After the renames: cover and `metadata.opf`, the details and cover
/// inside audiobooks, the details inside EPUB/AZW3/FB2 (and polishing),
/// the conversions.
pub async fn after_books(
    rule: &Value,
    books: &Books,
    preview: &Preview,
    report: &renome_core::ApplyReport,
    history: &renome_core::History,
) -> Vec<String> {
    use renome_services::openlibrary as ol;
    let mut notes = Vec::new();
    let on = |k: &str| rule.get(k).and_then(Value::as_bool).unwrap_or(false);
    let recorded = report.done.is_empty() || report.history_id.is_some();
    if !recorded {
        return notes;
    }
    let ext_of = |p: &Path| {
        p.extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    };
    let is_book = |p: &Path| renome_core::ebook::is_book(&ext_of(p));
    let is_audio = |p: &Path| renome_core::music::is_audio(&ext_of(p));
    let leads = |p: &PathBuf| books.parts.get(p).is_none_or(|g| &g.leader == p);
    // Renamed files, and those whose names were already right.
    let mut pairs: Vec<(PathBuf, PathBuf)> = report
        .done
        .iter()
        .map(|op| (op.from.clone(), op.to.clone()))
        .collect();
    pairs.extend(
        preview
            .items
            .iter()
            .filter(|i| i.status == renome_core::Status::Unchanged && !i.is_dir)
            .map(|i| (i.path.clone(), i.path.clone())),
    );
    let book_to: HashMap<&PathBuf, renome_core::bookconv::Target> = preview
        .items
        .iter()
        .filter_map(|i| Some((&i.path, i.book_to?)))
        .collect();
    // What a file's book is, with what came from the catalogue alongside.
    let detail = |from: &PathBuf| -> Option<(
        renome_core::books::BookMatch,
        &renome_core::books::Candidate,
        String,
    )> {
        let m = books.chosen.get(from)?.clone();
        let leader = books.parts.get(from).map_or(from, |g| &g.leader);
        let (g, c) = books.found.get(leader)?;
        let language = if c.exact {
            c.language.clone()
        } else {
            g.language.clone()
        };
        Some((m, c, language))
    };
    if on("cover") || on("metadataOpf") {
        let items: Vec<ol::SidecarItem> = pairs
            .iter()
            .filter(|(from, to)| is_book(to) || (is_audio(to) && leads(from)))
            .filter_map(|(from, to)| {
                let (book, c, language) = detail(from)?;
                Some(ol::SidecarItem {
                    path: to.clone(),
                    book,
                    cover: c.cover,
                    cover_url: c.cover_url.clone(),
                    description: c.description.clone(),
                    language,
                })
            })
            .collect();
        if !items.is_empty() {
            let mut r = ol::sidecars(
                &Cli,
                &books.ol,
                items,
                on("cover"),
                on("metadataOpf"),
                on("folders"),
            )
            .await;
            renome_services::after::record_created(history, report.history_id.clone(), &mut r);
            if !r.created.is_empty() {
                notes.push(t("sidecarsN", &[("n", &r.created.len())]));
            }
            for (file, why) in r.errors {
                out::error(&format!("{file}: {}", crate::text::error_text(&why)));
            }
        }
    }
    if on("audioTags")
        && rule
            .get("audiobooks")
            .and_then(Value::as_bool)
            .unwrap_or(true)
    {
        let items: Vec<renome_services::after::AudiobookTagJob> = pairs
            .iter()
            .filter(|(_, to)| is_audio(to))
            .filter_map(|(from, to)| {
                let (book, c, language) = detail(from)?;
                Some(renome_services::after::AudiobookTagJob {
                    path: to.clone(),
                    book,
                    cover: c.cover,
                    cover_url: c.cover_url.clone(),
                    description: c.description.clone(),
                    language,
                })
            })
            .collect();
        if !items.is_empty() {
            match renome_services::after::audiobook_tags(
                &Cli,
                history,
                &books.ol,
                report.history_id.clone(),
                items,
                true,
                on("audioClean"),
            )
            .await
            {
                Ok((r, _)) => {
                    notes.push(t("audioTagsN", &[("n", &r.written)]));
                    for (file, why) in r.errors {
                        out::error(&format!("{file}: {}", crate::text::error_text(&why)));
                    }
                }
                Err(e) => out::error(&crate::text::error_text(&e)),
            }
        }
    }
    // Inside the books: details, polishing, conversions.
    let polish = (on("epub3") || on("shrinkImages"))
        .then(|| serde_json::json!({ "epub3": on("epub3"), "shrink": on("shrinkImages") }));
    let original = rule
        .get("original")
        .and_then(Value::as_str)
        .unwrap_or("keep");
    let mut jobs: Vec<renome_services::after::InsideJob> = Vec::new();
    for (from, to) in &pairs {
        let mut job = serde_json::json!({ "path": to });
        let details = detail(from);
        if let Some((book, _, language)) = &details {
            job["book"] = serde_json::to_value(book).unwrap_or_default();
            job["language"] = Value::from(language.clone());
        }
        if let Some(target) = book_to.get(from) {
            job["bookTo"] = serde_json::to_value(target).unwrap_or_default();
            job["convert"] = Value::from(original);
        } else {
            let writable =
                renome_core::inside::book_writable(to) || renome_core::bookmeta::writable(to);
            let epub = matches!(ext_of(to).as_str(), "epub" | "kepub");
            let writes = on("writeEpub") && details.is_some() && writable;
            if epub && let Some(p) = &polish {
                job["polish"] = p.clone();
            } else if !writes {
                continue;
            }
            if !writes {
                job.as_object_mut().map(|o| o.remove("book"));
            }
        }
        if let Ok(j) = serde_json::from_value(job) {
            jobs.push(j);
        }
    }
    if !jobs.is_empty() {
        match renome_services::after::inside(&Cli, history, report.history_id.clone(), jobs).await {
            Ok(o) => {
                notes.push(t("insideN", &[("n", &o.report.written)]));
                for (file, why) in o.report.errors {
                    out::error(&format!("{file}: {}", crate::text::error_text(&why)));
                }
            }
            Err(e) => out::error(&crate::text::error_text(&e)),
        }
    }
    notes
}

/// After the renames, the subtitle rule's work: subtitles fixed where
/// they are (or made SubRip), text tracks taken out of videos.
pub async fn after_subtitles(
    rule: &Value,
    preview: &Preview,
    report: &renome_core::ApplyReport,
    history: &renome_core::History,
) -> Vec<String> {
    let mut notes = Vec::new();
    if !(report.done.is_empty() || report.history_id.is_some()) {
        return notes;
    }
    let to: HashMap<&PathBuf, &PathBuf> = report.done.iter().map(|op| (&op.from, &op.to)).collect();
    let jobs: Vec<renome_services::after::SubJob> = preview
        .items
        .iter()
        .filter(|i| i.status != renome_core::Status::Conflict)
        .filter(|i| to.contains_key(&i.path) || i.status == renome_core::Status::Unchanged)
        .filter_map(|i| {
            let fix = i.sub_fix.as_ref().is_some_and(|f| !f.no_fps);
            let extract = i.sub_extract > 0;
            (fix || extract).then(|| renome_services::after::SubJob {
                path: to.get(&i.path).map_or(i.path.clone(), |p| (*p).clone()),
                fix,
                extract,
            })
        })
        .collect();
    if jobs.is_empty() {
        return notes;
    }
    let tools: renome_core::subtext::Tools =
        serde_json::from_value(rule.clone()).unwrap_or_default();
    let original = match rule.get("original").and_then(Value::as_str) {
        Some("delete") => renome_core::convert::Original::Delete,
        _ => renome_core::convert::Original::Keep,
    };
    let langs = if rule
        .get("extract")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        rule.get("extractLangs")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    } else {
        String::new()
    };
    match renome_services::after::subtitle_tools(
        &Cli,
        history,
        report.history_id.clone(),
        jobs,
        tools,
        original,
        langs,
    )
    .await
    {
        Ok((r, _)) => {
            if r.written > 0 {
                notes.push(t("subToolsN", &[("n", &r.written)]));
            }
            if r.created > 0 {
                notes.push(t("subToolsM", &[("n", &r.created)]));
            }
            for (file, why) in r.errors {
                out::error(&format!("{file}: {}", crate::text::error_text(&why)));
            }
        }
        Err(e) => out::error(&crate::text::error_text(&e)),
    }
    notes
}
