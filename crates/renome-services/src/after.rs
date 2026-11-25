//! What runs after the renames, for the window and the command line
//! alike: files made beside the renamed ones (recorded in the History, so
//! undo deletes them), details written inside videos (recorded with what
//! they replaced, so undo puts it back).

use renome_core::sync::MutexExt;
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
        match renome_core::files::write_new(&path, text.as_bytes()) {
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

/// What was written inside files, which files they are now, and the
/// files given their own extension back (a conversion that failed): the
/// window follows them in its list.
pub struct InsideOutcome {
    pub report: TagReport,
    pub written: Vec<PathBuf>,
    pub moved: Vec<renome_core::apply::Operation>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsideJob {
    /// Where the file is now (after renaming).
    pub path: PathBuf,
    #[serde(default)]
    pub comic: Option<renome_core::comics::ComicMatch>,
    /// The issue's page in the catalogue.
    #[serde(default)]
    pub web: String,
    #[serde(default)]
    pub manga: bool,
    #[serde(default)]
    pub book: Option<renome_core::books::BookMatch>,
    /// The book's language as Open Library gives it.
    #[serde(default)]
    pub language: String,
    /// The comic's `ComicInfo.xml` is cleared: written with only `comic`'s
    /// details, or (without `comic`) taken out.
    #[serde(default)]
    pub fresh: bool,
    /// Converts the comic to CBZ first (with `comic`'s details in it, when
    /// given), its original kept beside or sent to the trash.
    #[serde(default)]
    pub convert: Option<renome_core::convert::Original>,
    /// How PDFs are converted (whether pages may be drawn, at what size,
    /// whether their own title and author go in).
    #[serde(default)]
    pub pdf: renome_core::convert::PdfOptions,
    /// Converts the book to this format (with `book`'s details in it, when
    /// given), its original kept beside or sent to the trash (`convert`).
    #[serde(default)]
    pub book_to: Option<renome_core::bookconv::Target>,
    /// A book printed to PDF on pages the size of a reader's screen
    /// (width, height in mm); A5 without.
    #[serde(default)]
    pub pdf_page: Option<(f64, f64)>,
    /// An EPUB polished (EPUB 3, smaller pictures), with `book`'s details
    /// written in the same pass when given.
    #[serde(default)]
    pub polish: Option<renome_core::bookmeta::Polish>,
    /// A comic made into an e-book in this format (with `comic`'s details
    /// when given), its original kept beside or sent to the trash
    /// (`convert`).
    #[serde(default)]
    pub comic_to: Option<renome_core::comicbook::Format>,
    #[serde(default)]
    pub ebook: renome_core::comicbook::EbookOptions,
}

/// The comics to join into volumes: those made e-books with "join", of
/// one series (and volume) in one folder, two or more of them; the rest
/// go on alone.
pub(crate) fn volume_groups(items: Vec<InsideJob>) -> (Vec<InsideJob>, Vec<Vec<InsideJob>>) {
    let mut alone = Vec::new();
    let mut groups: Vec<((PathBuf, String, String), Vec<InsideJob>)> = Vec::new();
    for mut j in items {
        if j.comic_to.is_some() && j.ebook.join {
            // Not looked up: what its name (and its ComicInfo.xml) says.
            if j.comic.is_none() {
                j.comic = renome_core::comics::from_path(&j.path, None, false);
            }
            let values = j
                .comic
                .as_ref()
                .map(|c| renome_core::inside::comic_values(c, &j.web, j.manga))
                .unwrap_or_default();
            let get = |k: &str| {
                values
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            };
            let series = get("Series").to_lowercase();
            if !series.is_empty() {
                let key = (
                    j.path.parent().map(Path::to_path_buf).unwrap_or_default(),
                    series,
                    get("Volume"),
                );
                match groups.iter_mut().find(|g| g.0 == key) {
                    Some(g) => g.1.push(j),
                    None => groups.push((key, vec![j])),
                }
                continue;
            }
        }
        alone.push(j);
    }
    let mut volumes = Vec::new();
    for (_, g) in groups {
        if g.len() < 2 {
            alone.extend(g);
        } else {
            volumes.push(g);
        }
    }
    (alone, volumes)
}

/// Joins one group of comics into a volume named `Series #1-5`: each
/// comic first takes back its own extension (renamed by the template, in
/// its own format, as the history then says), then the volume is made
/// beside them, a new file (with its parts, when too large) that undo
/// deletes. Errors: the file and why.
async fn join_volume(
    history: &History,
    id: &str,
    group: Vec<InsideJob>,
    moved: &mut Vec<renome_core::apply::Operation>,
) -> Result<Vec<PathBuf>, (String, String)> {
    use renome_core::{comicbook, convert};
    let first = &group[0];
    let (to, o, manga, pdf) = (
        first.comic_to.unwrap_or(comicbook::Format::Epub),
        first.ebook,
        first.manga,
        first.pdf,
    );
    let dir = first
        .path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let mut issues: Vec<(String, comicbook::Issue)> = Vec::new();
    for j in &group {
        let fail = |e: &str| (j.path.display().to_string(), e.to_string());
        let kind = convert::kind(&j.path).ok_or_else(|| fail("comicUnsupported"))?;
        let back = comicbook::original_of(&j.path, kind);
        if back == j.path || std::fs::symlink_metadata(&back).is_ok() {
            return Err(fail("convertTaken"));
        }
        // The history's rename moves on to its own extension (renaming
        // the file too), so undo finds it.
        if history.retarget(id, &j.path, &back) != Ok(true) {
            return Err(fail("convertTaken"));
        }
        moved.push(renome_core::apply::Operation {
            from: j.path.clone(),
            to: back.clone(),
            copy: false,
            within: None,
        });
        let values = j
            .comic
            .as_ref()
            .map(|c| renome_core::inside::comic_values(c, &j.web, j.manga))
            .unwrap_or_default();
        let number = values
            .iter()
            .find(|(k, _)| *k == "Number")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        issues.push((number, comicbook::Issue { path: back, values }));
    }
    // In order of their numbers (`1`, `2`, `10`, `10.5`), else of names.
    issues.sort_by(|a, b| match (a.0.parse::<f64>(), b.0.parse::<f64>()) {
        (Ok(x), Ok(y)) => x.total_cmp(&y),
        _ => a.1.path.cmp(&b.1.path),
    });
    let series = issues[0]
        .1
        .values
        .iter()
        .find(|(k, _)| *k == "Series")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    let (lo, hi) = (&issues[0].0, &issues[issues.len() - 1].0);
    let title = if lo.is_empty() || hi.is_empty() {
        series.clone()
    } else {
        format!("{series} #{lo}-{hi}")
    };
    let out = dir.join(format!(
        "{}.{}",
        renome_core::tmdb::safe_title(&title, " - "),
        to.ext()
    ));
    let issues: Vec<comicbook::Issue> = issues.into_iter().map(|(_, i)| i).collect();
    let shown = out.display().to_string();
    tokio::task::spawn_blocking(move || comicbook::join(&issues, to, o, manga, pdf, &out, &title))
        .await
        .map_err(|e| (shown.clone(), e.to_string()))?
        .map(|j| j.files)
        .map_err(|e| (shown, e))
}

/// Makes the book of `job` (renamed `.pdf`) into a PDF: its page is
/// written, printed by the system's web view, and takes the book's place.
async fn print_book(
    host: &dyn Host,
    job: InsideJob,
) -> Result<(Option<renome_core::tagwrite::Backup>, Option<String>), String> {
    use renome_core::bookconv::print;
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let work = std::env::temp_dir().join(format!("renome-print-{}-{n}", std::process::id()));
    let path = job.path.clone();
    let values = job
        .book
        .as_ref()
        .map(|b| renome_core::inside::book_values(b, &job.language));
    let original = job.convert.unwrap_or(renome_core::convert::Original::Keep);
    let page = job
        .pdf_page
        .filter(|&(w, h)| (20.0..=1000.0).contains(&w) && (20.0..=1000.0).contains(&h))
        .map_or(print::Page::A5, |(w, h)| print::Page::screen(w, h));
    let prep = {
        let (path, work) = (path.clone(), work.clone());
        tokio::task::spawn_blocking(move || print::prepare(&path, values.as_ref(), &work, page))
            .await
            .map_err(|e| e.to_string())??
    };
    if let Err(e) = host
        .print_pdf(prep.html.clone(), prep.pdf.clone(), page)
        .await
    {
        let _ = std::fs::remove_dir_all(&prep.dir);
        let _ = std::fs::remove_file(&prep.pdf);
        return Err(e);
    }
    let done = tokio::task::spawn_blocking(move || print::finish(&path, &prep, original))
        .await
        .map_err(|e| e.to_string())??;
    Ok((Some(done.backup), done.not_trashed))
}

/// Writes each comic's details into its `ComicInfo.xml` (CBZ) and each
/// book's into its OPF (EPUB), keeping what they held in the rename's
/// history entry (or one of its own) for undo, like `write_tags`. Comics
/// to convert (CBR, CB7, CBT renamed `.cbz`) are converted, the details
/// written in the same pass.
pub async fn inside(
    host: &dyn Host,
    history: &History,
    history_id: Option<String>,
    items: Vec<InsideJob>,
) -> Result<InsideOutcome, String> {
    let mut report = TagReport::default();
    let items = writable_jobs(items, &mut report);
    if items.is_empty() {
        return Ok(InsideOutcome {
            report,
            written: vec![],
            moved: vec![],
        });
    }
    let mut moved_ops = Vec::new();
    let own_entry = history_id.is_none();
    let id = match history_id {
        Some(id) => id,
        None => history.record_files(&[])?,
    };
    let store = history.files_dir(&id)?;
    let mut note = history.tagging(&id)?;
    let (items, volumes) = volume_groups(items);
    let total = items.len() + volumes.len();
    inside_progress(host, 0, total);
    // Files made besides the renamed ones (a book's other parts, joined
    // volumes): undo deletes them.
    let made = std::sync::Arc::new(std::sync::Mutex::new(Vec::<PathBuf>::new()));
    for (n, group) in volumes.into_iter().enumerate() {
        match join_volume(history, &id, group, &mut moved_ops).await {
            Ok(files) => {
                report.written += 1;
                made.locked().extend(files);
            }
            Err((file, e)) => report.errors.push((file, e)),
        }
        inside_progress(host, n + 1, total);
    }
    let joined = total - items.len();
    let mut backups = Vec::new();
    let mut written = Vec::new();
    let pages = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for (done, job) in items.into_iter().enumerate() {
        let done = done + joined;
        let job_path = job.path.clone();
        let how = Conversion::of(&job);
        note.start(&job_path);
        let result = run_job(host, job, &store, &made, &pages).await;
        let result = result.map(|(b, not_trashed)| {
            if let Some(why) = not_trashed {
                report.errors.push((job_path.display().to_string(), why));
            }
            b
        });
        match result {
            Ok(Some(b)) => {
                note.done(&b);
                written.push(b.path.clone());
                backups.push(b);
            }
            Ok(None) => {}
            Err(e) => {
                give_name_back(history, &id, &job_path, how, &mut moved_ops);
                report.errors.push((job_path.display().to_string(), e));
            }
        }
        inside_progress(host, done + 1, total);
    }
    report.written += backups.len();
    report.drawn = pages.load(std::sync::atomic::Ordering::Relaxed);
    let made = std::mem::take(&mut *made.locked());
    if !made.is_empty()
        && let Err(e) = history.add_created(&id, &made)
    {
        report
            .errors
            .push((String::new(), format!("err.history: {e}")));
    }
    let saved = if backups.is_empty() {
        if own_entry && made.is_empty() {
            let _ = history.delete(&id);
        } else {
            // Comics that could not be converted may have taken back the
            // only renames.
            let _ = history.drop_if_empty(&id);
        }
        Ok(())
    } else {
        history.add_tags(&id, backups)
    };
    match saved {
        Ok(()) => note.finish(),
        Err(e) => report
            .errors
            .push((String::new(), format!("err.history: {e}"))),
    }
    Ok(InsideOutcome {
        report,
        written,
        moved: moved_ops,
    })
}

fn inside_progress(host: &dyn Host, done: usize, total: usize) {
    crate::report(
        host,
        "inside-progress",
        serde_json::json!({ "done": done, "total": total }),
    );
}

/// The jobs whose file can take what they bring; the others are reported
/// (when they asked for something) as unsupported.
fn writable_jobs(items: Vec<InsideJob>, report: &mut TagReport) -> Vec<InsideJob> {
    use renome_core::{convert, inside};
    items
        .into_iter()
        .map(|mut j| {
            // Already a zip (or not an archive): only its details.
            if j.book_to.is_none()
                && j.comic_to.is_none()
                && j.convert.is_some()
                && !convert::convertible(&j.path)
            {
                j.convert = None;
            }
            j
        })
        .filter(|j| {
            let ok = match (&j.convert, &j.comic, &j.book) {
                _ if j.book_to.is_some() || j.comic_to.is_some() => true,
                _ if j.polish.is_some() => inside::book_writable(&j.path),
                (Some(_), _, _) => true,
                (_, Some(_), _) => inside::comic_writable(&j.path),
                (_, _, None) if j.fresh => inside::comic_writable(&j.path),
                (_, _, Some(_)) => {
                    inside::book_writable(&j.path) || renome_core::bookmeta::writable(&j.path)
                }
                _ => false,
            };
            if !ok && (j.comic.is_some() || j.book.is_some() || j.fresh || j.polish.is_some()) {
                let why = if j.comic.is_some() {
                    "comicUnsupported"
                } else {
                    "bookUnsupported"
                };
                report
                    .errors
                    .push((j.path.display().to_string(), why.into()));
            }
            ok
        })
        .collect()
}

/// What a job was asked to convert, kept for when it fails: its name is
/// then given back.
#[derive(Clone, Copy)]
struct Conversion {
    /// Comic to an e-book.
    comic_ebook: bool,
    /// Book to another format.
    book: bool,
    /// Comic archive to CBZ.
    comic_cbz: bool,
}

impl Conversion {
    fn of(job: &InsideJob) -> Self {
        Self {
            comic_ebook: job.comic_to.is_some(),
            book: job.book_to.is_some(),
            comic_cbz: job.convert.is_some() && job.book_to.is_none() && job.comic_to.is_none(),
        }
    }

    /// The name a file was given for a conversion that did not happen: its
    /// own extension.
    fn own_name(self, path: &Path) -> Option<PathBuf> {
        use renome_core::{bookconv, comicbook, convert};
        if self.comic_ebook {
            convert::kind(path).map(|k| comicbook::original_of(path, k))
        } else if self.book {
            bookconv::beside(path).filter(|b| b != path)
        } else if self.comic_cbz {
            convert::kind(path)
                .filter(|k| *k != convert::Kind::Zip)
                .map(|k| convert::beside(path, k))
        } else {
            None
        }
    }
}

/// Named `.cbz` (or `.epub`…) to be converted, a file that could not be
/// gets its own extension back, in the history too.
fn give_name_back(
    history: &History,
    id: &str,
    path: &Path,
    how: Conversion,
    moved: &mut Vec<renome_core::apply::Operation>,
) {
    if let Some(back) = how.own_name(path)
        && std::fs::symlink_metadata(&back).is_err()
        && history.retarget(id, path, &back) == Ok(true)
    {
        moved.push(renome_core::apply::Operation {
            from: path.to_path_buf(),
            to: back,
            copy: false,
            within: None,
        });
    }
}

/// What one job made of its file: the backup of what it replaced, and a
/// reason the original could not be sent to the trash.
type Done = (Option<renome_core::tagwrite::Backup>, Option<String>);

/// Does one job: a book made into PDF is printed by the window's web
/// view, the rest is work for a thread.
async fn run_job(
    host: &dyn Host,
    job: InsideJob,
    store: &Path,
    made: &std::sync::Arc<std::sync::Mutex<Vec<PathBuf>>>,
    pages: &std::sync::Arc<std::sync::atomic::AtomicUsize>,
) -> Result<Done, String> {
    if job.book_to == Some(renome_core::bookconv::Target::Pdf) {
        return print_book(host, job).await;
    }
    let (store, made, pages) = (store.to_path_buf(), made.clone(), pages.clone());
    tokio::task::spawn_blocking(move || work_on(&job, &store, &made, &pages))
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r)
}

/// The comic's details as `ComicInfo.xml` values.
fn comic_values(job: &InsideJob) -> Vec<(&'static str, String)> {
    job.comic
        .as_ref()
        .map(|c| renome_core::inside::comic_values(c, &job.web, job.manga))
        .unwrap_or_default()
}

/// The book's details as OPF values.
fn book_values(job: &InsideJob) -> Option<renome_core::inside::BookValues> {
    job.book
        .as_ref()
        .map(|b| renome_core::inside::book_values(b, &job.language))
}

fn work_on(
    job: &InsideJob,
    store: &Path,
    made: &std::sync::Mutex<Vec<PathBuf>>,
    pages: &std::sync::atomic::AtomicUsize,
) -> Result<Done, String> {
    if let Some(to) = job.comic_to {
        return comic_to_ebook(job, to, store, made);
    }
    if let Some(p) = job.polish {
        let values = book_values(job);
        return renome_core::bookmeta::polish(&job.path, p, values.as_ref(), store)
            .map(|d| (d.backup, None));
    }
    if let Some(to) = job.book_to {
        let values = book_values(job);
        let original = job.convert.unwrap_or(renome_core::convert::Original::Keep);
        return renome_core::bookconv::convert(&job.path, to, original, values.as_ref())
            .map(|d| (Some(d.backup), d.not_trashed));
    }
    details_in_place(job, store, pages)
}

/// A comic made into an e-book, its details in it.
fn comic_to_ebook(
    job: &InsideJob,
    to: renome_core::comicbook::Format,
    store: &Path,
    made: &std::sync::Mutex<Vec<PathBuf>>,
) -> Result<Done, String> {
    use renome_core::comicbook;
    let original = job.convert.unwrap_or(renome_core::convert::Original::Keep);
    // An e-book needs a language (a Kindle refuses one without); pictures
    // have none: the system's, unless the comic says its own.
    let mut v = comic_values(job);
    if matches!(
        to,
        comicbook::Format::Epub | comicbook::Format::Kepub | comicbook::Format::Azw3
    ) {
        v.push((comicbook::FALLBACK_LANGUAGE, crate::system_language()));
    }
    comicbook::convert(
        &job.path, to, job.ebook, &v, job.manga, original, job.pdf, store,
    )
    .map(|d| {
        made.locked().extend(d.parts);
        (Some(d.backup), d.not_trashed)
    })
}

/// The details written into the file itself (a comic converted to CBZ on
/// the way, a book's OPF, a `ComicInfo.xml` taken out).
fn details_in_place(
    job: &InsideJob,
    store: &Path,
    pages: &std::sync::atomic::AtomicUsize,
) -> Result<Done, String> {
    use renome_core::{convert, inside};
    let path = &job.path;
    match (job.convert, &job.comic, &job.book) {
        (Some(original), _, _) => {
            convert::convert_with(path, &comic_values(job), original, job.pdf).map(|d| {
                pages.fetch_add(d.drawn_pages, std::sync::atomic::Ordering::Relaxed);
                (Some(d.backup), d.not_trashed)
            })
        }
        (_, Some(_), _) => {
            inside::write_comic_with(path, &comic_values(job), job.fresh, store).map(|b| (b, None))
        }
        (_, _, Some(b)) => {
            let v = inside::book_values(b, &job.language);
            if inside::book_writable(path) {
                inside::write_book(path, &v, store).map(|b| (b, None))
            } else {
                renome_core::bookmeta::write(path, &v, store).map(|b| (b, None))
            }
        }
        _ if job.fresh => inside::remove_comic(path, store).map(|b| (b, None)),
        _ => Ok((None, None)),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookTagJob {
    pub path: PathBuf,
    pub book: renome_core::books::BookMatch,
    pub cover: Option<i64>,
    /// A cover elsewhere (Google Books), when there is no `cover`.
    #[serde(default)]
    pub cover_url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub language: String,
}

/// Writes each audiobook file's details (book, author, narrator, part,
/// series, synopsis) into its tags and, with `cover`, the book's cover in
/// place of its pictures; with `clean`, the other tags go. Kept for undo
/// like `write_tags`. Each cover and synopsis is fetched once per book.
pub async fn audiobook_tags(
    host: &dyn Host,
    history: &History,
    ol: &crate::openlibrary::Client,
    history_id: Option<String>,
    items: Vec<AudiobookTagJob>,
    cover: bool,
    clean: bool,
) -> Result<(TagReport, Vec<PathBuf>), String> {
    use renome_core::tagwrite;
    let mut report = TagReport::default();
    let items: Vec<AudiobookTagJob> = items
        .into_iter()
        .filter(|j| {
            let ok = tagwrite::writable(&j.path);
            if !ok {
                report
                    .errors
                    .push((j.path.display().to_string(), "tagsUnsupported".into()));
            }
            ok
        })
        .collect();
    if items.is_empty() {
        return Ok((report, vec![]));
    }
    let own_entry = history_id.is_none();
    let id = match history_id {
        Some(id) => id,
        None => history.record_files(&[])?,
    };
    let store = history.files_dir(&id)?;
    let mut note = history.tagging(&id)?;
    let mut covers: std::collections::HashMap<String, Option<Vec<u8>>> =
        std::collections::HashMap::new();
    let mut synopses: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let total = items.len();
    crate::report(
        host,
        "book-progress",
        serde_json::json!({ "done": 0, "total": total }),
    );
    let mut backups = Vec::new();
    let mut written = Vec::new();
    for (done, job) in items.into_iter().enumerate() {
        let image = if cover {
            let key = match job.cover {
                Some(c) => format!("ol:{c}"),
                None if !job.cover_url.is_empty() => job.cover_url.clone(),
                // None: the book's folder's own cover, below.
                None => String::new(),
            };
            if !key.is_empty() && !covers.contains_key(&key) {
                let got = match job.cover {
                    Some(c) => ol.cover(c).await,
                    None => {
                        ol.gb
                            .cover(&renome_core::books::google_cover(&job.cover_url, 0))
                            .await
                    }
                };
                covers.insert(key.clone(), got.ok());
            }
            covers
                .get(&key)
                .cloned()
                .flatten()
                .or_else(|| job.path.parent().and_then(folder_cover))
                .and_then(|b| tagwrite::embeddable_cover(&b, 1200))
        } else {
            None
        };
        let mut description = job.description.trim().to_string();
        if description.is_empty() && !job.book.work.is_empty() {
            if !synopses.contains_key(&job.book.work) {
                let text = ol
                    .work(&job.book.work)
                    .await
                    .ok()
                    .and_then(|w| w.description)
                    .map(|t| t.as_str().trim().to_string())
                    .unwrap_or_default();
                synopses.insert(job.book.work.clone(), text);
            }
            description = synopses[&job.book.work].clone();
        }
        let fields = tagwrite::audiobook_fields(&job.book, &description, &job.language);
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
            Err(e) => report.errors.push((job.path.display().to_string(), e)),
        }
        crate::report(
            host,
            "book-progress",
            serde_json::json!({ "done": done + 1, "total": total }),
        );
    }
    report.written = backups.len();
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
        Err(e) => report
            .errors
            .push((String::new(), format!("err.history: {e}"))),
    }
    Ok((report, written))
}

#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubToolsReport {
    /// Subtitles changed.
    pub written: usize,
    /// Subtitles taken out of videos.
    pub created: usize,
    /// File name and reason.
    pub errors: Vec<(String, String)>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubJob {
    /// Where the file is now (after renaming).
    pub path: PathBuf,
    /// A subtitle the tools change (encoding, cleaning, times, SubRip).
    #[serde(default)]
    pub fix: bool,
    /// A video whose text tracks are taken out.
    #[serde(default)]
    pub extract: bool,
}

/// The subtitle rule's work after the rename: subtitles changed where they
/// are (their old content kept in the history entry for undo) or made
/// into SubRip (their original kept beside or sent to the trash), and the
/// text tracks of videos written beside them (deleted by undo). `stop`
/// ends it between files (and while a video is read).
pub async fn subtitle_tools(
    host: &dyn Host,
    history: &History,
    history_id: Option<String>,
    items: Vec<SubJob>,
    tools: renome_core::subtext::Tools,
    original: renome_core::convert::Original,
    extract_langs: String,
) -> Result<(SubToolsReport, Vec<PathBuf>), String> {
    use renome_core::{mkvsubs, subtext};
    let mut report = SubToolsReport::default();
    let items: Vec<SubJob> = items.into_iter().filter(|j| j.fix || j.extract).collect();
    if items.is_empty() {
        return Ok((report, vec![]));
    }
    let own_entry = history_id.is_none();
    let id = match history_id {
        Some(id) => id,
        None => history.record_files(&[])?,
    };
    let store = history.files_dir(&id)?;
    let mut note = history.tagging(&id)?;
    let langs = std::sync::Arc::new(renome_core::subtitles::langs(&extract_langs));
    let stop = host.stop_signal();
    let total = items.len();
    let send = host.sender();
    let progress = |done: usize, part: f64| {
        send(
            "subtools-progress",
            serde_json::json!({ "done": done, "total": total, "part": part }),
        );
    };
    progress(0, 0.0);
    let mut backups = Vec::new();
    let mut created = Vec::new();
    let mut changed = Vec::new();
    for (done, job) in items.into_iter().enumerate() {
        if stop() {
            break;
        }
        let name = job
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if job.fix {
            note.start(&job.path);
            let (path, store) = (job.path.clone(), store.clone());
            let r = tokio::task::spawn_blocking(move || {
                subtext::fix_file(&path, &tools, original, &store)
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r);
            match r {
                Ok(f) => {
                    if let Some(why) = f.not_trashed {
                        report.errors.push((name.clone(), why));
                    }
                    if let Some(b) = f.backup {
                        note.done(&b);
                        changed.push(b.path.clone());
                        backups.push(b);
                    }
                }
                Err(e) => report.errors.push((name.clone(), e)),
            }
        }
        if job.extract {
            let (path, langs, stop) = (job.path.clone(), langs.clone(), stop.clone());
            let send = send.clone();
            let r = tokio::task::spawn_blocking(move || {
                let tell = |at: u64, size: u64| {
                    let part = if size > 0 {
                        at as f64 / size as f64
                    } else {
                        0.0
                    };
                    send(
                        "subtools-progress",
                        serde_json::json!({ "done": done, "total": total, "part": part }),
                    );
                };
                mkvsubs::extract(&path, &tools, &langs, &|| stop(), &tell)
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r);
            match r {
                Ok(x) => {
                    created.extend(x.created);
                    report.errors.extend(x.errors);
                }
                Err(e) if e == "stopped" => break,
                Err(e) => report.errors.push((name, e)),
            }
        }
        progress(done + 1, 0.0);
    }
    report.written = backups.len();
    report.created = created.len();
    let mut saved = Ok(());
    if !backups.is_empty() {
        saved = history.add_tags(&id, backups);
    }
    if saved.is_ok() && !created.is_empty() {
        saved = history.add_created(&id, &created);
    }
    if report.written == 0 && report.created == 0 && own_entry {
        let _ = history.delete(&id);
    }
    match saved {
        Ok(()) => note.finish(),
        Err(e) => report
            .errors
            .push((String::new(), format!("err.history: {e}"))),
    }
    Ok((report, changed))
}
