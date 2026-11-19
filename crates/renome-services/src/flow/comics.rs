//! The comic rule: on Metron, first each file's barcode (recorded in it,
//! or read in its pages when asked); then one series per group of files
//! (the series typed, else each series the files say), picked by itself
//! when only one has that name (and year) and fits, else asked; then each
//! file's issue by its number. Then each comic as the rule names it, with
//! its page in the catalogue.

use super::{Item, Question};
use crate::Host;
use crate::comics::Sources;
use renome_core::comics::{ComicMatch, Issue, Volume};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// What the rule asks of the search.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    /// `gcd`, `metron`, `comicvine`, `mangadex`, `anilist`, `mangaupdates`.
    pub source: String,
    /// The catalogue's key (Comic Vine, Metron).
    #[serde(default)]
    pub key: String,
    /// The series typed in the rule (empty: each the files say).
    #[serde(default)]
    pub series: String,
    /// Publishers left out.
    #[serde(default)]
    pub skip: String,
    /// The catalogues' language (`pt-br`).
    pub lang: String,
    /// The program's language (`pt`): an edition in another one is not
    /// picked by itself.
    pub ui_lang: String,
    #[serde(default)]
    pub by_folder: bool,
    /// Read the barcode printed in the pages (Metron).
    #[serde(default)]
    pub barcode: bool,
}

/// A series with its issues (none from a catalogue of manga series).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Listed {
    pub volume: Volume,
    pub issues: Vec<Issue>,
}

/// What was found for a comic.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub path: PathBuf,
    /// `source:id` of its series.
    pub volume_key: String,
    /// What the file says of itself.
    pub own: ComicMatch,
    /// Its issue's id (`""`: the series alone, with the file's numbers).
    pub choice: String,
    /// Picked with confidence; else its issue is a doubt (the file keeps
    /// the series meanwhile).
    pub sure: bool,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// No comic in the list, or none the catalogues can read.
    pub empty: bool,
    /// The series fetched, by `source:id`.
    pub volumes: HashMap<String, Listed>,
    pub files: Vec<Found>,
    /// The series picked, by `source:` and the folded search text: not
    /// asked again (the ones given, with this search's).
    pub picked: HashMap<String, Volume>,
}

/// Whether a series could hold issue `highest`: one with fewer issues
/// (when the catalogue says how many) is not the files' series.
pub fn fits(v: &Volume, highest: f64) -> bool {
    v.count == 0 || f64::from(v.count) >= highest.floor()
}

/// Series found, those that could hold issue `highest` first (the
/// window's picker and the command line's list order them so).
pub fn by_fit(found: Vec<Volume>, highest: f64) -> Vec<Volume> {
    let (mut yes, no): (Vec<Volume>, Vec<Volume>) =
        found.into_iter().partition(|v| fits(v, highest));
    yes.extend(no);
    yes
}

/// Catalogues of manga series, not of a publisher's issues: the file's
/// volume and chapter stay.
pub fn series_only(source: &str) -> bool {
    matches!(source, "mangadex" | "anilist" | "mangaupdates")
}

/// A name compared: no accents, no case, words only.
fn fold(s: &str) -> String {
    renome_core::text::strip_accents(s)
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The key a group of comics' series is picked under (see
/// [`Outcome::picked`]).
pub fn picked_key(source: &str, query: &str) -> String {
    format!("{source}:{}", fold(query))
}

/// Progress, for the page.
#[derive(Serialize)]
struct Progress<'a> {
    stage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    series: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<String>,
}

fn progress(host: &dyn Host, stage: &'static str, series: Option<&str>, n: Option<String>) {
    crate::report(host, "comic-progress", Progress { stage, series, n });
}

/// A series' details filled in from a fuller answer.
pub fn merge_volume(mut v: Volume, full: Volume) -> Volume {
    if !full.name.is_empty() {
        v.name = full.name;
    }
    v.start_year = full.start_year.or(v.start_year);
    for (to, from) in [
        (&mut v.publisher, full.publisher),
        (&mut v.cover, full.cover),
        (&mut v.country, full.country),
        (&mut v.language, full.language),
        (&mut v.format, full.format),
        (&mut v.writer, full.writer),
    ] {
        if !from.is_empty() {
            *to = from;
        }
    }
    if full.count > 0 {
        v.count = full.count;
    }
    if !full.id.is_empty() {
        v.id = full.id;
    }
    v
}

/// The files the rule looks up: comics, and folders of pictures that
/// become CBZ.
fn targets(items: &[Item], settled: &HashSet<PathBuf>) -> Vec<Item> {
    items
        .iter()
        .filter(|i| {
            (if i.is_dir {
                i.convert
            } else {
                renome_core::comics::is_comic(&i.ext())
            }) && !settled.contains(&i.path)
        })
        .cloned()
        .collect()
}

/// Looks up the comics of `items` not `settled`. A series the flow is not
/// sure of is asked ([`Question::ComicSeries`]).
pub async fn search(
    host: &dyn Host,
    sources: &Sources<'_>,
    p: &Params,
    items: &[Item],
    settled: &HashSet<PathBuf>,
    picked: HashMap<String, Volume>,
) -> Result<Outcome, String> {
    let source = p.source.as_str();
    let mut out = Outcome {
        picked,
        ..Outcome::default()
    };
    let files = targets(items, settled);
    if files.is_empty() {
        out.empty = true;
        return Ok(out);
    }
    progress(host, "reading", None, None);
    let mut owns: Vec<Option<ComicMatch>> = tokio::task::spawn_blocking({
        let files = files.clone();
        let by_folder = p.by_folder;
        move || {
            files
                .iter()
                .map(|f| {
                    let stem = f.lookup_name.as_ref().map(|_| f.read_stem());
                    renome_core::comics::from_path(&f.path, stem.as_deref(), by_folder)
                })
                .collect()
        }
    })
    .await
    .unwrap_or_default();
    let stop = host.stop_signal();
    // On Metron, a barcode tells the issue itself.
    let mut coded: HashSet<PathBuf> = HashSet::new();
    if source == "metron" {
        let mut codes: Vec<Option<String>> = owns
            .iter()
            .map(|o| {
                o.as_ref()
                    .and_then(|o| o.extra.get("gtin"))
                    .map(|c| c.chars().filter(char::is_ascii_digit).collect::<String>())
                    .filter(|c| !c.is_empty())
            })
            .collect();
        if p.barcode && codes.iter().any(Option::is_none) {
            progress(host, "barcodeReading", None, None);
            let unread: Vec<usize> = (0..files.len())
                .filter(|&i| codes[i].is_none() && owns[i].is_some())
                .collect();
            let paths: Vec<PathBuf> = unread.iter().map(|&i| files[i].path.clone()).collect();
            let read = tokio::task::spawn_blocking(move || crate::comics::barcodes(&paths))
                .await
                .unwrap_or_default();
            if stop() {
                return Ok(out);
            }
            for (n, &i) in unread.iter().enumerate() {
                if let (Some(Some(code)), Some(own)) = (read.get(n), owns[i].as_mut()) {
                    codes[i] = Some(code.clone());
                    // Written into the ComicInfo.xml with the rest.
                    own.extra.insert("gtin".into(), code.clone());
                }
            }
        }
        let with_code: Vec<usize> = (0..files.len())
            .filter(|&i| codes[i].is_some() && owns[i].is_some())
            .collect();
        for (n, &i) in with_code.iter().enumerate() {
            if stop() {
                return Ok(out);
            }
            progress(
                host,
                "barcodeLooking",
                None,
                Some(format!("{}/{}", n + 1, with_code.len())),
            );
            let code = codes[i].as_deref().unwrap_or_default();
            let Some(found) = sources.mt.by_code(&p.key, code).await? else {
                continue;
            };
            let vkey = format!("{source}:{}", found.volume.id);
            // The code tells the issue: its number too, when the name has none.
            let mut own = owns[i].clone().unwrap_or_default();
            if own.number.is_empty()
                && let Some(x) = found.issues.iter().find(|x| x.id == found.issue)
            {
                own.number = x.number.clone();
            }
            out.files.push(Found {
                path: files[i].path.clone(),
                volume_key: vkey.clone(),
                own,
                choice: found.issue.clone(),
                sure: true,
            });
            out.volumes.insert(
                vkey,
                Listed {
                    volume: found.volume,
                    issues: found.issues,
                },
            );
            coded.insert(files[i].path.clone());
        }
    }
    // The rest, by series.
    let typed = p.series.trim();
    let mut groups: Vec<(String, String, Vec<usize>)> = Vec::new();
    for (i, f) in files.iter().enumerate() {
        let Some(own) = &owns[i] else { continue };
        if coded.contains(&f.path) {
            continue;
        }
        let query = if typed.is_empty() {
            own.series.clone()
        } else {
            typed.to_string()
        };
        let k = fold(&query);
        match groups.iter_mut().find(|g| g.0 == k) {
            Some(g) => g.2.push(i),
            None => groups.push((k, query, vec![i])),
        }
    }
    if groups.is_empty() {
        out.empty = coded.is_empty();
        return Ok(out);
    }
    for (k, query, members) in groups {
        if stop() {
            break;
        }
        let gowns: Vec<ComicMatch> = members.iter().filter_map(|&i| owns[i].clone()).collect();
        // Loose chapters are no issues in the catalogues of issues.
        if !series_only(source)
            && gowns
                .iter()
                .all(|o| o.number.is_empty() && o.volume.is_none() && !o.chapter.is_empty())
        {
            continue;
        }
        let picked_key = picked_key(source, &query);
        let mut volume = out.picked.get(&picked_key).cloned();
        // A `ComicInfo.xml` link to a Comic Vine issue tells the series.
        if volume.is_none()
            && source == "comicvine"
            && let Some(linked) = gowns.iter().find(|o| !o.id.is_empty())
            && let Ok(id) = sources.cv.volume_of(&p.key, &linked.id).await
        {
            volume = Some(Volume {
                id,
                name: query.clone(),
                ..Default::default()
            });
        }
        if volume.is_none() {
            progress(host, "searching", Some(&query), None);
            let mut found = sources
                .search(source, &p.key, &query, None, "", &p.lang, &p.skip)
                .await?;
            // `Series: Story` may be a story in `Series`: the series alone.
            let head = query
                .split([':', '–'])
                .next()
                .unwrap_or_default()
                .split(" - ")
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            if found.is_empty() && !head.is_empty() && head != query.trim() {
                found = sources
                    .search(source, &p.key, &head, None, "", &p.lang, &p.skip)
                    .await?;
            }
            if stop() {
                break;
            }
            let year = gowns.iter().find_map(|o| o.series_year);
            // A common name may leave the series out of what the catalogue
            // answers: asked again with the files' year (theirs, or their
            // issues'), whose series come first.
            let dated = year.or_else(|| gowns.iter().find_map(|o| o.year));
            if let Some(d) = dated
                && !found.iter().any(|v| fold(&v.name) == k)
            {
                let more = sources
                    .search(source, &p.key, &query, Some(d), "", &p.lang, &p.skip)
                    .await
                    .unwrap_or_default();
                if stop() {
                    break;
                }
                let mut merged = more.clone();
                merged.extend(
                    found
                        .into_iter()
                        .filter(|v| !more.iter().any(|m| m.id == v.id)),
                );
                found = merged;
            }
            // The highest number among the files: a series with fewer
            // issues is not theirs.
            let highest = gowns
                .iter()
                .filter_map(|o| {
                    let n = if !o.number.is_empty() {
                        o.number.clone()
                    } else if o.chapter.is_empty() {
                        o.volume.map(|v| v.to_string()).unwrap_or_default()
                    } else {
                        String::new()
                    };
                    n.parse::<f64>().ok()
                })
                .fold(0.0, f64::max);
            let fits = |v: &Volume| fits(v, highest);
            let sorted = by_fit(found.clone(), highest);
            let same: Vec<&Volume> = sorted
                .iter()
                .filter(|v| {
                    fold(&v.name) == k && (year.is_none() || v.start_year == year) && fits(v)
                })
                .collect();
            // The same name in another language (a French edition for a
            // Brazilian reader, when there is one in Portuguese) is not
            // picked by itself.
            let mine = |v: &Volume| {
                v.language.is_empty() || v.language.to_lowercase().starts_with(&p.ui_lang)
            };
            let foreign = same.len() == 1 && !mine(same[0]) && found.iter().any(mine);
            // Without the files' year, the name alone does not tell one run
            // from another: asked when there is anything else to pick.
            let sure = year.is_some() || found.len() == 1;
            volume = if same.len() == 1 && !foreign && sure {
                Some(same[0].clone())
            } else {
                // The file whose cover is compared: the first with a number.
                let at = gowns
                    .iter()
                    .position(|o| !o.number.is_empty() || o.volume.is_some())
                    .unwrap_or(0);
                let q = Question::ComicSeries {
                    source: source.into(),
                    skip: p.skip.clone(),
                    query: query.clone(),
                    files: members.len(),
                    highest,
                    candidates: sorted,
                    path: members.get(at).map(|&i| files[i].path.clone()),
                    comic: gowns.get(at).cloned().map(Box::new),
                };
                host.ask(q)
                    .await
                    .and_then(|v| serde_json::from_value(v).ok())
            };
            if volume.is_none() {
                continue;
            }
        }
        let Some(mut volume) = volume else { continue };
        out.picked.insert(picked_key, volume.clone());
        // The manga catalogues have series, not a publisher's volumes: the
        // file's numbers stay. MangaUpdates tells a series' authors and
        // volumes only when asked for it.
        if series_only(source) {
            if source == "mangaupdates"
                && let Ok(full) = sources.issues(source, &p.key, &volume.id, &gowns).await
            {
                volume = merge_volume(volume, full.volume);
            }
            if stop() {
                break;
            }
            let vkey = format!("{source}:{}", volume.id);
            for &i in &members {
                if let Some(own) = &owns[i] {
                    out.files.push(Found {
                        path: files[i].path.clone(),
                        volume_key: vkey.clone(),
                        own: own.clone(),
                        choice: String::new(),
                        sure: true,
                    });
                }
            }
            out.volumes.insert(
                vkey,
                Listed {
                    volume,
                    issues: Vec::new(),
                },
            );
            continue;
        }
        if stop() {
            break;
        }
        progress(host, "loading", Some(&volume.name), None);
        let res = sources.issues(source, &p.key, &volume.id, &gowns).await?;
        let vkey = format!("{source}:{}", res.volume.id);
        // The search's own details where the series' lack them (a cover).
        let volume = merge_volume(volume, res.volume);
        let mut n = 0;
        for &i in &members {
            let Some(own) = &owns[i] else { continue };
            let found = res.found.get(n).cloned().flatten();
            n += 1;
            // A loose chapter is no issue there: the series is all there is.
            let chapter_only = own.number.is_empty() && !own.chapter.is_empty();
            out.files.push(Found {
                path: files[i].path.clone(),
                volume_key: vkey.clone(),
                own: own.clone(),
                sure: found.is_some() || chapter_only,
                // A file without its issue still takes the series, so a
                // series' files stay together; its issue can still be picked.
                choice: found.unwrap_or_default(),
            });
        }
        out.volumes.insert(
            vkey,
            Listed {
                volume,
                issues: res.issues,
            },
        );
    }
    Ok(out)
}

pub use renome_core::comics::with_issue;

/// The catalogue page of a series or issue.
pub fn web(source: &str, volume: &str, issue: &str) -> String {
    match source {
        "comicvine" if !issue.is_empty() => {
            format!("https://comicvine.gamespot.com/issue/4000-{issue}/")
        }
        "comicvine" => format!("https://comicvine.gamespot.com/volume/4050-{volume}/"),
        "gcd" if !issue.is_empty() => format!("https://www.comics.org/issue/{issue}/"),
        "gcd" => format!("https://www.comics.org/series/{volume}/"),
        "mangadex" => format!("https://mangadex.org/title/{volume}"),
        "anilist" => format!("https://anilist.co/manga/{volume}"),
        "mangaupdates" => volume
            .parse::<u64>()
            .map(|n| format!("https://www.mangaupdates.com/series/{}", to_base36(n)))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn to_base36(mut n: u64) -> String {
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    loop {
        out.push(digits[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// A comic's series and issue as they stand.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settled {
    pub path: PathBuf,
    pub volume_key: String,
    pub own: ComicMatch,
    /// `None`: no issue (none fits); `""`: the series alone.
    pub choice: Option<String>,
}

/// A comic as the rule names it, with its page in the catalogue and
/// whether that is a manga catalogue's (for `ComicInfo.xml`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Named {
    pub comic: ComicMatch,
    pub web: String,
    pub manga: bool,
}

/// Each comic's details from its series and issue.
pub fn named(files: &[Settled], volumes: &HashMap<String, Listed>) -> HashMap<PathBuf, Named> {
    files
        .iter()
        .filter_map(|f| {
            let listed = volumes.get(&f.volume_key)?;
            let choice = f.choice.as_ref()?;
            let issue = if choice.is_empty() {
                None
            } else {
                Some(listed.issues.iter().find(|i| &i.id == choice)?)
            };
            let source = f.volume_key.split_once(':').map_or("", |(s, _)| s);
            Some((
                f.path.clone(),
                Named {
                    comic: with_issue(&f.own, &listed.volume, issue),
                    web: web(source, &listed.volume.id, choice),
                    manga: series_only(source),
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_in_the_catalogues() {
        assert_eq!(web("gcd", "77", "901"), "https://www.comics.org/issue/901/");
        assert_eq!(
            web("mangaupdates", "36", ""),
            "https://www.mangaupdates.com/series/10"
        );
    }
}
