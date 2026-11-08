//! The same movie, episode, comic or book in more than one file: two
//! qualities of a film, an issue as CBR and as CBZ, a book as EPUB and as
//! PDF. Told offline by what names and files say (title and year, show
//! and episode, series and number, ISBN, the words of a book's name),
//! never by the content itself; identical copies are shown as such and
//! are not counted again. For each group, the file worth keeping when one
//! stands out: the sharpest video, the CBZ or larger comic, the most
//! usable book format.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::files::{FileEntry, natural_cmp};

/// What the files of a group are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Movie,
    Episode,
    Comic,
    Book,
}

/// One file of a group and what tells it apart.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub path: PathBuf,
    pub size: u64,
    /// A video's picture (`1920×1080`) and codec, when read.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub picture: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub codec: String,
}

/// Files that are the same work.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub kind: Kind,
    /// Sorted by path.
    pub items: Vec<Item>,
    /// The one worth keeping, when one stands out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best: Option<PathBuf>,
}

/// Words that tell a work apart: no accents, case, punctuation or
/// bracketed notes; release and format words left out.
fn words(s: &str) -> Vec<String> {
    static NOISE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)^(?:epub|pdf|mobi|azw3?|kepub|fb2|ebook|e-book|retail|scan|scans|ocr|digital|livro|libro|book|cbz|cbr)$",
        )
        .unwrap()
    });
    let s = crate::tmdb::BRACKETS.replace_all(s, " ");
    crate::text::strip_accents(&s)
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !NOISE.is_match(w) && !crate::tmdb::TAGS.is_match(w))
        .map(str::to_string)
        .collect()
}

fn joined(s: &str) -> String {
    words(s).join(" ")
}

/// The name of the folder a file is in.
fn folder_name(path: &Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Trailers, samples and other extras (by name or folder): not the work.
fn is_extra(f: &FileEntry) -> bool {
    static SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)[-._ ](?:trailer|sample|featurette|behindthescenes|deleted|deletedscene|interview|scene|short|extra|other|teaser|clip)\d*$",
        )
        .unwrap()
    });
    static FOLDER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)^(?:extras?|bonus|trailers?|samples?|featurettes?|behind the scenes|deleted scenes|interviews?|scenes|shorts|other|others|clips)$",
        )
        .unwrap()
    });
    SUFFIX.is_match(&f.stem) || FOLDER.is_match(folder_name(&f.path).trim())
}

/// The words after a film's year that follow a dash (`Movie (2010) -
/// Making Of` → `making of`); none for a release name's tags.
fn after_year(stem: &str) -> String {
    static YEAR_END: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[(\[]?\b(?:19|20)\d{2}\b[)\]]?").unwrap());
    let Some(m) = YEAR_END.find_iter(stem).last() else {
        return String::new();
    };
    let rest = crate::tmdb::BRACKETS.replace_all(&stem[m.end()..], " ");
    let rest = rest.trim_start();
    if !rest.starts_with(['-', '–', '—']) {
        return String::new();
    }
    joined(rest)
}

/// The keys a file is known by (any two files sharing one are the same
/// work), and what kind of work it is.
fn keys(f: &FileEntry) -> Option<(Kind, Vec<String>)> {
    if f.is_dir {
        return None;
    }
    let ext = f.ext.to_ascii_lowercase();
    if crate::tmdb::is_video(&ext) {
        if is_extra(f) {
            return None;
        }
        let folder = folder_name(&f.path);
        let ep = crate::tv::parse(&f.stem, &folder);
        // A bare number is an episode's only when it is not a year (a
        // film's name has one).
        let numbered = ep
            .number
            .is_some_and(|n| !(1900..=2100).contains(&n) && crate::tmdb::guess(&f.stem).year.is_none());
        if !ep.episodes.is_empty() || ep.date.is_some() || numbered {
            // The show: in the name, else the folder (above a season's).
            let mut show = joined(&ep.show);
            if show.is_empty() {
                let up = f.path.parent();
                let season = crate::tv::folder_season(&folder).is_some();
                let dir = if season { up.and_then(Path::parent) } else { up };
                show = dir
                    .and_then(|d| d.file_name())
                    .map(|n| joined(&crate::tmdb::guess(&n.to_string_lossy()).title))
                    .unwrap_or_default();
            }
            if show.is_empty() {
                return None;
            }
            let at = match (&ep.date, ep.season, ep.episodes.is_empty(), ep.number) {
                (Some(d), ..) => format!("d{d}"),
                (None, Some(s), false, _) => format!(
                    "s{s}e{}",
                    ep.episodes
                        .iter()
                        .map(u16::to_string)
                        .collect::<Vec<_>>()
                        .join("-")
                ),
                (None, None, false, _) => format!(
                    "s1e{}",
                    ep.episodes
                        .iter()
                        .map(u16::to_string)
                        .collect::<Vec<_>>()
                        .join("-")
                ),
                (None, _, true, Some(n)) if numbered => format!("n{n}"),
                _ => return None,
            };
            // `Episode - Part 2`: another file of a two-part episode.
            static PART: LazyLock<Regex> = LazyLock::new(|| {
                Regex::new(r"(?i)\b(?:parte?|pt)\.?\s*(\d+|i{1,3}|iv)\b").unwrap()
            });
            let part = PART
                .captures(&ep.title)
                .map(|c| c[1].to_lowercase())
                .unwrap_or_default();
            return Some((Kind::Episode, vec![format!("ep:{show}:{at}:{part}")]));
        }
        let g = crate::tmdb::guess(&f.stem);
        // `Movie (2010) - Bonus Feature`: what follows the year after a
        // dash is another thing of the same film.
        let after = after_year(&f.stem);
        let mut out = Vec::new();
        if let Some(id) = g.id
            && after.is_empty()
        {
            out.push(format!("tmdb:{id}"));
        }
        let title = joined(&g.title);
        // Without a year, a title could be a remake's.
        if let Some(y) = g.year
            && !title.is_empty()
        {
            out.push(format!("movie:{title}:{y}:{after}"));
        }
        return (!out.is_empty()).then_some((Kind::Movie, out));
    }
    let comic_file = matches!(ext.as_str(), "cbz" | "cbr" | "cb7" | "cbt");
    let book_file = crate::ebook::is_book(&ext);
    if !comic_file && !book_file {
        return None;
    }
    let mut out = Vec::new();
    // A comic: series, volume and number, from inside or from its name.
    let issue = |series: &str, volume: &str, number: &str| {
        let series = joined(series);
        let number = number.trim().trim_start_matches('0');
        let number = if number.is_empty() { "0" } else { number };
        (!series.is_empty()).then(|| format!("comic:{series}:{volume}:{number}"))
    };
    if comic_file || ext == "pdf" || ext == "epub" {
        if let Some(info) = f.comic_info().filter(|i| !i.series.is_empty() && !i.number.is_empty()) {
            // `volume` holds a volume number or the series' first year.
            let vol = info.volume.trim();
            let vol = if vol.len() == 4 { "" } else { vol };
            // Carried inside: as sure as an ISBN.
            out.extend(issue(&info.series, vol, &info.number).map(|k| format!("{k}:inside")));
        }
        let g = crate::comics::parse_name(&f.stem, &folder_name(&f.path));
        // A PDF's or EPUB's series must come from its own name, not its
        // folder's (`Bills/2026-07/Tax.pdf` is no issue).
        let own = comic_file || {
            let stem = joined(&f.stem);
            let series = joined(&g.series);
            !series.is_empty() && stem.contains(&series)
        };
        if own && !g.number.is_empty() {
            let vol = g.volume.map(|v| v.to_string()).unwrap_or_default();
            // From a PDF's or EPUB's name: as weak as a book's name.
            out.extend(issue(&g.series, &vol, &g.number).map(|k| {
                if comic_file { k } else { format!("book:{k}") }
            }));
        }
    }
    if comic_file {
        return (!out.is_empty()).then_some((Kind::Comic, out));
    }
    // A book: its ISBN, and the words of its name in any order (`Author -
    // Title` and `Title - Author` are one book); a year in brackets is
    // the edition's and left out.
    // With its title: books with wrong details share an ISBN (a
    // series', a publisher's) and are told apart by it.
    if ext != "pdf" {
        out.extend(
            f.isbn_title()
                .map(|(i, title)| format!("isbn:{i}:{}", joined(title))),
        );
    }
    let mut w = words(&f.stem);
    w.sort();
    w.dedup();
    if w.iter().any(|w| w.chars().any(char::is_alphabetic)) && w.join("").chars().count() >= 3 {
        out.push(format!("book:{}", w.join(" ")));
    }
    (!out.is_empty()).then_some((Kind::Book, out))
}

/// How usable a book's format is: EPUB (reflows, opens almost anywhere),
/// then Kindle's, FB2, then PDF (fixed pages), then the rest.
fn format_rank(ext: &str) -> u8 {
    match ext.to_ascii_lowercase().as_str() {
        "epub" | "kepub" => 5,
        "azw3" => 4,
        "mobi" | "azw" | "prc" => 3,
        "fb2" | "fbz" => 2,
        "pdf" => 1,
        _ => 0,
    }
}

/// The file worth keeping in a group, when one stands out.
fn best(kind: Kind, files: &[&FileEntry]) -> Option<PathBuf> {
    let score = |f: &FileEntry| -> (u64, u64, u64) {
        match kind {
            Kind::Movie | Kind::Episode => {
                let px = f
                    .video_info()
                    .map(|v| u64::from(v.width) * u64::from(v.height))
                    .unwrap_or(0);
                (px, 0, f.size)
            }
            // Bigger scans are sharper; a zip opens everywhere.
            Kind::Comic => (0, 0, f.size),
            Kind::Book => (u64::from(format_rank(&f.ext)), 0, f.size),
        }
    };
    let mut sorted: Vec<&FileEntry> = files.to_vec();
    sorted.sort_by_key(|f| std::cmp::Reverse(score(f)));
    let mut top = sorted[0];
    if kind == Kind::Comic && !top.ext.eq_ignore_ascii_case("cbz") {
        // A CBZ nearly as large is the same scans in a better box.
        if let Some(z) = sorted
            .iter()
            .find(|f| f.ext.eq_ignore_ascii_case("cbz") && f.size * 100 >= top.size * 95)
        {
            top = z;
        }
    }
    let differ = sorted.iter().any(|f| score(f) != score(top) || f.ext != top.ext);
    differ.then(|| top.path.clone())
}

/// Groups of files that are the same work (two or more each). Files that
/// are only identical copies of one another are left out (already shown
/// as copies). `None` when `stale` turned true.
pub fn same_works(
    files: &[FileEntry],
    identical: &HashMap<PathBuf, usize>,
    stale: &dyn Fn() -> bool,
) -> Option<Vec<Group>> {
    // Union-find over the files: each key joins the files that have it.
    let mut parent: Vec<usize> = (0..files.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut kinds: Vec<Option<Kind>> = vec![None; files.len()];
    let mut seen: HashMap<String, usize> = HashMap::new();
    // Files joined by something they carry (an ISBN, a comic's details),
    // not only by their names.
    let mut carried: Vec<bool> = vec![false; files.len()];
    for (i, f) in files.iter().enumerate() {
        if i % 50 == 0 && stale() {
            return None;
        }
        let Some((kind, ks)) = keys(f) else { continue };
        kinds[i] = Some(kind);
        for k in ks {
            // Movies and episodes are never joined with books.
            let k = format!("{kind:?}/{k}");
            let strong = k.contains("/isbn:") || k.ends_with(":inside");
            match seen.get(&k) {
                Some(&j) => {
                    if strong {
                        carried[i] = true;
                        carried[j] = true;
                    }
                    let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                    if a != b {
                        parent[a] = b;
                    }
                }
                None => {
                    seen.insert(k, i);
                }
            }
        }
    }
    let mut by_root: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, kind) in kinds.iter().enumerate() {
        if kind.is_some() {
            let r = root(&mut parent, i);
            by_root.entry(r).or_default().push(i);
        }
    }
    let mut groups: Vec<Group> = Vec::new();
    for members in by_root.into_values().filter(|m| m.len() > 1) {
        if stale() {
            return None;
        }
        let first = identical.get(&files[members[0]].path);
        if first.is_some() && members.iter().all(|&i| identical.get(&files[i].path) == first) {
            continue;
        }
        let kind = kinds[members[0]].unwrap();
        // Books joined only by their names: the same book in other
        // formats (same-format files with one name are as often
        // different documents, like a bill each month).
        if kind == Kind::Book && !members.iter().any(|&i| carried[i]) {
            let mut exts: Vec<String> = members
                .iter()
                .map(|&i| files[i].ext.to_ascii_lowercase())
                .collect();
            exts.sort();
            exts.dedup();
            if exts.len() < 2 {
                continue;
            }
        }
        let mut list: Vec<&FileEntry> = members.iter().map(|&i| &files[i]).collect();
        list.sort_by(|a, b| natural_cmp(&a.path.to_string_lossy(), &b.path.to_string_lossy()));
        let best = best(kind, &list);
        let items = list
            .iter()
            .map(|f| {
                let v = matches!(kind, Kind::Movie | Kind::Episode)
                    .then(|| f.video_info())
                    .flatten();
                Item {
                    path: f.path.clone(),
                    size: f.size,
                    picture: v.map(|v| format!("{}×{}", v.width, v.height)).unwrap_or_default(),
                    codec: v.map(|v| v.codec.clone()).unwrap_or_default(),
                }
            })
            .collect();
        groups.push(Group { kind, items, best });
    }
    groups.sort_by(|a, b| {
        natural_cmp(
            &a.items[0].path.to_string_lossy(),
            &b.items[0].path.to_string_lossy(),
        )
    });
    Some(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(dir: &Path, name: &str, size: usize) -> FileEntry {
        let p = dir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, vec![b'x'; size]).unwrap();
        FileEntry::from_path(&p, &std::fs::metadata(&p).unwrap()).unwrap()
    }

    fn names(g: &Group) -> Vec<String> {
        g.items
            .iter()
            .map(|i| i.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn same_works_found() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        let files = vec![
            file(p, "Some.Movie.2010.1080p.BluRay.x264-GRP.mkv", 30),
            file(p, "Some Movie (2010) [tmdbid-77].mp4", 20),
            // A remake: another year.
            file(p, "Some Movie (1975).avi", 10),
            file(p, "Show/Season 01/Show - S01E02 - Title.mkv", 10),
            file(p, "Show/Season 01/S01E02.720p.mkv", 12),
            file(p, "Show/Season 01/Show - S01E03.mkv", 10),
            file(p, "Comics/Hero 012 (2015).cbr", 100),
            file(p, "Comics/Hero #12.cbz", 97),
            file(p, "Comics/Hero 013.cbz", 90),
            file(p, "Books/Orwell, George - Animal Farm.epub", 5),
            file(p, "Books/Animal Farm - George Orwell (1945).pdf", 50),
            file(p, "Books/Animal Farm - George Orwell.srt", 1),
            file(p, "Other/same.mkv", 3),
            file(p, "Some Movie (2010) - Making Of.mkv", 3),
            file(p, "Some Movie (2010)-trailer.mkv", 3),
            file(p, "Bills/2026-07/Tax.pdf", 3),
            file(p, "Bills/2026-08/Tax.pdf", 4),
            file(p, "Other/same2.mkv", 3),
        ];
        let groups = same_works(&files, &HashMap::new(), &|| false).unwrap();
        let got: Vec<(Kind, Vec<String>, Option<String>)> = groups
            .iter()
            .map(|g| {
                (
                    g.kind,
                    names(g),
                    g.best
                        .as_ref()
                        .map(|b| b.file_name().unwrap().to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                (
                    Kind::Book,
                    vec![
                        "Animal Farm - George Orwell (1945).pdf".to_string(),
                        "Orwell, George - Animal Farm.epub".to_string()
                    ],
                    Some("Orwell, George - Animal Farm.epub".to_string())
                ),
                (
                    Kind::Comic,
                    vec!["Hero #12.cbz".to_string(), "Hero 012 (2015).cbr".to_string()],
                    Some("Hero #12.cbz".to_string())
                ),
                (
                    Kind::Episode,
                    vec![
                        "S01E02.720p.mkv".to_string(),
                        "Show - S01E02 - Title.mkv".to_string()
                    ],
                    // Not videos really: the larger.
                    Some("S01E02.720p.mkv".to_string())
                ),
                (
                    Kind::Movie,
                    vec![
                        "Some Movie (2010) [tmdbid-77].mp4".to_string(),
                        "Some.Movie.2010.1080p.BluRay.x264-GRP.mkv".to_string()
                    ],
                    Some("Some.Movie.2010.1080p.BluRay.x264-GRP.mkv".to_string())
                ),
            ]
        );
        assert!(same_works(&files, &HashMap::new(), &|| true).is_none());
        // Identical copies only: not again.
        let mut identical = HashMap::new();
        for g in &groups[1..2] {
            for i in &g.items {
                identical.insert(i.path.clone(), 0);
            }
        }
        assert_eq!(same_works(&files, &identical, &|| false).unwrap().len(), 3);
    }
}
