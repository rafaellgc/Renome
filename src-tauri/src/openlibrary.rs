//! Open Library client: book searches by ISBN or by title and author, a
//! few requests a second with the program identified, as the service asks;
//! and the lookups, covers and `metadata.opf` files built on it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures_util::{StreamExt, stream};
use renome_core::books::{self, AuthorPage, BookMatch, Candidate, Guess, Query, SearchPage, Work};
use renome_core::ebook;
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

/// Shared by all lookups, so the pace holds across them.
pub struct Client {
    http: reqwest::Client,
    last: tokio::sync::Mutex<Option<Instant>>,
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
        let res = self.send(&format!("{}{path}", api_base()), query).await?;
        res.json().await.map_err(net_error)
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
    /// An ISBN Open Library does not know is asked of BrasilAPI, which has
    /// most books published in Brazil: that edition comes first, with its
    /// work on Open Library when the search finds it.
    pub async fn find(&self, g: &Guess, lang: &str) -> Result<(Vec<Candidate>, bool), String> {
        let found = self.find_ol(g, lang).await?;
        if found.0.first().is_some_and(|c| c.exact) {
            return Ok(found);
        }
        for isbn in g.isbns.iter().take(3) {
            let Some((edition, ok)) = self
                .brasil(isbn)
                .await
                .and_then(|b| books::brasil_candidate(isbn, &b, g))
            else {
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
        Ok(found)
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
        if cover && let Some(id) = item.cover {
            if std::fs::symlink_metadata(&cover_path).is_ok() {
                cover_name = Some(name(&cover_path));
            } else {
                match client
                    .cover(id)
                    .await
                    .and_then(|b| write_new(&cover_path, &b))
                {
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
            let work = match item.book.work.is_empty() {
                true => Work::default(),
                false => client.work(&item.book.work).await.unwrap_or_default(),
            };
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
