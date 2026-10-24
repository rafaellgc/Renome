//! Preview: applies the rules to the visible files and flags conflicts.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::apply::Operation;
use crate::files::{FileEntry, join_name, natural_cmp, split_name};
use crate::filter::Filter;
use crate::music::{self, AudioTags};
use crate::rules::{Ctx, Move, Pipeline, RuleError, Step, song_tags};
use crate::tmdb::MovieMatch;
use crate::tv::EpisodeMatch;

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
    /// Longer than 255 bytes.
    TooLong,
    /// On Windows: contains `\ : * ? " < > |` or ends in a dot or space.
    WindowsChars,
    /// Another file in the batch would get the same name.
    Duplicate,
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
    #[serde(skip)]
    pub dest: Option<PathBuf>,
    pub status: Status,
    pub problem: Option<Problem>,
    /// Non-blocking warning: contains characters Windows does not accept.
    pub windows_unsafe: bool,
    pub manual: bool,
    /// A song the music rule cannot name (no title in its tags).
    pub untagged: bool,
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
            .collect()
    }
}

const WINDOWS_UNSAFE: &[char] = &['\\', ':', '*', '?', '"', '<', '>', '|'];

fn validate(name: &str) -> Option<Problem> {
    if name.is_empty() || name == "." || name == ".." {
        Some(Problem::Empty)
    } else if name.contains(['/', '\0']) {
        Some(Problem::Slash)
    } else if cfg!(windows) && (name.contains(WINDOWS_UNSAFE) || name.ends_with(['.', ' '])) {
        Some(Problem::WindowsChars)
    } else if name.len() > 255 {
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

/// Key for comparing paths: the default file systems on Windows and macOS
/// are case-insensitive, so `A.txt` and `a.txt` are the same target.
fn path_key(p: &Path) -> PathBuf {
    if cfg!(any(windows, target_os = "macos")) {
        PathBuf::from(p.to_string_lossy().to_lowercase())
    } else {
        p.to_path_buf()
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

/// Folders spelled differently only in case (`a-ha` and `A-Ha` from
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
    let key = |root: &Path, levels: &[String]| {
        format!("{}\0{}", root.display(), levels.join("/").to_lowercase())
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
                .find(|n| n.to_lowercase() == levels[k].to_lowercase())
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

/// Files that go with songs, when no rule changed them:
/// - lyrics with a song's name (`01 Song.lrc`) get its new name;
/// - with folders, other files (covers, cue sheets, logs) go with their
///   folder's songs when all of them go to one place; files in a subfolder
///   of such a folder (`Artwork/`) keep their subfolder; and files in a
///   folder whose songs are all in disc folders (`CD 01`, `CD 02`) going
///   to one place go there too. Two such files that would get the same
///   name (`CD 01/log.txt`, `CD 02/log.txt`) get their folder's name added.
fn follow_songs(items: &mut [PreviewItem], folders: bool) {
    /// Where the songs of a folder go: one place, or several.
    #[derive(Clone, PartialEq)]
    enum Album {
        One(Option<PathBuf>, Option<String>),
        Split,
    }
    let is_song = |i: &PreviewItem| !i.is_dir && music::is_audio(split_name(&i.original).1);
    /// A song's new stem, destination and folder shown.
    type Song = (String, Option<PathBuf>, Option<String>);
    let mut by_stem: HashMap<(PathBuf, String), Song> = HashMap::new();
    let mut albums: HashMap<PathBuf, Album> = HashMap::new();
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
        let here = Album::One(i.dest.clone(), i.folder.clone());
        albums
            .entry(i.dir.clone())
            .and_modify(|a| {
                if *a != here {
                    *a = Album::Split;
                }
            })
            .or_insert(here);
    }
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
        i.windows_unsafe = i.new_name.contains(WINDOWS_UNSAFE);
        followers.push(n);
    }
    // Same name in the same place: add the folder each came from.
    let mut by_target: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for &n in &followers {
        by_target
            .entry(path_key(&items[n].target()))
            .or_default()
            .push(n);
    }
    for group in by_target.values().filter(|g| g.len() > 1) {
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
                if let Some(t) = song_tags(f, from_name) {
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
    let mut per_folder: HashMap<&Path, usize> = HashMap::new();
    let mut items: Vec<PreviewItem> = files
        .iter()
        .filter(|f| filter.matches(&f.file_name()))
        .filter(|f| !view.identical_only || view.identical.contains_key(&f.path))
        .enumerate()
        .map(|(index, file)| {
            let original = file.file_name();
            let folder_index = {
                let n = per_folder.entry(&file.dir).or_default();
                *n += 1;
                *n - 1
            };
            // The rules emptied the name but left the extension: `.txt` would
            // be a hidden file with no name.
            let mut empty_stem = false;
            let mut folders: Vec<String> = Vec::new();
            let mut dest = None;
            let (new_name, manual) = match overrides.get(&file.path) {
                Some(name) => (name.clone(), true),
                None => {
                    let ctx = Ctx {
                        index,
                        folder_index,
                        file,
                        now,
                        movie: view.movies.get(&file.path),
                        episode: view.episodes.get(&file.path),
                        song: songs.get(file.path.as_path()),
                    };
                    let (stem, ext, moved) = pipeline.run_full(&ctx);
                    empty_stem = stem.is_empty() && !file.stem.is_empty();
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
                    (join_name(&stem, &ext), false)
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
            let windows_unsafe = new_name.contains(WINDOWS_UNSAFE);
            let untagged = music.is_some_and(|_| {
                !file.is_dir
                    && music::is_audio(&file.ext)
                    && !songs.contains_key(file.path.as_path())
            });
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
            }
        })
        .collect();
    unify_folders(&mut items);
    if let Some((_, folders)) = music {
        follow_songs(&mut items, folders);
    }

    // Repeated targets within the batch. A file that keeps its name is not
    // flagged: the one moving onto it is, as "exists", below.
    let mut targets: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for (i, item) in items.iter().enumerate() {
        if item.status == Status::Ok {
            targets.entry(path_key(&item.target())).or_default().push(i);
        }
    }
    for idx in targets.values().filter(|v| v.len() > 1) {
        for &i in idx {
            items[i].status = Status::Conflict;
            items[i].problem = Some(Problem::Duplicate);
        }
    }

    // Target already exists on disk and is not moving away. Repeat until
    // stable, since a conflict keeps that file from being moved.
    loop {
        let moving: HashSet<PathBuf> = items
            .iter()
            .filter(|i| i.status == Status::Ok)
            .map(|i| path_key(&i.path))
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
            if exists && !moving.contains(&path_key(&target)) && !same_file(&target, &item.path) {
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
    Preview {
        items,
        rule_errors: pipeline.errors,
        filter_error,
        total: files.len(),
        changed,
        conflicts,
    }
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
            },
        }];
        let files = entries(d.path(), &["show.s01e02.mkv", "show.s01e02.pt-BR.srt"]);
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
                "Show - S01E02 - Title.pt-BR.srt"
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
            },
        }];
        let p = preview(&files, &steps, &HashMap::new(), &View::default());
        let get = |n: &str| p.items.iter().find(|i| i.original == n).unwrap();
        let wywh = d.path().join("Pink Floyd/Wish You Were Here (1975)");
        // Next to the old folders, not inside them.
        assert_eq!(get("01.flac").target(), wywh.join("01 - Shine On.flac"));
        assert_eq!(get("01.lrc").target(), wywh.join("01 - Shine On.lrc"));
        // The songs of x.flac's folder do not all go to one place.
        assert_eq!(get("cover.jpg").status, Status::Unchanged);
        assert!(get("x.flac").untagged);
        assert_eq!(get("x.flac").status, Status::Unchanged);

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
