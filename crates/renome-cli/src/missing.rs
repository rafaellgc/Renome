//! What the lookups say is missing from the list (`--missing`), as the
//! window's faded rows: worked out by the shared flow
//! (`renome_services::flow::missing`) from what the lookups found.

use renome_core::Preview;
pub use renome_services::flow::missing::Missing;
use renome_services::flow::missing::{self, AlbumTracks, Known};
use serde_json::Value;

use crate::lookup::Found;

/// What is missing, for the files in the list, by show (or series,
/// album, collection), season (or disc) and number.
pub fn of(rules: &[Value], found: &Found, preview: &Preview, descending: bool) -> Vec<Missing> {
    let mut known = Known::default();
    match found {
        Found::Series(s) => {
            known.shows = s.shows.clone();
            known.episodes = s.chosen.clone();
        }
        Found::Comics(c) => {
            known.comics = c.settled.clone();
            known.volumes = c.volumes.clone();
        }
        Found::Music(m) => {
            known.songs = m.chosen.clone();
            known.albums = m
                .tracklists
                .iter()
                .zip(&m.albums)
                .map(|((dir, tracks, files), (_, release))| AlbumTracks {
                    dir: dir.clone(),
                    release: release.clone(),
                    tracks: tracks.clone(),
                    files: files.clone(),
                })
                .collect();
        }
        Found::Movies(mv) => {
            known.movies = mv.chosen.iter().map(|(p, c)| (p.clone(), c.id)).collect();
            if let Some(sets) = &mv.sets {
                known.set_of = sets.of.iter().copied().collect();
                known.sets = sets.sets.clone();
            }
        }
        Found::Books(b) => {
            known.book_guesses = b
                .guesses()
                .map(|(p, (g, _))| (p.clone(), g.clone()))
                .collect();
        }
    }
    let mut out = missing::of(rules, &known, preview, descending, &missing::today());
    out.sort_by(|a, b| {
        a.group
            .cmp(&b.group)
            .then(a.season.cmp(&b.season))
            .then(a.number.cmp(&b.number))
    });
    out
}
