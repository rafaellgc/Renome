//! The lookup rules' decisions, one flow per rule, shared by the window
//! and the command line: which files are looked up, what is picked with
//! confidence, and the doubts left for the person, each with the files
//! that share its answer. The window shows the doubts in its review and
//! the command line asks them in the terminal; neither decides on its own.

pub mod movies;
pub mod music;
pub mod series;
pub mod subtitles;

use renome_core::PreviewItem;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
        }
    }
}

impl Item {
    /// The name the lookups read, with extension.
    pub fn read_name(&self) -> &str {
        self.lookup_name.as_deref().unwrap_or(&self.original)
    }
    /// The same, without extension (a folder's whole name).
    pub fn read_stem(&self) -> String {
        let name = self.read_name();
        if self.is_dir {
            return name.to_string();
        }
        Path::new(name)
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
}
