//! Renome core.
//!
//! - [`scan`]: collects files from paths (files or folders).
//! - [`filter`]: list filter (which files are shown and will be renamed).
//! - [`rules`]: name transformation rules, applied in sequence.
//! - [`plan`]: computes the preview and checks for conflicts.
//! - [`apply`]: renames in two phases, with a history for undo.

pub mod apply;
pub mod files;
pub mod filter;
pub mod plan;
pub mod rules;
pub mod scan;
pub mod text;

pub use apply::{ApplyReport, History, HistoryEntry, Operation};
pub use files::{FileEntry, SortKey, sort_files};
pub use filter::Filter;
pub use plan::{Preview, PreviewItem, Problem, Status, View, preview};
pub use rules::{Pipeline, Rule, Step};
pub use scan::{ScanOptions, ScanResult, scan};
