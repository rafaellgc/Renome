//! The lookup rules on the command line: the catalogues searched as the
//! window's search buttons do, the doubtful files asked about in the
//! terminal (or left, or forced with `--pick`), and what runs after the
//! renames.

use std::collections::{HashMap, HashSet};
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
fn tmdb_access(settings: Option<&Value>) -> (String, String) {
    let secrets = renome_services::load_secrets();
    let key = secrets
        .get("secrets")
        .and_then(|s| s.get("tmdbKey"))
        .or_else(|| settings.and_then(|s| s.get("tmdbKey")))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let tmdb_lang = renome_services::options::tmdb_lang(settings, lang());
    (key, tmdb_lang)
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
    let (key, tmdb_lang) = tmdb_access(settings);
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
    // An answer goes to the undecided files of its group too (a movie's
    // subtitles: see `flow::same_answer`, the window's too).
    let states: Vec<(PathBuf, String, flow::Status)> = found
        .iter()
        .map(|f| (f.path.clone(), f.group.clone(), f.status))
        .collect();
    let mut answered: HashSet<PathBuf> = HashSet::new();
    let mut stop = false;
    for f in found {
        if let Some(c) = f.choice {
            chosen.insert(f.path, c);
            continue;
        }
        if stop || ask.choose != Choose::Ask || answered.contains(&f.path) {
            continue;
        }
        let pick = match choose_movie(&client, &f.path, &f.guess, f.candidates).await? {
            Answer::Pick(c) => Some(c),
            Answer::Skip => None,
            Answer::Stop => {
                stop = true;
                None
            }
        };
        let takers = flow::same_answer(
            &f.path,
            states.iter().map(|(p, g, s)| (p.as_path(), g.as_str(), *s)),
        );
        for p in takers {
            if let Some(c) = &pick {
                chosen.insert(p.clone(), c.clone());
            }
            answered.insert(p);
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
    let chosen = flow::movies::named(chosen, &extras);
    Ok(Movies {
        key,
        lang: tmdb_lang,
        chosen,
        sets: ask.missing.then_some(sets),
    })
}

/// A search typed by the person: a TMDB or IMDb link or id, else a title
/// A search typed by the person (see `flow::typed`, the window's too).
async fn typed_search(client: &tmdb::Client, text: &str) -> Result<Vec<Candidate>, String> {
    flow::typed::movie(client, text, None)
        .await
        .map_err(|e| crate::text::error_text(&e))
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

/// `S01E05`, `1x05`, `S01E05-E06` read as episode keys of the show (see
/// `flow::series::typed`, the window's review too).
fn typed_episodes(text: &str, show: &renome_core::tv::Show) -> Option<Vec<renome_core::tv::Key>> {
    flow::series::typed(show, text).exact
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
                    highest,
                    candidates,
                    path,
                    comic,
                    ..
                } => {
                    let (sources, p) = self.comics?;
                    let file = path.zip(comic).map(|(p, c)| (p, *c));
                    choose_volume(
                        sources,
                        p,
                        &query,
                        files,
                        highest,
                        candidates,
                        file.as_ref(),
                    )
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
        tmdb_access(settings)
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
    // An answer goes to the undecided files of its group too (an episode's
    // subtitles: see `flow::same_answer`, the window's too).
    let states: Vec<(PathBuf, String, flow::Status)> = found
        .files
        .iter()
        .map(|f| (f.path.clone(), f.group.clone(), f.status))
        .collect();
    let mut answered: HashMap<PathBuf, Option<Vec<renome_core::tv::Key>>> = HashMap::new();
    for f in found.files {
        let Some(show) = out_series.shows.iter().find(|s| s.id == f.show_id) else {
            continue;
        };
        let keys = if let Some(pick) = ask.picks.get(&f.path) {
            typed_episodes(pick, show)
        } else if !f.choice.is_empty() {
            Some(f.choice.clone())
        } else if let Some(k) = answered.get(&f.path) {
            k.clone()
        } else if ask.choose == Choose::Ask && std::io::stdin().is_terminal() {
            let k = choose_episode(&f.path, show, &f.alternatives, &f.read)?;
            let takers = flow::same_answer(
                &f.path,
                states.iter().map(|(p, g, s)| (p.as_path(), g.as_str(), *s)),
            );
            for p in takers {
                answered.insert(p, k.clone());
            }
            k
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
                let found = flow::typed::show(client, anime.then_some(anilist), &q, &[]).await;
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
    read: &str,
) -> Result<Option<Vec<renome_core::tv::Key>>, String> {
    // As the window's review: the code, then the episodes' names.
    let label = |keys: &Vec<renome_core::tv::Key>| {
        let names: Vec<String> = keys
            .iter()
            .filter_map(|k| {
                show.episodes
                    .iter()
                    .find(|e| e.season == k.0 && e.number == k.1)
            })
            .map(|e| e.name.clone())
            .collect();
        format!("{} {}", flow::series::code(keys), names.join(" & "))
    };
    let mut alternatives: Vec<Vec<renome_core::tv::Key>> = alternatives.to_vec();
    let mut options: Vec<String> = alternatives.iter().map(label).collect();
    let title = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let looked = if read.is_empty() {
        format!("{} — {}", show.name, t("epTypeHint", &[]))
    } else {
        format!("{} — {read} — {}", show.name, t("epTypeHint", &[]))
    };
    loop {
        match ask_one(&title, &looked, &options) {
            Pick::Number(n) => return Ok(Some(alternatives[n].clone())),
            Pick::Skip => return Ok(None),
            Pick::Stop => return Err(t("searchStopped", &[])),
            Pick::Search(text) => {
                // A code: that episode; words: the episodes they name.
                let typed = flow::series::typed(show, &text);
                match typed.exact {
                    Some(keys) => return Ok(Some(keys)),
                    None if !typed.matches.is_empty() => {
                        alternatives = typed.matches.iter().take(9).map(|k| vec![*k]).collect();
                        options = alternatives.iter().map(label).collect();
                    }
                    None => out::error(&t("epTypeBad", &[("text", &text)])),
                }
            }
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
                let query = renome_core::musicbrainz::AlbumQuery {
                    album: text.clone(),
                    ..album.query.clone()
                };
                let first = album
                    .choice
                    .as_ref()
                    .or(album.candidates.first())
                    .map(|c| c.id.as_str());
                let on_discogs = flow::typed::album_on_discogs(first, dg.ready());
                let found = flow::typed::album(mb, dg, &query, on_discogs).await;
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
    pub parts: HashMap<PathBuf, flow::books::Part>,
    /// What the lookup guessed of each book (chosen or not).
    pub guesses: HashMap<PathBuf, renome_core::books::Guess>,
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
    let lang = renome_services::options::tmdb_lang(settings, lang());
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
        guesses: found
            .found
            .iter()
            .map(|f| (f.path.clone(), f.guess.clone()))
            .collect(),
        chosen: HashMap::new(),
    };
    // Each group asked once: one book in several formats.
    let states: Vec<(PathBuf, String, flow::Status)> = found
        .found
        .iter()
        .map(|f| (f.path.clone(), f.group.clone(), f.status))
        .collect();
    let mut answered: HashMap<PathBuf, Option<renome_core::books::Candidate>> = HashMap::new();
    for f in found.found {
        let pick = if let Some(text) = ask.picks.get(&f.path) {
            book_search(&client, &lang, text).await?.into_iter().next()
        } else if f.choice.is_some() {
            f.choice
        } else if let Some(p) = answered.get(&f.path) {
            p.clone()
        } else if ask.choose == Choose::Ask && std::io::stdin().is_terminal() {
            let p = choose_book(&client, &lang, &f.path, &f.guess, f.candidates).await?;
            let takers = flow::same_answer(
                &f.path,
                states.iter().map(|(q, g, s)| (q.as_path(), g.as_str(), *s)),
            );
            for q in takers {
                answered.insert(q, p.clone());
            }
            p
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
    flow::typed::book(client, lang, text, "")
        .await
        .map_err(|e| crate::text::error_text(&e))
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

/// What the comic rule found and chose.
pub struct Comics {
    pub chosen: HashMap<PathBuf, renome_core::comics::ComicMatch>,
    /// Each comic's series and issue, and the series with their issues.
    pub settled: Vec<flow::comics::Settled>,
    pub volumes: HashMap<String, flow::comics::Listed>,
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
        "comicvine" | "metron" => secret(renome_services::options::comic_key_name(&source)),
        _ => String::new(),
    };
    let (cv, mt, gcd_c, md, al, mu) = (
        comicvine::Client::default(),
        metron::Client::default(),
        gcd::Client::default(),
        mangadex::Client::default(),
        anilist::Client::default(),
        mangaupdates::Client::default(),
    );
    gcd_c.set_account(&secret("gcdEmail"), &secret("gcdPassword"));
    // Without an account the GCD takes few searches (the window says so
    // once a session).
    if renome_services::options::gcd_few(&source, &secret("gcdEmail"), &secret("gcdPassword")) {
        out::note(&out::yellow(&t("gcdNoAccount", &[])));
    }
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
        lang: renome_services::options::comic_lang(lang()),
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
            choose_issue(&f.path, &f.own, issues).await?
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
        web: HashMap::new(),
        settled,
        volumes: found.volumes,
    };
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
    highest: f64,
    mut candidates: Vec<renome_core::comics::Volume>,
    file: Option<&(PathBuf, renome_core::comics::ComicMatch)>,
) -> Answer<renome_core::comics::Volume> {
    use renome_services::covers;
    // The file's cover compared with the series' (as in the window's
    // picker): those with it first, marked.
    let own = file.and_then(|(path, _)| covers::own(path));
    let cache = covers::Covers::default();
    let compare = |list: Vec<renome_core::comics::Volume>| async {
        match (own, file) {
            (Some(own), Some((_, comic))) => {
                let r = covers::series_by_cover(
                    &cache,
                    sources,
                    &p.source,
                    &p.key,
                    own,
                    comic,
                    list,
                    &|i, n| out::note(&t("coverPickComparing", &[("i", &i), ("n", &n)])),
                )
                .await;
                (r.candidates, r.same)
            }
            _ => (list, Default::default()),
        }
    };
    let mut far;
    (candidates, far) = compare(candidates).await;
    loop {
        let lines: Vec<String> = candidates
            .iter()
            .map(|v| {
                let same = far.contains_key(&v.id);
                if same {
                    format!(
                        "{} {}",
                        volume_line(v),
                        out::green(&format!("({})", t("sameCover", &[])))
                    )
                } else {
                    volume_line(v)
                }
            })
            .collect();
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
            Pick::Search(text) => match flow::typed::comic(
                sources, &p.source, &p.key, &text, "", "", &p.lang, &p.skip, highest,
            )
            .await
            {
                Ok(v) => (candidates, far) = compare(v).await,
                Err(e) => out::error(&crate::text::error_text(&e)),
            },
        }
    }
}

/// A comic's issue, in the terminal: in the order the window's review
/// shows them (those with the file's cover first, then those nearest its
/// number: see `covers::issue_order`), or the number typed.
async fn choose_issue(
    path: &Path,
    own: &renome_core::comics::ComicMatch,
    issues: &[renome_core::comics::Issue],
) -> Result<Option<String>, String> {
    use renome_services::covers;
    let mut far = std::collections::HashMap::new();
    let urls = covers::issue_covers(issues);
    // Few covers: compared at once (as the window's review does).
    if !urls.is_empty()
        && urls.len() <= covers::COMPARE_AT_ONCE
        && let Some(hash) = covers::own(path)
    {
        let d = covers::Covers::default().distances(hash, &urls).await;
        far = urls.into_iter().zip(d).collect();
    }
    let line = |i: &renome_core::comics::Issue| {
        let same = far
            .get(&i.cover)
            .copied()
            .flatten()
            .is_some_and(|d| d <= covers::SAME_COVER);
        format!(
            "#{}{}{}{}",
            i.number,
            if i.name.is_empty() {
                String::new()
            } else {
                format!(" · {}", i.name)
            },
            out::dim(&format!(" ({})", i.cover_date)),
            if same {
                format!(" {}", out::green(&format!("({})", t("sameCover", &[]))))
            } else {
                String::new()
            }
        )
    };
    let mut near: Vec<usize> = covers::issue_order(issues, own, &far, "");
    near.truncate(9);
    let mut lines: Vec<String> = near.iter().map(|&i| line(&issues[i])).collect();
    let title = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let looked = flow::comics::read(own);
    loop {
        match ask_one(&title, &looked, &lines) {
            Pick::Number(n) => return Ok(Some(issues[near[n]].id.clone())),
            Pick::Skip => return Ok(None),
            Pick::Stop => return Err(t("searchStopped", &[])),
            Pick::Search(text) => {
                if let Some(i) = covers::issue_typed(issues, &text) {
                    return Ok(Some(issues[i].id.clone()));
                }
                // Words: the issues they name, to choose from.
                let found = covers::issue_order(issues, own, &far, &text);
                if found.is_empty() {
                    out::error(&t("reviewNoResults", &[]));
                } else {
                    near = found.into_iter().take(9).collect();
                    lines = near.iter().map(|&i| line(&issues[i])).collect();
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
    rules: &[Value],
    preview: &Preview,
    auth: &renome_services::subtitles::Auth,
    chosen: &mut flow::after::Chosen,
) {
    // Which languages, and whether at all, the shared flow says.
    let Some(langs) = flow::subtitles::wanted(rules) else {
        return;
    };
    if !auth.ready() {
        out::error(&t("subsNeedKey", &[]));
        return;
    }
    let files: Vec<flow::subtitles::Identified> = preview
        .items
        .iter()
        .map(|i| flow::subtitles::Identified {
            path: i.path.clone(),
            original: i.original.clone(),
            is_dir: i.is_dir,
            movie: chosen.movies.get(&i.path).copied(),
            episode: chosen.episodes.get(&i.path).cloned(),
            conflict: i.status == renome_core::Status::Conflict,
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
    // More picked on OpenSubtitles than today's quota: the rest come from
    // SubDL, if any (the window's review says the same).
    let standing: Vec<flow::subtitles::Standing> = searched
        .found
        .iter()
        .map(|f| flow::subtitles::Standing {
            have: f.found.have.clone(),
            inside: f.found.inside.clone(),
            candidates: f.found.candidates.clone(),
            pick: f.pick.clone(),
        })
        .collect();
    let tally = flow::subtitles::tally(&standing, &searched.langs, searched.remaining);
    if let (true, Some(n)) = (tally.over_quota, searched.remaining) {
        out::note(&out::yellow(&format!(
            "{} · {}",
            t("subsRemaining", &[("n", &n)]),
            t("subsOverQuota", &[])
        )));
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
pub fn planned(ask: &flow::after::Ask) -> Vec<String> {
    let mut out = said(&ask.lines);
    if !ask.left.is_empty() {
        let suffix = t("leftoverSuffix", &[]);
        out.push(t(
            "confirmLeftoverHead",
            &[("n", &ask.left.len()), ("suffix", &suffix)],
        ));
        for f in &ask.left {
            out.push(format!(
                "  {} ({}) → {}",
                crate::out::path(&f.from),
                t("files", &[("n", &f.files)]),
                crate::out::path(&f.to)
            ));
        }
    }
    out
}

/// Lines said of the work (see `flow::after::{Ask, Counts, Report}`).
pub fn said(lines: &[flow::after::Line]) -> Vec<String> {
    lines
        .iter()
        .map(|l| {
            let mut vars: Vec<(&str, &dyn std::fmt::Display)> = vec![("n", &l.n)];
            vars.extend(
                l.more
                    .iter()
                    .map(|(k, v)| (*k, v as &dyn std::fmt::Display)),
            );
            t(l.key, &vars)
        })
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
        _ => tmdb_access(settings),
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
    said(&r.lines)
}
