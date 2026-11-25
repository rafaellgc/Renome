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
    pub status: super::Status,
    /// What was read from the file (see [`read`]).
    pub read: String,
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

/// A name compared: no accents, no case, words only (the core's).
use renome_core::text::fold_words as fold;

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

/// Why a catalogue cannot be asked with this key: Comic Vine and Metron
/// take none (the text's key), the others need none.
pub fn needs_key(source: &str, key: &str) -> Option<&'static str> {
    match source {
        "comicvine" if key.trim().is_empty() => Some("cvNeedsKey"),
        "metron" if key.trim().is_empty() => Some("mtNeedsKey"),
        _ => None,
    }
}

/// What a stage of the search says to the next: go on, or the person asked
/// to stop and what is found so far is the answer.
#[derive(PartialEq)]
enum Next {
    On,
    Stop,
}

/// What the stages share: who runs the work, the catalogues, the rule's
/// options, the files and what each says of itself.
struct Search<'a> {
    host: &'a dyn Host,
    sources: &'a Sources<'a>,
    p: &'a Params,
    files: Vec<Item>,
    owns: Vec<Option<ComicMatch>>,
    stop: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
}

/// The files that say the same series, by the name's folded text.
struct Group {
    key: String,
    query: String,
    members: Vec<usize>,
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
    if let Some(why) = needs_key(&p.source, &p.key) {
        return Err(why.to_string());
    }
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
    let owns = read_owns(&files, p.by_folder).await;
    let mut s = Search {
        host,
        sources,
        p,
        files,
        owns,
        stop: host.stop_signal(),
    };
    let mut coded: HashSet<PathBuf> = HashSet::new();
    // On Metron, a barcode tells the issue itself.
    if p.source == "metron" && s.by_barcodes(&mut out, &mut coded).await? == Next::Stop {
        return Ok(out);
    }
    let groups = s.groups(&coded);
    if groups.is_empty() {
        out.empty = coded.is_empty();
        return Ok(out);
    }
    for g in groups {
        if (s.stop)() || s.series_group(&g, &mut out).await? == Next::Stop {
            break;
        }
    }
    Ok(out)
}

/// What each file says of itself, read off the thread.
async fn read_owns(files: &[Item], by_folder: bool) -> Vec<Option<ComicMatch>> {
    let files = files.to_vec();
    tokio::task::spawn_blocking(move || {
        files
            .iter()
            .map(|f| {
                let stem = f.lookup_name.as_ref().map(|_| f.read_stem());
                renome_core::comics::from_path(&f.path, stem.as_deref(), by_folder)
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

impl Search<'_> {
    /// The digits of the barcode each file records, when it does.
    fn recorded_codes(&self) -> Vec<Option<String>> {
        self.owns
            .iter()
            .map(|o| {
                o.as_ref()
                    .and_then(|o| o.extra.get("gtin"))
                    .map(|c| c.chars().filter(char::is_ascii_digit).collect::<String>())
                    .filter(|c| !c.is_empty())
            })
            .collect()
    }

    /// Reads the barcodes printed in the pages of the files that record
    /// none, and notes them in what the files say.
    async fn read_printed_codes(&mut self, codes: &mut [Option<String>]) -> Next {
        progress(self.host, "barcodeReading", None, None);
        let unread: Vec<usize> = (0..self.files.len())
            .filter(|&i| codes[i].is_none() && self.owns[i].is_some())
            .collect();
        let paths: Vec<PathBuf> = unread.iter().map(|&i| self.files[i].path.clone()).collect();
        let read = tokio::task::spawn_blocking(move || crate::comics::barcodes(&paths))
            .await
            .unwrap_or_default();
        if (self.stop)() {
            return Next::Stop;
        }
        for (n, &i) in unread.iter().enumerate() {
            if let (Some(Some(code)), Some(own)) = (read.get(n), self.owns[i].as_mut()) {
                codes[i] = Some(code.clone());
                // Written into the ComicInfo.xml with the rest.
                own.extra.insert("gtin".into(), code.clone());
            }
        }
        Next::On
    }

    /// Each file with a barcode, asked of Metron: its issue and series.
    async fn by_barcodes(
        &mut self,
        out: &mut Outcome,
        coded: &mut HashSet<PathBuf>,
    ) -> Result<Next, String> {
        let mut codes = self.recorded_codes();
        if self.p.barcode
            && codes.iter().any(Option::is_none)
            && self.read_printed_codes(&mut codes).await == Next::Stop
        {
            return Ok(Next::Stop);
        }
        let with_code: Vec<usize> = (0..self.files.len())
            .filter(|&i| codes[i].is_some() && self.owns[i].is_some())
            .collect();
        for (n, &i) in with_code.iter().enumerate() {
            if (self.stop)() {
                return Ok(Next::Stop);
            }
            let step = Some(format!("{}/{}", n + 1, with_code.len()));
            progress(self.host, "barcodeLooking", None, step);
            let code = codes[i].as_deref().unwrap_or_default();
            let Some(found) = self.sources.mt.by_code(&self.p.key, code).await? else {
                continue;
            };
            let vkey = format!("{}:{}", self.p.source, found.volume.id);
            // The code tells the issue: its number too, when the name has none.
            let mut own = self.owns[i].clone().unwrap_or_default();
            if own.number.is_empty()
                && let Some(x) = found.issues.iter().find(|x| x.id == found.issue)
            {
                own.number = x.number.clone();
            }
            out.files.push(Found {
                path: self.files[i].path.clone(),
                volume_key: vkey.clone(),
                read: read(&own),
                own,
                choice: found.issue.clone(),
                sure: true,
                status: super::Status::Auto,
            });
            out.volumes.insert(
                vkey,
                Listed {
                    volume: found.volume,
                    issues: found.issues,
                },
            );
            coded.insert(self.files[i].path.clone());
        }
        Ok(Next::On)
    }

    /// The files not found by their barcode, grouped by the series they
    /// say (the series typed, when there is one).
    fn groups(&self, coded: &HashSet<PathBuf>) -> Vec<Group> {
        let typed = self.p.series.trim();
        let mut groups: Vec<Group> = Vec::new();
        for (i, f) in self.files.iter().enumerate() {
            let Some(own) = &self.owns[i] else { continue };
            if coded.contains(&f.path) {
                continue;
            }
            let query = if typed.is_empty() {
                own.series.clone()
            } else {
                typed.to_string()
            };
            let key = fold(&query);
            match groups.iter_mut().find(|g| g.key == key) {
                Some(g) => g.members.push(i),
                None => groups.push(Group {
                    key,
                    query,
                    members: vec![i],
                }),
            }
        }
        groups
    }

    /// One series' files: the series picked or asked, then its issues.
    async fn series_group(&self, g: &Group, out: &mut Outcome) -> Result<Next, String> {
        let source = self.p.source.as_str();
        let gowns: Vec<ComicMatch> = g
            .members
            .iter()
            .filter_map(|&i| self.owns[i].clone())
            .collect();
        // Loose chapters are no issues in the catalogues of issues.
        if !series_only(source)
            && gowns
                .iter()
                .all(|o| o.number.is_empty() && o.volume.is_none() && !o.chapter.is_empty())
        {
            return Ok(Next::On);
        }
        let picked_key = picked_key(source, &g.query);
        let mut volume = out.picked.get(&picked_key).cloned();
        if volume.is_none() {
            volume = self.linked_volume(g, &gowns).await;
        }
        if volume.is_none() {
            progress(self.host, "searching", Some(&g.query), None);
            let Some(picked) = self.pick_volume(g, &gowns).await? else {
                return Ok(Next::Stop);
            };
            volume = picked;
            if volume.is_none() {
                return Ok(Next::On);
            }
        }
        let Some(volume) = volume else {
            return Ok(Next::On);
        };
        out.picked.insert(picked_key, volume.clone());
        // The manga catalogues have series, not a publisher's volumes: the
        // file's numbers stay.
        if series_only(source) {
            return self.add_series_only(g, &gowns, volume, out).await;
        }
        if (self.stop)() {
            return Ok(Next::Stop);
        }
        progress(self.host, "loading", Some(&volume.name), None);
        self.add_issues(g, &gowns, volume, out).await
    }

    /// A `ComicInfo.xml` link to a Comic Vine issue tells the series.
    async fn linked_volume(&self, g: &Group, gowns: &[ComicMatch]) -> Option<Volume> {
        if self.p.source != "comicvine" {
            return None;
        }
        let linked = gowns.iter().find(|o| !o.id.is_empty())?;
        let id = self
            .sources
            .cv
            .volume_of(&self.p.key, &linked.id)
            .await
            .ok()?;
        Some(Volume {
            id,
            name: g.query.clone(),
            ..Default::default()
        })
    }

    /// The catalogue's series for a name (from `year`, when given).
    async fn lookup(&self, name: &str, year: Option<u16>) -> Result<Vec<Volume>, String> {
        let p = self.p;
        self.sources
            .search(&p.source, &p.key, name, year, "", &p.lang, &p.skip)
            .await
    }

    /// The series the catalogue holds for a name: searched as typed, as its
    /// head (`Series: Story` may be a story in `Series`), and with the
    /// files' year when the name alone did not bring it.
    /// `None`: the person asked to stop.
    async fn search_names(
        &self,
        g: &Group,
        gowns: &[ComicMatch],
        year: Option<u16>,
    ) -> Result<Option<Vec<Volume>>, String> {
        let mut found = self.lookup(&g.query, None).await?;
        let head = g
            .query
            .split([':', '–'])
            .next()
            .unwrap_or_default()
            .split(" - ")
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        if found.is_empty() && !head.is_empty() && head != g.query.trim() {
            found = self.lookup(&head, None).await?;
        }
        if (self.stop)() {
            return Ok(None);
        }
        // A common name may leave the series out of what the catalogue
        // answers: asked again with the files' year (theirs, or their
        // issues'), whose series come first.
        let dated = year.or_else(|| gowns.iter().find_map(|o| o.year));
        if let Some(d) = dated
            && !found.iter().any(|v| fold(&v.name) == g.key)
        {
            let more = self.lookup(&g.query, Some(d)).await.unwrap_or_default();
            if (self.stop)() {
                return Ok(None);
            }
            let mut merged = more.clone();
            merged.extend(
                found
                    .into_iter()
                    .filter(|v| !more.iter().any(|m| m.id == v.id)),
            );
            found = merged;
        }
        Ok(Some(found))
    }

    /// Searches the catalogue and picks the series by itself when only one
    /// fits, else asks. `None`: the person asked to stop; a `None` series:
    /// nothing was picked.
    async fn pick_volume(
        &self,
        g: &Group,
        gowns: &[ComicMatch],
    ) -> Result<Option<Option<Volume>>, String> {
        let year = gowns.iter().find_map(|o| o.series_year);
        let Some(found) = self.search_names(g, gowns, year).await? else {
            return Ok(None);
        };
        // The highest number among the files: a series with fewer issues is
        // not theirs.
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
        let sorted = by_fit(found.clone(), highest);
        let same: Vec<&Volume> = sorted
            .iter()
            .filter(|v| {
                fold(&v.name) == g.key
                    && (year.is_none() || v.start_year == year)
                    && fits(v, highest)
            })
            .collect();
        // The same name in another language (a French edition for a
        // Brazilian reader, when there is one in Portuguese) is not picked
        // by itself.
        let ui = &self.p.ui_lang;
        let mine = |v: &Volume| {
            v.language.is_empty() || v.language.to_lowercase().starts_with(ui.as_str())
        };
        let foreign = same.len() == 1 && !mine(same[0]) && found.iter().any(mine);
        // Without the files' year, the name alone does not tell one run from
        // another: asked when there is anything else to pick.
        let sure = year.is_some() || found.len() == 1;
        if same.len() == 1 && !foreign && sure {
            return Ok(Some(Some(same[0].clone())));
        }
        // The file whose cover is compared: the first with a number.
        let at = gowns
            .iter()
            .position(|o| !o.number.is_empty() || o.volume.is_some())
            .unwrap_or(0);
        let q = Question::ComicSeries {
            source: self.p.source.clone(),
            skip: self.p.skip.clone(),
            query: g.query.clone(),
            files: g.members.len(),
            highest,
            candidates: sorted,
            path: g.members.get(at).map(|&i| self.files[i].path.clone()),
            comic: gowns.get(at).cloned().map(Box::new),
        };
        let answer = self
            .host
            .ask(q)
            .await
            .and_then(|v| serde_json::from_value(v).ok());
        Ok(Some(answer))
    }

    /// The series' files in a catalogue of series: the file's numbers stay.
    /// MangaUpdates tells a series' authors and volumes only when asked.
    async fn add_series_only(
        &self,
        g: &Group,
        gowns: &[ComicMatch],
        mut volume: Volume,
        out: &mut Outcome,
    ) -> Result<Next, String> {
        let p = self.p;
        if p.source == "mangaupdates"
            && let Ok(full) = self
                .sources
                .issues(&p.source, &p.key, &volume.id, gowns)
                .await
        {
            volume = merge_volume(volume, full.volume);
        }
        if (self.stop)() {
            return Ok(Next::Stop);
        }
        let vkey = format!("{}:{}", p.source, volume.id);
        for &i in &g.members {
            if let Some(own) = &self.owns[i] {
                out.files.push(Found {
                    path: self.files[i].path.clone(),
                    volume_key: vkey.clone(),
                    own: own.clone(),
                    choice: String::new(),
                    sure: true,
                    status: super::Status::Auto,
                    read: read(own),
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
        Ok(Next::On)
    }

    /// The series' issues, and the issue of each file by its number.
    async fn add_issues(
        &self,
        g: &Group,
        gowns: &[ComicMatch],
        volume: Volume,
        out: &mut Outcome,
    ) -> Result<Next, String> {
        let p = self.p;
        let res = self
            .sources
            .issues(&p.source, &p.key, &volume.id, gowns)
            .await?;
        let vkey = format!("{}:{}", p.source, res.volume.id);
        // The search's own details where the series' lack them (a cover).
        let volume = merge_volume(volume, res.volume);
        let mut n = 0;
        for &i in &g.members {
            let Some(own) = &self.owns[i] else { continue };
            let found = res.found.get(n).cloned().flatten();
            n += 1;
            // A loose chapter is no issue there: the series is all there is.
            let chapter_only = own.number.is_empty() && !own.chapter.is_empty();
            let sure = found.is_some() || chapter_only;
            out.files.push(Found {
                path: self.files[i].path.clone(),
                volume_key: vkey.clone(),
                own: own.clone(),
                sure,
                status: if sure {
                    super::Status::Auto
                } else {
                    super::Status::Choose
                },
                read: read(own),
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
        Ok(Next::On)
    }
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

/// What was read from a comic file, for a review's header and the
/// chooser: `Series v01 #2 c3 (1999)`.
pub fn read(o: &ComicMatch) -> String {
    let mut parts = vec![o.series.clone()];
    if let Some(v) = o.volume {
        parts.push(format!("v{v:02}"));
    }
    if !o.number.is_empty() {
        parts.push(format!("#{}", o.number));
    }
    if !o.chapter.is_empty() {
        parts.push(format!("c{}", o.chapter));
    }
    if let Some(y) = o.year {
        parts.push(format!("({y})"));
    }
    parts.join(" ")
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
