//! Renome core.
//!
//! - [`scan`]: collects files from paths (files or folders).
//! - [`filter`]: list filter (which files are shown and will be renamed).
//! - [`rules`]: name transformation rules, applied in sequence.
//! - [`playlists`]: cue sheets and playlists follow renamed files.
//! - [`plan`]: computes the preview and checks for conflicts.
//! - [`apply`]: renames in two phases, with a history for undo.
//! - [`identical`]: finds files with identical content.
//! - [`music`]: tags of audio files and the names built from them.
//! - [`musicbrainz`]: albums from MusicBrainz (ranking, matching tracks).
//! - [`ebook`]: details of book files (EPUB, MOBI, PDF, FB2) and ISBNs.
//! - [`books`]: books from Open Library (guessing, ranking, names).
//! - [`tmdb`]: movie title guessing, ranking and `.nfo` files for TMDB.
//! - [`tv`]: season and episode numbers, and matching files to a show.

pub mod acoustid;
pub mod apply;
pub mod books;
pub mod ebook;
pub mod files;
pub mod filter;
pub mod fingerprint;
pub mod identical;
pub mod music;
pub mod musicbrainz;
pub mod plan;
pub mod playlists;
pub mod rules;
pub mod scan;
pub mod tagwrite;
pub mod text;
pub mod tmdb;
pub mod tv;

pub use apply::{ApplyReport, History, HistoryEntry, Operation};
pub use files::{FileEntry, SortKey, sort_files};
pub use filter::Filter;
pub use identical::{HashCache, find_identical};
pub use plan::{Preview, PreviewItem, Problem, Status, View, preview};
pub use rules::{Pipeline, Rule, Step};
pub use scan::{ScanOptions, ScanResult, scan};
