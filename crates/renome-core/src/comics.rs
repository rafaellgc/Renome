//! Comics, without the network part: what a comic file says about itself
//! (its `ComicInfo.xml`), what its name says (series, issue number, volume,
//! chapter, year), and the new name.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::files::FileEntry;
use crate::text;
use crate::tmdb::safe_title;

/// Extensions of comic files. CBR (RAR) is named from its file name only:
/// no RAR reader has a free licence.
pub fn is_comic(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "cbz" | "cbr" | "cb7" | "cbt" | "pdf" | "epub"
    )
}

/// The fields of a `ComicInfo.xml` that name a comic.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ComicInfo {
    pub series: String,
    pub number: String,
    pub count: Option<u32>,
    /// The volume number, or the year the series started (both are used).
    pub volume: String,
    pub title: String,
    pub year: Option<u16>,
    pub publisher: String,
    pub writer: String,
    pub penciller: String,
    /// Often a Comic Vine page of the issue.
    pub web: String,
    pub manga: bool,
}

/// Reads the `ComicInfo.xml` inside a CBZ, CB7, CBT or EPUB; `None` for
/// other files, files without one and unreadable files.
pub fn read(path: &Path) -> Option<ComicInfo> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    let xml = std::panic::catch_unwind(|| match ext.as_str() {
        "cbz" | "zip" | "epub" => from_zip(path),
        "cb7" | "7z" => from_7z(path),
        "cbt" | "tar" => from_tar(path),
        _ => None,
    })
    .ok()
    .flatten()?;
    parse_info(&xml)
}

const INFO: &str = "comicinfo.xml";
/// A `ComicInfo.xml` is a few kilobytes; more is not one.
const MAX_INFO: u64 = 1 << 20;

fn is_info(name: &str) -> bool {
    name.rsplit(['/', '\\'])
        .next()
        .is_some_and(|n| n.eq_ignore_ascii_case(INFO))
}

fn from_zip(path: &Path) -> Option<String> {
    let mut zip = zip::ZipArchive::new(File::open(path).ok()?).ok()?;
    let name = zip
        .file_names()
        .filter_map(Result::ok)
        .find(|n| is_info(n))?
        .into_owned();
    let entry = zip.by_name(&name).ok()?;
    let mut text = String::new();
    entry.take(MAX_INFO).read_to_string(&mut text).ok()?;
    Some(text)
}

fn from_7z(path: &Path) -> Option<String> {
    // A solid archive decodes everything before the file asked for: big
    // ones are not worth it for a few lines of text.
    let size = std::fs::metadata(path).ok()?.len();
    let mut reader =
        sevenz_rust2::ArchiveReader::open(path, sevenz_rust2::Password::empty()).ok()?;
    let entry = reader.archive().files.iter().find(|f| is_info(f.name()))?;
    // Read whole: one claiming to be bigger than a `ComicInfo.xml` is not.
    if entry.size() > MAX_INFO {
        return None;
    }
    let name = entry.name().to_string();
    if reader.archive().is_solid && size > 300 << 20 {
        return None;
    }
    let bytes = reader.read_file(&name).ok()?;
    String::from_utf8(bytes).ok()
}

fn from_tar(path: &Path) -> Option<String> {
    let mut archive = tar::Archive::new(File::open(path).ok()?);
    for entry in archive.entries().ok()? {
        let entry = entry.ok()?;
        let name = entry.path().ok()?.to_string_lossy().into_owned();
        if is_info(&name) {
            let mut text = String::new();
            entry.take(MAX_INFO).read_to_string(&mut text).ok()?;
            return Some(text);
        }
    }
    None
}

/// The fields of a `ComicInfo.xml` text.
pub fn parse_info(xml: &str) -> Option<ComicInfo> {
    let xml = xml.trim_start_matches('\u{feff}');
    let doc = roxmltree::Document::parse(xml).ok()?;
    let root = doc.root_element();
    if !root.tag_name().name().eq_ignore_ascii_case("ComicInfo") {
        return None;
    }
    let field = |name: &str| {
        root.children()
            .find(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case(name))
            .and_then(|n| n.text())
            .map(|t| text::nfc(&text::collapse_spaces(t.trim())))
            .unwrap_or_default()
    };
    let info = ComicInfo {
        series: field("Series"),
        number: field("Number"),
        count: field("Count").parse().ok().filter(|n| *n > 0),
        volume: field("Volume"),
        title: field("Title"),
        year: field("Year")
            .parse()
            .ok()
            .filter(|y| (1800..=2200).contains(y)),
        publisher: field("Publisher"),
        writer: field("Writer"),
        penciller: field("Penciller"),
        web: field("Web"),
        manga: field("Manga").to_lowercase().starts_with("yes"),
    };
    (!info.series.is_empty() || !info.number.is_empty()).then_some(info)
}

/// Series, numbers and year read from a file name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ComicGuess {
    pub series: String,
    /// The issue number as written (`12`, `12.1`, `0`, `½`).
    pub number: String,
    /// Volume, tome or book (`v2`, `Vol. 3`, `Tome 5`).
    pub volume: Option<u32>,
    /// A manga's chapter (`Ch. 45`, `c045`, `Cap. 21`).
    pub chapter: String,
    /// The issue's year.
    pub year: Option<u16>,
    /// The year the series started, from its folder (`Series (2012)`).
    pub series_year: Option<u16>,
    /// How many issues the series has (`(of 6)`).
    pub count: Option<u32>,
    /// What follows the number (`Series 012 - Title`).
    pub title: String,
}

static BRACKETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(([^()]*)\)|\[([^\[\]]*)\]|\{([^{}]*)\}").unwrap());
static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:19|20)\d{2}$").unwrap());
static OF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:of|de|di|von|sur)\s+(\d{1,4})$").unwrap());
/// Words of release notes, not of the name.
static JUNK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:digital|webrip|web-?dl|c2c|hq|hd|scan(?:ned)?|repack|fixed|cbz|cbr|cb7|cbt|pdf|epub|\d{3,4}p|ctc|noads|no ads)\b",
    )
    .unwrap()
});
static CHAPTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:ch|chap|chapter|cap|capítulo|capitulo|chapitre)\.?\s*(\d{1,4}(?:\.\d)?)\b|\bc(\d{2,4}(?:\.\d)?)\b",
    )
    .unwrap()
});
static VOLUME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:vol|volume|volumen|tome|tomo|band|livro|book|libro)\.?\s*(\d{1,3}|(?-i:[IVX]{1,5}))\b|\b[vt](\d{1,3})\b",
    )
    .unwrap()
});
static ISSUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)#\s*(-?\d{1,7}(?:\.\d{1,2})?[a-z]?|½)|\b(?:n[º°o]|no\.|nr\.?|issue|edição|edicao|edición|ed\.|número|numero|num\.?)\s*(\d{1,4}(?:\.\d{1,2})?)\b",
    )
    .unwrap()
});
static BARE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|\s)(\d{1,4}(?:\.\d{1,2})?)(?:\s|$)").unwrap());

/// A volume's number: `3`, or in Roman numerals (`III`).
fn volume_number(s: &str) -> Option<u32> {
    if let Ok(n) = s.parse() {
        return Some(n);
    }
    let value = |c| match c {
        'I' => Some(1),
        'V' => Some(5),
        'X' => Some(10),
        _ => None,
    };
    let digits: Vec<u32> = s.chars().map(value).collect::<Option<_>>()?;
    let mut n = 0;
    for (i, d) in digits.iter().enumerate() {
        match digits.get(i + 1) {
            Some(next) if next > d => n -= *d as i64,
            _ => n += *d as i64,
        }
    }
    u32::try_from(n).ok().filter(|n| *n > 0)
}

/// Reads series, numbers and year from a file name without extension;
/// `folder` (the name of the folder it is in) gives the series when the
/// name has none, and the series' year.
pub fn parse_name(stem: &str, folder: &str) -> ComicGuess {
    let mut g = ComicGuess::default();
    // `Series.012.2019`: dots stand for spaces, but not in `12.1`.
    let mut s = unescape(&colons(stem)).replace('_', " ");
    if !s.contains(' ') {
        let chars: Vec<char> = s.chars().collect();
        s = chars
            .iter()
            .enumerate()
            .map(|(i, &c)| {
                // A point in a number (`12.1`): a digit before, one or two after.
                let after = chars[i + 1..]
                    .iter()
                    .take_while(|c| c.is_ascii_digit())
                    .count();
                let decimal = i > 0 && chars[i - 1].is_ascii_digit() && (1..=2).contains(&after);
                match c == '.' && !decimal {
                    true => ' ',
                    false => c,
                }
            })
            .collect();
    }
    // What is in brackets: the year, the count, or release notes.
    for c in BRACKETS.captures_iter(&s.clone()) {
        let inner = c
            .iter()
            .skip(1)
            .flatten()
            .next()
            .map_or("", |m| m.as_str())
            .trim();
        if g.year.is_none() && YEAR.is_match(inner) {
            g.year = inner.parse().ok();
        } else if let Some(n) = OF.captures(inner) {
            g.count = n[1].parse().ok();
        }
    }
    // A volume in brackets (`Series (Vol.03) - Story`) is read as if it
    // were not in them; the rest in brackets goes.
    let s = BRACKETS.replace_all(&s, |c: &regex::Captures| {
        let inner = c.iter().skip(1).flatten().next().map_or("", |m| m.as_str());
        match VOLUME.find(inner.trim()) {
            Some(v) if v.start() == 0 => format!(" {} ", inner.trim()),
            _ => " ".to_string(),
        }
    });
    let s = JUNK.replace_all(&s, " ");
    let s = text::collapse_spaces(&s);

    let mut cut = s.len();
    let mut end = 0;
    let mut mark = |m: regex::Match| {
        cut = cut.min(m.start());
        end = end.max(m.end());
    };
    // What a marker took is blanked for the next ones (same length, so
    // positions stay): `Vol. 41` is no issue 41.
    let mut masked = s.clone();
    let mut blank =
        |r: std::ops::Range<usize>| masked.replace_range(r.clone(), &" ".repeat(r.len()));
    if let Some(c) = CHAPTER.captures(&s) {
        g.chapter = c.get(1).or_else(|| c.get(2)).unwrap().as_str().to_string();
        mark(c.get(0).unwrap());
        blank(c.get(0).unwrap().range());
    }
    let mut volume_end = None;
    if let Some(c) = VOLUME.captures(&s) {
        g.volume = volume_number(c.get(1).or_else(|| c.get(2)).unwrap().as_str());
        mark(c.get(0).unwrap());
        blank(c.get(0).unwrap().range());
        volume_end = Some(c.get(0).unwrap().end());
    }
    let s_all = s.clone();
    let s = masked;
    let mut number_start = None;
    if let Some(c) = ISSUE.captures(&s) {
        g.number = c.get(1).or_else(|| c.get(2)).unwrap().as_str().to_string();
        number_start = Some(c.get(0).unwrap().start());
        mark(c.get(0).unwrap());
    } else if g.chapter.is_empty() {
        // The number first (`19 - Story in 80 Words`): numbers in what
        // follows are no issue.
        static LEADING: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^(\d{1,4})\s*[-–.]\s+\S").unwrap());
        let leading = LEADING
            .captures(&s)
            .and_then(|c| c.get(1))
            .filter(|m| !YEAR.is_match(m.as_str()));
        // Else the last bare number that is not a year: `Series 012`,
        // `Series 2000 123`. A number alone is a number, with no series. A
        // number a word follows is part of the name (`7 Swords`, `Story
        // 714 Words`, `10 Years`).
        let found = leading.or_else(|| {
            BARE.captures_iter(&s)
                .filter_map(|c| c.get(1))
                .filter(|m| !YEAR.is_match(m.as_str()) || s.trim() == m.as_str())
                .filter(|m| !s[m.end()..].trim_start().starts_with(char::is_alphabetic))
                .last()
        });
        if let Some(m) = found
            && (m.start() > 0 || g.volume.is_none())
        {
            g.number = m.as_str().to_string();
            number_start = Some(m.start());
            mark(m);
        }
    }
    let s = s_all;
    let before = if cut < s.len() { &s[..cut] } else { s.as_str() };
    g.series = tidy_part(before);
    // Words between the volume and the number belong to the series
    // (`Series Vol. 5 Annual #01`: the annual, not issue 1).
    if let (Some(v), Some(n)) = (volume_end, number_start)
        && n > v
    {
        let mid = tidy_part(&s[v..n]);
        if mid.chars().any(char::is_alphabetic) {
            g.series = text::collapse_spaces(&format!("{} {mid}", g.series));
        }
    }
    // The number first (`03 - Series`): what follows is the series.
    if g.series.is_empty() && end > 0 && end < s.len() {
        let after = tidy_part(&s[end..]);
        if after.chars().any(char::is_alphabetic) {
            g.series = after;
            end = s.len();
        }
    }
    if end > 0 && end <= s.len() {
        let mut after = s[end..].trim_start_matches([' ', '-', '–', ':', '.']);
        // The count after the number (`#2 de 6`, `#2 of 6`).
        static COUNT: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?i)^(?:of|de|di|von|sur)\s+(\d{1,4})\b").unwrap());
        if let Some(c) = COUNT.captures(after) {
            g.count = g.count.or(c[1].parse().ok());
            after = &after[c.get(0).unwrap().end()..];
        }
        // A loose year right after the number (`Series 012 2005`).
        if let Some((year, rest)) = after.split_once(' ').or(Some((after, "")))
            && g.year.is_none()
            && YEAR.is_match(year)
        {
            g.year = year.parse().ok();
            after = rest;
        }
        g.title = tidy_part(after);
    }
    // A year written loose at the end of the series (`Series 2012 012`).
    if g.year.is_none() {
        static TRAILING_YEAR: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\s((?:19|20)\d{2})$").unwrap());
        if let Some(c) = TRAILING_YEAR.captures(&g.series.clone()) {
            g.year = c[1].parse().ok();
            g.series = g.series[..c.get(0).unwrap().start()].trim().to_string();
        }
    }
    // The folder: the series' year, its name for files without one, and
    // the volume (`Series/Vol. 3/012.cbz`).
    let f = parse_folder(folder);
    g.series_year = f.series_year;
    if g.series.is_empty() {
        g.series = f.series;
    }
    if g.volume.is_none() {
        g.volume = f.volume;
    }
    g
}

/// Letters that look like a colon, put in names instead of `:` (which
/// Windows does not accept), as the colon they stand for.
fn colons(s: &str) -> String {
    s.replace(
        ['\u{2d0}', '\u{a789}', '\u{fe13}', '\u{ff1a}', '\u{2236}'],
        ":",
    )
}

/// Accented letters some programs write as their code between
/// underscores (`M_244_nica` for `Mônica`, `Alcal_225` for `Alcalá`), put
/// back. Only codes of letters (192 to 255) count: `Series_012_` stays.
fn unescape(s: &str) -> String {
    static CODE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(\p{L})_(\d{3})(?:_|\b)").unwrap());
    CODE.replace_all(s, |c: &regex::Captures| {
        let letter = c[2]
            .parse::<u32>()
            .ok()
            .filter(|n| (192..=255).contains(n) && *n != 215 && *n != 247)
            .and_then(char::from_u32);
        match letter {
            Some(l) => format!("{}{l}", &c[1]),
            None => c[0].to_string(),
        }
    })
    .into_owned()
}

/// What a folder's name says: `Series (2012)`, `Vol. 03`.
struct FolderGuess {
    series: String,
    series_year: Option<u16>,
    volume: Option<u32>,
}

fn parse_folder(folder: &str) -> FolderGuess {
    let folder = &unescape(&colons(folder));
    let mut year = None;
    for c in BRACKETS.captures_iter(folder) {
        let inner = c
            .iter()
            .skip(1)
            .flatten()
            .next()
            .map_or("", |m| m.as_str())
            .trim();
        // `(2012)`, or a run of years (`(1988-1998)`, `(2011 – 2016)`).
        static RANGE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"^((?:19|20)\d{2})\s*[-–—]\s*(?:(?:19|20)\d{2})?$").unwrap()
        });
        if year.is_none() && YEAR.is_match(inner) {
            year = inner.parse().ok();
        } else if year.is_none()
            && let Some(c) = RANGE.captures(inner)
        {
            year = c[1].parse().ok();
        }
    }
    let s = text::collapse_spaces(
        &JUNK.replace_all(&BRACKETS.replace_all(&folder.replace('_', " "), " "), " "),
    );
    let volume = VOLUME
        .captures(&s)
        .and_then(|c| volume_number(c.get(1).or_else(|| c.get(2))?.as_str()));
    let series = match VOLUME.find(&s) {
        Some(m) => tidy_part(&s[..m.start()]),
        None => tidy_part(&s),
    };
    FolderGuess {
        series,
        series_year: year,
        volume,
    }
}

/// A part of a name without the separators around it.
fn tidy_part(s: &str) -> String {
    text::collapse_spaces(s.trim_matches(|c: char| c.is_whitespace() || "-–—:,.#".contains(c)))
}

/// The comic chosen for a file, or read from it, as the rule needs it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ComicMatch {
    pub series: String,
    pub series_year: Option<u16>,
    pub number: String,
    pub volume: Option<u32>,
    pub chapter: String,
    pub title: String,
    pub year: Option<u16>,
    pub count: Option<u32>,
    pub publisher: String,
    pub writer: String,
    pub artist: String,
    /// The issue on Comic Vine.
    pub id: String,
}

/// What the file says about itself: its `ComicInfo.xml` first, its name
/// for the rest. `None` when neither gives a series. A book without a
/// number is a one-shot: named by its series (and year). `by_folder`: the
/// series is the folder's (see [`in_folder`]).
pub fn from_file(file: &FileEntry, by_folder: bool) -> Option<ComicMatch> {
    from_name(file, &file.stem, by_folder)
}

/// Like [`from_file`], reading `stem` as its name (what the rules above
/// the comic rule made of it).
pub fn from_name(file: &FileEntry, stem: &str, by_folder: bool) -> Option<ComicMatch> {
    let folder = file.dir.file_name().unwrap_or_default().to_string_lossy();
    let g = parse_name(stem, &folder);
    let info = file.comic_info().cloned().unwrap_or_default();
    let mut m = merge(&g, &info);
    if by_folder {
        in_folder(&mut m, &folder);
    }
    (!m.series.is_empty()).then_some(m)
}

/// The series named by the folder the file is in, when the user says so
/// (`Series (Complete)/01 - Story.cbz`): what the name had for a series
/// becomes the title, unless it is the folder's series (or starts with
/// it: then the rest is).
pub fn in_folder(m: &mut ComicMatch, folder: &str) {
    let f = parse_folder(folder);
    if f.series.is_empty() {
        return;
    }
    let fold = |s: &str| text::strip_accents(&s.to_lowercase());
    if m.title.is_empty() {
        m.title = match fold(&m.series).starts_with(&fold(&f.series)) {
            true => story(&m.series, &f.series),
            false => m.series.clone(),
        };
    }
    m.series = f.series;
    m.series_year = f.series_year.or(m.series_year);
    m.volume = m.volume.or(f.volume);
}

/// Like [`from_name`], for a path; `stem`, when given, is read as its name.
pub fn from_path(path: &Path, stem: Option<&str>, by_folder: bool) -> Option<ComicMatch> {
    let ext = path.extension()?.to_string_lossy();
    if !is_comic(&ext) {
        return None;
    }
    let own = path.file_stem()?.to_string_lossy();
    let stem = stem.unwrap_or(&own);
    let folder = path
        .parent()?
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let mut m = merge(&parse_name(stem, &folder), &read(path).unwrap_or_default());
    if by_folder {
        in_folder(&mut m, &folder);
    }
    (!m.series.is_empty()).then_some(m)
}

/// A name's guess with the `ComicInfo.xml` over it.
pub fn merge(g: &ComicGuess, info: &ComicInfo) -> ComicMatch {
    let or = |a: &str, b: &str| {
        if a.is_empty() {
            b.to_string()
        } else {
            a.to_string()
        }
    };
    // A file named in another language than its `ComicInfo.xml` (a
    // translation that kept the original's): the name is the user's.
    let fold = |s: &str| simple(s);
    if !g.series.is_empty() && !info.series.is_empty() {
        let (a, b) = (fold(&g.series), fold(&info.series));
        if !a.starts_with(&b) && !b.starts_with(&a) {
            return merge(
                g,
                &ComicInfo {
                    writer: info.writer.clone(),
                    penciller: info.penciller.clone(),
                    ..Default::default()
                },
            );
        }
    }
    // `Volume` holds a volume number or the series' first year.
    let (volume, series_year) = match info.volume.parse::<u32>() {
        Ok(v) if (1800..=2200).contains(&v) => (g.volume, u16::try_from(v).ok()),
        Ok(v) => (Some(v), g.series_year),
        Err(_) => (g.volume, g.series_year),
    };
    ComicMatch {
        series: or(&info.series, &g.series),
        series_year,
        number: or(&info.number, &g.number),
        volume,
        chapter: g.chapter.clone(),
        title: or(&info.title, &g.title),
        year: info.year.or(g.year),
        count: info.count.or(g.count),
        publisher: info.publisher.clone(),
        writer: info.writer.clone(),
        artist: info.penciller.clone(),
        id: comic_vine_id(&info.web).unwrap_or_default(),
    }
}

/// The issue id in a Comic Vine link (`…/4000-12345/`).
pub fn comic_vine_id(web: &str) -> Option<String> {
    static ID: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"comicvine\.gamespot\.com/.*?/4000-(\d+)").unwrap());
    ID.captures(web).map(|c| c[1].to_string())
}

/// A number with at least `width` digits before its point: `12` → `012`,
/// `12.1` → `012.1`; `½` and other text stay.
fn padded(n: &str, width: usize) -> String {
    let n = n.trim();
    let (sign, rest) = n.strip_prefix('-').map_or(("", n), |r| ("-", r));
    let digits = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if digits == 0 {
        return n.to_string();
    }
    let whole = rest[..digits].trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    format!("{sign}{whole:0>width$}{}", &rest[digits..])
}

/// The new name from the rule's template: `{series}`, `{series year}`,
/// `{number}` (three digits), `{volume}` (two), `{chapter}` (three),
/// `{title}`, `{year}`, `{count}`, `{publisher}`, `{writer}`, `{artist}`,
/// `{id}` (also in Portuguese and Spanish). A key without a value takes
/// its label with it (`v{volume}`, `#{number}`, `Vol. {volume}`), and
/// brackets left empty go.
pub fn comic_name(template: &str, m: &ComicMatch, colon: &str) -> String {
    let safe = |s: &str| safe_title(s, colon);
    let series = safe(&m.series);
    let number = safe(&if m.number.is_empty() {
        String::new()
    } else {
        padded(&m.number, 3)
    });
    let volume = m.volume.map(|v| format!("{v:02}")).unwrap_or_default();
    let chapter = if m.chapter.is_empty() {
        String::new()
    } else {
        padded(&m.chapter, 3)
    };
    let title = safe(&m.title);
    let year = m.year.map(|y| y.to_string()).unwrap_or_default();
    let series_year = m.series_year.map(|y| y.to_string()).unwrap_or_default();
    let count = m.count.map(|c| c.to_string()).unwrap_or_default();
    let (publisher, writer, artist) = (safe(&m.publisher), safe(&m.writer), safe(&m.artist));
    let value = |key: &str| -> Option<&str> {
        Some(match key {
            "series" | "série" | "serie" => &series,
            "series year" | "ano da série" | "ano da serie" | "año de la serie" => &series_year,
            "number" | "número" | "numero" => &number,
            "volume" | "volumen" | "tomo" => &volume,
            "chapter" | "capítulo" | "capitulo" => &chapter,
            "title" | "título" | "titulo" => &title,
            "year" | "ano" | "año" => &year,
            "count" | "total" => &count,
            "publisher" | "editora" | "editorial" => &publisher,
            "writer" | "roteiro" | "guion" | "guión" => &writer,
            "artist" | "arte" | "dibujo" => &artist,
            "id" => &m.id,
            _ => return None,
        })
    };
    // A separator left before a bracket or at the end, by an empty title.
    static DANGLING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s[-–—:]\s*(\(|$)").unwrap());
    let name = expand_labeled(template, value);
    crate::books::tidy(&DANGLING.replace_all(&name, " $1"))
}

/// Like [`crate::rules::expand`], but a key left empty also drops the
/// label just before it: one stuck to it (`v`, `#`, `c`) or one ending in a
/// dot (`Vol. `, `Cap. `).
fn expand_labeled<'a>(template: &str, value: impl Fn(&str) -> Option<&'a str>) -> String {
    static LABEL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?:(?:^|\s)[^\s{}]{1,6}\.\s?|[^\s{}()\[\]]{1,6})$").unwrap());
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let tail = &rest[open..];
        match tail
            .find('}')
            .and_then(|close| Some((close, value(&tail[1..close])?)))
        {
            Some((close, v)) => {
                if v.is_empty() {
                    // Only what follows a space or the start is a label: the
                    // end of a word is not (`Series{number}`).
                    let cut = LABEL.find(&out).and_then(|m| {
                        let label = m.as_str();
                        let spaced = label.starts_with(char::is_whitespace);
                        let starts_word = m.start() == 0
                            || out[..m.start()].ends_with(char::is_whitespace)
                            || spaced;
                        ((!spaced && starts_word) || label.trim_end().ends_with('.'))
                            .then_some((m.start(), spaced))
                    });
                    if let Some((start, spaced)) = cut {
                        out.truncate(start);
                        if spaced {
                            out.push(' ');
                        }
                    }
                } else {
                    out.push_str(v);
                }
                rest = &tail[close + 1..];
            }
            None => {
                out.push('{');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Folders made from the rule's folder template (levels split at `/`);
/// levels that come out empty are skipped.
pub fn folders(template: &str, m: &ComicMatch, colon: &str) -> Vec<String> {
    template
        .split('/')
        .map(|level| comic_name(level, m, colon))
        .filter(|l| !l.is_empty())
        .collect()
}

/// A name for comparing series: no accents, case or punctuation.
fn simple(s: &str) -> String {
    let s = text::strip_accents(&s.to_lowercase());
    let s: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    text::collapse_spaces(&s)
}

/// Whether a comic already is in its series' folder (`Series Vol. 2
/// (1988-1998)` for `Series v02`, `Series` for `Series - Special`): then
/// it stays there, whatever the folder template would name it.
pub fn in_place(dir: &Path, m: &ComicMatch) -> bool {
    let name = dir.file_name().unwrap_or_default().to_string_lossy();
    let f = parse_folder(&name);
    // A volume's folder (`Series/Vol. 1`): the series' is the one above.
    if f.series.is_empty() {
        return f.volume.is_some()
            && f.volume == m.volume
            && dir.parent().is_some_and(|up| in_place(up, m));
    }
    let (folder, series) = (simple(&f.series), simple(&m.series));
    // The same words, or spelled a little differently (`Apocalipse`,
    // `Apocalypse`).
    let alike = spelled_alike(&folder, &series);
    (series == folder || series.starts_with(&format!("{folder} ")) || alike)
        && (f.volume.is_none() || f.volume == m.volume)
}

/// The same words, one of them spelled a letter apart (`Series
/// Apocalipse`, `Series Apocalypse`); not another word (`Album One`,
/// `Album Two`).
fn spelled_alike(a: &str, b: &str) -> bool {
    let (wa, wb): (Vec<&str>, Vec<&str>) = (a.split(' ').collect(), b.split(' ').collect());
    // Edit distance between two words.
    let apart = |x: &str, y: &str| {
        let (x, y): (Vec<char>, Vec<char>) = (x.chars().collect(), y.chars().collect());
        let mut row: Vec<usize> = (0..=y.len()).collect();
        for (i, cx) in x.iter().enumerate() {
            let mut prev = row[0];
            row[0] = i + 1;
            for (j, cy) in y.iter().enumerate() {
                let cur = row[j + 1];
                row[j + 1] = (prev + usize::from(cx != cy)).min(row[j] + 1).min(cur + 1);
                prev = cur;
            }
        }
        row[y.len()]
    };
    let differ: Vec<(&&str, &&str)> = wa.iter().zip(&wb).filter(|(x, y)| x != y).collect();
    wa.len() == wb.len()
        && differ.len() == 1
        && differ[0].0.chars().count() >= 5
        && apart(differ[0].0, differ[0].1) == 1
}

/// Comics read together, as a library: one spelling per series (the most
/// common; case, accents and a letter or two apart are the same series:
/// `Series Apocalipse`, `Series Apocalypse`), and the series each one's
/// folder is named after, when not its own (`None`): a series of one file
/// goes with the series its name starts with (`Series - Special` with
/// `Series`; the longest one), or, when two such files share what comes
/// before ` - ` (`Series - Album One`, `Series - Album Two`), with that.
pub fn arrange(comics: &mut [ComicMatch]) -> Vec<Option<ComicMatch>> {
    // One spelling per series.
    let keys: Vec<String> = comics.iter().map(|m| simple(&m.series)).collect();
    let mut reps: Vec<String> = Vec::new();
    let rep_of: Vec<String> = keys
        .iter()
        .map(|k| {
            let same = reps.iter().find(|r| *r == k || spelled_alike(r, k));
            match same {
                Some(r) => r.clone(),
                None => {
                    reps.push(k.clone());
                    k.clone()
                }
            }
        })
        .collect();
    let mut spellings: HashMap<&str, Vec<(String, usize)>> = HashMap::new();
    for (m, r) in comics.iter().zip(&rep_of) {
        let v = spellings.entry(r).or_default();
        match v.iter_mut().find(|(s, _)| *s == m.series) {
            Some(e) => e.1 += 1,
            None => v.push((m.series.clone(), 1)),
        }
    }
    let chosen: HashMap<&str, String> = spellings
        .iter()
        .map(|(r, v)| {
            let most = v.iter().map(|(_, n)| *n).max().unwrap_or(0);
            let s = v.iter().find(|(_, n)| *n == most).map(|(s, _)| s.clone());
            (*r, s.unwrap_or_default())
        })
        .collect();
    for (m, r) in comics.iter_mut().zip(&rep_of) {
        m.series = chosen[r.as_str()].clone();
    }

    // Each series' files, and its first comic (for its folder's name).
    let mut count: HashMap<&str, usize> = HashMap::new();
    let mut first: HashMap<&str, usize> = HashMap::new();
    for (i, r) in rep_of.iter().enumerate() {
        *count.entry(r).or_default() += 1;
        first.entry(r).or_insert(i);
    }
    let lone = |i: usize| count[rep_of[i].as_str()] == 1;
    let head = |m: &ComicMatch| {
        m.series
            .split_once(" - ")
            .map(|(h, _)| h.trim().to_string())
            .filter(|h| !h.is_empty())
    };
    let mut heads: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, m) in comics.iter().enumerate() {
        if lone(i)
            && let Some(h) = head(m)
        {
            heads.entry(simple(&h)).or_default().push(i);
        }
    }
    (0..comics.len())
        .map(|i| {
            if !lone(i) {
                return None;
            }
            let key = &rep_of[i];
            // The longest series in the batch this one's name starts with.
            let parent = count
                .keys()
                .filter(|r| key.starts_with(&format!("{r} ")))
                .max_by_key(|r| r.len());
            if let Some(p) = parent {
                let of = &comics[first[*p]];
                return Some(ComicMatch {
                    series: of.series.clone(),
                    series_year: of.series_year,
                    ..comics[i].clone()
                });
            }
            let h = head(&comics[i])?;
            (heads[&simple(&h)].len() > 1).then(|| ComicMatch {
                series: h,
                series_year: None,
                ..comics[i].clone()
            })
        })
        .collect()
}

/// Where the series' folders go: above folders already named after the
/// series (or one of its levels), else the file's own folder.
pub fn library_root(dir: &Path, m: &ComicMatch, levels: &[String]) -> PathBuf {
    let mut names: Vec<&str> = levels.iter().map(String::as_str).collect();
    names.push(m.series.as_str());
    let mut cur = dir;
    for _ in 0..levels.len() {
        let name = cur.file_name().unwrap_or_default().to_string_lossy();
        let folder = parse_folder(&name);
        let same = crate::music::names_any(&name, &names)
            || (!folder.series.is_empty()
                && crate::music::names_any(&folder.series, &[m.series.as_str()]));
        if !same {
            break;
        }
        match cur.parent() {
            Some(up) if !up.as_os_str().is_empty() => cur = up,
            _ => break,
        }
    }
    cur.to_path_buf()
}

/// A series in a catalogue (Grand Comics Database, Comic Vine, MangaDex):
/// one run of a series, by one publisher, from a year.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Volume {
    /// The catalogue's id (a number, or MangaDex's uuid).
    pub id: String,
    pub name: String,
    pub start_year: Option<u16>,
    pub publisher: String,
    /// Number of issues (or volumes) known; 0 when unknown.
    pub count: u32,
    pub cover: String,
    /// Two-letter codes, when the catalogue says.
    pub country: String,
    pub language: String,
    /// How it was published (`collected edition`, `encadernado`…).
    pub format: String,
    /// The author (MangaDex), for `{writer}` when the file has none.
    pub writer: String,
}

/// An issue of a series in a catalogue.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Issue {
    pub id: String,
    /// As the catalogue writes it (`1`, `12.1`, `½`; empty for none).
    pub number: String,
    pub name: String,
    pub cover_date: String,
    pub cover: String,
}

/// Series found by a search, the closest names first (a service's own
/// order may put `Time Masters` before `Disney Masters` for "Disney
/// Masters"), and among close ones those in the reader's language (`pt`);
/// the service's order counts among equals.
pub fn rank_volumes(query: &str, found: Vec<Volume>, lang: &str) -> Vec<Volume> {
    let fold = |s: &str| {
        text::strip_accents(&s.to_lowercase())
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let q = fold(query);
    let lang = lang.split('-').next().unwrap_or("").to_lowercase();
    let mut scored: Vec<(f64, usize, Volume)> = found
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            let name = fold(&v.name);
            let mut score = if name == q {
                2.0
            } else if name.starts_with(&format!("{q} ")) || q.starts_with(&format!("{name} ")) {
                1.0 + crate::tmdb::similarity(&name, &q)
            } else {
                crate::tmdb::similarity(&name, &q)
            };
            if !lang.is_empty() && v.language.to_lowercase().starts_with(&lang) {
                score += 0.5;
            }
            (score, i, v)
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, v)| v).collect()
}

/// The number in how a catalogue writes an issue: `12`, `12.1`; empty
/// for none (`[nn]`, `[2ª Edição]`, a reprint without one).
pub fn issue_number(descriptor: &str) -> String {
    static NUMBER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*#?\s*(-?\d+(?:\.\d+)?[a-z]?|½)\b").unwrap());
    NUMBER
        .captures(descriptor)
        .map(|c| c[1].to_string())
        .unwrap_or_default()
}

/// An issue number for comparing: `012` and `12`, `12.10` and `12.1`.
fn same_number(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        let s = s.trim().to_lowercase();
        let (whole, part) = s.split_once('.').unwrap_or((&s, ""));
        let whole = whole.trim_start_matches('0');
        let whole = if whole.is_empty() && !s.is_empty() {
            "0"
        } else {
            whole
        };
        let part = part.trim_end_matches('0');
        match part.is_empty() {
            true => whole.to_string(),
            false => format!("{whole}.{part}"),
        }
    };
    !a.trim().is_empty() && norm(a) == norm(b)
}

/// The issue of a series a comic is: by its number, or, for a volume
/// without one (`v03`, a manga's book), by the volume number; a book
/// without either is a one-shot: the series' only issue (or the first of
/// its printings, none of them numbered). Chapters get none.
pub fn find_issue<'a>(m: &ComicMatch, issues: &'a [Issue]) -> Option<&'a Issue> {
    let wanted = match (m.number.is_empty(), m.volume) {
        (false, _) => m.number.clone(),
        (true, Some(v)) if m.chapter.is_empty() => v.to_string(),
        (true, None) if m.chapter.is_empty() => {
            let unnumbered = issues.iter().all(|i| i.number.trim().is_empty());
            return (issues.len() == 1 || unnumbered)
                .then(|| issues.first())
                .flatten();
        }
        _ => return None,
    };
    let mut found = issues.iter().filter(|i| same_number(&i.number, &wanted));
    let first = found.next()?;
    // Two issues with one number (a variant listed twice): the review asks.
    found.next().is_none().then_some(first)
}

/// What a file's series has after the catalogue's: the story
/// (`Series: Story`, `Series - Story` for `Series`); empty otherwise.
pub fn story(own: &str, series: &str) -> String {
    let fold = |s: &str| text::strip_accents(&s.to_lowercase());
    let (o, s) = (fold(own), fold(series.trim()));
    if s.is_empty() || !o.starts_with(&s) || o.len() == s.len() {
        return String::new();
    }
    let rest = own.get(series.trim().len()..).unwrap_or_default();
    match rest.starts_with([':', ' ', '-', '–']) {
        true => tidy_part(rest),
        false => String::new(),
    }
}

/// A comic named after its series and issue in a catalogue: series,
/// number, title, year and publisher from there; volume and chapter from
/// the file. Without an issue (a catalogue of series only, MangaDex), the
/// series alone.
pub fn with_issue(own: &ComicMatch, volume: &Volume, issue: Option<&Issue>) -> ComicMatch {
    let name = issue.map_or("", |i| i.name.trim());
    let date = issue.map_or("", |i| i.cover_date.as_str());
    ComicMatch {
        series: volume.name.clone(),
        series_year: volume.start_year.or(own.series_year),
        // A book found by its volume (`v03`, a manga's) stays a volume, and
        // a one-shot gets no number.
        number: match (own.number.is_empty(), issue) {
            (false, Some(i)) if !i.number.is_empty() => i.number.clone(),
            _ => own.number.clone(),
        },
        volume: own.volume,
        chapter: own.chapter.clone(),
        // Many issues have no title there: the file's stays, or what its
        // series had after the catalogue's (`Series: Story`).
        title: match (name.is_empty(), own.title.is_empty()) {
            (false, _) => name.to_string(),
            (true, false) => own.title.clone(),
            (true, true) => story(&own.series, &volume.name),
        },
        year: date.get(..4).and_then(|y| y.parse().ok()).or(own.year),
        count: (volume.count > 0).then_some(volume.count).or(own.count),
        publisher: match volume.publisher.is_empty() {
            true => own.publisher.clone(),
            false => volume.publisher.clone(),
        },
        writer: match own.writer.is_empty() {
            true => volume.writer.clone(),
            false => own.writer.clone(),
        },
        artist: own.artist.clone(),
        id: issue.map_or_else(|| volume.id.clone(), |i| i.id.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(name: &str) -> ComicGuess {
        parse_name(name, "")
    }

    #[test]
    fn names() {
        let x = g("Saga 012 (2013) (Digital) (Zone-Empire)");
        assert_eq!(
            (x.series.as_str(), x.number.as_str(), x.year),
            ("Saga", "012", Some(2013))
        );
        let x = g("Batman v2 #12 (2012)");
        assert_eq!(
            (x.series.as_str(), x.number.as_str(), x.volume),
            ("Batman", "12", Some(2))
        );
        let x = g("2000 AD 123");
        assert_eq!((x.series.as_str(), x.number.as_str()), ("2000 AD", "123"));
        let x = g("Watchmen 03 (of 12)");
        assert_eq!((x.number.as_str(), x.count), ("03", Some(12)));
        let x = g("One Piece v03");
        assert_eq!(
            (x.series.as_str(), x.volume, x.number.as_str()),
            ("One Piece", Some(3), "")
        );
        let x = g("Berserk Vol. 3 Ch. 21");
        assert_eq!(
            (x.series.as_str(), x.volume, x.chapter.as_str()),
            ("Berserk", Some(3), "21")
        );
        let x = g("Naruto c045");
        assert_eq!((x.series.as_str(), x.chapter.as_str()), ("Naruto", "045"));
        let x = g("Asterix - Tome 05 - Le Tour de Gaule");
        assert_eq!(
            (x.series.as_str(), x.volume, x.title.as_str()),
            ("Asterix", Some(5), "Le Tour de Gaule")
        );
        let x = g("Turma da Mônica nº 12");
        assert_eq!(
            (x.series.as_str(), x.number.as_str()),
            ("Turma da Mônica", "12")
        );
        let x = g("Spawn #12.1");
        assert_eq!(x.number, "12.1");
        let x = g("The.Walking.Dead.012.2005");
        assert_eq!(
            (x.series.as_str(), x.number.as_str(), x.year),
            ("The Walking Dead", "012", Some(2005))
        );
        let x = g("03 - Desenhando Quadrinhos");
        assert_eq!(
            (x.series.as_str(), x.number.as_str(), x.title.as_str()),
            ("Desenhando Quadrinhos", "03", "")
        );
        let x = g("Rei Conan\u{2d0} A Cidadela Escarlate #3");
        assert_eq!(
            (x.series.as_str(), x.number.as_str()),
            ("Rei Conan: A Cidadela Escarlate", "3")
        );
        let x = g("Hellboy 001 - Seed of Destruction");
        assert_eq!(x.title, "Seed of Destruction");
        // A number alone, in the series' folder.
        let x = parse_name("012", "Saga (2012)");
        assert_eq!(
            (x.series.as_str(), x.number.as_str(), x.series_year),
            ("Saga", "012", Some(2012))
        );
        let x = parse_name("Capítulo 7", "Vinland Saga Vol. 2");
        assert_eq!(
            (x.series.as_str(), x.volume, x.chapter.as_str()),
            ("Vinland Saga", Some(2), "7")
        );
    }

    #[test]
    fn escaped_letters() {
        let x = g("Biblioteca_Mauricio_de_Sousa_-_M_244_nica_1970");
        assert_eq!(
            (x.series.as_str(), x.number.as_str(), x.year),
            ("Biblioteca Mauricio de Sousa - Mônica", "", Some(1970))
        );
        let x = g("V_243_ltar_-_Alfredo_Alcal_225");
        assert_eq!(
            (x.series.as_str(), x.number.as_str()),
            ("Vóltar - Alfredo Alcalá", "")
        );
        assert_eq!(g("Series_012_(2019)").number, "012");
    }

    #[test]
    fn info() {
        let i = parse_info(
            r#"<?xml version="1.0"?><ComicInfo xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <Title>Chapter One</Title><Series>Saga</Series><Number>1</Number><Count>54</Count>
  <Volume>2012</Volume><Year>2012</Year><Publisher>Image</Publisher><Writer>Brian K. Vaughan</Writer>
  <Penciller>Fiona Staples</Penciller><Web>https://comicvine.gamespot.com/saga-1-chapter-one/4000-321297/</Web>
  <Manga>No</Manga></ComicInfo>"#,
        )
        .unwrap();
        let m = merge(&g("saga1"), &i);
        assert_eq!(
            (
                m.series.as_str(),
                m.number.as_str(),
                m.series_year,
                m.volume
            ),
            ("Saga", "1", Some(2012), None)
        );
        assert_eq!(
            (m.writer.as_str(), m.id.as_str()),
            ("Brian K. Vaughan", "321297")
        );
        assert!(parse_info("<x/>").is_none());
    }

    #[test]
    fn naming() {
        let t = "{série} v{volume} #{número} c{capítulo} - {título} ({ano})";
        let m = ComicMatch {
            series: "Saga".into(),
            number: "12".into(),
            year: Some(2013),
            ..Default::default()
        };
        assert_eq!(comic_name(t, &m, " - "), "Saga #012 (2013)");
        let m = ComicMatch {
            series: "One Piece".into(),
            volume: Some(3),
            ..Default::default()
        };
        assert_eq!(comic_name(t, &m, " - "), "One Piece v03");
        let m = ComicMatch {
            series: "Berserk".into(),
            volume: Some(3),
            chapter: "21".into(),
            ..Default::default()
        };
        assert_eq!(comic_name(t, &m, " - "), "Berserk v03 c021");
        assert_eq!(
            comic_name("{series} Vol. {volume} - {title}", &m, " - "),
            "Berserk Vol. 03"
        );
        let m = ComicMatch {
            series: "Spawn".into(),
            number: "12.1".into(),
            ..Default::default()
        };
        assert_eq!(comic_name("{series} {number}", &m, " - "), "Spawn 012.1");
        assert_eq!(padded("½", 3), "½");
        assert_eq!(padded("0", 3), "000");
        assert_eq!(
            folders(
                "{série} ({ano da série})",
                &ComicMatch {
                    series: "Saga".into(),
                    series_year: Some(2012),
                    ..m.clone()
                },
                " - "
            ),
            ["Saga (2012)"]
        );
    }

    #[test]
    fn issues() {
        let issues: Vec<Issue> = ["1", "2", "12.1", "12"]
            .iter()
            .enumerate()
            .map(|(i, n)| Issue {
                id: i.to_string(),
                number: n.to_string(),
                ..Default::default()
            })
            .collect();
        let m = |number: &str, volume: Option<u32>| ComicMatch {
            number: number.into(),
            volume,
            ..Default::default()
        };
        assert_eq!(find_issue(&m("012", None), &issues).unwrap().id, "3");
        assert_eq!(find_issue(&m("12.10", None), &issues).unwrap().id, "2");
        assert_eq!(find_issue(&m("", Some(2)), &issues).unwrap().id, "1");
        assert!(find_issue(&m("7", None), &issues).is_none());
        // A one-shot: the first of its unnumbered printings.
        let printings: Vec<Issue> = ["", "[2ª Edição]"]
            .iter()
            .enumerate()
            .map(|(i, d)| Issue {
                id: i.to_string(),
                number: issue_number(d),
                ..Default::default()
            })
            .collect();
        assert_eq!(find_issue(&m("", None), &printings).unwrap().id, "0");
        assert_eq!(
            (issue_number("12 [variante]"), issue_number("[nn]")),
            ("12".to_string(), String::new())
        );
        let v = Volume {
            id: "9".into(),
            name: "Série".into(),
            start_year: Some(2012),
            publisher: "Editora".into(),
            count: 54,
            ..Default::default()
        };
        let i = Issue {
            id: "5".into(),
            number: "3".into(),
            name: "Capítulo Três".into(),
            cover_date: "2012-05-01".into(),
            cover: String::new(),
        };
        let out = with_issue(&m("", Some(3)), &v, Some(&i));
        // A series without issues (MangaDex): the file's numbers stay.
        let alone = with_issue(&m("", Some(3)), &v, None);
        assert_eq!(
            (alone.series.as_str(), alone.volume, alone.id.as_str()),
            ("Série", Some(3), "9")
        );
        assert_eq!(
            (
                out.series.as_str(),
                out.number.as_str(),
                out.volume,
                out.year,
                out.count
            ),
            ("Série", "", Some(3), Some(2012), Some(54))
        );
    }

    #[test]
    fn volume_order() {
        let v = |name: &str| Volume {
            name: name.into(),
            ..Default::default()
        };
        let found = vec![
            v("Tempo Mestres"),
            v("Mestres do Universo"),
            v("Os Mestres"),
            v("Os Mestres: Especial"),
        ];
        let names: Vec<String> = rank_volumes("Os Mestres", found, "pt")
            .into_iter()
            .map(|v| v.name)
            .collect();
        assert_eq!(names[..2], ["Os Mestres", "Os Mestres: Especial"]);
        let own = ComicMatch {
            title: "Título do arquivo".into(),
            ..Default::default()
        };
        let out = with_issue(
            &own,
            &Volume::default(),
            Some(&Issue {
                number: "1".into(),
                ..Default::default()
            }),
        );
        assert_eq!(
            (out.number.as_str(), out.title.as_str()),
            ("", "Título do arquivo")
        );
    }

    #[test]
    fn library_names() {
        let n = |s: &str| {
            let g = parse_name(s, "");
            (g.series, g.number, g.volume)
        };
        assert_eq!(
            n("Arqueiro Verde Vol. 5 Anual #01"),
            ("Arqueiro Verde Anual".into(), "01".into(), Some(5))
        );
        assert_eq!(
            n("Tintim - 22 - Voo 714 para Sidney"),
            ("Tintim".into(), "22".into(), None)
        );
        assert_eq!(
            n("[1995] Calvin & Hobbes - 10 Anos Vol. II"),
            ("Calvin & Hobbes - 10 Anos".into(), "".into(), Some(2))
        );
        assert_eq!(n("47 Ronin - 3"), ("47 Ronin".into(), "3".into(), None));
        assert_eq!(n("1984 em Quadrinhos - Orwell").1, "");
        assert_eq!(n("Elric - Vol III"), ("Elric".into(), "".into(), Some(3)));
        assert_eq!(
            n("Conan o Bárbaro - A Era Marvel (Vol.03)"),
            ("Conan o Bárbaro - A Era Marvel".into(), "".into(), Some(3))
        );
        let g = parse_name("Conan Omnibus (Vol.01) - Nasce uma lenda", "");
        assert_eq!(
            (g.series.as_str(), g.volume, g.title.as_str()),
            ("Conan Omnibus", Some(1), "Nasce uma lenda")
        );
        assert_eq!(n("Arqueiro Verde Vol. 2 #1000000").1, "1000000");
        let g = parse_name("Evnt Lvt #2 de 6 (2019)", "");
        assert_eq!(
            (g.number.as_str(), g.count, g.title.as_str()),
            ("2", Some(6), "")
        );
        assert_eq!(parse_folder("Arsenal (1998-1999)").series_year, Some(1998));
        assert_eq!(
            parse_folder("Arqueiro Verde Vol. 5 (2011 – 2016) [Os Novos 52]").series_year,
            Some(2011)
        );
        assert_eq!(
            parse_folder("Exterminador (2016—2020)").series_year,
            Some(2016)
        );
    }

    #[test]
    fn already_in_place() {
        let m = |series: &str, volume| ComicMatch {
            series: series.into(),
            volume,
            ..Default::default()
        };
        let dir = Path::new("/q/Arqueiro Verde Vol. 2 (1988-1998)");
        assert!(in_place(dir, &m("Arqueiro Verde", Some(2))));
        assert!(!in_place(dir, &m("Arqueiro Verde", Some(3))));
        assert!(in_place(
            Path::new("/q/Death Note"),
            &m("Death Note - Short Stories", None)
        ));
        assert!(in_place(
            Path::new("/q/Mulher Gato"),
            &m("Mulher-Gato Especial", None)
        ));
        assert!(!in_place(
            Path::new("/q/Pateta Faz História (Completo)"),
            &m("Teatro Disney", None)
        ));
        let pr = "Power Rangers & Tartarugas Ninja";
        assert!(in_place(
            Path::new("/q/Power Rangers - Tartarugas Ninja/Vol. 1"),
            &m(pr, Some(1))
        ));
        assert!(!in_place(
            Path::new("/q/Outra Série/Vol. 1"),
            &m(pr, Some(1))
        ));
        let sc = Path::new("/q/Scooby Apocalipse [parc.trad.] (2016-2019)");
        assert!(in_place(sc, &m("Scooby Apocalypse", None)));
        assert!(!in_place(Path::new("/q/Batman"), &m("Batwoman", None)));
    }

    #[test]
    fn arranged_as_a_library() {
        let c = |series: &str| ComicMatch {
            series: series.into(),
            ..Default::default()
        };
        let mut all = vec![
            c("Scooby Apocalypse"),
            c("Scooby Apocalypse"),
            c("Scooby Apocalipse"),
            c("Evento leviatã"),
            c("Evento Leviatã"),
            c("Evento Leviatã"),
            c("Arqueiro Verde"),
            c("Arqueiro Verde"),
            c("Arqueiro Verde - Ano Um"),
            c("Arqueiro Verde e Canário Negro"),
            c("Arqueiro Verde e Canário Negro"),
            c("Arqueiro Verde e Canário Negro Especial de Casamento"),
            c("Calvin & Haroldo - Álbum Um"),
            c("Calvin & Haroldo - Álbum Dois"),
            c("Aquaman - Arqueiro Verde"),
            c("Aquaman - Arqueiro Verde"),
            c("Red Sonja - A Balada"),
        ];
        let folders = arrange(&mut all);
        assert_eq!(all[2].series, "Scooby Apocalypse");
        assert_eq!(all[3].series, "Evento Leviatã");
        let f = |i: usize| folders[i].as_ref().map(|m| m.series.as_str());
        assert_eq!(f(0), None);
        assert_eq!(f(8), Some("Arqueiro Verde"));
        assert_eq!(f(11), Some("Arqueiro Verde e Canário Negro"));
        assert_eq!(
            (f(12), f(13)),
            (Some("Calvin & Haroldo"), Some("Calvin & Haroldo"))
        );
        assert_eq!((f(14), f(16)), (None, None));
    }

    #[test]
    fn translated_names_keep_their_series() {
        let g = parse_name("Xeque-Mate Vol. 1 #02", "");
        let info = ComicInfo {
            series: "Checkmate".into(),
            number: "2".into(),
            title: "A House Divided".into(),
            ..Default::default()
        };
        let m = merge(&g, &info);
        assert_eq!(
            (m.series.as_str(), m.title.as_str(), m.number.as_str()),
            ("Xeque-Mate", "", "02")
        );
    }

    #[test]
    fn numbered_stories() {
        let folder = "Pateta Faz História (Completo)";
        let g = parse_name("19 - A Volta ao Mundo em 80 Dias e Aladim", folder);
        assert_eq!(
            (g.series.as_str(), g.number.as_str()),
            ("A Volta ao Mundo em 80 Dias e Aladim", "19")
        );
        let mut m = merge(&g, &ComicInfo::default());
        in_folder(&mut m, folder);
        assert_eq!(
            (m.series.as_str(), m.number.as_str()),
            ("Pateta Faz História", "19")
        );
        assert_eq!(m.title, "A Volta ao Mundo em 80 Dias e Aladim");
        let mut m = merge(
            &parse_name("Pateta Faz História 03 - Teatro", folder),
            &ComicInfo::default(),
        );
        in_folder(&mut m, folder);
        assert_eq!(
            (m.series.as_str(), m.number.as_str(), m.title.as_str()),
            ("Pateta Faz História", "03", "Teatro")
        );
        let mut m = merge(
            &parse_name("Pateta Faz História 04", "Pateta Faz História (2001)"),
            &ComicInfo::default(),
        );
        in_folder(&mut m, "Pateta Faz História (2001)");
        assert_eq!((m.title.as_str(), m.series_year), ("", Some(2001)));
    }

    #[test]
    fn stories() {
        assert_eq!(
            story("Rei Conan: A Cidadela Escarlate", "Rei Conan"),
            "A Cidadela Escarlate"
        );
        assert_eq!(story("Série - História", "Série"), "História");
        assert_eq!(story("Séries", "Série"), "");
        assert_eq!(story("Série", "Série"), "");
    }

    #[test]
    fn library() {
        let m = ComicMatch {
            series: "Saga".into(),
            series_year: Some(2012),
            ..Default::default()
        };
        let levels = vec!["Saga (2012)".to_string()];
        assert_eq!(
            library_root(Path::new("/c/Saga"), &m, &levels),
            Path::new("/c")
        );
        assert_eq!(
            library_root(Path::new("/c/Saga (2012)"), &m, &levels),
            Path::new("/c")
        );
        assert_eq!(
            library_root(Path::new("/c/Quadrinhos"), &m, &levels),
            Path::new("/c/Quadrinhos")
        );
    }
}
