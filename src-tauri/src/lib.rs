//! The window: state, the host the shared flows talk to, and `run`. The
//! commands the page calls are in the modules below, one per subject; they
//! only decide what to call: the logic is in `renome-core` and
//! `renome-services`, which `renome-cli` uses too.

use renome_core::sync::MutexExt;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use renome_core::scan::Skipped;
use renome_core::{
    ApplyReport, FileEntry, HashCache, History, Preview, ScanOptions, SortKey, Step, View,
};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

mod files;
mod find;
mod lookups;
mod rename;
mod rules;
mod settings;
mod update;
use files::*;
use find::*;
use lookups::*;
use rename::*;
use rules::*;
use settings::*;

use renome_services::{
    ANSWERS, acoustid, anilist, cache_days, cache_root, comicvine, discogs, drop_old_cache, gcd,
    mangadex, mangaupdates, mediaserver, metron, musicbrainz, openlibrary, read_settings_file,
    secrets, subtitles, tmdb, write_settings_file,
};

/// The session's file list. It lives on the Rust side so the whole list
/// is not sent over on every preview update.
#[derive(Default)]
struct Session {
    files: Vec<FileEntry>,
    /// Earlier states of the list and of the hand-edited names, for Ctrl+Z.
    undo: Vec<(Vec<FileEntry>, Overrides)>,
    /// Group of each file with an identical copy in the list.
    identical: HashMap<PathBuf, usize>,
    /// Group of each song that is the same music as another in the list.
    alike: HashMap<PathBuf, usize>,
    /// Group of each picture that looks like another in the list.
    similar: HashMap<PathBuf, usize>,
}

/// How many list edits can be undone.
const UNDO_LIMIT: usize = 100;

struct AppState {
    session: Mutex<Session>,
    history: History,
    hashes: Mutex<HashCache>,
    /// Fingerprints of the catalogues' covers, by address (`None`: could
    /// not be read).
    covers: renome_services::covers::Covers,
    /// Fingerprints of pictures, kept between sessions.
    pictures: Mutex<renome_core::similar::PictureCache>,
    /// Bumped by each search for identical files; an older search that sees
    /// a newer number stops.
    identical_run: AtomicU64,
    /// Held while a preview reads the files or a renaming writes them: the
    /// searches for copies wait meanwhile (on a hard disk, two readers
    /// each go slower than one after the other).
    busy_disk: Mutex<()>,
    /// Bumped by Stop; a search that sees a newer number than it started
    /// with stops, keeping what it found.
    stop_run: AtomicU64,
    /// The flows' questions waiting for the page's answer, by number.
    asks: Mutex<HashMap<u64, tokio::sync::oneshot::Sender<Option<Value>>>>,
    ask_seq: AtomicU64,
    /// MusicBrainz, paced for all lookups together.
    mb: musicbrainz::Client,
    /// AcoustID, likewise.
    acoustid: acoustid::Client,
    /// The recordings each song was heard as (AcoustID), for the album
    /// review's choices.
    heard: Mutex<HashMap<PathBuf, Vec<String>>>,
    /// Open Library, likewise.
    ol: openlibrary::Client,
    /// Comic Vine, likewise, with its answers kept.
    cv: comicvine::Client,
    /// The Grand Comics Database, and the account signed in (in memory).
    gcd: gcd::Client,
    /// MangaDex.
    md: mangadex::Client,
    /// AniList (manga), with its answers kept.
    al: anilist::Client,
    /// MangaUpdates, likewise.
    mu: mangaupdates::Client,
    /// Metron, with the user's token, likewise.
    mt: metron::Client,
    /// Discogs, with the user's token.
    dg: discogs::Client,
    /// The list as last shown, and as last renamed: what is written after
    /// the renames is planned on them (see `flow::after`).
    shown: Mutex<Option<renome_core::Preview>>,
    /// What is missing from the list as last shown.
    missing: Mutex<Vec<renome_services::flow::missing::Missing>>,
    renamed: Mutex<Option<renome_core::Preview>>,
}

type Overrides = HashMap<PathBuf, String>;

use renome_services::protected_dirs;

/// The list as the window shows it, with what is said under it and of
/// its rows (see `flow::said`, the command line's preview too).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Shown {
    #[serde(flatten)]
    preview: Preview,
    /// Its totals (`said::totals`).
    totals: Vec<renome_services::flow::after::Line>,
    /// The old folders left with files (`said::left`).
    left: Vec<renome_services::flow::after::Line>,
    /// Each row's tags, by path (`said::tags`; rows without none left out).
    tags: HashMap<PathBuf, Vec<renome_services::flow::said::Tag>>,
    /// What each step's card says under it (`said::summaries`).
    summaries: Vec<Option<renome_services::flow::after::Line>>,
}

impl Shown {
    fn of(preview: Preview, steps: &[Step], warn_windows: bool) -> Self {
        use renome_services::flow::said;
        let tags = preview
            .items
            .iter()
            .filter_map(|i| {
                let t = said::tags(i, warn_windows);
                (!t.is_empty()).then(|| (i.path.clone(), t))
            })
            .collect();
        Self {
            totals: said::totals(&preview),
            left: said::left(&preview.left_folders),
            tags,
            summaries: said::summaries(&preview, steps),
            preview,
        }
    }
}

/// Whether Stop was pressed since this was called.
fn stop_signal(app: &AppHandle) -> impl Fn() -> bool + Send + Sync + 'static {
    let app = app.clone();
    let run = app.state::<AppState>().stop_run.load(Ordering::SeqCst);
    move || app.state::<AppState>().stop_run.load(Ordering::SeqCst) != run
}

/// The window's comic catalogues.
fn sources(state: &AppState) -> renome_services::comics::Sources<'_> {
    renome_services::comics::Sources {
        cv: &state.cv,
        mt: &state.mt,
        gcd: &state.gcd,
        md: &state.md,
        al: &state.al,
        mu: &state.mu,
    }
}

/// Files written: their tags are read again when next needed.
fn forget_tags_of(state: &AppState, written: Vec<PathBuf>) {
    let written: HashSet<PathBuf> = written.into_iter().collect();
    for f in state.session.locked().files.iter_mut() {
        if written.contains(&f.path) {
            f.forget_tags();
        }
    }
}

/// The window as the host of the shared work: reports become events for
/// the page, Stop is the page's button.
#[derive(Clone)]
struct Gui(AppHandle);

impl renome_services::Host for Gui {
    fn report(&self, event: &str, payload: Value) {
        let _ = self.0.emit(event, payload);
    }
    fn stop_signal(&self) -> std::sync::Arc<dyn Fn() -> bool + Send + Sync> {
        std::sync::Arc::new(stop_signal(&self.0))
    }
    fn sender(&self) -> renome_services::Sender {
        let app = self.0.clone();
        std::sync::Arc::new(move |event, payload| {
            let _ = app.emit(event, payload);
        })
    }
    fn print_pdf(
        &self,
        html: PathBuf,
        pdf: PathBuf,
        page: renome_core::bookconv::print::Page,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move { print_to_pdf(&self.0, &html, &pdf, page).await })
    }
    /// The question goes to the page (`flow-ask`), which opens its picker
    /// and answers with `flow_answer`.
    fn ask(
        &self,
        question: renome_services::flow::Question,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<Value>> + Send + '_>> {
        Box::pin(async move {
            let state = self.0.state::<AppState>();
            let id = state.ask_seq.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = tokio::sync::oneshot::channel();
            state.asks.locked().insert(id, tx);
            let _ = self.0.emit(
                "flow-ask",
                serde_json::json!({ "id": id, "question": question }),
            );
            rx.await.ok().flatten()
        })
    }
}

/// The page's answer to a flow's question (`None`: none fits).
#[tauri::command]
fn flow_answer(state: State<AppState>, id: u64, answer: Option<Value>) {
    if let Some(tx) = state.asks.locked().remove(&id) {
        let _ = tx.send(answer);
    }
}

/// Stops the searches under way (see [`stop_signal`]).
#[tauri::command]
fn stop_search(state: State<AppState>) {
    state.stop_run.fetch_add(1, Ordering::SeqCst);
}

/// For catalogue covers, fetched by the program (some catalogues refuse
/// their pictures to a page that is not theirs).
static HTTP: std::sync::LazyLock<reqwest::Client> =
    std::sync::LazyLock::new(|| renome_services::http(renome_services::AGENT, 10, 30));

/// The settings file is read and written whole by the settings and the
/// secrets apart: one at a time.
use renome_services::SETTINGS_FILE;

/// Inside the AppImage, GTK reads the GNOME schemas bundled with it, whose
/// default title bar layout is "close" only; distros that change that
/// default (e.g. Bazzite, with minimize/maximize) would be ignored. Ask the
/// system's `gsettings`, without the AppImage's variables.
#[cfg(target_os = "linux")]
fn sync_titlebar_buttons() {
    use gtk::prelude::GtkSettingsExt;

    if std::env::var_os("APPIMAGE").is_none() {
        return;
    }
    let Ok(out) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.wm.preferences", "button-layout"])
        .env_remove("GSETTINGS_SCHEMA_DIR")
        .env_remove("GIO_MODULE_DIR")
        .env_remove("XDG_DATA_DIRS")
        .env_remove("LD_LIBRARY_PATH")
        .output()
    else {
        return;
    };
    let layout = String::from_utf8_lossy(&out.stdout);
    let layout = layout.trim().trim_matches('\'');
    if out.status.success()
        && layout.contains(':')
        && let Some(s) = gtk::Settings::default()
    {
        s.set_gtk_decoration_layout(Some(layout));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Answers too old to use go, out of the way of the start.
    std::thread::spawn(|| {
        drop_old_cache();
        ANSWERS.purge();
    });

    let history_dir = renome_services::history_dir();

    // Paths given on the command line go into the initial list.
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let mut session = Session::default();
    if !args.is_empty() {
        let include_hidden = read_settings_file()
            .and_then(|s| s.get("includeHidden")?.as_bool())
            .unwrap_or(false);
        let opts = ScanOptions {
            include_hidden,
            ..Default::default()
        };
        session.files = renome_core::scan(&args, &opts, &HashSet::new()).files;
        renome_core::sort_files(&mut session.files, SortKey::Name, false);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|_| {
            #[cfg(target_os = "linux")]
            sync_titlebar_buttons();
            Ok(())
        })
        .manage(AppState {
            session: Mutex::new(session),
            history: History::new(history_dir),
            hashes: Mutex::default(),
            covers: renome_services::covers::Covers::default(),
            pictures: Mutex::new(renome_core::similar::PictureCache::in_file(
                cache_root().join("pictures"),
            )),
            identical_run: AtomicU64::new(0),
            busy_disk: Mutex::new(()),
            stop_run: AtomicU64::new(0),
            asks: Mutex::default(),
            ask_seq: AtomicU64::new(0),
            mb: musicbrainz::Client::default(),
            acoustid: acoustid::Client::default(),
            heard: Mutex::new(HashMap::new()),
            ol: openlibrary::Client::default(),
            cv: comicvine::Client::default(),
            gcd: gcd::Client::default(),
            md: mangadex::Client::default(),
            al: anilist::Client::default(),
            mu: mangaupdates::Client::default(),
            mt: metron::Client::default(),
            dg: discogs::Client::default(),
            shown: Mutex::default(),
            missing: Mutex::default(),
            renamed: Mutex::default(),
        })
        .invoke_handler(tauri::generate_handler![
            pdf_print_available,
            checkpoint,
            drop_checkpoint,
            undo_list,
            add_paths,
            remove_files,
            stop_search,
            find_similar,
            picture_cards,
            picture_picks,
            own_cover,
            cover_distances,
            series_by_cover,
            issue_order,
            issue_covers,
            remote_image,
            trash_files,
            tmdb_collections,
            clear_files,
            move_files,
            sort_files,
            file_details,
            export_rows,
            export_missing,
            discogs_token,
            google_books_key,
            preview,
            find_identical,
            movie_search,
            series_search,
            series_named,
            movie_named,
            episode_codes,
            rule_view,
            account_check,
            reader_choice,
            subs_tally,
            gcd_few,
            picture_places,
            flow_answer,
            tmdb_search,
            tv_search,
            subs_search,
            subs_targets,
            subs_wanted,
            subs_ready,
            server_check,
            server_read,
            server_new,
            sets_save,
            sets_rename,
            sets_order,
            server_keys,
            server_key_with,
            server_tell,
            update_check,
            update_install,
            update_way,
            music_search,
            mb_search,
            mb_choose,
            after_counts,
            rename_ask,
            rule_menu,
            manual_order,
            rule_reader,
            edit_details,
            edit_dates,
            edit_start,
            rule_table,
            rule_blocks,
            complete_steps,
            album_on_discogs,
            bigger_cover,
            episodes_typed,
            same_answer,
            columns_for,
            catalog_langs,
            missing_items,
            after_rename,
            books_search,
            book_matches,
            book_search,
            comic_search,
            anime_search,
            comics_search,
            comic_named,
            gcd_account,
            save_csv,
            tmdb_images,
            recover,
            apply,
            history,
            undo,
            delete_history,
            clear_history,
            cache_size,
            clear_cache,
            save_preset,
            load_preset,
            load_settings,
            save_settings,
            load_secrets,
            save_secrets,
            system_language,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Renome");
}
