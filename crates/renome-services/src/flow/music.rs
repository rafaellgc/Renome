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
    let heard = services.heard.lock().expect("lock not poisoned").clone();
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
    let heard = services.heard.lock().expect("lock not poisoned").clone();
    let (songs, unnamed) = mbs::songs_of(files, &heard, from_name);
    let mut songs = mbs::albums(&songs).concat();
    songs.extend(unnamed);
    mbs::choose(services.mb, services.dg, id, &songs).await
}

/// Where each album's cover is saved once its songs are renamed: the
/// folder most of its renamed songs went to (a song left where it was, by
/// a conflict, does not keep the cover behind); none renamed, its first
/// song's folder. `albums`: each release with its songs; `moved`: where
/// the renamed ones went. One cover per folder.
pub fn cover_items(
    albums: &[(String, Vec<PathBuf>)],
    moved: &HashMap<PathBuf, PathBuf>,
) -> Vec<mbs::CoverItem> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (release, songs) in albums {
        let mut count: Vec<(PathBuf, usize)> = Vec::new();
        for to in songs.iter().filter_map(|p| moved.get(p)) {
            let Some(dir) = to.parent() else { continue };
            match count.iter_mut().find(|(d, _)| d == dir) {
                Some((_, n)) => *n += 1,
                None => count.push((dir.to_path_buf(), 1)),
            }
        }
        // The first folder among the most counted.
        let best = count.iter().map(|(_, n)| *n).max();
        let dir = match count.into_iter().find(|(_, n)| Some(*n) == best) {
            Some((d, _)) => d,
            None => match songs.first().and_then(|p| p.parent()) {
                Some(d) => d.to_path_buf(),
                None => continue,
            },
        };
        if seen.insert(dir.clone()) {
            out.push(mbs::CoverItem {
                dir,
                release: release.clone(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_goes_where_the_album_went() {
        let p = PathBuf::from;
        let albums = vec![(
            "r1".to_string(),
            vec![p("/in/01.mp3"), p("/in/02.mp3"), p("/in/03.mp3")],
        )];
        // The first song stayed behind by a conflict.
        let moved: HashMap<PathBuf, PathBuf> = [
            (p("/in/02.mp3"), p("/lib/A/Album/02.mp3")),
            (p("/in/03.mp3"), p("/lib/A/Album/03.mp3")),
        ]
        .into();
        let c = cover_items(&albums, &moved);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].dir, p("/lib/A/Album"));
        // Nothing renamed (the names were right): the songs' folder.
        let c = cover_items(&albums, &HashMap::new());
        assert_eq!(c[0].dir, p("/in"));
    }
}
