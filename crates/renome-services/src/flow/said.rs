//! What is said of an undo, of a rename put right at start, of each
//! History entry and of each file that failed: the same lines in the
//! window and on the command line (as `after::renamed` for a rename).

use std::path::Path;

use renome_core::apply::{ErrorKind, HistoryEntry, OpError, Recovered};
use renome_core::plan::LeftFolder;
use renome_core::{ApplyReport, Preview, PreviewItem};
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
        ..Line::default()
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
                ..Line::default()
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
    fn totals_count_only_real_conflicts_and_the_filter() {
        let p = Preview {
            total: 5,
            changed: 3,
            conflicts: 2,
            same_there: 1,
            left_folders: vec![LeftFolder {
                from: PathBuf::from("/m/Old"),
                to: PathBuf::from("/m/Old (more)"),
                files: 2,
            }],
            ..Preview::default()
        };
        let keys: Vec<_> = totals(&p).iter().map(|l| (l.key, l.n)).collect();
        // No item shown of the five: the filter's count.
        assert_eq!(
            keys,
            [
                ("shownOf", 5),
                ("changed", 3),
                ("conflicts", 1),
                ("sameThereN", 1),
                ("leftoverCount", 1)
            ]
        );
        let l = &left(&p.left_folders)[0];
        assert_eq!((l.key, l.n), ("confirmLeftoverItem", 2));
        assert_eq!(
            l.more,
            [("from", "Old".into()), ("to", "Old (more)".into())]
        );
    }

    #[test]
    fn rules_left_out_are_named_once() {
        assert!(dropped(&[]).is_none());
        let l = dropped(&["movie".into(), "comic".into(), "movie".into()]).unwrap();
        assert_eq!(
            l.names,
            [("rules", vec!["rule.movie".into(), "rule.comic".into()])]
        );
    }

    #[test]
    fn troubles_tell_warnings_from_errors() {
        let r = super::super::after::Report {
            troubles: vec![super::super::after::Trouble {
                stage: "subtitles",
                done: 2,
                errors: vec![
                    ("a.srt".into(), "subsLengthKept".into()),
                    ("b.srt".into(), "subsDownload: 404".into()),
                    (String::new(), "osQuota".into()),
                ],
            }],
            ..Default::default()
        };
        let t = &troubles(&r)[0];
        assert_eq!(t.title, "subsTitle");
        assert_eq!((t.text[0].key, t.text[0].n), ("subsSavedN", 2));
        assert_eq!(t.warnings.len(), 1);
        let keys: Vec<_> = t.errors.iter().map(|l| l.key).collect();
        assert_eq!(keys, ["troubleFile", "troubleWhy"]);
    }

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

/// The list's totals under it: its files (those the filter shows), the
/// names that change, the real conflicts, the files already where they go
/// and the old folders left with files.
pub fn totals(p: &Preview) -> Vec<Line> {
    let mut out = vec![if p.items.len() == p.total {
        line("files", p.total)
    } else {
        Line {
            key: "shownOf",
            n: p.total,
            more: vec![
                ("shown", p.items.len().to_string()),
                ("total", p.total.to_string()),
            ],
            ..Line::default()
        }
    }];
    out.push(line("changed", p.changed));
    for (key, n) in [
        ("conflicts", p.real_conflicts()),
        ("sameThereN", p.same_there),
        ("leftoverCount", p.left_folders.len()),
    ] {
        if n > 0 {
            out.push(line(key, n));
        }
    }
    out
}

/// Each old folder left with files that do not go to the new one: its
/// name, how many it keeps and the name it gets.
pub fn left(folders: &[LeftFolder]) -> Vec<Line> {
    folders
        .iter()
        .map(|f| Line {
            key: "confirmLeftoverItem",
            n: f.files,
            more: vec![("from", name(&f.from)), ("to", name(&f.to))],
            ..Line::default()
        })
        .collect()
}

/// The rules left out of saved ones (one lookup rule at a time; types
/// not known), each named once; `None` when none was.
pub fn dropped(types: &[String]) -> Option<Line> {
    let mut names: Vec<String> = Vec::new();
    for k in types {
        let key = format!("rule.{k}");
        if !names.contains(&key) {
            names.push(key);
        }
    }
    (!names.is_empty()).then(|| Line {
        key: "lookupsDropped",
        n: names.len(),
        names: vec![("rules", names)],
        ..Line::default()
    })
}

/// A tag a row of the list carries: a few words, and why at more length.
#[derive(Debug, Clone, Serialize)]
pub struct Tag {
    /// Where the window shows it: `name` beside the current name, `new`
    /// beside the new one, `note` as the new name's note.
    pub at: &'static str,
    /// Said joined by ` · `.
    pub text: Vec<Line>,
    /// Said one per line.
    pub tip: Vec<Line>,
}

fn tag(at: &'static str, text: Line, tip: Line) -> Tag {
    Tag {
        at,
        text: vec![text],
        tip: vec![tip],
    }
}

fn with(key: &'static str, n: usize, more: Vec<(&'static str, String)>) -> Line {
    Line {
        key,
        n,
        more,
        ..Line::default()
    }
}

/// What a row of the list is tagged with besides its name (the window's
/// list and the command line's preview alike): left alone by the lookup
/// rule, pages drawn, locked by DRM, a movie's extra, a release's advert,
/// what the subtitle rule does, a name typed by hand, a name Windows does
/// not take (`warn_windows`: when asked to warn).
pub fn tags(i: &PreviewItem, warn_windows: bool) -> Vec<Tag> {
    let mut out = Vec::new();
    if let Some(kind) = i.outside {
        let mut k = kind.chars();
        let cap = k
            .next()
            .map(|c| c.to_uppercase().collect::<String>() + k.as_str())
            .unwrap_or_default();
        let why: &'static str = match cap.as_str() {
            "Movie" => "outsideMovie",
            "Series" => "outsideSeries",
            "Music" => "outsideMusic",
            "Book" => "outsideBook",
            _ => "outsideComic",
        };
        out.push(tag("name", line("outsideRow", 1), line(why, 1)));
    }
    if let Some(n) = i.drawn {
        out.push(tag("name", line("drawnRow", n), line("drawnRowTip", n)));
    }
    if i.drm {
        out.push(tag("name", line("drmRow", 1), line("drmRowTip", 1)));
    }
    if let Some(folder) = &i.extra {
        out.push(tag(
            "name",
            line("extraRow", 1),
            with("extraRowTip", 1, vec![("folder", folder.clone())]),
        ));
    }
    if i.release_junk {
        let (text, tip) = if i.to_trash {
            ("junkRowTrash", "junkRowTrashTip")
        } else {
            ("junkRow", "junkRowTip")
        };
        out.push(tag("name", line(text, 1), line(tip, 1)));
    }
    if let Some(f) = &i.sub_fix {
        if f.no_fps {
            out.push(tag("name", line("subNoFpsRow", 1), line("subNoFpsTip", 1)));
        } else {
            let mut text = Vec::new();
            let mut tip = Vec::new();
            if f.convert {
                text.push(line("subRowSrt", 1));
                tip.push(line("subTipConvert", 1));
            }
            if let Some(enc) = f.encoding {
                text.push(line("subRowUtf8", 1));
                tip.push(with("subTipEncoding", 1, vec![("enc", enc.to_string())]));
            }
            if f.ads > 0 {
                text.push(line("subRowAds", f.ads));
                tip.push(line("subTipAds", f.ads));
            }
            if f.hearing > 0 {
                text.push(line("subRowHearing", f.hearing));
                tip.push(line("subTipHearing", f.hearing));
            }
            if f.retime {
                text.push(line("subRowRetime", 1));
                tip.push(line("subTipRetime", 1));
            }
            if !text.is_empty() {
                out.push(Tag {
                    at: "name",
                    text,
                    tip,
                });
            }
        }
    }
    if i.sub_extract > 0 {
        out.push(tag(
            "name",
            line("subExtractRow", i.sub_extract),
            line("subExtractTip", i.sub_extract),
        ));
    }
    if i.sub_pictures > 0 {
        out.push(tag(
            "name",
            line("subPicturesRow", i.sub_pictures),
            line("subPicturesTip", i.sub_pictures),
        ));
    }
    if i.manual {
        out.push(tag("new", line("manualRow", 1), line("manualRowTip", 1)));
    }
    if i.windows_unsafe && warn_windows {
        out.push(tag(
            "note",
            line("windowsUnsafe", 1),
            line("windowsUnsafe", 1),
        ));
    }
    out
}

/// What is said of one stage that went wrong after the renames: its
/// title, what it did all the same, and each file's reason (`warnings`:
/// done all the same, maybe less well).
#[derive(Debug, Serialize)]
pub struct Trouble {
    pub title: &'static str,
    pub text: Vec<Line>,
    pub errors: Vec<Line>,
    pub warnings: Vec<Line>,
}

/// What is said of each stage of `r` that went wrong.
pub fn troubles(r: &super::after::Report) -> Vec<Trouble> {
    r.troubles
        .iter()
        .map(|x| {
            let count = |key| vec![line(key, x.done)];
            let (title, text) = match x.stage {
                "sidecars" => ("sidecarsTitle", count("sidecarsN")),
                "tags" => ("tagsTitle", count("tagsN")),
                "audioTags" => ("tagsTitle", count("audioTagsN")),
                "inside" => ("insideTitle", count("insideN")),
                "subTools" => (
                    "subToolsTitle",
                    r.lines
                        .iter()
                        .filter(|l| l.key.starts_with("subTools"))
                        .cloned()
                        .collect(),
                ),
                "subtitles" => ("subsTitle", count("subsSavedN")),
                _ => ("resultTitle", Vec::new()),
            };
            let mut errors = Vec::new();
            let mut warnings = Vec::new();
            for (file, why) in &x.errors {
                let said = if file.is_empty() {
                    Line {
                        key: "troubleWhy",
                        errors: vec![("why", why.clone())],
                        ..Line::default()
                    }
                } else {
                    Line {
                        key: "troubleFile",
                        more: vec![("file", file.clone())],
                        errors: vec![("why", why.clone())],
                        ..Line::default()
                    }
                };
                // Saved all the same, maybe out of time.
                if why == "subsLengthKept" {
                    warnings.push(said);
                } else {
                    errors.push(said);
                }
            }
            Trouble {
                title,
                text,
                errors,
                warnings,
            }
        })
        .collect()
}
