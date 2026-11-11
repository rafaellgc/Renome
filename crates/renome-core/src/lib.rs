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
//! - [`repeated`]: finds the same movie, episode, comic or book in several files.
//! - [`similar`]: finds pictures that look alike.
//! - [`music`]: tags of audio files and the names built from them.
//! - [`musicbrainz`]: albums from MusicBrainz (ranking, matching tracks).
//! - [`discogs`]: albums from Discogs, read as MusicBrainz's.
//! - [`ebook`]: details of book files (EPUB, MOBI, PDF, FB2) and ISBNs.
//! - [`books`]: books from Open Library (guessing, ranking, names).
//! - [`comics`]: comics (`ComicInfo.xml`, numbers in names, names).
//! - [`tmdb`]: movie title guessing, ranking and `.nfo` files for TMDB.
//! - [`tv`]: season and episode numbers, and matching files to a show.
//! - [`video`]: a video's picture size and codec, from its headers.
//! - [`videotags`]: title, date, show and cover written inside MKV and MP4.
//! - [`inside`]: details written inside CBZ and EPUB files.
//! - [`convert`]: comics converted to CBZ (from RAR, 7z, tar).
//! - [`comicbook`]: comics made into fixed-layout EPUB or AZW3 for readers.
//! - [`subtext`]: text subtitles read, cleaned, retimed, made SubRip.
//! - [`mkvsubs`]: text subtitles taken out of Matroska videos.
//! - [`details`]: what the optional columns of the list show.

pub mod acoustid;
pub mod alike;
pub mod answers;
pub mod apply;
pub mod barcode;
pub mod bookconv;
pub mod bookmeta;
pub mod comicbook;
pub mod books;
pub mod comics;
pub mod convert;
pub mod details;
pub mod discogs;
pub mod ebook;
pub mod fields;
pub mod files;
pub mod filter;
pub mod fingerprint;
pub mod identical;
pub mod inside;
pub mod kind;
pub mod links;
pub mod music;
pub mod musicbrainz;
pub(crate) mod pages;
pub mod photos;
pub mod plan;
pub mod playlists;
pub mod rules;
pub mod scan;
pub mod similar;
pub mod subtitles;
pub mod subtext;
pub mod mkvsubs;
pub mod repeated;
pub mod videotags;
pub mod tagwrite;
pub mod text;
pub mod tmdb;
pub mod tracknames;
pub mod tv;
pub mod video;

pub use apply::{ApplyReport, History, HistoryEntry, Operation};
pub use files::{FileEntry, SortKey, sort_files};
pub use filter::Filter;
pub use identical::{HashCache, find_identical};
pub use plan::{Preview, PreviewItem, Problem, Status, View, preview};
pub use rules::{Pipeline, Rule, Step};
pub use scan::{ScanOptions, ScanResult, scan};

/// Makes sure a file just written is on the disk. Windows only flushes a
/// file opened for writing (a read-only handle is "access denied"), so it
/// is opened that way; nothing is truncated or changed.
pub(crate) fn sync_file(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::File::options().write(true).open(path)?.sync_all()
}
