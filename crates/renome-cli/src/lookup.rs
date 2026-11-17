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
        let key = match event {
            "tmdb-progress" => "tmdbSearching",
            "inside-progress" => "insideWriting",
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
