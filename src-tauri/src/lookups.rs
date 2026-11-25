//! The window's commands: lookups.

use super::*;

/// Where TMDB images are shown from, without the size (`…/t/p`): tests
/// point it elsewhere with `RENOME_TMDB_IMAGES`.
#[tauri::command]
pub(crate) fn tmdb_images() -> String {
    std::env::var("RENOME_TMDB_IMAGES")
        .ok()
        .and_then(|b| b.strip_suffix("/original").map(str::to_string))
        .unwrap_or_else(|| "https://image.tmdb.org/t/p".into())
}

/// The movie rule's search: what is sure is picked, the rest are doubts
/// (the flow the command line runs too).
#[tauri::command]
pub(crate) async fn movie_search(
    app: AppHandle,
    key: String,
    lang: String,
    items: Vec<renome_services::flow::Item>,
) -> Result<Vec<renome_services::flow::movies::Found>, String> {
    let client = tmdb::Client::new(&key, &lang);
    renome_services::flow::movies::search(&Gui(app.clone()), &client, &items).await
}

/// Each chosen movie as the rule names it, with its template fields (the
/// shared flow's: `flow::movies::named`).
#[tauri::command]
pub(crate) fn movie_named(
    files: Vec<(PathBuf, renome_core::tmdb::Candidate)>,
    extras: HashMap<u64, renome_core::fields::Extra>,
) -> HashMap<PathBuf, renome_core::tmdb::MovieMatch> {
    renome_services::flow::movies::named(files, &extras)
}

/// The collections the chosen movies belong to, for those missing.
#[tauri::command]
pub(crate) async fn tmdb_collections(
    key: String,
    lang: String,
    ids: Vec<u64>,
    sets: bool,
) -> tmdb::MovieSets {
    tmdb::collections(&tmdb::Client::new(&key, &lang), ids, sets).await
}

/// A search typed by the user in the review window.
#[tauri::command]
pub(crate) async fn tmdb_search(
    key: String,
    lang: String,
    title: String,
    year: Option<u16>,
) -> Result<Vec<renome_core::tmdb::Candidate>, String> {
    // A link or id pasted: that movie (see `flow::typed`, the command
    // line's too).
    let client = tmdb::Client::new(&key, &lang);
    renome_services::flow::typed::movie(&client, &title, year).await
}

/// Finds a show by what the user typed (a name, a TMDB id or link), or by
/// the files' names when nothing was typed.
#[tauri::command]
pub(crate) async fn tv_search(
    key: String,
    lang: String,
    query: String,
    items: Vec<tmdb::TvItem>,
) -> Result<tmdb::ShowSearch, String> {
    let client = tmdb::Client::new(&key, &lang);
    renome_services::flow::typed::show(&client, None, &query, &items).await
}

/// Like `tv_search`, on AniList (anime, no key).
#[tauri::command]
pub(crate) async fn anime_search(
    app: AppHandle,
    query: String,
    items: Vec<tmdb::TvItem>,
) -> Result<tmdb::ShowSearch, String> {
    let state = app.state::<AppState>();
    let client = tmdb::Client::new("", "");
    renome_services::flow::typed::show(&client, Some(&state.al), &query, &items).await
}

/// The TV show rule's search (the flow the command line runs too): the
/// shows, asked with `flow-ask` when unsure, and each file's episode.
/// `settled`: [path, show id] of the files that keep their episode.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn series_search(
    app: AppHandle,
    key: String,
    lang: String,
    source: String,
    typed: String,
    items: Vec<renome_services::flow::Item>,
    settled: Vec<(PathBuf, u64)>,
    picked: HashMap<String, renome_core::tmdb::Candidate>,
) -> Result<renome_services::flow::series::Outcome, String> {
    use renome_services::flow::series::{self, Source};
    let client = tmdb::Client::new(&key, &lang);
    let state = app.state::<AppState>();
    let source = if source == "anilist" {
        Source::Anilist(&state.al)
    } else {
        Source::Tmdb(&client)
    };
    let settled: HashSet<(PathBuf, u64)> = settled.into_iter().collect();
    series::search(&Gui(app.clone()), source, &typed, &items, &settled, picked).await
}

/// Each file's episodes as the rule names them.
#[tauri::command]
pub(crate) fn series_named(
    files: Vec<renome_services::flow::series::Settled>,
    shows: Vec<renome_core::tv::Show>,
) -> HashMap<PathBuf, renome_core::tv::EpisodeMatch> {
    renome_services::flow::series::named(&files, &shows)
}

/// The episodes what was typed names (see `flow::series::typed`, the
/// command line's chooser too).
#[tauri::command]
pub(crate) fn episodes_typed(
    show: renome_core::tv::Show,
    text: String,
) -> renome_services::flow::series::Typed {
    renome_services::flow::series::typed(&show, &text)
}

/// Whether the GCD is searched without an account (see `options::gcd_few`).
#[tauri::command]
pub(crate) fn gcd_few(source: String, email: String, password: String) -> bool {
    renome_services::options::gcd_few(&source, &email, &password)
}

/// The options' Test button of a key or account (`checks::test`, the
/// command line's `keys test` too): what to say when it works.
#[tauri::command]
pub(crate) async fn account_check(
    name: String,
    values: HashMap<String, String>,
) -> Result<renome_services::flow::after::Line, String> {
    let get = |k: &str| values.get(k).cloned().unwrap_or_default();
    renome_services::checks::test(&name, &get).await
}

/// What a reader menu's choice stands for (`readers::choose`): a model
/// and its screen, or a screen alone.
#[tauri::command]
pub(crate) fn reader_choice(value: String) -> (String, String) {
    renome_core::readers::choose(&value)
}

/// Episodes as file names write them (`S01E02-E03`: `flow::series::code`,
/// the command line's too), for each list.
#[tauri::command]
pub(crate) fn episode_codes(lists: Vec<Vec<(u16, u16)>>) -> Vec<String> {
    lists
        .iter()
        .map(|keys| renome_services::flow::series::code(keys))
        .collect()
}

/// Where a new album search looks first (see `flow::typed`).
#[tauri::command]
pub(crate) fn album_on_discogs(first_found: Option<String>, token: bool) -> bool {
    renome_services::flow::typed::album_on_discogs(first_found.as_deref(), token)
}

/// The files that take an answer given for `path` (see `flow::same_answer`,
/// the command line's too).
#[tauri::command]
pub(crate) fn same_answer(
    path: PathBuf,
    files: Vec<(PathBuf, String, renome_services::flow::Status)>,
) -> Vec<PathBuf> {
    renome_services::flow::same_answer(
        &path,
        files.iter().map(|(p, g, s)| (p.as_path(), g.as_str(), *s)),
    )
}

/// The languages the catalogues are asked in: TMDB's (the one chosen, else
/// the interface's) and the comic catalogues'.
#[tauri::command]
pub(crate) fn catalog_langs(tmdb_lang: String, lang: String) -> Value {
    let chosen = serde_json::json!({ "tmdbLang": tmdb_lang });
    serde_json::json!({
        "tmdb": renome_services::options::tmdb_lang(Some(&chosen), &lang),
        "comic": renome_services::options::comic_lang(&lang),
    })
}

/// The details typed in a details window over each file's (see
/// `flow::edit`, the command line's `--edit` too): `kind` is `song`,
/// `book` or `comic`.
#[tauri::command]
pub(crate) fn edit_details(
    kind: String,
    items: Vec<Value>,
    changed: renome_services::flow::edit::Changed,
) -> Result<Vec<Value>, String> {
    use renome_services::flow::edit;
    items
        .into_iter()
        .map(|v| {
            let out = match kind.as_str() {
                "song" => serde_json::to_value(edit::song(
                    serde_json::from_value(v).map_err(|e| e.to_string())?,
                    &changed,
                )),
                "book" => serde_json::to_value(edit::book(
                    serde_json::from_value(v).map_err(|e| e.to_string())?,
                    &changed,
                )),
                _ => serde_json::to_value(edit::comic(
                    serde_json::from_value(v).map_err(|e| e.to_string())?,
                    &changed,
                )),
            };
            out.map_err(|e| e.to_string())
        })
        .collect()
}

/// A value the window sent, read as `T` (unreadable: none).
pub(crate) fn current<T: serde::de::DeserializeOwned>(v: &Option<Value>) -> Option<T> {
    v.clone().and_then(|v| serde_json::from_value(v).ok())
}

/// What the window knows of a file for its details: what was chosen or
/// typed (`current`), what the lookup read of it (`guess`: a book's
/// guess, a comic's own details).
#[derive(serde::Deserialize, Default)]
pub(crate) struct Known {
    #[serde(default)]
    pub(crate) current: Option<Value>,
    #[serde(default)]
    pub(crate) guess: Option<Value>,
}

/// The rows a details window opens on for those chosen, and what each
/// starts from (see `flow::edit`: the command line's `--edit` starts from
/// the same): `kind` is the rule's (`music`, `book`, `comic`, `photo`).
// The command's arguments are what the window sends, each by name.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn edit_start(
    app: AppHandle,
    kind: String,
    chosen: Vec<PathBuf>,
    known: HashMap<PathBuf, Known>,
    parts: HashMap<PathBuf, renome_services::flow::books::Part>,
    dates: HashMap<PathBuf, String>,
    from_name: bool,
    by_folder: bool,
    audiobooks: bool,
) -> Result<
    (
        Vec<PathBuf>,
        Vec<Value>,
        Vec<renome_services::flow::edit::Changed>,
    ),
    String,
> {
    use renome_services::flow::edit;
    in_background(app, move |state| {
        let items = state
            .shown
            .locked()
            .as_ref()
            .map(|p| p.items.clone())
            .unwrap_or_default();
        let chosen: HashSet<PathBuf> = chosen.into_iter().collect();
        let paths = edit::rows(&kind, &items, &chosen, &parts, audiobooks);
        let s = state.session.locked();
        let files: HashMap<&PathBuf, &FileEntry> = s.files.iter().map(|f| (&f.path, f)).collect();
        let by_path: HashMap<&PathBuf, &renome_core::PreviewItem> = items.iter().map(|i| (&i.path, i)).collect();
        let empty = Known::default();
        let values = paths
            .iter()
            .map(|p| {
                let k = known.get(p).unwrap_or(&empty);
                let item = by_path.get(p);
                let name = item
                    .map(|i| i.lookup_name.clone().unwrap_or_else(|| i.original.clone()))
                    .unwrap_or_default();
                let is_dir = item.is_some_and(|i| i.is_dir);
                match kind.as_str() {
                    "music" => serde_json::to_value(edit::song_start(current(&k.current), files.get(p).copied(), from_name)),
                    "book" => serde_json::to_value(edit::book_start(current(&k.current), current(&k.guess).as_ref(), p, &name)),
                    "comic" => serde_json::to_value(edit::comic_start(
                        current(&k.current),
                        current(&k.guess).as_ref(),
                        p,
                        &name,
                        is_dir,
                        by_folder,
                    )),
                    _ => {
                        let [year, month, day, hour, minute] =
                            edit::date_parts(dates.get(p).map_or("", String::as_str));
                        Ok(serde_json::json!({ "year": year, "month": month, "day": day, "hour": hour, "minute": minute }))
                    }
                }
                .unwrap_or_default()
            })
            .collect::<Vec<Value>>();
        // As the window shows them (the same text the fields are read back from).
        let rows = values.iter().map(|v| edit::as_text(&kind, v)).collect();
        (paths, values, rows)
    })
    .await
}

/// Photo dates typed over the ones they had (empty: what the file says);
/// `None` when the fields make no date.
#[tauri::command]
pub(crate) fn edit_dates(
    old: Vec<String>,
    changed: renome_services::flow::edit::Changed,
) -> Option<Vec<String>> {
    old.iter()
        .map(|o| renome_services::flow::edit::photo_date(o, &changed))
        .collect()
}

/// What the lookups say is missing from the list as shown (see
/// `flow::missing`): the faded rows, beside their files.
#[tauri::command]
pub(crate) fn missing_items(
    app: AppHandle,
    rules: Vec<Value>,
    known: renome_services::flow::missing::Known,
    descending: bool,
) -> Vec<renome_services::flow::missing::Missing> {
    use renome_services::flow::missing;
    let state = app.state::<AppState>();
    let shown = state.shown.locked();
    let found = match shown.as_ref() {
        Some(p) => missing::of(&rules, &known, p, descending, &missing::today()),
        None => Vec::new(),
    };
    // Kept for "Export the missing".
    *state.missing.locked() = found.clone();
    found
}

/// The videos whose subtitles are searched (the flow the command line
/// runs too).
#[tauri::command]
pub(crate) fn subs_targets(
    files: Vec<renome_services::flow::subtitles::Identified>,
    langs: String,
) -> Vec<Value> {
    use renome_services::flow::subtitles as flow;
    flow::targets(&files)
        .into_iter()
        .map(|item| {
            let key = flow::key(&item, &langs);
            let mut v = serde_json::to_value(&item).unwrap_or_default();
            v["key"] = Value::from(key);
            v
        })
        .collect()
}

/// Whether some subtitle service can be asked with these keys.
#[tauri::command]
pub(crate) fn subs_ready(auth: subtitles::Auth) -> bool {
    auth.ready()
}

/// The languages whose subtitles the rules on download, if any (see
/// `flow::subtitles::wanted`, the command line's too).
#[tauri::command]
pub(crate) fn subs_wanted(rules: Vec<Value>) -> Option<String> {
    renome_services::flow::subtitles::wanted(&rules)
}

/// Searches subtitles for the videos, in the languages not there yet,
/// with the one picked per language.
#[tauri::command]
pub(crate) async fn subs_search(
    app: AppHandle,
    auth: subtitles::Auth,
    langs: String,
    items: Vec<subtitles::Item>,
) -> renome_services::flow::subtitles::Outcome {
    let stop = stop_signal(&app);
    renome_services::flow::subtitles::search(&Gui(app.clone()), auth, &langs, items, stop).await
}

/// What is chosen across the videos' subtitles, and whether those from
/// OpenSubtitles pass the day's quota (see `flow::subtitles::tally`).
#[tauri::command]
pub(crate) fn subs_tally(
    videos: Vec<renome_services::flow::subtitles::Standing>,
    langs: Vec<String>,
    remaining: Option<i64>,
) -> renome_services::flow::subtitles::Tally {
    renome_services::flow::subtitles::tally(&videos, &langs, remaining)
}

/// The music rule's search (the flow the command line runs too): the
/// songs grouped in albums and looked up; `settled`: songs whose album
/// keeps its release.
#[tauri::command]
pub(crate) async fn music_search(
    app: AppHandle,
    items: Vec<renome_services::flow::Item>,
    settled: Vec<PathBuf>,
    from_name: bool,
    listen: bool,
) -> Result<Vec<musicbrainz::AlbumLookup>, String> {
    use renome_services::flow::music;
    let settled: HashSet<PathBuf> = settled.into_iter().collect();
    let wanted: HashSet<PathBuf> = music::targets(&items, listen, &settled)
        .into_iter()
        .collect();
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let files = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || session_files(&app, &wanted)
    })
    .await
    .map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    let services = musicbrainz::Services {
        mb: &state.mb,
        dg: &state.dg,
        acoustid: &state.acoustid,
        heard: &state.heard,
    };
    let mut found = music::search(
        &Gui(app.clone()),
        &services,
        &files.iter().collect::<Vec<_>>(),
        from_name,
        listen,
    )
    .await?;
    for a in found.iter_mut() {
        musicbrainz::thumbs(a.candidates.iter_mut().chain(a.choice.as_mut()));
    }
    Ok(found)
}

/// A search typed in the album review, on MusicBrainz or Discogs.
#[tauri::command]
pub(crate) async fn mb_search(
    app: AppHandle,
    query: renome_core::musicbrainz::AlbumQuery,
    discogs: Option<bool>,
) -> Result<Vec<renome_core::musicbrainz::Candidate>, String> {
    let state = app.state::<AppState>();
    let mut found =
        renome_services::flow::typed::album(&state.mb, &state.dg, &query, discogs == Some(true))
            .await?;
    renome_services::musicbrainz::thumbs(found.iter_mut());
    Ok(found)
}

/// The Discogs token from the options (empty: Discogs is not used).
#[tauri::command]
pub(crate) fn discogs_token(state: State<AppState>, token: String) {
    state.dg.set_token(&token);
}

/// The Google Books key from the options (empty: Google Books is not used).
#[tauri::command]
pub(crate) fn google_books_key(state: State<AppState>, key: String) {
    state.ol.gb.set_key(&key);
}

/// Matches an album's songs to the release the user chose.
#[tauri::command]
pub(crate) async fn mb_choose(
    app: AppHandle,
    id: String,
    paths: Vec<PathBuf>,
    from_name: bool,
) -> Result<musicbrainz::Chosen, String> {
    let files = session_files(&app, &paths.into_iter().collect());
    let state = app.state::<AppState>();
    let services = musicbrainz::Services {
        mb: &state.mb,
        dg: &state.dg,
        acoustid: &state.acoustid,
        heard: &state.heard,
    };
    renome_services::flow::music::choose(
        &services,
        &id,
        &files.iter().collect::<Vec<_>>(),
        from_name,
    )
    .await
}

/// The book rule's search (the flow the command line runs too): books
/// and audiobooks not `settled`, sure ones picked.
#[tauri::command]
pub(crate) async fn books_search(
    app: AppHandle,
    lang: String,
    items: Vec<renome_services::flow::Item>,
    settled: Vec<PathBuf>,
    audiobooks: bool,
) -> Result<renome_services::flow::books::Outcome, String> {
    let ol = &app.state::<AppState>().ol;
    let settled: HashSet<PathBuf> = settled.into_iter().collect();
    let mut out = renome_services::flow::books::search(
        &Gui(app.clone()),
        ol,
        &lang,
        &items,
        &settled,
        audiobooks,
    )
    .await?;
    for f in out.found.iter_mut() {
        renome_services::openlibrary::thumbs(f.candidates.iter_mut().chain(f.choice.as_mut()));
    }
    Ok(out)
}

/// Each file's book as the rule names it, from the books chosen.
#[tauri::command]
pub(crate) fn book_matches(
    chosen: Vec<renome_services::flow::books::Chosen>,
    parts: HashMap<PathBuf, renome_services::flow::books::Part>,
    audiobooks: bool,
) -> HashMap<PathBuf, renome_core::books::BookMatch> {
    renome_services::flow::books::matches(&chosen, &parts, audiobooks)
}

/// A search typed in the book review.
#[tauri::command]
pub(crate) async fn book_search(
    app: AppHandle,
    lang: String,
    title: String,
    author: String,
) -> Result<Vec<renome_core::books::Candidate>, String> {
    let ol = &app.state::<AppState>().ol;
    let mut found = renome_services::flow::typed::book(ol, &lang, &title, &author).await?;
    renome_services::openlibrary::thumbs(found.iter_mut());
    Ok(found)
}

/// The Grand Comics Database account from the options (empty: none).
#[tauri::command]
pub(crate) fn gcd_account(state: State<AppState>, email: String, password: String) {
    state.gcd.set_account(&email, &password);
}

/// Series with a name in a catalogue (`gcd`, `comicvine`, `metron`,
/// `mangadex`, `anilist`, `mangaupdates`), from a year and by a publisher
/// when given (the manga catalogues are not searched by publisher), best
/// first (see `flow::typed::comic`). `key` is Comic Vine's key or
/// Metron's token.
// The command's arguments are what the window sends, each by name.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn comic_search(
    app: AppHandle,
    source: String,
    key: String,
    query: String,
    year: String,
    publisher: String,
    lang: String,
    skip: Option<String>,
    highest: f64,
) -> Result<Vec<renome_core::comics::Volume>, String> {
    let state = app.state::<AppState>();
    // As the command line's chooser searches.
    renome_services::flow::typed::comic(
        &sources(&state),
        &source,
        &key,
        &query,
        &year,
        &publisher,
        &lang,
        skip.as_deref().unwrap_or_default(),
        highest,
    )
    .await
}

/// The comic rule's search (the flow the command line runs too): a
/// series it is not sure of is asked with `flow-ask`.
#[tauri::command]
pub(crate) async fn comics_search(
    app: AppHandle,
    params: renome_services::flow::comics::Params,
    items: Vec<renome_services::flow::Item>,
    picked: HashMap<String, renome_core::comics::Volume>,
) -> Result<renome_services::flow::comics::Outcome, String> {
    let state = app.state::<AppState>();
    renome_services::flow::comics::search(
        &Gui(app.clone()),
        &sources(&state),
        &params,
        &items,
        &HashSet::new(),
        picked,
    )
    .await
}

/// Each comic as the rule names it, with its page in the catalogue.
#[tauri::command]
pub(crate) fn comic_named(
    files: Vec<renome_services::flow::comics::Settled>,
    volumes: HashMap<String, renome_services::flow::comics::Listed>,
) -> HashMap<PathBuf, renome_services::flow::comics::Named> {
    renome_services::flow::comics::named(&files, &volumes)
}
