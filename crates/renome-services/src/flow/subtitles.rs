//! Subtitles for the videos the movie and TV show rules identified: which
//! videos are searched, the subtitle picked per language (the best one,
//! unless the video already has that language inside), and what is
//! downloaded once they are renamed.

use crate::Host;
use crate::subtitles::{self as subs, Auth};
use renome_core::subtitles::Candidate;
use renome_core::tv::EpisodeMatch;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A file of the list and what the lookup rules made of it.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identified {
    pub path: PathBuf,
    /// Its name on disk.
    pub original: String,
    #[serde(default)]
    pub is_dir: bool,
    /// The movie chosen for it (TMDB).
    #[serde(default)]
    pub movie: Option<u64>,
    /// The episode chosen for it.
    #[serde(default)]
    pub episode: Option<EpisodeMatch>,
}

/// The videos searched: a movie, or one TMDB episode (not AniList's, not
/// a file of several episodes).
pub fn targets(files: &[Identified]) -> Vec<subs::Item> {
    files
        .iter()
        .filter(|f| {
            let ext = std::path::Path::new(&f.original)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            !f.is_dir && renome_core::tmdb::is_video(&ext)
        })
        .filter_map(|f| {
            if let Some(id) = f.movie {
                return Some(subs::Item {
                    path: f.path.clone(),
                    tmdb: id,
                    season: None,
                    episode: None,
                });
            }
            let e = f
                .episode
                .as_ref()
                .filter(|e| e.source.is_empty() && e.episodes.len() == 1)?;
            Some(subs::Item {
                path: f.path.clone(),
                tmdb: e.id,
                season: Some(u32::from(e.season)),
                episode: Some(u32::from(e.episodes[0])),
            })
        })
        .collect()
}

/// What was found for a video, with the subtitle picked per language (an
/// index in `candidates`; -1: none).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    #[serde(flatten)]
    pub found: subs::Found,
    pub pick: BTreeMap<String, i64>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// The languages asked for, as understood (`pt-BR`, `en`).
    pub langs: Vec<String>,
    pub found: Vec<Found>,
    pub errors: Vec<String>,
    pub remaining: Option<i64>,
}

/// The first (best) subtitle of each language, none for a language the
/// video has inside.
pub fn picks(found: &subs::Found) -> BTreeMap<String, i64> {
    let mut pick = BTreeMap::new();
    for (i, c) in found.candidates.iter().enumerate() {
        if !pick.contains_key(&c.lang) {
            let n = if found.inside.contains(&c.lang) {
                -1
            } else {
                i as i64
            };
            pick.insert(c.lang.clone(), n);
        }
    }
    pick
}

/// Searches the videos (see [`targets`]).
pub async fn search(
    host: &dyn Host,
    auth: Auth,
    langs: &str,
    items: Vec<subs::Item>,
    stop: impl Fn() -> bool,
) -> Outcome {
    let s = subs::search(host, auth, langs, items, stop).await;
    Outcome {
        langs: s.langs,
        found: s
            .found
            .into_iter()
            .map(|f| Found {
                pick: picks(&f),
                found: f,
            })
            .collect(),
        errors: s.errors,
        remaining: s.remaining,
    }
}

/// A renamed video's subtitles as they stand when it is renamed.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chosen {
    /// Where the video is now.
    pub path: PathBuf,
    #[serde(default)]
    pub season: Option<u32>,
    #[serde(default)]
    pub episode: Option<u32>,
    pub have: Vec<String>,
    pub candidates: Vec<Candidate>,
    pub pick: BTreeMap<String, i64>,
}

/// What to download: per language not there yet, the one picked first
/// and the others of that language after it (tried when it fails); the
/// languages in the order asked (the first gets the downloads when the
/// day's quota is short).
pub fn writes(chosen: Vec<Chosen>, langs: &[String]) -> Vec<subs::Write> {
    let order = |l: &str| langs.iter().position(|x| x == l).unwrap_or(usize::MAX);
    chosen
        .into_iter()
        .filter_map(|c| {
            let mut picked: Vec<(&String, usize)> = c
                .pick
                .iter()
                .filter(|(l, i)| **i >= 0 && !c.have.contains(l))
                .filter_map(|(l, i)| {
                    usize::try_from(*i)
                        .ok()
                        .filter(|i| *i < c.candidates.len())
                        .map(|i| (l, i))
                })
                .collect();
            if picked.is_empty() {
                return None;
            }
            picked.sort_by_key(|(l, _)| order(l));
            let langs = picked
                .into_iter()
                .map(|(l, i)| {
                    let mut list = vec![c.candidates[i].clone()];
                    list.extend(
                        c.candidates
                            .iter()
                            .enumerate()
                            .filter(|(j, x)| *j != i && x.lang == *l)
                            .map(|(_, x)| x.clone()),
                    );
                    list
                })
                .collect();
            Some(subs::Write {
                path: c.path,
                season: c.season,
                episode: c.episode,
                langs,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(lang: &str, id: &str) -> Candidate {
        Candidate {
            lang: lang.into(),
            id: id.into(),
            ..Default::default()
        }
    }

    #[test]
    fn best_per_language_unless_inside() {
        let f = subs::Found {
            path: "/v.mkv".into(),
            have: vec![],
            inside: vec!["en".into()],
            candidates: vec![cand("pt-BR", "a"), cand("en", "b"), cand("pt-BR", "c")],
        };
        let p = picks(&f);
        assert_eq!(p["pt-BR"], 0);
        assert_eq!(p["en"], -1);
    }

    #[test]
    fn writes_in_asked_order_with_the_rest_after() {
        let c = Chosen {
            path: "/v.mkv".into(),
            season: None,
            episode: None,
            have: vec!["es".into()],
            candidates: vec![
                cand("en", "e1"),
                cand("pt-BR", "p1"),
                cand("pt-BR", "p2"),
                cand("es", "s1"),
            ],
            pick: [("en".into(), 0), ("pt-BR".into(), 2), ("es".into(), 3)].into(),
        };
        let w = writes(vec![c], &["pt-BR".into(), "en".into(), "es".into()]);
        let ids: Vec<Vec<&str>> = w[0]
            .langs
            .iter()
            .map(|l| l.iter().map(|c| c.id.as_str()).collect())
            .collect();
        assert_eq!(ids, [vec!["p2", "p1"], vec!["e1"]]);
    }
}
