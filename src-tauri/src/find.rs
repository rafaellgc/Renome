//! The window's commands: find.

use super::*;

#[derive(Clone, Serialize)]
pub(crate) struct IdenticalProgress {
    pub(crate) done: u64,
    pub(crate) total: u64,
}

/// What the search for copies found: identical files, and songs that are
/// the same music in different files.
#[derive(Serialize)]
pub(crate) struct Copies {
    pub(crate) identical: Vec<Vec<PathBuf>>,
    pub(crate) songs: Vec<Vec<PathBuf>>,
    /// The same movie, episode, comic or book in several files.
    pub(crate) works: Vec<renome_core::repeated::Group>,
}

/// Searches the list for files with identical content, then for songs
/// that are the same music, in the background. Returns the groups, or
/// `None` if a newer search replaced this one. Emits `identical-progress`
/// while reading whole files.
#[tauri::command]
pub(crate) async fn find_identical(app: AppHandle) -> Result<Option<Copies>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let run = state.identical_run.fetch_add(1, Ordering::SeqCst) + 1;
        let files = state.session.locked().files.clone();
        let stale = || {
            drop(state.busy_disk.lock());
            state.identical_run.load(Ordering::SeqCst) != run
        };
        let mut hashes = state.hashes.locked();
        let mut last = Instant::now();
        let groups = renome_core::find_identical(
            &files,
            &mut hashes,
            &mut |done, total| {
                if last.elapsed() > Duration::from_millis(150) || done == total {
                    last = Instant::now();
                    let _ = app.emit("identical-progress", IdenticalProgress { done, total });
                }
            },
            &stale,
        )?;
        if stale() {
            return None;
        }
        drop(hashes);
        use renome_core::repeated::by_group;
        let identical = by_group(&groups);
        // The songs' tags are read on the copy, without holding the list
        // (the command line's `duplicates` finds them the same way).
        let (songs, works) = renome_core::repeated::repeated(&files, &identical, &stale)?;
        if stale() {
            return None;
        }
        let mut s = state.session.locked();
        s.identical = identical;
        // One filter for both: songs' groups first, then the works'.
        let all: Vec<Vec<PathBuf>> = songs
            .iter()
            .cloned()
            .chain(
                works
                    .iter()
                    .map(|g| g.items.iter().map(|i| i.path.clone()).collect()),
            )
            .collect();
        s.alike = by_group(&all);
        let read: HashMap<&PathBuf, &FileEntry> = files.iter().map(|f| (&f.path, f)).collect();
        for f in s.files.iter_mut() {
            if let Some(other) = read.get(&f.path) {
                f.keep_read(other);
            }
        }
        Some(Copies {
            identical: groups,
            songs,
            works,
        })
    })
    .await
    .map_err(|e| e.to_string())
}

/// Searches the list for pictures that look alike, in the background,
/// after [`find_identical`] (a newer search for copies stops it). Returns
/// the groups, or `None` if it was stopped. Emits `similar-progress`
/// while reading pictures not seen before.
#[tauri::command]
pub(crate) async fn find_similar(app: AppHandle) -> Result<Option<Vec<Vec<PathBuf>>>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let run = state.identical_run.load(Ordering::SeqCst);
        // Asked before each picture: waits while a preview reads.
        let stale = || {
            drop(state.busy_disk.lock());
            state.identical_run.load(Ordering::SeqCst) != run
        };
        let (files, identical) = {
            let s = state.session.locked();
            (s.files.clone(), s.identical.clone())
        };
        let mut cache = state.pictures.locked();
        let mut last = Instant::now();
        let groups = renome_core::similar::similar_pictures(
            &files,
            &identical,
            &mut cache,
            &mut |done, total| {
                if last.elapsed() > Duration::from_millis(150) || done == total {
                    last = Instant::now();
                    let _ = app.emit("similar-progress", IdenticalProgress { done, total });
                }
            },
            &stale,
        );
        cache.save();
        drop(cache);
        let groups = groups.filter(|_| !stale())?;
        state.session.locked().similar = renome_core::repeated::by_group(&groups);
        Some(groups)
    })
    .await
    .map_err(|e| e.to_string())
}

/// A picture (or any file) as the comparison window shows it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PictureCard {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    /// The size as shown (`12 MB`).
    pub(crate) size_text: String,
    /// Unix seconds.
    pub(crate) modified: Option<i64>,
    /// When the photo was taken (EXIF), `YYYY-MM-DDTHH:MM:SS`.
    pub(crate) taken: Option<String>,
    /// Width and height as shown.
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    /// A smaller copy, as a `data:` URL; `None` for what is not a picture
    /// or cannot be read.
    pub(crate) image: Option<String>,
}

/// A picture of a group as the comparison window knows it.
#[derive(serde::Deserialize)]
pub(crate) struct CardFacts {
    pub(crate) path: PathBuf,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    pub(crate) size: u64,
    pub(crate) taken: Option<String>,
    pub(crate) modified: Option<i64>,
}

/// Which pictures of a group stand out (see `similar::picks`: the
/// command line marks the same).
#[tauri::command]
pub(crate) fn picture_picks(cards: Vec<CardFacts>) -> renome_core::similar::Picks {
    let worth: Vec<renome_core::similar::Worth> = cards
        .iter()
        .map(|c| {
            let taken = c
                .taken
                .as_deref()
                .and_then(|t| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S").ok());
            renome_core::similar::Worth::of(
                &c.path,
                c.width.zip(c.height),
                c.size,
                taken,
                c.modified,
            )
        })
        .collect();
    renome_core::similar::picks(&worth)
}

/// The files of a group as the comparison window shows them, pictures
/// with a copy whose longest side is at most `max` pixels.
#[tauri::command]
pub(crate) async fn picture_cards(paths: Vec<PathBuf>, max: u32) -> Vec<PictureCard> {
    tauri::async_runtime::spawn_blocking(move || {
        let card = |path: &PathBuf| {
            let meta = std::fs::symlink_metadata(path).ok();
            let entry = meta.as_ref().and_then(|m| FileEntry::from_path(path, m));
            let ext = entry
                .as_ref()
                .map(|e| e.ext.to_lowercase())
                .unwrap_or_default();
            let shown = (renome_core::similar::is_picture(&ext)
                && entry.as_ref().is_some_and(|e| !e.is_dir))
            .then(|| std::fs::read(path).ok())
            .flatten()
            .and_then(|b| renome_core::similar::shown_copy(&b, &ext, max.clamp(64, 4096)));
            let dims = shown.as_ref().and_then(|(_, d)| *d);
            PictureCard {
                path: path.clone(),
                size: entry.as_ref().map_or(0, |e| e.size),
                size_text: renome_core::text::size(entry.as_ref().map_or(0, |e| e.size)),
                modified: entry.as_ref().and_then(|e| e.modified),
                taken: entry
                    .as_ref()
                    .filter(|_| renome_core::similar::is_picture(&ext))
                    .and_then(|e| e.taken())
                    .map(|t| t.format("%Y-%m-%dT%H:%M:%S").to_string()),
                width: dims.map(|d| d.0),
                height: dims.map(|d| d.1),
                image: shown.map(|(jpeg, _)| {
                    format!("data:image/jpeg;base64,{}", renome_core::b64::encode(&jpeg))
                }),
            }
        };
        std::thread::scope(|scope| {
            let jobs: Vec<_> = paths
                .chunks(paths.len().div_ceil(4).max(1))
                .map(|part| scope.spawn(move || part.iter().map(card).collect::<Vec<_>>()))
                .collect();
            jobs.into_iter()
                .flat_map(|j| j.join().unwrap_or_default())
                .collect()
        })
    })
    .await
    .unwrap_or_default()
}

/// A comic's or book's own cover: a smaller copy to show and its
/// fingerprint (hex, to compare with the catalogues' covers).
#[derive(Serialize)]
pub(crate) struct OwnCover {
    pub(crate) image: String,
    pub(crate) hash: String,
}

/// The cover of a comic or book file (see [`renome_core::convert::cover`]).
#[tauri::command]
pub(crate) async fn own_cover(path: PathBuf) -> Option<OwnCover> {
    tauri::async_runtime::spawn_blocking(move || {
        let (bytes, hash) = renome_services::covers::own_cover(&path)?;
        let (jpeg, _) = renome_core::similar::shown_copy(&bytes, "img", 600)?;
        Some(OwnCover {
            image: format!("data:image/jpeg;base64,{}", renome_core::b64::encode(&jpeg)),
            hash: format!("{hash:016x}"),
        })
    })
    .await
    .ok()
    .flatten()
}

/// A catalogue's cover that the page could not show by its address, as
/// a data address; `None` when it cannot be read either.
#[tauri::command]
pub(crate) async fn remote_image(url: String) -> Option<String> {
    if !url.starts_with("https://") {
        return None;
    }
    let res = HTTP.get(&url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    let kind = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("image/"))?
        .to_string();
    let bytes = res.bytes().await.ok()?;
    (bytes.len() <= 4 << 20)
        .then(|| format!("data:{kind};base64,{}", renome_core::b64::encode(&bytes)))
}

/// How far each cover (by address) is from the fingerprint `hash`, in
/// bits; `None` for one that could not be read. Covers are fetched four at
/// a time and remembered for the session.
#[tauri::command]
pub(crate) async fn cover_distances(
    state: State<'_, AppState>,
    hash: String,
    urls: Vec<String>,
) -> Result<Vec<Option<u32>>, String> {
    // Only those that are the file's cover (see `covers::SAME_COVER`).
    let own = u64::from_str_radix(&hash, 16).map_err(|e| e.to_string())?;
    Ok(state.covers.same(own, &urls).await)
}

/// The series found, compared with the file's cover (see
/// `covers::series_by_cover`, the command line's chooser too); tells how
/// far it got as `cover-progress`.
#[tauri::command]
pub(crate) async fn series_by_cover(
    app: AppHandle,
    source: String,
    key: String,
    hash: String,
    comic: renome_core::comics::ComicMatch,
    candidates: Vec<renome_core::comics::Volume>,
) -> Result<renome_services::covers::SeriesByCover, String> {
    let own = u64::from_str_radix(&hash, 16).map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    let sources = sources(&state);
    let emitter = app.clone();
    Ok(renome_services::covers::series_by_cover(
        &state.covers,
        &sources,
        &source,
        &key,
        own,
        &comic,
        candidates,
        &move |i, n| {
            let _ = emitter.emit("cover-progress", serde_json::json!({ "i": i, "n": n }));
        },
    )
    .await)
}

/// A series' issues in the order the review shows them, those `typed`
/// names (see `covers::issue_order`, the command line's too).
#[tauri::command]
pub(crate) fn issue_order(
    issues: Vec<renome_core::comics::Issue>,
    own: renome_core::comics::ComicMatch,
    far: HashMap<String, Option<u32>>,
    typed: String,
) -> Vec<usize> {
    renome_services::covers::issue_order(&issues, &own, &far, &typed)
}

/// The covers a review compares, and whether at once (few) or when asked.
#[tauri::command]
pub(crate) fn issue_covers(
    issues: Vec<renome_core::comics::Issue>,
    volume_key: String,
) -> (Vec<String>, bool, Option<&'static str>) {
    let urls = renome_services::covers::issue_covers(&issues);
    let now = urls.len() <= renome_services::covers::COMPARE_AT_ONCE;
    // The GCD gives no covers to programs: said so.
    let note = volume_key.starts_with("gcd:").then_some("coverNoneGcd");
    (urls, now, note)
}

/// Where each picture of a group is, as the comparison window says it
/// (see `similar::places`).
#[tauri::command]
pub(crate) fn picture_places(group: Vec<PathBuf>) -> Vec<String> {
    renome_core::similar::places(&group)
}

/// A larger copy of a catalogue's cover, where there is one.
#[tauri::command]
pub(crate) fn bigger_cover(url: String) -> String {
    renome_services::mangadex::bigger(&url)
}
