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

/// Each album's playlist (`.m3u8`, in disc and track order) in these
/// folders, where none is yet; albums that had one are counted in `kept`.
pub fn playlists(
    history: &History,
    history_id: Option<String>,
    dirs: Vec<PathBuf>,
) -> tmdb::SidecarReport {
    let mut report = tmdb::SidecarReport::default();
    let mut seen = std::collections::HashSet::new();
    let mut kept = std::collections::HashSet::new();
    for dir in dirs {
        let Some((path, text)) = renome_core::playlists::album_playlist(&dir) else {
            if renome_core::playlists::has_playlist(&dir) {
                kept.insert(renome_core::music::album_dir(&dir).to_path_buf());
            }
            continue;
        };
        if !seen.insert(path.clone()) {
            continue;
        }
        match tmdb::write_new(&path, text.as_bytes()) {
            Ok(()) => report.created.push(path),
            Err(e) => report.errors.push((path.display().to_string(), e)),
        }
    }
    report.kept = kept.len();
    record_created(history, history_id, &mut report);
    report
}

/// A song whose tags are written: where it is now and what it is.
#[derive(Deserialize, Clone)]
pub struct TagJob {
    /// Where the song is now (after renaming).
    pub path: PathBuf,
    pub song: renome_core::musicbrainz::SongMatch,
}

/// A cover image in the folder (`cover.`, `folder.`, `front.` JPEG or PNG).
pub fn folder_cover(dir: &Path) -> Option<Vec<u8>> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut found: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            ["cover.", "folder.", "front."]
                .iter()
                .any(|s| n.starts_with(s))
                && (n.ends_with(".jpg") || n.ends_with(".jpeg") || n.ends_with(".png"))
        })
        .collect();
    found.sort();
    std::fs::read(found.first()?).ok()
}

/// Writes each song's release details into its tags (and, with `cover`,
/// the release's front cover in place of its pictures; with `clean`, the
/// other tags go), keeping what they held in the rename's History entry
/// (or one of their own). `mb-progress` reports (kind `tags`). The songs
/// written, for whoever keeps their tags read.
#[allow(clippy::too_many_arguments)]
pub async fn song_tags(
    host: &dyn Host,
    history: &History,
    mb: &crate::musicbrainz::Client,
    dg: &crate::discogs::Client,
    history_id: Option<String>,
    items: Vec<TagJob>,
    cover: bool,
    clean: bool,
    cover_size: Option<u32>,
) -> Result<(TagReport, Vec<PathBuf>), String> {
    use crate::musicbrainz;
    use renome_core::tagwrite;
    let mut out = TagReport::default();
    let items: Vec<TagJob> = items
        .into_iter()
        .filter(|j| {
            let ok = tagwrite::writable(&j.path);
            if !ok {
                out.errors
                    .push((j.path.display().to_string(), "tagsUnsupported".into()));
            }
            ok
        })
        .collect();
    if items.is_empty() {
        return Ok((out, vec![]));
    }
    let own_entry = history_id.is_none();
    let id = match history_id {
        Some(id) => id,
        None => history.record_files(&[])?,
    };
    let store = history.files_dir(&id)?;
    // Each song noted before and after, for a stop halfway.
    let mut note = history.tagging(&id)?;
    // One cover per release: the archive's, else the album folder's own.
    let mut covers: std::collections::HashMap<String, Option<Vec<u8>>> =
        std::collections::HashMap::new();
    let mut full: std::collections::HashMap<String, Option<renome_core::musicbrainz::Release>> =
        std::collections::HashMap::new();
    let total = items.len();
    report(
        host,
        "mb-progress",
        serde_json::json!({ "done": 0, "total": total, "kind": "tags" }),
    );
    let mut backups = Vec::new();
    let mut written = Vec::new();
    for (done, job) in items.into_iter().enumerate() {
        // Typed in: no release to ask (the folder's cover still counts).
        let typed = job.song.typed || job.song.release.is_empty();
        let image = if cover && typed {
            job.path.parent().and_then(folder_cover)
        } else if cover {
            if !covers.contains_key(&job.song.release) {
                let mut found =
                    musicbrainz::front_cover(mb, dg, &job.song.release, cover_size.unwrap_or(1200))
                        .await
                        .ok()
                        .flatten();
                if found.is_none() {
                    found = job.path.parent().and_then(folder_cover);
                }
                covers.insert(job.song.release.clone(), found);
            }
            covers[&job.song.release].clone()
        } else {
            None
        };
        if !typed && !full.contains_key(&job.song.release) {
            let r = if renome_core::discogs::is_discogs(&job.song.release) {
                dg.release(&job.song.release).await.ok()
            } else {
                mb.release_full(&job.song.release).await.ok()
            };
            full.insert(job.song.release.clone(), r);
        }
        let mut fields = tagwrite::fields_of(&job.song);
        if let Some(Some(r)) = full.get(&job.song.release).filter(|_| !typed) {
            // A genre typed in counts over the release's.
            let typed_genre = !job.song.genre.is_empty();
            fields.extend(
                tagwrite::details(r, &job.song.track_id)
                    .into_iter()
                    .filter(|(f, _)| !(typed_genre && *f == tagwrite::Field::Genre)),
            );
        }
        let (path, store) = (job.path.clone(), store.clone());
        note.start(&path);
        let result = tokio::task::spawn_blocking(move || {
            tagwrite::write(&path, &fields, image.as_deref(), clean, &store)
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);
        match result {
            Ok(Some(b)) => {
                note.done(&b);
                backups.push(b);
                written.push(job.path);
            }
            Ok(None) => {}
            Err(e) => out.errors.push((job.path.display().to_string(), e)),
        }
        report(
            host,
            "mb-progress",
            serde_json::json!({ "done": done + 1, "total": total, "kind": "tags" }),
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
        // The note stays: the next start puts the songs in the entry.
        Err(e) => out
            .errors
            .push((String::new(), format!("err.history: {e}"))),
    }
    Ok((out, written))
}
