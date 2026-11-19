//! The lookup rules on the command line: the catalogues searched as the
//! window's search buttons do, the doubtful files asked about in the
//! terminal (or left, or forced with `--pick`), and what runs after the
//! renames.

use std::collections::HashMap;
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use renome_core::Preview;
use renome_core::tmdb::{Candidate, Guess, MovieMatch};
use renome_services::{Host, flow, tmdb};
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
    /// What is missing is wanted (`--missing`): collections asked too.
    pub missing: bool,
    /// `--pick PATH=ID`, by full path.
    pub picks: HashMap<PathBuf, String>,
}

/// The command line as the host of the shared work: progress on stderr.
pub struct Cli;

impl Host for Cli {
    fn report(&self, event: &str, payload: Value) {
        if !out::live() {
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
            ("subs-progress", "save") => "subsWriting",
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
        Arc::new(crate::stop::requested)
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
    /// The collections the movies are in (with `--missing`).
    pub sets: Option<tmdb::MovieSets>,
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
    let items: Vec<flow::Item> = preview.items.iter().map(flow::Item::from).collect();
    let mut chosen: HashMap<PathBuf, Candidate> = HashMap::new();
    // Forced first: no search for them.
    for i in flow::movies::targets(&items) {
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
    let rest: Vec<flow::Item> = items
        .into_iter()
        .filter(|i| !chosen.contains_key(&i.path))
        .collect();
    let found = flow::movies::search(&Cli, &client, &rest)
        .await
        .map_err(|e| crate::text::error_text(&e))?;
    // Each group asked once: a movie's subtitles share its answer.
    let mut answered: HashMap<String, Option<Candidate>> = HashMap::new();
    let mut stop = false;
    for f in found {
        if let Some(c) = f.choice {
            chosen.insert(f.path, c);
            continue;
        }
        if stop || ask.choose != Choose::Ask {
            continue;
        }
        let pick = match answered.get(&f.group) {
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
                answered.insert(f.group, p.clone());
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
    // The template fields of each movie (genre, director…).
    let ids: Vec<u64> = {
        let mut v: Vec<u64> = chosen.values().map(|c| c.id).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let mut sets = tmdb::collections(&client, ids, ask.missing).await;
    let extras: HashMap<u64, renome_core::fields::Extra> =
        std::mem::take(&mut sets.extras).into_iter().collect();
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
        sets: ask.missing.then_some(sets),
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

impl<T> Answer<T> {
    fn map<U>(self, f: impl FnOnce(T) -> U) -> Answer<U> {
        match self {
            Answer::Pick(v) => Answer::Pick(f(v)),
            Answer::Skip => Answer::Skip,
            Answer::Stop => Answer::Stop,
        }
    }
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
    // Stopped: the remaining doubts stay as they are.
    if crate::stop::requested() {
        return Pick::Skip;
    }
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
        if crate::stop::asking(|| std::io::stdin().lock().read_line(&mut line).unwrap_or(0)) == 0 {
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
    pub chosen: HashMap<PathBuf, renome_core::tv::EpisodeMatch>,
    /// The shows fetched, with their episodes.
    pub shows: Vec<renome_core::tv::Show>,
    /// Files whose show could not be told.
    pub loose: usize,
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

/// The command line as the host of the series flow: the show it cannot
/// settle is asked in the terminal (or left, with `--choose auto|skip`).
struct Asker<'a> {
    client: &'a tmdb::Client,
    anilist: &'a renome_services::anilist::Client,
    /// The comic catalogues, with the rule's search options.
    comics: Option<(
        &'a renome_services::comics::Sources<'a>,
        &'a flow::comics::Params,
    )>,
    ask: &'a Ask,
    /// The person asked to stop (`q`).
    quit: Arc<std::sync::atomic::AtomicBool>,
}

impl Host for Asker<'_> {
    fn report(&self, event: &str, payload: Value) {
        Cli.report(event, payload)
    }
    fn stop_signal(&self) -> Arc<dyn Fn() -> bool + Send + Sync> {
        let quit = self.quit.clone();
        Arc::new(move || crate::stop::requested() || quit.load(std::sync::atomic::Ordering::SeqCst))
    }
    fn sender(&self) -> renome_services::Sender {
        Cli.sender()
    }
    fn ask(
        &self,
        question: flow::Question,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<Value>> + Send + '_>> {
        Box::pin(async move {
            if self.ask.choose != Choose::Ask || !std::io::stdin().is_terminal() {
                return None;
            }
            let answer = match question {
                flow::Question::Show {
                    source,
                    query,
                    candidates,
                } => choose_show(
                    self.client,
                    self.anilist,
                    source == "anilist",
                    &query,
                    candidates,
                )
                .await
                .map(|c| serde_json::to_value(c).ok()),
                flow::Question::ComicSeries {
                    query,
                    files,
                    candidates,
                    ..
                } => {
                    let (sources, p) = self.comics?;
                    choose_volume(sources, p, &query, files, candidates)
                        .await
                        .map(|v| serde_json::to_value(v).ok())
                }
            };
            match answer {
                Answer::Pick(v) => v,
                Answer::Skip => None,
                Answer::Stop => {
                    self.quit.store(true, std::sync::atomic::Ordering::SeqCst);
                    None
                }
            }
        })
    }
}

/// Looks up the episodes of the list's videos and subtitles (the shared
/// flow: the show typed in the rule, or each show the names and folders
/// tell, asked when unsure), then asks the doubtful files.
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
    let typed = rule.get("show").and_then(Value::as_str).unwrap_or_default();
    let items: Vec<flow::Item> = preview.items.iter().map(flow::Item::from).collect();
    let host = Asker {
        client: &client,
        anilist: &anilist,
        comics: None,
        ask,
        quit: Arc::default(),
    };
    let source = if anime {
        flow::series::Source::Anilist(&anilist)
    } else {
        flow::series::Source::Tmdb(&client)
    };
    let found = flow::series::search(
        &host,
        source,
        typed,
        &items,
        &Default::default(),
        HashMap::new(),
    )
    .await
    .map_err(|e| crate::text::error_text(&e))?;
    if host.quit.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(t("searchStopped", &[]));
    }
    let mut out_series = Series {
        key,
        lang: tmdb_lang,
        chosen: HashMap::new(),
        shows: found.shows,
        loose: found.loose,
    };
    // Each group asked once: an episode's subtitles share its answer.
    let mut answered: HashMap<String, Option<Vec<renome_core::tv::Key>>> = HashMap::new();
    for f in found.files {
        let Some(show) = out_series.shows.iter().find(|s| s.id == f.show_id) else {
            continue;
        };
        let keys = if let Some(pick) = ask.picks.get(&f.path) {
            typed_episodes(pick, show)
        } else if !f.choice.is_empty() {
            Some(f.choice.clone())
        } else if ask.choose == Choose::Ask && std::io::stdin().is_terminal() {
            match answered.get(&f.group) {
                Some(k) => k.clone(),
                None => {
                    let k = choose_episode(&f.path, show, &f.alternatives)?;
                    answered.insert(f.group.clone(), k.clone());
                    k
                }
            }
        } else {
            None
        };
        if let Some(keys) = keys.filter(|k| !k.is_empty()) {
            let m = flow::series::episode_match(show, &keys);
            out_series.chosen.insert(f.path, m);
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
) -> Answer<Candidate> {
    loop {
        let lines: Vec<String> = candidates.iter().map(candidate_line).collect();
        match ask_one(&t("showPickTitle", &[]), query, &lines) {
            Pick::Number(n) => return Answer::Pick(candidates.remove(n)),
            Pick::Skip => return Answer::Skip,
            Pick::Stop => return Answer::Stop,
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
    alternatives: &[Vec<renome_core::tv::Key>],
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
    let options: Vec<String> = alternatives.iter().map(label).collect();
    let title = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let looked = format!("{} — {}", show.name, t("epTypeHint", &[]));
    loop {
        match ask_one(&title, &looked, &options) {
            Pick::Number(n) => return Ok(Some(alternatives[n].clone())),
            Pick::Skip => return Ok(None),
            Pick::Stop => return Err(t("searchStopped", &[])),
            Pick::Search(text) => match typed_episodes(&text, show) {
                Some(keys) => return Ok(Some(keys)),
                None => out::error(&t("epTypeBad", &[("text", &text)])),
            },
        }
    }
}

/// What a lookup rule found and chose.
pub enum Found {
    Movies(Movies),
    Series(Series),
    Music(Box<Music>),
    Books(Box<Books>),
    Comics(Box<Comics>),
}

impl Found {
    /// Whether this file was matched.
    pub fn has(&self, path: &Path) -> bool {
        match self {
            Found::Movies(m) => m.chosen.contains_key(path),
            Found::Series(s) => s.chosen.contains_key(path),
            Found::Music(m) => m.chosen.contains_key(path),
            Found::Books(b) => b.chosen.contains_key(path),
            Found::Comics(c) => c.chosen.contains_key(path),
        }
    }

    /// Whether a file of this extension is one the rule looks up.
    pub fn wanted(&self, ext: &str) -> bool {
        match self {
            Found::Music(_) => renome_core::music::is_audio(ext),
            Found::Books(_) => renome_core::ebook::is_book(ext),
            Found::Comics(_) => renome_core::comics::is_comic(ext),
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
    /// Each album's folder, its release's tracks and its songs.
    pub tracklists: Vec<(
        PathBuf,
        Vec<renome_core::musicbrainz::ListedTrack>,
        Vec<PathBuf>,
    )>,
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
    let items: Vec<flow::Item> = preview.items.iter().map(flow::Item::from).collect();
    let wanted: std::collections::HashSet<PathBuf> =
        flow::music::targets(&items, listen, &Default::default())
            .into_iter()
            .collect();
    let list: Vec<&renome_core::FileEntry> =
        files.iter().filter(|f| wanted.contains(&f.path)).collect();
    let services = mbs::Services {
        mb: &mb,
        dg: &dg,
        acoustid: &acoustid,
        heard: &heard,
    };
    let found = flow::music::search(&Cli, &services, &list, from_name, listen)
        .await
        .map_err(|e| crate::text::error_text(&e))?;
    let mut out_music = Music {
        chosen: HashMap::new(),
        albums: Vec::new(),
        tracklists: Vec::new(),
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
            out_music.tracklists.push((
                album.dir.clone(),
                album.tracks.clone(),
                album.files.clone(),
            ));
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
        let chosen = flow::music::choose(&services, &id, &list, from_name)
            .await
            .map_err(|e| crate::text::error_text(&e))?;
        for (p, m) in chosen.matches {
            out_music.chosen.insert(p, m);
        }
        out_music
            .tracklists
            .push((album.dir.clone(), chosen.tracks, album.files.clone()));
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

/// What the book rule found and chose.
pub struct Books {
    pub ol: renome_services::openlibrary::Client,
    /// What each book (and audiobook's first file) was guessed as, and
    /// the one chosen.
    found: HashMap<PathBuf, (renome_core::books::Guess, renome_core::books::Candidate)>,
    parts: HashMap<PathBuf, flow::books::Part>,
    /// Each file's book, as the rule names it.
    pub chosen: HashMap<PathBuf, renome_core::books::BookMatch>,
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
    let audiobooks = rule
        .get("audiobooks")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let items: Vec<flow::Item> = preview.items.iter().map(flow::Item::from).collect();
    let found = flow::books::search(
        &Cli,
        &client,
        &lang,
        &items,
        &Default::default(),
        audiobooks,
    )
    .await
    .map_err(|e| crate::text::error_text(&e))?;
    let mut out_books = Books {
        ol: ol::Client::default(),
        found: HashMap::new(),
        parts: found.parts,
        chosen: HashMap::new(),
    };
    // Each group asked once: one book in several formats.
    let mut answered: HashMap<String, Option<renome_core::books::Candidate>> = HashMap::new();
    for f in found.found {
        let pick = if let Some(text) = ask.picks.get(&f.path) {
            book_search(&client, &lang, text).await?.into_iter().next()
        } else if f.choice.is_some() {
            f.choice
        } else if ask.choose == Choose::Ask && std::io::stdin().is_terminal() {
            match answered.get(&f.group) {
                Some(p) => p.clone(),
                None => {
                    let p = choose_book(&client, &lang, &f.path, &f.guess, f.candidates).await?;
                    answered.insert(f.group.clone(), p.clone());
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
    let chosen: Vec<flow::books::Chosen> = out_books
        .found
        .iter()
        .map(|(p, (g, c))| flow::books::Chosen {
            path: p.clone(),
            guess: g.clone(),
            choice: c.clone(),
        })
        .collect();
    out_books.chosen = flow::books::matches(&chosen, &out_books.parts, audiobooks);
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

/// A comic series looked up: its details, its issues (none for a manga
/// catalogue), and its files' issue ids with their own details.
pub type ComicSeries = (
    renome_core::comics::Volume,
    Vec<renome_core::comics::Issue>,
    Vec<(String, renome_core::comics::ComicMatch)>,
);

/// What the comic rule found and chose.
pub struct Comics {
    pub chosen: HashMap<PathBuf, renome_core::comics::ComicMatch>,
    /// Each series looked up: its details, its issues (none for a manga
    /// catalogue), and its files' issue ids with their own details.
    pub listed: Vec<ComicSeries>,
    /// Each matched file's catalogue page and whether it is a manga
    /// catalogue's (for `ComicInfo.xml`).
    web: HashMap<PathBuf, (String, bool)>,
}

fn volume_line(v: &renome_core::comics::Volume) -> String {
    let mut details = Vec::new();
    for s in [&v.publisher, &v.country, &v.format] {
        if !s.is_empty() {
            details.push(s.clone());
        }
    }
    if v.count > 0 {
        details.push(t("comicIssuesN", &[("n", &v.count)]));
    }
    let year = v.start_year.map(|y| format!(" ({y})")).unwrap_or_default();
    format!(
        "{}{year}{}",
        v.name,
        out::dim(&format!(" — {}", details.join(", ")))
    )
}

/// Looks up the comics' series and issues in the rule's catalogue
/// (Metron by barcode first, with the rule's option), the doubtful series
/// and issues asked in the terminal.
pub async fn comics(
    preview: &Preview,
    rule: &Value,
    settings: Option<&Value>,
    ask: &Ask,
) -> Result<Comics, String> {
    use renome_services::{
        anilist, comics as shared, comicvine, gcd, mangadex, mangaupdates, metron,
    };
    let source = rule
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("gcd")
        .to_string();
    let typed = rule
        .get("series")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let skip = rule
        .get("skipPublishers")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let by_folder = rule
        .get("seriesByFolder")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let secrets = renome_services::load_secrets();
    let secret = |k: &str| {
        secrets
            .get("secrets")
            .and_then(|s| s.get(k))
            .or_else(|| settings.and_then(|s| s.get(k)))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let key = match source.as_str() {
        "comicvine" => secret("comicVineKey"),
        "metron" => secret("metronToken"),
        _ => String::new(),
    };
    if matches!(source.as_str(), "comicvine" | "metron") && key.is_empty() {
        return Err(t(
            if source == "metron" {
                "mtNeedsKey"
            } else {
                "cvNeedsKey"
            },
            &[],
        ));
    }
    let (cv, mt, gcd_c, md, al, mu) = (
        comicvine::Client::default(),
        metron::Client::default(),
        gcd::Client::default(),
        mangadex::Client::default(),
        anilist::Client::default(),
        mangaupdates::Client::default(),
    );
    gcd_c.set_account(&secret("gcdEmail"), &secret("gcdPassword"));
    let sources = shared::Sources {
        cv: &cv,
        mt: &mt,
        gcd: &gcd_c,
        md: &md,
        al: &al,
        mu: &mu,
    };
    let params = flow::comics::Params {
        source: source.clone(),
        key: key.clone(),
        series: typed.clone(),
        skip: skip.clone(),
        lang: if lang() == "pt" {
            "pt-br".to_string()
        } else {
            lang().to_string()
        },
        ui_lang: lang().to_string(),
        by_folder,
        barcode: rule
            .get("barcode")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    let items: Vec<flow::Item> = preview.items.iter().map(flow::Item::from).collect();
    // A series forced with `--pick` for a file: its group's.
    let mut picked = HashMap::new();
    for i in &items {
        let Some(text) = ask.picks.get(&i.path) else {
            continue;
        };
        let stem = i.lookup_name.as_ref().map(|_| i.read_stem());
        let Some(own) = renome_core::comics::from_path(&i.path, stem.as_deref(), by_folder) else {
            continue;
        };
        let query = if typed.is_empty() {
            own.series
        } else {
            typed.clone()
        };
        let found = sources
            .search(&source, &key, text, None, "", &params.lang, &skip)
            .await
            .map_err(|e| crate::text::error_text(&e))?;
        if let Some(v) = found.into_iter().next() {
            picked.insert(flow::comics::picked_key(&source, &query), v);
        }
    }
    let tv = tmdb::Client::new("", "");
    let al_tv = renome_services::anilist::Client::default();
    let host = Asker {
        client: &tv,
        anilist: &al_tv,
        comics: Some((&sources, &params)),
        ask,
        quit: Arc::default(),
    };
    let found = flow::comics::search(
        &host,
        &sources,
        &params,
        &items,
        &Default::default(),
        picked,
    )
    .await
    .map_err(|e| crate::text::error_text(&e))?;
    if host.quit.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(t("searchStopped", &[]));
    }
    // The doubtful issues asked (or left).
    let mut settled = Vec::new();
    for f in found.files {
        let choice = if f.sure {
            Some(f.choice)
        } else if ask.choose == Choose::Ask && std::io::stdin().is_terminal() {
            let issues = found
                .volumes
                .get(&f.volume_key)
                .map_or(&[][..], |l| &l.issues[..]);
            choose_issue(&f.path, &f.own, issues)?
        } else {
            None
        };
        settled.push(flow::comics::Settled {
            path: f.path,
            volume_key: f.volume_key,
            own: f.own,
            choice,
        });
    }
    let named = flow::comics::named(&settled, &found.volumes);
    let mut out_comics = Comics {
        chosen: HashMap::new(),
        listed: Vec::new(),
        web: HashMap::new(),
    };
    for (key, listed) in &found.volumes {
        let held: Vec<(String, renome_core::comics::ComicMatch)> = settled
            .iter()
            .filter(|s| &s.volume_key == key && named.contains_key(&s.path))
            .filter_map(|s| {
                let id = s.choice.clone()?;
                (listed.issues.is_empty() || !id.is_empty()).then(|| (id, s.own.clone()))
            })
            .collect();
        out_comics
            .listed
            .push((listed.volume.clone(), listed.issues.clone(), held));
    }
    for (p, n) in named {
        out_comics.web.insert(p.clone(), (n.web, n.manga));
        out_comics.chosen.insert(p, n.comic);
    }
    Ok(out_comics)
}

/// A comic series, in the terminal; a new search with the words typed.
async fn choose_volume(
    sources: &renome_services::comics::Sources<'_>,
    p: &flow::comics::Params,
    query: &str,
    files: usize,
    mut candidates: Vec<renome_core::comics::Volume>,
) -> Answer<renome_core::comics::Volume> {
    loop {
        let lines: Vec<String> = candidates.iter().map(volume_line).collect();
        match ask_one(
            &format!(
                "{} ({})",
                t("comicPickTitle", &[]),
                t("files", &[("n", &files)])
            ),
            query,
            &lines,
        ) {
            Pick::Number(n) => return Answer::Pick(candidates.remove(n)),
            Pick::Skip => return Answer::Skip,
            Pick::Stop => return Answer::Stop,
            Pick::Search(text) => match sources
                .search(&p.source, &p.key, &text, None, "", &p.lang, &p.skip)
                .await
            {
                Ok(v) => candidates = v,
                Err(e) => out::error(&crate::text::error_text(&e)),
            },
        }
    }
}

/// A comic's issue, in the terminal: those near the number its name
/// says, or the number typed.
fn choose_issue(
    path: &Path,
    own: &renome_core::comics::ComicMatch,
    issues: &[renome_core::comics::Issue],
) -> Result<Option<String>, String> {
    let wanted: f64 = own.number.parse().unwrap_or(0.0);
    let mut near: Vec<&renome_core::comics::Issue> = issues.iter().collect();
    near.sort_by(|a, b| {
        let d = |i: &renome_core::comics::Issue| {
            (i.number.parse::<f64>().unwrap_or(f64::MAX) - wanted).abs()
        };
        d(a).total_cmp(&d(b))
    });
    near.truncate(9);
    let lines: Vec<String> = near
        .iter()
        .map(|i| {
            format!(
                "#{}{}{}",
                i.number,
                if i.name.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", i.name)
                },
                out::dim(&format!(" ({})", i.cover_date))
            )
        })
        .collect();
    let title = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let looked = format!("{} #{}", own.series, own.number);
    loop {
        match ask_one(&title, &looked, &lines) {
            Pick::Number(n) => return Ok(Some(near[n].id.clone())),
            Pick::Skip => return Ok(None),
            Pick::Stop => return Err(t("searchStopped", &[])),
            Pick::Search(text) => {
                let number = text.trim().trim_start_matches('#');
                match issues.iter().find(|i| i.number == number) {
                    Some(i) => return Ok(Some(i.id.clone())),
                    None => out::error(&t("reviewNoResults", &[])),
                }
            }
        }
    }
}

impl Books {
    /// Each book's (and audiobook's first file's) guess and choice.
    pub fn guesses(
        &self,
    ) -> impl Iterator<
        Item = (
            &PathBuf,
            &(renome_core::books::Guess, renome_core::books::Candidate),
        ),
    > {
        self.found.iter()
    }
}

/// What the lookups chose, for what is written after the renames (the
/// shared flow decides it, as for the window). `typed`: the files whose
/// details were typed in (`--edit`).
pub fn chosen(
    found: Option<&Found>,
    typed: &std::collections::HashSet<PathBuf>,
) -> flow::after::Chosen {
    let mut c = flow::after::Chosen::default();
    match found {
        Some(Found::Movies(m)) => {
            c.movies = m.chosen.iter().map(|(p, x)| (p.clone(), x.id)).collect();
        }
        Some(Found::Series(s)) => c.episodes = s.chosen.clone(),
        Some(Found::Music(m)) => {
            c.songs = m.chosen.clone();
            c.albums = m
                .albums
                .iter()
                .map(|(folder, release)| {
                    let mut songs: Vec<PathBuf> = m
                        .chosen
                        .iter()
                        .filter(|(p, s)| &s.release == release && p.starts_with(folder))
                        .map(|(p, _)| p.clone())
                        .collect();
                    songs.sort();
                    (release.clone(), songs)
                })
                .collect();
        }
        Some(Found::Books(b)) => {
            c.books = b.chosen.clone();
            c.book_found = b
                .found
                .iter()
                .map(|(p, (guess, choice))| {
                    (
                        p.clone(),
                        flow::after::BookFound {
                            guess: guess.clone(),
                            choice: choice.clone(),
                        },
                    )
                })
                .collect();
            c.book_parts = b.parts.clone();
            c.book_typed = typed.clone();
        }
        Some(Found::Comics(k)) => {
            c.comics = k
                .chosen
                .iter()
                .map(|(p, comic)| {
                    let (web, manga) = if typed.contains(p) {
                        Default::default()
                    } else {
                        k.web.get(p).cloned().unwrap_or_default()
                    };
                    (
                        p.clone(),
                        flow::comics::Named {
                            comic: comic.clone(),
                            web,
                            manga,
                        },
                    )
                })
                .collect();
        }
        None => {}
    }
    c
}

/// The subtitle services' keys, from the system's password store (or the
/// options file).
pub fn subs_auth(settings: Option<&Value>) -> renome_services::subtitles::Auth {
    let secrets = renome_services::load_secrets();
    let secret = |k: &str| {
        secrets
            .get("secrets")
            .and_then(|s| s.get(k))
            .or_else(|| settings.and_then(|s| s.get(k)))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    renome_services::subtitles::Auth {
        os_key: secret("osKey"),
        os_user: secret("osUser"),
        os_password: secret("osPassword"),
        subdl_key: secret("subdlKey"),
        subsource_key: secret("subsourceKey"),
        tmdb_key: secret("tmdbKey"),
    }
}

/// With the movie or series rule's `subtitles`: each identified video's
/// subtitles in the rule's languages searched before renaming, the best
/// one per language picked (as the window's review picks them) for
/// downloading once it is renamed.
pub async fn subtitles(
    rule: &Value,
    preview: &Preview,
    auth: &renome_services::subtitles::Auth,
    chosen: &mut flow::after::Chosen,
) {
    if !rule
        .get("subtitles")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return;
    }
    if auth.os_key.is_empty()
        && auth.subdl_key.is_empty()
        && (auth.subsource_key.is_empty() || auth.tmdb_key.is_empty())
    {
        out::error(&t("subsNeedKey", &[]));
        return;
    }
    let langs = rule
        .get("subLangs")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let files: Vec<flow::subtitles::Identified> = preview
        .items
        .iter()
        .filter(|i| i.status != renome_core::Status::Conflict)
        .map(|i| flow::subtitles::Identified {
            path: i.path.clone(),
            original: i.original.clone(),
            is_dir: i.is_dir,
            movie: chosen.movies.get(&i.path).copied(),
            episode: chosen.episodes.get(&i.path).cloned(),
        })
        .collect();
    let items = flow::subtitles::targets(&files);
    if items.is_empty() {
        return;
    }
    let searched =
        flow::subtitles::search(&Cli, auth.clone(), &langs, items, crate::stop::requested).await;
    for e in &searched.errors {
        out::error(&crate::text::error_text(e));
    }
    chosen.sub_langs = searched.langs;
    for f in searched.found {
        chosen.subtitles.insert(
            f.found.path,
            flow::after::SubsFound {
                have: f.found.have,
                candidates: f.found.candidates,
                pick: f.pick,
            },
        );
    }
}

/// The lines asked about before renaming, for what is written besides
/// the renames (as the window's question says them).
pub fn planned(c: &flow::after::Counts) -> Vec<String> {
    let line = |key: &str, n: usize| (n > 0).then(|| t(key, &[("n", &n)]));
    [
        line("confirmMeta", c.sidecars),
        line("confirmBookMeta", c.book_meta),
        line("confirmCovers", c.covers),
        line("confirmPlaylists", c.playlists),
        line("confirmTags", c.tags),
        line("confirmInside", c.inside + c.video_tags),
        line("confirmSubTools", c.sub_tools),
        line("confirmSubs", c.subtitles),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Everything written once the files are renamed, by the shared flow (as
/// in the window); what it did, as notices, and what failed, as errors.
#[allow(clippy::too_many_arguments)]
pub async fn after(
    rules: &[Value],
    found: Option<&Found>,
    preview: &Preview,
    chosen: &flow::after::Chosen,
    report: &renome_core::ApplyReport,
    history: &renome_core::History,
    settings: Option<&Value>,
    auth: renome_services::subtitles::Auth,
) -> Vec<String> {
    let moved: HashMap<PathBuf, PathBuf> = report
        .done
        .iter()
        .map(|op| (op.from.clone(), op.to.clone()))
        .collect();
    let jobs = flow::after::plan(rules, preview, chosen, &moved);
    let (tmdb_key, tmdb_lang) = match found {
        Some(Found::Movies(m)) => (m.key.clone(), m.lang.clone()),
        Some(Found::Series(s)) => (s.key.clone(), s.lang.clone()),
        _ => tmdb_access(settings).unwrap_or_default(),
    };
    let (own_mb, own_dg);
    let (mb, dg) = match found {
        Some(Found::Music(m)) => (&m.mb, &m.dg),
        _ => {
            own_mb = renome_services::musicbrainz::Client::default();
            own_dg = renome_services::discogs::Client::default();
            (&own_mb, &own_dg)
        }
    };
    let own_ol;
    let ol = match found {
        Some(Found::Books(b)) => &b.ol,
        _ => {
            own_ol = renome_services::openlibrary::Client::default();
            &own_ol
        }
    };
    let services = flow::after::Services {
        tmdb_key,
        tmdb_lang,
        mb,
        dg,
        ol,
        subs: auth,
    };
    let r = flow::after::run(
        &Cli,
        history,
        report.history_id.clone(),
        !report.done.is_empty(),
        rules,
        jobs,
        &services,
    )
    .await;
    for trouble in &r.troubles {
        for (file, why) in &trouble.errors {
            let text = if file.is_empty() {
                crate::text::error_text(why)
            } else {
                format!("{file}: {}", crate::text::error_text(why))
            };
            // Saved all the same, maybe out of time: a warning, not an error.
            if why == "subsLengthKept" {
                out::note(&out::yellow(&text));
            } else {
                out::error(&text);
            }
        }
    }
    let line = |key: &str, n: usize| (n > 0).then(|| t(key, &[("n", &n)]));
    [
        line("adsTrashedN", r.ads_trashed),
        line("adsFailedN", r.ads_failed),
        line("sidecarsN", r.sidecars),
        line("subsSavedN", r.subtitles),
        line("subToolsN", r.sub_fixed),
        line("subToolsM", r.sub_extracted),
        line("playlistsKeptN", r.kept_lists),
        line("tagsN", r.tags),
        line("audioTagsN", r.audio_tags),
        line("insideN", r.inside),
        line("insideDrawnN", r.drawn),
    ]
    .into_iter()
    .flatten()
    .collect()
}
