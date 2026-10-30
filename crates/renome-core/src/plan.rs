//! Preview: applies the rules to the visible files and flags conflicts.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::apply::Operation;
use crate::books::BookMatch;
use crate::comics::{self, ComicMatch};
use crate::files::{FileEntry, join_name, natural_cmp, split_name};
use crate::filter::Filter;
use crate::music::{self, AudioTags};
use crate::musicbrainz::SongMatch;
use crate::rules::{Ctx, Move, Pipeline, Rule, RuleError, Step, any_song_tags, song_tags};
use crate::text;
use crate::tmdb::{self, MovieMatch};
use crate::tv::{self, EpisodeMatch};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Unchanged,
    Ok,
    Conflict,
}

/// Reason for a conflict (translated by the UI).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Problem {
    /// Empty name, `.` or `..`.
    Empty,
    /// Contains `/` or a null character.
    Slash,
    /// Longer than the file system takes: 255 bytes (UTF-8), or on
    /// Windows 255 UTF-16 units.
    TooLong,
    /// On Windows: contains `\ : * ? " < > |` or ends in a dot or space.
    WindowsChars,
    /// The same, on a Linux disk formatted for Windows (FAT, exFAT).
    DiskChars,
    /// Another file in the batch would get the same name.
    Duplicate,
    /// An identical copy of a file that gets that name: it stays put.
    Copy,
    /// A file with that name already exists outside the batch.
    Exists,
}

/// How the list is shown: which files are included and in what order.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct View {
    pub filter: String,
    /// Sorts the preview by new name (otherwise keeps the list order).
    pub sort_by_new_name: bool,
    pub descending: bool,
    /// Shows only files with an identical copy in the list, grouped.
    pub identical_only: bool,
    /// Group of each file with identical copies (filled in by the app).
    #[serde(skip)]
    pub identical: HashMap<PathBuf, usize>,
    /// The movie chosen for each file (TMDB), for the movie rule.
    pub movies: HashMap<PathBuf, MovieMatch>,
    /// The episode chosen for each file (TMDB), for the series rule.
    pub episodes: HashMap<PathBuf, EpisodeMatch>,
    /// Each song's details from the release chosen on MusicBrainz, for the
    /// music rule (they replace the tags').
    pub songs: HashMap<PathBuf, SongMatch>,
    /// The book chosen for each file (Open Library), for the book rule.
    pub books: HashMap<PathBuf, BookMatch>,
    /// The issue chosen for each file (Comic Vine), for the comic rule;
    /// other comics are named from what they say about themselves.
    #[serde(default)]
    pub comics: HashMap<PathBuf, crate::comics::ComicMatch>,
    /// What an old folder is called after when files stay in it
    /// (`Show (2010) [tmdbid-7] (additional files)`), in the UI's language.
    #[serde(default)]
    pub leftover_suffix: String,
    /// Folders never taken for a show's or movie's own, nor renamed for
    /// leftovers (home, Downloads…; filled in by the app).
    #[serde(skip)]
    pub protected: Vec<PathBuf>,
}

/// An old folder (a show's, movie's, album's…) that keeps files after the
/// others moved to its new folder next to it (extras, a file in conflict):
/// renamed after the new one, so both are found together.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftFolder {
    pub from: PathBuf,
    pub to: PathBuf,
    /// Files left in it.
    pub files: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewItem {
    pub path: PathBuf,
    pub dir: PathBuf,
    pub size: u64,
    pub modified: Option<i64>,
    pub is_dir: bool,
    pub original: String,
    pub new_name: String,
    /// Folder the file moves into (`Show (2008)/Season 01`), shown to the
    /// user; it is inside `dest`'s root, not always inside `dir`.
    pub folder: Option<String>,
    /// The folder the file ends up in, when it moves.
    pub dest: Option<PathBuf>,
    pub status: Status,
    pub problem: Option<Problem>,
    /// Non-blocking warning: contains characters Windows does not accept.
    pub windows_unsafe: bool,
    pub manual: bool,
    /// A song the music rule cannot name (no title in its tags).
    pub untagged: bool,
    /// A song matched to a release whose tags say something else (the
    /// music rule can write them).
    pub retag: bool,
    /// A matched song with tags that cleaning would take away.
    pub junk: bool,
    /// The name the lookups (movie, series, book, comic) read: what the
    /// rules above the first lookup rule make of it, when not the original.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lookup_name: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub items: Vec<PreviewItem>,
    pub rule_errors: Vec<RuleError>,
    pub filter_error: Option<String>,
    /// Total files in the list, including those hidden by the filter.
    pub total: usize,
    pub changed: usize,
    pub conflicts: usize,
    /// Old folders renamed for what stays in them.
    pub left_folders: Vec<LeftFolder>,
}

impl PreviewItem {
    /// Where the file ends up.
    pub fn target(&self) -> PathBuf {
        self.dest.as_ref().unwrap_or(&self.dir).join(&self.new_name)
    }
}

impl Preview {
    /// Operations to run (only changed rows without conflicts).
    pub fn operations(&self) -> Vec<Operation> {
        self.items
            .iter()
            .filter(|i| i.status == Status::Ok)
            .map(|i| Operation {
                from: i.path.clone(),
                to: i.target(),
            })
            // Shallower than the files leaving them: renamed after.
            .chain(self.left_folders.iter().map(|l| Operation {
                from: l.from.clone(),
                to: l.to.clone(),
            }))
            .collect()
    }
}

const WINDOWS_UNSAFE: &[char] = &['\\', ':', '*', '?', '"', '<', '>', '|'];

/// A name Windows does not accept: its forbidden characters, a dot or space
/// at the end, or a device's name (`CON`, `NUL`, `COM1`…, with any
/// extension).
fn windows_bad(name: &str) -> bool {
    let base = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_uppercase();
    let device = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base.as_bytes()[3].is_ascii_digit()
            && base.as_bytes()[3] != b'0');
    name.contains(WINDOWS_UNSAFE) || name.ends_with(['.', ' ']) || device
}

/// A name's length as the system counts it against its limit of 255.
fn name_length(name: &str) -> usize {
    if cfg!(windows) {
        name.encode_utf16().count()
    } else {
        name.len()
    }
}

fn validate(name: &str) -> Option<Problem> {
    if name.is_empty() || name == "." || name == ".." {
        Some(Problem::Empty)
    } else if name.contains(['/', '\0']) {
        Some(Problem::Slash)
    } else if cfg!(windows) && windows_bad(name) {
        Some(Problem::WindowsChars)
    } else if name_length(name) > 255 {
        Some(Problem::TooLong)
    } else {
        None
    }
}

#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::symlink_metadata(a), std::fs::symlink_metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
        _ => false,
    }
}

/// Keys for comparing paths: where names ignore case, `A.txt` and `a.txt`
/// are the same target. That is the default on Windows and macOS; on Linux,
/// disks formatted for them (exFAT, FAT, NTFS mounted so) do it too, found
/// by looking an existing name up in the other case.
#[derive(Default)]
struct PathKeys {
    folded: HashMap<PathBuf, bool>,
}

impl PathKeys {
    fn key(&mut self, p: &Path) -> PathBuf {
        let dir = p.parent().unwrap_or(p);
        if self.ignores_case(dir) {
            PathBuf::from(p.to_string_lossy().to_lowercase())
        } else {
            p.to_path_buf()
        }
    }

    /// Whether names in `dir` (or in its nearest existing folder) ignore case.
    fn ignores_case(&mut self, dir: &Path) -> bool {
        if cfg!(any(windows, target_os = "macos")) {
            return true;
        }
        let Some(dir) = dir.ancestors().find(|d| d.is_dir()) else {
            return false;
        };
        if let Some(&v) = self.folded.get(dir) {
            return v;
        }
        let swap = |s: &str| -> String {
            s.chars()
                .flat_map(|c| -> Vec<char> {
                    if c.is_lowercase() {
                        c.to_uppercase().collect()
                    } else {
                        c.to_lowercase().collect()
                    }
                })
                .collect()
        };
        // A name in it that reads differently in the other case.
        let v = std::fs::read_dir(dir).ok().and_then(|rd| {
            rd.flatten().find_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let other = swap(&name);
                (other != name).then(|| same_file(&dir.join(&name), &dir.join(other)))
            })
        });
        let v = v.unwrap_or(false);
        self.folded.insert(dir.to_path_buf(), v);
        v
    }
}

/// Without inodes (Windows), compares canonical paths, which there carry
/// the real on-disk casing.
#[cfg(not(unix))]
fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Folders spelled differently only in case (`os vagalumes` and `Os Vagalumes` from
/// different albums' tags) become one: an existing folder's spelling, else
/// the most common in the batch (the first on a tie).
fn unify_folders(items: &mut [PreviewItem]) {
    let levels_of = |i: &PreviewItem| -> Option<(PathBuf, Vec<String>)> {
        let levels: Vec<String> = i.folder.as_ref()?.split('/').map(str::to_string).collect();
        let root = i
            .dest
            .as_ref()?
            .ancestors()
            .nth(levels.len())?
            .to_path_buf();
        Some((root, levels))
    };
    // The same folder in either Unicode form of its accents is one too.
    let key = |root: &Path, levels: &[String]| {
        format!(
            "{}\0{}",
            root.display(),
            text::nfc(&levels.join("/").to_lowercase())
        )
    };
    // Spellings of each folder, in order of appearance, with counts.
    let mut spellings: HashMap<String, Vec<(String, usize)>> = HashMap::new();
    for i in items.iter() {
        let Some((root, levels)) = levels_of(i) else {
            continue;
        };
        for k in 0..levels.len() {
            let v = spellings.entry(key(&root, &levels[..=k])).or_default();
            match v.iter_mut().find(|(s, _)| *s == levels[k]) {
                Some(e) => e.1 += 1,
                None => v.push((levels[k].clone(), 1)),
            }
        }
    }
    if spellings.values().all(|v| v.len() < 2) {
        return;
    }
    let mut listed: HashMap<PathBuf, Vec<String>> = HashMap::new();
    for i in items.iter_mut() {
        let Some((root, levels)) = levels_of(i) else {
            continue;
        };
        let mut dir = root.clone();
        let mut chosen = Vec::new();
        for k in 0..levels.len() {
            let on_disk = listed
                .entry(dir.clone())
                .or_insert_with(|| {
                    std::fs::read_dir(&dir)
                        .map(|rd| {
                            rd.flatten()
                                .filter_map(|e| e.file_name().into_string().ok())
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .iter()
                .find(|n| text::nfc(&n.to_lowercase()) == text::nfc(&levels[k].to_lowercase()))
                .cloned();
            let name = on_disk.unwrap_or_else(|| {
                let v = &spellings[&key(&root, &levels[..=k])];
                let most = v.iter().map(|(_, n)| *n).max().unwrap_or(0);
                v.iter()
                    .find(|(_, n)| *n == most)
                    .map(|(s, _)| s.clone())
                    .unwrap_or_default()
            });
            dir = dir.join(&name);
            chosen.push(name);
        }
        i.folder = Some(chosen.join("/"));
        i.dest = Some(dir);
    }
}

/// Folders some songs are moved out of as old artist or album folders
/// (`Banda Azul [restored]` for Banda Azul's albums) are not where other songs in them
/// should get their new folders either (`Banda Azul [restored]/Rui Prado`):
/// those go up past them too.
fn climb_shared_roots(items: &mut [PreviewItem]) {
    let levels_of = |i: &PreviewItem| -> Option<(PathBuf, usize)> {
        let n = i.folder.as_ref()?.split('/').count();
        Some((i.dest.as_ref()?.ancestors().nth(n)?.to_path_buf(), n))
    };
    let mut left: HashSet<PathBuf> = HashSet::new();
    for i in items.iter() {
        let Some((root, _)) = levels_of(i) else {
            continue;
        };
        for dir in i
            .dir
            .ancestors()
            .take_while(|d| *d != root && d.starts_with(&root))
        {
            left.insert(dir.to_path_buf());
        }
    }
    if left.is_empty() {
        return;
    }
    for i in items.iter_mut() {
        let Some((root, n)) = levels_of(i) else {
            continue;
        };
        if !left.contains(&root) {
            continue;
        }
        let mut up = root.clone();
        while left.contains(&up) {
            match up.parent() {
                Some(p) => up = p.to_path_buf(),
                None => break,
            }
        }
        let dest = i.dest.as_ref().unwrap();
        let tail: Vec<_> = dest
            .components()
            .skip(dest.components().count() - n)
            .collect();
        let new_dest = tail.iter().fold(up, |p, c| p.join(c));
        i.dest = Some(new_dest);
    }
}

/// A folder of loose songs (`Misc`, `Singles`): at least three songs from
/// at least three albums, nearly all of which (four in five) have only one
/// or two songs there. It is a collection someone put together, not an
/// album: its songs are renamed but stay in it, instead of each going to a
/// folder of its own album.
fn keep_collections(items: &mut [PreviewItem], songs: &HashMap<&Path, AudioTags>) {
    let mut albums: HashMap<&Path, HashMap<String, usize>> = HashMap::new();
    for i in items.iter() {
        if let Some(t) = songs.get(i.path.as_path()) {
            *albums
                .entry(i.dir.as_path())
                .or_default()
                .entry(t.album.to_lowercase())
                .or_default() += 1;
        }
    }
    let loose: HashSet<PathBuf> = albums
        .into_iter()
        .filter(|(_, by_album)| {
            let songs: usize = by_album.values().sum();
            let small = by_album.values().filter(|n| **n <= 2).count();
            songs >= 3 && by_album.len() >= 3 && small * 5 >= by_album.len() * 4
        })
        .map(|(dir, _)| dir.to_path_buf())
        .collect();
    for i in items.iter_mut() {
        if i.dest.is_some() && loose.contains(&i.dir) && songs.contains_key(i.path.as_path()) {
            i.dest = None;
            i.folder = None;
            if i.status == Status::Ok && i.new_name == i.original && i.problem.is_none() {
                i.status = Status::Unchanged;
            }
        }
    }
}

/// A folder that already gathers a collection (a show's folder,
/// `Аэростат (1000 - ..)` for "Аэростат 1037 - …") needs no artist folders
/// inside it: when most songs going into it have albums named after the
/// folder, the artist level is dropped for all of them.
fn skip_artist_level(items: &mut [PreviewItem], songs: &HashMap<&Path, AudioTags>) {
    #[derive(Default)]
    struct Root {
        named: usize,
        /// Every song has an artist level to drop.
        deep: bool,
        members: Vec<usize>,
    }
    let mut roots: HashMap<PathBuf, Root> = HashMap::new();
    for (n, i) in items.iter().enumerate() {
        let (Some(t), Some(folder), Some(dest)) = (songs.get(i.path.as_path()), &i.folder, &i.dest)
        else {
            continue;
        };
        let levels: Vec<&str> = folder.split('/').collect();
        let Some(root) = dest.ancestors().nth(levels.len()) else {
            continue;
        };
        let name = root.file_name().unwrap_or_default().to_string_lossy();
        let named = levels.len() > 1 && music::names_collection(&name, &t.album);
        let r = roots.entry(root.to_path_buf()).or_insert(Root {
            deep: true,
            ..Default::default()
        });
        r.named += usize::from(named);
        r.deep &= levels.len() > 1;
        r.members.push(n);
    }
    for (root, r) in roots
        .into_iter()
        .filter(|(_, r)| r.deep && r.named * 2 > r.members.len())
    {
        for n in r.members {
            let i = &mut items[n];
            let rest: Vec<String> = i
                .folder
                .as_deref()
                .unwrap_or_default()
                .split('/')
                .skip(1)
                .map(str::to_string)
                .collect();
            let dest = rest.iter().fold(root.clone(), |p, l| p.join(l));
            if dest == i.dir {
                // Already where it belongs.
                i.dest = None;
                i.folder = None;
                if i.new_name == i.original && i.status == Status::Ok {
                    i.status = Status::Unchanged;
                }
            } else {
                i.folder = Some(rest.join("/"));
                i.dest = Some(dest);
            }
        }
    }
}

/// A folder holding one work and nothing else of its kind, whatever its
/// name (`New folder`, `Downloads/stuff`): one album of songs, one series
/// of comics, one book in several formats. It is the work's own folder, so the new folders go next to
/// it, not inside. `keys`: each file's work; `kind`: the extensions
/// counted; `sub`: subfolders that are part of it (disc folders).
fn place_own_folders(
    items: &mut [PreviewItem],
    keys: &HashMap<&Path, String>,
    kind: fn(&str) -> bool,
    sub: fn(&str) -> bool,
    view: &View,
) {
    let root_of = |i: &PreviewItem| -> Option<PathBuf> {
        let n = i.folder.as_ref()?.split('/').count();
        Some(i.dest.as_ref()?.ancestors().nth(n)?.to_path_buf())
    };
    let mut roots: HashMap<PathBuf, Option<&String>> = HashMap::new();
    for i in items.iter() {
        let (Some(key), Some(root)) = (keys.get(i.path.as_path()), root_of(i)) else {
            continue;
        };
        // Its files' own folder (or the one above disc folders), not one
        // reached by names (`Music` above `Artist/Album`).
        let own = i.dir == root
            || (i.dir.parent() == Some(root.as_path())
                && sub(&i.dir.file_name().unwrap_or_default().to_string_lossy()));
        let key = own.then_some(key);
        roots
            .entry(root)
            .and_modify(|r| {
                if *r != key {
                    *r = None;
                }
            })
            .or_insert(key);
    }
    for (root, key) in roots {
        let (Some(key), Some(up)) = (key, root.parent()) else {
            continue;
        };
        if protected(&root, view) {
            continue;
        }
        // Every file of its kind in it (and in its subfolders) is that work.
        let only = walkdir::WalkDir::new(&root)
            .max_depth(3)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| {
                !e.file_type().is_dir()
                    && kind(
                        &e.path()
                            .extension()
                            .map(|x| x.to_string_lossy().to_lowercase())
                            .unwrap_or_default(),
                    )
            })
            .all(|e| keys.get(e.path()) == Some(key));
        if !only {
            continue;
        }
        for i in items.iter_mut() {
            if root_of(i).as_ref() != Some(&root) {
                continue;
            }
            let Some(rest) = i.dest.as_ref().and_then(|d| d.strip_prefix(&root).ok()) else {
                continue;
            };
            let dest = up.join(rest);
            if dest == i.dir {
                i.dest = None;
                i.folder = None;
                if i.new_name == i.original && i.status == Status::Ok {
                    i.status = Status::Unchanged;
                }
            } else {
                i.dest = Some(dest);
            }
        }
    }
}

/// Files that go with songs, when no rule changed them:
/// - lyrics with a song's name (`01 Song.lrc`) get its new name;
/// - with folders, other files (covers, cue sheets, logs) go with their
///   folder's songs when all of them go to one place; files in a subfolder
///   of such a folder (`Artwork/`) keep their subfolder; and files in a
///   folder whose songs are all in disc folders (`CD 01`, `CD 02`) going
///   to one place go there too. Two such files that would get the same
///   name (`CD 01/log.txt`, `CD 02/log.txt`) get their folder's name added.
fn follow_songs(items: &mut [PreviewItem], folders: bool, identical: &HashMap<PathBuf, usize>) {
    /// Where the songs of a folder go: where most of them go, or nowhere
    /// in particular.
    #[derive(Clone, PartialEq)]
    enum Album {
        One(Option<PathBuf>, Option<String>),
        Split,
    }
    type Place = (Option<PathBuf>, Option<String>);
    let is_song = |i: &PreviewItem| !i.is_dir && music::is_audio(split_name(&i.original).1);
    /// A song's new stem, destination and folder shown.
    type Song = (String, Option<PathBuf>, Option<String>);
    let mut by_stem: HashMap<(PathBuf, String), Song> = HashMap::new();
    let mut places: HashMap<PathBuf, Vec<(Place, usize)>> = HashMap::new();
    for i in items
        .iter()
        .filter(|i| is_song(i) && i.status != Status::Conflict)
    {
        let stem = split_name(&i.original).0.to_string();
        let new_stem = split_name(&i.new_name).0.to_string();
        by_stem.insert(
            (i.dir.clone(), stem),
            (new_stem, i.dest.clone(), i.folder.clone()),
        );
        let here = (i.dest.clone(), i.folder.clone());
        let counts = places.entry(i.dir.clone()).or_default();
        match counts.iter_mut().find(|(p, _)| *p == here) {
            Some(c) => c.1 += 1,
            None => counts.push((here, 1)),
        }
    }
    // Bonus tracks tagged with other albums go their way; the folder's
    // other files go with most of its songs.
    let albums: HashMap<PathBuf, Album> = places
        .into_iter()
        .map(|(dir, counts)| {
            let total: usize = counts.iter().map(|(_, n)| n).sum();
            let album = match counts.into_iter().max_by_key(|(_, n)| *n) {
                Some(((dest, folder), n)) if n * 2 > total => Album::One(dest, folder),
                _ => Album::Split,
            };
            (dir, album)
        })
        .collect();
    // Where a song-less folder's files go, if anywhere.
    let place = |dir: &Path| -> Option<(PathBuf, String)> {
        if let Some(a) = albums.get(dir) {
            return match a {
                Album::One(Some(dest), Some(folder)) => Some((dest.clone(), folder.clone())),
                _ => None,
            };
        }
        // A subfolder of an album folder (`Artwork`).
        for up in dir.ancestors().skip(1).take(2) {
            if let Some(a) = albums.get(up) {
                let Album::One(Some(dest), Some(folder)) = a else {
                    return None;
                };
                let rel = dir.strip_prefix(up).ok()?;
                let rel_shown = rel.to_string_lossy().replace('\\', "/");
                return Some((dest.join(rel), format!("{folder}/{rel_shown}")));
            }
        }
        // The folder above disc folders.
        let mut found: Option<&Album> = None;
        for (d, a) in &albums {
            if d.parent() == Some(dir) {
                let name = d.file_name().unwrap_or_default().to_string_lossy();
                if music::disc_folder(&name).is_none() || found.is_some_and(|f| f != a) {
                    return None;
                }
                found = Some(a);
            }
        }
        match found {
            Some(Album::One(Some(dest), Some(folder))) => Some((dest.clone(), folder.clone())),
            _ => None,
        }
    };
    let mut followers = Vec::new();
    for (n, i) in items.iter_mut().enumerate() {
        if i.is_dir || i.manual || is_song(i) || i.new_name != i.original || i.dest.is_some() {
            continue;
        }
        let (stem, ext) = split_name(&i.original);
        if let Some((new_stem, dest, folder)) = by_stem.get(&(i.dir.clone(), stem.to_string())) {
            i.new_name = join_name(new_stem, ext);
            i.dest = dest.clone();
            i.folder = folder.clone();
        } else if folders && let Some((dest, folder)) = place(&i.dir) {
            i.dest = Some(dest);
            i.folder = Some(folder);
        } else {
            continue;
        }
        if i.status == Status::Unchanged {
            i.status = Status::Ok;
        }
        i.windows_unsafe = windows_bad(&i.new_name);
        followers.push(n);
    }
    // Same name in the same place: add the folder each came from (identical
    // copies are left to the duplicate check, which lets one through).
    let mut keys = PathKeys::default();
    let mut by_target: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for &n in &followers {
        by_target
            .entry(keys.key(&items[n].target()))
            .or_default()
            .push(n);
    }
    let copies = |g: &Vec<usize>| {
        let first = identical.get(&items[g[0]].path);
        first.is_some() && g.iter().all(|&n| identical.get(&items[n].path) == first)
    };
    let clashes: Vec<Vec<usize>> = by_target
        .into_values()
        .filter(|g| g.len() > 1 && !copies(g))
        .collect();
    for group in &clashes {
        for &n in group {
            let i = &mut items[n];
            let from = i
                .dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let (stem, ext) = split_name(&i.new_name);
            i.new_name = join_name(&format!("{stem} ({from})"), ext);
        }
    }
}

/// Computes the preview. `overrides` maps path → hand-edited name.
/// Files hidden by the filter are left out (and will not be renamed);
/// numbering only counts the visible ones.
pub fn preview(
    files: &[FileEntry],
    steps: &[Step],
    overrides: &HashMap<PathBuf, String>,
    view: &View,
) -> Preview {
    let pipeline = Pipeline::new(steps);
    let now = Local::now();
    let (filter, filter_error) = match Filter::new(&view.filter) {
        Ok(f) => (f, None),
        Err(e) => (Filter::default(), Some(e)),
    };

    let music = pipeline.music();
    // Songs' tags, with the album-wide fields agreed across each album.
    let songs: HashMap<&Path, AudioTags> = match music {
        None => HashMap::new(),
        Some((from_name, _)) => {
            let visible: Vec<&FileEntry> = files
                .iter()
                .filter(|f| filter.matches(&f.file_name()))
                .collect();
            let mut tags: Vec<(PathBuf, AudioTags)> = Vec::new();
            let mut paths: Vec<&Path> = Vec::new();
            for f in visible {
                // A song without a title is named from its release, when it
                // was identified by its sound.
                let t = song_tags(f, from_name)
                    .or_else(|| view.songs.get(&f.path).and_then(|_| any_song_tags(f)));
                if let Some(mut t) = t {
                    if let Some(m) = view.songs.get(&f.path) {
                        m.apply(&mut t);
                    }
                    tags.push((f.dir.clone(), t));
                    paths.push(&f.path);
                }
            }
            music::fill_albums(&mut tags);
            paths
                .into_iter()
                .zip(tags.into_iter().map(|(_, t)| t))
                .collect()
        }
    };
    // The files shown, with their place in the list and in their folder.
    let mut per_folder: HashMap<&Path, usize> = HashMap::new();
    let shown: Vec<(usize, usize, &FileEntry)> = files
        .iter()
        .filter(|f| filter.matches(&f.file_name()))
        .filter(|f| !view.identical_only || view.identical.contains_key(&f.path))
        .enumerate()
        .map(|(index, file)| {
            let n = per_folder.entry(&file.dir).or_default();
            *n += 1;
            (index, *n - 1, file)
        })
        .collect();
    let ctx_of = |index, folder_index, file| Ctx {
        index,
        folder_index,
        file,
        now,
        // Each lookup names only its own kind of file.
        movie: view
            .movies
            .get(&file.path)
            .filter(|_| file.is_dir || tmdb::is_screen(&file.ext)),
        episode: view
            .episodes
            .get(&file.path)
            .filter(|_| !file.is_dir && tmdb::is_screen(&file.ext)),
        song: songs.get(file.path.as_path()),
        book: view
            .books
            .get(&file.path)
            .filter(|_| !file.is_dir && crate::ebook::is_book(&file.ext)),
        comic: None,
        comic_folder: None,
    };
    // What each lookup reads (see `Pipeline::lookup_name`).
    let lookup: HashMap<&Path, (String, String)> = shown
        .iter()
        .filter_map(|&(index, folder_index, file)| {
            let name = pipeline.lookup_name(&ctx_of(index, folder_index, file))?;
            Some((file.path.as_path(), name))
        })
        .collect();
    // The comic rule names the batch's comics together (see
    // `comics::arrange`): what each is, and the series of its folder.
    let by_folder = steps.iter().find_map(|s| match &s.rule {
        Rule::Comic { by_folder, .. } if s.enabled => Some(*by_folder),
        _ => None,
    });
    let comics: HashMap<&Path, (ComicMatch, Option<ComicMatch>)> = match by_folder {
        None => HashMap::new(),
        Some(by_folder) => {
            let (paths, mut found): (Vec<&Path>, Vec<ComicMatch>) = shown
                .iter()
                .map(|&(_, _, f)| f)
                .filter(|f| !f.is_dir && comics::is_comic(&f.ext))
                .filter_map(|f| {
                    let stem = lookup.get(f.path.as_path()).map_or(&f.stem, |(s, _)| s);
                    let m = view
                        .comics
                        .get(&f.path)
                        .cloned()
                        .or_else(|| comics::from_name(f, stem, by_folder))?;
                    Some((f.path.as_path(), m))
                })
                .unzip();
            let folders = comics::arrange(&mut found);
            paths
                .into_iter()
                .zip(found.into_iter().zip(folders))
                .collect()
        }
    };
    let mut items: Vec<PreviewItem> = shown
        .iter()
        .map(|&(index, folder_index, file)| {
            let original = file.file_name();
            let lookup_name = lookup
                .get(file.path.as_path())
                .map(|(stem, ext)| join_name(stem, ext))
                .filter(|n| *n != original);
            // The rules emptied the name but left the extension: `.txt` would
            // be a hidden file with no name.
            let empty_stem;
            let mut folders: Vec<String> = Vec::new();
            let mut dest = None;
            // A name typed by hand replaces the rules' name, not the folder
            // they move the file into.
            let (new_name, manual) = {
                let ctx = Ctx {
                    comic: comics
                        .get(file.path.as_path())
                        .map(|(m, _)| m)
                        .or_else(|| view.comics.get(&file.path)),
                    comic_folder: comics
                        .get(file.path.as_path())
                        .and_then(|(_, f)| f.as_ref()),
                    ..ctx_of(index, folder_index, file)
                };
                let (stem, ext, moved) = pipeline.run_full(&ctx);
                empty_stem =
                    stem.is_empty() && !file.stem.is_empty() && !overrides.contains_key(&file.path);
                // A movie already in a folder with its name stays there.
                let inside = file
                    .dir
                    .file_name()
                    .is_some_and(|d| d.to_string_lossy() == stem);
                let (root, names) = match moved {
                    Some(Move::OwnFolder) if !inside => (file.dir.clone(), vec![stem.clone()]),
                    Some(Move::Into { root, folders }) => (root, folders),
                    _ => (file.dir.clone(), Vec::new()),
                };
                let to = names.iter().fold(root, |p, n| p.join(n));
                // Already in that folder: stay there.
                if to != file.dir {
                    folders = names;
                    dest = Some(to);
                }
                match overrides.get(&file.path) {
                    Some(name) => (name.clone(), true),
                    None => (join_name(&stem, &ext), false),
                }
            };
            let problem = if empty_stem {
                Some(Problem::Empty)
            } else {
                validate(&new_name).or_else(|| folders.iter().find_map(|f| validate(f)))
            };
            let folder = (!folders.is_empty()).then(|| folders.join("/"));
            let status = if problem.is_some() {
                Status::Conflict
            } else if new_name == original && dest.is_none() {
                Status::Unchanged
            } else {
                Status::Ok
            };
            let windows_unsafe = windows_bad(&new_name);
            let untagged = music.is_some_and(|_| {
                !file.is_dir
                    && music::is_audio(&file.ext)
                    && !songs.contains_key(file.path.as_path())
            });
            let own = file.audio_tags().cloned().unwrap_or_default();
            let matched = view.songs.get(&file.path);
            let retag = matched.is_some_and(|m| m.differs(&own));
            let junk = matched.is_some() && own.junk;
            PreviewItem {
                path: file.path.clone(),
                dir: file.dir.clone(),
                size: file.size,
                modified: file.modified,
                is_dir: file.is_dir,
                original,
                new_name,
                folder,
                dest,
                status,
                problem,
                windows_unsafe,
                manual,
                untagged,
                retag,
                junk,
                lookup_name,
            }
        })
        .collect();
    climb_shared_roots(&mut items);
    unify_folders(&mut items);
    place_show_folders(&mut items, view);
    place_movie_folders(&mut items, view);
    let series: HashMap<&Path, String> = comics
        .iter()
        .map(|(p, (m, folder))| (*p, folder.as_ref().unwrap_or(m).series.to_lowercase()))
        .filter(|(_, s)| !s.is_empty())
        .collect();
    place_own_folders(&mut items, &series, comics::is_comic, |_| false, view);
    let books: HashMap<&Path, String> = view
        .books
        .iter()
        .map(|(p, m)| {
            (
                p.as_path(),
                format!("{}\0{}", m.authors.join("\0"), m.title).to_lowercase(),
            )
        })
        .collect();
    place_own_folders(&mut items, &books, crate::ebook::is_book, |_| false, view);
    if let Some((_, folders)) = music {
        if folders {
            keep_collections(&mut items, &songs);
            skip_artist_level(&mut items, &songs);
            let albums: HashMap<&Path, String> = songs
                .iter()
                .filter(|(_, t)| !t.album.is_empty())
                .map(|(p, t)| {
                    (
                        *p,
                        format!("{}\0{}", t.album_artist(), t.album).to_lowercase(),
                    )
                })
                .collect();
            place_own_folders(
                &mut items,
                &albums,
                music::is_audio,
                |n| music::disc_folder(n).is_some(),
                view,
            );
        }
        follow_songs(&mut items, folders, &view.identical);
    }

    // Repeated targets within the batch. A file that keeps its name is not
    // flagged: the one moving onto it is, as "exists", below.
    let mut keys = PathKeys::default();
    let mut targets: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for (i, item) in items.iter().enumerate() {
        if item.status == Status::Ok {
            targets.entry(keys.key(&item.target())).or_default().push(i);
        }
    }
    for idx in targets.values().filter(|v| v.len() > 1) {
        // Identical copies (found in the background): the first goes
        // ahead, the others stay where they are.
        let first = view.identical.get(&items[idx[0]].path);
        let copies = first.is_some()
            && idx
                .iter()
                .all(|&i| view.identical.get(&items[i].path) == first);
        for &i in idx.iter().skip(usize::from(copies)) {
            items[i].status = Status::Conflict;
            items[i].problem = Some(if copies {
                Problem::Copy
            } else {
                Problem::Duplicate
            });
        }
    }

    // On Linux, disks formatted for Windows (FAT, exFAT: found by their
    // ignoring case) refuse what Windows refuses, so it is an error there.
    if cfg!(target_os = "linux") {
        for item in items.iter_mut().filter(|i| i.status == Status::Ok) {
            let bad = item.windows_unsafe
                || item
                    .folder
                    .as_deref()
                    .is_some_and(|f| f.split('/').any(windows_bad));
            let target = item.target();
            if bad && keys.ignores_case(target.parent().unwrap_or(&target)) {
                item.status = Status::Conflict;
                item.problem = Some(Problem::DiskChars);
            }
        }
    }

    // Target already exists on disk and is not moving away. Repeat until
    // stable, since a conflict keeps that file from being moved.
    loop {
        let moving: HashSet<PathBuf> = items
            .iter()
            .filter(|i| i.status == Status::Ok)
            .map(|i| keys.key(&i.path))
            .collect();
        let mut blocked = Vec::new();
        for (i, item) in items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.status == Status::Ok)
        {
            let target = item.target();
            let exists = std::fs::symlink_metadata(&target).is_ok();
            // Case-only change on a case-insensitive file system:
            // the "target" is the file itself.
            if exists && !moving.contains(&keys.key(&target)) && !same_file(&target, &item.path) {
                blocked.push(i);
            }
        }
        if blocked.is_empty() {
            break;
        }
        for i in blocked {
            items[i].status = Status::Conflict;
            items[i].problem = Some(Problem::Exists);
        }
    }

    if view.sort_by_new_name {
        items.sort_by(|a, b| {
            let ord = natural_cmp(&a.new_name, &b.new_name);
            if view.descending { ord.reverse() } else { ord }
        });
    }
    if view.identical_only {
        // Copies side by side; the order above holds within each group.
        items.sort_by_key(|i| view.identical.get(&i.path).copied());
    }

    let changed = items.iter().filter(|i| i.status == Status::Ok).count();
    let conflicts = items
        .iter()
        .filter(|i| i.status == Status::Conflict)
        .count();
    let left_folders = left_folders(&items, view);
    Preview {
        items,
        rule_errors: pipeline.errors,
        filter_error,
        total: files.len(),
        changed,
        conflicts,
        left_folders,
    }
}

/// Episodes moving into show folders, by show and by the folder they come
/// from (the one above a season folder): (show, old folder, items).
fn show_moves<'a>(
    items: &[PreviewItem],
    view: &'a View,
) -> Vec<(&'a EpisodeMatch, PathBuf, Vec<usize>)> {
    let mut groups: Vec<(&EpisodeMatch, PathBuf, Vec<usize>)> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let Some(m) = view.episodes.get(&item.path) else {
            continue;
        };
        if item.is_dir || item.dest.is_none() {
            continue;
        }
        let old = tv::show_root(&item.dir).to_path_buf();
        match groups.iter_mut().find(|g| g.0.id == m.id && g.1 == old) {
            Some(g) => g.2.push(i),
            None => groups.push((m, old, vec![i])),
        }
    }
    groups
}

/// The show's folder in a destination (`Show/Season 01`: `Show`).
fn show_folder_of(dest: &Path) -> &Path {
    let season = dest
        .file_name()
        .is_some_and(|n| tv::folder_season(&n.to_string_lossy()).is_some());
    match dest.parent() {
        Some(up) if season => up,
        _ => dest,
    }
}

/// A folder that may not be taken for one show's: the user's own folders,
/// a disk's top folder, the file system's.
fn protected(dir: &Path, view: &View) -> bool {
    let Some(up) = dir.parent().filter(|u| u.parent().is_some()) else {
        return true;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(a), Ok(b)) = (std::fs::metadata(dir), std::fs::metadata(up))
            && a.dev() != b.dev()
        {
            return true;
        }
    }
    view.protected.iter().any(|p| p == dir)
}

/// Whether every video in a folder (and its subfolders) is a listed episode
/// of the show: a show's folder, whatever its name.
fn only_show_videos(dir: &Path, id: u64, view: &View) -> bool {
    let mut any = false;
    for e in walkdir::WalkDir::new(dir).max_depth(4).into_iter() {
        let Ok(e) = e else { return false };
        let ext = e
            .path()
            .extension()
            .map(|x| x.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if e.file_type().is_dir() || !tmdb::is_video(&ext) {
            continue;
        }
        if view.episodes.get(e.path()).is_none_or(|m| m.id != id) {
            return false;
        }
        any = true;
    }
    any
}

/// A show's folder found by what it holds, not by its name (`Série ABC
/// Completa 1080p`): the show's new folder goes next to it, not inside.
fn place_show_folders(items: &mut [PreviewItem], view: &View) {
    for (m, old, idx) in show_moves(items, view) {
        let Some(dest) = items[idx[0]].dest.clone() else {
            continue;
        };
        let new = show_folder_of(&dest).to_path_buf();
        let (Some(name), Some(up)) = (new.file_name(), old.parent()) else {
            continue;
        };
        if new.parent() != Some(old.as_path())
            || tv::names_show(&old, m)
            || protected(&old, view)
            || !only_show_videos(&old, m.id, view)
        {
            continue;
        }
        let to = up.join(name);
        for i in idx {
            if let Some(rest) = items[i]
                .dest
                .as_ref()
                .and_then(|d| d.strip_prefix(&new).ok())
            {
                items[i].dest = Some(to.join(rest));
            }
        }
    }
}

/// Whether a folder holds a movie and only what goes with one: its videos
/// (no other), subtitles, images, `.nfo` and `.txt` files.
fn only_movie_files(dir: &Path, id: u64, view: &View) -> bool {
    let mut any = false;
    for e in walkdir::WalkDir::new(dir).max_depth(3).into_iter() {
        let Ok(e) = e else { return false };
        if e.file_type().is_dir() {
            continue;
        }
        let ext = e
            .path()
            .extension()
            .map(|x| x.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if tmdb::is_video(&ext) {
            if view.movies.get(e.path()).is_none_or(|m| m.id != id) {
                return false;
            }
            any = true;
        } else if !(tmdb::is_subtitle(&ext)
            || matches!(
                ext.as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "nfo" | "txt"
            ))
        {
            return false;
        }
    }
    any
}

/// A movie's own folder under another name: named after it
/// (`Movie.2010.1080p.BluRay`), or holding only it and what goes with it.
/// The movie's new folder goes next to it, not inside.
fn place_movie_folders(items: &mut [PreviewItem], view: &View) {
    // The movies in each folder: a folder of several is no movie's own.
    let mut movies: HashMap<PathBuf, HashSet<u64>> = HashMap::new();
    for i in items.iter().filter(|i| !i.is_dir) {
        if let Some(m) = view.movies.get(&i.path) {
            movies.entry(i.dir.clone()).or_default().insert(m.id);
        }
    }
    let mut own: HashMap<PathBuf, bool> = HashMap::new();
    for i in items.iter_mut() {
        let Some(m) = view.movies.get(&i.path) else {
            continue;
        };
        let (Some(dest), Some(up)) = (&i.dest, i.dir.parent()) else {
            continue;
        };
        let Some(name) = dest.file_name().filter(|_| dest.parent() == Some(&i.dir)) else {
            continue;
        };
        // A video alone tells nothing about a folder holding other things
        // (`Videos 2024` with a film and the family's documents).
        let is_own = *own.entry(i.dir.clone()).or_insert_with(|| {
            movies.get(&i.dir).is_some_and(|ids| ids.len() == 1)
                && !protected(&i.dir, view)
                && (tmdb::names_movie(&i.dir, m) || only_movie_files(&i.dir, m.id, view))
        });
        if is_own {
            i.dest = Some(up.join(name));
        }
    }
}

/// The folders files leave for a new one next to them (a show's, movie's,
/// album's, author's or series' folder named anew by the rules) that still
/// hold files afterwards (extras, a file in conflict, one hidden by the
/// filter): renamed `<new folder> (<suffix>)`, next to the new one. Without
/// anything left, the old folder is removed after the move instead.
fn left_folders(items: &[PreviewItem], view: &View) -> Vec<LeftFolder> {
    // Each old folder and the new one its files go to (None: several).
    let mut pairs: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    for i in items.iter().filter(|i| i.status == Status::Ok && !i.is_dir) {
        let (Some(folder), Some(dest)) = (&i.folder, &i.dest) else {
            continue;
        };
        let Some(root) = dest.ancestors().nth(folder.split('/').count()) else {
            continue;
        };
        let (Ok(old), Ok(new)) = (i.dir.strip_prefix(root), dest.strip_prefix(root)) else {
            continue;
        };
        let old: Vec<_> = old.components().collect();
        let new: Vec<_> = new.components().collect();
        // A folder that becomes several (`Title` → `Author/Title`) is not
        // one of them; a show's folder is its new one's, season apart.
        if old.len() < new.len() && !view.episodes.contains_key(&i.path) {
            continue;
        }
        // The first level where they part.
        let Some(k) = (0..old.len().min(new.len())).find(|&k| old[k] != new[k]) else {
            continue;
        };
        let fold = |levels: &[std::path::Component]| {
            levels.iter().fold(root.to_path_buf(), |p, c| p.join(c))
        };
        let (from, to) = (fold(&old[..=k]), fold(&new[..=k]));
        // Only the case differs: one folder where case is ignored.
        let lower =
            |c: &std::path::Component| text::nfc(&c.as_os_str().to_string_lossy().to_lowercase());
        let to = (lower(&old[k]) != lower(&new[k])).then_some(to);
        match pairs.iter_mut().find(|p| p.0 == from) {
            Some(p) if p.1 != to => p.1 = None,
            Some(_) => {}
            None => pairs.push((from, to)),
        }
    }
    let suffix = match view.leftover_suffix.trim() {
        "" => "extras",
        s => s,
    };
    let mut left = Vec::new();
    for (from, to) in pairs {
        let Some(name) = to.as_ref().and_then(|t| t.file_name()) else {
            continue;
        };
        // A folder renamed itself, or still where some files go.
        if protected(&from, view)
            || items.iter().any(|i| {
                i.path == from || (i.status == Status::Ok && i.target().starts_with(&from))
            })
        {
            continue;
        }
        let leaving: HashSet<&Path> = items
            .iter()
            .filter(|i| i.status == Status::Ok)
            .map(|i| i.path.as_path())
            .collect();
        let files = walkdir::WalkDir::new(&from)
            .min_depth(1)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir() && !leaving.contains(e.path()))
            .count();
        if files == 0 {
            continue;
        }
        let Some(up) = from.parent() else { continue };
        let to = up.join(format!("{} ({suffix})", name.to_string_lossy()));
        if std::fs::symlink_metadata(&to).is_err() {
            left.push(LeftFolder { from, to, files });
        }
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Rule, Target};
    use crate::tmdb::MovieMatch;
    use std::fs;

    fn entries(dir: &Path, names: &[&str]) -> Vec<FileEntry> {
        names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                fs::write(&p, n).unwrap();
                FileEntry::from_path(&p, &fs::metadata(&p).unwrap()).unwrap()
            })
            .collect()
    }

    fn replace(find: &str, with: &str) -> Vec<Step> {
        vec![Step {
            enabled: true,
            rule: Rule::Replace {
                find: find.into(),
                replace: with.into(),
                case_sensitive: true,
                first_only: false,
                target: Target::Name,
            },
        }]
    }

    #[test]
    fn loose_songs_stay_together() {
        use crate::tagwrite::{Field, write};
        let d = tempfile::tempdir().unwrap();
        let tag = |dir: &str, name: &str, album: &str| {
            let p = d.path().join(dir);
            fs::create_dir_all(&p).unwrap();
            let p = p.join(name);
            fs::copy("tests/fixtures/song.mp3", &p).unwrap();
            let f = vec![
                (Field::Title, vec![name.to_string()]),
                (Field::Artist, vec!["Band".to_string()]),
                (Field::Album, vec![album.to_string()]),
            ];
            write(&p, &f, None, false, &d.path().join("store")).unwrap();
            FileEntry::from_path(&p, &fs::metadata(&p).unwrap()).unwrap()
        };
        let files = vec![
            tag("Misc", "a.mp3", "One"),
            tag("Misc", "b.mp3", "Two"),
            tag("Misc", "c.mp3", "Three"),
            tag("Album", "x.mp3", "Real"),
            tag("Album", "y.mp3", "Real"),
            tag("Album", "z.mp3", "Real"),
        ];
        let steps: Vec<Step> = vec![
            serde_json::from_value(serde_json::json!({"rule": {"type": "music",
            "template": "{title}", "colon": " - ", "folders": true,
            "folderTemplate": "{album artist}/{album}"}}))
            .unwrap(),
        ];
        let p = preview(&files, &steps, &HashMap::new(), &View::default());
        let folder = |n: &str| {
            p.items
                .iter()
                .find(|i| i.original == n)
                .unwrap()
                .folder
                .clone()
        };
        // The loose songs stay in their folder; the album's get one.
        assert_eq!(folder("a.mp3"), None);
        assert_eq!(folder("x.mp3").as_deref(), Some("Band/Real"));
    }

    #[test]
    fn names_windows_refuses() {
        for bad in [
            "CON",
            "con.txt",
            "Nul.tar.gz",
            "COM1.mkv",
            "lpt9",
            "a:b",
            "x.",
            "x ",
        ] {
            assert!(windows_bad(bad), "{bad}");
        }
        for ok in ["CONTO.txt", "COM0", "COM10.txt", "console.log", "a.b"] {
            assert!(!windows_bad(ok), "{ok}");
        }
    }

    #[test]
    fn lookups_read_what_the_rules_above_make() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["SrNm #01.cbz", "SrNm #02.cbz"]);
        let mut steps = replace("SrNm", "Series Name");
        steps.push(
            serde_json::from_value(serde_json::json!({"rule": {"type": "comic",
            "template": "{series} #{number}", "colon": " - "}}))
            .unwrap(),
        );
        let p = preview(&files, &steps, &HashMap::new(), &View::default());
        assert_eq!(p.items[0].new_name, "Series Name #001.cbz");
        assert_eq!(
            p.items[0].lookup_name.as_deref(),
            Some("Series Name #01.cbz")
        );
        // Without a lookup rule, nothing to read.
        let p = preview(
            &files,
            &replace("SrNm", "X"),
            &HashMap::new(),
            &View::default(),
        );
        assert_eq!(p.items[0].lookup_name, None);
    }

    #[test]
    fn duplicates_conflict() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["a1.txt", "a2.txt", "b.txt"]);
        let p = preview(&files, &replace_digits(), &HashMap::new(), &View::default());
        assert_eq!(p.items[0].status, Status::Conflict);
        assert_eq!(p.items[1].status, Status::Conflict);
        assert_eq!(p.items[2].status, Status::Unchanged);
    }

    fn replace_digits() -> Vec<Step> {
        vec![Step {
            enabled: true,
            rule: Rule::Regex {
                pattern: r"\d".into(),
                replace: "".into(),
                case_sensitive: true,
                first_only: false,
                target: Target::Name,
            },
        }]
    }

    #[test]
    fn existing_file_conflict_unless_moving() {
        let d = tempfile::tempdir().unwrap();
        // "x" -> "y" blocked: y.txt exists and is not in the batch.
        let files = entries(d.path(), &["x.txt"]);
        fs::write(d.path().join("y.txt"), "").unwrap();
        let p = preview(
            &files,
            &replace("x", "y"),
            &HashMap::new(),
            &View::default(),
        );
        assert_eq!(p.items[0].status, Status::Conflict);

        // Swapping a <-> b within the batch is allowed.
        let files = entries(d.path(), &["a.md", "b.md"]);
        let mut ov = HashMap::new();
        ov.insert(files[0].path.clone(), "b.md".to_string());
        ov.insert(files[1].path.clone(), "a.md".to_string());
        let p = preview(&files, &[], &ov, &View::default());
        assert!(
            p.items.iter().all(|i| i.status == Status::Ok),
            "{:?}",
            p.items
        );
    }

    #[test]
    fn unchanged_file_is_not_the_conflict() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["a.txt", "b.txt"]);
        let mut ov = HashMap::new();
        ov.insert(files[0].path.clone(), "b.txt".to_string());
        let p = preview(&files, &[], &ov, &View::default());
        assert_eq!(p.items[0].problem, Some(Problem::Exists));
        assert_eq!(p.items[1].status, Status::Unchanged);
        assert_eq!(p.conflicts, 1);
    }

    #[test]
    fn identical_copies_let_one_through() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["a.txt", "b.txt"]);
        let mut ov = HashMap::new();
        for f in &files {
            ov.insert(f.path.clone(), "same.txt".to_string());
        }
        // Not known to be identical: both blocked.
        let mut view = View::default();
        let p = preview(&files, &[], &ov, &view);
        assert!(
            p.items
                .iter()
                .all(|i| i.problem == Some(Problem::Duplicate))
        );
        // Identical copies: the first goes ahead.
        for f in &files {
            view.identical.insert(f.path.clone(), 0);
        }
        let p = preview(&files, &[], &ov, &view);
        assert_eq!(p.items[0].status, Status::Ok);
        assert_eq!(p.items[1].problem, Some(Problem::Copy));
    }

    #[test]
    fn invalid_names() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["a.txt"]);
        let p = preview(
            &files,
            &replace("a", "x/y"),
            &HashMap::new(),
            &View::default(),
        );
        assert_eq!(p.items[0].status, Status::Conflict);
        let p = preview(
            &files,
            &replace("a", "a:b"),
            &HashMap::new(),
            &View::default(),
        );
        assert_eq!(p.items[0].status, Status::Ok);
        assert!(p.items[0].windows_unsafe);
    }

    #[test]
    fn emptied_name_is_a_conflict() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["abc.txt"]);
        let p = preview(
            &files,
            &replace("abc", ""),
            &HashMap::new(),
            &View::default(),
        );
        assert_eq!(p.items[0].new_name, ".txt");
        assert_eq!(p.items[0].problem, Some(Problem::Empty));
    }

    #[test]
    fn filter_limits_batch_and_numbering() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["a.jpg", "b.txt", "c.jpg"]);
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Number {
                template: "{n}".into(),
                start: 1,
                step: 1,
                digits: 1,
                per_folder: false,
            },
        }];
        let view = View {
            filter: "*.jpg".into(),
            ..Default::default()
        };
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert_eq!(p.total, 3);
        let names: Vec<_> = p.items.iter().map(|i| i.new_name.as_str()).collect();
        assert_eq!(names, ["1.jpg", "2.jpg"]);
        assert_eq!(p.operations().len(), 2);
    }

    #[test]
    fn numbering_per_folder() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("x")).unwrap();
        fs::create_dir(d.path().join("y")).unwrap();
        let mut files = entries(&d.path().join("x"), &["a", "b"]);
        files.extend(entries(&d.path().join("y"), &["c"]));
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Number {
                template: "{folder}-{n}".into(),
                start: 1,
                step: 1,
                digits: 1,
                per_folder: true,
            },
        }];
        let p = preview(&files, &steps, &HashMap::new(), &View::default());
        let names: Vec<_> = p.items.iter().map(|i| i.new_name.as_str()).collect();
        assert_eq!(names, ["x-1", "x-2", "y-1"]);
    }

    #[test]
    fn movie_into_folder() {
        let d = tempfile::tempdir().unwrap();
        let m = MovieMatch {
            id: 1,
            title: "Filme".into(),
            original_title: String::new(),
            year: Some(2001),
        };
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Movie {
                template: "{title} ({year})".into(),
                colon: " - ".into(),
                sidecar: false,
                fanart: false,
                folder: true,
            },
        }];
        // With something else in the folder: not the movie's own.
        fs::write(d.path().join("notes.ods"), "n").unwrap();
        let files = entries(d.path(), &["f.2001.mkv"]);
        let mut view = View::default();
        view.movies.insert(files[0].path.clone(), m.clone());
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert_eq!(p.items[0].folder.as_deref(), Some("Filme (2001)"));
        assert_eq!(
            p.operations()[0].to,
            d.path().join("Filme (2001)/Filme (2001).mkv")
        );

        // Already organized: no folder inside the folder, nothing to do.
        let inner = d.path().join("Filme (2001)");
        fs::create_dir(&inner).unwrap();
        let files = entries(&inner, &["Filme (2001).mkv"]);
        let mut view = View::default();
        view.movies.insert(files[0].path.clone(), m);
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert_eq!(p.items[0].folder, None);
        assert_eq!(p.items[0].status, Status::Unchanged);
    }

    #[test]
    fn book_into_author_folder() {
        let d = tempfile::tempdir().unwrap();
        let m = crate::books::BookMatch {
            title: "Dom Casmurro".into(),
            authors: vec!["Machado de Assis".into()],
            year: Some(1899),
            ..Default::default()
        };
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Book {
                template: "{author} - {title} ({year})".into(),
                colon: " - ".into(),
                folders: true,
                folder_template: "{author}/{title}".into(),
                cover: false,
                opf: false,
            },
        }];
        // The same book in two formats, and a file the rule knows nothing of.
        let files = entries(
            d.path(),
            &["dom_casmurro.epub", "dom_casmurro.pdf", "outro.pdf"],
        );
        let mut view = View::default();
        view.books.insert(files[0].path.clone(), m.clone());
        view.books.insert(files[1].path.clone(), m.clone());
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let ops = p.operations();
        assert_eq!(ops.len(), 2);
        let to = d.path().join("Machado de Assis/Dom Casmurro");
        assert_eq!(
            ops[0].to,
            to.join("Machado de Assis - Dom Casmurro (1899).epub")
        );
        assert_eq!(
            ops[1].to,
            to.join("Machado de Assis - Dom Casmurro (1899).pdf")
        );

        // Already in the author's folder: it is reused.
        let inner = d.path().join("Machado de Assis");
        fs::create_dir(&inner).unwrap();
        let files = entries(&inner, &["dc.epub"]);
        let mut view = View::default();
        view.books.insert(files[0].path.clone(), m);
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert_eq!(
            p.operations()[0].to,
            to.join("Machado de Assis - Dom Casmurro (1899).epub")
        );
    }

    #[test]
    fn episode_keeps_subtitle_language() {
        let d = tempfile::tempdir().unwrap();
        let m = EpisodeMatch {
            id: 1,
            show: "Show".into(),
            original_show: String::new(),
            year: None,
            season: 1,
            episodes: vec![2],
            titles: vec!["Title".into()],
        };
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Series {
                template: "{show} - S{season}E{episode} - {title}".into(),
                colon: " - ".into(),
                show: String::new(),
                folders: false,
                folder_template: String::new(),
                sidecar: false,
                fanart: false,
                missing: true,
            },
        }];
        let files = entries(
            d.path(),
            &["show.s01e02.mkv", "show.s01e02.pt-BR.srt", "poster.jpg"],
        );
        let mut view = View::default();
        for f in &files {
            view.episodes.insert(f.path.clone(), m.clone());
        }
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let names: Vec<_> = p.items.iter().map(|i| i.new_name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Show - S01E02 - Title.mkv",
                "Show - S01E02 - Title.pt-BR.srt",
                // Not a video or subtitle: left alone even with a match.
                "poster.jpg"
            ]
        );
    }

    #[test]
    fn episodes_into_show_folders() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join("Show/Season 1");
        fs::create_dir_all(&old).unwrap();
        let m = EpisodeMatch {
            id: 7,
            show: "Show".into(),
            original_show: String::new(),
            year: Some(2010),
            season: 1,
            episodes: vec![2],
            titles: vec!["Title".into()],
        };
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Series {
                template: "{show} - S{season}E{episode}".into(),
                colon: " - ".into(),
                show: String::new(),
                folders: true,
                folder_template: "{show} ({year}) [tmdbid-{id}]".into(),
                sidecar: false,
                fanart: false,
                missing: true,
            },
        }];
        let files = entries(&old, &["show.s01e02.mkv"]);
        let mut view = View::default();
        view.episodes.insert(files[0].path.clone(), m);
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert_eq!(
            p.items[0].folder.as_deref(),
            Some("Show (2010) [tmdbid-7]/Season 01")
        );
        assert_eq!(
            p.operations()[0].to,
            d.path()
                .join("Show (2010) [tmdbid-7]/Season 01/Show - S01E02.mkv")
        );
    }

    fn series_steps() -> Vec<Step> {
        vec![Step {
            enabled: true,
            rule: Rule::Series {
                template: "{show} - S{season}E{episode}".into(),
                colon: " - ".into(),
                show: String::new(),
                folders: true,
                folder_template: "{show} ({year}) [tmdbid-{id}]".into(),
                sidecar: false,
                fanart: false,
                missing: true,
            },
        }]
    }

    fn episode(n: u16) -> EpisodeMatch {
        EpisodeMatch {
            id: 7,
            show: "Show".into(),
            original_show: String::new(),
            year: Some(2010),
            season: 1,
            episodes: vec![n],
            titles: Vec::new(),
        }
    }

    #[test]
    fn hand_named_episode_keeps_its_folder() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join("Show/Season 1");
        fs::create_dir_all(&old).unwrap();
        let files = entries(&old, &["show.s01e02.mkv"]);
        let mut view = View::default();
        view.episodes.insert(files[0].path.clone(), episode(2));
        let overrides = HashMap::from([(files[0].path.clone(), "Mine.mkv".to_string())]);
        let p = preview(&files, &series_steps(), &overrides, &view);
        assert_eq!(
            p.operations()[0].to,
            d.path().join("Show (2010) [tmdbid-7]/Season 01/Mine.mkv")
        );
    }

    #[test]
    fn show_folder_known_by_what_it_holds() {
        // A folder named nothing like the show, holding only its episodes:
        // the new folder goes next to it, not inside.
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join("Complete Box 1080p");
        fs::create_dir_all(&old).unwrap();
        let files = entries(&old, &["e01.mkv", "e02.mkv"]);
        let mut view = View::default();
        for (n, f) in files.iter().enumerate() {
            view.episodes.insert(f.path.clone(), episode(n as u16 + 1));
        }
        let p = preview(&files, &series_steps(), &HashMap::new(), &view);
        assert_eq!(
            p.operations()[0].to,
            d.path()
                .join("Show (2010) [tmdbid-7]/Season 01/Show - S01E01.mkv")
        );
        assert!(p.left_folders.is_empty());

        // Another video in it (not this show's): a folder like any other.
        fs::write(old.join("other.mkv"), "x").unwrap();
        let p = preview(&files, &series_steps(), &HashMap::new(), &view);
        assert_eq!(
            p.operations()[0].to,
            old.join("Show (2010) [tmdbid-7]/Season 01/Show - S01E01.mkv")
        );
        assert!(p.left_folders.is_empty());
    }

    #[test]
    fn show_folder_with_leftovers_is_renamed_beside() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join("Show/Season 1");
        fs::create_dir_all(&old).unwrap();
        fs::write(d.path().join("Show/notes.txt"), "x").unwrap();
        let files = entries(&old, &["show.s01e01.mkv", "show.s01e02.mkv"]);
        let mut view = View {
            leftover_suffix: "additional files".into(),
            ..View::default()
        };
        for (n, f) in files.iter().enumerate() {
            view.episodes.insert(f.path.clone(), episode(n as u16 + 1));
        }
        let p = preview(&files, &series_steps(), &HashMap::new(), &view);
        assert_eq!(p.left_folders.len(), 1);
        assert_eq!(p.left_folders[0].files, 1);
        let left = d.path().join("Show (2010) [tmdbid-7] (additional files)");
        assert_eq!(p.left_folders[0].to, left);

        let h = crate::apply::History::new(d.path().join("hist"));
        let r = h.apply(&p.operations());
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let new = d.path().join("Show (2010) [tmdbid-7]/Season 01");
        assert!(new.join("Show - S01E01.mkv").exists());
        assert!(new.join("Show - S01E02.mkv").exists());
        assert!(left.join("notes.txt").exists());
        // The emptied season folder goes, under the folder's new name too.
        assert!(!left.join("Season 1").exists());
        assert!(!d.path().join("Show").exists());

        h.undo(r.history_id.as_deref().unwrap()).unwrap();
        assert!(old.join("show.s01e01.mkv").exists());
        assert!(d.path().join("Show/notes.txt").exists());
        assert!(!left.exists());
    }

    use crate::music::flac;

    #[test]
    fn songs_by_tags_with_their_companions() {
        let d = tempfile::tempdir().unwrap();
        let album = d.path().join("Pink Floyd/WYWH");
        fs::create_dir_all(&album).unwrap();
        flac(
            &album.join("01.flac"),
            &[
                ("TITLE", "Shine On"),
                ("ARTIST", "Pink Floyd"),
                ("ALBUM", "Wish You Were Here"),
                ("DATE", "1975"),
                ("TRACKNUMBER", "1"),
            ],
        );
        flac(
            &album.join("02.flac"),
            &[
                ("TITLE", "Welcome"),
                ("ARTIST", "Pink Floyd"),
                ("ALBUM", "Wish You Were Here"),
                ("DATE", "1975"),
                ("TRACKNUMBER", "2"),
            ],
        );
        flac(&album.join("x.flac"), &[]);
        fs::write(album.join("01.lrc"), "[00:00]").unwrap();
        fs::write(album.join("cover.jpg"), "img").unwrap();
        let files = crate::scan(
            std::slice::from_ref(&album),
            &Default::default(),
            &Default::default(),
        )
        .files;
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Music {
                template: "{track} - {title}".into(),
                colon: " - ".into(),
                disc_prefix: true,
                from_name: false,
                folders: true,
                folder_template: "{album artist}/{album} ({year})".into(),
                cover: false,
                playlist: false,
                listen: false,
                write_tags: false,
                embed_cover: false,
                clean_tags: false,
            },
        }];
        let p = preview(&files, &steps, &HashMap::new(), &View::default());
        let get = |n: &str| p.items.iter().find(|i| i.original == n).unwrap();
        let wywh = d.path().join("Pink Floyd/Wish You Were Here (1975)");
        // Next to the old folders, not inside them.
        assert_eq!(get("01.flac").target(), wywh.join("01 - Shine On.flac"));
        assert_eq!(get("01.lrc").target(), wywh.join("01 - Shine On.lrc"));
        // The cover goes with most of the folder's songs; the untagged one
        // stays.
        assert_eq!(get("cover.jpg").target(), wywh.join("cover.jpg"));
        assert!(get("x.flac").untagged);
        assert_eq!(get("x.flac").status, Status::Unchanged);

        // Two songs staying, one leaving: the cover stays.
        flac(&album.join("y.flac"), &[]);
        let files = crate::scan(
            std::slice::from_ref(&album),
            &Default::default(),
            &Default::default(),
        )
        .files;
        let p = preview(&files, &steps, &HashMap::new(), &View::default());
        let cover = p.items.iter().find(|i| i.original == "cover.jpg").unwrap();
        assert_eq!(cover.status, Status::Unchanged);
        fs::remove_file(album.join("y.flac")).unwrap();

        // Without the untagged song, the cover goes with the album.
        fs::remove_file(album.join("x.flac")).unwrap();
        let files = crate::scan(
            std::slice::from_ref(&album),
            &Default::default(),
            &Default::default(),
        )
        .files;
        let p = preview(&files, &steps, &HashMap::new(), &View::default());
        let cover = p.items.iter().find(|i| i.original == "cover.jpg").unwrap();
        assert_eq!(cover.target(), wywh.join("cover.jpg"));
    }

    #[test]
    fn albums_with_discs_artwork_and_spellings() {
        let d = tempfile::tempdir().unwrap();
        let song = |dir: &Path, file: &str, artist: &str, album: &str, track: &str, disc: &str| {
            fs::create_dir_all(dir).unwrap();
            flac(
                &dir.join(format!("{file}.flac")),
                &[
                    ("TITLE", file),
                    ("ARTIST", artist),
                    ("ALBUM", album),
                    ("DATE", "2000"),
                    ("TRACKNUMBER", track),
                    ("DISCNUMBER", disc),
                    ("DISCTOTAL", "2"),
                ],
            );
        };
        let deluxe = d.path().join("A-Ha - Analogue (Deluxe) (2000) [FLAC]");
        song(
            &deluxe.join("CD 01"),
            "a",
            "A-Ha",
            "Analogue (Deluxe)",
            "1",
            "1",
        );
        song(
            &deluxe.join("CD 02"),
            "b",
            "A-Ha",
            "Analogue (Deluxe)",
            "1",
            "2",
        );
        fs::write(deluxe.join("Cover.jpg"), "c").unwrap();
        fs::write(deluxe.join("CD 01/log.txt"), "1").unwrap();
        fs::write(deluxe.join("CD 02/log.txt"), "2").unwrap();
        let other = d.path().join("a-ha - Other [FLAC]");
        song(&other, "c", "a-ha", "Other", "1", "1");
        fs::create_dir_all(other.join("Artwork")).unwrap();
        fs::write(other.join("Artwork/img.png"), "i").unwrap();
        let opts = crate::ScanOptions {
            recursive: true,
            ..Default::default()
        };
        let files = crate::scan(&[d.path().to_path_buf()], &opts, &Default::default()).files;
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Music {
                template: "{track} - {title}".into(),
                colon: " - ".into(),
                disc_prefix: true,
                from_name: false,
                folders: true,
                folder_template: "{album artist}/{album}".into(),
                cover: false,
                playlist: false,
                listen: false,
                write_tags: false,
                embed_cover: false,
                clean_tags: false,
            },
        }];
        let p = preview(&files, &steps, &HashMap::new(), &View::default());
        let targets: HashSet<PathBuf> = p.items.iter().map(|i| i.target()).collect();
        let base = d.path().join("A-Ha");
        for want in [
            "Analogue (Deluxe)/1-01 - a.flac",
            "Analogue (Deluxe)/2-01 - b.flac",
            "Analogue (Deluxe)/Cover.jpg",
            "Analogue (Deluxe)/log (CD 01).txt",
            "Analogue (Deluxe)/log (CD 02).txt",
            "Other/1-01 - c.flac",
            "Other/Artwork/img.png",
        ] {
            assert!(
                targets.contains(&base.join(want)),
                "missing {want} in {targets:?}"
            );
        }
    }

    #[test]
    fn movie_release_folder_is_renamed_beside() {
        let d = tempfile::tempdir().unwrap();
        let m = MovieMatch {
            id: 1,
            title: "Filme".into(),
            original_title: String::new(),
            year: Some(2001),
        };
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Movie {
                template: "{title} ({year})".into(),
                colon: " - ".into(),
                sidecar: false,
                fanart: false,
                folder: true,
            },
        }];
        let old = d.path().join("Filme.2001.1080p.BluRay");
        fs::create_dir(&old).unwrap();
        fs::write(old.join("info.txt"), "x").unwrap();
        let files = entries(&old, &["f.2001.mkv"]);
        let mut view = View {
            leftover_suffix: "additional files".into(),
            ..View::default()
        };
        view.movies.insert(files[0].path.clone(), m.clone());
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let ops = p.operations();
        assert_eq!(ops[0].to, d.path().join("Filme (2001)/Filme (2001).mkv"));
        assert_eq!(p.left_folders.len(), 1);
        assert_eq!(
            p.left_folders[0].to,
            d.path().join("Filme (2001) (additional files)")
        );

        // Nothing else in it: the folder goes after the move.
        fs::remove_file(old.join("info.txt")).unwrap();
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert!(p.left_folders.is_empty());
        let h = crate::apply::History::new(d.path().join("hist"));
        let r = h.apply(&p.operations());
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(d.path().join("Filme (2001)/Filme (2001).mkv").exists());
        assert!(!old.exists());

        // Another name, holding only the movie (and a subtitle): its own.
        fs::remove_dir_all(d.path().join("Filme (2001)")).unwrap();
        let other = d.path().join("Videos");
        fs::create_dir(&other).unwrap();
        let files = entries(&other, &["f.2001.mkv", "f.2001.srt"]);
        let mut view = View::default();
        for f in &files {
            view.movies.insert(f.path.clone(), m.clone());
        }
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let ops = p.operations();
        let video = ops.iter().find(|o| o.from == files[0].path).unwrap();
        assert_eq!(video.to, d.path().join("Filme (2001)/Filme (2001).mkv"));

        // Something else in it: the movie's folder goes inside.
        fs::write(other.join("budget.ods"), "x").unwrap();
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let ops = p.operations();
        let video = ops.iter().find(|o| o.from == files[0].path).unwrap();
        assert_eq!(video.to, other.join("Filme (2001)/Filme (2001).mkv"));
        assert!(p.left_folders.is_empty());
    }

    #[test]
    fn album_folder_with_leftovers_is_renamed_beside() {
        let d = tempfile::tempdir().unwrap();
        let album = d.path().join("A-Ha/Analogue");
        fs::create_dir_all(&album).unwrap();
        for (file, track) in [("a", "1"), ("b", "2")] {
            flac(
                &album.join(format!("{file}.flac")),
                &[
                    ("TITLE", file),
                    ("ARTIST", "A-Ha"),
                    ("ALBUM", "Analogue"),
                    ("DATE", "2005"),
                    ("TRACKNUMBER", track),
                ],
            );
        }
        // Left out of the list: it stays.
        fs::write(album.join("notes.txt"), "n").unwrap();
        let opts = crate::ScanOptions {
            recursive: true,
            ..Default::default()
        };
        let files: Vec<FileEntry> =
            crate::scan(&[d.path().to_path_buf()], &opts, &Default::default())
                .files
                .into_iter()
                .filter(|f| f.ext == "flac")
                .collect();
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Music {
                template: "{track} - {title}".into(),
                colon: " - ".into(),
                disc_prefix: true,
                from_name: false,
                folders: true,
                folder_template: "{album artist}/{album} ({year})".into(),
                cover: false,
                playlist: false,
                listen: false,
                write_tags: false,
                embed_cover: false,
                clean_tags: false,
            },
        }];
        let view = View {
            leftover_suffix: "additional files".into(),
            ..View::default()
        };
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let new = d.path().join("A-Ha/Analogue (2005)");
        assert!(p.items.iter().all(|i| i.target().starts_with(&new)));
        assert_eq!(p.left_folders.len(), 1);
        assert_eq!(p.left_folders[0].from, album);
        assert_eq!(
            p.left_folders[0].to,
            d.path().join("A-Ha/Analogue (2005) (additional files)")
        );
        assert_eq!(p.left_folders[0].files, 1);
    }

    #[test]
    fn book_folder_that_becomes_two_keeps_its_name() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join("Dom Casmurro");
        fs::create_dir(&old).unwrap();
        fs::write(old.join("notas.txt"), "n").unwrap();
        let files = entries(&old, &["dc.epub"]);
        let mut view = View::default();
        view.books.insert(
            files[0].path.clone(),
            crate::books::BookMatch {
                title: "Dom Casmurro".into(),
                authors: vec!["Machado de Assis".into()],
                year: Some(1899),
                ..Default::default()
            },
        );
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Book {
                template: "{title}".into(),
                colon: " - ".into(),
                folders: true,
                folder_template: "{author}/{title}".into(),
                cover: false,
                opf: false,
            },
        }];
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert_eq!(
            p.operations()[0].to,
            d.path()
                .join("Machado de Assis/Dom Casmurro/Dom Casmurro.epub")
        );
        assert!(p.left_folders.is_empty());

        // A folder with that book alone, whatever its name: its folders go
        // next to it.
        let other = d.path().join("stuff");
        fs::create_dir(&other).unwrap();
        let more = entries(&other, &["dc.epub", "dc.pdf"]);
        for f in &more {
            view.books
                .insert(f.path.clone(), view.books[&files[0].path].clone());
        }
        let p = preview(&more, &steps, &HashMap::new(), &view);
        let to = d.path().join("Machado de Assis/Dom Casmurro");
        assert_eq!(p.operations()[0].to, to.join("Dom Casmurro.epub"));
        assert_eq!(p.operations()[1].to, to.join("Dom Casmurro.pdf"));
    }

    #[test]
    fn album_folder_known_by_what_it_holds() {
        let d = tempfile::tempdir().unwrap();
        let song = |dir: &Path, file: &str, album: &str| {
            fs::create_dir_all(dir).unwrap();
            flac(
                &dir.join(format!("{file}.flac")),
                &[
                    ("TITLE", file),
                    ("ARTIST", "A-Ha"),
                    ("ALBUM", album),
                    ("DATE", "2005"),
                    ("TRACKNUMBER", "1"),
                ],
            );
        };
        let one = d.path().join("new folder");
        song(&one.join("CD 1"), "a", "Analogue");
        song(&one, "b", "Analogue");
        fs::write(one.join("cover.jpg"), "c").unwrap();
        // Two albums: a folder like any other.
        let mixed = d.path().join("mixed");
        song(&mixed, "c", "Analogue");
        song(&mixed, "e", "Other");
        let opts = crate::ScanOptions {
            recursive: true,
            ..Default::default()
        };
        let files = crate::scan(&[d.path().to_path_buf()], &opts, &Default::default()).files;
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Music {
                template: "{title}".into(),
                colon: " - ".into(),
                disc_prefix: false,
                from_name: false,
                folders: true,
                folder_template: "{album artist}/{album} ({year})".into(),
                cover: false,
                playlist: false,
                listen: false,
                write_tags: false,
                embed_cover: false,
                clean_tags: false,
            },
        }];
        let p = preview(&files, &steps, &HashMap::new(), &View::default());
        let to = |name: &str| {
            p.items
                .iter()
                .find(|i| i.original == name)
                .map(|i| i.target())
                .unwrap()
        };
        let beside = d.path().join("A-Ha/Analogue (2005)");
        assert_eq!(to("a.flac"), beside.join("a.flac"));
        assert_eq!(to("b.flac"), beside.join("b.flac"));
        assert_eq!(to("cover.jpg"), beside.join("cover.jpg"));
        assert_eq!(to("c.flac"), mixed.join("A-Ha/Analogue (2005)/c.flac"));
        assert_eq!(to("e.flac"), mixed.join("A-Ha/Other (2005)/e.flac"));
    }

    #[test]
    fn series_folder_known_by_what_it_holds() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join("my comics");
        fs::create_dir(&old).unwrap();
        // Not a comic: it stays, in the old folder renamed beside the new.
        fs::write(old.join("drawing.xcf"), "x").unwrap();
        let files = entries(&old, &["SERIES v01.cbz", "SERIES VOL.02.pdf"]);
        let mut view = View {
            leftover_suffix: "additional files".into(),
            ..View::default()
        };
        for (n, f) in files.iter().enumerate() {
            view.comics.insert(
                f.path.clone(),
                crate::comics::ComicMatch {
                    series: "Series".into(),
                    series_year: Some(2020),
                    number: (n + 1).to_string(),
                    ..Default::default()
                },
            );
        }
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Comic {
                template: "{series} {number}".into(),
                colon: " - ".into(),
                series: String::new(),
                source: "gcd".into(),
                folders: true,
                folder_template: "{series} ({series year})".into(),
                by_folder: false,
                missing: false,
            },
        }];
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let new = d.path().join("Series (2020)");
        assert!(
            p.items.iter().all(|i| i.target().parent() == Some(&new)),
            "{:?}",
            p.items.iter().map(|i| i.target()).collect::<Vec<_>>()
        );
        assert_eq!(p.left_folders.len(), 1);
        assert_eq!(
            p.left_folders[0].to,
            d.path().join("Series (2020) (additional files)")
        );

        // Another series' comic in it: a folder like any other.
        fs::write(old.join("Other 01.cbz"), "o").unwrap();
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert!(
            p.items
                .iter()
                .all(|i| i.target().starts_with(old.join("Series (2020)")))
        );
        assert!(p.left_folders.is_empty());
    }

    #[test]
    fn sort_by_new_name() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["b.txt", "a.txt"]);
        let mut ov = HashMap::new();
        ov.insert(files[1].path.clone(), "z.txt".to_string());
        let view = View {
            sort_by_new_name: true,
            ..Default::default()
        };
        let p = preview(&files, &[], &ov, &view);
        assert_eq!(p.items[0].new_name, "b.txt");
        assert_eq!(p.items[1].new_name, "z.txt");
    }
}
