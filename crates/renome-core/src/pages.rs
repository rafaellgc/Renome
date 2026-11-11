//! Comics kept as e-books: an EPUB, or a Kindle book (MOBI, AZW3), whose
//! every page is one picture. Its pictures are found in reading order, to
//! be put in a CBZ as they are (see [`crate::convert`]). A book with text
//! (a page with more than a few letters, or with no picture) is not a
//! comic: nothing could show its text in a CBZ. Neither is one whose
//! pictures need a password (DRM).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

/// Letters a page may show besides its picture (a page number).
const PAGE_TEXT: usize = 20;

/// Picture formats a comic reader shows.
fn picture_ext(name: &str) -> Option<&'static str> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" | "jpe" => "jpg",
        "png" => "png",
        "gif" => "gif",
        "webp" => "webp",
        "avif" => "avif",
        "jxl" => "jxl",
        _ => return None,
    })
}

/// Markup that shows nothing: the head, scripts, styles, notes.
static HIDDEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<head\b.*?</head\s*>|<script\b.*?</script\s*>|<style\b.*?</style\s*>|<!--.*?-->|<guide\b.*?</guide\s*>").unwrap()
});
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());
static ENTITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&#?\w{1,10};").unwrap());
/// Tags that show a picture.
static PICTURE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<(?:\w+:)?(?:img|image)\b[^>]*>").unwrap());

/// How many letters the markup shows (spaces left out).
fn shown_letters(markup: &str) -> usize {
    let text = TAG.replace_all(markup, " ");
    let text = ENTITY.replace_all(&text, "x");
    text.chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{a0}')
        .count()
}

/// The value of an attribute (`name`, or `prefix:name`) in a tag.
fn attr(tag: &str, name: &str) -> Option<String> {
    static ATTR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?s)\s(?:[\w-]+:)?([\w-]+)\s*=\s*(?:"([^"]*)"|'([^']*)')"#).unwrap()
    });
    ATTR.captures_iter(tag).find_map(|c| {
        c[1].eq_ignore_ascii_case(name).then(|| {
            c.get(2)
                .or(c.get(3))
                .map_or("", |m| m.as_str())
                .trim()
                .to_string()
        })
    })
}

// ---------------------------------------------------------------- EPUB

/// A picture page of an EPUB: the zip entry of its picture.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EpubPage {
    pub entry: String,
    pub ext: &'static str,
}

/// An EPUB comic: its pages, and the `ComicInfo.xml` it carries, if any.
#[derive(Debug, Default)]
pub(crate) struct EpubComic {
    pub pages: Vec<EpubPage>,
    pub info: Option<String>,
}

fn entry_text<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<String, String> {
    let f = zip
        .by_name(name)
        .map_err(|e| format!("archiveBroken: {name}: {e}"))?;
    let mut bytes = Vec::new();
    f.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("archiveBroken: {e}"))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn xml(text: &str) -> Result<roxmltree::Document<'_>, String> {
    roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .map_err(|e| format!("archiveBroken: {e}"))
}

/// The pages of the EPUB at `path`, when it is a comic. Errors:
/// `archiveLocked` (DRM), `comicUnsupported` (a book with text, or a
/// page that is not one picture), `archiveBroken: …`.
pub(crate) fn epub(path: &Path) -> Result<EpubComic, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut zip =
        zip::ZipArchive::new(BufReader::new(file)).map_err(|e| format!("archiveBroken: {e}"))?;
    let names: HashSet<String> = zip
        .file_names()
        .filter_map(Result::ok)
        .map(|n| n.into_owned())
        .collect();
    let container = entry_text(&mut zip, "META-INF/container.xml", 1 << 20)?;
    let opf_path = xml(&container)?
        .descendants()
        .find(|n| n.has_tag_name("rootfile") && n.attribute("full-path").is_some())
        .and_then(|n| n.attribute("full-path"))
        .map(str::to_string)
        .ok_or("archiveBroken: no package")?;
    // Pictures behind a password: the store's DRM (fonts may be only
    // scrambled, which does not matter here).
    let locked: HashSet<String> = if names.contains("META-INF/encryption.xml") {
        let enc = entry_text(&mut zip, "META-INF/encryption.xml", 8 << 20)?;
        let doc = xml(&enc)?;
        doc.descendants()
            .filter(|n| n.has_tag_name("CipherReference"))
            .filter_map(|n| n.attribute("URI"))
            .map(|u| crate::ebook::join_zip("", u))
            .collect()
    } else {
        HashSet::new()
    };
    let opf = entry_text(&mut zip, &opf_path, 16 << 20)?;
    let doc = xml(&opf)?;
    // id → (path in the zip, media type, properties)
    let mut manifest: HashMap<&str, (String, &str, &str)> = HashMap::new();
    let mut types: HashMap<String, &str> = HashMap::new();
    for n in doc.descendants().filter(|n| n.has_tag_name("item")) {
        let (Some(id), Some(href)) = (n.attribute("id"), n.attribute("href")) else {
            continue;
        };
        let at = crate::ebook::join_zip(&opf_path, href);
        let media = n.attribute("media-type").unwrap_or_default();
        types.insert(at.clone(), media);
        manifest.insert(
            id,
            (at, media, n.attribute("properties").unwrap_or_default()),
        );
    }
    let picture_of = |at: &str| -> Result<EpubPage, String> {
        if locked.contains(at) {
            return Err("archiveLocked".into());
        }
        if !names.contains(at) {
            return Err(format!("archiveBroken: {at} missing"));
        }
        let media = types.get(at).copied().unwrap_or_default();
        let ext = picture_ext(at)
            .filter(|_| media.is_empty() || media.starts_with("image/"))
            .ok_or("comicUnsupported")?;
        Ok(EpubPage {
            entry: at.to_string(),
            ext,
        })
    };
    let mut pages = Vec::new();
    for item in doc.descendants().filter(|n| n.has_tag_name("itemref")) {
        let Some((at, media, _)) = item.attribute("idref").and_then(|id| manifest.get(id)) else {
            continue;
        };
        if media.starts_with("image/") && *media != "image/svg+xml" {
            pages.push(picture_of(at)?);
            continue;
        }
        if locked.contains(at) {
            return Err("archiveLocked".into());
        }
        let markup = entry_text(&mut zip, at, 8 << 20)?;
        let shown = HIDDEN.replace_all(&markup, " ");
        let mut pictures: Vec<String> = Vec::new();
        for tag in PICTURE.find_iter(&shown) {
            let link = attr(tag.as_str(), "src").or_else(|| attr(tag.as_str(), "href"));
            if let Some(link) = link.filter(|l| !l.is_empty() && !l.starts_with("data:")) {
                let p = crate::ebook::join_zip(at, &link);
                if !pictures.contains(&p) {
                    pictures.push(p);
                }
            }
        }
        if shown_letters(&shown) > PAGE_TEXT {
            return Err("comicUnsupported".into());
        }
        match pictures.as_slice() {
            // A blank page: nothing to keep.
            [] => {}
            [one] => pages.push(picture_of(one)?),
            _ => return Err("comicUnsupported".into()),
        }
    }
    if pages.is_empty() {
        return Err("comicUnsupported".into());
    }
    // A cover kept apart from the pages goes first.
    let cover = manifest
        .values()
        .find(|(_, media, props)| media.starts_with("image/") && props.contains("cover-image"))
        .map(|(at, ..)| at.clone())
        .or_else(|| {
            let id = doc
                .descendants()
                .find(|n| n.has_tag_name("meta") && n.attribute("name") == Some("cover"))?
                .attribute("content")?;
            let (at, media, _) = manifest.get(id)?;
            media.starts_with("image/").then(|| at.clone())
        });
    if let Some(cover) = cover
        && !pages.iter().any(|p| p.entry == cover)
        && let Ok(p) = picture_of(&cover)
    {
        pages.insert(0, p);
    }
    let info = names
        .iter()
        .find(|n| {
            n.rsplit('/')
                .next()
                .is_some_and(|f| f.eq_ignore_ascii_case("ComicInfo.xml"))
        })
        .cloned();
    Ok(EpubComic { pages, info })
}

// ---------------------------------------------------------------- Kindle

pub(crate) fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

pub(crate) fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The records of a Palm database (what a Kindle book is), read from the
/// file as asked.
pub(crate) struct Records {
    file: RefCell<File>,
    pub(crate) starts: Vec<u64>,
}

impl Records {
    pub(crate) fn open(path: &Path) -> Result<Records, String> {
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        let mut head = [0u8; 78];
        file.read_exact(&mut head)
            .map_err(|_| "comicUnsupported".to_string())?;
        if &head[60..68] != b"BOOKMOBI" && &head[60..68] != b"TEXtREAd" {
            return Err("comicUnsupported".into());
        }
        let count = be16(&head, 76).unwrap_or(0) as usize;
        let mut list = vec![0u8; count * 8];
        file.read_exact(&mut list)
            .map_err(|_| "archiveBroken: records".to_string())?;
        let mut starts: Vec<u64> = (0..count)
            .map(|i| be32(&list, i * 8).unwrap_or(0) as u64)
            .collect();
        starts.push(size);
        if starts.windows(2).any(|w| w[0] > w[1]) {
            return Err("archiveBroken: records".into());
        }
        Ok(Records {
            file: RefCell::new(file),
            starts,
        })
    }

    pub(crate) fn span(&self, i: usize) -> Option<(u64, u64)> {
        Some((*self.starts.get(i)?, *self.starts.get(i + 1)?))
    }

    /// At most `limit` bytes from the start of record `i`.
    pub(crate) fn read(&self, i: usize, limit: u64) -> Option<Vec<u8>> {
        let (a, b) = self.span(i)?;
        let len = (b - a).min(limit);
        let mut f = self.file.borrow_mut();
        f.seek(SeekFrom::Start(a)).ok()?;
        let mut out = vec![0u8; len as usize];
        f.read_exact(&mut out).ok()?;
        Some(out)
    }
}

/// The book part of a Kindle file: an old one (MOBI), or the newer one
/// (KF8, AZW3), alone or after an old one in the same file.
pub(crate) struct Part {
    /// The record of its header.
    pub(crate) at: usize,
    pub(crate) compression: u16,
    pub(crate) text_records: usize,
    pub(crate) encrypted: bool,
    pub(crate) first_picture: usize,
    pub(crate) trailing: u16,
    pub(crate) kf8: bool,
    /// Where the newer part starts (in an old part's header).
    pub(crate) boundary: Option<usize>,
    /// The cover, counted from the first picture.
    pub(crate) cover: Option<usize>,
}

pub(crate) fn part(r: &Records, at: usize) -> Option<Part> {
    let h = r.read(at, 64 << 10)?;
    if h.get(16..20)? != b"MOBI" {
        return None;
    }
    let len = be32(&h, 20)? as usize;
    let version = be32(&h, 36)?;
    let mut p = Part {
        at,
        compression: be16(&h, 0)?,
        text_records: be16(&h, 8)? as usize,
        encrypted: be16(&h, 12)? != 0,
        first_picture: be32(&h, 108)? as usize,
        trailing: if len >= 0xE4 {
            be16(&h, 0xF2).unwrap_or(0)
        } else {
            0
        },
        kf8: version >= 8,
        boundary: None,
        cover: None,
    };
    if be32(&h, 128).is_some_and(|f| f & 0x40 != 0) {
        let e = 16 + len;
        if h.get(e..e + 4) == Some(b"EXTH") {
            let count = be32(&h, e + 8)?;
            let mut q = e + 12;
            for _ in 0..count {
                let (kind, size) = (be32(&h, q)?, be32(&h, q + 4)? as usize);
                if size < 8 {
                    break;
                }
                let v = be32(&h, q + 8).map(|v| v as usize);
                match kind {
                    121 => p.boundary = v.filter(|v| *v > at && *v != u32::MAX as usize),
                    201 => p.cover = v.filter(|v| *v != u32::MAX as usize),
                    _ => {}
                }
                q += size;
            }
        }
    }
    Some(p)
}

/// A text record without the bytes appended after its text.
pub(crate) fn without_trailing(rec: &[u8], flags: u16) -> &[u8] {
    let mut end = rec.len();
    for bit in 1..16 {
        if flags & (1 << bit) == 0 {
            continue;
        }
        // A size written backwards at the end, its own bytes counted.
        let mut size = 0usize;
        for k in 0..4.min(end) {
            let b = rec[end - 1 - k];
            size |= ((b & 0x7f) as usize) << (7 * k);
            if b & 0x80 != 0 {
                break;
            }
        }
        end = end.saturating_sub(size);
    }
    if flags & 1 != 0 && end > 0 {
        end = end.saturating_sub((rec[end - 1] & 3) as usize + 1);
    }
    &rec[..end]
}

/// PalmDOC compression undone.
pub(crate) fn palmdoc(input: &[u8], out: &mut Vec<u8>) {
    let start = out.len();
    let mut i = 0;
    while i < input.len() {
        let b = input[i];
        i += 1;
        match b {
            0x01..=0x08 => {
                let n = (b as usize).min(input.len() - i);
                out.extend_from_slice(&input[i..i + n]);
                i += n;
            }
            0x80..=0xbf => {
                let Some(&next) = input.get(i) else { break };
                i += 1;
                let pair = ((b as usize) << 8) | next as usize;
                let back = (pair >> 3) & 0x7ff;
                let len = (pair & 7) + 3;
                if back == 0 || back > out.len() - start {
                    continue;
                }
                for _ in 0..len {
                    out.push(out[out.len() - back]);
                }
            }
            0xc0..=0xff => {
                out.push(b' ');
                out.push(b ^ 0x80);
            }
            _ => out.push(b),
        }
    }
}

/// The part's text (markup), at most `limit` bytes.
fn text_of(r: &Records, p: &Part, limit: usize) -> Result<String, String> {
    let mut out = Vec::new();
    for i in p.at + 1..=p.at + p.text_records {
        let rec = r.read(i, 1 << 20).ok_or("archiveBroken: text")?;
        let rec = without_trailing(&rec, p.trailing);
        match p.compression {
            1 => out.extend_from_slice(rec),
            2 => palmdoc(rec, &mut out),
            // The dictionary compression of some Kindle books.
            _ => return Err("comicUnsupported".into()),
        }
        if out.len() > limit {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

/// A picture of a Kindle book: where it is in the file, its format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct KindlePage {
    pub start: u64,
    pub len: u64,
    pub ext: &'static str,
}

/// Record `i` as a picture (a larger one is wrapped in a `CRES` record).
fn picture_at(r: &Records, i: usize) -> Option<KindlePage> {
    let (a, b) = r.span(i)?;
    let head = r.read(i, 16)?;
    let (skip, magic) = if head.starts_with(b"CRES") {
        (12, head.get(12..)?)
    } else {
        (0, &head[..])
    };
    let ext = if magic.starts_with(&[0xff, 0xd8, 0xff]) {
        "jpg"
    } else if magic.starts_with(b"\x89PNG") {
        "png"
    } else if magic.starts_with(b"GIF8") {
        "gif"
    } else {
        return None;
    };
    Some(KindlePage {
        start: a + skip,
        len: b - a - skip,
        ext,
    })
}

/// Pictures shown by the part's text, in order: record numbers counted
/// from the part's first picture (from 0).
fn shown_pictures(text: &str, kf8: bool) -> Option<Vec<usize>> {
    let mut v = Vec::new();
    for tag in PICTURE.find_iter(text) {
        let tag = tag.as_str();
        let n = if kf8 {
            let link = attr(tag, "src").or_else(|| attr(tag, "href"))?;
            let code = link.strip_prefix("kindle:embed:")?;
            let code: String = code
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            usize::from_str_radix(&code, 32).ok()?
        } else {
            ["hirecindex", "recindex", "lorecindex"]
                .iter()
                .find_map(|a| attr(tag, a))?
                .parse()
                .ok()?
        };
        v.push(n.checked_sub(1)?);
    }
    Some(v)
}

/// The pictures of one part, when its text shows only pictures.
fn part_pages(r: &Records, p: &Part) -> Result<Vec<KindlePage>, String> {
    if p.encrypted {
        return Err("archiveLocked".into());
    }
    let text = text_of(r, p, 64 << 20)?;
    let shown = HIDDEN.replace_all(&text, " ");
    let mut found = shown_pictures(&shown, p.kf8).ok_or("comicUnsupported")?;
    if found.is_empty() || shown_letters(&shown) > PAGE_TEXT * found.len() {
        return Err("comicUnsupported".into());
    }
    if let Some(c) = p.cover
        && !found.contains(&c)
    {
        found.insert(0, c);
    }
    // In a newer part after an old one, pictures are counted either from
    // the file's start or from the part's: the count that finds them all.
    let bases = if p.at > 0 {
        vec![p.first_picture + p.at, p.first_picture]
    } else {
        vec![p.first_picture]
    };
    for base in bases {
        let pages: Option<Vec<KindlePage>> =
            found.iter().map(|n| picture_at(r, base + n)).collect();
        if let Some(pages) = pages {
            return Ok(pages);
        }
    }
    Err("comicUnsupported".into())
}

/// The pages of the Kindle book at `path` (MOBI or AZW3), when it is a
/// comic: the newer part's pictures (sharper) when it has one. Errors as
/// for [`epub`].
/// The header record of a file's newer (KF8) part, from where its older
/// part's header says that part starts: the header itself, or the
/// `BOUN` record just before it (tools differ).
pub(crate) fn kf8_header(r: &Records, boundary: usize) -> usize {
    if r.read(boundary, 4).as_deref() == Some(b"BOUN") {
        boundary + 1
    } else {
        boundary
    }
}

pub(crate) fn kindle(path: &Path) -> Result<Vec<KindlePage>, String> {
    let r = Records::open(path)?;
    let first = part(&r, 0).ok_or("comicUnsupported")?;
    if first.encrypted {
        return Err("archiveLocked".into());
    }
    if let Some(newer) = first.boundary.and_then(|b| part(&r, kf8_header(&r, b)))
        && let Ok(pages) = part_pages(&r, &newer)
    {
        return Ok(pages);
    }
    part_pages(&r, &first)
}

/// Whether the Kindle book holds its newer part only (an AZW3).
pub(crate) fn kindle_is_kf8(path: &Path) -> bool {
    Records::open(path)
        .ok()
        .and_then(|r| part(&r, 0))
        .is_some_and(|p| p.kf8)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// A zip of `(name, bytes)`.
    pub(crate) fn zip_of(path: &Path, files: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, data) in files {
            let o = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            z.start_file(*name, o).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }

    const CONTAINER: &[u8] = br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;

    /// An EPUB with `pages` (name, markup) in its spine and `pictures`.
    pub(crate) fn epub_of(
        path: &Path,
        pages: &[(&str, &str)],
        pictures: &[(&str, &[u8])],
        extra_manifest: &str,
    ) {
        let mut manifest = String::new();
        let mut spine = String::new();
        for (i, (name, _)) in pages.iter().enumerate() {
            manifest.push_str(&format!(
                r#"<item id="p{i}" href="{name}" media-type="application/xhtml+xml"/>"#
            ));
            spine.push_str(&format!(r#"<itemref idref="p{i}"/>"#));
        }
        for (i, (name, _)) in pictures.iter().enumerate() {
            manifest.push_str(&format!(
                r#"<item id="i{i}" href="{name}" media-type="image/jpeg"/>"#
            ));
        }
        manifest.push_str(extra_manifest);
        let opf = format!(
            r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Um Quadrinho</dc:title><dc:creator>Fulano de Tal</dc:creator></metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#
        );
        let mut files: Vec<(String, Vec<u8>)> = vec![
            ("mimetype".into(), b"application/epub+zip".to_vec()),
            ("META-INF/container.xml".into(), CONTAINER.to_vec()),
            ("OEBPS/content.opf".into(), opf.into_bytes()),
        ];
        for (name, markup) in pages {
            files.push((format!("OEBPS/{name}"), markup.as_bytes().to_vec()));
        }
        for (name, data) in pictures {
            files.push((format!("OEBPS/{name}"), data.to_vec()));
        }
        let refs: Vec<(&str, &[u8])> = files
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_slice()))
            .collect();
        zip_of(path, &refs);
    }

    pub(crate) fn page(picture: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="utf-8"?><!DOCTYPE html><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Página com um título longo</title><style>body{{margin:0}}</style></head><body><div><img src="{picture}" alt="uma página do quadrinho"/></div></body></html>"#
        )
    }

    const SVG_PAGE: &str = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><title>x</title></head><body><svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 10 10"><image width="10" height="10" xlink:href="../Images/p%202.jpg"/></svg></body></html>"#;

    #[test]
    fn epub_comics_are_found_in_reading_order() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.epub");
        let jpeg = b"\xff\xd8\xff\xe0data";
        let one = page("../Images/b.jpg");
        let blank = r#"<html><head><title>em branco</title></head><body><p> </p></body></html>"#;
        epub_of(
            &p,
            &[
                ("Text/1.xhtml", &one),
                ("Text/2.xhtml", blank),
                ("Text/3.xhtml", SVG_PAGE),
            ],
            &[
                ("Images/b.jpg", jpeg),
                ("Images/p 2.jpg", jpeg),
                ("Images/c.jpg", jpeg),
            ],
            r#"<item id="cv" href="Images/c.jpg" media-type="image/jpeg" properties="cover-image"/>"#,
        );
        let c = epub(&p).unwrap();
        let entries: Vec<&str> = c.pages.iter().map(|p| p.entry.as_str()).collect();
        // The cover apart from the pages first, the blank page left out.
        assert_eq!(
            entries,
            [
                "OEBPS/Images/c.jpg",
                "OEBPS/Images/b.jpg",
                "OEBPS/Images/p 2.jpg"
            ]
        );
        assert!(c.info.is_none());
    }

    #[test]
    fn books_with_text_are_not_comics() {
        let dir = tempfile::tempdir().unwrap();
        let jpeg: &[u8] = b"\xff\xd8\xff\xe0data";
        let text = r#"<html><head><title>x</title></head><body><p>Era uma vez um reino muito distante.</p><img src="a.jpg"/></body></html>"#;
        let p = dir.path().join("t.epub");
        epub_of(&p, &[("1.xhtml", text)], &[("a.jpg", jpeg)], "");
        assert_eq!(epub(&p).unwrap_err(), "comicUnsupported");
        // Two pictures on a page: how they are laid out is unknown.
        let two = r#"<html><body><img src="a.jpg"/><img src="b.jpg"/></body></html>"#;
        epub_of(
            &p,
            &[("1.xhtml", two)],
            &[("a.jpg", jpeg), ("b.jpg", jpeg)],
            "",
        );
        assert_eq!(epub(&p).unwrap_err(), "comicUnsupported");
        // A page of text only.
        let only = r#"<html><body><p>Todos os direitos reservados ao autor.</p></body></html>"#;
        let pic = page("a.jpg");
        epub_of(
            &p,
            &[("1.xhtml", &pic), ("2.xhtml", only)],
            &[("a.jpg", jpeg)],
            "",
        );
        assert_eq!(epub(&p).unwrap_err(), "comicUnsupported");
        // Pictures under DRM.
        let enc = br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:CipherData><enc:CipherReference URI="OEBPS/a.jpg"/></enc:CipherData></enc:EncryptedData></encryption>"#;
        epub_of(&p, &[("1.xhtml", &pic)], &[("a.jpg", jpeg)], "");
        let mut z =
            zip::ZipWriter::new_append(File::options().read(true).write(true).open(&p).unwrap())
                .unwrap();
        z.start_file(
            "META-INF/encryption.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        z.write_all(enc).unwrap();
        z.finish().unwrap();
        assert_eq!(epub(&p).unwrap_err(), "archiveLocked");
    }

    #[test]
    fn trailing_bytes_and_palmdoc() {
        // Two trailing entries (sizes 3 and 2, written backwards), then
        // the multibyte one (1 byte + its count).
        let rec = [b'a', b'b', 0x00, b'x', b'y', 0x83, b'z', 0x82];
        assert_eq!(without_trailing(&rec, 0b111), b"ab");
        assert_eq!(without_trailing(b"abc", 0), b"abc");
        // A literal, a space pair, a copy back.
        let mut out = Vec::new();
        palmdoc(&[b'a', b'b', b'c', 0xc0 | b'd', 0x02, b'x', b'y'], &mut out);
        assert_eq!(out, b"abc dxy");
        let mut out = Vec::new();
        // "abcabcab": copy 5 from 3 back.
        let pair: u16 = 0x8000 | (3 << 3) | (5 - 3);
        palmdoc(&[b'a', b'b', b'c', (pair >> 8) as u8, pair as u8], &mut out);
        assert_eq!(out, b"abcabcab");
    }

    /// A Kindle book: one text record (stored), then the pictures.
    pub(crate) fn kindle_of(path: &Path, text: &str, pictures: &[&[u8]], kf8: bool, locked: bool) {
        let mut r0 = vec![0u8; 16 + 0xE8];
        r0[0..2].copy_from_slice(&1u16.to_be_bytes());
        r0[4..8].copy_from_slice(&(text.len() as u32).to_be_bytes());
        r0[8..10].copy_from_slice(&1u16.to_be_bytes());
        r0[12..14].copy_from_slice(&(locked as u16 * 2).to_be_bytes());
        r0[16..20].copy_from_slice(b"MOBI");
        r0[20..24].copy_from_slice(&0xE8u32.to_be_bytes());
        r0[36..40].copy_from_slice(&(if kf8 { 8u32 } else { 6 }).to_be_bytes());
        r0[108..112].copy_from_slice(&2u32.to_be_bytes());
        let mut records: Vec<Vec<u8>> = vec![r0, text.as_bytes().to_vec()];
        records.extend(pictures.iter().map(|p| p.to_vec()));
        records.push(b"FLIS\0\0\0\x08".to_vec());
        let mut out = vec![0u8; 78];
        out[60..68].copy_from_slice(b"BOOKMOBI");
        out[76..78].copy_from_slice(&(records.len() as u16).to_be_bytes());
        let mut at = 78 + records.len() * 8 + 2;
        for r in &records {
            out.extend_from_slice(&(at as u32).to_be_bytes());
            out.extend_from_slice(&[0, 0, 0, 0]);
            at += r.len();
        }
        out.extend_from_slice(&[0, 0]);
        for r in &records {
            out.extend_from_slice(r);
        }
        std::fs::write(path, out).unwrap();
    }

    #[test]
    fn kindle_comics_are_found_in_reading_order() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.azw3");
        let a: &[u8] = b"\xff\xd8\xff\xe0aaaa";
        let b: &[u8] = b"\x89PNG\r\n\x1a\nbbbb";
        let mut cres = b"CRES\0\0\0\0\0\0\0\0".to_vec();
        cres.extend_from_slice(a);
        let text = r#"<html><head><title>Um título</title></head><body><div><img src="kindle:embed:0002?mime=image/png"/></div><div><svg><image xlink:href="kindle:embed:0003?mime=image/jpeg"/></svg></div></body></html>"#;
        kindle_of(&p, text, &[a, b, &cres], true, false);
        let pages = kindle(&p).unwrap();
        assert_eq!(
            pages.iter().map(|p| p.ext).collect::<Vec<_>>(),
            ["png", "jpg"]
        );
        assert_eq!(pages[1].len, a.len() as u64);
        assert!(kindle_is_kf8(&p));
        // The old kind, with its record numbers.
        let p6 = dir.path().join("a.mobi");
        let text = r#"<html><body><img recindex="00001"/><mbp:pagebreak/><img hirecindex="00003" recindex="00002"/></body></html>"#;
        kindle_of(&p6, text, &[a, b, &cres], false, false);
        let pages = kindle(&p6).unwrap();
        assert_eq!(
            pages.iter().map(|p| p.ext).collect::<Vec<_>>(),
            ["jpg", "jpg"]
        );
        assert!(!kindle_is_kf8(&p6));
        // Text, and DRM.
        let book = r#"<html><body><p>Capítulo um. Era uma vez, num lugar muito distante, alguém.</p><img recindex="00001"/></body></html>"#;
        kindle_of(&p6, book, &[a], false, false);
        assert_eq!(kindle(&p6).unwrap_err(), "comicUnsupported");
        kindle_of(&p6, text, &[a, b, &cres], false, true);
        assert_eq!(kindle(&p6).unwrap_err(), "archiveLocked");
    }
}
