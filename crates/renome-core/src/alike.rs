//! Songs that are the same music in different files: an MP3 and a FLAC of
//! one song, or a song on its album and again on a compilation. Told by
//! the tags (title and main artist, without what is in brackets) and the
//! length, never by the sound; files that are identical copies of each
//! other are already shown as such and are not counted again.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::files::{FileEntry, natural_cmp};

/// Lengths this close (seconds) are the same song.
const SLACK: u32 = 3;

/// A title or artist reduced to what tells it apart: no accents, case,
/// punctuation or bracketed parts (`Lua (Remaster 2011)` = `lua`), nor a
/// trailing `- 2011 Remaster`.
fn key(s: &str) -> String {
    static REMASTER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\s+-\s+[^-]*\b(?:remaster(?:ed)?|remix|mono|stereo)\b.*$").unwrap()
    });
    let s = crate::tmdb::BRACKETS.replace_all(s, " ");
    let s = REMASTER.replace(&s, "");
    crate::text::strip_accents(&s)
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Groups of songs that are the same music (two or more each, sorted by
/// path; the groups by their first path). Songs without a title, an
/// artist or a length are left out. `None` when `stale` turned true.
pub fn same_songs(
    files: &[FileEntry],
    identical: &HashMap<PathBuf, usize>,
    stale: &dyn Fn() -> bool,
) -> Option<Vec<Vec<PathBuf>>> {
    let mut by_key: HashMap<(String, String), Vec<(u32, &FileEntry)>> = HashMap::new();
    for (n, f) in files.iter().enumerate() {
        if n % 100 == 0 && stale() {
            return None;
        }
        let Some(t) = f.audio_tags() else { continue };
        let artist = if t.artist.is_empty() {
            &t.album_artist
        } else {
            &t.artist
        };
        let (title, artist) = (key(&t.title), key(&crate::music::main_artist(artist)));
        let Some(length) = t.duration else { continue };
        if title.is_empty() || artist.is_empty() {
            continue;
        }
        by_key.entry((artist, title)).or_default().push((length, f));
    }
    let mut groups: Vec<Vec<PathBuf>> = Vec::new();
    for mut songs in by_key.into_values().filter(|s| s.len() > 1) {
        songs.sort_by_key(|(length, _)| *length);
        // Lengths within a few seconds of the group's shortest.
        let mut runs: Vec<Vec<&FileEntry>> = Vec::new();
        let mut start = 0;
        for (length, f) in songs {
            match runs.last_mut() {
                Some(run) if length - start <= SLACK => run.push(f),
                _ => {
                    start = length;
                    runs.push(vec![f]);
                }
            }
        }
        for run in runs.into_iter().filter(|r| r.len() > 1) {
            // Only identical copies of one file: shown as copies already.
            let first = identical.get(&run[0].path);
            if first.is_some() && run.iter().all(|f| identical.get(&f.path) == first) {
                continue;
            }
            let mut paths: Vec<PathBuf> = run.iter().map(|f| f.path.clone()).collect();
            paths.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
            groups.push(paths);
        }
    }
    groups.sort_by(|a, b| natural_cmp(&a[0].to_string_lossy(), &b[0].to_string_lossy()));
    Some(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::AudioTags;

    fn song(path: &str, artist: &str, title: &str, secs: u32) -> FileEntry {
        let f = FileEntry {
            path: path.into(),
            ext: "mp3".into(),
            ..FileEntry::default()
        };
        f.tags
            .set(Some(AudioTags {
                artist: artist.into(),
                title: title.into(),
                duration: Some(secs),
                ..Default::default()
            }))
            .unwrap();
        f
    }

    #[test]
    fn finds_the_same_song() {
        let files = vec![
            song("/a/01.flac", "Banda Azul", "Lua Cheia", 200),
            song(
                "/b/05.mp3",
                "Banda Azul feat. Rui",
                "Lua Cheia (Remaster 2011)",
                202,
            ),
            song("/c/09.mp3", "banda azul", "Lua cheia - 2011 Remaster", 199),
            // Live: much longer.
            song("/d/01.mp3", "Banda Azul", "Lua Cheia (Ao Vivo)", 260),
            // Another artist.
            song("/e/01.mp3", "Outra", "Lua Cheia", 200),
            song("/f/01.mp3", "Mar", "Onda", 180),
            song("/g/01.mp3", "Mar", "Onda", 180),
        ];
        let mut identical = HashMap::new();
        identical.insert(PathBuf::from("/f/01.mp3"), 0);
        identical.insert(PathBuf::from("/g/01.mp3"), 0);
        let groups = same_songs(&files, &identical, &|| false).unwrap();
        assert_eq!(
            groups,
            [vec![
                PathBuf::from("/a/01.flac"),
                PathBuf::from("/b/05.mp3"),
                PathBuf::from("/c/09.mp3")
            ]]
        );
        assert!(same_songs(&files, &identical, &|| true).is_none());
    }
}
