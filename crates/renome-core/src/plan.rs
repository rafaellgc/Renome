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
    /// The same, and one of those others is an identical copy of it.
    DuplicateIdentical,
    /// An identical copy of a file that gets that name: it stays put.
    Copy,
    /// A file with that name already exists outside the batch.
    Exists,
    /// The same file is already where it goes, under that name: it stays
    /// where it is (nothing to move, copy or number).
    SameThere,
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
    /// Shows only songs that are the same music as another, grouped.
    pub alike_only: bool,
    /// Group of each such song (filled in by the app; see [`crate::alike`]).
    #[serde(skip)]
    pub alike: HashMap<PathBuf, usize>,
    /// Shows only pictures that look like another, grouped.
    pub similar_only: bool,
    /// Group of each such picture (filled in by the app; see
    /// [`crate::similar`]).
    #[serde(skip)]
    pub similar: HashMap<PathBuf, usize>,
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
    /// Dates typed in for photos and videos (`2018-03-27`, with the time
    /// after a space when typed), by path, over what the photo rule reads.
    #[serde(default)]
    pub dates: HashMap<PathBuf, String>,
    /// What an old folder is called after when files stay in it
    /// (`Show (2010) [tmdbid-7] (additional files)`), in the UI's language.
    #[serde(default)]
    pub leftover_suffix: String,
    /// The UI's language (`pt`, `en`, `es`): countries' names.
    #[serde(default)]
    pub lang: String,
    /// Two files given the same name: the later ones get ` (2)`, ` (3)`…
    /// instead of being left as they are.
    #[serde(default)]
    pub number_clashes: bool,
    /// Folders never taken for a show's or movie's own, nor renamed for
    /// leftovers (home, Downloads…; filled in by the app).
    #[serde(skip)]
    pub protected: Vec<PathBuf>,
    /// Another folder the files go to, moved or copied.
    #[serde(default)]
    pub send_to: Option<SendTo>,
    /// The release's advertisements go to the system's trash once the
    /// files are moved, with their old folder when nothing else is left
    /// in it (not when copying).
    #[serde(default)]
    pub trash_ads: bool,
}

/// Two files given the same name, or a name another file has and keeps:
/// the later ones get ` (2)`, ` (3)`… after the name, the first free one.
/// Identical copies (when looked for) are left to be flagged as such.
fn number_clashes(items: &mut [PreviewItem], keys: &mut PathKeys, view: &View) {
    let copying = view.send_to.as_ref().is_some_and(|s| s.copy);
    let moving: HashSet<PathBuf> = items
        .iter()
        .filter(|i| i.status == Status::Ok && !copying)
        .map(|i| keys.key(&i.path))
        .collect();
    let mut used: HashSet<PathBuf> = HashSet::new();
    for item in items.iter_mut() {
        if item.status != Status::Ok || view.identical.contains_key(&item.path) || item.companion {
            continue;
        }
        let taken = |keys: &mut PathKeys, used: &HashSet<PathBuf>, target: &Path| {
            let key = keys.key(target);
            used.contains(&key)
                || (std::fs::symlink_metadata(target).is_ok()
                    && !moving.contains(&key)
                    && !same_file(target, &item.path))
        };
        let first = item.target();
        // The same file already there: not numbered (it stays, below).
        if !used.contains(&keys.key(&first)) && crate::identical::same_content(&item.path, &first) {
            used.insert(keys.key(&first));
            continue;
        }
        if taken(keys, &used, &first) {
            let name = item.new_name.clone();
            let (stem, ext) = if item.is_dir {
                (name.as_str(), "")
            } else {
                crate::files::split_name(&name)
            };
            for n in 2..10_000 {
                let new = crate::files::join_name(&format!("{stem} ({n})"), ext);
                let target = first.with_file_name(&new);
                if !taken(keys, &used, &target) {
                    item.new_name = new;
                    break;
                }
            }
        }
        let key = keys.key(&item.target());
        used.insert(key);
    }
}

/// Where the files go instead of staying where they are: the folders the
/// rules make go into `dir`, and other files keep the subfolders below
/// the folder they were added with.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTo {
    pub dir: PathBuf,
    /// Copies are made; the files stay as they are.
    #[serde(default)]
    pub copy: bool,
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
    /// The size as shown (`12 MB`: [`crate::text::size`]).
    pub size_text: String,
    pub modified: Option<i64>,
    pub is_dir: bool,
    pub original: String,
    pub new_name: String,
    /// Folder the file moves into (`Show (2008)/Season 01`), shown to the
    /// user; it is inside `dest`'s root, not always inside `dir`.
    pub folder: Option<String>,
    /// The folder the file ends up in, when it moves.
    pub dest: Option<PathBuf>,
    /// Its folder and `dest` as people read them (`~/…`:
    /// [`crate::text::home_short`]).
    pub dir_shown: String,
    pub dest_shown: Option<String>,
    pub status: Status,
    pub problem: Option<Problem>,
    /// Non-blocking warning: contains characters Windows does not accept.
    pub windows_unsafe: bool,
    pub manual: bool,
    /// A song the music rule cannot name (no title in its tags).
    pub untagged: bool,
    /// A song matched to a release whose tags say something else (the
    /// music rule can write them), or a CBZ matched to an issue its
    /// `ComicInfo.xml` does not tell (the comic rule can write it).
    pub retag: bool,
    /// A matched song with tags that cleaning would take away, or a CBZ
    /// with details the comic rule clears.
    pub junk: bool,
    /// A comic the comic rule converts to CBZ (a RAR, 7z or tar inside, or
    /// a PDF; see [`crate::convert`]), named `.cbz` here.
    #[serde(default)]
    pub convert: bool,
    /// A book the book rule converts, to this format (named with its
    /// extension here; see [`crate::bookconv`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_to: Option<crate::bookconv::Target>,
    /// A comic the comic rule makes an e-book, in this format (named with
    /// its extension here; see [`crate::comicbook`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comic_to: Option<crate::comicbook::Format>,
    /// A book locked by DRM: never converted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub drm: bool,
    /// A PDF converted: how many of its pages would be drawn (none:
    /// `None`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawn: Option<usize>,
    /// The name the lookups (movie, series, book, comic) read: what the
    /// rules above the first lookup rule make of it, when not the original.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lookup_name: Option<String>,
    /// Sent to another folder: the folder it was added with (see
    /// [`Operation::within`]).
    #[serde(skip)]
    pub within: Option<PathBuf>,
    /// With a photo rule, a photo's or video's date came from (`none`:
    /// it has none and stays as it is).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo_date: Option<crate::photos::DateFrom>,
    /// The change date (Unix seconds) the file gets once renamed: when the
    /// photo was taken, with the photo rule's "set the file date".
    #[serde(skip)]
    pub touch: Option<i64>,
    /// What the subtitle rule does to this subtitle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_fix: Option<crate::subtext::Fix>,
    /// Text subtitle tracks the subtitle rule takes out of this video.
    #[serde(skip_serializing_if = "is_zero")]
    pub sub_extract: usize,
    /// Its picture subtitle tracks, which are not taken out.
    #[serde(skip_serializing_if = "is_zero")]
    pub sub_pictures: usize,
    /// With the movie rule, a movie's extra (a making-of, a trailer): the
    /// extras folder it goes to next to the movie (see [`crate::extras`]).
    /// It is not looked up as a movie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
    /// With the movie rule, what a release's makers added to advertise
    /// themselves (see [`crate::extras::release_junk`]): it stays in the
    /// old folder, with what else does not go with the movie.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub release_junk: bool,
    /// A release's advertisement that goes to the trash (see
    /// [`View::trash_ads`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub to_trash: bool,
    /// The work (movie, episode, book, comic) this file is named after and
    /// goes with (see [`follow_works`]): when the work stays, so does it.
    #[serde(skip)]
    pub follows: Option<PathBuf>,
    /// Goes with a work (named after it, or from its own folder): never
    /// numbered when its place is taken (a `season (2).nfo` is no use).
    #[serde(skip)]
    pub companion: bool,
    /// Goes with the works of this folder (its own folder's other files):
    /// when none of them moves, it stays too.
    #[serde(skip)]
    pub with_folder: Option<PathBuf>,
    /// Left in a work's own folder that moves (a release's notes, a
    /// badge): it stays there, even when the rest is sent elsewhere.
    #[serde(skip)]
    pub stays_behind: bool,
    /// What the file is, by its extension, as the rules tell files apart
    /// (`video`, `subtitle`, `audio`, `book`, `comic`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<&'static str>,
    /// The details window of the rules on edits it (see [`editable`]).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub editable: bool,
    /// Its name for files counted as one (a video and its subtitles: see
    /// [`crate::rules::pair_split`]); a folder's whole name.
    pub pair: String,
    /// The lookup rule on leaves it alone (a `.zip` in a show's folder):
    /// that rule's type, for the list to tell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outside: Option<&'static str>,
}

/// What a file is, by its extension (see [`PreviewItem::kinds`]).
fn kinds_of(file: &FileEntry) -> Vec<&'static str> {
    if file.is_dir {
        return Vec::new();
    }
    let ext = file.ext.as_str();
    [
        ("video", crate::tmdb::is_video(ext)),
        ("subtitle", crate::tmdb::is_subtitle(ext)),
        ("audio", crate::music::is_audio(ext)),
        ("book", crate::ebook::is_book(ext)),
        ("comic", crate::comics::is_comic(ext)),
    ]
    .into_iter()
    .filter_map(|(k, yes)| yes.then_some(k))
    .collect()
}

/// Whether the lookup rule `kind` names this file (with what goes along:
/// a song's lyrics, cue sheet, playlist and cover).
fn rule_takes(kind: &str, audiobooks: bool, i: &PreviewItem) -> bool {
    let is = |k| i.kinds.contains(&k);
    let ext = Path::new(&i.original)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match kind {
        "movie" => i.is_dir || is("video") || is("subtitle"),
        "series" => !i.is_dir && (is("video") || is("subtitle")),
        "music" => {
            !i.is_dir
                && (is("audio")
                    || matches!(
                        ext.as_str(),
                        "lrc" | "cue" | "m3u" | "m3u8" | "pls" | "jpg" | "jpeg" | "png" | "webp"
                    ))
        }
        "book" => !i.is_dir && (is("book") || (audiobooks && is("audio")) || i.book_to.is_some()),
        "comic" => {
            if i.is_dir {
                i.convert
            } else {
                is("comic")
            }
        }
        _ => true,
    }
}

impl Preview {
    /// The conflicts, without the files already where they go (the same
    /// file is there): those stay, and nothing is wrong.
    pub fn real_conflicts(&self) -> usize {
        self.conflicts - self.same_there
    }
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub items: Vec<PreviewItem>,
    pub rule_errors: Vec<RuleError>,
    pub filter_error: Option<String>,
    /// Total files in the list, including those hidden by the filter.
    pub total: usize,
    pub changed: usize,
    pub conflicts: usize,
    /// Of the conflicts, the files that stay because the same file is
    /// already where they go (nothing is wrong with those).
    pub same_there: usize,
    /// Old folders renamed for what stays in them.
    pub left_folders: Vec<LeftFolder>,
    /// The release's advertisements to send to the trash after the moves,
    /// where they are then (see [`View::trash_ads`]), and old folders left
    /// with nothing else, sent whole (see [`trash_leftovers`]).
    pub trash: Vec<PathBuf>,
    pub trash_folders: Vec<PathBuf>,
    /// The files are copied, not moved (see [`SendTo`]).
    #[serde(skip)]
    pub copy: bool,
    /// What the rule cards show of the list.
    pub hints: Hints,
}

/// What the rule cards show of the list (the window's).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hints {
    /// A name to show examples with: the first in natural order, so it
    /// does not jump while the list re-sorts.
    pub sample: String,
    /// The files shown, and the same with files of one name and other
    /// extensions counted as one (as the list rule counts them).
    pub files: usize,
    pub pairs: usize,
    /// The names shown without extensions, in order (what the list rule's
    /// box can start from), when a list rule is there; the same with files
    /// of one name and other extensions once.
    pub names: Vec<String>,
    pub pair_names: Vec<String>,
    /// Per step: a list rule's names, a rearrange rule's parts of the
    /// sample.
    pub steps: Vec<StepHint>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepHint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub names: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<String>,
}

/// The details window the rules on offer (the lookup rule's or the photo
/// rule's), and whether a book rule takes audiobooks.
pub fn editor(steps: &[Step]) -> Option<(&'static str, bool)> {
    steps
        .iter()
        .filter(|s| s.enabled)
        .find_map(|s| match &s.rule {
            Rule::Music { .. } => Some(("music", false)),
            Rule::Book { audiobooks, .. } => Some(("book", *audiobooks)),
            Rule::Comic { .. } => Some(("comic", false)),
            Rule::Photo { .. } => Some(("photo", false)),
            _ => None,
        })
}

/// Whether a details window edits this row: a song for the music rule, a
/// book (or an audiobook's audio) for the book rule, a comic (or a folder
/// it converts) for the comic rule, a photo or video the photo rule reads.
pub fn editable(kind: &str, i: &PreviewItem, audiobooks: bool) -> bool {
    match kind {
        "music" => i.kinds.contains(&"audio"),
        "book" => {
            !i.is_dir && (i.kinds.contains(&"book") || (audiobooks && i.kinds.contains(&"audio")))
        }
        "comic" => {
            if i.is_dir {
                i.convert
            } else {
                i.kinds.contains(&"comic")
            }
        }
        "photo" => i.photo_date.is_some(),
        _ => false,
    }
}

/// The rule cards' [`Hints`] for these files.
fn hints(shown: &[&FileEntry], steps: &[Step]) -> Hints {
    let sample = shown
        .iter()
        .map(|f| f.file_name())
        .min_by(|a, b| crate::files::natural_cmp(a, b))
        .unwrap_or_default();
    let sample_file = shown.iter().find(|f| f.file_name() == sample);
    let pairs = {
        let mut seen = HashSet::new();
        for f in shown {
            seen.insert((
                f.dir.as_path(),
                crate::rules::pair_split(f).0.to_lowercase(),
            ));
        }
        seen.len()
    };
    let has_list = steps.iter().any(|s| matches!(s.rule, Rule::List { .. }));
    let names = if has_list {
        shown
            .iter()
            .map(|f| {
                if f.is_dir {
                    f.file_name()
                } else {
                    f.stem.clone()
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let pair_names = if has_list {
        let mut seen = HashSet::new();
        shown
            .iter()
            .filter_map(|f| {
                let base = crate::rules::pair_split(f).0;
                seen.insert((f.dir.as_path(), base.to_lowercase()))
                    .then(|| base.to_string())
            })
            .collect()
    } else {
        Vec::new()
    };
    let steps = steps
        .iter()
        .map(|s| match &s.rule {
            Rule::List { names, .. } => StepHint {
                names: Some(names.lines().filter(|l| !l.trim().is_empty()).count()),
                parts: Vec::new(),
            },
            Rule::Rearrange {
                separator, target, ..
            } if !separator.is_empty() => {
                let text = match (target, sample_file) {
                    (_, None) => String::new(),
                    (crate::rules::Target::Name, Some(f)) if !f.is_dir => f.stem.clone(),
                    (crate::rules::Target::Ext, Some(f)) => f.ext.clone(),
                    (_, Some(f)) => f.file_name(),
                };
                StepHint {
                    names: None,
                    parts: text
                        .split(separator.as_str())
                        .map(str::trim)
                        .filter(|p| !p.is_empty())
                        .map(String::from)
                        .collect(),
                }
            }
            _ => StepHint::default(),
        })
        .collect();
    Hints {
        sample,
        files: shown.len(),
        pairs,
        names,
        pair_names,
        steps,
    }
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
                copy: self.copy,
                within: i.within.clone(),
            })
            // Shallower than the files leaving them: renamed after.
            .chain(self.left_folders.iter().map(|l| Operation {
                from: l.from.clone(),
                to: l.to.clone(),
                copy: false,
                within: None,
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
        // ASCII letters only: every disk that ignores case ignores theirs,
        // but FAT mounted with `iocharset=ascii` tells `ç` from `Ç`.
        let swap = |s: &str| -> String {
            s.chars()
                .map(|c| {
                    if c.is_ascii_lowercase() {
                        c.to_ascii_uppercase()
                    } else {
                        c.to_ascii_lowercase()
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

/// Sent elsewhere: the files no rule (or song) gave a folder keep the
/// subfolders below the folder they were added with.
fn send_rest(items: &mut [PreviewItem], files: &[FileEntry], to: &SendTo) {
    let base: HashMap<&Path, &Path> = files
        .iter()
        .map(|f| (f.path.as_path(), f.base.as_path()))
        .collect();
    // The release's advertisements stay where they were, and so does what
    // a moving work's own folder keeps.
    for i in items
        .iter_mut()
        .filter(|i| i.dest.is_none() && !i.release_junk && !i.stays_behind)
    {
        let rel = base
            .get(i.path.as_path())
            .and_then(|b| i.dir.strip_prefix(b).ok())
            .unwrap_or(Path::new(""));
        let dest = to.dir.join(rel);
        if dest == i.dir {
            continue;
        }
        let levels: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        if i.status == Status::Unchanged {
            i.status = Status::Ok;
        }
        if i.problem.is_none()
            && let Some(p) = levels.iter().find_map(|l| validate(l))
        {
            i.status = Status::Conflict;
            i.problem = Some(p);
        }
        i.folder = (!levels.is_empty()).then(|| levels.join("/"));
        i.dest = Some(dest);
    }
}

/// A book told apart from others: its authors and title.
fn book_key(m: &BookMatch) -> String {
    format!("{}\0{}", m.authors.join("\0"), m.title).to_lowercase()
}

fn is_audio_path(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| crate::music::is_audio(&e.to_string_lossy()))
}

/// A movie's, episode's or audiobook's place: where its folder's files
/// go (none: nowhere) and the folder shown.
type WorkPlace = (Option<PathBuf>, Option<String>);

/// What `follow_works` reads of the works of each folder.
struct Works {
    /// Each work folder's place (none: not all its works go to one new
    /// folder, or it is not their own).
    places: HashMap<PathBuf, Option<WorkPlace>>,
    /// The movies of each folder.
    ids: HashMap<PathBuf, HashSet<u64>>,
    /// Audiobooks' folders (above disc folders) and their books.
    audiobooks: HashMap<PathBuf, HashSet<String>>,
    /// Episodes' folders (a season's) and their shows.
    shows: HashMap<PathBuf, HashSet<(String, u64)>>,
    /// Each folder's works' names: old stem, new stem, where it goes and
    /// the work.
    stems: HashMap<PathBuf, Vec<(String, String, WorkPlace, PathBuf)>>,
}

/// Whether the file is a movie, episode, book or comic found in a catalogue.
fn is_work(i: &PreviewItem, view: &View) -> bool {
    !i.is_dir
        && i.extra.is_none()
        && (view.movies.contains_key(&i.path)
            || view.episodes.contains_key(&i.path)
            || view.books.contains_key(&i.path)
            || view.comics.contains_key(&i.path))
}

/// Notes where one work of `dir` goes: a folder keeps a place only while
/// all its works go to the same one.
fn note_place(
    places: &mut HashMap<PathBuf, Option<WorkPlace>>,
    dir: PathBuf,
    moving: Option<WorkPlace>,
) {
    match places.get_mut(&dir) {
        Some(p) if *p != moving => *p = None,
        Some(_) => {}
        None => {
            places.insert(dir, moving);
        }
    }
}

/// The works of each folder and where they go.
fn collect_works(items: &[PreviewItem], view: &View) -> Works {
    let mut places: HashMap<PathBuf, Option<WorkPlace>> = HashMap::new();
    let mut ids: HashMap<PathBuf, HashSet<u64>> = HashMap::new();
    let mut audiobooks: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    let mut shows: HashMap<PathBuf, HashSet<(String, u64)>> = HashMap::new();
    let mut stems: HashMap<PathBuf, Vec<(String, String, WorkPlace, PathBuf)>> = HashMap::new();
    for i in items
        .iter()
        .filter(|i| is_work(i, view) && i.status != Status::Conflict)
    {
        let here = (i.dest.clone(), i.folder.clone());
        if let Some(m) = view.movies.get(&i.path) {
            ids.entry(i.dir.clone()).or_default().insert(m.id);
            note_place(
                &mut places,
                i.dir.clone(),
                i.dest.is_some().then(|| here.clone()),
            );
        } else if let Some(e) = view.episodes.get(&i.path) {
            shows
                .entry(i.dir.clone())
                .or_default()
                .insert((e.source.clone(), e.id));
            note_place(
                &mut places,
                i.dir.clone(),
                i.dest.is_some().then(|| here.clone()),
            );
        } else if is_audio_path(&i.path)
            && let Some(m) = view.books.get(&i.path)
        {
            let dir = crate::audiobook::book_dir(&i.dir);
            audiobooks
                .entry(dir.clone())
                .or_default()
                .insert(book_key(m));
            note_place(&mut places, dir, i.dest.is_some().then(|| here.clone()));
        }
        let (old, new) = (split_name(&i.original).0, split_name(&i.new_name).0);
        stems.entry(i.dir.clone()).or_default().push((
            old.to_string(),
            new.to_string(),
            here,
            i.path.clone(),
        ));
    }
    Works {
        places,
        ids,
        audiobooks,
        shows,
        stems,
    }
}

/// Only a movie's own folder, an audiobook's or a season's keeps its
/// place: every audio file in an audiobook's folder (and its disc
/// folders) is that book, a season's folder holds one show's episodes.
fn keep_own_folders(w: &mut Works, items: &[PreviewItem], view: &View) {
    // Only a movie's own folder, or an audiobook's: every audio file in
    // it (and in its disc folders) is that book.
    for (dir, place) in w.places.iter_mut() {
        if let Some(books) = w.audiobooks.get(dir) {
            let own = books.len() == 1
                && walkdir::WalkDir::new(dir)
                    .max_depth(2)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|e| !e.file_type().is_dir() && is_audio_path(e.path()))
                    .all(|e| {
                        view.books.get(e.path()).map(book_key).as_ref() == books.iter().next()
                    });
            if !own {
                *place = None;
            }
            continue;
        }
        // A season's folder: only episodes of one show and what goes with
        // them.
        if let Some(show) = w.shows.get(dir) {
            let own = show.len() == 1
                && !w.ids.contains_key(dir)
                && items
                    .iter()
                    .filter(|i| &i.dir == dir)
                    .find_map(|i| view.episodes.get(&i.path))
                    .is_some_and(|e| own_episode_folder(dir, e, view));
            if !own {
                *place = None;
            }
            continue;
        }
        let own = w.ids.get(dir).is_some_and(|s| s.len() == 1)
            && items
                .iter()
                .filter(|i| &i.dir == dir && i.extra.is_none())
                .find_map(|i| Some((i, view.movies.get(&i.path)?)))
                .is_some_and(|(i, m)| own_movie_folder(dir, &i.path, m, view));
        if !own {
            *place = None;
        }
    }
}

/// Files that go with a movie, an episode, a book or a comic found in a
/// catalogue, when no rule changed them (artwork and `.nfo` a media server
/// left, extras):
/// - those named after it (`Movie.nfo`, `Movie-poster.jpg`) get its new
///   name and go where it goes;
/// - in a movie's own folder (named after it, or holding only it with its
///   subtitles, images and `.nfo`), when the movie goes to a new folder the
///   others go there too (`backdrop.jpg`), with their subfolder
///   (`extrafanart/`, two levels at most).
fn follow_works(items: &mut [PreviewItem], files: &[FileEntry], view: &View) {
    // The folder each file was added with: a folder's files follow its
    // movie only when added with it (not a folder of photos added on its
    // own that happens to be inside).
    let base: HashMap<&Path, &Path> = files
        .iter()
        .map(|f| (f.path.as_path(), f.base.as_path()))
        .collect();
    let mut works = collect_works(items, view);
    // Each movie folder's files were added with: the movie's.
    let mut added: HashMap<PathBuf, &Path> = items
        .iter()
        .filter(|i| i.extra.is_none() && view.movies.contains_key(&i.path))
        .filter_map(|i| Some((i.dir.clone(), *base.get(i.path.as_path())?)))
        .collect();
    for i in items
        .iter()
        .filter(|i| is_audio_path(&i.path) && view.books.contains_key(&i.path))
    {
        if let Some(b) = base.get(i.path.as_path()) {
            added.insert(crate::audiobook::book_dir(&i.dir), b);
        }
    }
    for i in items.iter().filter(|i| view.episodes.contains_key(&i.path)) {
        if let Some(b) = base.get(i.path.as_path()) {
            added.insert(i.dir.clone(), b);
        }
    }
    keep_own_folders(&mut works, items, view);
    let Works {
        places,
        audiobooks,
        shows,
        stems,
        ..
    } = works;
    // The movie folder a folder is in (two levels at most), where its movie
    // goes, and the folder shown.
    let home =
        |dir: &Path, own_base: Option<&Path>| -> Option<(PathBuf, PathBuf, Option<String>)> {
            let up = dir
                .ancestors()
                .take(3)
                .find(|up| places.contains_key(*up))?;
            if added.get(up).copied() != own_base {
                return None;
            }
            let (Some(dest), folder) = places.get(up)?.clone()? else {
                return None;
            };
            Some((up.to_path_buf(), dest, folder))
        };
    let place = |dir: &Path, own_base: Option<&Path>| -> Option<(PathBuf, Option<String>)> {
        let (up, dest, folder) = home(dir, own_base)?;
        if up == dir {
            return Some((dest, folder));
        }
        let rel = dir.strip_prefix(&up).ok()?;
        let shown = rel.to_string_lossy().replace('\\', "/");
        let folder = match folder {
            Some(f) => format!("{f}/{shown}"),
            None => shown,
        };
        Some((dest.join(rel), Some(folder)))
    };
    for i in items.iter_mut() {
        if i.is_dir || i.manual || is_work(i, view) || i.dest.is_some() {
            continue;
        }
        // Named after a work of its folder: the longest stem it starts with.
        let named = stems
            .get(&i.dir)
            .into_iter()
            .flatten()
            .filter(|(old, new, (dest, _), _)| {
                (old != new || dest.is_some())
                    && i.original.len() > old.len()
                    && i.original.starts_with(old.as_str())
                    && i.original[old.len()..].starts_with(['.', '-', ' '])
            })
            .max_by_key(|(old, _, _, _)| old.len())
            .cloned();
        // Changed by a rule and named after no work: the rule's. Named
        // after one, it keeps going with it (a text rule run on every file
        // must not part a video from its .nfo and pictures).
        if i.new_name != i.original && named.is_none() {
            continue;
        }
        let own_base = base.get(i.path.as_path()).copied();
        if i.release_junk {
            // Stays in the old folder, with what else does not go.
            continue;
        } else if let Some((old, new, (dest, folder), work)) = named {
            i.new_name = format!("{new}{}", &i.original[old.len()..]);
            i.dest = dest;
            i.folder = folder;
            i.follows = Some(work);
            i.companion = true;
        } else if let Some(kind) = i.extra.clone() {
            // In the extras folder next to the movie, whatever folder it
            // was in.
            let Some((_, dest, folder)) = home(&i.dir, own_base) else {
                continue;
            };
            i.dest = Some(dest.join(&kind));
            i.folder = Some(match folder {
                Some(f) => format!("{f}/{kind}"),
                None => kind,
            });
        } else if let Some((up, _, _)) = home(&i.dir, own_base)
            && !audiobooks.contains_key(&up)
            && !goes_along(i, &up)
        {
            i.stays_behind = true;
            continue;
        } else if let Some((up, _, _)) = home(&i.dir, own_base)
            && let Some((dest, folder)) = place(&i.dir, own_base)
        {
            i.with_folder = Some(up);
            // A season's folder: the show's own files go to the show's
            // folder, the rest (season.nfo, its poster) to the season's.
            let up = folder
                .as_deref()
                .and_then(|f| f.rsplit_once('/'))
                .map(|(show, _)| show.to_string());
            match (
                shows.contains_key(&i.dir) && show_level(&i.original),
                up,
                dest.parent(),
            ) {
                (true, Some(show), Some(parent)) => {
                    i.dest = Some(parent.to_path_buf());
                    i.folder = Some(show);
                }
                _ => {
                    i.dest = Some(dest);
                    i.folder = folder;
                }
            }
            i.companion = true;
        } else {
            continue;
        }
        if i.status == Status::Unchanged {
            i.status = Status::Ok;
        }
        i.windows_unsafe = windows_bad(&i.new_name);
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

/// Photos taken in the same second (bursts), or whose date gives a name
/// another file has: `-1`, `-2`… after the name, in the order of their
/// names; a photo's files (raw, edits) take the same number. Identical
/// copies take their original's number and are left to the duplicate
/// check. A name already used by a file that is not in the list counts
/// as taken.
fn number_shots(
    items: &mut [PreviewItem],
    names: &HashMap<&Path, crate::photos::Named>,
    groups: &HashMap<&Path, (usize, String)>,
    identical: &HashMap<PathBuf, usize>,
) {
    let mut keys = PathKeys::default();
    let in_batch: HashSet<PathBuf> = items.iter().map(|i| keys.key(&i.path)).collect();
    // The name before its kept part (`.CR2`) and extension, where it goes.
    let base_of = |i: &PreviewItem, kept: &str| -> Option<(PathBuf, String, String)> {
        let (stem, ext) = split_name(&i.new_name);
        let base = stem.strip_suffix(kept)?;
        Some((
            i.dest.clone().unwrap_or_else(|| i.dir.clone()),
            base.to_string(),
            ext.to_string(),
        ))
    };
    // Each place and name: the groups going there, and their files.
    let mut places: HashMap<PathBuf, Vec<(usize, Vec<usize>)>> = HashMap::new();
    for (n, i) in items.iter().enumerate() {
        if i.manual || i.status == Status::Conflict {
            continue;
        }
        let (Some(_), Some((group, kept))) =
            (names.get(i.path.as_path()), groups.get(i.path.as_path()))
        else {
            continue;
        };
        let Some((dir, base, _)) = base_of(i, kept) else {
            continue;
        };
        let units = places.entry(keys.key(&dir.join(&base))).or_default();
        match units.iter_mut().find(|(g, _)| g == group) {
            Some((_, files)) => files.push(n),
            None => units.push((*group, vec![n])),
        }
    }
    let mut taken: HashSet<PathBuf> = HashSet::new();
    let mut changes: Vec<(usize, PathBuf)> = Vec::new();
    for units in places.values_mut() {
        let target = |i: &PreviewItem, number: Option<usize>| -> Option<PathBuf> {
            let kept = &groups.get(i.path.as_path())?.1;
            let (dir, base, ext) = base_of(i, kept)?;
            let stem = match number {
                Some(k) => format!("{base}-{k}{kept}"),
                None => format!("{base}{kept}"),
            };
            Some(dir.join(join_name(&stem, &ext)))
        };
        // On disk already, by a file that is not in the list.
        let foreign = |keys: &mut PathKeys, p: &Path| {
            p.symlink_metadata().is_ok() && !in_batch.contains(&keys.key(p))
        };
        // Identical copies are one photo here: the duplicate check tells them.
        let mut kinds: Vec<Option<usize>> = Vec::new();
        let mut distinct = 0;
        for (_, files) in units.iter() {
            let copy = files
                .iter()
                .find_map(|&n| identical.get(&items[n].path).copied());
            if copy.is_none() || !kinds.contains(&copy) {
                distinct += 1;
                kinds.push(copy);
            }
        }
        let crowded = distinct > 1
            || units[0].1.iter().any(|&n| {
                target(&items[n], None)
                    .is_some_and(|t| foreign(&mut keys, &t) && t != items[n].path)
            });
        if !crowded {
            continue;
        }
        let first_name = |files: &Vec<usize>| {
            files
                .iter()
                .map(|&n| items[n].original.clone())
                .min_by(|a, b| natural_cmp(a, b))
                .unwrap_or_default()
        };
        units.sort_by(|a, b| natural_cmp(&first_name(&a.1), &first_name(&b.1)));
        let copy_of = |files: &Vec<usize>| {
            files
                .iter()
                .find_map(|&n| identical.get(&items[n].path).copied())
        };
        let mut given: Vec<(Option<usize>, usize)> = Vec::new();
        let mut next = 1;
        for (_, files) in units.iter() {
            let copy = copy_of(files);
            let number = match given.iter().find(|(c, _)| c.is_some() && *c == copy) {
                Some(&(_, k)) => k,
                None => loop {
                    let k = next;
                    next += 1;
                    let free = files.iter().all(|&n| {
                        target(&items[n], Some(k)).is_some_and(|t| {
                            let key = keys.key(&t);
                            !taken.contains(&key) && (!foreign(&mut keys, &t) || t == items[n].path)
                        })
                    });
                    if free {
                        break k;
                    }
                },
            };
            given.push((copy, number));
            for &n in files {
                if let Some(t) = target(&items[n], Some(number)) {
                    taken.insert(keys.key(&t));
                    changes.push((n, t));
                }
            }
        }
    }
    for (n, t) in changes {
        let i = &mut items[n];
        i.new_name = t
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        i.windows_unsafe = windows_bad(&i.new_name);
        i.status = if i.target() == i.path {
            Status::Unchanged
        } else {
            Status::Ok
        };
    }
}

/// Where each file's folders go: the roots shared by a release, the
/// folders of a show, a movie, an album, a book or a comic series, and
/// what follows a work or goes with the rest.
#[allow(clippy::too_many_arguments)]
fn place_folders(
    items: &mut [PreviewItem],
    files: &[FileEntry],
    view: &View,
    comics: &HashMap<&Path, (ComicMatch, Option<ComicMatch>)>,
    songs: &HashMap<&Path, AudioTags>,
    music_folders: Option<bool>,
    photos_named: bool,
    photos: &HashMap<&Path, crate::photos::Named>,
    photo_groups: &HashMap<&Path, (usize, String)>,
) {
    // Sent elsewhere, the destination is the library: no root is looked
    // for around the files, nor old folders taken for new ones.
    let in_place = view.send_to.is_none();
    if in_place {
        climb_shared_roots(items);
    }
    unify_folders(items);
    if in_place {
        place_show_folders(items, view);
        place_movie_folders(items, view);
    }
    let series: HashMap<&Path, String> = comics
        .iter()
        .map(|(p, (m, folder))| (*p, folder.as_ref().unwrap_or(m).series.to_lowercase()))
        .filter(|(_, s)| !s.is_empty())
        .collect();
    if in_place {
        place_own_folders(items, &series, comics::is_comic, |_| false, view);
    }
    let books: HashMap<&Path, String> = view
        .books
        .iter()
        .map(|(p, m)| (p.as_path(), book_key(m)))
        .collect();
    if in_place {
        // With audiobooks, their audio files are the books, and their disc
        // folders (`CD1`) are inside their own.
        let audio = view.books.keys().any(|p| is_audio_path(p));
        if audio {
            place_own_folders(
                items,
                &books,
                |e| crate::ebook::is_book(e) || crate::music::is_audio(e),
                |n| crate::music::disc_folder(n).is_some(),
                view,
            );
        } else {
            place_own_folders(items, &books, crate::ebook::is_book, |_| false, view);
        }
    }
    if let Some(folders) = music_folders {
        if folders && in_place {
            keep_collections(items, songs);
            skip_artist_level(items, songs);
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
                items,
                &albums,
                music::is_audio,
                |n| music::disc_folder(n).is_some(),
                view,
            );
        }
        follow_songs(items, folders, &view.identical);
    }
    follow_works(items, files, view);
    if let Some(s) = &view.send_to {
        send_rest(items, files, s);
    }
    if photos_named {
        number_shots(items, photos, photo_groups, &view.identical);
    }
}

/// Files of one batch that land on the same name: all but the first are
/// in conflict.
fn flag_repeated_targets(items: &mut [PreviewItem], keys: &mut PathKeys, view: &View) {
    // Repeated targets within the batch. A file that keeps its name is not
    // flagged: the one moving onto it is, as "exists", below.
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
            let own = view.identical.get(&items[i].path);
            let twin = own.is_some()
                && idx
                    .iter()
                    .any(|&j| j != i && view.identical.get(&items[j].path) == own);
            items[i].status = Status::Conflict;
            items[i].problem = Some(if copies {
                Problem::Copy
            } else if twin {
                Problem::DuplicateIdentical
            } else {
                Problem::Duplicate
            });
        }
    }
}

/// On Linux, a disk formatted for Windows refuses the names Windows refuses.
fn flag_windows_disks(items: &mut [PreviewItem], keys: &mut PathKeys) {
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
}

/// Files whose target is already on the disk (and not moving away) are in
/// conflict.
fn flag_existing_targets(items: &mut [PreviewItem], keys: &mut PathKeys, copying: bool) {
    // Target already exists on disk and is not moving away (copies leave
    // nothing). Repeat until stable, since a conflict keeps that file from
    // being moved.
    loop {
        let moving: HashSet<PathBuf> = items
            .iter()
            .filter(|i| i.status == Status::Ok && !copying)
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
            items[i].problem = Some(
                if crate::identical::same_content(&items[i].path, &items[i].target()) {
                    Problem::SameThere
                } else {
                    Problem::Exists
                },
            );
        }
    }
}

/// What goes with a work that stays, or with a folder where nothing moves,
/// stays as it is.
fn keep_followers(items: &mut [PreviewItem], view: &View) {
    // What goes with a work that stays (in conflict, or the same file
    // already there) stays with it, as it is.
    let staying: HashSet<PathBuf> = items
        .iter()
        .filter(|i| i.status == Status::Conflict)
        .map(|i| i.path.clone())
        .collect();
    // Folders where some work still moves.
    let moving_from: HashSet<PathBuf> = items
        .iter()
        .filter(|i| i.status == Status::Ok && i.follows.is_none() && i.with_folder.is_none())
        .filter(|i| {
            view.movies.contains_key(&i.path)
                || view.episodes.contains_key(&i.path)
                || view.books.contains_key(&i.path)
                || view.comics.contains_key(&i.path)
        })
        .flat_map(|i| {
            i.path
                .ancestors()
                .skip(1)
                .take(3)
                .map(Path::to_path_buf)
                .collect::<Vec<_>>()
        })
        .collect();
    for i in items.iter_mut() {
        if i.follows.as_ref().is_some_and(|w| staying.contains(w))
            || i.with_folder
                .as_ref()
                .is_some_and(|d| !moving_from.contains(d))
        {
            i.new_name = i.original.clone();
            i.dest = None;
            i.folder = None;
            i.status = Status::Unchanged;
            i.problem = None;
            i.windows_unsafe = false;
        }
    }
}

/// The list in the order the view asks for.
fn sort_items(items: &mut [PreviewItem], view: &View) {
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
    if view.alike_only {
        items.sort_by_key(|i| view.alike.get(&i.path).copied());
    }
    if view.similar_only {
        items.sort_by_key(|i| view.similar.get(&i.path).copied());
    }
}

/// Marks the files the lookup rule leaves alone (renamed by another rule,
/// or an advert or extra with its own tag: nothing to tell).
fn mark_outside(items: &mut [PreviewItem], pipeline: &Pipeline) {
    // Files the lookup rule on leaves alone (renamed by another rule, or
    // an advert or extra with its own tag: nothing to tell).
    if let Some((kind, audiobooks)) = pipeline.lookup_kind() {
        for i in items.iter_mut() {
            let told = (i.status == Status::Ok && !i.manual)
                || i.release_junk
                || i.extra.is_some()
                || rule_takes(kind, audiobooks, i);
            if !told {
                i.outside = Some(kind);
            }
        }
    }
}

/// The folders a file's rules move it into, and where that is: none when
/// it is already there.
fn move_target(
    file: &FileEntry,
    view: &View,
    moved: Option<Move>,
    folder: String,
) -> (Vec<String>, Option<PathBuf>) {
    // A movie already in a folder with its name stays there.
    let inside = file
        .dir
        .file_name()
        .is_some_and(|d| d.to_string_lossy() == folder);
    let (root, names) = match (&view.send_to, moved) {
        // The rules' folders go into the destination (the
        // other files get theirs at the end, see `send_rest`).
        (Some(s), Some(Move::OwnFolder)) => (s.dir.clone(), vec![folder]),
        (Some(s), Some(Move::Into { folders, .. })) => (s.dir.clone(), folders),
        (None, Some(Move::OwnFolder)) if !inside => (file.dir.clone(), vec![folder]),
        (None, Some(Move::Into { root, folders })) => (root, folders),
        _ => (file.dir.clone(), Vec::new()),
    };
    let to = names.iter().fold(root, |p, n| p.join(n));
    // Already in that folder: stay there.
    if to != file.dir {
        (names, Some(to))
    } else {
        (Vec::new(), None)
    }
}

/// Whether the file's tags (or a comic's `ComicInfo.xml`) change, and
/// whether the comic rule clears the `ComicInfo.xml`.
fn tag_state(file: &FileEntry, view: &View, cleans: bool) -> (bool, bool) {
    let own = file.audio_tags().cloned().unwrap_or_default();
    let matched = view.songs.get(&file.path);
    let retag = matched.is_some_and(|m| m.differs(&own))
        || view.comics.get(&file.path).is_some_and(|m| {
            file.ext.eq_ignore_ascii_case("cbz")
                && crate::inside::comic_differs(m, file.comic_info())
        });
    // A CBZ whose `ComicInfo.xml` the comic rule clears.
    let junk = (matched.is_some() && own.junk)
        || (cleans && file.ext.eq_ignore_ascii_case("cbz") && file.comic_info().is_some());
    (retag, junk)
}

/// What the book and comic rules do with the file besides renaming it:
/// converts it (to CBZ, to a book format), finds it locked, counts its pictures.
fn convert_state(
    file: &FileEntry,
    converts: Option<bool>,
    book_to: Option<crate::bookconv::Target>,
) -> (bool, Option<crate::bookconv::Target>, bool, Option<usize>) {
    let convert = converts.is_some_and(|draw| crate::rules::converts(file, draw));
    let to_book = book_to.filter(|t| crate::rules::converts_book(file, *t));
    let drm = book_to.is_some()
        && !file.is_dir
        && (crate::ebook::is_book(&file.ext) || crate::bookconv::is_document(&file.ext))
        && file.book_source().is_some_and(|(_, locked)| locked);
    let drawn = convert
        .then(|| file.convert_check())
        .flatten()
        .filter(|n| *n > 0 && file.ext.eq_ignore_ascii_case("pdf"));
    (convert, to_book, drm, drawn)
}

/// What the subtitle tools do with the file: the fix, the tracks to
/// extract and the picture tracks that cannot be.
fn subtitle_state(
    file: &FileEntry,
    new_name: &str,
    sub_tools: &Option<(crate::subtext::Tools, Option<String>)>,
    sub_langs: &Option<Vec<crate::subtitles::Lang>>,
) -> (Option<crate::subtext::Fix>, usize, usize) {
    let sub_fix = sub_tools.as_ref().and_then(|(t, _)| {
        let ext = new_name.rsplit_once('.').map_or("", |(_, e)| e);
        file.subtitle()
            .and_then(|s| crate::subtext::plan(s, t, ext))
    });
    let (sub_extract, sub_pictures) = match (&sub_tools, &sub_langs) {
        (Some((t, _)), Some(langs)) if !file.is_dir => {
            let tracks = file.sub_tracks();
            (
                crate::mkvsubs::targets(&file.path, tracks, langs, t.to_srt).len(),
                tracks.iter().filter(|t| !t.readable()).count(),
            )
        }
        _ => (0, 0),
    };
    (sub_fix, sub_extract, sub_pictures)
}

/// A movie's extra, and whether the file is a release's advert.
fn extra_state(
    file: &FileEntry,
    view: &View,
    movie_rule: bool,
    video_rule: bool,
) -> (Option<String>, bool) {
    let extra = movie_rule
        .then(|| crate::extras::extra_kind(&file.path))
        .flatten()
        .map(str::to_string);
    let release_junk = video_rule
        && !file.is_dir
        && extra.is_none()
        && !view.movies.contains_key(&file.path)
        && !view.episodes.contains_key(&file.path)
        && crate::extras::release_junk(&file.path, file.size);
    (extra, release_junk)
}

/// The time a photo's file is set to, when the photo rule sets times.
fn photo_touch(
    file: &FileEntry,
    set_times: bool,
    photos: &HashMap<&Path, crate::photos::Named>,
) -> Option<i64> {
    use chrono::TimeZone;
    set_times
        .then_some(())
        .and_then(|()| photos.get(file.path.as_path()))
        .filter(|n| n.shot.from != crate::photos::DateFrom::Modified)
        .and_then(|n| {
            Local
                .from_local_datetime(&n.shot.taken)
                .earliest()
                .map(|t| t.timestamp())
        })
}

/// The songs' tags, with the album-wide fields agreed across each album.
fn song_tags_of<'a>(
    files: &'a [FileEntry],
    filter: &Filter,
    view: &View,
    from_name: bool,
) -> HashMap<&'a Path, AudioTags> {
    let visible: Vec<&FileEntry> = files.iter().filter(|f| filter.takes(f)).collect();
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

/// The files shown, with their place in the list and in their folder.
fn list_shown<'a>(
    files: &'a [FileEntry],
    filter: &Filter,
    view: &View,
) -> Vec<(usize, usize, &'a FileEntry)> {
    // The files shown, with their place in the list and in their folder.
    let mut per_folder: HashMap<&Path, usize> = HashMap::new();
    files
        .iter()
        .filter(|f| filter.takes(f))
        .filter(|f| !view.identical_only || view.identical.contains_key(&f.path))
        .filter(|f| !view.alike_only || view.alike.contains_key(&f.path))
        .filter(|f| !view.similar_only || view.similar.contains_key(&f.path))
        .enumerate()
        .map(|(index, file)| {
            let n = per_folder.entry(&file.dir).or_default();
            *n += 1;
            (index, *n - 1, file)
        })
        .collect()
}

/// Each photo's date (the lead's of its group) and the name it keeps.
fn photo_names<'a>(
    shown: &[(usize, usize, &'a FileEntry)],
    photo_groups: &HashMap<&'a Path, (usize, String)>,
    opts: &crate::photos::Options,
    view: &View,
) -> HashMap<&'a Path, crate::photos::Named> {
    let todo: Vec<&FileEntry> = shown
        .iter()
        .map(|&(_, _, f)| f)
        .filter(|f| photo_groups.contains_key(f.path.as_path()))
        .collect();
    crate::photos::read_all(&todo, &|_, _| {});
    let mut lead: HashMap<usize, (&FileEntry, crate::photos::Shot)> = HashMap::new();
    for &(_, _, f) in shown {
        let Some(&(group, _)) = photo_groups.get(f.path.as_path()) else {
            continue;
        };
        let own = crate::photos::shot(f, opts);
        // A date typed in counts over the file's.
        let s = match view.dates.get(&f.path) {
            Some(d) => crate::photos::typed(f, own, d),
            None => own,
        };
        let Some(s) = s else {
            continue;
        };
        let better = match lead.get(&group) {
            None => true,
            Some((g, gs)) => {
                (crate::photos::lead_rank(f, &s), f.file_name())
                    < (crate::photos::lead_rank(g, gs), g.file_name())
            }
        };
        if better {
            lead.insert(group, (f, s));
        }
    }
    photo_groups
        .iter()
        .filter_map(|(path, (group, kept))| {
            let (_, s) = lead.get(group)?;
            Some((
                *path,
                crate::photos::Named {
                    shot: s.clone(),
                    kept: kept.clone(),
                },
            ))
        })
        .collect()
}

/// The video each subtitle goes with, whose picture names it too: the one
/// with the same movie or episode in its folder, the one with its name
/// first.
fn subtitle_videos<'a>(files: &'a [FileEntry], view: &View) -> HashMap<&'a Path, &'a FileEntry> {
    // The video each subtitle goes with, whose picture names it too: the
    // one with the same movie or episode in its folder, the one with its
    // name first.
    let same_lookup = |a: &FileEntry, b: &FileEntry| {
        if let (Some(x), Some(y)) = (view.movies.get(&a.path), view.movies.get(&b.path)) {
            return x.id == y.id;
        }
        matches!(
            (view.episodes.get(&a.path), view.episodes.get(&b.path)),
            (Some(x), Some(y)) if x.id == y.id && x.season == y.season && x.episodes == y.episodes
        )
    };
    files
        .iter()
        .filter(|f| !f.is_dir && tmdb::is_subtitle(&f.ext))
        .filter_map(|sub| {
            let base =
                &sub.stem[..sub.stem.len() - tmdb::subtitle_suffix(&sub.stem, &sub.ext).len()];
            let mine: Vec<&FileEntry> = files
                .iter()
                .filter(|v| {
                    !v.is_dir && tmdb::is_video(&v.ext) && v.dir == sub.dir && same_lookup(sub, v)
                })
                .collect();
            let video = mine
                .iter()
                .find(|v| v.stem == base)
                .or_else(|| (mine.len() == 1).then(|| &mine[0]))?;
            Some((sub.path.as_path(), *video))
        })
        .collect()
}

/// The comic rule names the batch's comics together (see `comics::arrange`):
/// what each is, and the series of its folder.
fn comic_matches<'a>(
    shown: &[(usize, usize, &'a FileEntry)],
    view: &View,
    lookup: &HashMap<&Path, (String, String)>,
    converts: Option<bool>,
    by_folder: bool,
) -> HashMap<&'a Path, (ComicMatch, Option<ComicMatch>)> {
    let (paths, mut found): (Vec<&Path>, Vec<ComicMatch>) = shown
        .iter()
        .map(|&(_, _, f)| f)
        .filter(|f| {
            if f.is_dir {
                // A folder of pictures, when it becomes a CBZ.
                converts.is_some_and(|draw| crate::rules::converts(f, draw))
            } else {
                comics::is_comic(&f.ext)
            }
        })
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
    let movie_rule = pipeline.movie();
    // Rules that move a release's videos out of its folder.
    let video_rule = movie_rule || pipeline.series();
    let converts = pipeline.converts_comics();
    let book_to = pipeline.converts_books();
    let comic_ebook = pipeline.comic_ebook();
    let cleans = pipeline.cleans_comic_info();
    // Songs' tags, with the album-wide fields agreed across each album.
    let songs: HashMap<&Path, AudioTags> = match music {
        None => HashMap::new(),
        Some((from_name, _)) => song_tags_of(files, &filter, view, from_name),
    };
    let shown = list_shown(files, &filter, view);
    // The photo rule names a photo's files (raw, edits, a Live Photo's
    // video) by the date of the one that has it (see `photos::lead_rank`).
    let photo_rule = pipeline.photos().map(|(mut o, set)| {
        o.lang = crate::geo::Lang::from_code(&view.lang);
        (o, set)
    });
    let sub_tools = pipeline.subtitle_tools();
    let sub_langs = sub_tools
        .as_ref()
        .and_then(|(_, l)| l.as_deref())
        .map(crate::subtitles::langs);
    let photo_groups: HashMap<&Path, (usize, String)> = match photo_rule {
        None => HashMap::new(),
        Some(_) => {
            let list: Vec<&FileEntry> = shown.iter().map(|&(_, _, f)| f).collect();
            crate::photos::groups(&list)
        }
    };
    let photos: HashMap<&Path, crate::photos::Named> = match photo_rule {
        None => HashMap::new(),
        Some((opts, _)) => photo_names(&shown, &photo_groups, &opts, view),
    };
    let subtitle_video = subtitle_videos(files, view);
    // The same, files of one name with other extensions counting as one.
    let pairs = crate::rules::Places::of(shown.iter().map(|&(_, _, f)| f));
    // A rule for some files only counts among them.
    let step_places: Vec<Option<crate::rules::Places>> = pipeline
        .filters()
        .map(|only| {
            only.map(|o| {
                crate::rules::Places::of(shown.iter().map(|&(_, _, f)| f).filter(|f| o.takes(f)))
            })
        })
        .collect();
    let ctx_of = |index, folder_index, file| Ctx {
        file,
        index,
        folder_index,
        pair_index: pairs.get(&file.path).map_or(index, |p| p[2]),
        pair_folder_index: pairs.get(&file.path).map_or(folder_index, |p| p[3]),
        step_places: Some(&step_places),
        now,
        // Each lookup names only its own kind of file.
        movie: view
            .movies
            .get(&file.path)
            .filter(|_| file.is_dir || tmdb::is_screen(&file.ext))
            .filter(|_| !movie_rule || crate::extras::extra_kind(&file.path).is_none()),
        episode: view
            .episodes
            .get(&file.path)
            .filter(|_| !file.is_dir && tmdb::is_screen(&file.ext)),
        song: songs.get(file.path.as_path()),
        book: view.books.get(&file.path).filter(|_| {
            !file.is_dir && (crate::ebook::is_book(&file.ext) || crate::music::is_audio(&file.ext))
        }),
        comic: None,
        comic_folder: None,
        video: if tmdb::is_video(&file.ext) {
            Some(file)
        } else {
            subtitle_video.get(file.path.as_path()).copied()
        },
        as_folder: false,
        photo: photos.get(file.path.as_path()),
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
        Some(by_folder) => comic_matches(&shown, view, &lookup, converts, by_folder),
    };
    let photo_set = photo_rule.is_some_and(|(_, set)| set);
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
            let folders: Vec<String>;
            let dest: Option<PathBuf>;
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
                // The movie's folder is named as the video, without a
                // subtitle's language or the picture's size.
                let folder = match moved {
                    Some(Move::OwnFolder) => {
                        pipeline
                            .run_full(&Ctx {
                                as_folder: true,
                                ..ctx
                            })
                            .0
                    }
                    _ => String::new(),
                };
                (folders, dest) = move_target(file, view, moved, folder);
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
            let (retag, junk) = tag_state(file, view, cleans);
            let (convert, to_book, drm, drawn) = convert_state(file, converts, book_to);
            let (sub_fix, sub_extract, sub_pictures) =
                subtitle_state(file, &new_name, &sub_tools, &sub_langs);
            let comic_to = comic_ebook
                .filter(|(_, draw)| crate::rules::to_ebook(file, *draw))
                .map(|(t, _)| t);
            let (extra, release_junk) = extra_state(file, view, movie_rule, video_rule);
            PreviewItem {
                kinds: kinds_of(file),
                editable: false,
                pair: if file.is_dir {
                    file.stem.clone()
                } else {
                    crate::rules::pair_split(file).0.to_string()
                },
                outside: None,
                follows: None,
                companion: false,
                with_folder: None,
                stays_behind: false,
                extra,
                release_junk,
                to_trash: release_junk
                    && view.trash_ads
                    && !view.send_to.as_ref().is_some_and(|s| s.copy),
                comic_to,
                sub_fix,
                sub_extract,
                sub_pictures,
                path: file.path.clone(),
                dir: file.dir.clone(),
                size: file.size,
                size_text: crate::text::size(file.size),
                modified: file.modified,
                is_dir: file.is_dir,
                original,
                new_name,
                folder,
                dir_shown: crate::text::home_short(&file.dir),
                dest_shown: dest.as_deref().map(crate::text::home_short),
                dest,
                status,
                problem,
                windows_unsafe,
                manual,
                untagged,
                retag,
                junk,
                convert,
                book_to: to_book,
                drm,
                drawn,
                lookup_name,
                within: view.send_to.as_ref().map(|_| file.base.clone()),
                photo_date: photo_groups.get(file.path.as_path()).map(|_| {
                    photos
                        .get(file.path.as_path())
                        .map_or(crate::photos::DateFrom::None, |n| n.shot.from)
                }),
                touch: photo_touch(file, photo_set, &photos),
            }
        })
        .collect();
    place_folders(
        &mut items,
        files,
        view,
        &comics,
        &songs,
        music.map(|(_, folders)| folders),
        photo_rule.is_some(),
        &photos,
        &photo_groups,
    );
    let mut keys = PathKeys::default();
    if view.number_clashes {
        number_clashes(&mut items, &mut keys, view);
    }

    flag_repeated_targets(&mut items, &mut keys, view);
    flag_windows_disks(&mut items, &mut keys);
    let copying = view.send_to.as_ref().is_some_and(|s| s.copy);
    flag_existing_targets(&mut items, &mut keys, copying);
    keep_followers(&mut items, view);
    sort_items(&mut items, view);
    mark_outside(&mut items, &pipeline);
    let changed = items.iter().filter(|i| i.status == Status::Ok).count();
    let conflicts = items
        .iter()
        .filter(|i| i.status == Status::Conflict)
        .count();
    let same_there = items
        .iter()
        .filter(|i| i.problem == Some(Problem::SameThere))
        .count();
    let (left_folders, trash_folders) = left_folders(&items, view);
    // Where each advertisement is once its old folder is renamed.
    let trash = items
        .iter()
        .filter(|i| i.to_trash && !trash_folders.iter().any(|f| i.path.starts_with(f)))
        .map(|i| {
            left_folders
                .iter()
                .find_map(|l| Some(l.to.join(i.path.strip_prefix(&l.from).ok()?)))
                .unwrap_or_else(|| i.path.clone())
        })
        .collect();
    let hints = hints(&shown.iter().map(|&(_, _, f)| f).collect::<Vec<_>>(), steps);
    if let Some((kind, audiobooks)) = editor(steps) {
        for i in items.iter_mut() {
            i.editable = editable(kind, i, audiobooks);
        }
    }
    Preview {
        hints,
        items,
        rule_errors: pipeline.errors,
        filter_error,
        total: files.len(),
        changed,
        conflicts,
        same_there,
        left_folders,
        trash,
        trash_folders,
        copy: copying,
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
    // Without devices to compare: the disk's top folder is caught above.
    #[cfg(not(unix))]
    let _ = up;
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
        let found = view.movies.get(e.path());
        // Its extras, and what the release's makers added.
        if found.is_none_or(|m| m.id == id) && crate::extras::extra_kind(e.path()).is_some() {
            continue;
        }
        let size = e.metadata().map(|m| m.len()).unwrap_or(u64::MAX);
        if found.is_none() && crate::extras::release_junk(e.path(), size) {
            continue;
        }
        if tmdb::is_video(&ext) {
            if found.is_none_or(|m| m.id != id) {
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

/// Whether `dir`, holding the movie `m` in `video` and no other movie, is
/// that movie's own folder: named after it (`Movie.2010.1080p.BluRay`, in
/// the language of the catalogue or the original one), named like its
/// video (a release named in another language), or holding only it and what
/// goes with it (subtitles, images, `.nfo`, its extras, the release's
/// advertisements).
fn own_movie_folder(dir: &Path, video: &Path, m: &tmdb::MovieMatch, view: &View) -> bool {
    !protected(dir, view)
        && (tmdb::names_movie(dir, m)
            || named_like(dir, video)
            || only_movie_files(dir, m.id, view))
}

/// What of a work's own folder goes along with it to its new folder:
/// what a media server or player uses (subtitles, `.nfo`, artwork under
/// the names Jellyfin and Kodi read, what is in its subfolders such as
/// `extrafanart/`). Other texts and pictures (a release's notes, a
/// "Blu-ray" badge) stay in the old folder.
fn goes_along(i: &PreviewItem, own: &Path) -> bool {
    if i.dir != own {
        return true;
    }
    let n = i.original.to_lowercase();
    let (stem, ext) = n.rsplit_once('.').unwrap_or((n.as_str(), ""));
    if tmdb::is_subtitle(ext) || ext == "nfo" {
        return true;
    }
    if !matches!(ext, "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "tbn") {
        return false;
    }
    let base = stem.trim_end_matches(|c: char| c.is_ascii_digit());
    [
        "poster",
        "folder",
        "cover",
        "default",
        "movie",
        "show",
        "fanart",
        "backdrop",
        "background",
        "art",
        "banner",
        "logo",
        "clearlogo",
        "clearart",
        "disc",
        "discart",
        "cdart",
        "landscape",
        "thumb",
        "keyart",
        "characterart",
        "season",
    ]
    .contains(&base)
        || stem.starts_with("season")
        || [
            "-poster",
            "-fanart",
            "-landscape",
            "-banner",
            "-logo",
            "-clearart",
            "-thumb",
            "-disc",
        ]
        .iter()
        .any(|s| stem.ends_with(s))
}

/// A show's own file found in a season's folder (Kodi's and Jellyfin's
/// names): `tvshow.nfo`, the show's artwork and the seasons' posters.
fn show_level(name: &str) -> bool {
    let n = name.to_lowercase();
    let stem = n.rsplit_once('.').map_or(n.as_str(), |(s, _)| s);
    stem == "tvshow"
        || [
            "fanart",
            "logo",
            "clearlogo",
            "clearart",
            "banner",
            "landscape",
            "backdrop",
        ]
        .contains(&stem)
        || (stem.starts_with("season")
            && stem.contains('-')
            && !stem.starts_with("season.")
            && stem != "season")
}

/// Whether `dir` is a release of the show of `e`: not one of the user's
/// own folders, and holding only episodes of that show and what goes with
/// them (subtitles, images, `.nfo`, `.txt`, the release's advertisements).
fn own_episode_folder(dir: &Path, e: &crate::tv::EpisodeMatch, view: &View) -> bool {
    if protected(dir, view) {
        return false;
    }
    let mut any = false;
    for f in walkdir::WalkDir::new(dir).max_depth(3).into_iter() {
        let Ok(f) = f else { return false };
        if f.file_type().is_dir() {
            continue;
        }
        let ext = f
            .path()
            .extension()
            .map(|x| x.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let found = view.episodes.get(f.path());
        let size = f.metadata().map(|m| m.len()).unwrap_or(u64::MAX);
        if found.is_none() && crate::extras::release_junk(f.path(), size) {
            continue;
        }
        if tmdb::is_video(&ext) {
            if found.is_none_or(|m| m.id != e.id || m.source != e.source) {
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

/// A folder named like a file in it (`Movie.2017.1080p.WEB-DL/` holding
/// `Movie.2017.1080p.WEB-DL.mkv`): the same title and year.
fn named_like(dir: &Path, file: &Path) -> bool {
    let (Some(d), Some(f)) = (dir.file_name(), file.file_stem()) else {
        return false;
    };
    let (a, b) = (
        tmdb::guess(&d.to_string_lossy()),
        tmdb::guess(&f.to_string_lossy()),
    );
    !a.title.is_empty() && a.year == b.year && tmdb::similarity(&a.title, &b.title) >= 0.9
}

/// A movie's own folder under another name (see [`own_movie_folder`]).
/// The movie's new folder goes next to it, not inside.
fn place_movie_folders(items: &mut [PreviewItem], view: &View) {
    // The movies in each folder: a folder of several is no movie's own.
    let mut movies: HashMap<PathBuf, HashSet<u64>> = HashMap::new();
    for i in items.iter().filter(|i| !i.is_dir && i.extra.is_none()) {
        if let Some(m) = view.movies.get(&i.path) {
            movies.entry(i.dir.clone()).or_default().insert(m.id);
        }
    }
    let mut own: HashMap<PathBuf, bool> = HashMap::new();
    for i in items.iter_mut() {
        let Some(m) = view.movies.get(&i.path).filter(|_| i.extra.is_none()) else {
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
                && own_movie_folder(&i.dir, &i.path, m, view)
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
///
/// Also the folders left with only the release's advertisements, when they
/// go to the trash: sent there whole instead.
fn left_folders(items: &[PreviewItem], view: &View) -> (Vec<LeftFolder>, Vec<PathBuf>) {
    // Each old folder and the new one its files go to (None: several).
    let mut pairs: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    for i in items.iter().filter(|i| i.status == Status::Ok && !i.is_dir) {
        let (Some(folder), Some(dest)) = (&i.folder, &i.dest) else {
            continue;
        };
        let Some(root) = dest.ancestors().nth(folder.split('/').count()) else {
            continue;
        };
        // Sent to another folder: the movie's own folder it left (the
        // release's) is named after its new one, where it is.
        // Copied, nothing leaves it.
        if let Some(s) = &view.send_to
            && !i.dir.starts_with(root)
        {
            // An episode's: named after its show's new folder.
            let to = if s.copy {
                None
            } else if let Some(m) = view.movies.get(&i.path)
                && i.extra.is_none()
                && own_movie_folder(&i.dir, &i.path, m, view)
            {
                Some(dest.clone())
            } else if let Some(e) = view.episodes.get(&i.path)
                && let Some(show) = folder.split('/').next()
                && own_episode_folder(&i.dir, e, view)
            {
                Some(root.join(show))
            } else {
                None
            };
            if let Some(to) = to {
                let to = Some(to);
                match pairs.iter_mut().find(|p| p.0 == i.dir) {
                    Some(p) if p.1 != to => p.1 = None,
                    Some(_) => {}
                    None => pairs.push((i.dir.clone(), to)),
                }
            }
            continue;
        }
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
    let mut trash = Vec::new();
    let ads: HashSet<&Path> = items
        .iter()
        .filter(|i| i.to_trash)
        .map(|i| i.path.as_path())
        .collect();
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
            .filter(|e| !ads.contains(e.path()))
            .count();
        if files == 0 {
            // Only advertisements left: the folder goes to the trash.
            if items
                .iter()
                .any(|i| i.to_trash && i.path.starts_with(&from))
            {
                trash.push(from);
            }
            continue;
        }
        let Some(up) = from.parent() else { continue };
        let to = up.join(format!("{} ({suffix})", name.to_string_lossy()));
        if std::fs::symlink_metadata(&to).is_err() {
            left.push(LeftFolder { from, to, files });
        }
    }
    (left, trash)
}

/// Sends the release's advertisements to the trash after the moves (see
/// [`Preview::trash`]): the files that still look like advertisements, and
/// the folders that hold nothing else (any other file keeps a folder, and
/// only its advertisements go).
pub fn trash_leftovers(
    files: &[PathBuf],
    folders: &[PathBuf],
) -> (Vec<PathBuf>, Vec<(PathBuf, String)>) {
    let is_ad = |p: &Path| {
        std::fs::symlink_metadata(p)
            .is_ok_and(|m| m.is_file() && crate::extras::release_junk(p, m.len()))
    };
    let mut ads: Vec<PathBuf> = files.iter().filter(|p| is_ad(p)).cloned().collect();
    let mut whole = Vec::new();
    for dir in folders.iter().filter(|d| d.is_dir()) {
        let inside: Vec<PathBuf> = walkdir::WalkDir::new(dir)
            .min_depth(1)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir())
            .map(|e| e.into_path())
            .collect();
        if inside.iter().all(|p| is_ad(p)) {
            whole.push(dir.clone());
        } else {
            ads.extend(inside.into_iter().filter(|p| is_ad(p)));
        }
    }
    let (mut done, mut failed) = (Vec::new(), Vec::new());
    for p in whole.into_iter().chain(ads) {
        match crate::files::to_trash(&p) {
            Ok(()) => done.push(p),
            Err(e) => failed.push((p, e)),
        }
    }
    (done, failed)
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
            only: String::new(),
            rule: Rule::Replace {
                find: find.into(),
                replace: with.into(),
                case_sensitive: true,
                first_only: false,
                target: Target::Name,
            },
        }]
    }

    fn names_of(p: &Preview) -> Vec<String> {
        p.items.iter().map(|i| i.new_name.clone()).collect()
    }

    #[test]
    fn pairs_count_as_one_and_rules_only_on_some() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(
            d.path(),
            &[
                "a.mkv",
                "a.pt-BR.srt",
                "b.mkv",
                "b.srt",
                "c.jpg",
                "c.CR2.xmp",
                "d.mkv",
                "d-thumb.jpg",
                "d.nfo",
            ],
        );
        let number = |pairs| Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Number {
                template: "Ep {n}".into(),
                start: 1,
                step: 1,
                digits: 1,
                per_folder: false,
                pairs,
            },
        };
        let p = preview(&files, &[number(true)], &HashMap::new(), &View::default());
        assert_eq!(
            names_of(&p),
            [
                "Ep 1.mkv",
                "Ep 1.pt-BR.srt",
                "Ep 2.mkv",
                "Ep 2.srt",
                "Ep 3.jpg",
                "Ep 3.CR2.xmp",
                "Ep 4.mkv",
                "Ep 4-thumb.jpg",
                "Ep 4.nfo"
            ]
        );
        let p = preview(&files, &[number(false)], &HashMap::new(), &View::default());
        assert_eq!(names_of(&p)[1], "Ep 2.srt");
        // A rule for some files only: the others pass it by.
        let mut only = replace("a", "x");
        only[0].only = "srt".into();
        let p = preview(&files, &only, &HashMap::new(), &View::default());
        assert_eq!(names_of(&p)[..2], ["a.mkv", "x.pt-BR.srt"]);
        // Counted among the files it takes.
        let mut videos = vec![number(false)];
        videos[0].only = "mkv".into();
        let p = preview(&files, &videos, &HashMap::new(), &View::default());
        assert_eq!(
            names_of(&p)[..4],
            ["Ep 1.mkv", "a.pt-BR.srt", "Ep 2.mkv", "b.srt"]
        );
    }

    #[test]
    fn clashing_names_numbered_when_asked() {
        let d = tempfile::tempdir().unwrap();
        let files = entries(d.path(), &["x1.txt", "x2.txt", "x3.txt"]);
        fs::write(d.path().join("Nota.txt"), "").unwrap();
        let steps = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::List {
                names: "Nota\nNota\nOutra".into(),
                pairs: false,
            },
        }];
        let p = preview(&files, &steps, &HashMap::new(), &View::default());
        assert!(p.items[..2].iter().all(|i| i.status == Status::Conflict));
        let view = View {
            number_clashes: true,
            ..View::default()
        };
        let p = preview(&files, &steps, &HashMap::new(), &view);
        // `Nota.txt` is on the disk already, and stays.
        assert_eq!(names_of(&p), ["Nota (2).txt", "Nota (3).txt", "Outra.txt"]);
        assert!(p.items.iter().all(|i| i.status == Status::Ok));
    }

    #[test]
    fn subtitle_rule_names_and_marks() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path();
        fs::write(
            dir.join("Movie.en.ass"),
            "[Script Info]\nScriptType: v4.00+\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hi\n",
        )
        .unwrap();
        fs::write(
            dir.join("Movie.pt.srt"),
            b"1\r\n00:00:01,000 --> 00:00:02,000\r\nOl\xE1 [RISOS]\r\n\r\n",
        )
        .unwrap();
        fs::write(dir.join("Movie.mkv"), crate::mkvsubs::tests::sample()).unwrap();
        fs::write(dir.join("Movie.sub"), "{25}{50}Hi\n").unwrap();
        let files: Vec<FileEntry> = ["Movie.en.ass", "Movie.pt.srt", "Movie.mkv", "Movie.sub"]
            .iter()
            .map(|n| {
                let p = dir.join(n);
                FileEntry::from_path(&p, &fs::metadata(&p).unwrap()).unwrap()
            })
            .collect();
        let steps = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Subtitle {
                to_srt: true,
                utf8: true,
                ads: false,
                hearing: true,
                shift_ms: 0,
                fps_from: 0.0,
                fps_to: 0.0,
                extract: true,
                extract_langs: String::new(),
            },
        }];
        let p = preview(&files, &steps, &HashMap::new(), &View::default());
        let get = |name: &str| p.items.iter().find(|i| i.original == name).unwrap();
        assert_eq!(get("Movie.en.ass").new_name, "Movie.en.srt");
        assert!(get("Movie.en.ass").sub_fix.as_ref().unwrap().convert);
        let srt = get("Movie.pt.srt");
        assert_eq!(srt.status, Status::Unchanged);
        let fix = srt.sub_fix.as_ref().unwrap();
        assert_eq!((fix.encoding, fix.hearing), (Some("windows-1252"), 1));
        // Its times are frames of a rate not known: left as it is.
        assert_eq!(get("Movie.sub").new_name, "Movie.sub");
        let mkv = get("Movie.mkv");
        // Its Portuguese track is beside it already.
        assert_eq!((mkv.sub_extract, mkv.sub_pictures), (1, 1));
    }

    #[test]
    fn photos_by_date() {
        let d = tempfile::tempdir().unwrap();
        let fotos = d.path().join("Fotos");
        fs::create_dir_all(fotos.join("2019/2019-05")).unwrap();
        // Already there, not in the list: its name is taken.
        fs::write(fotos.join("2019/2019-05/2019-05-12 14.30.22.jpg"), "x").unwrap();
        let files = entries(
            &fotos,
            &[
                "IMG_20190512_143022.jpg",
                "IMG_20190512_143022.dng",
                "IMG_20190512_143022.xmp",
                "PXL_20190512_150000002.jpg",
                "PXL_20190512_150000001.jpg",
                "IMG-20190513-WA0001.jpg",
                "IMG-20190513-WA0002.jpg",
                "foto.jpg",
                "notes.txt",
            ],
        );
        let steps = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Photo {
                template: "{ano}-{mês}-{dia} {hora}.{minuto}.{segundo}".into(),
                from_name: true,
                modified: false,
                shift_hours: 0,
                folders: true,
                folder_template: "{ano}/{ano}-{mês}".into(),
                set_modified: false,
            },
        }];
        let p = preview(&files, &steps, &HashMap::new(), &View::default());
        let got = |name: &str| {
            let i = p.items.iter().find(|i| i.original == name).unwrap();
            (
                i.folder.clone().unwrap_or_default(),
                i.new_name.clone(),
                i.photo_date,
            )
        };
        use crate::photos::DateFrom;
        let named = |name: &str| (got(name).0, got(name).1);
        let may = |s: &str| ("2019/2019-05".to_string(), s.to_string());
        assert_eq!(
            named("IMG_20190512_143022.jpg"),
            may("2019-05-12 14.30.22-1.jpg")
        );
        assert_eq!(
            named("IMG_20190512_143022.dng"),
            may("2019-05-12 14.30.22-1.dng")
        );
        assert_eq!(
            named("IMG_20190512_143022.xmp"),
            may("2019-05-12 14.30.22-1.xmp")
        );
        assert_eq!(
            named("PXL_20190512_150000001.jpg"),
            may("2019-05-12 15.00.00-1.jpg")
        );
        assert_eq!(
            named("PXL_20190512_150000002.jpg"),
            may("2019-05-12 15.00.00-2.jpg")
        );
        assert_eq!(named("IMG-20190513-WA0001.jpg"), may("2019-05-13-1.jpg"));
        assert_eq!(named("IMG-20190513-WA0002.jpg"), may("2019-05-13-2.jpg"));
        assert_eq!(got("IMG-20190513-WA0001.jpg").2, Some(DateFrom::Name));
        assert_eq!(
            got("foto.jpg"),
            (String::new(), "foto.jpg".into(), Some(DateFrom::None))
        );
        assert_eq!(got("notes.txt"), (String::new(), "notes.txt".into(), None));
        assert!(p.items.iter().all(|i| i.status != Status::Conflict));

        // A date typed in counts over the name's and a dateless file's;
        // a time typed names it too.
        let typed = View {
            dates: HashMap::from([
                (fotos.join("IMG-20190513-WA0001.jpg"), "2018-03-27".into()),
                (fotos.join("foto.jpg"), "2018-03-27 14:05".into()),
            ]),
            ..View::default()
        };
        let p2 = preview(&files, &steps, &HashMap::new(), &typed);
        let got2 = |name: &str| {
            let i = p2.items.iter().find(|i| i.original == name).unwrap();
            (
                i.folder.clone().unwrap_or_default(),
                i.new_name.clone(),
                i.photo_date,
            )
        };
        let march = |s: &str| {
            (
                "2018/2018-03".to_string(),
                s.to_string(),
                Some(DateFrom::Typed),
            )
        };
        assert_eq!(got2("IMG-20190513-WA0001.jpg"), march("2018-03-27.jpg"));
        assert_eq!(got2("foto.jpg"), march("2018-03-27 14.05.00.jpg"));
        assert_eq!(got2("IMG-20190513-WA0002.jpg").2, Some(DateFrom::Name));

        // An identical copy alone is not numbered: the duplicate check
        // tells it.
        let copies = entries(
            &fotos,
            &["IMG_20190601_101010.jpg", "copia IMG_20190601_101010.jpg"],
        );
        let mut view = View::default();
        for f in &copies {
            view.identical.insert(f.path.clone(), 7);
        }
        let p = preview(&copies, &steps, &HashMap::new(), &view);
        let names: Vec<&str> = p.items.iter().map(|i| i.new_name.as_str()).collect();
        assert_eq!(
            names,
            ["2019-06-01 10.10.10.jpg", "2019-06-01 10.10.10.jpg"]
        );
        assert_eq!(
            p.items
                .iter()
                .filter(|i| i.problem == Some(Problem::Copy))
                .count(),
            1
        );

        // An identical copy alone is not numbered: the duplicate check
        // tells it.
        let copies = entries(
            &fotos,
            &["IMG_20190601_101010.jpg", "copia IMG_20190601_101010.jpg"],
        );
        let mut view = View::default();
        for f in &copies {
            view.identical.insert(f.path.clone(), 7);
        }
        let p = preview(&copies, &steps, &HashMap::new(), &view);
        let names: Vec<&str> = p.items.iter().map(|i| i.new_name.as_str()).collect();
        assert_eq!(
            names,
            ["2019-06-01 10.10.10.jpg", "2019-06-01 10.10.10.jpg"]
        );
        assert_eq!(
            p.items
                .iter()
                .filter(|i| i.problem == Some(Problem::Copy))
                .count(),
            1
        );

        // Named so, they stay so.
        let again = entries(
            &fotos.join("2019/2019-05"),
            &["2019-05-12 15.00.00-1.jpg", "2019-05-12 15.00.00-2.jpg"],
        );
        let p = preview(&again, &steps, &HashMap::new(), &View::default());
        assert!(
            p.items.iter().all(|i| i.status == Status::Unchanged),
            "{:?}",
            p.items
        );
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
            only: String::new(),
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
        // An error on Windows, a warning elsewhere.
        if cfg!(windows) {
            assert_eq!(p.items[0].problem, Some(Problem::WindowsChars));
        } else {
            assert_eq!(p.items[0].status, Status::Ok);
        }
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
            only: String::new(),
            rule: Rule::Number {
                template: "{n}".into(),
                start: 1,
                step: 1,
                digits: 1,
                per_folder: false,
                pairs: false,
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
            only: String::new(),
            rule: Rule::Number {
                template: "{folder}-{n}".into(),
                start: 1,
                step: 1,
                digits: 1,
                per_folder: true,
                pairs: false,
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
            extra: Default::default(),
            id: 1,
            title: "Filme".into(),
            original_title: String::new(),
            year: Some(2001),
        };
        let steps = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Movie {
                template: "{title} ({year})".into(),
                colon: " - ".into(),
                sidecar: false,
                fanart: false,
                folder: true,
                missing: false,
                subtitles: false,
                sub_langs: String::new(),
                video_tags: false,
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

        // The subtitle goes into the same folder.
        let files = entries(d.path(), &["f.2001.mkv", "f.2001.pt-BR.srt"]);
        let mut view = View::default();
        for f in &files {
            view.movies.insert(f.path.clone(), m.clone());
        }
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let to: Vec<_> = p.operations().iter().map(|o| o.to.clone()).collect();
        assert!(
            to.contains(&d.path().join("Filme (2001)/Filme (2001).pt-BR.srt")),
            "{to:?}"
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
    fn forced_subtitle_stays_forced() {
        let d = tempfile::tempdir().unwrap();
        let m = MovieMatch {
            extra: Default::default(),
            id: 12155,
            title: "Alice no País das Maravilhas".into(),
            original_title: String::new(),
            year: Some(2010),
        };
        let steps = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Movie {
                template: "{title} ({year}) [tmdbid-{id}]".into(),
                colon: " - ".into(),
                sidecar: false,
                fanart: false,
                folder: false,
                missing: false,
                subtitles: false,
                sub_langs: String::new(),
                video_tags: false,
            },
        }];
        let old = "Alice no Pais das Maravilhas (2010) [tmdbid-12155]";
        let files = entries(
            d.path(),
            &[
                &format!("{old}.mkv"),
                &format!("{old}-forced.srt"),
                &format!("{old}.srt"),
            ],
        );
        let mut view = View::default();
        for f in &files {
            view.movies.insert(f.path.clone(), m.clone());
        }
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert_eq!(p.conflicts, 0);
        let mut names: Vec<_> = p.items.iter().map(|i| i.new_name.as_str()).collect();
        names.sort();
        let new = "Alice no País das Maravilhas (2010) [tmdbid-12155]";
        assert_eq!(
            names,
            [
                format!("{new}.forced.srt"),
                format!("{new}.mkv"),
                format!("{new}.srt"),
            ]
        );
    }

    #[test]
    fn movie_versions_by_picture() {
        let d = tempfile::tempdir().unwrap();
        let m = MovieMatch {
            extra: Default::default(),
            id: 1,
            title: "Filme".into(),
            original_title: String::new(),
            year: Some(2001),
        };
        let steps = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Movie {
                template: "{title} ({year})< - {resolution}>".into(),
                colon: " - ".into(),
                sidecar: false,
                fanart: false,
                folder: true,
                missing: false,
                subtitles: false,
                sub_langs: String::new(),
                video_tags: false,
            },
        }];
        fs::write(d.path().join("notes.ods"), "n").unwrap();
        let mut files = entries(d.path(), &["f.2001.pt-BR.srt"]);
        let video = d.path().join("f.2001.mkv");
        fs::copy("tests/fixtures/video.mkv", &video).unwrap();
        files.push(FileEntry::from_path(&video, &fs::metadata(&video).unwrap()).unwrap());
        let mut view = View::default();
        for f in &files {
            view.movies.insert(f.path.clone(), m.clone());
        }
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let mut to: Vec<_> = p.operations().iter().map(|o| o.to.clone()).collect();
        to.sort();
        // The subtitle takes its video's picture; the folder has neither.
        assert_eq!(
            to,
            [
                d.path().join("Filme (2001)/Filme (2001) - 136p.mkv"),
                d.path().join("Filme (2001)/Filme (2001) - 136p.pt-BR.srt"),
            ]
        );
    }

    #[test]
    fn artwork_follows_its_movie() {
        let d = tempfile::tempdir().unwrap();
        let out = d.path().join("Destino");
        fs::create_dir_all(&out).unwrap();
        let steps = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Movie {
                template: "{title} ({year})".into(),
                colon: " - ".into(),
                sidecar: false,
                fanart: false,
                folder: true,
                missing: false,
                subtitles: false,
                sub_langs: String::new(),
                video_tags: false,
            },
        }];
        // Two movie folders, each added on its own, with what a media
        // server left in them.
        let mut files = Vec::new();
        let mut view = View::default();
        for (n, dir) in ["um", "dois"].iter().enumerate() {
            let at = d.path().join("Filmes").join(dir);
            fs::create_dir_all(at.join("extrafanart")).unwrap();
            let mut here = entries(
                &at,
                &[
                    "filme.mkv",
                    "filme.nfo",
                    "filme-poster.jpg",
                    "backdrop.jpg",
                    "extrafanart/f1.jpg",
                ],
            );
            for f in &mut here {
                f.base = at.clone();
            }
            view.movies.insert(
                here[0].path.clone(),
                MovieMatch {
                    extra: Default::default(),
                    id: n as u64 + 1,
                    title: format!("Filme {dir}"),
                    original_title: String::new(),
                    year: Some(2001),
                },
            );
            files.extend(here);
        }
        view.send_to = Some(SendTo {
            dir: out.clone(),
            copy: true,
        });
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert_eq!(p.conflicts, 0, "{:?}", p.items);
        let mut to: Vec<_> = p.operations().iter().map(|o| o.to.clone()).collect();
        to.sort();
        let f = |dir: &str, name: &str| out.join(format!("Filme {dir} (2001)")).join(name);
        let mut want = Vec::new();
        for dir in ["um", "dois"] {
            let stem = format!("Filme {dir} (2001)");
            want.extend([
                f(dir, "backdrop.jpg"),
                f(dir, "extrafanart/f1.jpg"),
                f(dir, &format!("{stem}-poster.jpg")),
                f(dir, &format!("{stem}.mkv")),
                f(dir, &format!("{stem}.nfo")),
            ]);
        }
        want.sort();
        assert_eq!(to, want);

        // In place: they go into the new folder too.
        view.send_to = None;
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let to: Vec<_> = p.operations().iter().map(|o| o.to.clone()).collect();
        assert!(
            to.contains(&d.path().join("Filmes/Filme um (2001)/backdrop.jpg")),
            "{to:?}"
        );
        assert!(p.left_folders.is_empty(), "{:?}", p.left_folders);
    }

    #[test]
    fn sent_to_another_folder() {
        let d = tempfile::tempdir().unwrap();
        let (inp, out) = (d.path().join("Fotos"), d.path().join("Destino"));
        fs::create_dir_all(inp.join("2024/Praia")).unwrap();
        fs::create_dir_all(&out).unwrap();
        let mut files = entries(&inp, &["a.jpg", "2024/Praia/b.jpg"]);
        for f in &mut files {
            f.base = inp.clone();
        }
        let movie = d.path().join("f.2001.mkv");
        fs::write(&movie, "v").unwrap();
        files.extend(entries(d.path(), &["f.2001.mkv"]));
        let m = MovieMatch {
            extra: Default::default(),
            id: 1,
            title: "Filme".into(),
            original_title: String::new(),
            year: Some(2001),
        };
        let mut view = View::default();
        view.movies.insert(movie.clone(), m);
        view.send_to = Some(SendTo {
            dir: out.clone(),
            copy: false,
        });
        let steps = vec![
            Step {
                enabled: true,
                only: String::new(),
                rule: Rule::Replace {
                    find: "b".into(),
                    replace: "B".into(),
                    case_sensitive: true,
                    first_only: false,
                    target: crate::rules::Target::Name,
                },
            },
            Step {
                enabled: true,
                only: String::new(),
                rule: Rule::Movie {
                    template: "{title} ({year})".into(),
                    colon: " - ".into(),
                    sidecar: false,
                    fanart: false,
                    folder: true,
                    missing: false,
                    subtitles: false,
                    sub_langs: String::new(),
                    video_tags: false,
                },
            },
        ];
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let mut to: Vec<_> = p.operations().iter().map(|o| o.to.clone()).collect();
        to.sort();
        // Unchanged names go too; subfolders below the folder added stay;
        // the rule's folder goes straight into the destination.
        assert_eq!(
            to,
            [
                out.join("2024/Praia/B.jpg"),
                out.join("Filme (2001)/Filme (2001).mkv"),
                out.join("a.jpg"),
            ]
        );
        assert!(p.operations().iter().all(|o| !o.copy));

        // Copied: the originals stay, so a name another file of the batch
        // has is taken, not freed.
        fs::write(out.join("a.jpg"), "old").unwrap();
        view.send_to = Some(SendTo {
            dir: inp.clone(),
            copy: true,
        });
        let swap = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Replace {
                find: "a".into(),
                replace: "b".into(),
                case_sensitive: true,
                first_only: false,
                target: crate::rules::Target::Name,
            },
        }];
        let files = entries(&inp, &["a.jpg", "b.jpg"]);
        let p = preview(&files, &swap, &HashMap::new(), &view);
        let a = p.items.iter().find(|i| i.original == "a.jpg").unwrap();
        assert_eq!(a.problem, Some(Problem::Exists));
        // Copied onto itself: nothing to do.
        let b = p.items.iter().find(|i| i.original == "b.jpg").unwrap();
        assert_eq!(b.status, Status::Unchanged);
    }

    #[test]
    fn audiobook_in_disc_folders_with_its_cover() {
        let d = tempfile::tempdir().unwrap();
        let book = d.path().join("ecos mp3 64k");
        fs::create_dir_all(book.join("CD1")).unwrap();
        fs::create_dir_all(book.join("CD2")).unwrap();
        let m = |part| crate::books::BookMatch {
            title: "Ecos".into(),
            authors: vec!["Lia Marta Quaresma".into()],
            part,
            parts: 2,
            ..Default::default()
        };
        let steps = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Book {
                template: "{title}".into(),
                colon: " - ".into(),
                folders: true,
                folder_template: "{author}/{title}".into(),
                cover: false,
                opf: false,
                missing: false,
                convert_to: String::new(),
                audiobooks: true,
            },
        }];
        let mut files = entries(&book.join("CD1"), &["faixa 01.mp3"]);
        files.extend(entries(&book.join("CD2"), &["faixa 01.mp3"]));
        files.extend(entries(&book, &["cover.jpg", "capa.jpg", "encarte.pdf"]));
        for f in &mut files {
            f.base = book.clone();
        }
        let mut view = View::default();
        view.books.insert(files[0].path.clone(), m(1));
        view.books.insert(files[1].path.clone(), m(2));
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let to = d.path().join("Lia Marta Quaresma/Ecos");
        let dest = |name: &str| {
            let i = p.items.iter().find(|i| i.path.ends_with(name)).unwrap();
            i.dest.clone().unwrap_or(i.dir.clone()).join(&i.new_name)
        };
        // Out of its old folder and its disc folders, parts numbered, the
        // cover along.
        assert_eq!(
            dest("CD1/faixa 01.mp3"),
            to.join("Ecos - 1.mp3".replace("1.", "01."))
        );
        assert_eq!(dest("CD2/faixa 01.mp3"), to.join("Ecos - 02.mp3"));
        assert_eq!(dest("cover.jpg"), to.join("cover.jpg"));
        // Whatever else the book's folder holds goes too (not only the
        // pictures a video player reads).
        assert_eq!(dest("capa.jpg"), to.join("capa.jpg"));
        assert_eq!(dest("encarte.pdf"), to.join("encarte.pdf"));
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
            only: String::new(),
            rule: Rule::Book {
                template: "{author} - {title} ({year})".into(),
                colon: " - ".into(),
                folders: true,
                folder_template: "{author}/{title}".into(),
                cover: false,
                opf: false,
                missing: false,
                convert_to: String::new(),
                audiobooks: true,
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
            source: String::new(),
            extra: Default::default(),
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
            only: String::new(),
            rule: Rule::Series {
                template: "{show} - S{season}E{episode} - {title}".into(),
                colon: " - ".into(),
                show: String::new(),
                folders: false,
                folder_template: String::new(),
                sidecar: false,
                fanart: false,
                missing: true,
                subtitles: false,
                sub_langs: String::new(),
                video_tags: false,
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
            source: String::new(),
            extra: Default::default(),
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
            only: String::new(),
            rule: Rule::Series {
                template: "{show} - S{season}E{episode}".into(),
                colon: " - ".into(),
                show: String::new(),
                folders: true,
                folder_template: "{show} ({year}) [tmdbid-{id}]".into(),
                sidecar: false,
                fanart: false,
                missing: true,
                subtitles: false,
                sub_langs: String::new(),
                video_tags: false,
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
            only: String::new(),
            rule: Rule::Series {
                template: "{show} - S{season}E{episode}".into(),
                colon: " - ".into(),
                show: String::new(),
                folders: true,
                folder_template: "{show} ({year}) [tmdbid-{id}]".into(),
                sidecar: false,
                fanart: false,
                missing: true,
                subtitles: false,
                sub_langs: String::new(),
                video_tags: false,
            },
        }]
    }

    fn episode(n: u16) -> EpisodeMatch {
        EpisodeMatch {
            source: String::new(),
            extra: Default::default(),
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

    #[test]
    fn the_same_file_already_there_stays_put() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("baixando");
        let lib = d.path().join("lib");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&lib).unwrap();
        let big: Vec<u8> = (0..400_000u32).map(|n| (n % 251) as u8).collect();
        fs::write(src.join("a.mkv"), &big).unwrap();
        fs::write(lib.join("a.mkv"), &big).unwrap();
        // The same size, another middle: not the same file.
        let mut other = big.clone();
        other[200_000] ^= 1;
        fs::write(src.join("b.mkv"), &big).unwrap();
        fs::write(lib.join("b.mkv"), &other).unwrap();
        let files: Vec<FileEntry> = ["a.mkv", "b.mkv"]
            .iter()
            .map(|n| {
                let p = src.join(n);
                FileEntry::from_path(&p, &fs::metadata(&p).unwrap()).unwrap()
            })
            .collect();
        for number in [false, true] {
            let view = View {
                number_clashes: number,
                send_to: Some(SendTo {
                    dir: lib.clone(),
                    copy: false,
                }),
                ..View::default()
            };
            let p = preview(&files, &[], &HashMap::new(), &view);
            let a = p.items.iter().find(|i| i.original == "a.mkv").unwrap();
            assert_eq!(a.status, Status::Conflict);
            assert_eq!(a.problem, Some(Problem::SameThere));
            assert_eq!(a.new_name, "a.mkv");
            let b = p.items.iter().find(|i| i.original == "b.mkv").unwrap();
            if number {
                assert_eq!((b.status, b.new_name.as_str()), (Status::Ok, "b (2).mkv"));
            } else {
                assert_eq!(b.problem, Some(Problem::Exists));
            }
        }
    }

    #[test]
    fn an_episodes_nfo_and_pictures_go_with_it_whatever_the_rules() {
        let d = tempfile::tempdir().unwrap();
        let season = d.path().join("baixando/Season 01");
        let lib = d.path().join("Series");
        fs::create_dir_all(&season).unwrap();
        fs::create_dir_all(&lib).unwrap();
        let ep = "Show (2010) - S01E01";
        let files = entries(
            &season,
            &[
                &format!("{ep}.mkv"),
                &format!("{ep}.nfo"),
                &format!("{ep}-thumb.jpg"),
                "season.nfo",
                "tvshow.nfo",
            ],
        );
        // A text rule run on every file first (it takes "(2010)" out).
        let mut steps: Vec<Step> = vec![
            serde_json::from_value(serde_json::json!({
                "enabled": true,
                "rule": {"type": "removeRange", "from": "text", "fromText": "(",
                         "to": "text", "toText": ")", "inclusive": true}
            }))
            .unwrap(),
        ];
        steps.extend(series_steps());
        let mut view = View {
            number_clashes: true,
            send_to: Some(SendTo {
                dir: lib.clone(),
                copy: false,
            }),
            ..View::default()
        };
        view.episodes.insert(files[0].path.clone(), episode(1));
        let new = lib.join("Show (2010) [tmdbid-7]/Season 01");
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let get = |p: &Preview, n: &str| p.items.iter().find(|i| i.original == n).unwrap().clone();
        assert_eq!(
            get(&p, &format!("{ep}.nfo")).target(),
            new.join("Show - S01E01.nfo")
        );
        assert_eq!(
            get(&p, &format!("{ep}-thumb.jpg")).target(),
            new.join("Show - S01E01-thumb.jpg")
        );
        // The season's folder's own files: to the new season's folder, the
        // show's to the show's.
        assert_eq!(get(&p, "season.nfo").target(), new.join("season.nfo"));
        assert_eq!(
            get(&p, "tvshow.nfo").target(),
            lib.join("Show (2010) [tmdbid-7]/tvshow.nfo")
        );
        // The same video already there: it stays, and so do they (no
        // "(2)", nothing left at the library's top).
        fs::create_dir_all(&new).unwrap();
        fs::copy(&files[0].path, new.join("Show - S01E01.mkv")).unwrap();
        fs::write(new.join("Show - S01E01.nfo"), "other").unwrap();
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert_eq!(
            get(&p, &format!("{ep}.mkv")).problem,
            Some(Problem::SameThere)
        );
        for n in [
            format!("{ep}.nfo"),
            format!("{ep}-thumb.jpg"),
            "season.nfo".into(),
            "tvshow.nfo".into(),
        ] {
            let i = get(&p, &n);
            assert_eq!(
                (i.status, i.target()),
                (Status::Unchanged, i.path.clone()),
                "{n}"
            );
        }
        assert_eq!(p.changed, 0);
    }

    #[test]
    fn episodes_sent_to_another_folder_leave_their_release_folder() {
        let d = tempfile::tempdir().unwrap();
        let release = "Show.S01E01-02.1080p.WEB-DL";
        let downloads = d.path().join("baixando");
        let old = downloads.join(release);
        let library = d.path().join("Series");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&library).unwrap();
        let files = entries(
            &old,
            &[
                "Show.S01E01.1080p.mkv",
                "Show.S01E02.1080p.mkv",
                "BAIXAR SERIES.url",
            ],
        );
        let view_with = |trash_ads: bool| {
            let mut view = View {
                leftover_suffix: "additional files".into(),
                send_to: Some(SendTo {
                    dir: library.clone(),
                    copy: false,
                }),
                trash_ads,
                ..View::default()
            };
            for (n, f) in files[..2].iter().enumerate() {
                view.episodes.insert(f.path.clone(), episode(n as u16 + 1));
            }
            view
        };
        // Kept: the old folder named after the show's new one.
        let p = preview(&files, &series_steps(), &HashMap::new(), &view_with(false));
        let ad = p
            .items
            .iter()
            .find(|i| i.original == "BAIXAR SERIES.url")
            .unwrap();
        assert!(ad.release_junk && !ad.to_trash);
        assert_eq!(p.left_folders.len(), 1);
        assert_eq!(
            p.left_folders[0].to,
            downloads.join("Show (2010) [tmdbid-7] (additional files)")
        );
        // To the trash: only the advertisement is left, the folder goes.
        let p = preview(&files, &series_steps(), &HashMap::new(), &view_with(true));
        assert!(p.left_folders.is_empty());
        assert_eq!(p.trash_folders, vec![old.clone()]);
        // Another show's episode in it: not the release's own folder.
        let mut view = view_with(false);
        let mut other = episode(3);
        other.id = 99;
        view.episodes.insert(files[1].path.clone(), other);
        let p = preview(&files, &series_steps(), &HashMap::new(), &view);
        assert!(p.left_folders.is_empty());
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
            only: String::new(),
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
                cover_size: 1200,
                clean_tags: false,
                missing: false,
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
            only: String::new(),
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
                cover_size: 1200,
                clean_tags: false,
                missing: false,
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
            extra: Default::default(),
            id: 1,
            title: "Filme".into(),
            original_title: String::new(),
            year: Some(2001),
        };
        let steps = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Movie {
                template: "{title} ({year})".into(),
                colon: " - ".into(),
                sidecar: false,
                fanart: false,
                folder: true,
                missing: false,
                subtitles: false,
                sub_langs: String::new(),
                video_tags: false,
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
    fn folder_named_like_its_video() {
        let d = Path::new("/x");
        let r = "The.Ash.Gardens.2017.1080p.WEB-DL";
        assert!(named_like(&d.join(r), &d.join(r).join(format!("{r}.mkv"))));
        assert!(named_like(
            &d.join("The Ash Gardens (2017)"),
            &d.join("x/The.Ash.Gardens.2017.mkv")
        ));
        assert!(!named_like(
            &d.join("Filmes 2017"),
            &d.join("x/The.Ash.Gardens.2017.mkv")
        ));
        // Another year: another movie.
        assert!(!named_like(
            &d.join("The Ash Gardens 2019"),
            &d.join("x/The.Ash.Gardens.2017.mkv")
        ));
    }

    #[test]
    fn sent_movie_leaves_its_advertisements_behind() {
        let d = tempfile::tempdir().unwrap();
        let m = MovieMatch {
            extra: Default::default(),
            id: 7,
            title: "Os Jardins de Cinza".into(),
            original_title: "Os Jardins de Cinza".into(),
            year: Some(2017),
        };
        let steps = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Movie {
                template: "{title} ({year})".into(),
                colon: " - ".into(),
                sidecar: false,
                fanart: false,
                folder: true,
                missing: false,
                subtitles: false,
                sub_langs: String::new(),
                video_tags: false,
            },
        }];
        let release = "Os.Jardins.de.Cinza.2017.1080p.WEB-DL";
        let downloads = d.path().join("baixando");
        let old = downloads.join(release);
        let library = d.path().join("Filmes");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&library).unwrap();
        // Added by itself: its own folder is its base.
        let files = entries(&old, &[&format!("{release}.mkv"), "BAIXE MAIS FILMES.url"]);
        for copy in [false, true] {
            let mut view = View {
                leftover_suffix: "additional files".into(),
                send_to: Some(SendTo {
                    dir: library.clone(),
                    copy,
                }),
                ..View::default()
            };
            view.movies.insert(files[0].path.clone(), m.clone());
            let p = preview(&files, &steps, &HashMap::new(), &view);
            let new = library.join("Os Jardins de Cinza (2017)");
            let ad = p
                .items
                .iter()
                .find(|i| i.original == "BAIXE MAIS FILMES.url")
                .unwrap();
            assert!(ad.release_junk);
            assert_eq!(ad.target(), old.join("BAIXE MAIS FILMES.url"));
            assert_eq!(
                p.items[0].target(),
                new.join("Os Jardins de Cinza (2017).mkv")
            );
            if copy {
                assert!(p.left_folders.is_empty());
            } else {
                assert_eq!(p.left_folders.len(), 1);
                assert_eq!(
                    p.left_folders[0].to,
                    downloads.join("Os Jardins de Cinza (2017) (additional files)")
                );
            }

            // To the trash: the folder goes whole, nothing is renamed.
            view.trash_ads = true;
            let p = preview(&files, &steps, &HashMap::new(), &view);
            let ad = p
                .items
                .iter()
                .find(|i| i.original == "BAIXE MAIS FILMES.url")
                .unwrap();
            assert_eq!(ad.to_trash, !copy);
            assert!(p.left_folders.is_empty());
            assert!(p.trash.is_empty());
            assert_eq!(
                p.trash_folders,
                if copy { vec![] } else { vec![old.clone()] }
            );
        }
        // Something else stays: the folder is renamed, and only the
        // advertisement goes, from there.
        fs::write(old.join("notas.odt"), b"x").unwrap();
        let mut view = View {
            leftover_suffix: "additional files".into(),
            send_to: Some(SendTo {
                dir: library.clone(),
                copy: false,
            }),
            trash_ads: true,
            ..View::default()
        };
        view.movies.insert(files[0].path.clone(), m.clone());
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let left = downloads.join("Os Jardins de Cinza (2017) (additional files)");
        assert_eq!(p.left_folders.len(), 1);
        assert_eq!((&p.left_folders[0].to, p.left_folders[0].files), (&left, 1));
        assert!(p.trash_folders.is_empty());
        assert_eq!(p.trash, vec![left.join("BAIXE MAIS FILMES.url")]);
    }

    #[test]
    fn only_what_a_player_uses_goes_with_the_movie() {
        let d = tempfile::tempdir().unwrap();
        let m = MovieMatch {
            extra: Default::default(),
            id: 7,
            title: "Chumbo".into(),
            original_title: "Chumbo".into(),
            year: Some(2007),
        };
        let steps = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Movie {
                template: "{title} ({year})".into(),
                colon: " - ".into(),
                sidecar: false,
                fanart: false,
                folder: true,
                missing: false,
                subtitles: false,
                sub_langs: String::new(),
                video_tags: false,
            },
        }];
        let release = "Chumbo (2007) 720p Dublado";
        let downloads = d.path().join("baixando");
        let old = downloads.join(release);
        let lib = d.path().join("Filmes");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&lib).unwrap();
        let files = entries(
            &old,
            &[
                &format!("{release}.mp4"),
                "poster.jpg",
                "fanart.jpg",
                "Blu_Ray.jpg",
                "Como Configurar o Utorrent.txt",
                &format!("{release}.pt-BR.srt"),
            ],
        );
        let mut view = View {
            leftover_suffix: "additional files".into(),
            send_to: Some(SendTo {
                dir: lib.clone(),
                copy: false,
            }),
            ..View::default()
        };
        view.movies.insert(files[0].path.clone(), m);
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let new = lib.join("Chumbo (2007)");
        let target = |n: &str| p.items.iter().find(|i| i.original == n).unwrap().target();
        assert_eq!(target("poster.jpg"), new.join("poster.jpg"));
        assert_eq!(target("fanart.jpg"), new.join("fanart.jpg"));
        assert_eq!(
            target(&format!("{release}.pt-BR.srt")),
            new.join("Chumbo (2007).pt-BR.srt")
        );
        // A badge and a how-to stay where they were.
        let left = downloads.join("Chumbo (2007) (additional files)");
        assert_eq!(p.left_folders.len(), 1);
        assert_eq!(p.left_folders[0].to, left);
        assert_eq!(target("Blu_Ray.jpg"), old.join("Blu_Ray.jpg"));
        let howto = p
            .items
            .iter()
            .find(|i| i.original.starts_with("Como"))
            .unwrap();
        assert!(howto.release_junk);
        assert_eq!(howto.target(), howto.path);
    }

    #[test]
    fn movie_release_extras_and_advertisements() {
        let d = tempfile::tempdir().unwrap();
        let m = MovieMatch {
            extra: Default::default(),
            id: 7,
            title: "Os Jardins de Cinza".into(),
            original_title: "Os Jardins de Cinza".into(),
            year: Some(2017),
        };
        let steps = vec![Step {
            enabled: true,
            only: String::new(),
            rule: Rule::Movie {
                template: "{title} ({year})".into(),
                colon: " - ".into(),
                sidecar: false,
                fanart: false,
                folder: true,
                missing: false,
                subtitles: false,
                sub_langs: String::new(),
                video_tags: false,
            },
        }];
        // Named in another language than the catalogue's, with an extra and
        // the release's advertisements.
        let release = "The.Ash.Gardens.2017.1080p.WEB-DL.x264.NAC.WWW.EXEMPLO.COM";
        let old = d.path().join(release);
        fs::create_dir_all(old.join("Making Of")).unwrap();
        let mut files = entries(
            &old,
            &[
                &format!("{release}.mkv"),
                "EXEMPLO.COM.mp4",
                "EXEMPLO.COM.url",
                "LEIA.txt",
            ],
        );
        fs::write(old.join("LEIA.txt"), "baixe mais em https://exemplo.com").unwrap();
        files.extend(entries(
            &old.join("Making Of"),
            &["Making.of.The.Ash.Gardens.2017.1080p.mkv"],
        ));
        for f in &mut files {
            f.base = d.path().to_path_buf();
        }
        let mut view = View {
            leftover_suffix: "additional files".into(),
            ..View::default()
        };
        view.movies.insert(files[0].path.clone(), m.clone());
        let p = preview(&files, &steps, &HashMap::new(), &view);
        let new = d.path().join("Os Jardins de Cinza (2017)");
        let to = |name: &str| {
            p.items
                .iter()
                .find(|i| i.original == name)
                .map(|i| i.target())
                .unwrap()
        };
        assert_eq!(
            to(&format!("{release}.mkv")),
            new.join("Os Jardins de Cinza (2017).mkv")
        );
        assert_eq!(
            to("Making.of.The.Ash.Gardens.2017.1080p.mkv"),
            new.join("behind the scenes/Making.of.The.Ash.Gardens.2017.1080p.mkv")
        );
        for junk in ["EXEMPLO.COM.mp4", "EXEMPLO.COM.url", "LEIA.txt"] {
            let i = p.items.iter().find(|i| i.original == junk).unwrap();
            assert!(i.release_junk, "{junk}");
            assert_eq!(i.target(), old.join(junk));
        }
        assert_eq!(p.left_folders.len(), 1);
        assert_eq!(
            p.left_folders[0].to,
            d.path()
                .join("Os Jardins de Cinza (2017) (additional files)")
        );
        assert_eq!(p.left_folders[0].files, 3);

        let h = crate::apply::History::new(d.path().join("hist"));
        let r = h.apply(&p.operations());
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let left = d
            .path()
            .join("Os Jardins de Cinza (2017) (additional files)");
        assert!(left.join("EXEMPLO.COM.url").exists());
        assert!(
            new.join("behind the scenes/Making.of.The.Ash.Gardens.2017.1080p.mkv")
                .exists()
        );
        assert!(!old.exists());

        // Named after the movie: its advertisements stay behind too.
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join("Os Jardins de Cinza 2017 WEB-DL 1080p");
        fs::create_dir(&old).unwrap();
        let files = entries(
            &old,
            &["Os Jardins de Cinza 2017.mkv", "GRUPO TELEGRAM.url"],
        );
        let mut view = View {
            leftover_suffix: "additional files".into(),
            ..View::default()
        };
        view.movies.insert(files[0].path.clone(), m.clone());
        let p = preview(&files, &steps, &HashMap::new(), &view);
        assert_eq!(p.items[1].target(), files[1].path);
        assert_eq!(p.left_folders.len(), 1);
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
            only: String::new(),
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
                cover_size: 1200,
                clean_tags: false,
                missing: false,
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
            only: String::new(),
            rule: Rule::Book {
                template: "{title}".into(),
                colon: " - ".into(),
                folders: true,
                folder_template: "{author}/{title}".into(),
                cover: false,
                opf: false,
                missing: false,
                convert_to: String::new(),
                audiobooks: true,
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
            only: String::new(),
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
                cover_size: 1200,
                clean_tags: false,
                missing: false,
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
    fn comics_to_convert_are_named_cbz() {
        let d = tempfile::tempdir().unwrap();
        let rar = include_bytes!("../../renome-archive/tests/data/rar4.rar");
        let mut files = Vec::new();
        for (name, data) in [
            ("a.cbr", &rar[..]),
            ("b.cbz", &rar[..]),
            ("c.cbr", b"PK\x05\x06\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"),
            ("d.rar", &rar[..]),
        ] {
            let p = d.path().join(name);
            fs::write(&p, data).unwrap();
            files.push(FileEntry::from_path(&p, &fs::metadata(&p).unwrap()).unwrap());
        }
        // Folders: one of pages, one with a video.
        for (name, inside) in [("Herói 005", "01.jpg"), ("Filme", "filme.mp4")] {
            let p = d.path().join(name);
            fs::create_dir(&p).unwrap();
            fs::write(p.join(inside), b"x").unwrap();
            files.push(FileEntry::from_path(&p, &fs::metadata(&p).unwrap()).unwrap());
        }
        let comic = |convert| {
            vec![Step {
                enabled: true,
                only: String::new(),
                rule: Rule::Comic {
                    template: "{series} {number}".into(),
                    colon: " - ".into(),
                    series: String::new(),
                    source: "gcd".into(),
                    folders: false,
                    folder_template: String::new(),
                    by_folder: false,
                    missing: false,
                    convert,
                    draw_pages: true,
                    page_size: 2400,
                    clean_info: false,
                    ebook: String::new(),
                    device: Default::default(),
                    crop: false,
                    split: false,
                },
            }]
        };
        let p = preview(&files, &comic(true), &HashMap::new(), &View::default());
        let got: Vec<(&str, &str, bool)> = p
            .items
            .iter()
            .map(|i| (i.original.as_str(), i.new_name.as_str(), i.convert))
            .collect();
        assert_eq!(
            got,
            [
                ("a.cbr", "a.cbz", true),
                // A RAR named `.cbz`: the name stays, the content changes.
                ("b.cbz", "b.cbz", true),
                // Already a zip: only named right (not here: no change).
                ("c.cbr", "c.cbr", false),
                // Not a comic.
                ("d.rar", "d.rar", false),
                // A folder of pages becomes one, named as a comic.
                ("Herói 005", "Herói 005.cbz", true),
                ("Filme", "Filme", false),
            ]
        );
        let p = preview(&files, &comic(false), &HashMap::new(), &View::default());
        assert!(
            p.items
                .iter()
                .all(|i| !i.convert && i.new_name == i.original)
        );
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
            only: String::new(),
            rule: Rule::Comic {
                template: "{series} {number}".into(),
                colon: " - ".into(),
                series: String::new(),
                source: "gcd".into(),
                folders: true,
                folder_template: "{series} ({series year})".into(),
                by_folder: false,
                missing: false,
                convert: false,
                draw_pages: true,
                page_size: 2400,
                clean_info: false,
                ebook: String::new(),
                device: Default::default(),
                crop: false,
                split: false,
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
