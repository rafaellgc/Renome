//! Music: the tags in audio files (ID3, Vorbis comments, MP4 and others)
//! and the names and folders built from them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use lofty::config::ParseOptions;
use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, Tag};
use regex::Regex;
use serde::Serialize;

use crate::text;
use crate::tmdb::{self, safe_text, safe_title, similarity};

/// What a song's name is made of.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTags {
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub year: Option<u16>,
    pub track: Option<u32>,
    pub track_total: Option<u32>,
    pub disc: Option<u32>,
    pub disc_total: Option<u32>,
    pub genre: String,
    pub composer: String,
    /// Length in seconds.
    pub duration: Option<u32>,
    /// The MusicBrainz release, when the files were tagged with it.
    pub mb_release: String,
    /// The MusicBrainz release track: songs whose tags were written from
    /// MusicBrainz (by Renome or another tagger) have it.
    #[serde(skip)]
    pub mb_track: String,
    /// The file has tags that writing with cleaning would take away
    /// (comments, sites, an MP3's ID3v1 beside its ID3v2…).
    #[serde(skip)]
    pub junk: bool,
    /// The album's songs are by several artists (set by [`fill_albums`]):
    /// file names then show each song's artist.
    #[serde(skip)]
    pub compilation: bool,
    /// The MusicBrainz recordings the song was heard as (AcoustID).
    #[serde(skip)]
    pub recordings: Vec<String>,
}

/// Audio formats whose tags can be read (video containers left out).
pub fn is_audio(ext: &str) -> bool {
    let ext = ext.to_lowercase();
    !matches!(ext.as_str(), "mp4" | "m4v" | "3gp") && FileType::from_ext(&ext).is_some()
}

fn text_of(tags: &[&Tag], get: impl Fn(&Tag) -> Option<String>) -> String {
    tags.iter()
        .find_map(|t| {
            get(t)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_default()
}

fn number_of(tags: &[&Tag], get: impl Fn(&Tag) -> Option<u32>) -> Option<u32> {
    tags.iter().find_map(|t| get(t).filter(|&n| n > 0))
}

/// Reads the tags and the length (no audio is decoded); `None` when the
/// file cannot be read.
/// With several tags (ID3v2 and ID3v1), each field comes from the first
/// that has it, the main one first.
pub fn read(path: &Path) -> Option<AudioTags> {
    let file = Probe::open(path)
        .ok()?
        .options(ParseOptions::new().read_cover_art(false))
        .read()
        .ok()?;
    let mut tags: Vec<&Tag> = file.primary_tag().into_iter().collect();
    for t in file.tags() {
        if !tags.iter().any(|x| std::ptr::eq(*x, t)) {
            tags.push(t);
        }
    }
    let t = AudioTags {
        title: text_of(&tags, |t| t.title().map(|s| s.into_owned())),
        artist: text_of(&tags, |t| t.artist().map(|s| s.into_owned())),
        album_artist: text_of(&tags, |t| {
            t.get_string(ItemKey::AlbumArtist).map(str::to_string)
        }),
        album: text_of(&tags, |t| t.album().map(|s| s.into_owned())),
        year: tags
            .iter()
            .find_map(|t| t.date().map(|d| d.year).filter(|&y| y > 0)),
        track: number_of(&tags, Accessor::track),
        track_total: number_of(&tags, Accessor::track_total),
        disc: number_of(&tags, Accessor::disk),
        disc_total: number_of(&tags, Accessor::disk_total),
        genre: text_of(&tags, |t| t.genre().map(|s| s.into_owned())),
        composer: text_of(&tags, |t| {
            t.get_string(ItemKey::Composer).map(str::to_string)
        }),
        duration: Some(file.properties().duration().as_secs() as u32).filter(|&s| s > 0),
        mb_release: text_of(&tags, |t| {
            t.get_string(ItemKey::MusicBrainzReleaseId)
                .map(str::to_string)
        }),
        mb_track: text_of(&tags, |t| {
            t.get_string(ItemKey::MusicBrainzTrackId)
                .map(str::to_string)
        }),
        junk: file.tags().len() > 1
            || tags
                .iter()
                .any(|t| t.items().any(|i| !crate::tagwrite::kept(i.key()))),
        compilation: false,
        recordings: Vec::new(),
    };
    (t != AudioTags::default()).then_some(t)
}

static LEADING_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(\d{1,2})[-.])?(\d{1,3})(?:\s*[-.)_]\s*|\s+)(.+)$").unwrap());
static FOLDER_YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*[(\[]((?:19|20)\d{2})[)\]]\s*$").unwrap());
static DISC_FOLDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:cd|disc|disco|disk)[ ._-]*(\d{1,2})$").unwrap());

/// The disc a folder stands for (`CD1`, `Disc 2`, `Disco 2`).
pub fn disc_folder(name: &str) -> Option<u32> {
    DISC_FOLDER.captures(name.trim())?[1].parse().ok()
}

/// What the file name and its folder say, for files without tags:
/// `01 - Artist - Title`, `01. Title`, `1-05 Title`, `Artist - Title`;
/// the album from the folder (`Artist - Album (1999)`, `Album`), the disc
/// from a `CD2` folder.
pub fn from_name(stem: &str, dir: &Path) -> AudioTags {
    let mut t = AudioTags::default();
    let spaced = text::collapse_spaces(&stem.replace('_', " "));
    let rest = match LEADING_NUMBER.captures(&spaced) {
        Some(c) => {
            t.disc = c.get(1).and_then(|m| m.as_str().parse().ok());
            t.track = c[2].parse().ok();
            c[3].to_string()
        }
        None => spaced.clone(),
    };
    let parts: Vec<&str> = rest
        .split(" - ")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    match parts.as_slice() {
        [] => {}
        [title] => t.title = title.to_string(),
        [artist, title @ ..] => {
            t.artist = artist.to_string();
            t.title = title.join(" - ");
        }
    }
    let name = |p: &Path| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    let mut folder = name(dir);
    if let Some(d) = disc_folder(&folder) {
        t.disc = t.disc.or(Some(d));
        folder = dir.parent().map(name).unwrap_or_default();
    }
    if let Some(c) = FOLDER_YEAR.captures(&folder) {
        t.year = c[1].parse().ok();
        folder = folder[..c.get(0).unwrap().start()].to_string();
    }
    match folder.split_once(" - ") {
        Some((artist, album)) => {
            t.album = album.trim().to_string();
            if t.artist.is_empty() {
                t.artist = artist.trim().to_string();
            }
        }
        None => t.album = folder.trim().to_string(),
    }
    t
}

/// The album artist of compilations whose songs are by several artists.
pub const VARIOUS_ARTISTS: &str = "Various Artists";

fn most_common<T: Clone + PartialEq>(values: impl Iterator<Item = T>) -> Option<T> {
    let mut counts: Vec<(T, usize)> = Vec::new();
    for v in values {
        match counts.iter_mut().find(|(x, _)| *x == v) {
            Some(e) => e.1 += 1,
            None => counts.push((v, 1)),
        }
    }
    let most = counts.iter().map(|(_, n)| *n).max()?;
    counts.into_iter().find(|(_, n)| *n == most).map(|(v, _)| v)
}

static FEATURING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s+(?:feat\.?|ft\.|featuring|with)\s.*$").unwrap());

/// The main artist: `Banda Azul feat. Rui` → `Banda Azul`.
fn main_artist(artist: &str) -> String {
    FEATURING.replace(artist, "").trim().to_string()
}

/// The songs of each album (indexes into `songs`, in order): same album
/// tag, in one folder or in disc folders of one folder. `songs` holds each
/// song's folder. Songs without an album tag are left out.
pub fn album_groups(songs: &[(PathBuf, AudioTags)]) -> Vec<Vec<usize>> {
    let mut keys: Vec<(PathBuf, String)> = Vec::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (i, (dir, t)) in songs.iter().enumerate() {
        if t.album.is_empty() {
            continue;
        }
        let name = dir.file_name().unwrap_or_default().to_string_lossy();
        let base = match dir.parent() {
            Some(up) if disc_folder(&name).is_some() => up.to_path_buf(),
            _ => dir.clone(),
        };
        let key = (base, t.album.to_lowercase());
        match keys.iter().position(|k| *k == key) {
            Some(g) => groups[g].push(i),
            None => {
                keys.push(key);
                groups.push(vec![i]);
            }
        }
    }
    groups
}

/// The folder a song's album is in: its own, or the one above a disc
/// folder (`CD 01`).
pub fn album_dir(dir: &Path) -> &Path {
    let name = dir.file_name().unwrap_or_default().to_string_lossy();
    match dir.parent() {
        Some(up) if disc_folder(&name).is_some() => up,
        _ => dir,
    }
}

/// "Single" is a kind of release, not an album's name: such a song is its
/// own album.
fn name_singles(songs: &mut [(PathBuf, AudioTags)]) {
    for (_, t) in songs.iter_mut() {
        if matches!(t.album.trim().to_lowercase().as_str(), "single" | "singles")
            && !t.title.is_empty()
        {
            t.album = t.title.clone();
        }
    }
}

/// Bonus tracks tagged with other albums (a single, a soundtrack) in a
/// folder where most songs are one album's join that album, when their
/// numbers come after the album's own (`15`, `16` after `01`–`14`).
fn join_bonus_tracks(songs: &mut [(PathBuf, AudioTags)]) {
    let mut by_dir: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for (i, (dir, _)) in songs.iter().enumerate() {
        by_dir
            .entry(album_dir(dir).to_path_buf())
            .or_default()
            .push(i);
    }
    for idx in by_dir.into_values().filter(|v| v.len() > 2) {
        let Some(album) = most_common(
            idx.iter()
                .map(|&i| songs[i].1.album.to_lowercase())
                .filter(|a| !a.is_empty()),
        ) else {
            continue;
        };
        let (own, others): (Vec<usize>, Vec<usize>) = idx
            .iter()
            .partition(|&&i| songs[i].1.album.to_lowercase() == album);
        if own.len() * 2 <= idx.len() || others.is_empty() {
            continue;
        }
        let last = |disc: Option<u32>| {
            own.iter()
                .filter(|&&i| songs[i].1.disc.unwrap_or(1) == disc.unwrap_or(1))
                .filter_map(|&i| songs[i].1.track)
                .max()
        };
        let bonus: Vec<usize> = others
            .into_iter()
            .filter(|&i| {
                let t = &songs[i].1;
                matches!((t.track, last(t.disc)), (Some(n), Some(max)) if n > max)
            })
            .collect();
        let first = songs[own[0]].1.clone();
        for i in bonus {
            let t = &mut songs[i].1;
            t.album = first.album.clone();
            t.album_artist = first.album_artist.clone();
            t.year = first.year;
            t.mb_release = first.mb_release.clone();
        }
    }
}

/// Makes the album-wide fields agree across an album's songs (same album
/// tag, same folder or disc folders of one folder): a song without a year
/// gets the album's; without an album artist, the others'. When none has
/// one, the artist of most songs (guests left out) is the album's; with
/// no such majority, the album is a compilation ("Various Artists").
/// Albums where most songs are by others than the album's artist are
/// marked compilations. Songs tagged "Single" become their own albums, and
/// bonus tracks tagged with other albums join their folder's album.
/// `songs` holds each song's folder.
pub fn fill_albums(songs: &mut [(PathBuf, AudioTags)]) {
    name_singles(songs);
    join_bonus_tracks(songs);
    for idx in album_groups(songs) {
        mark_compilation(songs, &idx);
    }
    for idx in album_groups(songs).iter().filter(|g| g.len() > 1) {
        let year = most_common(idx.iter().filter_map(|&i| songs[i].1.year));
        let artist = most_common(
            idx.iter()
                .map(|&i| songs[i].1.album_artist.clone())
                .filter(|a| !a.is_empty()),
        )
        .or_else(|| {
            let artists: Vec<String> = idx
                .iter()
                .map(|&i| main_artist(&songs[i].1.artist))
                .filter(|a| !a.is_empty())
                .collect();
            let lower = |a: &String| a.to_lowercase();
            let main = most_common(artists.iter().map(lower))?;
            let count = artists.iter().filter(|a| lower(a) == main).count();
            if count * 2 > idx.len() {
                artists.iter().find(|a| lower(a) == main).cloned()
            } else {
                Some(VARIOUS_ARTISTS.to_string())
            }
        });
        for &i in idx {
            let t = &mut songs[i].1;
            t.year = t.year.or(year);
            if t.album_artist.is_empty()
                && let Some(a) = &artist
            {
                t.album_artist = a.clone();
            }
        }
        mark_compilation(songs, idx);
    }
}

/// Marks an album a compilation when fewer than four in five of its songs
/// are by its artist (guests left out).
fn mark_compilation(songs: &mut [(PathBuf, AudioTags)], idx: &[usize]) {
    let by_album_artist = idx
        .iter()
        .filter(|&&i| {
            let t = &songs[i].1;
            main_artist(&t.artist).to_lowercase() == main_artist(t.album_artist()).to_lowercase()
        })
        .count();
    let compilation = by_album_artist * 5 < idx.len() * 4;
    for &i in idx {
        songs[i].1.compilation = compilation;
    }
}

impl AudioTags {
    /// Fills the empty fields from `other` (the guess from the file name).
    pub fn fill_from(&mut self, other: AudioTags) {
        let fill = |a: &mut String, b: String| {
            if a.is_empty() {
                *a = b;
            }
        };
        fill(&mut self.title, other.title);
        fill(&mut self.artist, other.artist);
        fill(&mut self.album_artist, other.album_artist);
        fill(&mut self.album, other.album);
        fill(&mut self.genre, other.genre);
        fill(&mut self.composer, other.composer);
        self.year = self.year.or(other.year);
        self.track = self.track.or(other.track);
        self.disc = self.disc.or(other.disc);
    }

    /// The album's artist; the track's when not tagged.
    pub fn album_artist(&self) -> &str {
        if self.album_artist.is_empty() {
            &self.artist
        } else {
            &self.album_artist
        }
    }
}

/// A year in an album's name (`Show 12 (2025-05-04)`).
static YEAR_IN_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(?:19|20)\d{2}\b").unwrap());

static REPEATED_DASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" [-–](?: [-–])+ ").unwrap());

/// Expands a template with the tags: `{title}`, `{artist}`,
/// `{album artist}`, `{album}`, `{year}`, `{track}` (two digits, three for
/// albums over 99 tracks; `1-01` with `disc_prefix` on albums with more
/// than one disc), `{disc}`, `{genre}`, `{composer}` (also in Portuguese
/// and Spanish). Parts left empty by missing tags are removed.
/// In a compilation, a template without the artist gets it before the
/// title.
pub fn expand(template: &str, t: &AudioTags, colon: &str, disc_prefix: bool) -> String {
    const ARTIST: [&str; 2] = ["{artist}", "{artista}"];
    const TITLE: [&str; 3] = ["{title}", "{título}", "{titulo}"];
    if t.compilation
        && !t.artist.is_empty()
        && !ARTIST.iter().any(|k| template.contains(k))
        && let Some(at) = TITLE.iter().find_map(|k| template.find(k))
    {
        let template = format!("{}{{artist}} - {}", &template[..at], &template[at..]);
        return expand_as(&template, t, colon, disc_prefix, false);
    }
    expand_as(template, t, colon, disc_prefix, false)
}

/// Folder names lose final dots (Windows refuses them); file names keep
/// them, since the extension follows.
fn expand_as(
    template: &str,
    t: &AudioTags,
    colon: &str,
    disc_prefix: bool,
    folder: bool,
) -> String {
    let safe = |s: &str| {
        if folder {
            safe_title(s, colon)
        } else {
            safe_text(s, colon)
        }
    };
    let (title, artist, album_artist, album) = (
        safe(&t.title),
        safe(&t.artist),
        safe(t.album_artist()),
        safe(&t.album),
    );
    let (genre, composer) = (safe(&t.genre), safe(&t.composer));
    // An album named with its date (`Show 12 (2025-05-04)`) needs no year
    // again beside it.
    let mut year = t.year.map(|y| y.to_string()).unwrap_or_default();
    let names_album = ["{album}", "{álbum}"].iter().any(|k| template.contains(k));
    if names_album && !year.is_empty() && YEAR_IN_NAME.is_match(&t.album) {
        year.clear();
    }
    let disc = t.disc.map(|d| d.to_string()).unwrap_or_default();
    let several_discs = t.disc_total.is_some_and(|n| n > 1) || t.disc.is_some_and(|d| d > 1);
    let track = t
        .track
        .map(|n| {
            let width = if t.track_total.is_some_and(|n| n > 99) || n > 99 {
                3
            } else {
                2
            };
            match t.disc {
                Some(d) if disc_prefix && several_discs => format!("{d}-{n:0width$}"),
                _ => format!("{n:0width$}"),
            }
        })
        .unwrap_or_default();
    let out = crate::rules::expand(template, |key| match key {
        "title" | "título" | "titulo" => Some(title.as_str()),
        "artist" | "artista" => Some(artist.as_str()),
        "album artist" | "albumartist" | "artista do álbum" | "artista do album"
        | "artista del álbum" | "artista del album" => Some(album_artist.as_str()),
        "album" | "álbum" => Some(album.as_str()),
        "year" | "ano" | "año" => Some(year.as_str()),
        "track" | "faixa" | "pista" => Some(track.as_str()),
        "disc" | "disco" => Some(disc.as_str()),
        "genre" | "gênero" | "genero" | "género" => Some(genre.as_str()),
        "composer" | "compositor" => Some(composer.as_str()),
        _ => None,
    });
    let out = out.replace("()", "").replace("[]", "");
    let out = text::collapse_spaces(&out);
    let out = REPEATED_DASH.replace_all(&out, " - ");
    let out = out.trim_start_matches(|c: char| c.is_whitespace() || "-–_.".contains(c));
    if folder {
        out.trim_end_matches(|c: char| c.is_whitespace() || "-–_.".contains(c))
    } else {
        out.trim_end_matches(|c: char| c.is_whitespace() || "-–_".contains(c))
    }
    .to_string()
}

/// The folders for a song, one per `/` in the template (`{album artist}/
/// {album} ({year})`); empty levels are left out.
pub fn folders(template: &str, t: &AudioTags, colon: &str) -> Vec<String> {
    template
        .split('/')
        .map(|level| expand_as(level, t, colon, false, true))
        .filter(|l| !l.is_empty())
        .collect()
}

fn words(s: &str) -> Vec<String> {
    text::strip_accents(&s.to_lowercase())
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// A bracketed part, or a year or date.
static ASIDE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\([^()]*\)|\[[^\[\]]*\]|\b(?:19|20)\d{2}(?:-\d{2}){0,2}\b").unwrap()
});

/// The words of a name without its bracketed parts, years and dates
/// (`1012 - Outono 2024` gives "1012 outono").
fn plain_words(s: &str) -> Vec<String> {
    words(&ASIDE.replace_all(s, " "))
}

/// Whether a folder is named after one of `names`, ignoring years and
/// bracketed parts on both sides (`Os Vagalumes - Maré Alta (Deluxe) (2026) [FLAC]`
/// is named after "Os Vagalumes - Maré Alta (Deluxe)"), or starts with one.
pub(crate) fn names_any(folder: &str, names: &[&str]) -> bool {
    let bare = tmdb::guess(folder).title;
    let folder_words = words(&bare);
    let folder_plain = plain_words(folder);
    names.iter().filter(|n| !n.is_empty()).any(|n| {
        let plain = plain_words(n);
        // Years inside the name (`Outono 2024`, `(Live …, 2025)`) cut the
        // guessed title short; without them the words may still agree.
        if !plain.is_empty()
            && (folder_plain.starts_with(&plain)
                || (folder_plain.len() > 1 && plain.ends_with(&folder_plain)))
        {
            return true;
        }
        let name = tmdb::guess(n).title;
        let name_words = words(&name);
        similarity(&bare, &name) >= 0.8
            || similarity(folder, n) >= 0.8
            || (!name_words.is_empty() && folder_words.starts_with(&name_words))
            // A short form of the name, even with a typo on either side:
            // `1053 - Сказка об Ветре` for "Show 1053 - Сказка о Ветре".
            || (folder_words.len() > 1
                && name_words.len() >= folder_words.len()
                && (0..=name_words.len() - folder_words.len()).any(|k| {
                    similarity(&folder_words.join(" "), &name_words[k..].join(" ")) >= 0.85
                }))
    })
}

/// Whether a folder is a collection named like its albums: `Аэростат
/// (1000 - ..)` for "Аэростат 1037 - То да се 42" (the album's name starts
/// with the folder's, without years and brackets).
pub fn names_collection(folder: &str, album: &str) -> bool {
    let folder = words(&tmdb::guess(folder).title);
    !folder.is_empty() && words(album).starts_with(&folder)
}

/// Where the artist's folder goes: above the folders already named after
/// the album, the artist or a disc (so `Artist/Album/CD1/x.flac` becomes
/// `Artist/Album (1999)/…` next to them, not inside), else the file's own
/// folder.
pub fn library_root(dir: &Path, t: &AudioTags, levels: &[String]) -> PathBuf {
    let combined = format!("{} - {}", t.album_artist(), t.album);
    // `Album - Artist (2026) [24-192]`, as classical downloads come.
    let reversed = format!("{} - {}", t.album, t.album_artist());
    let mut names: Vec<&str> = levels.iter().map(String::as_str).collect();
    names.extend([
        t.album.as_str(),
        t.album_artist(),
        t.artist.as_str(),
        combined.as_str(),
        reversed.as_str(),
    ]);
    let artists = [t.album_artist(), t.artist.as_str()];
    let name_of = |p: &Path| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    let mut cur = dir;
    for _ in 0..=levels.len() {
        let name = name_of(cur);
        // A folder inside the artist's is the album's, whatever its name.
        let in_artist = cur
            .parent()
            .is_some_and(|up| names_any(&name_of(up), &artists));
        if !(disc_folder(&name).is_some() || names_any(&name, &names) || in_artist) {
            break;
        }
        match cur.parent() {
            Some(up) if !up.as_os_str().is_empty() => cur = up,
            _ => break,
        }
    }
    cur.to_path_buf()
}

/// Test files: a FLAC file with no audio, just the given Vorbis comments.
#[cfg(test)]
pub(crate) use test_files::flac;

#[cfg(test)]
mod test_files {
    use std::fs;
    use std::path::Path;

    /// A FLAC file with no audio, just the given Vorbis comments.
    pub(crate) fn flac(path: &Path, comments: &[(&str, &str)]) {
        let mut b = b"fLaC".to_vec();
        b.extend([0, 0, 0, 34]);
        b.extend([0x10, 0, 0x10, 0, 0, 0, 0, 0, 0, 0]);
        b.extend(((44_100u64 << 44) | (1 << 41) | (15 << 36)).to_be_bytes());
        b.extend([0; 16]);
        let mut vc = Vec::new();
        vc.extend(4u32.to_le_bytes());
        vc.extend(b"test");
        vc.extend((comments.len() as u32).to_le_bytes());
        for (k, v) in comments {
            let c = format!("{k}={v}");
            vc.extend((c.len() as u32).to_le_bytes());
            vc.extend(c.as_bytes());
        }
        b.push(0x84);
        b.extend(&(vc.len() as u32).to_be_bytes()[1..]);
        b.extend(vc);
        fs::write(path, b).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags() -> AudioTags {
        AudioTags {
            title: "Wish You Were Here".into(),
            artist: "Pink Floyd".into(),
            album: "Wish You Were Here".into(),
            year: Some(1975),
            track: Some(4),
            track_total: Some(5),
            ..Default::default()
        }
    }

    #[test]
    fn names() {
        let t = tags();
        assert_eq!(
            expand("{faixa} - {título}", &t, " - ", true),
            "04 - Wish You Were Here"
        );
        let tv = AudioTags {
            title: "The Sun Always Shines On T.V.".into(),
            ..Default::default()
        };
        assert_eq!(
            expand("{title}", &tv, " - ", true),
            "The Sun Always Shines On T.V."
        );
        let dotted = AudioTags {
            album: "Vol. 2...".into(),
            ..Default::default()
        };
        assert_eq!(folders("{album}", &dotted, " - "), ["Vol. 2"]);
        assert_eq!(
            expand(
                "{artista} - {álbum} ({ano}) - {track} - {title}",
                &t,
                " - ",
                true
            ),
            "Pink Floyd - Wish You Were Here (1975) - 04 - Wish You Were Here"
        );
        // Missing tags leave no dangling separators.
        let bare = AudioTags {
            title: "Song".into(),
            ..Default::default()
        };
        assert_eq!(
            expand("{track} - {artist} - {title} ({year})", &bare, " - ", true),
            "Song"
        );
        // Several discs.
        let multi = AudioTags {
            disc: Some(2),
            disc_total: Some(2),
            ..t.clone()
        };
        assert_eq!(
            expand("{track} {title}", &multi, " - ", true),
            "2-04 Wish You Were Here"
        );
        assert_eq!(
            expand("{track} {title}", &multi, " - ", false),
            "04 Wish You Were Here"
        );
        // AC/DC and "Title: Part 2".
        let odd = AudioTags {
            title: "Rock: Part 2".into(),
            artist: "AC/DC".into(),
            ..Default::default()
        };
        assert_eq!(
            expand("{artist} - {title}", &odd, " - ", true),
            "AC-DC - Rock - Part 2"
        );
        assert_eq!(
            folders("{album artist}/{album} ({year})", &t, " - "),
            ["Pink Floyd", "Wish You Were Here (1975)"]
        );
        assert_eq!(
            folders("{album artist}/{album}", &AudioTags::default(), " - "),
            Vec::<String>::new()
        );
    }

    #[test]
    fn guesses_from_names() {
        let t = from_name(
            "01 - Pink Floyd - Shine On",
            Path::new("/m/Pink Floyd - Wish You Were Here (1975)"),
        );
        assert_eq!(
            (t.track, t.artist.as_str(), t.title.as_str()),
            (Some(1), "Pink Floyd", "Shine On")
        );
        assert_eq!(
            (t.album.as_str(), t.year),
            ("Wish You Were Here", Some(1975))
        );
        let t = from_name("05. Have a Cigar", Path::new("/m/Wish You Were Here/CD2"));
        assert_eq!(
            (t.track, t.title.as_str(), t.disc, t.album.as_str()),
            (Some(5), "Have a Cigar", Some(2), "Wish You Were Here")
        );
        let t = from_name("1-05 Welcome", Path::new("/m/x"));
        assert_eq!(
            (t.disc, t.track, t.title.as_str()),
            (Some(1), Some(5), "Welcome")
        );
        let t = from_name("Artista - Música", Path::new("/m/Downloads"));
        assert_eq!(
            (t.track, t.artist.as_str(), t.title.as_str()),
            (None, "Artista", "Música")
        );
    }

    #[test]
    fn roots() {
        let t = tags();
        let levels = folders("{album artist}/{album} ({year})", &t, " - ");
        let root = |d: &str| library_root(Path::new(d), &t, &levels);
        assert_eq!(
            root("/music/Pink Floyd/Wish You Were Here"),
            Path::new("/music")
        );
        assert_eq!(
            root("/music/Pink Floyd/Wish You Were Here/CD1"),
            Path::new("/music")
        );
        assert_eq!(
            root("/music/Pink Floyd - Wish You Were Here [FLAC]"),
            Path::new("/music")
        );
        assert_eq!(root("/downloads/novos"), Path::new("/downloads/novos"));
        assert_eq!(root("/music/Pink Floyd/WYWH"), Path::new("/music"));
        let t = AudioTags {
            title: "Celice".into(),
            artist: "A-Ha".into(),
            album: "Analogue (20th Anniversary Edition)".into(),
            year: Some(2026),
            ..Default::default()
        };
        let levels = folders("{album artist}/{album} ({year})", &t, " - ");
        assert_eq!(
            library_root(
                Path::new(
                    "/m/A-Ha - Analogue (20th Anniversary Edition) (2026) [FLAC] (24bit-96kHz)/CD 01"
                ),
                &t,
                &levels
            ),
            Path::new("/m")
        );
    }

    #[test]
    fn albums_agree() {
        let song = |artist: &str, year: Option<u16>| AudioTags {
            title: "x".into(),
            artist: artist.into(),
            album: "Аэростат 1037 - То да се 42 (2025-05-04)".into(),
            year,
            ..Default::default()
        };
        let dir = PathBuf::from("/m/Аэростат (1000 - ..)/1037 - То да се 42");
        let mut songs = vec![
            (dir.clone(), song("BG", Some(2025))),
            (dir.clone(), song("Moby", Some(2025))),
            (dir.clone(), song("Patti Smith", None)),
        ];
        fill_albums(&mut songs);
        assert!(
            songs
                .iter()
                .all(|(_, t)| t.year == Some(2025) && t.album_artist == VARIOUS_ARTISTS)
        );
        let t = songs[0].1.clone();
        let t = &t;
        // Most songs by one artist, one with a guest: that artist's album.
        let mut songs = vec![
            (dir.clone(), song("Korn", None)),
            (dir.clone(), song("Korn", None)),
            (dir.clone(), song("Korn feat. Nas", None)),
        ];
        fill_albums(&mut songs);
        assert!(songs.iter().all(|(_, t)| t.album_artist == "Korn"));
        let levels = folders("{album artist}/{album} ({year})", t, " - ");
        assert_eq!(
            levels,
            [
                "Various Artists",
                "Аэростат 1037 - То да се 42 (2025-05-04)"
            ]
        );
        assert_eq!(
            library_root(&dir, t, &levels),
            Path::new("/m/Аэростат (1000 - ..)")
        );
        // One artist throughout: no compilation.
        let mut songs = vec![(dir.clone(), song("Korn", None)), (dir, song("Korn", None))];
        fill_albums(&mut songs);
        assert_eq!(songs[0].1.album_artist(), "Korn");
    }

    #[test]
    fn bonus_singles_compilations() {
        let song = |title: &str, artist: &str, album: &str, track: u32| AudioTags {
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track: Some(track),
            ..Default::default()
        };
        let dir = PathBuf::from("/m/Jonathan Davis/2018 - Black Labyrinth");
        let mut songs = vec![
            (
                dir.clone(),
                song("Underneath My Skin", "Jonathan Davis", "Black Labyrinth", 1),
            ),
            (
                dir.clone(),
                song("Final Days", "Jonathan Davis", "Black Labyrinth", 2),
            ),
            (
                dir.clone(),
                song("Everyone", "Jonathan Davis", "Black Labyrinth", 3),
            ),
            (
                dir.clone(),
                song("Got Money", "Jonathan Davis & The SFA", "Single", 4),
            ),
            (
                dir.clone(),
                song("Silent Hill", "Jonathan Davis", "Silent Hill Downpour", 5),
            ),
        ];
        fill_albums(&mut songs);
        // Bonus tracks join the album.
        assert!(songs.iter().all(|(_, t)| t.album == "Black Labyrinth"));
        assert_eq!(songs[3].1.album_artist(), "Jonathan Davis");
        let mut songs = vec![
            (
                dir.clone(),
                song("Underneath My Skin", "Jonathan Davis", "Black Labyrinth", 1),
            ),
            (
                dir.clone(),
                song("Final Days", "Jonathan Davis", "Black Labyrinth", 2),
            ),
            (
                dir.clone(),
                song("Everyone", "Jonathan Davis", "Black Labyrinth", 3),
            ),
            (
                dir.clone(),
                song("Got Money", "Jonathan Davis & The SFA", "Single", 2),
            ),
        ];
        fill_albums(&mut songs);
        // "Single" is no album's name; a song numbered among the album's
        // own is no bonus track.
        assert_eq!(songs[3].1.album, "Got Money");
        // No majority (a folder of loose songs): each keeps its album.
        let misc = PathBuf::from("/m/Korn/Misc");
        let mut songs = vec![
            (misc.clone(), song("Camel Song", "Korn", "End of Days", 10)),
            (misc.clone(), song("Haze", "Korn", "Haze", 11)),
            (misc.clone(), song("Appears", "Korn", "Single", 7)),
        ];
        fill_albums(&mut songs);
        assert_eq!(songs[0].1.album, "End of Days");
        assert_eq!(songs[2].1.album, "Appears");

        // Songs by several artists: the artist goes into the file name.
        let show = PathBuf::from("/m/Аэростат (1000 - ..)/1000 - Творчество");
        let ep = "Аэростат 1000 - Творчество (2024-08-04)";
        let mut songs = vec![
            (
                show.clone(),
                AudioTags {
                    album_artist: "BG".into(),
                    ..song("Intro", "BG", ep, 1)
                },
            ),
            (
                show.clone(),
                AudioTags {
                    album_artist: "BG".into(),
                    ..song("Orange and Red Beams", "Eric Burdon", ep, 3)
                },
            ),
            (
                show.clone(),
                AudioTags {
                    album_artist: "BG".into(),
                    ..song("I Want You", "Bob Dylan", ep, 5)
                },
            ),
        ];
        fill_albums(&mut songs);
        assert_eq!(
            expand("{faixa} - {título}", &songs[1].1, " - ", true),
            "03 - Eric Burdon - Orange and Red Beams"
        );
        assert_eq!(
            expand("{faixa} - {artista} - {título}", &songs[1].1, " - ", true),
            "03 - Eric Burdon - Orange and Red Beams"
        );
        // One artist with a guest: no compilation.
        let mut songs = vec![
            (dir.clone(), song("Right Now", "Korn", "Mirror", 1)),
            (dir.clone(), song("Break Some Off", "Korn", "Mirror", 2)),
            (dir.clone(), song("Counting on Me", "Korn", "Mirror", 3)),
            (
                dir.clone(),
                song("Here It Comes Again", "Korn", "Mirror", 4),
            ),
            (dir.clone(), song("Play Me", "Korn feat. Nas", "Mirror", 5)),
        ];
        fill_albums(&mut songs);
        assert_eq!(
            expand("{faixa} - {título}", &songs[4].1, " - ", true),
            "05 - Play Me"
        );
    }

    #[test]
    fn collections() {
        assert!(names_collection(
            "Аэростат (1000 - ..)",
            "Аэростат 1037 - То да се 42 (2025-05-04)"
        ));
        assert!(!names_collection("Músicas", "Untouchables"));
        assert!(!names_collection("Korn", "Untouchables"));
        // A typo in the tag or in the folder name.
        let t = AudioTags {
            album: "Аэростат 1053 - Сага о Иннокентии".into(),
            album_artist: "BG".into(),
            ..Default::default()
        };
        let levels = folders("{album artist}/{album}", &t, " - ");
        assert_eq!(
            library_root(
                Path::new("/m/Аэростат (1000 - ..)/1053 - Сага об Иннокентии"),
                &t,
                &levels
            ),
            Path::new("/m/Аэростат (1000 - ..)")
        );
        // A year in the folder's and the album's names.
        let t = AudioTags {
            album: "Аэростат 1012 - Samhain 2024 (2024-10-27)".into(),
            album_artist: "BG".into(),
            ..Default::default()
        };
        let levels = folders("{album artist}/{album}", &t, " - ");
        assert_eq!(
            library_root(
                Path::new("/m/Аэростат (1000 - ..)/1012 - Samhain 2024"),
                &t,
                &levels
            ),
            Path::new("/m/Аэростат (1000 - ..)")
        );
        // The artist after the album, a year inside the album's name.
        let t = AudioTags {
            album:
                "Beethoven - Piano Sonatas, Opp. 109, 110 & 111 (Live at Suntory Hall, Tokyo, 2025)"
                    .into(),
            album_artist: "Mitsuko Uchida".into(),
            year: Some(2026),
            ..Default::default()
        };
        let levels = folders("{album artist}/{album} ({year})", &t, " - ");
        assert_eq!(
            library_root(
                Path::new(
                    "/m/Beethoven - Piano Sonatas, Opp. 109, 110 & 111 - Mitsuko Uchida (2026) [24-192]"
                ),
                &t,
                &levels
            ),
            Path::new("/m")
        );
        // A date in the album's name: no year again.
        let t = AudioTags {
            album: "Show 1096 (2025-06-21)".into(),
            year: Some(2026),
            ..Default::default()
        };
        assert_eq!(
            folders("{album} ({year})", &t, " - "),
            ["Show 1096 (2025-06-21)"]
        );
    }

    #[test]
    fn audio_extensions() {
        assert!(is_audio("MP3") && is_audio("flac") && is_audio("m4a") && is_audio("opus"));
        assert!(!is_audio("mp4") && !is_audio("jpg") && !is_audio("lrc"));
    }
}
