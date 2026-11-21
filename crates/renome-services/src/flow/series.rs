//! The TV show rule: the show typed in the rule, or each show the names
//! and folders tell, is found on TMDB or AniList (asked when unsure), then
//! each video's and subtitle's episode; the files without a sure episode
//! are doubts.

use super::{Item, Question};
use crate::tmdb::{self, TvItem};
use crate::{Host, anilist};
use renome_core::tmdb::Candidate;
use renome_core::tv::{EpisodeGuess, Key, Show};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Where the shows come from.
pub enum Source<'a> {
    Tmdb(&'a tmdb::Client),
    /// Anime, with no key.
    Anilist(&'a anilist::Client),
}

impl Source<'_> {
    pub fn name(&self) -> &'static str {
        match self {
            Source::Tmdb(_) => "tmdb",
            Source::Anilist(_) => "anilist",
        }
    }
}

/// What was found for a file.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub path: PathBuf,
    /// The show's id in its source.
    pub show_id: u64,
    pub guess: EpisodeGuess,
    /// Likely episodes, best first.
    pub alternatives: Vec<Vec<Key>>,
    /// The numbers in the name point to another episode than its title.
    pub conflict: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doubled: Option<u32>,
    /// Picked with confidence; empty: a doubt.
    pub choice: Vec<Key>,
    /// Doubts of one group share one answer (an episode and its
    /// subtitles).
    pub group: String,
    pub status: super::Status,
    /// What was read from its name (see [`read`]).
    pub read: String,
}

/// What the search came to.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// No video or subtitle in the list.
    pub empty: bool,
    /// The shows fetched, with their episodes.
    pub shows: Vec<Show>,
    pub files: Vec<Found>,
    /// The shows of this search.
    pub found: Vec<Candidate>,
    /// Some file was looked for in a show (none: all were settled).
    pub searched: bool,
    /// Files whose show could not be told.
    pub loose: usize,
    /// The shows picked, by source and what they were found by: not asked
    /// again (the ones given, with this search's).
    pub picked: HashMap<String, Candidate>,
}

/// The files the rule looks up: videos and subtitles.
pub fn targets(items: &[Item]) -> Vec<TvItem> {
    items
        .iter()
        .filter(|i| is_target(i))
        .map(|i| TvItem {
            path: i.path.clone(),
            name: i.read_name(),
            is_dir: false,
        })
        .collect()
}

/// Whether the series rule looks this file up.
pub fn is_target(i: &Item) -> bool {
    !i.is_dir && renome_core::tmdb::is_screen(&i.ext())
}

/// Progress, for the page: finding the show, or loading one's episodes.
#[derive(Serialize)]
struct Progress<'a> {
    stage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    show: Option<&'a str>,
}

/// Looks up the episodes of `items`. `typed`: the show typed in the rule
/// (empty: each show the files tell). `settled`: files that already have
/// an episode of a show (by id), not looked up again in it. `picked`: the
/// shows picked before (see [`Outcome::picked`]).
pub async fn search(
    host: &dyn Host,
    source: Source<'_>,
    typed: &str,
    items: &[Item],
    settled: &HashSet<(PathBuf, u64)>,
    picked: HashMap<String, Candidate>,
) -> Result<Outcome, String> {
    // AniList takes no key; TMDB answers nothing without one.
    if let Source::Tmdb(client) = &source
        && !client.ready()
    {
        return Err("tmdbNeedsKey".into());
    }
    let typed = typed.trim();
    let all = targets(items);
    let mut out = Outcome {
        empty: all.is_empty(),
        picked,
        ..Outcome::default()
    };
    if all.is_empty() {
        return Ok(out);
    }
    let stop = host.stop_signal();
    let mut groups = vec![all.clone()];
    if typed.is_empty() {
        let g = tmdb::group_shows(&all);
        out.loose = g.loose.len();
        let by_path: HashMap<&PathBuf, &TvItem> = all.iter().map(|i| (&i.path, i)).collect();
        if !g.groups.is_empty() {
            groups = g
                .groups
                .iter()
                .map(|list| {
                    list.iter()
                        .filter_map(|p| by_path.get(p).map(|i| (*i).clone()))
                        .collect()
                })
                .collect();
        }
    }
    let several = groups.len() > 1;
    for group in groups {
        if stop() {
            break;
        }
        crate::report(
            host,
            "tv-progress",
            Progress {
                stage: "finding",
                show: None,
            },
        );
        let res = match &source {
            Source::Tmdb(c) => tmdb::search_show(c, typed, &group).await,
            Source::Anilist(c) => c.anime_search(typed, &group).await,
        };
        let res = match res {
            Ok(r) => r,
            // A group whose show cannot be told is skipped, not the others.
            Err(e) if several && e == "tvNoShow" => continue,
            Err(e) => return Err(e),
        };
        if stop() {
            break;
        }
        let key = format!("{}:{}", source.name(), res.query);
        let show = match out.picked.get(&key) {
            Some(c) => c.clone(),
            None => {
                let c = if res.auto {
                    res.candidates.first().cloned()
                } else {
                    let q = Question::Show {
                        source: source.name().into(),
                        query: res.query.clone(),
                        candidates: res.candidates.clone(),
                    };
                    host.ask(q)
                        .await
                        .and_then(|v| serde_json::from_value(v).ok())
                };
                let Some(c) = c else { continue };
                out.picked.insert(key, c.clone());
                c
            }
        };
        if !out.found.iter().any(|c| c.id == show.id) {
            out.found.push(show.clone());
        }
        // Files already settled for this show keep their episode.
        let group: Vec<TvItem> = group
            .into_iter()
            .filter(|i| !settled.contains(&(i.path.clone(), show.id)))
            .collect();
        if group.is_empty() || stop() {
            continue;
        }
        out.searched = true;
        crate::report(
            host,
            "tv-progress",
            Progress {
                stage: "loading",
                show: Some(&show.title),
            },
        );
        let typed_or = if typed.is_empty() {
            res.query.as_str()
        } else {
            typed
        };
        let look = match &source {
            Source::Tmdb(c) => tmdb::lookup_episodes(c, show.id, group, typed_or).await,
            Source::Anilist(c) => c.anime_lookup(show.id, &group, typed_or).await,
        }?;
        for f in look.files {
            let found = f.found;
            out.files.push(Found {
                group: format!(
                    "{}\u{1f}{}",
                    look.show.id,
                    serde_json::to_string(&found.guess).unwrap_or_default()
                ),
                status: super::Status::of(!found.choice.is_empty(), !found.alternatives.is_empty()),
                read: read(&found.guess),
                path: f.path,
                show_id: look.show.id,
                guess: found.guess,
                alternatives: found.alternatives,
                conflict: found.conflict,
                doubled: found.doubled,
                choice: found.choice,
            });
        }
        if let Some(i) = out.shows.iter().position(|s| s.id == look.show.id) {
            out.shows[i] = look.show;
        } else {
            out.shows.push(look.show);
        }
    }
    Ok(out)
}

/// An episode's template fields: the show's, its air date and its number
/// counted from the first episode (specials apart).
pub fn episode_extra(show: &Show, season: u16, number: u16) -> renome_core::fields::Extra {
    let mut extra = show.extra.clone();
    let ep = show
        .episodes
        .iter()
        .find(|e| e.season == season && e.number == number);
    if let Some(date) = ep.and_then(|e| e.air_date.clone()) {
        extra.insert("date".into(), date);
    }
    if season > 0 && ep.is_some() {
        let absolute = show
            .episodes
            .iter()
            .filter(|e| {
                e.season > 0 && (e.season < season || (e.season == season && e.number <= number))
            })
            .count();
        extra.insert("absolute".into(), absolute.to_string());
    }
    extra
}

/// A file's episodes, as the rule names it.
pub fn episode_match(show: &Show, keys: &[Key]) -> renome_core::tv::EpisodeMatch {
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

/// A file's episodes as they stand: `show`, its show's place in the list
/// of shows given with it.
#[derive(Clone, Debug, Deserialize)]
pub struct Settled {
    pub path: PathBuf,
    pub show: usize,
    pub choice: Vec<Key>,
}

/// Each file's episodes as the rule names them.
pub fn named(files: &[Settled], shows: &[Show]) -> HashMap<PathBuf, renome_core::tv::EpisodeMatch> {
    files
        .iter()
        .filter(|f| !f.choice.is_empty())
        .filter_map(|f| Some((f.path.clone(), episode_match(shows.get(f.show)?, &f.choice))))
        .collect()
}

/// Episodes named by what was typed in a review or chooser: `exact` when
/// it is a code of the show's episodes (`S01E05`, `1x05`, `S01E05-E06`);
/// `matches`, those whose name or code holds it (accents and case
/// ignored), by season (specials last, as the show lists them).
#[derive(Debug, Default, Serialize)]
pub struct Typed {
    pub exact: Option<Vec<Key>>,
    pub matches: Vec<Key>,
}

pub fn typed(show: &Show, text: &str) -> Typed {
    let fold = renome_core::text::fold;
    let q = fold(text.trim());
    let compact: String = q.chars().filter(|c| !c.is_whitespace()).collect();
    let matches = show
        .episodes
        .iter()
        .filter(|e| {
            q.is_empty()
                || std::iter::once(&e.name)
                    .chain(&e.names)
                    .any(|n| fold(n).contains(&q))
                || format!("s{:02}e{:02}", e.season, e.number).contains(&compact)
                || format!("{}x{:02}", e.season, e.number).contains(&q)
        })
        .map(|e| (e.season, e.number))
        .collect();
    let exact = read_code(&q).and_then(|(season, first, last)| {
        let keys: Vec<Key> = (first..=last).map(|n| (season, n)).collect();
        keys.iter()
            .all(|k| show.episodes.iter().any(|e| (e.season, e.number) == *k))
            .then_some(keys)
    });
    Typed { exact, matches }
}

/// Season and episode range from `S01E05`, `S1E5-E6`, `1x05`, `1x05-06`.
fn read_code(text: &str) -> Option<(u16, u16, u16)> {
    let t: String = text
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
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

/// Episodes as file names write them: `S01E02`, `S01E02-E03`.
pub fn code(keys: &[Key]) -> String {
    match keys.first() {
        None => String::new(),
        Some((season, _)) => format!(
            "S{season:02}E{}",
            keys.iter()
                .map(|k| format!("{:02}", k.1))
                .collect::<Vec<_>>()
                .join("-E")
        ),
    }
}

/// What was read from a file's name, for a review's header and the
/// chooser: its episodes (`S01E02-E03`, `E05`), number or date, and the
/// title after them; empty when nothing.
pub fn read(g: &EpisodeGuess) -> String {
    let mut parts = Vec::new();
    if !g.episodes.is_empty() {
        let eps: Vec<String> = g.episodes.iter().map(|e| format!("E{e:02}")).collect();
        parts.push(match g.season {
            Some(s) => format!("S{s:02}{}", eps.join("-")),
            None => eps.join("-"),
        });
    } else if let Some(n) = g.number {
        parts.push(n.to_string());
    } else if let Some(d) = &g.date {
        parts.push(d.clone());
    }
    if !g.title.is_empty() {
        parts.push(format!("“{}”", g.title));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod typed_tests {
    use super::*;

    #[test]
    fn codes_and_names() {
        let ep = |season, number, name: &str| renome_core::tv::Episode {
            season,
            number,
            name: name.into(),
            names: vec![],
            air_date: None,
            overview: String::new(),
            still_path: None,
            runtime: None,
        };
        let show: Show = serde_json::from_value(serde_json::json!({
            "id": 1, "name": "A", "originalName": "A", "year": null, "overview": "",
            "posterPath": null, "episodes": [],
        }))
        .unwrap();
        let show = Show {
            episodes: vec![ep(1, 1, "Começo"), ep(1, 2, "Meio"), ep(2, 1, "Volta")],
            ..show
        };
        assert_eq!(typed(&show, "S01E01-E02").exact, Some(vec![(1, 1), (1, 2)]));
        assert_eq!(typed(&show, "2x01").exact, Some(vec![(2, 1)]));
        assert_eq!(typed(&show, "comeco").matches, vec![(1, 1)]);
        assert_eq!(typed(&show, "s01").matches, vec![(1, 1), (1, 2)]);
        assert!(typed(&show, "S09E01").exact.is_none());
    }
}
