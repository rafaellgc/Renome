//! Renome core.
//!
//! - [`scan`]: collects files from paths (files or folders).
//! - [`filter`]: list filter (which files are shown and will be renamed).
//! - [`rules`]: name transformation rules, applied in sequence.
//! - [`playlists`]: cue sheets and playlists follow renamed files.
//! - [`plan`]: computes the preview and checks for conflicts.
//! - [`apply`]: renames in two phases, with a history for undo.
//! - [`identical`]: finds files with identical content.
//! - [`alike`]: finds songs that are the same music in different files.
//! - [`music`]: tags of audio files and the names built from them.
//! - [`musicbrainz`]: albums from MusicBrainz (ranking, matching tracks).
//! - [`discogs`]: albums from Discogs, read as MusicBrainz's.
//! - [`ebook`]: details of book files (EPUB, MOBI, PDF, FB2) and ISBNs.
//! - [`books`]: books from Open Library (guessing, ranking, names).
//! - [`comics`]: comics (`ComicInfo.xml`, numbers in names, names).
//! - [`tmdb`]: movie title guessing, ranking and `.nfo` files for TMDB.
//! - [`tv`]: season and episode numbers, and matching files to a show.
//! - [`video`]: a video's picture size and codec, from its headers.
//! - [`details`]: what the optional columns of the list show.

pub mod acoustid;
pub mod alike;
pub mod answers;
pub mod apply;
pub mod books;
pub mod comics;
pub mod details;
pub mod discogs;
pub mod ebook;
pub mod files;
pub mod filter;
pub mod fingerprint;
pub mod identical;
pub mod kind;
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
pub mod video;

pub use apply::{ApplyReport, History, HistoryEntry, Operation};
pub use files::{FileEntry, SortKey, sort_files};
pub use filter::Filter;
pub use identical::{HashCache, find_identical};
pub use plan::{Preview, PreviewItem, Problem, Status, View, preview};
pub use rules::{Pipeline, Rule, Step};
pub use scan::{ScanOptions, ScanResult, scan};
