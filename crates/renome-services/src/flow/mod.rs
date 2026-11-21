//! The lookup rules' decisions, one flow per rule, shared by the window
//! and the command line: which files are looked up, what is picked with
//! confidence, and the doubts left for the person, each with the files
//! that share its answer. The window shows the doubts in its review and
//! the command line asks them in the terminal; neither decides on its own.

pub mod after;
pub mod books;
pub mod comics;
pub mod edit;
pub mod export;
pub mod missing;
pub mod movies;
pub mod music;
pub mod said;
pub mod series;
pub mod subtitles;
pub mod typed;

use renome_core::PreviewItem;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How a file (or album) stands after a lookup, the same for the window's
/// marks and reviews and the command line's questions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// Picked with confidence.
    Auto,
    /// Picked by the person.
    Chosen,
    /// Candidates to choose from.
    Choose,
    /// Nothing found (or none of them, by the person).
    #[default]
    None,
}

impl Status {
    /// Picked with confidence, or candidates, or nothing.
    pub fn of(picked: bool, candidates: bool) -> Self {
        if picked {
            Status::Auto
        } else if candidates {
            Status::Choose
        } else {
            Status::None
        }
    }
}

/// The files that take an answer given for `path`: it, and the files of
/// its group still undecided (a movie's subtitles, the same book in other
/// formats). `files`: each file's group and status.
pub fn same_answer<'a>(
    path: &Path,
    files: impl IntoIterator<Item = (&'a Path, &'a str, Status)>,
) -> Vec<PathBuf> {
    let files: Vec<(&Path, &str, Status)> = files.into_iter().collect();
    let Some(group) = files.iter().find(|f| f.0 == path).map(|f| f.1) else {
        return vec![path.to_path_buf()];
    };
    files
        .iter()
        .filter(|f| f.0 == path || (f.1 == group && matches!(f.2, Status::Choose | Status::None)))
        .map(|f| f.0.to_path_buf())
        .collect()
}

/// A file of the list, as the lookups see it.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub path: PathBuf,
    /// Its name on disk, with extension.
    pub original: String,
    /// The name the rules above the first lookup rule make of it.
    #[serde(default)]
    pub lookup_name: Option<String>,
    #[serde(default)]
    pub is_dir: bool,
    /// A movie's extra (a making-of…): not looked up.
    #[serde(default)]
    pub extra: Option<String>,
    /// A release's advertisement: not looked up.
    #[serde(default)]
    pub release_junk: bool,
    /// A song with no title in its tags.
    #[serde(default)]
    pub untagged: bool,
    /// A folder (or comic) the comic rule converts to CBZ.
    #[serde(default)]
    pub convert: bool,
}

impl From<&PreviewItem> for Item {
    fn from(i: &PreviewItem) -> Self {
        Item {
            path: i.path.clone(),
            original: i.original.clone(),
            lookup_name: i.lookup_name.clone(),
            is_dir: i.is_dir,
            extra: i.extra.clone(),
            release_junk: i.release_junk,
            untagged: i.untagged,
            convert: i.convert,
        }
    }
}

impl Item {
    /// The name the lookups read, with extension, as it reads (`？` as
    /// `?`, no invisible characters: see [`renome_core::text::plain`]).
    pub fn read_name(&self) -> String {
        renome_core::text::plain(self.lookup_name.as_deref().unwrap_or(&self.original))
    }
    /// The same, without extension (a folder's whole name).
    pub fn read_stem(&self) -> String {
        let name = self.read_name();
        if self.is_dir {
            return name;
        }
        Path::new(&name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.to_string())
    }
    /// The extension of its name on disk, in lower case.
    pub fn ext(&self) -> String {
        Path::new(&self.original)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    }
}

/// How many files the lookup rule `rule` (`movie`, `series`, `music`,
/// `book` or `comic`) looks up and has no answer for (`answered` tells), not
/// counting a movie's extras and a release's adverts: the files left for
/// the person to settle.
pub fn left_unanswered(rule: &str, items: &[Item], answered: &dyn Fn(&Path) -> bool) -> usize {
    items
        .iter()
        .filter(|i| i.extra.is_none() && !i.release_junk)
        .filter(|i| looked_up(rule, i))
        .filter(|i| !answered(&i.path))
        .count()
}

/// Whether the lookup rule `rule` looks this file up.
fn looked_up(rule: &str, i: &Item) -> bool {
    let ext = i.ext();
    match rule {
        "music" => renome_core::music::is_audio(&ext),
        "book" => renome_core::ebook::is_book(&ext),
        "comic" => renome_core::comics::is_comic(&ext),
        "movie" => movies::is_target(i),
        _ => series::is_target(i),
    }
}

/// What a flow asks the person in the middle of its work (see
/// [`crate::Host::ask`]).
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Question {
    /// Which show the files are: `source` is `tmdb` or `anilist`; `query`,
    /// what was searched. The answer: one of the candidates.
    Show {
        source: String,
        query: String,
        candidates: Vec<renome_core::tmdb::Candidate>,
    },
    /// Which series a group of comics is: `query`, what was searched;
    /// `files`, how many; `highest`, their highest number; `path` and
    /// `comic`, the file whose cover is compared. The answer: a series.
    ComicSeries {
        source: String,
        skip: String,
        query: String,
        files: usize,
        highest: f64,
        candidates: Vec<renome_core::comics::Volume>,
        path: Option<PathBuf>,
        comic: Option<Box<renome_core::comics::ComicMatch>>,
    },
}

#[cfg(test)]
mod left_tests {
    use super::*;

    fn item(name: &str) -> Item {
        Item {
            path: PathBuf::from(name),
            original: name.to_string(),
            lookup_name: None,
            is_dir: false,
            extra: None,
            release_junk: false,
            untagged: false,
            convert: false,
        }
    }

    #[test]
    fn counts_only_what_the_rule_looks_up_and_has_no_answer() {
        let mut extra = item("making of.mkv");
        extra.extra = Some("making-of".into());
        let items = vec![
            item("a.mkv"),
            item("a.srt"),
            item("b.mkv"),
            item("notes.txt"),
            extra,
        ];
        let answered = |p: &Path| p == Path::new("a.mkv");
        // The subtitle and the second video have no answer; the text file
        // and the extra are not looked up.
        assert_eq!(left_unanswered("movie", &items, &answered), 2);
        assert_eq!(left_unanswered("series", &items, &answered), 2);
        assert_eq!(left_unanswered("music", &items, &answered), 0);
        assert_eq!(left_unanswered("movie", &items, &|_| true), 0);
    }

    #[test]
    fn a_folder_is_a_movie_but_not_an_episode() {
        let mut dir = item("Some Movie");
        dir.is_dir = true;
        assert_eq!(left_unanswered("movie", &[dir.clone()], &|_| false), 1);
        assert_eq!(left_unanswered("series", &[dir], &|_| false), 0);
    }
}
