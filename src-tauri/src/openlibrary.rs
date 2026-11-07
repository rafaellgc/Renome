//! Open Library client: book searches by ISBN or by title and author, a
//! few requests a second with the program identified, as the service asks;
//! and the lookups, covers and `metadata.opf` files built on it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures_util::{StreamExt, stream};
use renome_core::books::{self, AuthorPage, BookMatch, Candidate, Guess, Query, SearchPage, Work};
use renome_core::ebook;
use renome_core::links::Link;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::tmdb::{SidecarReport, write_new};

const API: &str = "https://openlibrary.org";
const COVERS: &str = "https://covers.openlibrary.org";
const BRASILAPI: &str = "https://brasilapi.com.br/api/isbn/v1";

/// Tests point these at a local server with `RENOME_OL_API` and
/// `RENOME_OL_COVERS`.
fn api_base() -> String {
    std::env::var("RENOME_OL_API").unwrap_or_else(|_| API.into())
}

fn brasilapi_base() -> String {
    std::env::var("RENOME_BRASILAPI").unwrap_or_else(|_| BRASILAPI.into())
}

fn covers_base() -> String {
    std::env::var("RENOME_OL_COVERS").unwrap_or_else(|_| COVERS.into())
}

/// Shared by all lookups, so the pace holds across them. Google Books,
/// with the user's key, is asked too.
pub struct Client {
    http: reqwest::Client,
    last: tokio::sync::Mutex<Option<Instant>>,
    pub gb: crate::googlebooks::Client,
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_connect() {
        "olOffline".into()
    } else if e.is_timeout() {
        "olTimeout".into()
    } else {
        format!("olNetwork: {}", e.without_url())
    }
}

impl Default for Client {
    fn default() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!(
                    "Renome/",
                    env!("CARGO_PKG_VERSION"),
                    " ( https://github.com/rafaellgc/Renome )"
                ))
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            last: tokio::sync::Mutex::new(None),
            gb: crate::googlebooks::Client::default(),
        }
    }
}

/// `pt-BR` → `pt`, as Open Library takes it.
fn lang2(lang: &str) -> &str {
    lang.split('-').next().unwrap_or("en")
}

impl Client {
    /// A request at most every 0.4 seconds (identified programs may make
    /// three a second); a busy service is asked again a little later.
    async fn send(&self, url: &str, query: &[(&str, &str)]) -> Result<reqwest::Response, String> {
        for attempt in 0..3 {
            let mut last = self.last.lock().await;
            if let Some(t) = *last {
                tokio::time::sleep(Duration::from_millis(400).saturating_sub(t.elapsed())).await;
            }
            let res = self.http.get(url).query(query).send().await;
            *last = Some(Instant::now());
            drop(last);
            let res = res.map_err(net_error)?;
            match res.status().as_u16() {
                200 => return Ok(res),
                404 => return Err("olNotFound".into()),
                429 | 500..=599 if attempt < 2 => {
                    tokio::time::sleep(Duration::from_secs(3 * (attempt + 1))).await
                }
                429 | 503 => return Err("olBusy".into()),
                code => return Err(format!("olNetwork: HTTP {code}")),
            }
        }
        Err("olBusy".into())
    }

    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, String> {
        let question = format!("{path} {query:?}");
        if let Some(v) = crate::kept("openlibrary", &question) {
            return Ok(v);
        }
        let res = self.send(&format!("{}{path}", api_base()), query).await?;
        let body = res.bytes().await.map_err(net_error)?;
        crate::keep("openlibrary", &question, &body).map_err(|e| format!("olNetwork: {e}"))
    }

    async fn search(&self, q: &Query, lang: &str) -> Result<SearchPage, String> {
        let by_author;
        let mut query = vec![
            ("fields", books::FIELDS),
            ("lang", lang2(lang)),
            ("limit", "12"),
        ];
        match q {
            Query::Isbn(i) => query.push(("isbn", i)),
            Query::Titled(t) | Query::Text(t) => query.push(("q", t)),
            Query::Author(name) => {
                let Some(key) = self.author(name).await? else {
                    return Ok(SearchPage::default());
                };
                // The best-known books first, more of them: the one sought
                // may be among them under its original title.
                by_author = format!("author_key:{key}");
                query.push(("q", &by_author));
                query.push(("sort", "editions"));
                query[2].1 = "20";
            }
        }
        self.get("/search.json", &query).await
    }

    /// The key of the author meant by a name, also found by the surname
    /// alone (`Lúcio Aneu Tércio` is `Tércio` there).
    async fn author(&self, name: &str) -> Result<Option<String>, String> {
        let surname = books::surname(name);
        for q in [name, surname.as_str()] {
            if q.is_empty() {
                continue;
            }
            let page: AuthorPage = self
                .get("/search/authors.json", &[("q", q), ("limit", "20")])
                .await?;
            if let Some(a) = books::pick_author(q, &page.docs) {
                return Ok(Some(a.key.trim_start_matches("/authors/").to_string()));
            }
        }
        Ok(None)
    }

    /// Searches for a guess: by its ISBNs (the edition itself), else by
    /// title and author, trying the next search while the results are not
    /// a safe pick and two searches have not found anything yet; then by
    /// the author.
    ///
    /// An ISBN Open Library does not know is asked of Google Books (with
    /// the user's key), then of BrasilAPI, which has most books published
    /// in Brazil: that edition comes first, with its work on Open Library
    /// when the search finds it. Without an ISBN, a book Open Library does
    /// not settle is searched on Google Books too, and the books found
    /// there join those (see [`books::with_google`]).
    pub async fn find(&self, g: &Guess, lang: &str) -> Result<(Vec<Candidate>, bool), String> {
        let found = match self.find_ol(g, lang).await {
            Ok(f) => f,
            // Google Books may still find it.
            Err(e) if self.gb.enabled() => {
                return self
                    .find_google(g, lang, (Vec::new(), false))
                    .await
                    .map_err(|_| e);
            }
            Err(e) => return Err(e),
        };
        if found.0.first().is_some_and(|c| c.exact) {
            return Ok(found);
        }
        for isbn in g.isbns.iter().take(3) {
            let google = match self.gb.by_isbn(isbn).await {
                Ok(b) => b.and_then(|b| books::google_candidate(&b, g, Some(isbn))),
                Err(_) => None,
            };
            let edition = match google {
                Some(e) => Some(e),
                None => self
                    .brasil(isbn)
                    .await
                    .and_then(|b| books::brasil_candidate(isbn, &b, g)),
            };
            let Some((edition, ok)) = edition else {
                continue;
            };
            let merged = books::with_edition(edition.clone(), ok, found.clone());
            if !ok || !merged.0[0].work.is_empty() {
                return Ok(merged);
            }
            // The file's title may differ from the edition's (a series
            // name before it): its work is searched by the edition's.
            let by_edition = Guess {
                title: edition.title.clone(),
                author: edition.authors.first().cloned().unwrap_or_default(),
                from_file: true,
                ..Default::default()
            };
            return Ok(match self.find_ol(&by_edition, lang).await {
                Ok(again) => books::with_edition(edition, ok, again),
                Err(_) => merged,
            });
        }
        if found.1 || !self.gb.enabled() {
            return Ok(found);
        }
        // Google's failures leave what Open Library found.
        let fallback = found.clone();
        Ok(self.find_google(g, lang, found).await.unwrap_or(fallback))
    }

    /// The books Google Books finds by title and author (the guess's
    /// readings, two at most) joined to `found`.
    async fn find_google(
        &self,
        g: &Guess,
        lang: &str,
        found: (Vec<Candidate>, bool),
    ) -> Result<(Vec<Candidate>, bool), String> {
        let mut pairs = vec![(g.title.clone(), g.author.clone())];
        pairs.extend(g.alts.iter().cloned());
        let mut extra = Vec::new();
        for (t, a) in pairs.iter().filter(|(t, _)| !t.trim().is_empty()).take(2) {
            let list = self.gb.search(t, a, lang).await?;
            extra.extend(
                list.iter()
                    .filter_map(|b| books::google_candidate(b, g, None)),
            );
            if extra.iter().any(|(_, fits)| *fits) {
                break;
            }
        }
        Ok(books::with_google(g, found, extra))
    }

    /// The edition of an ISBN on BrasilAPI; `None` when it is not there or
    /// the service fails, which leaves the search as it was.
    async fn brasil(&self, isbn: &str) -> Option<books::BrasilBook> {
        let url = format!("{}/{isbn}", brasilapi_base());
        let res = self.http.get(url).send().await.ok()?;
        if !res.status().is_success() {
            return None;
        }
        res.json().await.ok()
    }

    async fn find_ol(&self, g: &Guess, lang: &str) -> Result<(Vec<Candidate>, bool), String> {
        let mut docs: Vec<books::Doc> = Vec::new();
        let mut found = 0;
        let mut best = (Vec::new(), false);
        for q in books::queries(g) {
            let page = self.search(&q, lang).await?;
            if page.docs.is_empty() {
                continue;
            }
            if let Query::Isbn(i) = &q {
                return Ok(books::rank(g, page.docs, Some(i), lang));
            }
            let titled = matches!(q, Query::Titled(_));
            for (i, mut d) in page.docs.into_iter().enumerate() {
                let top = titled && i == 0;
                match docs.iter_mut().find(|x| x.key == d.key) {
                    Some(x) => x.top_by_title |= top,
                    None => {
                        d.top_by_title = top;
                        docs.push(d);
                    }
                }
            }
            best = books::rank(g, docs.clone(), None, lang);
            // Results that were all left out do not count.
            if !best.0.is_empty() {
                found += 1;
            }
            // The author's books are the last try.
            if best.1 || found == 2 || (found > 0 && matches!(q, Query::Author(_))) {
                break;
            }
        }
        Ok(best)
    }

    /// The book a pasted ISBN or Open Library link names.
    pub async fn by_link(&self, link: &Link, lang: &str) -> Result<Vec<Candidate>, String> {
        let q = match link {
            Link::Isbn(i) => {
                let g = Guess {
                    isbns: vec![i.clone()],
                    from_file: true,
                    ..Default::default()
                };
                return Ok(self.find(&g, lang).await?.0);
            }
            Link::OpenLibrary(id) if id.ends_with('W') => format!("key:\"/works/{id}\""),
            Link::OpenLibrary(id) => format!("edition_key:{id}"),
            _ => return Err("linkOther".into()),
        };
        let page = self.search(&Query::Text(q), lang).await?;
        let Some(first) = page.docs.first() else {
            return Err("linkNotFound".into());
        };
        // Its own title, so the ranking keeps it.
        let g = Guess {
            title: first.title.clone(),
            from_file: true,
            ..Default::default()
        };
        Ok(books::rank(&g, page.docs, None, lang).0)
    }

    pub async fn work(&self, id: &str) -> Result<Work, String> {
        self.get(&format!("/works/{id}.json"), &[]).await
    }

    async fn cover(&self, id: i64) -> Result<Vec<u8>, String> {
        let url = format!("{}/b/id/{id}-L.jpg", covers_base());
        let res = self.send(&url, &[("default", "false")]).await?;
        Ok(res.bytes().await.map_err(net_error)?.to_vec())
    }
}

#[derive(Deserialize)]
pub struct LookupItem {
    path: PathBuf,
    /// File name without extension.
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lookup {
    path: PathBuf,
    guess: Guess,
    candidates: Vec<Candidate>,
    /// The first candidate is a safe automatic choice.
    auto: bool,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
}

/// Reads each book's details, then looks them up, a few at a time. Files
/// with the same guess (one book in several formats) share one search.
pub async fn lookup(
    app: &AppHandle,
    client: &Client,
    lang: &str,
    items: Vec<LookupItem>,
) -> Vec<Lookup> {
    let guesses: Vec<(PathBuf, Guess)> = tauri::async_runtime::spawn_blocking(move || {
        items
            .into_iter()
            .map(|i| {
                let meta = ebook::read(&i.path);
                let g = books::guess(&i.name, &meta);
                (i.path, g)
            })
            .collect()
    })
    .await
    .unwrap_or_default();
    let key = |g: &Guess| (g.title.clone(), g.author.clone(), g.isbns.clone());
    let mut unique: Vec<Guess> = Vec::new();
    for (_, g) in &guesses {
        if (!g.title.is_empty() || !g.isbns.is_empty()) && !unique.iter().any(|u| key(u) == key(g))
        {
            unique.push(g.clone());
        }
    }
    let stopped = crate::stop_signal(app);
    let total = unique.len();
    let _ = app.emit("book-progress", Progress { done: 0, total });
    let mut done = 0;
    let mut found: HashMap<usize, Result<(Vec<Candidate>, bool), String>> = HashMap::new();
    let mut tasks = stream::iter(unique.clone().into_iter().enumerate())
        .map(|(i, g)| async move { (i, client.find(&g, lang).await) })
        .buffer_unordered(3);
    while let Some((i, r)) = tasks.next().await {
        found.insert(i, r);
        done += 1;
        let _ = app.emit("book-progress", Progress { done, total });
        if stopped() {
            break;
        }
    }
    // Books not searched before Stop are left out: they are still to do.
    guesses
        .into_iter()
        .filter_map(|(path, guess)| {
            let hit = unique
                .iter()
                .position(|u| key(u) == key(&guess))
                .map(|i| found.get(&i));
            let (candidates, auto, error) = match hit {
                Some(Some(Ok((c, auto)))) => (c.clone(), *auto, None),
                Some(Some(Err(e))) => (Vec::new(), false, Some(e.clone())),
                Some(None) => return None,
                None => (Vec::new(), false, None),
            };
            let mut guess = guess;
            if let Some(best) = candidates.first() {
                (guess.title, guess.author) = books::reading(&guess, best);
            }
            Some(Lookup {
                path,
                guess,
                candidates,
                auto,
                error,
            })
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarItem {
    /// The renamed book.
    path: PathBuf,
    book: BookMatch,
    cover: Option<i64>,
    /// A cover elsewhere (Google Books), when there is no `cover`.
    #[serde(default)]
    cover_url: String,
    /// A description from elsewhere, when Open Library has none.
    #[serde(default)]
    description: String,
    #[serde(default)]
    language: String,
}

/// Saves the cover and `metadata.opf` of each book: in a book's own folder
/// as `cover.jpg` and `metadata.opf`, next to other books with the book's
/// name. Files already there are kept, so one book in several formats gets
/// them once.
pub async fn sidecars(
    app: &AppHandle,
    client: &Client,
    items: Vec<SidecarItem>,
    cover: bool,
    opf: bool,
    in_folder: bool,
) -> SidecarReport {
    let total = items.len();
    let mut report = SidecarReport::default();
    let _ = app.emit("book-progress", Progress { done: 0, total });
    let name = |p: &Path| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    // One at a time: books of one folder share their files.
    for (i, item) in items.into_iter().enumerate() {
        let file = |plain: &str, ext: &str| {
            if in_folder {
                item.path.with_file_name(plain)
            } else {
                let stem = item.path.file_stem().unwrap_or_default().to_string_lossy();
                item.path.with_file_name(format!("{stem}.{ext}"))
            }
        };
        let cover_path = file("cover.jpg", "jpg");
        let mut cover_name = None;
        let has_cover = item.cover.is_some() || !item.cover_url.is_empty();
        if cover && has_cover {
            if std::fs::symlink_metadata(&cover_path).is_ok() {
                cover_name = Some(name(&cover_path));
            } else {
                let picture = match item.cover {
                    Some(id) => client.cover(id).await,
                    // The largest Google has.
                    None => {
                        client
                            .gb
                            .cover(&books::google_cover(&item.cover_url, 0))
                            .await
                    }
                };
                match picture.and_then(|b| write_new(&cover_path, &b)) {
                    Ok(()) => {
                        cover_name = Some(name(&cover_path));
                        report.created.push(cover_path.clone());
                    }
                    Err(e) => report.errors.push((name(&cover_path), e)),
                }
            }
        }
        let opf_path = file("metadata.opf", "opf");
        if opf && std::fs::symlink_metadata(&opf_path).is_err() {
            // Without the work's description the file is still worth having.
            let mut work = match item.book.work.is_empty() {
                true => Work::default(),
                false => client.work(&item.book.work).await.unwrap_or_default(),
            };
            if work.description.is_none() && !item.description.trim().is_empty() {
                work.description = Some(books::Text::Plain(item.description.clone()));
            }
            let c = Candidate {
                language: item.language.clone(),
                ..Default::default()
            };
            let text = books::opf(&item.book, &c, &work, cover_name.as_deref());
            match write_new(&opf_path, text.as_bytes()) {
                Ok(()) => report.created.push(opf_path),
                Err(e) => report.errors.push((name(&opf_path), e)),
            }
        }
        let _ = app.emit("book-progress", Progress { done: i + 1, total });
    }
    report
}
