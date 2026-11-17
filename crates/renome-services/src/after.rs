//! What runs after the renames, for the window and the command line
//! alike: files made beside the renamed ones (recorded in the History, so
//! undo deletes them), details written inside videos (recorded with what
//! they replaced, so undo puts it back).

use std::path::{Path, PathBuf};

use renome_core::History;
use serde::{Deserialize, Serialize};

use crate::{Host, report, tmdb};

/// Files made along with an entry of the History (or in one of their
/// own, when nothing was renamed): undo deletes them.
pub fn record_created(
    history: &History,
    history_id: Option<String>,
    report: &mut tmdb::SidecarReport,
) {
    if report.created.is_empty() {
        return;
    }
    let saved = match history_id {
        Some(id) => history.add_created(&id, &report.created),
        None => history.record_files(&report.created).map(|_| ()),
    };
    if let Err(e) = saved {
        report
            .errors
            .push((String::new(), format!("err.history: {e}")));
    }
}

/// The movies' `.nfo` and images beside their renamed videos.
#[allow(clippy::too_many_arguments)]
pub async fn movie_sidecars(
    host: &dyn Host,
    history: &History,
    key: &str,
    lang: &str,
    history_id: Option<String>,
    items: Vec<tmdb::SidecarItem>,
    backdrop: bool,
    in_folder: bool,
) -> tmdb::SidecarReport {
    let client = tmdb::Client::new(key, lang);
    let mut report = tmdb::sidecars(host, &client, items, backdrop, in_folder).await;
    record_created(history, history_id, &mut report);
    report
}

/// Files whose details were written inside them, and what went wrong.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TagReport {
    pub written: usize,
    /// PDF pages drawn while converting comics.
    pub drawn: usize,
    /// File name and reason.
    pub errors: Vec<(String, String)>,
}

#[derive(Deserialize, Clone, Copy, Default)]
pub struct VideoTagOptions {
    #[serde(default)]
    pub cover: bool,
    #[serde(default)]
    pub clean: bool,
    #[serde(default)]
    pub tracks: bool,
}

/// Title, date, cover and the rest of TMDB's details written inside MKV
/// and MP4 videos (with `clean`, what they had taken out first; with
/// `tracks`, their tracks named by language), each backed up in the
/// History. `inside-progress` reports.
#[allow(clippy::too_many_arguments)]
pub async fn video_tags(
    host: &dyn Host,
    history: &History,
    key: &str,
    lang: &str,
    history_id: Option<String>,
    items: Vec<tmdb::VideoTagItem>,
    options: VideoTagOptions,
) -> Result<TagReport, String> {
    use renome_core::videotags;
    let opt = videotags::Options {
        clean: options.clean,
        tracks: options.tracks,
    };
    let mut out = TagReport::default();
    let name = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let items: Vec<tmdb::VideoTagItem> = items
        .into_iter()
        .filter(|i| {
            let ok = videotags::writable(&i.path);
            if !ok {
                out.errors.push((name(&i.path), "videoUnsupported".into()));
            }
            ok
        })
        .collect();
    if items.is_empty() {
        return Ok(out);
    }
    let total = items.len();
    report(
        host,
        "inside-progress",
        serde_json::json!({ "done": 0, "total": total }),
    );
    let client = tmdb::Client::new(key, lang);
    let metas = tmdb::video_meta(&client, &items, options.cover).await;
    let own_entry = history_id.is_none();
    let id = match history_id {
        Some(id) => id,
        None => history.record_files(&[])?,
    };
    let store = history.files_dir(&id)?;
    let mut note = history.tagging(&id)?;
    let stop = host.stop_signal();
    let mut backups = Vec::new();
    for (done, (item, meta)) in items.into_iter().zip(metas).enumerate() {
        if stop() {
            break;
        }
        let meta = match meta {
            Ok(m) => m,
            Err(e) => {
                out.errors.push((name(&item.path), e));
                continue;
            }
        };
        note.start(&item.path);
        let (path, store): (PathBuf, PathBuf) = (item.path.clone(), store.clone());
        let r = tokio::task::spawn_blocking(move || videotags::write(&path, &meta, &opt, &store))
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r);
        match r {
            Ok(Some(edit)) => {
                let b = renome_core::tagwrite::Backup {
                    video: Some(edit),
                    ..renome_core::tagwrite::Backup::empty(&item.path)
                };
                note.done(&b);
                backups.push(b);
            }
            Ok(None) => {}
            Err(e) => out.errors.push((name(&item.path), e)),
        }
        report(
            host,
            "inside-progress",
            serde_json::json!({ "done": done + 1, "total": total }),
        );
    }
    out.written = backups.len();
    let saved = if backups.is_empty() {
        if own_entry {
            let _ = history.delete(&id);
        }
        Ok(())
    } else {
        history.add_tags(&id, backups)
    };
    match saved {
        Ok(()) => note.finish(),
        Err(e) => out
            .errors
            .push((String::new(), format!("err.history: {e}"))),
    }
    Ok(out)
}
