//! What is said of an undo, of a rename put right at start, of each
//! History entry and of each file that failed: the same lines in the
//! window and on the command line (as `after::renamed` for a rename).

use std::path::Path;

use renome_core::ApplyReport;
use renome_core::apply::{ErrorKind, HistoryEntry, OpError, Recovered};
use serde::Serialize;

use super::after::Line;

fn line(key: &'static str, n: usize) -> Line {
    Line {
        key,
        n,
        ..Line::default()
    }
}

fn name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

/// An undo: the files back under their old names, the copies taken away,
/// whether it was stopped.
pub fn undone(r: &ApplyReport) -> Vec<Line> {
    let restored = r.done.iter().filter(|op| !op.copy).count();
    let mut out = Vec::new();
    if restored > 0 || r.copies_removed == 0 {
        out.push(line("restoredN", restored));
    }
    if r.copies_removed > 0 {
        out.push(line("copiesRemovedN", r.copies_removed));
    }
    if r.stopped {
        out.push(line("undoStopped", 1));
    }
    out
}

/// A file that failed: its name and why.
pub fn failed(e: &OpError) -> Line {
    let kind = serde_json::to_value(e.kind)
        .ok()
        .and_then(|v| v.as_str().map(|k| format!("err.{k}")))
        .unwrap_or_else(|| "err.io".into());
    let file = vec![("file", name(&e.from))];
    let (key, errors) = match e.kind {
        ErrorKind::History => (
            "opErrorParen",
            vec![("why", kind), ("detail", e.detail.clone())],
        ),
        ErrorKind::Io if !e.detail.is_empty() => ("opError", vec![("why", e.detail.clone())]),
        ErrorKind::Tags => (
            "opErrorDetail",
            vec![("why", kind), ("detail", e.detail.clone())],
        ),
        // The temporary name it was left under, to find it.
        ErrorKind::RestoreFailed if !e.detail.is_empty() => (
            "opErrorDetail",
            vec![("why", kind), ("detail", name(Path::new(&e.detail)))],
        ),
        _ => ("opError", vec![("why", kind)]),
    };
    Line {
        key,
        n: 1,
        more: file,
        errors,
    }
}

/// What is said of a rename the program had left halfway, put right at
/// start; `None` when there was nothing.
#[derive(Debug, Serialize)]
pub struct Recovery {
    pub title: &'static str,
    pub lines: Vec<Line>,
    /// Temporary files that could not be put back: where each is, and the
    /// name it should have.
    pub stuck: Vec<(String, String)>,
}

pub fn recovered(r: &Recovered) -> Option<Recovery> {
    let renames = r.restored + r.renamed + r.stuck.len() > 0;
    if !renames && r.undone == 0 && r.tagged == 0 {
        return None;
    }
    let mut lines = Vec::new();
    if renames {
        lines.push(Line {
            key: "recoverText",
            n: r.restored,
            more: vec![
                ("restored", r.restored.to_string()),
                ("renamed", r.renamed.to_string()),
            ],
            ..Line::default()
        });
    }
    if r.undone > 0 {
        lines.push(Line {
            key: "recoverUndone",
            n: r.undone,
            more: vec![("undone", r.undone.to_string())],
            ..Line::default()
        });
    }
    if r.tagged > 0 {
        lines.push(line("recoverTags", r.tagged));
    }
    if !r.stuck.is_empty() {
        lines.push(line("recoverStuck", r.stuck.len()));
    }
    Some(Recovery {
        title: if renames || r.undone > 0 {
            "recoverTitle"
        } else {
            "recoverTagsTitle"
        },
        lines,
        stuck: r
            .stuck
            .iter()
            .map(|s| (s.from.display().to_string(), s.to.display().to_string()))
            .collect(),
    })
}

/// A History entry: what it holds, and how it stands (undone, partly or
/// failed; `None`: can be undone).
#[derive(Debug, Serialize)]
pub struct Entry {
    pub counts: Vec<Line>,
    pub state: Option<Line>,
    /// An undo was tried and did not finish: it can be tried again.
    pub retry: bool,
}

pub fn entry(e: &HistoryEntry) -> Entry {
    let mut counts = Vec::new();
    if e.count > 0 || (e.extra == 0 && e.tagged == 0) {
        counts.push(line("files", e.count));
    }
    if e.count == 0 && e.extra > 0 {
        counts.push(line("metadataN", e.extra));
    }
    if e.tagged > 0 {
        counts.push(line("taggedN", e.tagged));
    }
    let state = if e.undone {
        Some(line("undone", 0))
    } else if e.reverted > 0 {
        Some(Line {
            key: "partlyUndone",
            n: e.reverted,
            more: vec![("total", e.count.to_string())],
            ..Line::default()
        })
    } else if e.failed > 0 {
        Some(line("undoFailed", 0))
    } else {
        None
    };
    Entry {
        counts,
        retry: !e.undone && (e.failed > 0 || e.reverted > 0),
        state,
    }
}

/// What the media servers were told (`mediaserver::tell_all`): those
/// told about some folders, and those that could not be.
pub fn told(answers: &[crate::mediaserver::Told]) -> Vec<Line> {
    answers
        .iter()
        .filter_map(|a| match &a.folders {
            Ok(0) => None,
            Ok(n) => Some(Line {
                key: "serverTold",
                n: *n,
                more: vec![("name", a.name.clone())],
                ..Line::default()
            }),
            Err(e) => Some(Line {
                key: "serverNotToldWhy",
                n: 1,
                more: vec![("name", a.name.clone())],
                errors: vec![("why", e.clone())],
            }),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use renome_core::Operation;
    use std::path::PathBuf;

    #[test]
    fn an_undo_tells_copies_apart() {
        let op = |copy| Operation {
            from: PathBuf::from("/a"),
            to: PathBuf::from("/b"),
            copy,
            within: None,
        };
        let r = ApplyReport {
            done: vec![op(false), op(true)],
            copies_removed: 1,
            stopped: true,
            ..ApplyReport::default()
        };
        let keys: Vec<_> = undone(&r).iter().map(|l| (l.key, l.n)).collect();
        assert_eq!(
            keys,
            [("restoredN", 1), ("copiesRemovedN", 1), ("undoStopped", 1)]
        );
    }

    #[test]
    fn stuck_files_are_said() {
        let r = Recovered {
            stuck: vec![Operation {
                from: PathBuf::from("/x/.tmp"),
                to: PathBuf::from("/x/a"),
                copy: false,
                within: None,
            }],
            ..Recovered::default()
        };
        let said = recovered(&r).unwrap();
        assert_eq!(said.title, "recoverTitle");
        assert!(said.lines.iter().any(|l| l.key == "recoverStuck"));
        assert_eq!(said.stuck.len(), 1);
    }
}
