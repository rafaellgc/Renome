//! What the lookups say is missing from the list (`--missing`), as the
//! window's faded rows: aired episodes of the seasons the videos cover,
//! published issues of the comics' series, tracks of the albums,
//! released movies of the collections, volumes between a book series'
//! files. Only what is out by today.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use renome_core::Preview;
use serde::Serialize;

use crate::lookup::Found;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Missing {
    /// `episode`, `issue`, `track`, `movie` or `book`.
    pub kind: &'static str,
    /// The show, series, album or collection.
    pub group: String,
    /// As in file names: S01E16, #7, 1-05, 1999, 3.
    pub code: String,
    pub title: String,
    /// Air, cover or release date (YYYY-MM-DD), when known.
    pub date: String,
}

fn named(name: &str, year: Option<u16>) -> String {
    year.map_or_else(|| name.to_string(), |y| format!("{name} ({y})"))
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// What is missing, for the files in the list.
pub fn of(found: &Found, preview: &Preview) -> Vec<Missing> {
    let listed: HashSet<&PathBuf> = preview.items.iter().map(|i| &i.path).collect();
    let day = today();
    let mut out = Vec::new();
    match found {
        Found::Series(s) => {
            // Each season a listed video covers.
            let mut seasons: BTreeMap<(String, u64, u16), HashSet<u16>> = BTreeMap::new();
            for (p, m) in &s.chosen {
                if listed.contains(p) && is_video(p) {
                    seasons
                        .entry((m.source.clone(), m.id, m.season))
                        .or_default()
                        .extend(m.episodes.iter().copied());
                }
            }
            for ((source, id, season), have) in seasons {
                let Some(show) = s.shows.iter().find(|x| x.id == id && x.source == source) else {
                    continue;
                };
                for e in show
                    .episodes
                    .iter()
                    .filter(|e| e.season == season && !have.contains(&e.number))
                {
                    let Some(date) = e.air_date.clone().filter(|d| *d <= day) else {
                        continue;
                    };
                    out.push(Missing {
                        kind: "episode",
                        group: named(&show.name, show.year),
                        code: format!("S{season:02}E{:02}", e.number),
                        title: e.name.clone(),
                        date,
                    });
                }
            }
        }
        Found::Comics(c) => {
            for (volume, issues, held) in &c.listed {
                let group = named(&volume.name, volume.start_year);
                if issues.is_empty() {
                    // No list of issues (a manga catalogue): the gaps
                    // between the files' own numbers.
                    let numbers: Vec<f64> = held
                        .iter()
                        .filter_map(|(_, own)| {
                            own.volume
                                .map(f64::from)
                                .or_else(|| own.number.parse().ok())
                        })
                        .collect();
                    let have: HashSet<i64> = numbers
                        .iter()
                        .filter(|n| n.fract() == 0.0)
                        .map(|n| *n as i64)
                        .collect();
                    let top = have.iter().max().copied().unwrap_or(0);
                    for n in (1..top).filter(|n| !have.contains(n)) {
                        out.push(Missing {
                            kind: "issue",
                            group: group.clone(),
                            code: format!("#{n}"),
                            title: String::new(),
                            date: String::new(),
                        });
                    }
                    continue;
                }
                let have: HashSet<&String> = held.iter().map(|(id, _)| id).collect();
                if have.is_empty() {
                    continue;
                }
                let mut order: Vec<&renome_core::comics::Issue> = issues.iter().collect();
                order.sort_by(|a, b| {
                    let n = |i: &renome_core::comics::Issue| {
                        i.number.parse::<f64>().unwrap_or(f64::INFINITY)
                    };
                    n(a).total_cmp(&n(b))
                });
                for i in order {
                    if have.contains(&i.id) || (!i.cover_date.is_empty() && i.cover_date > day) {
                        continue;
                    }
                    let code = if i.number.starts_with(|c: char| c.is_ascii_digit()) {
                        format!("#{}", i.number)
                    } else {
                        i.number.clone()
                    };
                    out.push(Missing {
                        kind: "issue",
                        group: group.clone(),
                        code,
                        title: i.name.clone(),
                        date: i.cover_date.clone(),
                    });
                }
            }
        }
        Found::Music(m) => {
            // A folder heard on several releases: the one most songs are on.
            let mut held_of: Vec<(
                &PathBuf,
                &Vec<renome_core::musicbrainz::ListedTrack>,
                Vec<usize>,
            )> = Vec::new();
            for (dir, tracks, files) in &m.tracklists {
                let at: Vec<usize> = files
                    .iter()
                    .filter(|p| listed.contains(p))
                    .filter_map(|p| m.chosen.get(p))
                    .filter_map(|s| tracks.iter().position(|t| t.id == s.track_id))
                    .collect();
                if !at.is_empty() {
                    held_of.push((dir, tracks, at));
                }
            }
            let mut most: HashMap<&PathBuf, usize> = HashMap::new();
            let mut taken: HashMap<&PathBuf, HashSet<(u32, u32)>> = HashMap::new();
            for (dir, tracks, at) in &held_of {
                let e = most.entry(dir).or_default();
                *e = (*e).max(at.len());
                taken
                    .entry(dir)
                    .or_default()
                    .extend(at.iter().map(|&k| (tracks[k].disc, tracks[k].position)));
            }
            for (dir, tracks, at) in &held_of {
                if at.len() < most[dir] {
                    continue;
                }
                let song = m
                    .tracklists
                    .iter()
                    .find(|t| &t.0 == *dir)
                    .and_then(|t| t.2.iter().find_map(|p| m.chosen.get(p)));
                let group = song.map_or_else(String::new, |s| {
                    format!("{} - {}", s.album_artist, named(&s.album, s.year))
                });
                let discs: HashSet<u32> = tracks.iter().map(|t| t.disc).collect();
                for (k, t) in tracks.iter().enumerate() {
                    if at.contains(&k) || taken[dir].contains(&(t.disc, t.position)) {
                        continue;
                    }
                    let n = format!("{:02}", t.position);
                    let code = if discs.len() > 1 {
                        format!("{}-{n}", t.disc)
                    } else {
                        n
                    };
                    out.push(Missing {
                        kind: "track",
                        group: group.clone(),
                        code,
                        title: t.title.clone(),
                        date: String::new(),
                    });
                }
            }
        }
        Found::Movies(mv) => {
            let Some(sets) = &mv.sets else { return out };
            let set_of: HashMap<u64, u64> = sets
                .of
                .iter()
                .filter_map(|(id, s)| Some((*id, (*s)?)))
                .collect();
            let mut held: HashMap<u64, HashSet<u64>> = HashMap::new();
            for (p, c) in &mv.chosen {
                if listed.contains(p)
                    && is_video(p)
                    && let Some(set) = set_of.get(&c.id)
                {
                    held.entry(*set).or_default().insert(c.id);
                }
            }
            for set in &sets.sets {
                let Some(have) = held.get(&set.id) else {
                    continue;
                };
                let mut order: Vec<_> = set.parts.iter().collect();
                order.sort_by(|a, b| {
                    let d = |s: &str| {
                        if s.is_empty() {
                            "9".to_string()
                        } else {
                            s.to_string()
                        }
                    };
                    d(&a.release_date).cmp(&d(&b.release_date))
                });
                for part in order {
                    if have.contains(&part.id)
                        || part.release_date.is_empty()
                        || part.release_date > day
                    {
                        continue;
                    }
                    out.push(Missing {
                        kind: "movie",
                        group: set.name.clone(),
                        code: part.release_date.get(..4).unwrap_or_default().to_string(),
                        title: part.title.clone(),
                        date: part.release_date.clone(),
                    });
                }
            }
        }
        Found::Books(b) => {
            let mut series: BTreeMap<String, (String, HashSet<i64>)> = BTreeMap::new();
            for (p, (guess, _)) in b.guesses() {
                let n: f64 = match guess.series_index.trim().parse() {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                if !listed.contains(p) || guess.series.trim().is_empty() || !n.is_finite() {
                    continue;
                }
                let key = renome_core::text::strip_accents(&guess.series).to_lowercase();
                let e = series
                    .entry(key)
                    .or_insert_with(|| (guess.series.trim().to_string(), HashSet::new()));
                // Half volumes (2.5) are extras: they neither fill nor
                // make a gap.
                if n.fract() == 0.0 {
                    e.1.insert(n as i64);
                }
            }
            for (_, (name, have)) in series {
                let top = have.iter().max().copied().unwrap_or(0);
                for n in (1..top).filter(|n| !have.contains(n)) {
                    out.push(Missing {
                        kind: "book",
                        group: name.clone(),
                        code: n.to_string(),
                        title: String::new(),
                        date: String::new(),
                    });
                }
            }
        }
    }
    out
}

fn is_video(p: &Path) -> bool {
    p.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| renome_core::tmdb::is_video(&e))
}
