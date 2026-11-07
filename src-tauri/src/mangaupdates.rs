//! MangaUpdates client: manga series by title, with no key. Its terms
//! ask to credit MangaUpdates, to space requests and to keep answers, so
//! requests are paced and answers kept. Like MangaDex, only the series is
//! looked up: the volume or chapter stays the one in the file. The search
//! gives names, years and covers; the series picked is asked again for its
//! authors, publisher and number of volumes.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use renome_core::comics::Volume;
use serde_json::{Value, json};

const API: &str = "https://api.mangaupdates.com/v1";

/// Tests point this at a local server with `RENOME_MANGAUPDATES_API`.
fn api_base() -> String {
    std::env::var("RENOME_MANGAUPDATES_API").unwrap_or_else(|_| API.into())
}

pub struct Client {
    http: reqwest::Client,
    last: tokio::sync::Mutex<Option<Instant>>,
    cache: tokio::sync::Mutex<HashMap<String, Value>>,
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
            cache: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_connect() {
        "muOffline".into()
    } else if e.is_timeout() {
        "muTimeout".into()
    } else {
        format!("muNetwork: {}", e.without_url())
    }
}

/// Kinds that are not the series itself: fan works and prose.
fn not_comic(kind: &str) -> bool {
    matches!(
        kind.to_lowercase().as_str(),
        "doujinshi" | "novel" | "artbook"
    )
}

/// The number of volumes in a status like `15 Volumes (Hiatus)`.
fn volumes_in(status: &str) -> u32 {
    let mut words = status.split_whitespace();
    match (words.next(), words.next()) {
        (Some(n), Some(w)) if w.to_lowercase().starts_with("volume") => n.parse().unwrap_or(0),
        _ => 0,
    }
}

fn found(r: &Value) -> Volume {
    let s = |v: &Value| v.as_str().unwrap_or_default().trim().to_string();
    Volume {
        id: r["series_id"].as_u64().unwrap_or_default().to_string(),
        name: s(&r["title"]),
        start_year: s(&r["year"]).parse().ok(),
        cover: s(&r["image"]["url"]["thumb"]),
        format: s(&r["type"]).to_lowercase(),
        ..Default::default()
    }
}

/// The series' details over what the search gave.
fn details(d: &Value, query: &str) -> Volume {
    let s = |v: &Value| v.as_str().unwrap_or_default().trim().to_string();
    let mut v = found(d);
    let others: Vec<&str> = d["associated"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a["title"].as_str())
        .collect();
    let main = s(&d["title"]);
    v.name = crate::anilist::pick_title(query, &[&main], &others);
    v.cover = s(&d["image"]["url"]["thumb"]);
    let list = |field: &str| d[field].as_array().into_iter().flatten();
    v.writer = list("authors")
        .filter(|a| a["type"].as_str() == Some("Author"))
        .filter_map(|a| a["name"].as_str())
        .collect::<Vec<_>>()
        .join(", ");
    v.publisher = list("publishers")
        .find(|p| p["type"].as_str() == Some("Original"))
        .map(|p| s(&p["publisher_name"]))
        .unwrap_or_default();
    v.count = volumes_in(&s(&d["status"]));
    v
}

impl Client {
    /// A question asked a second apart at least, its answer kept.
    async fn ask(&self, id: &str, body: Option<Value>) -> Result<Value, String> {
        if let Some(v) = self.cache.lock().await.get(id) {
            return Ok(v.clone());
        }
        if let Some(v) = crate::kept::<Value>("mangaupdates", id) {
            self.cache.lock().await.insert(id.into(), v.clone());
            return Ok(v);
        }
        let mut last = self.last.lock().await;
        if let Some(t) = *last {
            tokio::time::sleep(Duration::from_millis(1000).saturating_sub(t.elapsed())).await;
        }
        let res = match &body {
            Some(b) => {
                self.http
                    .post(format!("{}/series/search", api_base()))
                    .json(b)
                    .send()
                    .await
            }
            None => {
                self.http
                    .get(format!("{}/series/{id}", api_base()))
                    .send()
                    .await
            }
        };
        *last = Some(Instant::now());
        drop(last);
        let res = res.map_err(net_error)?;
        match res.status().as_u16() {
            200 => {}
            404 => return Err("muNotFound".into()),
            429 => return Err("muBusy".into()),
            code => return Err(format!("muNetwork: HTTP {code}")),
        }
        let bytes = res.bytes().await.map_err(net_error)?;
        let v: Value =
            crate::keep("mangaupdates", id, &bytes).map_err(|e| format!("muNetwork: {e}"))?;
        self.cache.lock().await.insert(id.into(), v.clone());
        Ok(v)
    }

    /// Manga with a title, from `year` when given, the closest first.
    pub async fn search(&self, query: &str, year: Option<u16>) -> Result<Vec<Volume>, String> {
        let v = self
            .ask(
                &format!("search {query}"),
                Some(json!({ "search": query, "perpage": 25 })),
            )
            .await?;
        let found: Vec<Volume> = v["results"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|r| &r["record"])
            .filter(|r| !not_comic(r["type"].as_str().unwrap_or_default()))
            .map(found)
            .collect();
        let found = renome_core::comics::filter_volumes(found, year, "");
        Ok(renome_core::comics::rank_volumes(query, found, ""))
    }

    /// A series with its authors, publisher and number of volumes;
    /// `query` is what it was found by (its name among the series' titles
    /// is kept).
    pub async fn series(&self, id: &str, query: &str) -> Result<Volume, String> {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err("muNotFound".into());
        }
        let d = self.ask(id, None).await?;
        Ok(details(&d, query))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn details_give_authors_publisher_and_volumes() {
        let d: Value = serde_json::from_str(
            r#"{"series_id":42,"title":"Hoshi no Tabi","type":"Manga","year":"2020",
            "image":{"url":{"thumb":"https://x/t.jpg"}},
            "associated":[{"title":"Jornada das Estrelas"}],"status":"15 Volumes (Hiatus)",
            "authors":[{"name":"LIMA Ana","type":"Author"},{"name":"SA Rui","type":"Artist"}],
            "publishers":[{"publisher_name":"Editora Um","type":"Original"},
            {"publisher_name":"Two Press","type":"English"}]}"#,
        )
        .unwrap();
        let v = details(&d, "Jornada das Estrelas");
        assert_eq!(v.name, "Jornada das Estrelas");
        assert_eq!(
            (v.id.as_str(), v.start_year, v.count),
            ("42", Some(2020), 15)
        );
        assert_eq!(
            (v.writer.as_str(), v.publisher.as_str()),
            ("LIMA Ana", "Editora Um")
        );
        assert_eq!(details(&d, "x").name, "Hoshi no Tabi");
        assert_eq!(volumes_in("Ongoing"), 0);
        assert!(not_comic("Doujinshi") && !not_comic("Manhwa"));
    }
}

/// Asks the real services (`RENOME_LIVE=1`): AniList and MangaUpdates.
#[cfg(test)]
mod live {
    #[test]
    fn real_answers() {
        if std::env::var("RENOME_LIVE").is_err() {
            return;
        }
        tauri::async_runtime::block_on(async {
            let al = crate::anilist::Client::default();
            let found = al.search("Sousou no Frieren", None).await.unwrap();
            println!(
                "anilist manga: {:?}",
                found
                    .iter()
                    .take(3)
                    .map(|v| (&v.name, v.start_year, v.count, &v.writer))
                    .collect::<Vec<_>>()
            );
            let shows = al.anime_search("Frieren", &[]).await.unwrap();
            println!(
                "anilist anime: {} {:?} auto={}",
                shows.query,
                shows
                    .candidates
                    .iter()
                    .take(4)
                    .map(|c| (c.id, &c.title, c.year))
                    .collect::<Vec<_>>(),
                shows.auto
            );
            let look = al
                .anime_lookup(shows.candidates[0].id, &[], "Frieren")
                .await
                .unwrap();
            let seasons: std::collections::BTreeMap<u16, usize> =
                look.show
                    .episodes
                    .iter()
                    .fold(Default::default(), |mut m, e| {
                        *m.entry(e.season).or_default() += 1;
                        m
                    });
            println!(
                "show {} / {} seasons {:?} first {:?}",
                look.show.name,
                look.show.original_name,
                seasons,
                look.show.episodes.first().map(|e| (&e.name, &e.air_date))
            );
            let mu = super::Client::default();
            let found = mu.search("Sousou no Frieren", None).await.unwrap();
            println!(
                "mu: {:?}",
                found
                    .iter()
                    .take(3)
                    .map(|v| (&v.id, &v.name, v.start_year))
                    .collect::<Vec<_>>()
            );
            let v = mu
                .series(&found[0].id, "Frieren e a Jornada para o Além")
                .await
                .unwrap();
            println!(
                "mu series: {} {:?} {} {} {}",
                v.name, v.start_year, v.count, v.writer, v.publisher
            );
        });
    }
}

/// Pasted links fetched from the real services (`RENOME_LIVE=1`), those
/// that need no key.
#[cfg(test)]
mod live_links {
    use renome_core::links::{Link, parse};

    #[test]
    fn real_links() {
        if std::env::var("RENOME_LIVE").is_err() {
            return;
        }
        tauri::async_runtime::block_on(async {
            let Some(Link::MangaUpdates(id)) =
                parse("https://www.mangaupdates.com/series/pb8uwds/one-piece")
            else {
                panic!()
            };
            let v = crate::mangaupdates::Client::default()
                .series(&id, "")
                .await
                .unwrap();
            println!("mangaupdates: {} {:?} {}", v.name, v.start_year, v.count);
            let al = crate::anilist::Client::default();
            let v = al.manga(30013).await.unwrap();
            println!("anilist manga: {} {:?} {}", v.name, v.start_year, v.writer);
            println!("anilist bad: {:?}", al.manga(999_999_999).await.err());
            let md = crate::mangadex::Client::default();
            let v = md
                .series("a1c7c817-4e59-43b7-9365-09675a149a6f", "pt-BR")
                .await
                .unwrap();
            println!("mangadex: {} {:?} {}", v.name, v.start_year, v.writer);
            let ol = crate::openlibrary::Client::default();
            for l in [
                "https://openlibrary.org/works/OL45804W",
                "https://openlibrary.org/books/OL7353617M",
                "978-0-14-032872-1",
            ] {
                let c = ol.by_link(&parse(l).unwrap(), "pt-BR").await.unwrap();
                let c = &c[0];
                println!(
                    "openlibrary {l}: {} / {:?} / {} / {}",
                    c.title, c.authors, c.edition, c.isbn
                );
            }
            let mb = crate::musicbrainz::Client::default();
            let r = mb
                .release("b84ee12a-09ef-421b-82de-0441a926375b")
                .await
                .unwrap();
            let c = renome_core::musicbrainz::summary(&r, 1.0);
            println!(
                "musicbrainz: {} - {} {} {}",
                c.artist, c.title, c.date, c.tracks
            );
        });
    }
}
