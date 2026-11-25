//! Details written inside comic and book files: the `ComicInfo.xml` of a
//! CBZ and the OPF of an EPUB. The archive is written again beside the
//! file with every other entry copied as it is (pages are not compressed
//! again), then takes the file's place; what the entry held is kept for
//! undo, like a song's tags (see [`crate::tagwrite`]).

use crate::files::ext_of;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use chrono::{Datelike, Timelike};
use serde::{Deserialize, Serialize};

use crate::books::BookMatch;
use crate::comics::{ComicInfo, ComicMatch};
use crate::tagwrite::{Backup, temp_of};
use crate::text;
use crate::tmdb::xml as esc;

/// What an archive's entry held before it was written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Its name in the archive.
    pub name: String,
    /// The file (in the history's folder) with its old content; `None`
    /// when the entry was added.
    pub old: Option<String>,
}

const INFO: &str = "ComicInfo.xml";
/// An entry with details is a few kilobytes; more is not one.
const MAX_TEXT: u64 = 8 << 20;

/// A zip by its content (a `.cbz` is sometimes a 7z or a RAR).
fn is_zip(path: &Path) -> bool {
    let mut head = [0u8; 4];
    File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok()
        && (head == *b"PK\x03\x04" || head == *b"PK\x05\x06")
}

/// Comics whose details can be written: CBZ files that are zips.
pub fn comic_writable(path: &Path) -> bool {
    ext_of(path) == "cbz" && is_zip(path)
}

/// Books whose details can be written: EPUB files.
pub fn book_writable(path: &Path) -> bool {
    matches!(ext_of(path).as_str(), "epub" | "kepub") && is_zip(path)
}

// ---------------------------------------------------------------- archive

fn read_text<R: Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<String, String> {
    let f = zip.by_name(name).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    f.take(MAX_TEXT)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|_| "notUtf8".to_string())
}

fn names<R: Read + std::io::Seek>(zip: &zip::ZipArchive<R>) -> Vec<String> {
    zip.file_names()
        .filter_map(Result::ok)
        .map(|n| n.into_owned())
        .collect()
}

fn open(path: &Path) -> Result<zip::ZipArchive<BufReader<File>>, String> {
    let f = File::open(path).map_err(|e| e.to_string())?;
    zip::ZipArchive::new(BufReader::new(f)).map_err(broken)
}

/// An archive that cannot be read (or copied) as it is.
fn broken(e: zip::result::ZipError) -> String {
    format!("archiveBroken: {e}")
}

fn now() -> zip::DateTime {
    let t = chrono::Local::now().naive_local();
    zip::DateTime::from_date_and_time(
        t.year().clamp(1980, 2107) as u16,
        t.month() as u8,
        t.day() as u8,
        t.hour() as u8,
        t.minute() as u8,
        t.second() as u8,
    )
    .unwrap_or_default()
}

/// Writes the archive again with entry `name` holding `data` (in its place,
/// or last when it is new), or without it for `None`; the other entries
/// and the archive's comment are copied as they are.
fn rewrite(path: &Path, name: &str, data: Option<&[u8]>) -> Result<(), String> {
    let tmp = temp_of(path);
    let written = (|| -> Result<(), String> {
        let err = broken;
        let mut zin = open(path)?;
        let out = File::create(&tmp).map_err(|e| e.to_string())?;
        let mut zout = zip::ZipWriter::new(BufWriter::new(out));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(now());
        let put = |zout: &mut zip::ZipWriter<BufWriter<File>>, data: &[u8]| -> Result<(), String> {
            zout.start_file(name, options).map_err(err)?;
            zout.write_all(data).map_err(|e| e.to_string())
        };
        let mut placed = false;
        for i in 0..zin.len() {
            let f = zin.by_index_raw(i).map_err(err)?;
            if f.name().map_err(err)? == name {
                if let Some(d) = data
                    && !placed
                {
                    drop(f);
                    put(&mut zout, d)?;
                    placed = true;
                }
                continue;
            }
            zout.raw_copy_file(f).map_err(err)?;
        }
        if !placed && let Some(d) = data {
            put(&mut zout, d)?;
        }
        zout.set_raw_comment(zin.comment().into()).map_err(err)?;
        let w = zout.finish().map_err(err)?;
        let file = w.into_inner().map_err(|e| e.error().to_string())?;
        if let Ok(m) = fs::metadata(path) {
            let _ = file.set_permissions(m.permissions());
        }
        file.sync_all().map_err(|e| e.to_string())
    })();
    match written {
        Ok(()) => fs::rename(&tmp, path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            e.to_string()
        }),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Keeps an entry's old content in `store`; returns the file's name.
fn keep(text: &str, store: &Path) -> Result<String, String> {
    fs::create_dir_all(store).map_err(|e| e.to_string())?;
    let file = format!("{:016x}.txt", xxhash_rust::xxh3::xxh3_64(text.as_bytes()));
    let at = store.join(&file);
    if !at.exists() {
        fs::write(&at, text).map_err(|e| e.to_string())?;
    }
    Ok(file)
}

/// Writes `new` as entry `name` (which held `old`), keeping `old` for undo.
fn write_entry(
    path: &Path,
    name: &str,
    old: Option<&str>,
    new: &str,
    store: &Path,
) -> Result<Backup, String> {
    let kept = old.map(|o| keep(o, store)).transpose()?;
    rewrite(path, name, Some(new.as_bytes()))?;
    Ok(Backup {
        path: path.to_path_buf(),
        fields: Vec::new(),
        pictures: None,
        cleaned: false,
        removed: Vec::new(),
        raw: Vec::new(),
        kept_text: Vec::new(),
        saved: None,
        video: None,
        converted: None,
        entry: Some(Entry {
            name: name.to_string(),
            old: kept,
        }),
    })
}

/// Puts back what an entry held (or takes out one that was added).
pub fn restore(path: &Path, e: &Entry, store: &Path) -> Result<(), String> {
    let old = match &e.old {
        Some(f) => Some(fs::read(store.join(f)).map_err(|e| e.to_string())?),
        None => None,
    };
    rewrite(path, &e.name, old.as_deref())
}

// ---------------------------------------------------------------- comics

/// The order of the elements in `ComicInfo.xml` (its schema's).
const ORDER: &[&str] = &[
    "Title",
    "Series",
    "Number",
    "Count",
    "Volume",
    "AlternateSeries",
    "AlternateNumber",
    "AlternateCount",
    "Summary",
    "Notes",
    "Year",
    "Month",
    "Day",
    "Writer",
    "Penciller",
    "Inker",
    "Colorist",
    "Letterer",
    "CoverArtist",
    "Editor",
    "Translator",
    "Publisher",
    "Imprint",
    "Genre",
    "Tags",
    "Web",
    "PageCount",
    "LanguageISO",
    "Format",
    "BlackAndWhite",
    "Manga",
    "Characters",
    "Teams",
    "Locations",
    "ScanInformation",
    "StoryArc",
    "StoryArcNumber",
    "SeriesGroup",
    "AgeRating",
    "Pages",
    "CommunityRating",
    "MainCharacterOrTeam",
    "Review",
    "GTIN",
];

/// The elements written from a match's extra fields (see
/// [`crate::comics::parse_info`], which reads them).
const EXTRA: &[(&str, &str)] = &[
    ("StoryArc", "story arc"),
    ("Imprint", "imprint"),
    ("Format", "format"),
    ("Genre", "genre"),
    ("Inker", "inker"),
    ("Colorist", "colorist"),
    ("Letterer", "letterer"),
    ("CoverArtist", "cover artist"),
    ("Editor", "editor"),
    ("Translator", "translator"),
    ("AgeRating", "age rating"),
    ("LanguageISO", "language"),
    ("AlternateSeries", "alternate series"),
    ("AlternateNumber", "alternate number"),
    ("GTIN", "gtin"),
];

/// What goes into a comic's `ComicInfo.xml`, by element (empty values
/// are left out). `web`: the issue's page in the catalogue; `manga`: it is
/// one.
pub fn comic_values(m: &ComicMatch, web: &str, manga: bool) -> Vec<(&'static str, String)> {
    let number = match m.number.is_empty() {
        true => m.chapter.clone(),
        false => m.number.clone(),
    };
    let int = |k: &str| {
        m.extra
            .get(k)
            .and_then(|v| v.parse::<u8>().ok())
            .map(|n| n.to_string())
            .unwrap_or_default()
    };
    let mut out: Vec<(&'static str, String)> = vec![
        ("Title", m.title.clone()),
        ("Series", m.series.clone()),
        ("Number", number),
        ("Count", m.count.map(|c| c.to_string()).unwrap_or_default()),
        (
            "Volume",
            m.volume.map(|v| v.to_string()).unwrap_or_default(),
        ),
        ("Year", m.year.map(|y| y.to_string()).unwrap_or_default()),
        ("Month", int("month")),
        ("Day", int("day")),
        ("Writer", m.writer.clone()),
        ("Penciller", m.artist.clone()),
        ("Publisher", m.publisher.clone()),
        ("Web", web.to_string()),
        ("Manga", if manga { "Yes".into() } else { String::new() }),
    ];
    for (tag, key) in EXTRA {
        out.push((tag, m.extra.get(*key).cloned().unwrap_or_default()));
    }
    out.into_iter()
        .map(|(k, v)| (k, text::collapse_spaces(v.trim())))
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

/// The same elements, as a comic's own `ComicInfo.xml` has them.
fn info_values(i: &ComicInfo) -> Vec<(&'static str, String)> {
    let int = |k: &str| {
        i.extra
            .get(k)
            .and_then(|v| v.parse::<u8>().ok())
            .map(|n| n.to_string())
            .unwrap_or_default()
    };
    let mut out: Vec<(&'static str, String)> = vec![
        ("Title", i.title.clone()),
        ("Series", i.series.clone()),
        ("Number", i.number.clone()),
        ("Count", i.count.map(|c| c.to_string()).unwrap_or_default()),
        ("Volume", i.volume.clone()),
        ("Year", i.year.map(|y| y.to_string()).unwrap_or_default()),
        ("Month", int("month")),
        ("Day", int("day")),
        ("Writer", i.writer.clone()),
        ("Penciller", i.penciller.clone()),
        ("Publisher", i.publisher.clone()),
    ];
    for (tag, key) in EXTRA {
        out.push((tag, i.extra.get(*key).cloned().unwrap_or_default()));
    }
    out
}

/// Whether writing `m` would change what the comic says about itself
/// (`info`: what it has, `None` for nothing). Catalogue links are not
/// compared: they never decide alone.
pub fn comic_differs(m: &ComicMatch, info: Option<&ComicInfo>) -> bool {
    let Some(info) = info else {
        return true;
    };
    let have = info_values(info);
    comic_values(m, "", false).iter().any(|(k, v)| {
        have.iter()
            .find(|(h, _)| h == k)
            .is_none_or(|(_, old)| old != v)
    })
}

/// The `ComicInfo.xml` with `values` in it: the elements of `old` (when
/// it is one) are kept but for those `values` change, all in the schema's
/// order. `None` when `old` already has them all.
pub fn comic_xml(old: Option<&str>, values: &[(&'static str, String)]) -> Option<String> {
    comic_xml_with(old, values, false)
}

/// Like [`comic_xml`]; `fresh`: of `old`, only the list of pages (which
/// tells readers the cover and double pages) is kept.
pub fn comic_xml_with(
    old: Option<&str>,
    values: &[(&'static str, String)],
    fresh: bool,
) -> Option<String> {
    let old = old.map(|o| o.trim_start_matches('\u{feff}'));
    let doc = old.and_then(|o| roxmltree::Document::parse(o).ok());
    let root = doc
        .as_ref()
        .map(|d| d.root_element())
        .filter(|r| r.tag_name().name().eq_ignore_ascii_case("ComicInfo"));
    // (element name, its XML) in the file's order.
    let mut children: Vec<(String, String)> = match (root, old) {
        (Some(r), Some(src)) => r
            .children()
            .filter(|n| n.is_element())
            .map(|n| (n.tag_name().name().to_string(), src[n.range()].to_string()))
            .collect(),
        _ => Vec::new(),
    };
    let before = children.len();
    if fresh {
        children.retain(|(n, _)| {
            n.eq_ignore_ascii_case("Pages") || values.iter().any(|(v, _)| v.eq_ignore_ascii_case(n))
        });
    }
    let current = |name: &str, children: &[(String, String)]| -> Option<String> {
        let (_, raw) = children
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))?;
        let d = roxmltree::Document::parse(raw).ok()?;
        Some(text::collapse_spaces(
            d.root_element().text().unwrap_or_default().trim(),
        ))
    };
    let mut changed = root.is_none() || children.len() != before;
    for (name, value) in values {
        if current(name, &children).as_deref() == Some(value.as_str()) {
            continue;
        }
        changed = true;
        let new = format!("<{name}>{}</{name}>", esc(value));
        match children
            .iter_mut()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
        {
            Some(c) => *c = (name.to_string(), new),
            None => children.push((name.to_string(), new)),
        }
    }
    if !changed {
        return None;
    }
    let rank = |n: &str| {
        ORDER
            .iter()
            .position(|o| o.eq_ignore_ascii_case(n))
            .unwrap_or(ORDER.len())
    };
    children.sort_by_key(|(n, _)| rank(n));
    let mut o = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ComicInfo \
         xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n",
    );
    for (_, raw) in &children {
        o.push_str("  ");
        o.push_str(raw);
        o.push('\n');
    }
    o.push_str("</ComicInfo>\n");
    Some(o)
}

/// Writes `values` into the comic's `ComicInfo.xml` (made when it has
/// none). Returns what to put back, or `None` when it already had them.
pub fn write_comic(
    path: &Path,
    values: &[(&'static str, String)],
    store: &Path,
) -> Result<Option<Backup>, String> {
    write_comic_with(path, values, false, store)
}

/// The comic's `ComicInfo.xml` (its name in the archive) and text.
fn comic_entry(path: &Path) -> Result<(Option<String>, Option<String>), String> {
    let mut zip = open(path)?;
    let all = names(&zip);
    // The one a reader takes: at the root first.
    let found = all
        .iter()
        .find(|n| n.eq_ignore_ascii_case(INFO))
        .or_else(|| {
            all.iter().find(|n| {
                n.rsplit('/')
                    .next()
                    .is_some_and(|f| f.eq_ignore_ascii_case(INFO))
            })
        })
        .cloned();
    let old = match &found {
        Some(n) => Some(read_text(&mut zip, n)?),
        None => None,
    };
    Ok((found, old))
}

/// Like [`write_comic`]; `fresh`: what the `ComicInfo.xml` had goes (but
/// its list of pages), so only `values` are left.
pub fn write_comic_with(
    path: &Path,
    values: &[(&'static str, String)],
    fresh: bool,
    store: &Path,
) -> Result<Option<Backup>, String> {
    if !comic_writable(path) {
        return Err("comicUnsupported".into());
    }
    let (found, old) = comic_entry(path)?;
    let Some(new) = comic_xml_with(old.as_deref(), values, fresh) else {
        return Ok(None);
    };
    let name = found.unwrap_or_else(|| INFO.to_string());
    write_entry(path, &name, old.as_deref(), &new, store).map(Some)
}

/// Takes the `ComicInfo.xml` out of a CBZ, kept for undo. `None` when it
/// has none.
pub fn remove_comic(path: &Path, store: &Path) -> Result<Option<Backup>, String> {
    if !comic_writable(path) {
        return Err("comicUnsupported".into());
    }
    let (Some(name), Some(old)) = comic_entry(path)? else {
        return Ok(None);
    };
    let kept = keep(&old, store)?;
    rewrite(path, &name, None)?;
    Ok(Some(Backup {
        path: path.to_path_buf(),
        fields: Vec::new(),
        pictures: None,
        cleaned: false,
        removed: Vec::new(),
        raw: Vec::new(),
        kept_text: Vec::new(),
        saved: None,
        video: None,
        converted: None,
        entry: Some(Entry {
            name,
            old: Some(kept),
        }),
    }))
}

// ---------------------------------------------------------------- books

const DC: &str = "http://purl.org/dc/elements/1.1/";

/// What goes into an EPUB's details (empty values are left alone).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookValues {
    pub title: String,
    pub authors: Vec<String>,
    pub year: String,
    pub publisher: String,
    /// A language code (`pt`, `en`).
    pub language: String,
    pub isbn: String,
    pub series: String,
    pub series_index: String,
}

/// The values of a book match; `language` as Open Library gives it.
pub fn book_values(m: &BookMatch, language: &str) -> BookValues {
    let tidy = |s: &str| text::collapse_spaces(s.trim());
    BookValues {
        title: tidy(&m.title),
        authors: m
            .authors
            .iter()
            .map(|a| tidy(a))
            .filter(|a| !a.is_empty())
            .collect(),
        year: m
            .edition_year
            .or(m.year)
            .map(|y| y.to_string())
            .unwrap_or_default(),
        publisher: tidy(&m.publisher),
        language: crate::books::language(language.trim()),
        isbn: m.isbn.clone(),
        series: tidy(&m.series),
        series_index: tidy(&m.series_index),
    }
}

/// Text edits on a document: byte ranges and what takes their place.
#[derive(Default)]
struct Edits(Vec<(std::ops::Range<usize>, String)>);

impl Edits {
    fn put(&mut self, at: std::ops::Range<usize>, s: impl Into<String>) {
        self.0.push((at, s.into()));
    }

    fn apply(mut self, src: &str) -> String {
        self.0.sort_by_key(|(r, _)| (r.start, r.end));
        let mut o = String::with_capacity(src.len() + 512);
        let mut at = 0;
        for (r, s) in self.0 {
            if r.start < at {
                continue;
            }
            o.push_str(&src[at..r.start]);
            o.push_str(&s);
            at = r.end;
        }
        o.push_str(&src[at..]);
        o
    }
}

fn node_text(n: roxmltree::Node) -> String {
    text::collapse_spaces(n.text().unwrap_or_default().trim())
}

/// The element and the whitespace before it on its line, to take it out
/// without leaving an empty line.
fn whole_line(src: &str, r: std::ops::Range<usize>) -> std::ops::Range<usize> {
    let before = &src[..r.start];
    let start = before.trim_end_matches([' ', '\t']).len();
    let start = match before[..start].ends_with('\n') {
        true => start - 1,
        false => r.start,
    };
    start..r.end
}

/// Sets an element's text, keeping its tags and attributes.
fn set_text(e: &mut Edits, src: &str, n: roxmltree::Node, value: &str) {
    let r = n.range();
    let raw = &src[r.clone()];
    if let Some(open) = raw.strip_suffix("/>") {
        let open = open.trim_end();
        let name = raw[1..]
            .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .next()
            .unwrap_or_default();
        e.put(r, format!("{open}>{}</{name}>", esc(value)));
        return;
    }
    match n.first_child().filter(|c| c.is_text()) {
        Some(t) if n.children().count() == 1 => e.put(t.range(), esc(value)),
        _ => {
            // Start tag end and end tag start.
            let open_end = raw.find('>').map(|i| r.start + i + 1).unwrap_or(r.start);
            let close_start = raw.rfind("</").map(|i| r.start + i).unwrap_or(r.end);
            e.put(open_end..close_start, esc(value));
        }
    }
}

fn same_language(a: &str, b: &str) -> bool {
    let main = |s: &str| {
        s.split(['-', '_'])
            .next()
            .unwrap_or_default()
            .to_lowercase()
    };
    main(a) == main(b)
}

fn is_dc(n: &roxmltree::Node, name: &str) -> bool {
    n.tag_name().name() == name && n.tag_name().namespace() == Some(DC)
}

/// What an edit of the details gives: changes in place, and new elements
/// to add before the end of the details.
#[derive(Default)]
struct Changes {
    edits: Edits,
    added: Vec<String>,
}

/// The OPF being edited: where its details are and how they are written.
struct Opf<'a> {
    src: &'a str,
    doc: &'a roxmltree::Document<'a>,
    epub3: bool,
    uid: Option<&'a str>,
    /// The details' elements (the children of `metadata`).
    elements: Vec<roxmltree::Node<'a, 'a>>,
    /// The prefix the details' elements are written with (`dc:`, or none)
    /// and the namespace declaration they need when it is not declared.
    dc: String,
    declare: String,
}

impl<'a> Opf<'a> {
    /// A new details element, written as the others are.
    fn el(&self, name: &str, value: &str) -> String {
        format!(
            "<{dc}{name}{declare}>{}</{dc}{name}>",
            esc(value),
            dc = self.dc,
            declare = self.declare
        )
    }

    /// The first details element called `name`.
    fn first(&self, name: &str) -> Option<roxmltree::Node<'a, 'a>> {
        self.elements.iter().find(|n| is_dc(n, name)).copied()
    }

    /// The `meta` that gives `property` to the element with this id.
    fn refining(&self, id: Option<&str>, property: &str) -> Option<&roxmltree::Node<'a, 'a>> {
        id.and_then(|id| {
            self.elements.iter().find(|m| {
                m.tag_name().name() == "meta"
                    && m.attribute("refines").map(|r| r.trim_start_matches('#')) == Some(id)
                    && m.attribute("property") == Some(property)
            })
        })
    }

    /// The places of every `meta` that refines the element `n`.
    fn refines_of(&self, n: &roxmltree::Node) -> Vec<std::ops::Range<usize>> {
        let Some(id) = n.attribute("id") else {
            return Vec::new();
        };
        self.elements
            .iter()
            .filter(|m| {
                m.tag_name().name() == "meta"
                    && m.attribute("refines").map(|r| r.trim_start_matches('#')) == Some(id)
            })
            .map(|m| m.range())
            .collect()
    }

    fn title(&self, v: &BookValues, c: &mut Changes) {
        if v.title.is_empty() {
            return;
        }
        let main = self.elements.iter().find(|n| {
            is_dc(n, "title")
                && self
                    .refining(n.attribute("id"), "title-type")
                    .is_none_or(|t| node_text(*t) == "main")
        });
        match main {
            Some(n) if node_text(*n) == v.title => {}
            Some(n) => set_text(&mut c.edits, self.src, *n, &v.title),
            None => c.added.push(self.el("title", &v.title)),
        }
    }

    fn authors(&self, v: &BookValues, c: &mut Changes) {
        if v.authors.is_empty() {
            return;
        }
        let authors: Vec<&roxmltree::Node> = self
            .elements
            .iter()
            .filter(|n| {
                is_dc(n, "creator") && {
                    let role = n
                        .attributes()
                        .find(|a| a.name() == "role")
                        .map(|a| a.value().to_string())
                        .or_else(|| {
                            self.refining(n.attribute("id"), "role")
                                .map(|r| node_text(*r))
                        });
                    role.as_deref().is_none_or(|r| r == "aut")
                }
            })
            .collect();
        let have: Vec<String> = authors.iter().map(|n| node_text(**n)).collect();
        if have == v.authors {
            return;
        }
        let src = self.src;
        for (i, n) in authors.iter().enumerate() {
            match v.authors.get(i) {
                Some(a) if *a == have[i] => {}
                Some(a) => {
                    set_text(&mut c.edits, src, **n, a);
                    // The old name's sort form and the like.
                    for r in self.refines_of(n) {
                        let m = self
                            .elements
                            .iter()
                            .find(|m| m.range() == r)
                            .expect("a refined element is one of the metadata");
                        if m.attribute("property") != Some("role") {
                            c.edits.put(whole_line(src, r), "");
                        }
                    }
                    // EPUB 2 keeps the sort form in an attribute.
                    if let Some(a) = n.attributes().find(|a| a.name() == "file-as") {
                        let r = a.range();
                        let start = src[..r.start].trim_end_matches([' ', '\t', '\n']).len();
                        c.edits.put(start..r.end, "");
                    }
                }
                None => {
                    c.edits.put(whole_line(src, n.range()), "");
                    for r in self.refines_of(n) {
                        c.edits.put(whole_line(src, r), "");
                    }
                }
            }
        }
        for a in v.authors.iter().skip(authors.len()) {
            c.added.push(self.el("creator", a));
        }
    }

    fn date(&self, v: &BookValues, c: &mut Changes) {
        if v.year.is_empty() {
            return;
        }
        let date = self.elements.iter().find(|n| {
            is_dc(n, "date")
                && !n
                    .attributes()
                    .any(|a| a.name() == "event" && a.value().contains("modif"))
        });
        match date {
            Some(n) if node_text(*n).starts_with(&v.year) => {}
            Some(n) => set_text(&mut c.edits, self.src, *n, &v.year),
            None => c.added.push(self.el("date", &v.year)),
        }
    }

    fn publisher(&self, v: &BookValues, c: &mut Changes) {
        if v.publisher.is_empty() {
            return;
        }
        match self.first("publisher") {
            Some(n) if node_text(n) == v.publisher => {}
            Some(n) => set_text(&mut c.edits, self.src, n, &v.publisher),
            None => c.added.push(self.el("publisher", &v.publisher)),
        }
    }

    fn language(&self, v: &BookValues, c: &mut Changes) {
        if v.language.is_empty() {
            return;
        }
        match self.first("language") {
            Some(n) if same_language(&node_text(n), &v.language) => {}
            Some(n) => set_text(&mut c.edits, self.src, n, &v.language),
            None => c.added.push(self.el("language", &v.language)),
        }
    }

    fn isbn(&self, v: &BookValues, c: &mut Changes) {
        if v.isbn.is_empty() {
            return;
        }
        let isbns: Vec<&roxmltree::Node> = self
            .elements
            .iter()
            .filter(|n| {
                is_dc(n, "identifier") && {
                    let t = node_text(**n).to_lowercase();
                    let scheme = n
                        .attributes()
                        .any(|a| a.name() == "scheme" && a.value().to_lowercase().contains("isbn"));
                    (scheme || t.contains("isbn"))
                        || crate::ebook::isbn13(t.trim_start_matches("urn:isbn:")).is_some()
                }
            })
            .collect();
        let has = isbns.iter().any(|n| {
            crate::ebook::isbn13(
                node_text(**n)
                    .to_lowercase()
                    .trim_start_matches("urn:isbn:"),
            )
            .as_deref()
                == Some(v.isbn.as_str())
        });
        if has {
            return;
        }
        // The book's own identifier stays: readers know it by it.
        match isbns
            .iter()
            .find(|n| n.attribute("id") != self.uid || self.uid.is_none())
        {
            Some(n) => {
                let urn = node_text(**n).to_lowercase().starts_with("urn:isbn:");
                let value = match urn {
                    true => format!("urn:isbn:{}", v.isbn),
                    false => v.isbn.clone(),
                };
                set_text(&mut c.edits, self.src, **n, &value);
            }
            None => c
                .added
                .push(self.el("identifier", &format!("urn:isbn:{}", v.isbn))),
        }
    }

    /// A `meta` of the old (Calibre) kind, by its name.
    fn named(&self, name: &str) -> Option<roxmltree::Node<'a, 'a>> {
        self.elements
            .iter()
            .find(|n| n.tag_name().name() == "meta" && n.attribute("name") == Some(name))
            .copied()
    }

    /// The series in the old kind of `meta`, when the OPF has it: says
    /// whether it did.
    fn calibre_series(&self, v: &BookValues, c: &mut Changes) -> bool {
        let Some(n) = self.named("calibre:series") else {
            return false;
        };
        let set_content = |e: &mut Edits, n: roxmltree::Node, value: &str| {
            if let Some(a) = n.attributes().find(|a| a.name() == "content") {
                e.put(a.range_value(), esc(value));
            }
        };
        if n.attribute("content").map(|c| c.trim()) != Some(v.series.as_str()) {
            set_content(&mut c.edits, n, &v.series);
        }
        if !v.series_index.is_empty() {
            match self.named("calibre:series_index") {
                Some(i) if i.attribute("content").map(str::trim) == Some(&v.series_index) => {}
                Some(i) => set_content(&mut c.edits, i, &v.series_index),
                None => c.added.push(format!(
                    "<meta name=\"calibre:series_index\" content=\"{}\"/>",
                    esc(&v.series_index)
                )),
            }
        }
        true
    }

    /// The series as an EPUB 3 collection, when the OPF has one: says
    /// whether it did.
    fn collection_series(&self, v: &BookValues, c: &mut Changes) -> bool {
        let Some(n) = self
            .elements
            .iter()
            .find(|n| {
                n.tag_name().name() == "meta"
                    && n.attribute("property") == Some("belongs-to-collection")
            })
            .copied()
        else {
            return false;
        };
        if node_text(n) != v.series {
            set_text(&mut c.edits, self.src, n, &v.series);
        }
        if !v.series_index.is_empty() {
            match self.refining(n.attribute("id"), "group-position") {
                Some(p) if node_text(*p) == v.series_index => {}
                Some(p) => set_text(&mut c.edits, self.src, *p, &v.series_index),
                None if n.attribute("id").is_some() => c.added.push(format!(
                    "<meta refines=\"#{}\" property=\"group-position\">{}</meta>",
                    esc(n.attribute("id").expect("the arm checks the id")),
                    esc(&v.series_index)
                )),
                None => {}
            }
        }
        true
    }

    /// A series the OPF had no place for: both kinds are added (the
    /// collection only to an EPUB 3).
    fn new_series(&self, v: &BookValues, c: &mut Changes) {
        c.added.push(format!(
            "<meta name=\"calibre:series\" content=\"{}\"/>",
            esc(&v.series)
        ));
        if !v.series_index.is_empty() {
            c.added.push(format!(
                "<meta name=\"calibre:series_index\" content=\"{}\"/>",
                esc(&v.series_index)
            ));
        }
        if !self.epub3 {
            return;
        }
        let taken = |id: &str| {
            self.doc
                .descendants()
                .any(|n| n.attribute("id") == Some(id))
        };
        let id = (0..)
            .map(|i| match i {
                0 => "renome-series".to_string(),
                i => format!("renome-series-{i}"),
            })
            .find(|id| !taken(id))
            .expect("the numbers never run out");
        c.added.push(format!(
            "<meta property=\"belongs-to-collection\" id=\"{id}\">{}</meta>",
            esc(&v.series)
        ));
        c.added.push(format!(
            "<meta refines=\"#{id}\" property=\"collection-type\">series</meta>"
        ));
        if !v.series_index.is_empty() {
            c.added.push(format!(
                "<meta refines=\"#{id}\" property=\"group-position\">{}</meta>",
                esc(&v.series_index)
            ));
        }
    }

    fn series(&self, v: &BookValues, c: &mut Changes) {
        if v.series.is_empty() {
            return;
        }
        let calibre = self.calibre_series(v, c);
        let collection = self.collection_series(v, c);
        if !(calibre || collection) {
            self.new_series(v, c);
        }
    }
}

/// The indentation of the details' children.
fn details_indent(src: &str, meta: roxmltree::Node) -> String {
    meta.children()
        .find(|n| n.is_element())
        .map(|n| {
            let before = &src[..n.range().start];
            let line = &before[before.rfind('\n').map_or(0, |i| i + 1)..];
            match line.trim().is_empty() {
                true => line.to_string(),
                false => "    ".to_string(),
            }
        })
        .unwrap_or_else(|| "    ".to_string())
}

/// The new elements, before the closing tag, on lines of their own.
fn put_added(e: &mut Edits, added: &[String], src: &str, close: usize, indent: &str) {
    let before = &src[..close];
    let trimmed = before.trim_end_matches([' ', '\t']).len();
    let ends_line = before[..trimmed].ends_with('\n');
    let closing_indent = &before[trimmed..];
    let mut s = String::new();
    if !ends_line {
        s.push('\n');
    }
    for a in added {
        s.push_str(indent);
        s.push_str(a);
        s.push('\n');
    }
    s.push_str(closing_indent);
    let at = if ends_line { trimmed } else { close };
    e.put(at..close, s);
}

/// The EPUB's OPF with `v` in its details: elements that already say the
/// same are left alone, others get the new text, missing ones are added.
/// `None` when nothing changes, or the OPF cannot be read.
pub fn book_opf(old: &str, v: &BookValues) -> Option<String> {
    let bom = old.len() - old.trim_start_matches('\u{feff}').len();
    let src = &old[bom..];
    let doc = roxmltree::Document::parse_with_options(
        src,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .ok()?;
    let package = doc.root_element();
    let meta = package
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "metadata")?;
    let raw_meta = &src[meta.range()];
    if raw_meta.ends_with("/>") {
        return None;
    }
    let close = meta.range().start + raw_meta.rfind("</")?;
    // How the details' elements are written here.
    let (dc, declare) = match meta.lookup_prefix(DC) {
        Some("") => (String::new(), String::new()),
        Some(p) => (format!("{p}:"), String::new()),
        None => ("dc:".to_string(), format!(" xmlns:dc=\"{DC}\"")),
    };
    let indent = details_indent(src, meta);
    let opf = Opf {
        src,
        doc: &doc,
        epub3: package
            .attribute("version")
            .is_some_and(|v| v.trim().starts_with('3')),
        uid: package.attribute("unique-identifier"),
        elements: meta.children().filter(|n| n.is_element()).collect(),
        dc,
        declare,
    };
    let mut c = Changes::default();
    opf.title(v, &mut c);
    opf.authors(v, &mut c);
    opf.date(v, &mut c);
    opf.publisher(v, &mut c);
    opf.language(v, &mut c);
    opf.isbn(v, &mut c);
    opf.series(v, &mut c);

    if c.edits.0.is_empty() && c.added.is_empty() {
        return None;
    }
    if !c.added.is_empty() {
        put_added(&mut c.edits, &c.added, src, close, &indent);
    }
    Some(format!("{}{}", &old[..bom], c.edits.apply(src)))
}

/// Whether an EPUB is locked to a reader (DRM): its rights file, or
/// content encrypted otherwise than by the fonts' obfuscation. Readers may
/// refuse such a book once it is changed.
fn drm<R: Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>) -> bool {
    if zip.by_name("META-INF/rights.xml").is_ok() {
        return true;
    }
    let Ok(enc) = read_text(zip, "META-INF/encryption.xml") else {
        return false;
    };
    let Ok(doc) = roxmltree::Document::parse(enc.trim_start_matches('\u{feff}')) else {
        return true;
    };
    const FONTS: &[&str] = &[
        "http://www.idpf.org/2008/embedding",
        "http://ns.adobe.com/pdf/enc#RC",
    ];
    doc.descendants()
        .filter(|n| n.tag_name().name() == "EncryptionMethod")
        .any(|n| !FONTS.contains(&n.attribute("Algorithm").unwrap_or_default()))
}

/// Writes `v` into the EPUB's details. Returns what to put back, or
/// `None` when it already had them.
pub fn write_book(path: &Path, v: &BookValues, store: &Path) -> Result<Option<Backup>, String> {
    if !book_writable(path) {
        return Err("bookUnsupported".into());
    }
    let mut zip = open(path)?;
    if drm(&mut zip) {
        return Err("epubDrm".into());
    }
    let container = read_text(&mut zip, "META-INF/container.xml")?;
    let opf_name = {
        let doc = roxmltree::Document::parse(container.trim_start_matches('\u{feff}'))
            .map_err(|e| e.to_string())?;
        doc.descendants()
            .find(|n| n.has_tag_name("rootfile") && n.attribute("full-path").is_some())
            .and_then(|n| n.attribute("full-path"))
            .ok_or("epubNoOpf")?
            .to_string()
    };
    let old = read_text(&mut zip, &opf_name)?;
    drop(zip);
    let Some(new) = book_opf(&old, v) else {
        return Ok(None);
    };
    write_entry(path, &opf_name, Some(&old), &new, store).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zip_with(path: &Path, entries: &[(&str, &[u8], bool)], comment: &str) {
        let mut z = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, data, stored) in entries {
            let o = zip::write::SimpleFileOptions::default().compression_method(match stored {
                true => zip::CompressionMethod::Stored,
                false => zip::CompressionMethod::Deflated,
            });
            z.start_file(*name, o).unwrap();
            z.write_all(data).unwrap();
        }
        z.set_comment(comment).unwrap();
        z.finish().unwrap();
    }

    fn entry_of(path: &Path, name: &str) -> Option<String> {
        let mut z = open(path).unwrap();
        read_text(&mut z, name).ok()
    }

    fn comic() -> ComicMatch {
        let mut m = ComicMatch {
            series: "Lanterna & Vento".into(),
            number: "3".into(),
            title: "A ponte".into(),
            year: Some(2011),
            count: Some(12),
            publisher: "Casa Inventada".into(),
            writer: "C. S. Moraes".into(),
            ..Default::default()
        };
        crate::fields::add_ymd(&mut m.extra, Some(2011), Some(4), None);
        m.extra.insert("colorist".into(), "Tomé Alvarenga".into());
        m
    }

    #[test]
    fn new_comic_info() {
        let x = comic_xml(
            None,
            &comic_values(&comic(), "https://example.org/3", false),
        )
        .unwrap();
        assert!(x.contains("<Series>Lanterna &amp; Vento</Series>"));
        assert!(x.contains("<Month>4</Month>"));
        assert!(!x.contains("<Day>"));
        // The schema's order.
        let at = |s: &str| x.find(s).unwrap();
        assert!(at("<Title>") < at("<Series>"));
        assert!(at("<Year>") < at("<Writer>"));
        assert!(at("<Publisher>") < at("<Web>"));
        let info = crate::comics::parse_info(&x).unwrap();
        assert_eq!(info.series, "Lanterna & Vento");
        assert_eq!(info.extra.get("colorist").unwrap(), "Tomé Alvarenga");
        assert!(!comic_differs(&comic(), Some(&info)));
    }

    #[test]
    fn comic_info_keeps_what_it_had() {
        let old = "\u{feff}<?xml version=\"1.0\"?>\n<ComicInfo><Pages><Page Image=\"0\" Type=\"FrontCover\"/></Pages>\n<Summary>Texto.</Summary><Series>Velha</Series><Number>3</Number></ComicInfo>";
        let x = comic_xml(Some(old), &comic_values(&comic(), "", false)).unwrap();
        assert!(x.contains("<Summary>Texto.</Summary>"));
        assert!(x.contains("<Page Image=\"0\" Type=\"FrontCover\"/>"));
        assert!(x.contains("<Series>Lanterna &amp; Vento</Series>"));
        assert!(!x.contains("Velha"));
        assert!(x.find("<Summary>").unwrap() < x.find("<Pages>").unwrap());
        // Written again: nothing to do.
        assert_eq!(
            comic_xml(Some(&x), &comic_values(&comic(), "", false)),
            None
        );
    }

    #[test]
    fn writes_and_restores_a_cbz() {
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("store");
        let cbz = d.path().join("a.cbz");
        let page: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        zip_with(
            &cbz,
            &[("01.jpg", &page, true), ("02.jpg", &page, false)],
            "nota",
        );
        let before = fs::read(&cbz).unwrap();
        let b = write_comic(&cbz, &comic_values(&comic(), "", false), &store)
            .unwrap()
            .unwrap();
        let x = entry_of(&cbz, INFO).unwrap();
        assert!(x.contains("<Number>3</Number>"));
        let mut z = open(&cbz).unwrap();
        assert_eq!(z.comment(), b"nota");
        let mut got = Vec::new();
        z.by_name("01.jpg").unwrap().read_to_end(&mut got).unwrap();
        assert_eq!(got, page);
        assert_eq!(
            z.by_name("01.jpg").unwrap().compression(),
            zip::CompressionMethod::Stored
        );
        drop(z);
        assert!(!temp_of(&cbz).exists());
        // Again: already there.
        assert_eq!(
            write_comic(&cbz, &comic_values(&comic(), "", false), &store).unwrap(),
            None
        );
        crate::tagwrite::restore(&b, &store).unwrap();
        assert_eq!(entry_of(&cbz, INFO), None);
        let mut z = open(&cbz).unwrap();
        assert_eq!(names(&z), vec!["01.jpg", "02.jpg"]);
        let mut got = Vec::new();
        z.by_name("02.jpg").unwrap().read_to_end(&mut got).unwrap();
        assert_eq!(got, page);
        assert!(fs::read(&cbz).unwrap().len() <= before.len() + 64);
    }

    #[test]
    fn restores_an_old_comic_info() {
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("store");
        let cbz = d.path().join("a.cbz");
        let old = "<ComicInfo><Series>Velha</Series></ComicInfo>";
        zip_with(
            &cbz,
            &[
                ("01.jpg", b"x", true),
                ("sub/comicinfo.xml", old.as_bytes(), false),
            ],
            "",
        );
        let b = write_comic(&cbz, &comic_values(&comic(), "", false), &store)
            .unwrap()
            .unwrap();
        assert!(
            entry_of(&cbz, "sub/comicinfo.xml")
                .unwrap()
                .contains("Lanterna")
        );
        assert_eq!(entry_of(&cbz, INFO), None);
        crate::tagwrite::restore(&b, &store).unwrap();
        assert_eq!(entry_of(&cbz, "sub/comicinfo.xml").unwrap(), old);
    }

    #[test]
    fn comic_info_cleared_or_removed() {
        let old = "<ComicInfo><Summary>Lixo de outro programa.</Summary><Pages><Page Image=\"0\" Type=\"FrontCover\"/></Pages><Series>Velha</Series></ComicInfo>";
        let x = comic_xml_with(Some(old), &comic_values(&comic(), "", false), true).unwrap();
        assert!(!x.contains("Lixo") && !x.contains("Velha"));
        assert!(x.contains("FrontCover") && x.contains("Lanterna"));
        // Nothing but the pages left, already: nothing to do.
        assert_eq!(
            comic_xml_with(Some(&x), &comic_values(&comic(), "", false), true),
            None
        );

        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("store");
        let cbz = d.path().join("a.cbz");
        zip_with(
            &cbz,
            &[
                ("01.jpg", b"x", true),
                ("ComicInfo.xml", old.as_bytes(), false),
            ],
            "",
        );
        let b = remove_comic(&cbz, &store).unwrap().unwrap();
        assert_eq!(entry_of(&cbz, INFO), None);
        assert_eq!(names(&open(&cbz).unwrap()), vec!["01.jpg"]);
        assert_eq!(remove_comic(&cbz, &store).unwrap(), None);
        crate::tagwrite::restore(&b, &store).unwrap();
        assert_eq!(entry_of(&cbz, INFO).unwrap(), old);
    }

    #[test]
    fn not_a_zip() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.cbz");
        fs::write(&f, b"Rar!\x1a\x07\x00 nothing").unwrap();
        assert!(!comic_writable(&f));
        assert!(write_comic(&f, &[], d.path()).is_err());
    }

    const OPF2: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="uid" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>titulo errado</dc:title>
    <dc:creator opf:role="aut" opf:file-as="Desconhecido">Desconhecido</dc:creator>
    <dc:creator opf:role="ill">Graciano Rocha Filho</dc:creator>
    <dc:identifier id="uid">urn:uuid:1234</dc:identifier>
    <dc:language>pt-BR</dc:language>
    <dc:description>Uma descrição.</dc:description>
  </metadata>
  <manifest/>
</package>
"#;

    fn book() -> BookValues {
        BookValues {
            title: "O Relógio de Areia".into(),
            authors: vec!["C. S. Moraes".into(), "Tomé de Alvarenga".into()],
            year: "1987".into(),
            publisher: "Casa Inventada".into(),
            language: "pt".into(),
            isbn: "9780000000002".into(),
            series: "Crônicas do Vale".into(),
            series_index: "2".into(),
        }
    }

    #[test]
    fn epub2_details() {
        let x = book_opf(OPF2, &book()).unwrap();
        assert!(x.contains("<dc:title>O Relógio de Areia</dc:title>"));
        assert!(x.contains("<dc:creator opf:role=\"aut\">C. S. Moraes</dc:creator>"));
        assert!(x.contains("<dc:creator>Tomé de Alvarenga</dc:creator>"));
        // Other people and details stay; the language already said so.
        assert!(x.contains("opf:role=\"ill\">Graciano Rocha Filho"));
        assert!(x.contains("<dc:description>Uma descrição.</dc:description>"));
        assert!(x.contains("<dc:language>pt-BR</dc:language>"));
        assert!(x.contains("urn:uuid:1234"));
        assert!(x.contains("<dc:identifier>urn:isbn:9780000000002</dc:identifier>"));
        assert!(x.contains("<meta name=\"calibre:series\" content=\"Crônicas do Vale\"/>"));
        assert!(!x.contains("belongs-to-collection"));
        assert!(!x.contains("Desconhecido"));
        assert!(x.contains("\n  </metadata>"));
        let doc = roxmltree::Document::parse(&x).unwrap();
        let m = crate::ebook::opf_meta(&doc);
        assert_eq!(m.title, "O Relógio de Areia");
        assert_eq!(m.authors, vec!["C. S. Moraes", "Tomé de Alvarenga"]);
        assert_eq!(m.year, Some(1987));
        assert_eq!(m.series, "Crônicas do Vale");
        assert_eq!(m.series_index, "2");
        assert_eq!(m.isbns, vec!["9780000000002"]);
        assert_eq!(book_opf(&x, &book()), None);
    }

    const OPF3: &str = r##"<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:identifier id="id">9780000000002</dc:identifier>
<dc:title id="t">Outro</dc:title>
<meta refines="#t" property="title-type">main</meta>
<dc:creator id="c1">Alguém</dc:creator>
<meta refines="#c1" property="role" scheme="marc:relators">aut</meta>
<meta refines="#c1" property="file-as">Alguém</meta>
<dc:creator id="c2">Outra Pessoa</dc:creator>
<meta refines="#c2" property="file-as">Pessoa, Outra</meta>
<meta property="dcterms:modified">2020-01-01T00:00:00Z</meta>
</metadata>
</package>"##;

    #[test]
    fn epub_with_drm_is_left_alone() {
        let d = tempfile::tempdir().unwrap();
        let epub = d.path().join("a.epub");
        let enc = br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#aes128-cbc"/></enc:EncryptedData></encryption>"#;
        zip_with(
            &epub,
            &[
                ("mimetype", b"application/epub+zip", true),
                ("META-INF/encryption.xml", enc, false),
            ],
            "",
        );
        assert_eq!(write_book(&epub, &book(), d.path()), Err("epubDrm".into()));
        let fonts = br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"/></enc:EncryptedData></encryption>"#;
        let mut z = open_fonts(&epub, fonts);
        assert!(!drm(&mut z));
    }

    fn open_fonts(epub: &Path, enc: &[u8]) -> zip::ZipArchive<BufReader<File>> {
        zip_with(
            epub,
            &[
                ("mimetype", b"application/epub+zip", true),
                ("META-INF/encryption.xml", enc, false),
            ],
            "",
        );
        open(epub).unwrap()
    }

    #[test]
    fn empty_elements_get_text() {
        let opf = "<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"2.0\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title/><dc:publisher></dc:publisher></metadata></package>";
        let v = BookValues {
            title: "O Relógio".into(),
            publisher: "Casa".into(),
            ..Default::default()
        };
        let x = book_opf(opf, &v).unwrap();
        assert!(x.contains("<dc:title>O Relógio</dc:title>"), "{x}");
        assert!(x.contains("<dc:publisher>Casa</dc:publisher>"), "{x}");
    }

    #[test]
    fn epub3_details() {
        let mut v = book();
        v.authors.truncate(1);
        let x = book_opf(OPF3, &v).unwrap();
        assert!(x.contains("<dc:title id=\"t\">O Relógio de Areia</dc:title>"));
        assert!(x.contains("<meta refines=\"#t\" property=\"title-type\">main</meta>"));
        assert!(x.contains("<dc:creator id=\"c1\">C. S. Moraes</dc:creator>"));
        assert!(x.contains("property=\"role\" scheme=\"marc:relators\">aut"));
        assert!(!x.contains("Alguém"));
        assert!(!x.contains("Outra Pessoa"));
        assert!(!x.contains("Pessoa, Outra"));
        // The ISBN was there (it is the book's identifier).
        assert!(!x.contains("urn:isbn"));
        assert!(
            x.contains("property=\"belongs-to-collection\" id=\"renome-series\">Crônicas do Vale")
        );
        assert!(
            x.contains("<meta refines=\"#renome-series\" property=\"group-position\">2</meta>")
        );
        let doc = roxmltree::Document::parse(&x).unwrap();
        let m = crate::ebook::opf_meta(&doc);
        assert_eq!(m.authors, vec!["C. S. Moraes"]);
        assert_eq!(m.series, "Crônicas do Vale");
        assert_eq!(book_opf(&x, &v), None);
    }

    #[test]
    fn writes_and_restores_an_epub() {
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("store");
        let epub = d.path().join("a.epub");
        let container = br#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
        zip_with(
            &epub,
            &[
                ("mimetype", b"application/epub+zip", true),
                ("META-INF/container.xml", container, false),
                ("OEBPS/content.opf", OPF2.as_bytes(), false),
                ("OEBPS/text.xhtml", b"<html/>", false),
            ],
            "",
        );
        let b = write_book(&epub, &book(), &store).unwrap().unwrap();
        let z = open(&epub).unwrap();
        // The mimetype stays first and stored, as readers want it.
        assert_eq!(names(&z)[0], "mimetype");
        assert_eq!(names(&z)[2], "OEBPS/content.opf");
        drop(z);
        let mut z = open(&epub).unwrap();
        assert_eq!(
            z.by_index(0).unwrap().compression(),
            zip::CompressionMethod::Stored
        );
        drop(z);
        assert_eq!(crate::ebook::read(&epub).title, "O Relógio de Areia");
        assert_eq!(write_book(&epub, &book(), &store).unwrap(), None);
        crate::tagwrite::restore(&b, &store).unwrap();
        assert_eq!(entry_of(&epub, "OEBPS/content.opf").unwrap(), OPF2);
    }

    /// The OPFs the golden test edits: each way details are written.
    fn golden_opfs() -> Vec<(&'static str, String)> {
        let head2 = "<?xml version=\"1.0\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"2.0\" unique-identifier=\"id\">\n";
        let head3 = "<?xml version=\"1.0\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"id\">\n";
        let tail = "<manifest><item id=\"c\" href=\"c.xhtml\" media-type=\"application/xhtml+xml\"/></manifest>\n</package>\n";
        let two = format!(
            "{head2}  <metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:opf=\"http://www.idpf.org/2007/opf\">\n    <dc:title>Old Title</dc:title>\n    <dc:creator opf:role=\"aut\" opf:file-as=\"One, Old\">Old One</dc:creator>\n    <dc:creator opf:role=\"aut\">Old Two</dc:creator>\n    <dc:creator opf:role=\"ill\">Drawer</dc:creator>\n    <dc:date>1999-05-01</dc:date>\n    <dc:publisher>Old Press</dc:publisher>\n    <dc:language>pt-BR</dc:language>\n    <dc:identifier id=\"id\">uuid:1234</dc:identifier>\n    <dc:identifier opf:scheme=\"ISBN\">9780306406157</dc:identifier>\n    <meta name=\"calibre:series\" content=\"Old Series\"/>\n    <meta name=\"calibre:series_index\" content=\"1\"/>\n  </metadata>\n{tail}"
        );
        let three = format!(
            "{head3}  <metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n    <dc:identifier id=\"id\">urn:uuid:1234</dc:identifier>\n    <dc:title id=\"t1\">Old Title</dc:title>\n    <meta refines=\"#t1\" property=\"title-type\">main</meta>\n    <dc:title id=\"t2\">A Subtitle</dc:title>\n    <meta refines=\"#t2\" property=\"title-type\">subtitle</meta>\n    <dc:creator id=\"a1\">Old One</dc:creator>\n    <meta refines=\"#a1\" property=\"role\" scheme=\"marc:relators\">aut</meta>\n    <meta refines=\"#a1\" property=\"file-as\">One, Old</meta>\n    <dc:date>2001</dc:date>\n    <dc:language>en</dc:language>\n    <dc:identifier>urn:isbn:9781402894626</dc:identifier>\n    <meta property=\"belongs-to-collection\" id=\"c1\">Old Series</meta>\n    <meta refines=\"#c1\" property=\"collection-type\">series</meta>\n    <meta refines=\"#c1\" property=\"group-position\">3</meta>\n    <meta property=\"dcterms:modified\">2020-01-01T00:00:00Z</meta>\n  </metadata>\n{tail}"
        );
        let bare = format!(
            "{head3}<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>Only</dc:title></metadata>\n{tail}"
        );
        let bom = format!(
            "\u{feff}{head2}\t<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n\t\t<dc:title>Bom</dc:title>\n\t\t<dc:creator>Someone</dc:creator>\n\t</metadata>\n{tail}"
        );
        let empty = format!("{head2}<metadata/>\n{tail}");
        vec![
            ("two", two),
            ("three", three),
            ("bare", bare),
            ("bom", bom),
            ("empty", empty),
        ]
    }

    fn golden_values() -> Vec<(&'static str, BookValues)> {
        let s = |x: &str| x.to_string();
        vec![
            (
                "full",
                BookValues {
                    title: s("New Title"),
                    authors: vec![s("New One"), s("New Two"), s("New Three")],
                    year: s("2015"),
                    publisher: s("New Press"),
                    language: s("en"),
                    isbn: s("9780306406157"),
                    series: s("New Series"),
                    series_index: s("7"),
                },
            ),
            (
                "fewer",
                BookValues {
                    title: s("Old Title"),
                    authors: vec![s("Only Author")],
                    isbn: s("9781402894626"),
                    series: s("Old Series"),
                    series_index: s("9"),
                    ..Default::default()
                },
            ),
            (
                "same",
                BookValues {
                    title: s("Old Title"),
                    authors: vec![s("Old One")],
                    year: s("2001"),
                    language: s("en"),
                    ..Default::default()
                },
            ),
            ("none", BookValues::default()),
        ]
    }

    /// What `book_opf` writes for each OPF and each set of values. A change
    /// here is a change in the EPUBs people's books are saved as: update
    /// these only when that is the point, never for a refactor.
    #[test]
    fn book_opf_does_not_change() {
        let mut got: Vec<(String, u64)> = Vec::new();
        for (on, opf) in golden_opfs() {
            for (vn, v) in golden_values() {
                let out = format!("{:?}", book_opf(&opf, &v));
                if std::env::var_os("GOLDEN_SHOW").is_some() {
                    eprintln!("#### {on}/{vn}\n{out}");
                }
                got.push((
                    format!("{on}/{vn}"),
                    xxhash_rust::xxh3::xxh3_64(out.as_bytes()),
                ));
            }
        }
        let want: Vec<(String, u64)> = vec![
            ("two/full".to_string(), 5746460103816541910),
            ("two/fewer".to_string(), 3645695271312550084),
            ("two/same".to_string(), 7480915198357479523),
            ("two/none".to_string(), 8169143652509480441),
            ("three/full".to_string(), 5074791247990556880),
            ("three/fewer".to_string(), 15986331220408464918),
            ("three/same".to_string(), 8169143652509480441),
            ("three/none".to_string(), 8169143652509480441),
            ("bare/full".to_string(), 10417967633236188159),
            ("bare/fewer".to_string(), 15146349055602855066),
            ("bare/same".to_string(), 12419388878216890658),
            ("bare/none".to_string(), 8169143652509480441),
            ("bom/full".to_string(), 14210168727809629928),
            ("bom/fewer".to_string(), 8404833533463941923),
            ("bom/same".to_string(), 7086486855153481615),
            ("bom/none".to_string(), 8169143652509480441),
            ("empty/full".to_string(), 8169143652509480441),
            ("empty/fewer".to_string(), 8169143652509480441),
            ("empty/same".to_string(), 8169143652509480441),
            ("empty/none".to_string(), 8169143652509480441),
        ];
        assert_eq!(got, want);
    }
}
