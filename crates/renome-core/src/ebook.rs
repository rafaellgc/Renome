//! What book files say about themselves: title, authors, ISBN and the like
//! from EPUB, MOBI/AZW3, PDF and FB2 files, and ISBNs found in their text
//! (the copyright page) when their details lack one.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Extensions of books (the ones whose details can be read, and DjVu,
/// which is only named).
const BOOKS: &[&str] = &[
    "epub", "kepub", "mobi", "azw", "azw3", "prc", "pdf", "fb2", "djvu", "djv",
];

pub fn is_book(ext: &str) -> bool {
    BOOKS.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

/// A book's details as its file has them; empty fields are unknown.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BookMeta {
    pub title: String,
    pub authors: Vec<String>,
    /// ISBN-13s, those in the details first, then those in the text.
    pub isbns: Vec<String>,
    /// Year of the edition (or of the file).
    pub year: Option<u16>,
    pub publisher: String,
    pub language: String,
    pub series: String,
    /// The book's number in its series (`1`, `2.5`).
    pub series_index: String,
}

/// Reads a book's details; unknown formats and unreadable files give
/// empty ones. A broken file never takes the program down.
pub fn read(path: &Path) -> BookMeta {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let path = path.to_path_buf();
    let m = std::panic::catch_unwind(move || match ext.as_str() {
        "epub" | "kepub" => epub(&path),
        "mobi" | "azw" | "azw3" | "prc" => mobi(&path),
        "pdf" => pdf(&path),
        "fb2" => fb2(&path),
        _ => None,
    });
    let mut m = m.ok().flatten().unwrap_or_default();
    m.title = clean_title(&m.title);
    // Some books put everyone in one field (`Author;Other;tradução de X`):
    // a name never has a `;`, and translators and the like are not authors.
    static ROLE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(tradu[cç][aã]o|traducci[oó]n|translat(ed|ion)|trad\.|ilustra[cç][oõ]es|ilustraciones|illustrat(ed|ions)|organiza[cç][aã]o|prefácio|pref[aá]cio|foreword|introdu[cç][aã]o)\b").unwrap()
    });
    m.authors = m
        .authors
        .iter()
        .flat_map(|a| a.split(';'))
        .map(|a| crate::text::collapse_spaces(a).trim().to_string())
        .filter(|a| !a.is_empty() && !is_junk_author(a) && !ROLE.is_match(a))
        .fold(Vec::new(), |mut v, a| {
            if !v.contains(&a) {
                v.push(a);
            }
            v
        });
    m.series = m.series.trim().to_string();
    m.series_index = series_index(&m.series_index);
    m
}

/// `1.0` → `1`; anything else as it is.
fn series_index(s: &str) -> String {
    let s = s.trim();
    match s.strip_suffix(".0").or_else(|| s.strip_suffix(".00")) {
        Some(n) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => n.to_string(),
        _ => s.to_string(),
    }
}

/// Titles that programs put in by themselves (`Microsoft Word - x.doc`,
/// `Untitled`) are no title at all.
fn clean_title(t: &str) -> String {
    static JUNK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?ix)^(
                untitled.*|sem\s+t[ií]tulo.*|sin\s+t[ií]tulo.*|document\d*|documento\d*|
                unknown|desconhecido|title|t[ií]tulo|book|livro|libro|ebook|e-book|
                microsoft\s+(word|powerpoint).*|apresenta[çc][ãa]o\s+do\s+powerpoint|
                .*\.(docx?|odt|rtf|pdf|indd|qxd|qxp|tex|dvi|ps|txt|html?|p65|pm\d|fm|wpd)|
                [^\p{L}\p{N}]*
            )$",
        )
        .unwrap()
    });
    let t = crate::text::collapse_spaces(t.trim());
    if JUNK.is_match(&t) || (t.contains(['\\', '/']) && t.contains('.')) {
        String::new()
    } else {
        t
    }
}

fn is_junk_author(a: &str) -> bool {
    static JUNK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)^(unknown|desconhecido|desconocido|an[oó]nimo|anonymous|user|usu[aá]rio|usuario|admin|administra(dor|tor)|owner|propriet[aá]rio|hp|dell|lenovo|asus|acer|pc|windows user|calibre.*|[^\p{L}]*)$",
        )
        .unwrap()
    });
    JUNK.is_match(a.trim())
}

// ---------------------------------------------------------------- ISBN

fn check10(d: &[u8]) -> bool {
    let sum: u32 = d
        .iter()
        .enumerate()
        .map(|(i, &c)| (10 - i as u32) * if c == b'X' { 10 } else { (c - b'0') as u32 })
        .sum();
    sum.is_multiple_of(11)
}

fn check13(d: &[u8]) -> bool {
    let sum: u32 = d
        .iter()
        .enumerate()
        .map(|(i, &c)| (c - b'0') as u32 * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    sum.is_multiple_of(10)
}

/// A valid ISBN (10 or 13 digits, with or without dashes) as ISBN-13.
pub fn isbn13(raw: &str) -> Option<String> {
    let d: Vec<u8> = raw
        .bytes()
        .filter(|c| c.is_ascii_digit() || *c == b'X' || *c == b'x')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    match d.len() {
        13 if (d.starts_with(b"978") || d.starts_with(b"979"))
            && !d.contains(&b'X')
            && check13(&d) =>
        {
            Some(String::from_utf8(d).ok()?)
        }
        10 if !d[..9].contains(&b'X') && check10(&d) => {
            let mut out = b"978".to_vec();
            out.extend_from_slice(&d[..9]);
            let sum: u32 = out
                .iter()
                .enumerate()
                .map(|(i, &c)| (c - b'0') as u32 * if i % 2 == 0 { 1 } else { 3 })
                .sum();
            out.push(b'0' + ((10 - sum % 10) % 10) as u8);
            Some(String::from_utf8(out).ok()?)
        }
        _ => None,
    }
}

/// A digit of a printed ISBN and what may follow it: spaces and the many
/// dashes (two of them where markup was taken out).
const DIGIT: &str = r"[0-9][\s\x{a0}\-\x{2010}-\x{2015}.]{0,2}";

/// The ISBNs in a text: those after the word ISBN first, then bare
/// 13-digit ones starting with 978 or 979. Each one once.
pub fn find_isbns(text: &str) -> Vec<String> {
    static LABELLED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(
            r"(?i)ISBN(?:-?1[03])?[^0-9\n]{{0,25}}?((?:{DIGIT}){{9}}[0-9Xx](?:[\s\x{{a0}}\-\x{{2010}}-\x{{2015}}.]{{0,2}}[0-9]){{0,3}})"
        ))
        .unwrap()
    });
    static BARE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(
            r"\b(97[89][\s\x{{a0}}\-\x{{2010}}-\x{{2015}}.]{{0,2}}(?:{DIGIT}){{9}}[0-9])\b"
        ))
        .unwrap()
    });
    let mut out: Vec<String> = Vec::new();
    let mut add = |s: &str| {
        // A match may run into the digits that follow the ISBN.
        let digits: String = s.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let found = isbn13(&digits)
            .or_else(|| digits.get(..13).and_then(isbn13))
            .or_else(|| digits.get(..10).and_then(isbn13));
        if let Some(i) = found
            && !out.contains(&i)
        {
            out.push(i);
        }
    };
    for c in LABELLED.captures_iter(text) {
        add(&c[1]);
    }
    for c in BARE.captures_iter(text) {
        add(&c[1]);
    }
    out
}

// ---------------------------------------------------------------- EPUB

/// Reads at most `limit` bytes of a zip entry.
fn entry<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, name: &str, limit: u64) -> Option<String> {
    let f = zip.by_name(name).ok()?;
    let mut bytes = Vec::new();
    f.take(limit).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn xml(text: &str) -> Option<roxmltree::Document<'_>> {
    roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .ok()
}

/// `%20` and friends in a manifest link.
fn unpercent(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(h) = s.get(i + 1..i + 3)
            && let Ok(v) = u8::from_str_radix(h, 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A path inside the book, relative to the folder of `base`.
fn join_zip(base: &str, href: &str) -> String {
    let href = unpercent(href.split('#').next().unwrap_or_default());
    let mut parts: Vec<&str> = base
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("")
        .split('/')
        .filter(|p| !p.is_empty())
        .collect();
    for p in href.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/")
}

/// The text of a page, without its markup.
fn strip_tags(html: &str) -> String {
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());
    TAG.replace_all(html, " ")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&#8209;", "-")
        .replace("&#8211;", "-")
        .replace("&ndash;", "-")
        .replace("&amp;", "&")
}

fn epub(path: &Path) -> Option<BookMeta> {
    let mut zip = zip::ZipArchive::new(BufReader::new(File::open(path).ok()?)).ok()?;
    let container = entry(&mut zip, "META-INF/container.xml", 1 << 20)?;
    let opf_path = {
        let doc = xml(&container)?;
        doc.descendants()
            .find(|n| n.has_tag_name("rootfile") && n.attribute("full-path").is_some())?
            .attribute("full-path")?
            .to_string()
    };
    let opf = entry(&mut zip, &opf_path, 8 << 20)?;
    let doc = xml(&opf)?;
    let mut m = opf_meta(&doc);
    if m.isbns.is_empty() {
        // The copyright page is near the start, sometimes at the end.
        let manifest: std::collections::HashMap<&str, &str> = doc
            .descendants()
            .filter(|n| n.has_tag_name("item"))
            .filter_map(|n| Some((n.attribute("id")?, n.attribute("href")?)))
            .collect();
        let spine: Vec<String> = doc
            .descendants()
            .filter(|n| n.has_tag_name("itemref"))
            .filter_map(|n| manifest.get(n.attribute("idref")?))
            .map(|href| join_zip(&opf_path, href))
            .collect();
        let n = spine.len();
        let pages = spine
            .iter()
            .take(8)
            .chain(spine.iter().skip(n.saturating_sub(4).max(8)));
        for page in pages {
            if let Some(html) = entry(&mut zip, page, 2 << 20) {
                m.isbns = find_isbns(&strip_tags(&html));
                if !m.isbns.is_empty() {
                    break;
                }
            }
        }
    }
    Some(m)
}

/// The details in a package document (`content.opf`), EPUB 2 or 3.
pub(crate) fn opf_meta(doc: &roxmltree::Document) -> BookMeta {
    let mut m = BookMeta::default();
    let Some(meta) = doc.descendants().find(|n| n.has_tag_name("metadata")) else {
        return m;
    };
    let text =
        |n: roxmltree::Node| crate::text::collapse_spaces(n.text().unwrap_or_default().trim());
    // EPUB 3 details about other elements: `<meta refines="#id" property="role">`.
    let refines = |id: Option<&str>, property: &str| -> Option<String> {
        let id = id?;
        meta.descendants()
            .find(|n| {
                n.has_tag_name("meta")
                    && n.attribute("refines").map(|r| r.trim_start_matches('#')) == Some(id)
                    && n.attribute("property") == Some(property)
            })
            .map(text)
    };
    let attr = |n: roxmltree::Node, name: &str| {
        n.attributes()
            .find(|a| a.name() == name)
            .map(|a| a.value().to_string())
    };
    let mut titles: Vec<(String, bool)> = Vec::new();
    let mut collection: Option<(String, Option<String>)> = None;
    for n in meta.descendants().filter(|n| n.is_element()) {
        match n.tag_name().name() {
            "title" => {
                let main = refines(n.attribute("id"), "title-type").is_none_or(|t| t == "main");
                titles.push((text(n), main));
            }
            "creator" => {
                let role = attr(n, "role").or_else(|| refines(n.attribute("id"), "role"));
                if role.as_deref().is_none_or(|r| r == "aut") {
                    m.authors.push(text(n));
                }
            }
            "identifier" => {
                let value = text(n);
                let scheme = attr(n, "scheme").unwrap_or_default().to_lowercase();
                let lower = value.to_lowercase();
                let says = scheme.contains("isbn") || lower.contains("isbn");
                let digits = value.chars().filter(|c| c.is_ascii_digit()).count();
                if (says || digits == 13)
                    && let Some(i) = isbn13(lower.trim_start_matches("urn:isbn:"))
                    && !m.isbns.contains(&i)
                {
                    m.isbns.push(i);
                }
            }
            "date" if m.year.is_none() => {
                let event = attr(n, "event").unwrap_or_default();
                if !event.contains("modif") {
                    m.year = text(n)
                        .get(..4)
                        .and_then(|y| y.parse().ok())
                        .filter(|y| *y > 1000);
                }
            }
            "publisher" if m.publisher.is_empty() => m.publisher = text(n),
            "language" if m.language.is_empty() => m.language = text(n),
            "meta" => {
                let content = n
                    .attribute("content")
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                match (n.attribute("name"), n.attribute("property")) {
                    (Some("calibre:series"), _) => m.series = content,
                    (Some("calibre:series_index"), _) => m.series_index = content,
                    (_, Some("belongs-to-collection")) if collection.is_none() => {
                        let index = refines(n.attribute("id"), "group-position");
                        collection = Some((text(n), index));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    if m.series.is_empty()
        && let Some((name, index)) = collection
    {
        m.series = name;
        m.series_index = index.unwrap_or_default();
    }
    m.title = titles
        .iter()
        .find(|(_, main)| *main)
        .or(titles.first())
        .map(|(t, _)| t.clone())
        .unwrap_or_default();
    m
}

// ---------------------------------------------------------------- MOBI

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// MOBI and AZW3 (Kindle): the full title and the EXTH records of the
/// first record's header.
fn mobi(path: &Path) -> Option<BookMeta> {
    let mut f = File::open(path).ok()?;
    let mut head = [0u8; 86];
    f.read_exact(&mut head).ok()?;
    if &head[60..68] != b"BOOKMOBI" && &head[60..68] != b"TEXtREAd" {
        return None;
    }
    let first = be32(&head, 78)? as u64;
    f.seek(SeekFrom::Start(first)).ok()?;
    let mut rec = Vec::new();
    f.take(1 << 20).read_to_end(&mut rec).ok()?;
    parse_mobi(&rec)
}

/// The first record of a MOBI file.
fn parse_mobi(rec: &[u8]) -> Option<BookMeta> {
    if rec.get(16..20)? != b"MOBI" {
        return None;
    }
    let header_len = be32(rec, 20)? as usize;
    let utf8 = be32(rec, 28)? == 65001;
    let decode = |b: &[u8]| -> String {
        if utf8 {
            String::from_utf8_lossy(b).into_owned()
        } else {
            encoding_rs::WINDOWS_1252.decode(b).0.into_owned()
        }
    };
    let mut m = BookMeta::default();
    let (name_at, name_len) = (be32(rec, 84)? as usize, be32(rec, 88)? as usize);
    if let Some(name) = rec.get(name_at..name_at + name_len) {
        m.title = decode(name);
    }
    let has_exth = be32(rec, 128).is_some_and(|f| f & 0x40 != 0);
    let exth = 16 + header_len;
    if has_exth && rec.get(exth..exth + 4) == Some(b"EXTH") {
        let count = be32(rec, exth + 8)?;
        let mut at = exth + 12;
        for _ in 0..count {
            let (kind, len) = (be32(rec, at)?, be32(rec, at + 4)? as usize);
            if len < 8 {
                break;
            }
            let data = rec.get(at + 8..at + len)?;
            let value = decode(data).trim().to_string();
            match kind {
                100 => m
                    .authors
                    .extend(value.split(['&', ';']).map(|a| a.trim().to_string())),
                101 => m.publisher = value,
                104 => {
                    if let Some(i) = isbn13(&value) {
                        m.isbns.push(i);
                    }
                }
                106 => m.year = value.get(..4).and_then(|y| y.parse().ok()),
                503 if !value.is_empty() => m.title = value,
                524 => m.language = value,
                _ => {}
            }
            at += len;
        }
    }
    Some(m)
}

// ---------------------------------------------------------------- PDF

/// Big PDFs are scans: their text, if any, is not worth reading whole.
const PDF_TEXT_LIMIT: u64 = 150 << 20;

/// Images are dropped while loading: only the text is wanted.
fn no_images(id: (u32, u16), obj: &mut lopdf::Object) -> Option<((u32, u16), lopdf::Object)> {
    if let lopdf::Object::Stream(s) = obj
        && s.dict
            .get(b"Subtype")
            .and_then(|v| v.as_name())
            .is_ok_and(|n| n == b"Image")
    {
        return None;
    }
    // lopdf keeps `obj` for some objects and the returned one for others.
    Some((id, obj.clone()))
}

fn pdf(path: &Path) -> Option<BookMeta> {
    let mut m = BookMeta::default();
    if let Ok(info) = lopdf::Document::load_metadata(path) {
        m.title = info.title.unwrap_or_default();
        if let Some(a) = info.author {
            m.authors.extend(split_authors(&a));
        }
        for s in [info.subject, info.keywords].into_iter().flatten() {
            m.isbns.extend(find_isbns(&s));
        }
    }
    let size = std::fs::metadata(path).ok()?.len();
    if size > PDF_TEXT_LIMIT {
        return Some(m);
    }
    let options = lopdf::LoadOptions {
        filter: Some(no_images),
        max_decompressed_size: Some(64 << 20),
        ..Default::default()
    };
    let Ok(doc) = lopdf::Document::load_with_options(path, options) else {
        return Some(m);
    };
    if let Some(x) = xmp(&doc) {
        let found = xmp_meta(&x);
        if clean_title(&m.title).is_empty() {
            m.title = found.title;
        }
        if m.authors.is_empty() {
            m.authors = found.authors;
        }
        m.isbns.extend(found.isbns);
    }
    if m.isbns.is_empty() {
        let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
        let n = pages.len();
        let wanted = pages
            .iter()
            .take(8)
            .chain(pages.iter().skip(n.saturating_sub(3).max(8)));
        for &p in wanted {
            if let Ok(text) = doc.extract_text_with_limit(&[p], 8 << 20) {
                m.isbns = find_isbns(&text);
                if !m.isbns.is_empty() {
                    break;
                }
            }
        }
    }
    let mut seen = Vec::new();
    m.isbns.retain(|i| {
        let new = !seen.contains(i);
        seen.push(i.clone());
        new
    });
    Some(m)
}

/// The XMP packet of a PDF (`/Metadata` in its catalog).
fn xmp(doc: &lopdf::Document) -> Option<String> {
    let r = doc
        .catalog()
        .ok()?
        .get(b"Metadata")
        .ok()?
        .as_reference()
        .ok()?;
    let s = doc.get_object(r).ok()?.as_stream().ok()?;
    let bytes = s
        .decompressed_content_with_limit(4 << 20)
        .unwrap_or_else(|_| s.content.clone());
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn xmp_meta(x: &str) -> BookMeta {
    static TITLE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?s)<dc:title>.*?<rdf:li[^>]*>(.*?)</rdf:li>").unwrap());
    static CREATOR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?s)<dc:creator>(.*?)</dc:creator>").unwrap());
    static LI: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?s)<rdf:li[^>]*>(.*?)</rdf:li>").unwrap());
    static ISBN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?is)<(?:prism:isbn|prism:eIsbn|pdfx:isbn|xmp:Identifier|dc:identifier)[^>]*>(.*?)</",
        )
        .unwrap()
    });
    let unxml = |s: &str| {
        s.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .trim()
            .to_string()
    };
    let mut m = BookMeta::default();
    if let Some(c) = TITLE.captures(x) {
        m.title = unxml(&c[1]);
    }
    if let Some(c) = CREATOR.captures(x) {
        m.authors = LI.captures_iter(&c[1]).map(|l| unxml(&l[1])).collect();
    }
    for c in ISBN.captures_iter(x) {
        m.isbns
            .extend(find_isbns(&format!("ISBN {}", strip_tags(&c[1]))));
    }
    m
}

/// `A; B`, `A & B`, `A and B` → each one.
fn split_authors(s: &str) -> Vec<String> {
    static SPLIT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\s*(?:;|&|\band\b|\s+e\s+|\s+y\s+)\s*").unwrap());
    SPLIT
        .split(s)
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect()
}

// ---------------------------------------------------------------- FB2

fn fb2(path: &Path) -> Option<BookMeta> {
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(64 << 20)
        .read_to_end(&mut bytes)
        .ok()?;
    parse_fb2(&bytes)
}

/// FictionBook 2: the `<description>` part, in the encoding its first
/// line declares (often windows-1251).
fn parse_fb2(bytes: &[u8]) -> Option<BookMeta> {
    static ENCODING: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"encoding=["']([A-Za-z0-9_-]+)["']"#).unwrap());
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]).into_owned();
    let enc = ENCODING
        .captures(&head)
        .and_then(|c| encoding_rs::Encoding::for_label(c[1].as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    let text = enc.decode(bytes).0;
    // Only the description matters; the rest may be megabytes of text and images.
    let end = text.find("</description>")? + "</description>".len();
    let start = text.find("<description")?;
    // Cut from its root, it needs the prefixes declared again.
    let part = format!(
        "<d xmlns:l=\"http://www.w3.org/1999/xlink\" xmlns:xlink=\"http://www.w3.org/1999/xlink\">{}</d>",
        &text[start..end]
    );
    let doc = xml(&part)?;
    let mut m = BookMeta::default();
    let child = |n: roxmltree::Node, name: &str| -> String {
        n.children()
            .find(|c| c.has_tag_name(name))
            .and_then(|c| c.text())
            .map(|t| crate::text::collapse_spaces(t.trim()))
            .unwrap_or_default()
    };
    if let Some(info) = doc.descendants().find(|n| n.has_tag_name("title-info")) {
        m.title = child(info, "book-title");
        for a in info.children().filter(|c| c.has_tag_name("author")) {
            let name = ["first-name", "middle-name", "last-name"]
                .map(|p| child(a, p))
                .join(" ");
            let name = crate::text::collapse_spaces(name.trim());
            m.authors.push(if name.is_empty() {
                child(a, "nickname")
            } else {
                name
            });
        }
        m.language = child(info, "lang");
        if let Some(s) = info.children().find(|c| c.has_tag_name("sequence")) {
            m.series = s.attribute("name").unwrap_or_default().to_string();
            m.series_index = s.attribute("number").unwrap_or_default().to_string();
        }
    }
    if let Some(publish) = doc.descendants().find(|n| n.has_tag_name("publish-info")) {
        m.publisher = child(publish, "publisher");
        m.year = child(publish, "year").get(..4).and_then(|y| y.parse().ok());
        m.isbns
            .extend(find_isbns(&format!("ISBN {}", child(publish, "isbn"))));
    }
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isbns() {
        assert_eq!(
            isbn13("978-0-00-000001-9").as_deref(),
            Some("9780000000019")
        );
        assert_eq!(isbn13("0-00-000001-9").as_deref(), Some("9780000000019"));
        assert_eq!(isbn13("0-00-000006-X").as_deref(), Some("9780000000064"));
        assert_eq!(isbn13("978-0-00-000001-8"), None);
        assert_eq!(isbn13("1234567890123"), None);
        let text = "Dados Internacionais\nISBN 978‑0‑00‑000001‑9 (impresso)\ne-ISBN: 978-0-00-000009-4\nOutro 9780000000026.";
        let found = find_isbns(text);
        assert_eq!(found[0], "9780000000019");
        assert!(found.contains(&"9780000000026".to_string()));
        assert_eq!(find_isbns("ISBN-10: 0000000027"), vec!["9780000000026"]);
        assert!(find_isbns("telefone 978 1234 5678 9").is_empty());
    }

    #[test]
    fn one_field_authors() {
        let opf = r#"<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>X</dc:title><dc:creator>Ana Lima;Rui Prado Jr.;tradução de Eva Souza</dc:creator>
</metadata></package>"#;
        let dir = std::env::temp_dir().join(format!("renome-ebook-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.epub");
        let mut z = zip::ZipWriter::new(File::create(&path).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("META-INF/container.xml", o).unwrap();
        std::io::Write::write_all(
            &mut z,
            br#"<container><rootfiles><rootfile full-path="c.opf"/></rootfiles></container>"#,
        )
        .unwrap();
        z.start_file("c.opf", o).unwrap();
        std::io::Write::write_all(&mut z, opf.as_bytes()).unwrap();
        z.finish().unwrap();
        assert_eq!(read(&path).authors, vec!["Ana Lima", "Rui Prado Jr."]);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn junk_titles() {
        assert_eq!(clean_title("Microsoft Word - livro final.docx"), "");
        assert_eq!(clean_title("Untitled-1"), "");
        assert_eq!(clean_title("capa.indd"), "");
        assert_eq!(clean_title("  Vento   Leste "), "Vento Leste");
        assert_eq!(clean_title("2077"), "2077");
        assert_eq!(clean_title("..."), "");
        assert!(is_junk_author("Usuário"));
        assert!(!is_junk_author("Tomé de Alvarenga"));
    }

    #[test]
    fn opf() {
        let opf = r##"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
  <dc:title id="t1">Nina Torres e a Chave de Vidro</dc:title>
  <dc:title id="t2">Edição comemorativa</dc:title>
  <meta refines="#t2" property="title-type">subtitle</meta>
  <dc:creator id="c1">J. K. Brandão</dc:creator>
  <dc:creator id="c2">Rita Nunes</dc:creator>
  <meta refines="#c2" property="role" scheme="marc:relators">trl</meta>
  <dc:identifier opf:scheme="ISBN">978-0-00-000003-3</dc:identifier>
  <dc:identifier>urn:uuid:1234</dc:identifier>
  <dc:date opf:event="modification">2020-01-01</dc:date>
  <dc:date>2017-03-14</dc:date>
  <dc:publisher>Editora Vela</dc:publisher>
  <dc:language>pt-BR</dc:language>
  <meta property="belongs-to-collection" id="s">Nina Torres</meta>
  <meta refines="#s" property="group-position">1</meta>
 </metadata>
</package>"##;
        let doc = xml(opf).unwrap();
        let m = opf_meta(&doc);
        assert_eq!(m.title, "Nina Torres e a Chave de Vidro");
        assert_eq!(m.authors, vec!["J. K. Brandão"]);
        assert_eq!(m.isbns, vec!["9780000000033"]);
        assert_eq!(m.year, Some(2017));
        assert_eq!(m.publisher, "Editora Vela");
        assert_eq!(
            (m.series.as_str(), m.series_index.as_str()),
            ("Nina Torres", "1")
        );

        let series_meta = r#"<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
  <dc:title>Vento Leste</dc:title><dc:creator opf:role="aut">Tomé de Alvarenga</dc:creator>
  <dc:identifier opf:scheme="ISBN">0000000043</dc:identifier>
  <meta name="calibre:series" content="Obras"/><meta name="calibre:series_index" content="2.0"/>
 </metadata></package>"#;
        let m = opf_meta(&xml(series_meta).unwrap());
        assert_eq!(m.isbns, vec!["9780000000040"]);
        assert_eq!(series_index(&m.series_index), "2");
    }

    #[test]
    fn zip_paths() {
        assert_eq!(
            join_zip("OEBPS/content.opf", "Text/a%20b.xhtml#x"),
            "OEBPS/Text/a b.xhtml"
        );
        assert_eq!(
            join_zip("OPS/xml/content.opf", "../Text/c.xhtml"),
            "OPS/Text/c.xhtml"
        );
        assert_eq!(join_zip("content.opf", "c.xhtml"), "c.xhtml");
    }

    fn mobi_record(title: &str, exth: &[(u32, &str)]) -> Vec<u8> {
        let mut r = vec![0u8; 16];
        r.extend_from_slice(b"MOBI");
        let header_len = 232u32;
        r.extend_from_slice(&header_len.to_be_bytes());
        r.resize(16 + header_len as usize, 0);
        r[28..32].copy_from_slice(&65001u32.to_be_bytes());
        r[128..132].copy_from_slice(&0x40u32.to_be_bytes());
        let mut e = Vec::new();
        for (kind, v) in exth {
            e.extend_from_slice(&kind.to_be_bytes());
            e.extend_from_slice(&(8 + v.len() as u32).to_be_bytes());
            e.extend_from_slice(v.as_bytes());
        }
        r.extend_from_slice(b"EXTH");
        r.extend_from_slice(&(12 + e.len() as u32).to_be_bytes());
        r.extend_from_slice(&(exth.len() as u32).to_be_bytes());
        r.extend_from_slice(&e);
        let at = r.len() as u32;
        r.extend_from_slice(title.as_bytes());
        r[84..88].copy_from_slice(&at.to_be_bytes());
        r[88..92].copy_from_slice(&(title.len() as u32).to_be_bytes());
        r
    }

    #[test]
    fn mobi_header() {
        let r = mobi_record(
            "Ensaio sobre a neblina",
            &[
                (100, "Josué Saraiva"),
                (101, "Casa das Letras"),
                (104, "9780000000057"),
                (106, "1995-01-01"),
            ],
        );
        let m = parse_mobi(&r).unwrap();
        assert_eq!(m.title, "Ensaio sobre a neblina");
        assert_eq!(m.authors, vec!["Josué Saraiva"]);
        assert_eq!(m.isbns, vec!["9780000000057"]);
        assert_eq!(m.year, Some(1995));
        assert_eq!(m.publisher, "Casa das Letras");
    }

    #[test]
    fn fictionbook() {
        let fb = "<?xml version=\"1.0\" encoding=\"windows-1251\"?>\n<FictionBook><description><title-info>\
<author><first-name>\u{41b}\u{435}\u{432}</first-name><last-name>Volkov</last-name></author>\
<book-title>Inverno e Brasa</book-title><lang>pt</lang><sequence name=\"Obras\" number=\"3\"/></title-info>\
<publish-info><publisher>X</publisher><year>2011</year><isbn>978-0-00-000001-9</isbn></publish-info>\
</description><body>...</body></FictionBook>";
        let (bytes, _, _) = encoding_rs::WINDOWS_1251.encode(fb);
        let m = parse_fb2(&bytes).unwrap();
        assert_eq!(m.title, "Inverno e Brasa");
        assert_eq!(m.authors, vec!["\u{41b}\u{435}\u{432} Volkov"]);
        assert_eq!(m.isbns, vec!["9780000000019"]);
        assert_eq!((m.series.as_str(), m.series_index.as_str()), ("Obras", "3"));
        assert_eq!(m.year, Some(2011));
    }

    #[test]
    fn xmp_packet() {
        let x = r#"<x:xmpmeta><rdf:RDF><rdf:Description>
<dc:title><rdf:Alt><rdf:li xml:lang="x-default">O Casarão</rdf:li></rdf:Alt></dc:title>
<dc:creator><rdf:Seq><rdf:li>Aurélio Arruda</rdf:li></rdf:Seq></dc:creator>
<prism:isbn>978-0-00-000008-7</prism:isbn>
</rdf:Description></rdf:RDF></x:xmpmeta>"#;
        let m = xmp_meta(x);
        assert_eq!(m.title, "O Casarão");
        assert_eq!(m.authors, vec!["Aurélio Arruda"]);
    }
}
