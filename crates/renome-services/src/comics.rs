//! The comic catalogues behind one door: series searched, a series'
//! issues fetched and matched to the files, barcodes read, for the window
//! and the command line alike.

use renome_core::sync::MutexExt;
use std::path::PathBuf;

use renome_core::comics::{ComicMatch, Issue, Volume};
use renome_core::links::{self, Link};
use serde::Serialize;

use crate::{anilist, comicvine, gcd, mangadex, mangaupdates, metron};

/// The catalogues, as whoever holds them keeps them.
pub struct Sources<'a> {
    pub cv: &'a comicvine::Client,
    pub mt: &'a metron::Client,
    pub gcd: &'a gcd::Client,
    pub md: &'a mangadex::Client,
    pub al: &'a anilist::Client,
    pub mu: &'a mangaupdates::Client,
}

/// A series and its issues, with the issue of each comic asked about when
/// its number says it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComicIssues {
    pub volume: Volume,
    pub issues: Vec<Issue>,
    pub found: Vec<Option<String>>,
}

impl Sources<'_> {
    /// Series by name (a link pasted: that series, from the catalogue the
    /// rule uses), the publishers in `skip` left out.
    #[allow(clippy::too_many_arguments)]
    pub async fn search(
        &self,
        source: &str,
        key: &str,
        query: &str,
        year: Option<u16>,
        publisher: &str,
        lang: &str,
        skip: &str,
    ) -> Result<Vec<Volume>, String> {
        if let Some(link) = links::parse(query) {
            let ours = if source.is_empty() { "gcd" } else { source };
            if link.comic_source() != Some(ours) {
                return Err("linkOther".into());
            }
            let v = match link {
                Link::ComicVine { issue, id } => {
                    let id = match issue {
                        true => self.cv.volume_of(key, &id).await?,
                        false => id,
                    };
                    self.cv.issues(key, &id).await?.0
                }
                Link::Metron(id) => self.mt.issues(key, &id).await?.0,
                Link::Gcd(id) => self.gcd.issues(&id).await?.0,
                Link::MangaDex(id) => self.md.series(&id, lang).await?,
                Link::AniList { id, .. } => self.al.manga(id).await?,
                Link::MangaUpdates(id) => self.mu.series(&id, "").await?,
                _ => return Err("linkOther".into()),
            };
            return Ok(vec![v]);
        }
        let found = match source {
            "comicvine" => self.cv.search(key, query, year, publisher, lang).await,
            "metron" => self.mt.search(key, query, year, publisher, lang).await,
            "mangadex" => self.md.search(query, year, lang).await,
            "anilist" => self.al.search(query, year).await,
            "mangaupdates" => self.mu.search(query, year).await,
            _ => self.gcd.search(query, year, publisher, lang).await,
        }?;
        Ok(renome_core::comics::skip_publishers(found, skip))
    }

    /// A series' issues, and each comic's among them.
    pub async fn issues(
        &self,
        source: &str,
        key: &str,
        id: &str,
        comics: &[ComicMatch],
    ) -> Result<ComicIssues, String> {
        let (volume, issues) = match source {
            "comicvine" => self.cv.issues(key, id).await?,
            "metron" => self.mt.issues(key, id).await?,
            "mangaupdates" => {
                let query = comics
                    .first()
                    .map(|c| c.series.as_str())
                    .unwrap_or_default();
                (self.mu.series(id, query).await?, Vec::new())
            }
            _ => self.gcd.issues(id).await?,
        };
        let found = comics
            .iter()
            .map(|m| renome_core::comics::find_issue(m, &issues).map(|i| i.id.clone()))
            .collect();
        Ok(ComicIssues {
            volume,
            issues,
            found,
        })
    }
}

/// The barcodes printed on comics (read in their first and last pages),
/// a few at a time.
pub fn barcodes(paths: &[PathBuf]) -> Vec<Option<String>> {
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get().min(6));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(vec![None; paths.len()]);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(p) = paths.get(i) else { break };
                    let code = renome_core::barcode::of_comic(p);
                    out.locked()[i] = code;
                }
            });
        }
    });
    out.into_inner().unwrap_or_default()
}
