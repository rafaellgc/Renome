//! The music rule: the songs are grouped in albums and looked up on
//! MusicBrainz (Discogs when it finds nothing, AcoustID by their sound);
//! a release that fits without doubt is picked, the rest are doubts. A
//! release chosen by the person is matched to the album's songs the same
//! way for both.

use super::Item;
use crate::Host;
use crate::musicbrainz::{self as mbs, AlbumLookup, Chosen, Services};
use renome_core::FileEntry;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// The songs looked up: audio files (those without a title only when
/// they can be told by their sound), not the ones already settled.
pub fn targets(items: &[Item], listen: bool, settled: &HashSet<PathBuf>) -> Vec<PathBuf> {
    items
        .iter()
        .filter(|i| {
            !i.is_dir
                && (!i.untagged || listen)
                && renome_core::music::is_audio(&i.ext())
                && !settled.contains(&i.path)
        })
        .map(|i| i.path.clone())
        .collect()
}

/// The songs as the lookups read them: with a title, and without (only
/// told by their sound; without listening, a title from the file name is
/// all there is).
fn songs(
    files: &[&FileEntry],
    heard: &HashMap<PathBuf, Vec<String>>,
    from_name: bool,
    listen: bool,
) -> (mbs::Songs, mbs::Songs) {
    let (mut named, mut unnamed) = mbs::songs_of(files, heard, from_name);
    if !listen {
        let (guessed, none): (Vec<_>, Vec<_>) =
            unnamed.into_iter().partition(|(_, t)| !t.title.is_empty());
        named.extend(guessed);
        unnamed = none;
    }
    (named, unnamed)
}

/// Looks up the albums of `files` (see [`targets`]). An error when every
/// album failed (no connection, say).
pub async fn search(
    host: &dyn Host,
    services: &Services<'_>,
    files: &[&FileEntry],
    from_name: bool,
    listen: bool,
) -> Result<Vec<AlbumLookup>, String> {
    let heard = services.heard.lock().unwrap().clone();
    let (named, unnamed) = songs(files, &heard, from_name, listen);
    let found = mbs::lookup(host, services, mbs::albums(&named), unnamed, listen).await;
    if let Some(failed) = found.iter().find_map(|f| f.error.clone())
        && found.iter().all(|f| f.error.is_some())
    {
        return Err(failed);
    }
    Ok(found)
}

/// The release chosen for an album (`id`: MusicBrainz's, or `dg:` and
/// Discogs'), matched to its songs: those without a title by the
/// recording heard, or by what their file name tells.
pub async fn choose(
    services: &Services<'_>,
    id: &str,
    files: &[&FileEntry],
    from_name: bool,
) -> Result<Chosen, String> {
    let heard = services.heard.lock().unwrap().clone();
    let (songs, unnamed) = mbs::songs_of(files, &heard, from_name);
    let mut songs = mbs::albums(&songs).concat();
    songs.extend(unnamed);
    mbs::choose(services.mb, services.dg, id, &songs).await
}
