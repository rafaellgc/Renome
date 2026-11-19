//! A file's cover compared with the catalogue's (the window's reviews and
//! pickers, the command line's choosers): which series or issue has the
//! file's cover, to show it first.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt;
use renome_core::comics::{ComicMatch, Issue, Volume};

use crate::comics::Sources;

/// Covers this close (in the fingerprints' bits) are the same picture.
pub const SAME_COVER: u32 = 12;

/// The fingerprint of a comic's or book's own cover.
pub fn own(path: &Path) -> Option<u64> {
    own_cover(path).map(|(_, h)| h)
}

/// A comic's or book's own cover, and its fingerprint.
pub fn own_cover(path: &Path) -> Option<(Vec<u8>, u64)> {
    let bytes = renome_core::convert::cover(path)?;
    let hash = renome_core::similar::fingerprint(&bytes, "img")?;
    Some((bytes, hash))
}

/// The catalogue covers read so far, by address (`None`: not readable).
pub struct Covers {
    http: reqwest::Client,
    known: tokio::sync::Mutex<HashMap<String, Option<u64>>>,
}

impl Default for Covers {
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
            known: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
}

impl Covers {
    /// How far each cover is from the file's (`None`: not readable).
    pub async fn distances(&self, own: u64, urls: &[String]) -> Vec<Option<u32>> {
        let missing: Vec<String> = {
            let known = self.known.lock().await;
            let mut m: Vec<String> = urls
                .iter()
                .filter(|u| u.starts_with("http") && !known.contains_key(*u))
                .cloned()
                .collect();
            m.sort();
            m.dedup();
            m
        };
        let http = &self.http;
        let found: Vec<(String, Option<u64>)> = futures_util::stream::iter(missing)
            .map(|url| async move {
                let bytes = match http.get(&url).send().await {
                    Ok(r) if r.status().is_success() => r.bytes().await.ok(),
                    _ => None,
                };
                let hash = match bytes {
                    Some(b) => tokio::task::spawn_blocking(move || {
                        renome_core::similar::fingerprint(&b, "img")
                    })
                    .await
                    .ok()
                    .flatten(),
                    None => None,
                };
                (url, hash)
            })
            .buffer_unordered(4)
            .collect()
            .await;
        let mut known = self.known.lock().await;
        known.extend(found);
        urls.iter()
            .map(|u| {
                known
                    .get(u)
                    .copied()
                    .flatten()
                    .map(|h| renome_core::similar::distance(own, h))
            })
            .collect()
    }
}

impl Covers {
    /// The covers that are the file's (another scan or edition of the same
    /// art: within [`SAME_COVER`]), how far; `None` for the others.
    pub async fn same(&self, own: u64, urls: &[String]) -> Vec<Option<u32>> {
        self.distances(own, urls)
            .await
            .into_iter()
            .map(|d| d.filter(|d| *d <= SAME_COVER))
            .collect()
    }
}

/// The cover of a series the file's is compared with: the issue's (or
/// volume's) of the file's number in that series.
pub async fn series_covers(
    sources: &Sources<'_>,
    source: &str,
    key: &str,
    id: &str,
    comic: &ComicMatch,
) -> Result<Vec<String>, String> {
    let issues = match source {
        "mangadex" => {
            return match comic.volume {
                Some(v) => sources.md.volume_covers(id, v).await,
                None => Ok(Vec::new()),
            };
        }
        "comicvine" => sources.cv.issues(key, id).await?.1,
        "metron" => sources.mt.issues(key, id).await?.1,
        _ => return Ok(Vec::new()),
    };
    Ok(renome_core::comics::find_issue(comic, &issues)
        .map(|i| i.cover.clone())
        .filter(|c| !c.is_empty())
        .into_iter()
        .collect())
}

/// Series compared with the file's cover: those with it first (nearest
/// first), each one's distance and the cover it was compared with.
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesByCover {
    pub candidates: Vec<Volume>,
    /// The series with the file's cover, how far, by id.
    pub same: HashMap<String, u32>,
    pub covers: HashMap<String, String>,
    /// Whether they were compared at all.
    pub compared: bool,
    /// Why the covers say little here (a key of the texts): the GCD gives
    /// none to programs; a catalogue of whole series has only the first
    /// volume's.
    pub note: Option<&'static str>,
}

/// The series found, compared with the file's cover: the first nine
/// (each is one or two requests); not on the GCD (no covers), and on the
/// catalogues of whole series only when the file is the first volume
/// (they keep that cover alone); a long series (many pages of issues) is
/// left out. `progress(i, n)` as each is compared.
pub async fn series_by_cover(
    covers: &Covers,
    sources: &Sources<'_>,
    source: &str,
    key: &str,
    own: u64,
    comic: &ComicMatch,
    candidates: Vec<Volume>,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> SeriesByCover {
    let first_only = matches!(source, "anilist" | "mangaupdates");
    let is_first = comic.volume.unwrap_or(1) == 1
        && (comic.number.is_empty() || comic.number.parse::<f64>().ok() == Some(1.0));
    let note = if source == "gcd" {
        Some("coverNoneGcd")
    } else if first_only {
        Some("coverNoneSeries")
    } else {
        None
    };
    if source == "gcd" || (first_only && !is_first) {
        return SeriesByCover {
            candidates,
            note,
            ..Default::default()
        };
    }
    let mut far = HashMap::new();
    let mut pics = HashMap::new();
    let top: Vec<&Volume> = candidates.iter().take(9).collect();
    for (i, v) in top.iter().enumerate() {
        progress(i + 1, top.len());
        let urls: Vec<String> = if first_only {
            (!v.cover.is_empty())
                .then(|| v.cover.clone())
                .into_iter()
                .collect()
        } else if v.count <= 200 {
            series_covers(sources, source, key, &v.id, comic)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let d = if urls.is_empty() {
            Vec::new()
        } else {
            covers.distances(own, &urls).await
        };
        let nearest = d.iter().flatten().min().copied();
        far.insert(v.id.clone(), nearest);
        let at = d
            .iter()
            .position(|x| x.is_some() && *x == nearest)
            .unwrap_or(0);
        if let Some(u) = urls.get(at) {
            pics.insert(v.id.clone(), u.clone());
        }
    }
    let same = |v: &Volume| {
        far.get(&v.id)
            .copied()
            .flatten()
            .is_some_and(|d| d <= SAME_COVER)
    };
    let (mut yes, no): (Vec<Volume>, Vec<Volume>) = candidates.into_iter().partition(same);
    yes.sort_by_key(|v| far.get(&v.id).copied().flatten().unwrap_or(u32::MAX));
    yes.extend(no);
    SeriesByCover {
        candidates: yes,
        same: far
            .into_iter()
            .filter_map(|(id, d)| Some((id, d.filter(|d| *d <= SAME_COVER)?)))
            .collect(),
        covers: pics,
        compared: true,
        note,
    }
}

/// Few issues' covers are compared at once; many, when asked (each is a
/// download).
pub const COMPARE_AT_ONCE: usize = 60;
/// The most issues compared.
pub const COMPARE_MOST: usize = 300;

/// The issues to compare with the file's cover: those with one, the
/// first [`COMPARE_MOST`].
pub fn issue_covers(issues: &[Issue]) -> Vec<String> {
    issues
        .iter()
        .filter(|i| !i.cover.is_empty())
        .take(COMPARE_MOST)
        .map(|i| i.cover.clone())
        .collect()
}

/// A series' issues in the order shown: those with the file's cover
/// first (nearest first), then those nearest the file's number; with
/// `typed`, only those it names (by number, or words of their name).
pub fn issue_order(
    issues: &[Issue],
    own: &ComicMatch,
    far: &HashMap<String, Option<u32>>,
    typed: &str,
) -> Vec<usize> {
    let wanted: Option<f64> = own.number.parse().ok();
    let same = |i: &Issue| {
        far.get(&i.cover)
            .copied()
            .flatten()
            .filter(|d| *d <= SAME_COVER)
    };
    let mut order: Vec<usize> = (0..issues.len()).collect();
    order.sort_by(|&a, &b| {
        let (a, b) = (&issues[a], &issues[b]);
        let by_cover = same(a)
            .unwrap_or(u32::MAX)
            .cmp(&same(b).unwrap_or(u32::MAX));
        let near = |i: &Issue| match (wanted, i.number.parse::<f64>()) {
            (Some(w), Ok(n)) => (n - w).abs(),
            _ => f64::MAX,
        };
        by_cover.then(near(a).total_cmp(&near(b)))
    });
    let typed = fold(typed.trim());
    let typed = typed.trim_start_matches('#');
    if typed.is_empty() {
        return order;
    }
    let number = unpadded(typed);
    order
        .into_iter()
        .filter(|&i| {
            let n = fold(&issues[i].number);
            n == number || n.starts_with(typed) || fold(&issues[i].name).contains(typed)
        })
        .collect()
}

/// The issue a number typed names exactly, if one does.
pub fn issue_typed(issues: &[Issue], typed: &str) -> Option<usize> {
    let t = unpadded(typed.trim().trim_start_matches('#'));
    (!t.is_empty()).then_some(())?;
    issues.iter().position(|i| unpadded(&i.number) == t)
}

/// A number without the zeros before it (`007` is `7`, `0` stays).
fn unpadded(s: &str) -> &str {
    let mut t = s;
    while t.len() > 1 && t.starts_with('0') && t[1..].starts_with(|c: char| c.is_ascii_digit()) {
        t = &t[1..];
    }
    t
}

fn fold(s: &str) -> String {
    renome_core::text::strip_accents(s).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(number: &str, name: &str) -> Issue {
        Issue {
            number: number.into(),
            name: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn issues_nearest_and_typed() {
        let list = vec![issue("1", "Começo"), issue("2", "Meio"), issue("10", "Fim")];
        let own = ComicMatch {
            number: "2".into(),
            ..Default::default()
        };
        assert_eq!(issue_order(&list, &own, &HashMap::new(), ""), vec![1, 0, 2]);
        assert_eq!(issue_order(&list, &own, &HashMap::new(), "1"), vec![0, 2]);
        assert_eq!(issue_order(&list, &own, &HashMap::new(), "#01"), vec![0]);
        assert_eq!(issue_order(&list, &own, &HashMap::new(), "comeco"), vec![0]);
        assert_eq!(issue_typed(&list, "#002"), Some(1));
    }
}
