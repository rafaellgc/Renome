//! Comics turned into CBZ: a CBR (RAR), CB7 (7z) or CBT (tar), whatever its
//! name says, is written again as a zip with the same pages, in the same
//! order, and the details of [`crate::inside`] when asked. So is a PDF,
//! page by page: each page's picture taken out as it is when it can be,
//! else the page drawn (see [`pdf_pages`]). And an EPUB or a Kindle book
//! (MOBI, AZW3) whose every page is one picture, its pictures taken as
//! they are (see [`crate::pages`]). And a folder of pictures (see
//! [`folder_pages`]). Pictures are
//! stored as they are (they do not get smaller compressed); other files
//! are compressed. RAR archives are read with libarchive (see
//! `renome-archive`); they cannot be written, so a CBR's details can only
//! be written by converting it.
//!
//! The zip is written beside the comic; when whole, the original takes a
//! name with its own extension beside it (`Name.cbr`) and the zip the
//! comic's name. The original then stays there, or goes to the system's
//! trash. Undo puts a kept original back in the comic's place.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{Datelike, Local, TimeZone, Timelike};
use serde::{Deserialize, Serialize};

use crate::tagwrite::{Backup, temp_of};

/// What a comic archive really is, by its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Zip,
    Rar,
    SevenZip,
    Tar,
    Pdf,
    Epub,
    /// MOBI or AZW3.
    Kindle,
    /// A folder of pictures.
    Folder,
}

impl Kind {
    /// The comic extension of the kind.
    pub fn ext(self) -> &'static str {
        match self {
            Kind::Zip => "cbz",
            Kind::Rar => "cbr",
            Kind::SevenZip => "cb7",
            Kind::Tar => "cbt",
            Kind::Pdf => "pdf",
            Kind::Epub => "epub",
            Kind::Kindle => "mobi",
            Kind::Folder => "",
        }
    }
}

/// The kind of the archive at `path`, by its first bytes.
pub fn kind(path: &Path) -> Option<Kind> {
    if fs::symlink_metadata(path).ok()?.is_dir() {
        return Some(Kind::Folder);
    }
    let mut head = [0u8; 262];
    let mut f = File::open(path).ok()?;
    let mut n = 0;
    while n < head.len() {
        match f.read(&mut head[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(_) => return None,
        }
    }
    let h = &head[..n];
    if h.starts_with(b"PK\x03\x04") || h.starts_with(b"PK\x05\x06") {
        Some(if is_epub(path, h) {
            Kind::Epub
        } else {
            Kind::Zip
        })
    } else if renome_archive::is_rar(h) {
        Some(Kind::Rar)
    } else if h.starts_with(&[0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c]) {
        Some(Kind::SevenZip)
    } else if n >= 262 && &h[257..262] == b"ustar" {
        Some(Kind::Tar)
    } else if h.windows(5).any(|w| w == b"%PDF-") {
        Some(Kind::Pdf)
    } else if h.get(60..68) == Some(b"BOOKMOBI") {
        Some(Kind::Kindle)
    } else {
        None
    }
}

/// A zip that is an EPUB: says so in its first file (`mimetype`), or
/// has an EPUB's container.
fn is_epub(path: &Path, head: &[u8]) -> bool {
    if head.get(30..38) == Some(b"mimetype") {
        return head
            .get(38..)
            .is_some_and(|r| r.starts_with(b"application/epub+zip"));
    }
    File::open(path)
        .ok()
        .and_then(|f| zip::ZipArchive::new(f).ok())
        .is_some_and(|z| z.index_for_name("META-INF/container.xml").is_some())
}

/// Comics that can be converted: a comic's extension (or a PDF's, an
/// EPUB's or a Kindle book's), with a RAR, 7z or tar inside, or a PDF, or
/// an EPUB or Kindle book of pictures only.
pub fn convertible(path: &Path) -> bool {
    if path.is_dir() {
        return folder_pages(path).is_ok();
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !matches!(
        ext.as_str(),
        "cbz" | "cbr" | "cb7" | "cbt" | "pdf" | "epub" | "mobi" | "azw3" | "azw"
    ) {
        return false;
    }
    match kind(path) {
        Some(Kind::Rar | Kind::SevenZip | Kind::Tar) => {
            matches!(ext.as_str(), "cbz" | "cbr" | "cb7" | "cbt")
        }
        Some(k) => check_kind(path, k).is_some(),
        None => false,
    }
}

/// Whether the comic at `path` can be converted, and how many of its
/// pages would be drawn (only a PDF's are, see [`pdf_check`]). `None`: it
/// cannot be (an EPUB or Kindle book with text or DRM, a PDF that does
/// not open, already a CBZ…).
pub fn check(path: &Path) -> Option<usize> {
    check_kind(path, kind(path)?)
}

fn check_kind(path: &Path, kind: Kind) -> Option<usize> {
    match kind {
        Kind::Pdf => pdf_check(path),
        Kind::Epub => crate::pages::epub(path).ok().map(|_| 0),
        Kind::Kindle => crate::pages::kindle(path).ok().map(|_| 0),
        Kind::Folder => folder_pages(path).ok().map(|_| 0),
        Kind::Rar | Kind::SevenZip | Kind::Tar => Some(0),
        Kind::Zip => None,
    }
}

/// Files a folder of pages may hold besides them, left out.
fn folder_junk(name: &str) -> bool {
    name.starts_with('.')
        || ["thumbs.db", "desktop.ini", "ehthumbs.db"]
            .iter()
            .any(|j| name.eq_ignore_ascii_case(j))
}

/// Pictures a CBZ holds.
fn is_page(name: &str) -> bool {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "jxl"
    )
}

/// Folders inside a folder of pages (chapters): at most this deep.
const FOLDER_DEPTH: usize = 3;

/// The pages of a folder of pictures (a comic not yet put in a CBZ): its
/// files, under it, in order (numbers by their value), and its
/// `ComicInfo.xml`. Anything else in it (a video, a document, a link)
/// makes it no comic: `comicUnsupported`.
pub fn folder_pages(dir: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    fn walk(
        dir: &Path,
        under: &str,
        depth: usize,
        out: &mut Vec<(PathBuf, String)>,
    ) -> Result<(), String> {
        for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let name = e.file_name().to_string_lossy().into_owned();
            if folder_junk(&name) {
                continue;
            }
            let t = e.file_type().map_err(|e| e.to_string())?;
            let inner = if under.is_empty() {
                name.clone()
            } else {
                format!("{under}/{name}")
            };
            if t.is_dir() && depth < FOLDER_DEPTH {
                walk(&e.path(), &inner, depth + 1, out)?;
            } else if t.is_file() && (is_page(&name) || name.eq_ignore_ascii_case("ComicInfo.xml"))
            {
                out.push((e.path(), inner));
            } else {
                return Err("comicUnsupported".into());
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, "", 0, &mut out)?;
    if !out.iter().any(|(_, n)| is_page(n)) {
        return Err("comicUnsupported".into());
    }
    out.sort_by(|a, b| natural(&a.1, &b.1));
    Ok(out)
}

/// Names in the order a person reads them: `2` before `10`.
fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |s: &str| -> Vec<(bool, String)> {
        let mut v: Vec<(bool, String)> = Vec::new();
        for c in s.to_lowercase().chars() {
            let digit = c.is_ascii_digit();
            match v.last_mut() {
                Some((d, part)) if *d == digit => part.push(c),
                _ => v.push((digit, c.to_string())),
            }
        }
        v
    };
    let (pa, pb) = (parts(a), parts(b));
    for (x, y) in pa.iter().zip(&pb) {
        let o = match (x.0, y.0) {
            (true, true) => {
                let (tx, ty) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
                tx.len().cmp(&ty.len()).then_with(|| tx.cmp(ty))
            }
            _ => x.1.cmp(&y.1),
        };
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    pa.len().cmp(&pb.len()).then_with(|| a.cmp(b))
}

/// The largest picture read for [`end_pages`].
const END_PAGE_MAX: u64 = 40 << 20;

/// The cover of a comic or book, as picture bytes: an EPUB's cover item,
/// a PDF's first page drawn, else the first page of the archive, folder
/// or picture-only Kindle book. `None` when there is none to read.
pub fn cover(path: &Path) -> Option<Vec<u8>> {
    match kind(path)? {
        Kind::Epub => crate::ebook::epub_cover(path).or_else(|| end_pages(path, 1, 0).pop()),
        Kind::Pdf => {
            let data = Arc::new(fs::read(path).ok()?);
            let pdf = hayro_pdf(&data).ok()?;
            let (pw, ph) = pdf.pages().first()?.render_dimensions();
            drawn(&pdf, 0, 1000.0 / pw.max(ph).max(1.0))
                .ok()
                .map(|(b, _)| b)
        }
        _ => end_pages(path, 1, 0).pop(),
    }
}

/// The pictures at the start and at the end of a comic, where a barcode is
/// printed: the first `head` and the last `tail` pages, in reading order
/// (fewer when the comic is shorter). A PDF gives none (its pages would
/// have to be drawn); an unreadable comic gives none.
pub fn end_pages(path: &Path, head: usize, tail: usize) -> Vec<Vec<u8>> {
    let Some(kind) = kind(path) else {
        return Vec::new();
    };
    let pick = |names: &[String]| -> Vec<usize> {
        let mut order: Vec<usize> = (0..names.len()).filter(|&i| is_page(&names[i])).collect();
        order.sort_by(|&a, &b| natural(&names[a], &names[b]));
        let mut out: Vec<usize> = order.iter().take(head).copied().collect();
        let from = order.len().saturating_sub(tail).max(out.len());
        out.extend(&order[from..]);
        out
    };
    let read = |r: &mut dyn Read| -> Option<Vec<u8>> {
        let mut v = Vec::new();
        r.take(END_PAGE_MAX).read_to_end(&mut v).ok()?;
        Some(v)
    };
    match kind {
        Kind::Pdf => Vec::new(),
        Kind::Zip => {
            let Ok(f) = File::open(path) else {
                return Vec::new();
            };
            let Ok(mut zip) = zip::ZipArchive::new(std::io::BufReader::new(f)) else {
                return Vec::new();
            };
            let names: Vec<String> = zip
                .file_names()
                .map(|n| n.map(|n| n.into_owned()).unwrap_or_default())
                .collect();
            pick(&names)
                .into_iter()
                .filter_map(|i| read(&mut zip.by_name(&names[i]).ok()?))
                .collect()
        }
        Kind::Folder => {
            let Ok(files) = folder_pages(path) else {
                return Vec::new();
            };
            let names: Vec<String> = files.iter().map(|(_, n)| n.clone()).collect();
            pick(&names)
                .into_iter()
                .filter_map(|i| read(&mut File::open(&files[i].0).ok()?))
                .collect()
        }
        _ => {
            // Read in one pass: the names are known only at the end, so
            // every page that may be wanted is kept until then.
            let mut names: Vec<String> = Vec::new();
            let mut kept: HashMap<usize, Vec<u8>> = HashMap::new();
            let mut first: Vec<String> = Vec::new();
            let mut last: Vec<String> = Vec::new();
            let mut drawn = 0;
            let _ = each_file(
                path,
                kind,
                |page, data| {
                    if !is_page(&page.name) {
                        return Ok(());
                    }
                    // Among the first or last names seen so far: may be wanted.
                    let n = &page.name;
                    let early = first.len() < head || first.iter().any(|f| natural(n, f).is_lt());
                    let late = last.len() < tail || last.iter().any(|l| natural(n, l).is_gt());
                    if early || late {
                        if let Some(v) = read(data) {
                            kept.insert(names.len(), v);
                        }
                        if early {
                            first.push(n.clone());
                            first.sort_by(|a, b| natural(a, b));
                            first.truncate(head);
                        }
                        if late {
                            last.push(n.clone());
                            last.sort_by(|a, b| natural(b, a));
                            last.truncate(tail);
                        }
                    }
                    names.push(n.clone());
                    // Pages pushed out of both ends are no longer wanted.
                    kept.retain(|&i, _| first.contains(&names[i]) || last.contains(&names[i]));
                    Ok(())
                },
                &mut drawn,
                PdfOptions::default(),
            );
            pick(&names)
                .into_iter()
                .filter_map(|i| kept.remove(&i))
                .collect()
        }
    }
}

/// What becomes of the original once converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Original {
    /// Stays beside the CBZ, with its own extension.
    Keep,
    /// Goes to the system's trash.
    Delete,
}

/// A conversion, for undo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Converted {
    /// Where the original was left (beside the CBZ); undo puts it back in
    /// the CBZ's place. `None`: it went to the trash.
    pub original: Option<PathBuf>,
    /// The original was a folder of pictures.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub folder: bool,
}

/// The outcome of [`convert`].
#[derive(Debug)]
pub struct Done {
    pub backup: Backup,
    /// The original was to go to the trash but could not: it was kept
    /// beside (the reason).
    pub not_trashed: Option<String>,
    /// PDF pages that could not be taken out as they are, and were drawn.
    pub drawn_pages: usize,
}

/// One file of the archive, as read.
struct Page {
    name: String,
    modified: Option<i64>,
}

/// Reads each file (not folder) of the archive in order: `each` gets its
/// name, date and content. A PDF's pages are its files (`drawn_pages`
/// counts those drawn, see [`pdf_files`]).
fn each_file(
    path: &Path,
    kind: Kind,
    mut each: impl FnMut(&Page, &mut dyn Read) -> Result<(), String>,
    drawn_pages: &mut usize,
    opts: PdfOptions,
) -> Result<(), String> {
    let broken = |e: String| format!("archiveBroken: {e}");
    match kind {
        Kind::Rar => renome_archive::read(path, |e, data| {
            if e.dir || e.other {
                return Ok(true);
            }
            if e.encrypted {
                return Err("archiveLocked".into());
            }
            each(
                &Page {
                    name: e.name.clone(),
                    modified: e.modified,
                },
                data,
            )?;
            Ok(true)
        })
        .map_err(|e| {
            if e == "archiveLocked" || e.starts_with("archiveBroken") {
                e
            } else if e.to_lowercase().contains("encrypt") || e.to_lowercase().contains("password")
            {
                "archiveLocked".into()
            } else {
                broken(e)
            }
        }),
        Kind::SevenZip => {
            let mut reader =
                sevenz_rust2::ArchiveReader::open(path, sevenz_rust2::Password::empty()).map_err(
                    |e| {
                        let e = e.to_string();
                        if e.to_lowercase().contains("password") {
                            "archiveLocked".to_string()
                        } else {
                            broken(e)
                        }
                    },
                )?;
            let mut failed = None;
            reader
                .for_each_entries(|entry, data| {
                    if entry.is_directory() {
                        return Ok(true);
                    }
                    let page = Page {
                        name: entry.name().to_string(),
                        modified: None,
                    };
                    match each(&page, data) {
                        Ok(()) => Ok(true),
                        Err(e) => {
                            failed = Some(e);
                            Ok(false)
                        }
                    }
                })
                .map_err(|e| broken(e.to_string()))?;
            failed.map_or(Ok(()), Err)
        }
        Kind::Tar => {
            let mut archive = tar::Archive::new(File::open(path).map_err(|e| e.to_string())?);
            for entry in archive.entries().map_err(|e| broken(e.to_string()))? {
                let mut entry = entry.map_err(|e| broken(e.to_string()))?;
                if !entry.header().entry_type().is_file() {
                    continue;
                }
                let name = entry
                    .path()
                    .map_err(|e| broken(e.to_string()))?
                    .to_string_lossy()
                    .into_owned();
                let modified = entry.header().mtime().ok().map(|t| t as i64);
                each(&Page { name, modified }, &mut entry)?;
            }
            Ok(())
        }
        Kind::Pdf => pdf_files(path, &mut each, drawn_pages, opts),
        Kind::Epub => {
            let comic = crate::pages::epub(path)?;
            let mut zip = zip::ZipArchive::new(std::io::BufReader::new(
                File::open(path).map_err(|e| e.to_string())?,
            ))
            .map_err(|e| broken(e.to_string()))?;
            let digits = comic.pages.len().to_string().len().max(3);
            for (i, page) in comic.pages.iter().enumerate() {
                let mut entry = zip
                    .by_name(&page.entry)
                    .map_err(|e| broken(e.to_string()))?;
                let modified = entry.last_modified().and_then(zip_secs);
                let name = format!("{:0digits$}.{}", i + 1, page.ext);
                each(&Page { name, modified }, &mut entry)?;
            }
            if let Some(info) = comic.info {
                let mut entry = zip.by_name(&info).map_err(|e| broken(e.to_string()))?;
                let page = Page {
                    name: "ComicInfo.xml".into(),
                    modified: None,
                };
                each(&page, &mut entry)?;
            }
            Ok(())
        }
        Kind::Kindle => {
            let pages = crate::pages::kindle(path)?;
            let mut file = File::open(path).map_err(|e| e.to_string())?;
            let digits = pages.len().to_string().len().max(3);
            for (i, page) in pages.iter().enumerate() {
                use std::io::Seek;
                file.seek(std::io::SeekFrom::Start(page.start))
                    .map_err(|e| broken(e.to_string()))?;
                let name = format!("{:0digits$}.{}", i + 1, page.ext);
                let mut data = (&mut file).take(page.len);
                each(
                    &Page {
                        name,
                        modified: None,
                    },
                    &mut data,
                )?;
            }
            Ok(())
        }
        Kind::Folder => {
            for (file, name) in folder_pages(path)? {
                let modified = fs::metadata(&file)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);
                let mut f = File::open(&file).map_err(|e| e.to_string())?;
                each(&Page { name, modified }, &mut f)?;
            }
            Ok(())
        }
        Kind::Zip => Err("comicUnsupported".into()),
    }
}

// ---------------------------------------------------------------- PDF
//
// Each page's picture is taken out as it is when it can be (a JPEG copied,
// pixels written as a PNG), else decoded without loss (JPEG 2000, JBIG2,
// fax) and written as a PNG, else with colors made RGB (CMYK). Only when
// none of that fits the page (drawn pages, layers, several pictures…) is
// the page drawn, with hayro, as a reader would show it.

/// A direction on the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Dir {
    Right,
    Left,
    Up,
    Down,
}

impl Dir {
    /// One step that way, `y` growing downwards.
    fn step(self) -> (i64, i64) {
        match self {
            Dir::Right => (1, 0),
            Dir::Left => (-1, 0),
            Dir::Up => (0, -1),
            Dir::Down => (0, 1),
        }
    }

    fn across(self) -> bool {
        matches!(self, Dir::Right | Dir::Left)
    }
}

/// Where a picture's rows run (`x`, its first pixel to its last) and where
/// its bottom-to-top runs (`y`) on the screen. Upright: right and up.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Turn {
    x: Dir,
    y: Dir,
}

const UPRIGHT: Turn = Turn {
    x: Dir::Right,
    y: Dir::Up,
};

/// The colors of a picture's pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Colors {
    Gray,
    Rgb,
    Cmyk,
    /// Numbers of colors in a palette (see [`palette`]).
    Indexed,
}

impl Colors {
    fn channels(self) -> usize {
        match self {
            Colors::Gray | Colors::Indexed => 1,
            Colors::Rgb => 3,
            Colors::Cmyk => 4,
        }
    }
}

/// A picture's compression that needs decoding.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Codec {
    Jpeg,
    Jpx,
    Jbig2,
    Ccitt,
}

/// How a page's picture is taken out of the PDF.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Pic {
    /// Compressed with `codec`, after `pre` plain layers (Flate, ASCII85…).
    Coded {
        codec: Codec,
        pre: usize,
        colors: Option<Colors>,
        invert: bool,
    },
    /// Plain pixels; `invert`: the values are the other way round (1 for
    /// black, ink amounts).
    Pixels {
        width: u32,
        height: u32,
        colors: Colors,
        bits: u8,
        invert: bool,
    },
    /// A page with nothing on it: a white page as big as the others.
    Blank,
    /// Drawn as a reader shows it, `scale` pixels per point.
    Render { scale: f32 },
}

/// Where a picture is: an object of the PDF, or in a page's content.
#[derive(Debug, Clone, PartialEq)]
enum Src {
    Obj(lopdf::ObjectId),
    Inline(Box<lopdf::Stream>),
}

/// A page of the PDF and how it is taken out.
#[derive(Debug)]
struct PdfPage {
    src: Option<Src>,
    pic: Pic,
    turn: Turn,
    /// A soft mask: taken out only when it hides nothing.
    smask: Option<lopdf::ObjectId>,
    /// How to draw it, if taking it out fails.
    scale: f32,
}

/// Loads a PDF; with `light`, without the pictures' data (only what they
/// are).
fn load_pdf(path: &Path, light: bool) -> Result<lopdf::Document, String> {
    load_pdf_from(Err(path), light)
}

/// [`load_pdf`] from the file's bytes (`Ok`) or its path.
fn load_pdf_from(from: Result<&[u8], &Path>, light: bool) -> Result<lopdf::Document, String> {
    fn no_image_data(
        id: lopdf::ObjectId,
        obj: &mut lopdf::Object,
    ) -> Option<(lopdf::ObjectId, lopdf::Object)> {
        if let lopdf::Object::Stream(s) = obj
            && s.dict
                .get(b"Subtype")
                .and_then(|v| v.as_name())
                .is_ok_and(|n| n == b"Image")
        {
            s.content.clear();
        }
        Some((id, obj.clone()))
    }
    let options = lopdf::LoadOptions {
        filter: light.then_some(no_image_data as lopdf::FilterFunc),
        max_decompressed_size: Some(256 << 20),
        ..Default::default()
    };
    // One that opens without a password is read; one that needs it fails
    // here.
    let loaded = match from {
        Ok(bytes) => lopdf::Document::load_mem_with_options(bytes, options),
        Err(path) => lopdf::Document::load_with_options(path, options),
    };
    let doc = loaded.map_err(|e| {
        let e = e.to_string();
        if e.to_lowercase().contains("password") || e.to_lowercase().contains("decrypt") {
            "pdfLocked".to_string()
        } else {
            format!("archiveBroken: {e}")
        }
    })?;
    if doc.is_encrypted() {
        return Err("pdfLocked".into());
    }
    Ok(doc)
}

/// The PDF as hayro reads it (for drawing pages), from bytes shared with
/// the other readers.
fn hayro_pdf(data: &Arc<Vec<u8>>) -> Result<hayro::hayro_syntax::Pdf, String> {
    hayro::hayro_syntax::Pdf::new(hayro::hayro_syntax::PdfData::from(data.clone())).map_err(|e| {
        match e {
            hayro::hayro_syntax::LoadPdfError::Decryption(_) => "pdfLocked".to_string(),
            e => format!("archiveBroken: {e:?}"),
        }
    })
}

/// Whether the PDF at `path` can be converted, and how many of its pages
/// would be drawn (an estimate: read without the pictures' data, a
/// picture that fails to decode is found only when converting). `None`:
/// it does not open without a password, or has no pages.
pub fn pdf_check(path: &Path) -> Option<usize> {
    match load_pdf(path, true) {
        Ok(doc) => {
            let pages = pdf_pages(&doc, PdfOptions::default().size);
            let drawn = pages
                .iter()
                .filter(|p| matches!(p.pic, Pic::Render { .. }))
                .count();
            (!pages.is_empty()).then_some(drawn)
        }
        Err(e) if e == "pdfLocked" => None,
        // One the PDF library cannot read: every page drawn.
        Err(_) => {
            let data = Arc::new(fs::read(path).ok()?);
            let n = hayro_pdf(&data).ok()?.pages().len();
            (n > 0).then_some(n)
        }
    }
}

/// Whether the PDF at `path` can be converted (see [`pdf_check`]).
pub fn pdf_of_pictures(path: &Path) -> bool {
    pdf_check(path).is_some()
}

/// How PDFs are converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PdfOptions {
    /// Pages that are not just a picture are drawn; else such a PDF is not
    /// converted (`pdfDrawn`).
    pub draw: bool,
    /// The long side, in pixels, of a drawn page with no picture (one with
    /// pictures is drawn at the sharpest one's resolution).
    pub size: u32,
    /// The file's own title and author (a PDF's, an EPUB's, a Kindle
    /// book's) go into the `ComicInfo.xml` when the values do not have them.
    pub info: bool,
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions {
            draw: true,
            size: 2400,
            info: false,
        }
    }
}

fn deref<'a>(doc: &'a lopdf::Document, o: &'a lopdf::Object) -> &'a lopdf::Object {
    doc.dereference(o).map(|(_, o)| o).unwrap_or(o)
}

fn name_of<'a>(doc: &'a lopdf::Document, o: &'a lopdf::Object) -> Option<&'a [u8]> {
    deref(doc, o).as_name().ok()
}

fn num(o: &lopdf::Object) -> Option<f64> {
    match o {
        lopdf::Object::Integer(i) => Some(*i as f64),
        lopdf::Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

/// A page's own entry, else the one it inherits.
fn inherited<'a>(
    doc: &'a lopdf::Document,
    page: lopdf::ObjectId,
    key: &[u8],
) -> Option<&'a lopdf::Object> {
    let mut dict = doc.get_dictionary(page).ok()?;
    for _ in 0..32 {
        if let Ok(v) = dict.get(key) {
            return Some(deref(doc, v));
        }
        let parent = dict
            .get(b"Parent")
            .and_then(lopdf::Object::as_reference)
            .ok()?;
        dict = doc.get_dictionary(parent).ok()?;
    }
    None
}

/// The part of the page shown (crop box, else media box), as
/// `[x0, y0, x1, y1]`.
fn page_box(doc: &lopdf::Document, page: lopdf::ObjectId) -> Option<[f64; 4]> {
    let b = inherited(doc, page, b"CropBox").or_else(|| inherited(doc, page, b"MediaBox"))?;
    let v: Vec<f64> = b
        .as_array()
        .ok()?
        .iter()
        .filter_map(|o| num(deref(doc, o)))
        .collect();
    let [a, b, c, d] = v.as_slice() else {
        return None;
    };
    Some([a.min(*c), b.min(*d), a.max(*c), b.max(*d)])
}

/// Visible annotations with their own look (stamps, comments, signatures,
/// filled-in fields): drawn over the page, outside its content.
fn annotated(doc: &lopdf::Document, page: lopdf::ObjectId) -> bool {
    let Ok(page) = doc.get_dictionary(page) else {
        return false;
    };
    let Ok(annots) = page.get(b"Annots").map(|a| deref(doc, a)) else {
        return false;
    };
    let Ok(annots) = annots.as_array() else {
        return false;
    };
    annots.iter().any(|a| {
        let Ok(a) = deref(doc, a).as_dict() else {
            return false;
        };
        let kind = a.get(b"Subtype").ok().and_then(|s| name_of(doc, s));
        let flags = a
            .get(b"F")
            .ok()
            .and_then(|f| num(deref(doc, f)))
            .unwrap_or(0.0) as i64;
        // Hidden, or not shown on the screen.
        let unseen = flags & (2 | 32) != 0;
        !matches!(kind, Some(b"Link" | b"Popup")) && !unseen && a.has(b"AP")
    })
}

/// The XObject named `name` in a page's resources (its own, else those it
/// inherits).
fn xobject(doc: &lopdf::Document, page: lopdf::ObjectId, name: &[u8]) -> Option<lopdf::ObjectId> {
    let (own, inherited) = doc.get_page_resources(page).ok()?;
    let dicts = own.into_iter().chain(
        inherited
            .iter()
            .filter_map(|id| doc.get_dictionary(*id).ok()),
    );
    for res in dicts {
        let Ok(x) = res.get(b"XObject") else {
            continue;
        };
        if let Ok(x) = deref(doc, x).as_dict()
            && let Ok(r) = x.get(name).and_then(lopdf::Object::as_reference)
        {
            return Some(r);
        }
    }
    None
}

/// The colors of a color space, and whether its values are ink amounts
/// (one ink, Separation or DeviceN: 1 is full ink, shown as gray the
/// other way round). `None` for those the program does not read (Lab,
/// several inks, patterns…). Indexed ones are [`Colors::Indexed`].
fn colors_of(doc: &lopdf::Document, cs: &lopdf::Object) -> Option<(Colors, bool)> {
    let cs = deref(doc, cs);
    if let Ok(n) = cs.as_name() {
        return match n {
            b"DeviceGray" | b"CalGray" | b"G" => Some((Colors::Gray, false)),
            b"DeviceRGB" | b"CalRGB" | b"RGB" => Some((Colors::Rgb, false)),
            b"DeviceCMYK" | b"CMYK" => Some((Colors::Cmyk, false)),
            _ => None,
        };
    }
    let arr = cs.as_array().ok()?;
    match arr.first().and_then(|f| name_of(doc, f))? {
        b"ICCBased" => {
            let s = deref(doc, arr.get(1)?).as_stream().ok()?;
            match s.dict.get(b"N").ok().and_then(num)? as i64 {
                1 => Some((Colors::Gray, false)),
                3 => Some((Colors::Rgb, false)),
                4 => Some((Colors::Cmyk, false)),
                _ => None,
            }
        }
        b"CalGray" => Some((Colors::Gray, false)),
        b"CalRGB" => Some((Colors::Rgb, false)),
        b"Indexed" | b"I" => Some((Colors::Indexed, false)),
        b"Separation" => Some((Colors::Gray, true)),
        b"DeviceN" => {
            let inks = deref(doc, arr.get(1)?).as_array().ok()?;
            (inks.len() == 1).then_some((Colors::Gray, true))
        }
        _ => None,
    }
}

/// An RGB value from CMYK ones (an approximation: no color profile).
fn rgb_of_cmyk(c: u8, m: u8, y: u8, k: u8) -> [u8; 3] {
    let k = 255 - k as u32;
    [c, m, y].map(|v| ((255 - v as u32) * k / 255) as u8)
}

/// The palette of an indexed color space (`[/Indexed base hival lookup]`),
/// as RGB colors.
fn palette(doc: &lopdf::Document, cs: Option<&lopdf::Object>) -> Option<Vec<u8>> {
    let arr = deref(doc, cs?).as_array().ok()?;
    if !matches!(
        arr.first().and_then(|f| name_of(doc, f))?,
        b"Indexed" | b"I"
    ) {
        return None;
    }
    let (base, ink) = colors_of(doc, arr.get(1)?)?;
    let colors = deref(doc, arr.get(2)?).as_i64().ok()? + 1;
    if !(1..=256).contains(&colors) {
        return None;
    }
    let lookup = match deref(doc, arr.get(3)?) {
        lopdf::Object::String(b, _) => b.clone(),
        lopdf::Object::Stream(st) => st.decompressed_content_with_limit(256 * 4 + 64).ok()?,
        _ => return None,
    };
    let n = base.channels();
    if base == Colors::Indexed || lookup.len() < colors as usize * n {
        return None;
    }
    let rgb = lookup[..colors as usize * n]
        .chunks(n)
        .flat_map(|c| match base {
            Colors::Gray => {
                let g = if ink { 255 - c[0] } else { c[0] };
                [g, g, g]
            }
            Colors::Rgb => [c[0], c[1], c[2]],
            _ => rgb_of_cmyk(c[0], c[1], c[2], c[3]),
        })
        .collect();
    Some(rgb)
}

/// The filters of a picture, by their full names.
fn filters_of(doc: &lopdf::Document, d: &lopdf::Dictionary) -> Option<Vec<Vec<u8>>> {
    let full = |n: &[u8]| -> Vec<u8> {
        match n {
            b"Fl" => b"FlateDecode".to_vec(),
            b"DCT" => b"DCTDecode".to_vec(),
            b"LZW" => b"LZWDecode".to_vec(),
            b"AHx" => b"ASCIIHexDecode".to_vec(),
            b"A85" => b"ASCII85Decode".to_vec(),
            b"RL" => b"RunLengthDecode".to_vec(),
            b"CCF" => b"CCITTFaxDecode".to_vec(),
            n => n.to_vec(),
        }
    };
    match d.get(b"Filter").map(|f| deref(doc, f)) {
        Ok(lopdf::Object::Name(n)) => Some(vec![full(n)]),
        Ok(lopdf::Object::Array(a)) => a.iter().map(|f| name_of(doc, f).map(full)).collect(),
        Ok(_) => None,
        Err(_) => Some(Vec::new()),
    }
}

/// Plain layers the PDF library undoes.
fn plain_filter(f: &[u8]) -> bool {
    matches!(
        f,
        b"FlateDecode"
            | b"LZWDecode"
            | b"ASCII85Decode"
            | b"ASCIIHexDecode"
            | b"RunLengthDecode"
            | b"BrotliDecode"
    )
}

/// An inline picture's entries by their full names (`W` → `Width`…).
fn full_keys(s: &lopdf::Stream) -> lopdf::Stream {
    let mut d = lopdf::Dictionary::new();
    for (k, v) in s.dict.iter() {
        let key: &[u8] = match k.as_slice() {
            b"W" => b"Width",
            b"H" => b"Height",
            b"BPC" => b"BitsPerComponent",
            b"CS" => b"ColorSpace",
            b"F" => b"Filter",
            b"DP" => b"DecodeParms",
            b"D" => b"Decode",
            b"IM" => b"ImageMask",
            b"I" => b"Interpolate",
            k => k,
        };
        d.set(key, v.clone());
    }
    lopdf::Stream::new(d, s.content.clone())
}

/// How a picture (its entries `d`) can be taken out, and its soft mask;
/// `None` when it cannot (the page is then drawn).
fn pic_of(doc: &lopdf::Document, d: &lopdf::Dictionary) -> Option<(Pic, Option<lopdf::ObjectId>)> {
    let truthy = |k: &[u8]| d.get(k).and_then(lopdf::Object::as_bool).unwrap_or(false);
    // A mask by colors, or one by a picture (it may hide nothing).
    if d.has(b"Mask")
        || d.get(b"SMaskInData")
            .ok()
            .and_then(num)
            .is_some_and(|v| v != 0.0)
    {
        return None;
    }
    let smask = match d.get(b"SMask") {
        Ok(s) => Some(s.as_reference().ok()?),
        Err(_) => None,
    };
    let filters = filters_of(doc, d)?;
    let image_mask = truthy(b"ImageMask");
    let (colors, ink) = if image_mask {
        (Some(Colors::Gray), false)
    } else {
        match d.get(b"ColorSpace") {
            Ok(cs) => {
                let (c, ink) = colors_of(doc, cs)?;
                (Some(c), ink)
            }
            // A JPEG or JPEG 2000 tells its own.
            Err(_) => (None, false),
        }
    };
    let int = |k: &[u8]| d.get(k).ok().and_then(|v| deref(doc, v).as_i64().ok());
    let bits = if image_mask {
        1
    } else {
        int(b"BitsPerComponent").unwrap_or(8)
    };
    // A `Decode` that changes nothing is fine; one value the other way
    // round turns the picture over (black for white).
    let mut invert = ink;
    if let Ok(dec) = d.get(b"Decode") {
        let dec: Vec<f64> = deref(doc, dec)
            .as_array()
            .ok()?
            .iter()
            .filter_map(|o| num(deref(doc, o)))
            .collect();
        let n = colors.map_or(dec.len() / 2, Colors::channels);
        let plain: Vec<f64> = if colors == Some(Colors::Indexed) {
            vec![0.0, ((1i64 << bits.clamp(1, 8)) - 1) as f64]
        } else {
            [0.0, 1.0].repeat(n)
        };
        if dec == plain {
        } else if n == 1 && colors != Some(Colors::Indexed) && dec == [1.0, 0.0] {
            invert = !invert;
        } else {
            return None;
        }
    }
    let (last, before) = filters
        .split_last()
        .map_or((None, &[][..]), |(l, b)| (Some(l), b));
    let codec = match last.map(Vec::as_slice) {
        Some(b"DCTDecode") => Some(Codec::Jpeg),
        Some(b"JPXDecode") => Some(Codec::Jpx),
        Some(b"JBIG2Decode") => Some(Codec::Jbig2),
        Some(b"CCITTFaxDecode") => Some(Codec::Ccitt),
        _ => None,
    };
    if let Some(codec) = codec {
        if !before.iter().all(|f| plain_filter(f)) {
            return None;
        }
        // Black and white: one value of one bit.
        let bilevel = matches!(codec, Codec::Jbig2 | Codec::Ccitt);
        if bilevel && !matches!(colors, Some(Colors::Gray) | None) {
            return None;
        }
        // The JPEG's own colors would come the other way round.
        if codec == Codec::Jpeg && invert && colors != Some(Colors::Gray) {
            return None;
        }
        return Some((
            Pic::Coded {
                codec,
                pre: before.len(),
                colors,
                invert,
            },
            smask,
        ));
    }
    if !filters.iter().all(|f| plain_filter(f)) {
        return None;
    }
    let (width, height) = (int(b"Width")?, int(b"Height")?);
    let colors = colors?;
    let bits_ok = match colors {
        Colors::Indexed => matches!(bits, 1 | 2 | 4 | 8),
        Colors::Gray => matches!(bits, 1 | 2 | 4 | 8 | 16),
        Colors::Rgb => matches!(bits, 8 | 16),
        Colors::Cmyk => bits == 8,
    };
    let ok = (1..=30_000).contains(&width) && (1..=30_000).contains(&height) && bits_ok;
    ok.then_some((
        Pic::Pixels {
            width: width as u32,
            height: height as u32,
            colors,
            bits: bits as u8,
            invert,
        },
        smask,
    ))
}

/// A transformation: `[a b c d e f]`, as PDF writes them.
type Matrix = [f64; 6];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `m` then `t` (PDF's `m × t`).
fn times(m: &Matrix, t: &Matrix) -> Matrix {
    [
        m[0] * t[0] + m[1] * t[2],
        m[0] * t[1] + m[1] * t[3],
        m[2] * t[0] + m[3] * t[2],
        m[2] * t[1] + m[3] * t[3],
        m[4] * t[0] + m[5] * t[2] + t[4],
        m[4] * t[1] + m[5] * t[3] + t[5],
    ]
}

fn matrix_of(doc: &lopdf::Document, ops: &[lopdf::Object]) -> Option<Matrix> {
    let v: Vec<f64> = ops.iter().filter_map(|o| num(deref(doc, o))).collect();
    v.try_into().ok()
}

/// What a page's content draws, in order.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Drawn {
    /// The picture at this place in [`Seen::pictures`].
    Picture(usize),
    /// A filled or stroked shape, a shading or visible text.
    Shape,
}

/// A picture drawn, and where.
struct Shown {
    src: Src,
    ctm: Matrix,
    width: u64,
    height: u64,
    /// The fill color was black (what an image mask is painted with).
    black: bool,
}

impl Shown {
    fn pixels(&self) -> u64 {
        self.width * self.height
    }
}

/// What a page's content draws.
#[derive(Default)]
struct Seen {
    events: Vec<Drawn>,
    pictures: Vec<Shown>,
}

/// The drawing state that matters here.
#[derive(Clone, Copy)]
struct State {
    ctm: Matrix,
    /// Text rendering mode 3 or 7: no ink.
    invisible: bool,
    /// The fill color is black.
    black: bool,
}

/// Forms inside forms, at most.
const MAX_FORMS: usize = 8;

fn size_of(doc: &lopdf::Document, d: &lopdf::Dictionary) -> (u64, u64) {
    let side = |k: &[u8]| {
        d.get(k)
            .ok()
            .and_then(|v| deref(doc, v).as_i64().ok())
            .unwrap_or(0)
            .max(0) as u64
    };
    (side(b"Width"), side(b"Height"))
}

/// A fill color's operands, black or not (gray 0, RGB 0 0 0, CMYK 0 0 0 1).
fn is_black(ops: &[lopdf::Object]) -> bool {
    let v: Vec<f64> = ops.iter().filter_map(num).collect();
    match v.as_slice() {
        [g] => *g == 0.0,
        [r, g, b] => *r == 0.0 && *g == 0.0 && *b == 0.0,
        [c, m, y, k] => *c == 0.0 && *m == 0.0 && *y == 0.0 && *k == 1.0,
        _ => false,
    }
}

/// Goes through drawing operations (a page's or a form's), `find` giving
/// the XObject a name stands for. Errors for what cannot be followed.
fn look(
    doc: &lopdf::Document,
    ops: &[lopdf::content::Operation],
    find: &dyn Fn(&[u8]) -> Option<lopdf::ObjectId>,
    depth: usize,
    start: State,
    seen: &mut Seen,
) -> Option<()> {
    let mut st = start;
    let mut saved = Vec::new();
    for op in ops {
        match op.operator.as_str() {
            "q" => saved.push(st),
            "Q" => {
                if let Some(s) = saved.pop() {
                    st = s;
                }
            }
            "cm" => {
                let m = matrix_of(doc, &op.operands)?;
                st.ctm = times(&m, &st.ctm);
            }
            "Tr" => {
                let mode = op.operands.first().and_then(num);
                st.invisible = matches!(mode, Some(m) if m == 3.0 || m == 7.0);
            }
            "g" | "rg" | "k" | "sc" | "scn" => st.black = is_black(&op.operands),
            // A new color space starts at black.
            "cs" => st.black = true,
            "Tj" | "TJ" | "'" | "\"" if !st.invisible => seen.events.push(Drawn::Shape),
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "sh" => {
                seen.events.push(Drawn::Shape);
            }
            "BI" => {
                // Read only when not compressed: others come without data.
                let Some(lopdf::Object::Stream(s)) = op.operands.first() else {
                    return None;
                };
                let s = full_keys(s);
                let (width, height) = size_of(doc, &s.dict);
                seen.events.push(Drawn::Picture(seen.pictures.len()));
                seen.pictures.push(Shown {
                    src: Src::Inline(Box::new(s)),
                    ctm: st.ctm,
                    width,
                    height,
                    black: st.black,
                });
            }
            "Do" => {
                let name = op.operands.first().and_then(|o| o.as_name().ok())?;
                let id = find(name)?;
                let stream = doc.get_object(id).and_then(lopdf::Object::as_stream).ok()?;
                match stream.dict.get(b"Subtype").and_then(|t| t.as_name()) {
                    Ok(b"Image") => {
                        let (width, height) = size_of(doc, &stream.dict);
                        seen.events.push(Drawn::Picture(seen.pictures.len()));
                        seen.pictures.push(Shown {
                            src: Src::Obj(id),
                            ctm: st.ctm,
                            width,
                            height,
                            black: st.black,
                        });
                    }
                    Ok(b"Form") if depth < MAX_FORMS => {
                        let content = stream.decompressed_content_with_limit(64 << 20).ok()?;
                        let inner = lopdf::content::Content::decode(&content).ok()?;
                        // Its own resources, else those of where it is.
                        let own = stream
                            .dict
                            .get(b"Resources")
                            .ok()
                            .and_then(|r| deref(doc, r).as_dict().ok());
                        let in_form = |n: &[u8]| -> Option<lopdf::ObjectId> {
                            let x = own?.get(b"XObject").ok()?;
                            deref(doc, x)
                                .as_dict()
                                .ok()?
                                .get(n)
                                .and_then(lopdf::Object::as_reference)
                                .ok()
                        };
                        let find_here = |n: &[u8]| {
                            if own.is_some() { in_form(n) } else { find(n) }
                        };
                        let m = match stream.dict.get(b"Matrix") {
                            Ok(m) => matrix_of(doc, deref(doc, m).as_array().ok()?)?,
                            Err(_) => IDENTITY,
                        };
                        let inside = State {
                            ctm: times(&m, &st.ctm),
                            ..st
                        };
                        look(doc, &inner.operations, &find_here, depth + 1, inside, seen)?;
                    }
                    _ => return None,
                }
            }
            _ => {}
        }
    }
    Some(())
}

/// The direction of `(x, y)` (PDF's: `y` up) on a page turned `turns`
/// quarters clockwise; `None` when slanted.
fn dir_of(x: f64, y: f64, turns: i64) -> Option<Dir> {
    let (mut x, mut y) = (x, y);
    for _ in 0..turns.rem_euclid(4) {
        (x, y) = (y, -x);
    }
    if x.abs() > 100.0 * y.abs() {
        Some(if x > 0.0 { Dir::Right } else { Dir::Left })
    } else if y.abs() > 100.0 * x.abs() {
        Some(if y > 0.0 { Dir::Up } else { Dir::Down })
    } else {
        None
    }
}

/// How a picture drawn with `ctm` shows on a page turned `turns` quarters;
/// `None` when slanted or skewed.
fn turn_of(ctm: &Matrix, turns: i64) -> Option<Turn> {
    let x = dir_of(ctm[0], ctm[1], turns)?;
    let y = dir_of(ctm[2], ctm[3], turns)?;
    (x.across() != y.across()).then_some(Turn { x, y })
}

/// How much of `area` (`[x0, y0, x1, y1]`) the picture drawn with `ctm`
/// covers, from 0 to 1.
fn coverage(ctm: &Matrix, area: &[f64; 4]) -> f64 {
    let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)].map(|(u, v)| {
        (
            u * ctm[0] + v * ctm[2] + ctm[4],
            u * ctm[1] + v * ctm[3] + ctm[5],
        )
    });
    let lo = |f: fn(&(f64, f64)) -> f64| corners.iter().map(f).fold(f64::MAX, f64::min);
    let hi = |f: fn(&(f64, f64)) -> f64| corners.iter().map(f).fold(f64::MIN, f64::max);
    let w = (hi(|p| p.0).min(area[2]) - lo(|p| p.0).max(area[0])).max(0.0);
    let h = (hi(|p| p.1).min(area[3]) - lo(|p| p.1).max(area[1])).max(0.0);
    let total = (area[2] - area[0]) * (area[3] - area[1]);
    if total > 0.0 { w * h / total } else { 0.0 }
}

/// The page's picture, from what it draws: the biggest one, last drawn
/// (itself again is fine), with only decoration before it, under it —
/// shapes, text, other pictures — and then covering most of the page
/// (85%).
fn main_picture(seen: &Seen, area: Option<[f64; 4]>) -> Option<usize> {
    let pics = &seen.pictures;
    let main = (0..pics.len()).max_by_key(|i| pics[*i].pixels())?;
    let same = |i: usize| {
        i == main || matches!((&pics[i].src, &pics[main].src), (Src::Obj(a), Src::Obj(b)) if a == b)
    };
    let is_main = |e: &Drawn| matches!(e, Drawn::Picture(i) if same(*i));
    let first = seen.events.iter().position(is_main)?;
    let last = seen.events.iter().rposition(is_main)?;
    let alone = seen.events[first..=last].iter().all(is_main) && last == seen.events.len() - 1;
    let Drawn::Picture(shown) = seen.events[first] else {
        return None;
    };
    let covers = first == 0 || area.is_some_and(|a| coverage(&pics[shown].ctm, &a) >= 0.85);
    (alone && covers).then_some(shown)
}

/// Pixels per point to draw a page at: those of its sharpest picture,
/// else `size` pixels on its long side; at most 6000 pixels on it.
fn scale_for(seen: &Seen, area: Option<[f64; 4]>, size: u32) -> f32 {
    let long = area.map_or(842.0, |a| (a[2] - a[0]).max(a[3] - a[1]).max(1.0));
    let sharpest = seen
        .pictures
        .iter()
        .filter_map(|p| {
            let shown = p.ctm[0].hypot(p.ctm[1]);
            (shown > 1.0 && p.width > 0).then(|| p.width as f64 / shown)
        })
        .fold(0.0, f64::max);
    let scale = if sharpest > 0.0 {
        sharpest
    } else {
        size as f64 / long
    };
    scale.clamp(0.5, 6000.0 / long) as f32
}

/// How each page of the PDF comes out: its picture taken out when the page
/// shows one picture and draws nothing over it (no visible text, shape or
/// other picture after it, no visible annotation; what is drawn before,
/// under it, is decoration and left out when the picture covers most of
/// the page; a picture shown turned or mirrored is turned back); a white
/// page when it draws nothing; else drawn.
fn pdf_pages(doc: &lopdf::Document, size: u32) -> Vec<PdfPage> {
    let pages = doc.get_pages();
    let mut out = Vec::with_capacity(pages.len());
    for (_, page) in pages {
        let area = page_box(doc, page);
        let mut seen = Seen::default();
        let followed = doc
            .get_and_decode_page_content(page)
            .ok()
            .and_then(|content| {
                let find = |name: &[u8]| xobject(doc, page, name);
                let start = State {
                    ctm: IDENTITY,
                    invisible: false,
                    black: true,
                };
                look(doc, &content.operations, &find, 0, start, &mut seen)
            });
        let scale = scale_for(&seen, area, size);
        let drawn = PdfPage {
            src: None,
            pic: Pic::Render { scale },
            turn: UPRIGHT,
            smask: None,
            scale,
        };
        if followed.is_none() || annotated(doc, page) {
            out.push(drawn);
            continue;
        }
        if seen.events.is_empty() {
            out.push(PdfPage {
                pic: Pic::Blank,
                ..drawn
            });
            continue;
        }
        let taken = (|| {
            let i = main_picture(&seen, area)?;
            let shown = &seen.pictures[i];
            let turns = inherited(doc, page, b"Rotate")
                .and_then(num)
                .map(|r| (r / 90.0).round() as i64)
                .unwrap_or(0);
            let turn = turn_of(&shown.ctm, turns)?;
            let dict = match &shown.src {
                Src::Obj(id) => {
                    &doc.get_object(*id)
                        .and_then(lopdf::Object::as_stream)
                        .ok()?
                        .dict
                }
                Src::Inline(s) => &s.dict,
            };
            // An image mask is painted: black, on nothing but the page.
            let mask = dict
                .get(b"ImageMask")
                .and_then(lopdf::Object::as_bool)
                .unwrap_or(false);
            if mask && (!shown.black || seen.events.len() > 1) {
                return None;
            }
            let (pic, smask) = pic_of(doc, dict)?;
            Some(PdfPage {
                src: Some(shown.src.clone()),
                pic,
                turn,
                smask,
                scale,
            })
        })();
        out.push(taken.unwrap_or(drawn));
    }
    out
}

/// The size a white page takes: the most common of the others' (as shown).
fn blank_size(doc: &lopdf::Document, pages: &[PdfPage]) -> (u32, u32) {
    let mut count: HashMap<(u32, u32), usize> = HashMap::new();
    for p in pages {
        let dict = match &p.src {
            Some(Src::Obj(id)) => doc
                .get_object(*id)
                .and_then(lopdf::Object::as_stream)
                .ok()
                .map(|s| &s.dict),
            Some(Src::Inline(s)) => Some(&s.dict),
            None => None,
        };
        if let Some(d) = dict {
            let (w, h) = size_of(doc, d);
            if w > 0 && h > 0 {
                let shown = if p.turn.x.across() { (w, h) } else { (h, w) };
                *count.entry((shown.0 as u32, shown.1 as u32)).or_default() += 1;
            }
        }
    }
    count
        .into_iter()
        .max_by_key(|(size, n)| (*n, *size))
        .map(|(s, _)| s)
        .unwrap_or((1000, 1500))
}

/// Pixels of `k` bytes each, in rows, as shown upright.
fn turned(raw: &[u8], w: usize, h: usize, k: usize, turn: Turn) -> (Vec<u8>, usize, usize) {
    let (xs, ys) = (turn.x.step(), turn.y.step());
    // A pixel's place: along `x` by its column, along `y` by its height
    // from the bottom.
    let at = |i: usize, j: usize| {
        let (i, up) = (i as i64, (h - 1 - j) as i64);
        (i * xs.0 + up * ys.0, i * xs.1 + up * ys.1)
    };
    let corners = [at(0, 0), at(w - 1, 0), at(0, h - 1), at(w - 1, h - 1)];
    let x0 = corners.iter().map(|c| c.0).min().unwrap_or(0);
    let y0 = corners.iter().map(|c| c.1).min().unwrap_or(0);
    let (ow, oh) = if turn.x.across() { (w, h) } else { (h, w) };
    let mut out = vec![0u8; ow * oh * k];
    for j in 0..h {
        for i in 0..w {
            let (x, y) = at(i, j);
            let o = ((y - y0) as usize * ow + (x - x0) as usize) * k;
            let s = (j * w + i) * k;
            out[o..o + k].copy_from_slice(&raw[s..s + k]);
        }
    }
    (out, ow, oh)
}

fn write_png(
    w: u32,
    h: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    palette: Option<Vec<u8>>,
    rows: &[u8],
) -> Result<Vec<u8>, String> {
    let broken = |e: png::EncodingError| format!("archiveBroken: {e}");
    let mut png = Vec::new();
    let mut enc = png::Encoder::new(&mut png, w, h);
    enc.set_color(color);
    enc.set_depth(depth);
    if let Some(p) = palette {
        enc.set_palette(p);
    }
    let mut wr = enc.write_header().map_err(broken)?;
    wr.write_image_data(rows).map_err(broken)?;
    wr.finish().map_err(broken)?;
    Ok(png)
}

fn write_jpeg(w: u32, h: u32, gray: bool, rows: &[u8], quality: u8) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let color = if gray {
        image::ExtendedColorType::L8
    } else {
        image::ExtendedColorType::Rgb8
    };
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality)
        .encode(rows, w, h, color)
        .map_err(|e| format!("archiveBroken: {e}"))?;
    Ok(out)
}

/// Pixels as they come out of a picture.
struct Samples {
    width: usize,
    height: usize,
    /// Gray, RGB or indexed (with the palette, RGB); never CMYK.
    colors: Colors,
    palette: Option<Vec<u8>>,
    /// 1, 2, 4, 8 or 16; rows start on a byte, the first pixel in the high
    /// bits.
    bits: u8,
    data: Vec<u8>,
    /// From a lossy picture with its colors changed (CMYK): kept as a JPEG.
    lossy: bool,
}

/// A JPEG turned upright without decoding it (no loss), with
/// libjpeg-turbo; `None` when its size is not a whole number of its
/// blocks along the edges that would move.
fn jpeg_turned(jpeg: &[u8], turn: Turn) -> Option<Vec<u8>> {
    use turbojpeg::TransformOp as Op;
    let op = match (turn.x, turn.y) {
        (Dir::Right, Dir::Up) => return Some(jpeg.to_vec()),
        (Dir::Left, Dir::Up) => Op::Hflip,
        (Dir::Right, Dir::Down) => Op::Vflip,
        (Dir::Left, Dir::Down) => Op::Rot180,
        (Dir::Down, Dir::Right) => Op::Rot90,
        (Dir::Up, Dir::Left) => Op::Rot270,
        (Dir::Down, Dir::Left) => Op::Transpose,
        (Dir::Up, Dir::Right) => Op::Transverse,
        _ => return None,
    };
    let mut t = turbojpeg::Transform::default();
    t.op = op;
    t.perfect = true;
    std::panic::catch_unwind(|| turbojpeg::transform(&t, jpeg))
        .ok()?
        .ok()
        .map(|b| b.to_vec())
}

/// A JPEG's pixels: gray or RGB (CMYK made RGB).
fn decode_jpeg(jpeg: &[u8]) -> Result<Samples, String> {
    let img = image::load_from_memory_with_format(jpeg, image::ImageFormat::Jpeg)
        .map_err(|e| format!("archiveBroken: {e}"))?;
    let gray = matches!(img.color(), image::ColorType::L8 | image::ColorType::L16);
    let (width, height) = (img.width() as usize, img.height() as usize);
    let (data, colors) = if gray {
        (img.into_luma8().into_raw(), Colors::Gray)
    } else {
        (img.into_rgb8().into_raw(), Colors::Rgb)
    };
    Ok(Samples {
        width,
        height,
        colors,
        palette: None,
        bits: 8,
        data,
        lossy: false,
    })
}

/// A JPEG 2000 picture's pixels, gray or RGB, transparency laid on white.
fn decode_jpx(data: &[u8]) -> Result<Samples, String> {
    use hayro_jpeg2000::{ColorSpace, DecodeSettings, DecoderContext, Image};
    let broken = |e: String| format!("archiveBroken: {e}");
    let image =
        Image::new(data, &DecodeSettings::default()).map_err(|e| broken(format!("{e:?}")))?;
    let (width, height) = (image.width() as usize, image.height() as usize);
    let alpha = image.has_alpha();
    let space = image.color_space().clone();
    let n = space.num_channels() as usize;
    let mut ctx = DecoderContext::default();
    let decoded = image
        .decode(&mut ctx)
        .map_err(|e| broken(format!("{e:?}")))?;
    let raw = decoded.data_u8();
    let k = n + alpha as usize;
    if raw.len() < width * height * k {
        return Err(broken("picture cut short".into()));
    }
    let kind = match (&space, n) {
        (ColorSpace::Gray, _) | (ColorSpace::Icc { .. }, 1) => Colors::Gray,
        (ColorSpace::RGB, _) | (ColorSpace::Icc { .. }, 3) => Colors::Rgb,
        (ColorSpace::CMYK, _) | (ColorSpace::Icc { .. }, 4) => Colors::Cmyk,
        _ => return Err(broken("color space".into())),
    };
    let on_white = |v: u8, a: u8| (v as u32 * a as u32 / 255 + 255 - a as u32) as u8;
    let mut out = Vec::with_capacity(width * height * kind.channels().min(3));
    for px in raw.chunks(k).take(width * height) {
        let a = if alpha { px[n] } else { 255 };
        match kind {
            Colors::Gray => out.push(on_white(px[0], a)),
            Colors::Rgb => out.extend(px[..3].iter().map(|v| on_white(*v, a))),
            _ => out.extend(rgb_of_cmyk(px[0], px[1], px[2], px[3]).map(|v| on_white(v, a))),
        }
    }
    let lossy = kind == Colors::Cmyk;
    Ok(Samples {
        width,
        height,
        colors: if kind == Colors::Gray {
            Colors::Gray
        } else {
            Colors::Rgb
        },
        palette: None,
        bits: 8,
        data: out,
        lossy,
    })
}

/// Black and white rows, 1 bit each, 1 for white (as PDF gray).
struct Bilevel {
    width: usize,
    row: Vec<u8>,
    x: usize,
    data: Vec<u8>,
    rows: usize,
}

impl Bilevel {
    fn new(width: usize) -> Self {
        Bilevel {
            width,
            row: vec![0; width.div_ceil(8)],
            x: 0,
            data: Vec::new(),
            rows: 0,
        }
    }

    fn push(&mut self, white: bool) {
        if self.x < self.width && white {
            self.row[self.x / 8] |= 0x80 >> (self.x % 8);
        }
        self.x += 1;
    }

    fn end_row(&mut self) {
        self.data.extend_from_slice(&self.row);
        self.row.iter_mut().for_each(|b| *b = 0);
        self.x = 0;
        self.rows += 1;
    }
}

impl hayro_jbig2::Decoder for Bilevel {
    fn push_pixel(&mut self, black: bool) {
        self.push(!black);
    }

    fn push_pixel_chunk(&mut self, black: bool, chunk_count: u32) {
        for _ in 0..chunk_count * 8 {
            self.push(!black);
        }
    }

    fn next_line(&mut self) {
        self.end_row();
    }
}

/// A JBIG2 picture (with the globals of its `DecodeParms`).
fn decode_jbig2(
    doc: &lopdf::Document,
    d: &lopdf::Dictionary,
    data: &[u8],
) -> Result<Samples, String> {
    let broken = |e: String| format!("archiveBroken: {e}");
    let settings = hayro_jbig2::DecodeSettings::default();
    let globals = params(doc, d)
        .and_then(|p| p.get(b"JBIG2Globals").ok())
        .and_then(|g| deref(doc, g).as_stream().ok())
        .and_then(|s| s.decompressed_content_with_limit(64 << 20).ok());
    let globals = match &globals {
        Some(g) => {
            Some(hayro_jbig2::Globals::new(g, &settings).map_err(|e| broken(format!("{e:?}")))?)
        }
        None => None,
    };
    let page = hayro_jbig2::Page::new_embedded(data, globals.as_ref(), &settings)
        .map_err(|e| broken(format!("{e:?}")))?;
    let (width, height) = (page.width() as usize, page.height() as usize);
    let mut out = Bilevel::new(width);
    page.decode(&mut out)
        .map_err(|e| broken(format!("{e:?}")))?;
    if out.rows < height {
        return Err(broken("picture cut short".into()));
    }
    out.data.truncate(width.div_ceil(8) * height);
    Ok(Samples {
        width,
        height,
        colors: Colors::Gray,
        palette: None,
        bits: 1,
        data: out.data,
        lossy: false,
    })
}

/// The (last) `DecodeParms` of a picture.
fn params<'a>(doc: &'a lopdf::Document, d: &'a lopdf::Dictionary) -> Option<&'a lopdf::Dictionary> {
    match deref(doc, d.get(b"DecodeParms").ok()?) {
        lopdf::Object::Dictionary(p) => Some(p),
        lopdf::Object::Array(a) => a.last().and_then(|p| deref(doc, p).as_dict().ok()),
        _ => None,
    }
}

/// A fax picture (Group 4 only; others are drawn).
fn decode_ccitt(
    doc: &lopdf::Document,
    d: &lopdf::Dictionary,
    data: &[u8],
) -> Result<Samples, String> {
    let broken = |e: &str| format!("archiveBroken: {e}");
    let p = params(doc, d);
    let get = |k: &[u8]| p.and_then(|p| p.get(k).ok()).map(|v| deref(doc, v));
    let k = get(b"K").and_then(num).unwrap_or(0.0);
    if k >= 0.0 || get(b"EncodedByteAlign").and_then(|v| v.as_bool().ok()) == Some(true) {
        return Err(broken("fax kind"));
    }
    let width = get(b"Columns").and_then(num).unwrap_or(1728.0) as usize;
    let height = get(b"Rows")
        .and_then(num)
        .map(|r| r as usize)
        .or_else(|| d.get(b"Height").ok().and_then(num).map(|h| h as usize))
        .unwrap_or(0);
    let black_is_1 = get(b"BlackIs1").and_then(|v| v.as_bool().ok()) == Some(true);
    let mut out = Bilevel::new(width);
    fax::decoder::decode_g4(
        data.iter().copied(),
        width as u32,
        (height > 0).then_some(height as u32),
        |line| {
            for c in fax::decoder::pels(line, width as u32) {
                // The PDF's values: 0 for black, unless `BlackIs1`.
                let one = (c == fax::Color::White) != black_is_1;
                out.push(one);
            }
            out.end_row();
        },
    )
    .ok_or_else(|| broken("fax"))?;
    let height = if height > 0 { height } else { out.rows };
    if out.rows < height || height == 0 {
        return Err(broken("picture cut short"));
    }
    out.data.truncate(width.div_ceil(8) * height);
    Ok(Samples {
        width,
        height,
        colors: Colors::Gray,
        palette: None,
        bits: 1,
        data: out.data,
        lossy: false,
    })
}

/// A soft mask hides nothing: every value of it is the highest.
fn opaque(doc: &lopdf::Document, id: lopdf::ObjectId) -> bool {
    let Ok(s) = doc.get_object(id).and_then(lopdf::Object::as_stream) else {
        return false;
    };
    let (w, h) = size_of(doc, &s.dict);
    let bits = s
        .dict
        .get(b"BitsPerComponent")
        .ok()
        .and_then(num)
        .unwrap_or(8.0) as usize;
    if !matches!(bits, 1 | 2 | 4 | 8 | 16) || s.dict.has(b"Decode") {
        return false;
    }
    let row = (w as usize * bits).div_ceil(8);
    let Ok(data) = s.decompressed_content_with_limit(row * h as usize + 4096) else {
        return false;
    };
    if data.len() < row * h as usize {
        return false;
    }
    (0..h as usize).all(|r| {
        let line = &data[r * row..(r + 1) * row];
        let full = (w as usize * bits) / 8;
        let rest = (w as usize * bits) % 8;
        line[..full].iter().all(|&b| b == 0xff)
            && (rest == 0 || line[full] >> (8 - rest) == (1u8 << rest) - 1)
    })
}

/// The plain layers (`pre` first filters) taken off a picture's data.
fn unlayered(s: &lopdf::Stream, doc: &lopdf::Document, pre: usize) -> Result<Vec<u8>, String> {
    if pre == 0 {
        return Ok(s.content.clone());
    }
    let filters = filters_of(doc, &s.dict).unwrap_or_default();
    let mut d = lopdf::Dictionary::new();
    d.set(
        "Filter",
        filters[..pre]
            .iter()
            .map(|f| lopdf::Object::Name(f.clone()))
            .collect::<Vec<_>>(),
    );
    if let Ok(p) = s.dict.get(b"DecodeParms") {
        let p = deref(doc, p);
        if let lopdf::Object::Array(a) = p {
            d.set("DecodeParms", a[..pre.min(a.len())].to_vec());
        }
    }
    lopdf::Stream::new(d, s.content.clone())
        .decompressed_content_with_limit(1 << 30)
        .map_err(|e| format!("archiveBroken: {e}"))
}

/// Writes pixels as a file, turned upright: a PNG (lossless), or a JPEG
/// when they came from a lossy picture whose colors were changed.
fn write_samples(s: Samples, turn: Turn, invert: bool) -> Result<(Vec<u8>, &'static str), String> {
    let Samples {
        width: w,
        height: h,
        colors,
        palette,
        bits,
        mut data,
        lossy,
    } = s;
    if invert {
        data.iter_mut().for_each(|b| *b = !*b);
    }
    let ch = colors.channels();
    let row = (w * ch * bits as usize).div_ceil(8);
    let color = match colors {
        Colors::Indexed => png::ColorType::Indexed,
        Colors::Gray => png::ColorType::Grayscale,
        _ => png::ColorType::Rgb,
    };
    let depth = |b: u8| match b {
        1 => png::BitDepth::One,
        2 => png::BitDepth::Two,
        4 => png::BitDepth::Four,
        16 => png::BitDepth::Sixteen,
        _ => png::BitDepth::Eight,
    };
    // PDF and PNG keep pixels the same way (rows start on a byte, the
    // first pixel in the high bits; gray 0 is black): upright, they go in
    // as they are.
    let (rows, ow, oh, out_bits) = if turn == UPRIGHT {
        (data, w, h, bits)
    } else {
        let (raw, k, out_bits) = if bits >= 8 {
            (data, ch * bits as usize / 8, bits)
        } else {
            let per = 8 / bits as usize;
            let mask = ((1u16 << bits) - 1) as u8;
            let top = mask as u16;
            let mut v = Vec::with_capacity(w * h);
            for r in 0..h {
                let line = &data[r * row..(r + 1) * row];
                for x in 0..w {
                    let shift = 8 - bits as usize * (x % per + 1);
                    let n = (line[x / per] >> shift) & mask;
                    // Gray stretched to 8 bits; a palette number stays.
                    v.push(if colors == Colors::Indexed {
                        n
                    } else {
                        (n as u16 * 255 / top) as u8
                    });
                }
            }
            (v, 1, 8)
        };
        let (rows, ow, oh) = turned(&raw, w, h, k, turn);
        (rows, ow, oh, out_bits)
    };
    if lossy && out_bits == 8 && colors != Colors::Indexed {
        let jpeg = write_jpeg(ow as u32, oh as u32, colors == Colors::Gray, &rows, 95)?;
        return Ok((jpeg, "jpg"));
    }
    let png = write_png(ow as u32, oh as u32, color, depth(out_bits), palette, &rows)?;
    Ok((png, "png"))
}

/// A page's picture taken out (not drawn): its bytes and extension
/// (`blank`: the size of a white page).
fn taken_out(
    doc: &lopdf::Document,
    page: &PdfPage,
    blank: (u32, u32),
) -> Result<(Vec<u8>, &'static str), String> {
    let broken = |e: String| format!("archiveBroken: {e}");
    if page.pic == Pic::Blank {
        let (w, h) = blank;
        let row = vec![0xffu8; (w as usize).div_ceil(8)];
        let rows = row.repeat(h as usize);
        let png = write_png(
            w,
            h,
            png::ColorType::Grayscale,
            png::BitDepth::One,
            None,
            &rows,
        )?;
        return Ok((png, "png"));
    }
    if let Some(m) = page.smask
        && !opaque(doc, m)
    {
        return Err("transparent".into());
    }
    let s: &lopdf::Stream = match &page.src {
        Some(Src::Obj(id)) => doc
            .get_object(*id)
            .and_then(lopdf::Object::as_stream)
            .map_err(|e| broken(e.to_string()))?,
        Some(Src::Inline(s)) => s,
        None => return Err("drawn".into()),
    };
    match page.pic {
        Pic::Coded {
            codec,
            pre,
            colors,
            invert,
        } => {
            let data = unlayered(s, doc, pre)?;
            let samples = match codec {
                Codec::Jpeg => {
                    if !data.starts_with(&[0xff, 0xd8]) {
                        return Err(broken("not a JPEG".into()));
                    }
                    let cmyk = colors == Some(Colors::Cmyk);
                    if page.turn == UPRIGHT && !cmyk && !invert {
                        return Ok((data, "jpg"));
                    }
                    // Turned without decoding, when its size allows.
                    if !cmyk
                        && !invert
                        && let Some(j) = jpeg_turned(&data, page.turn)
                    {
                        return Ok((j, "jpg"));
                    }
                    let mut s = decode_jpeg(&data)?;
                    s.lossy = cmyk;
                    s
                }
                Codec::Jpx => decode_jpx(&data)?,
                Codec::Jbig2 => decode_jbig2(doc, &s.dict, &data)?,
                Codec::Ccitt => decode_ccitt(doc, &s.dict, &data)?,
            };
            let (w, h) = size_of(doc, &s.dict);
            if (w, h) != (samples.width as u64, samples.height as u64) {
                return Err(broken("picture size".into()));
            }
            write_samples(samples, page.turn, invert)
        }
        Pic::Pixels {
            width,
            height,
            colors,
            bits,
            invert,
        } => {
            let (w, h) = (width as usize, height as usize);
            let row = (w * colors.channels() * bits as usize).div_ceil(8);
            let mut data = if s.dict.has(b"Filter") {
                s.decompressed_content_with_limit(row * h + 4096)
                    .map_err(|e| broken(e.to_string()))?
            } else {
                s.content.clone()
            };
            if data.len() < row * h {
                return Err(broken("picture cut short".into()));
            }
            data.truncate(row * h);
            let (colors, palette, data) = match colors {
                Colors::Indexed => {
                    let p = palette(doc, s.dict.get(b"ColorSpace").ok())
                        .ok_or_else(|| broken("palette".into()))?;
                    (Colors::Indexed, Some(p), data)
                }
                // Made RGB, no loss beyond the colors.
                Colors::Cmyk => {
                    let rgb = data
                        .chunks(4)
                        .flat_map(|c| rgb_of_cmyk(c[0], c[1], c[2], c[3]))
                        .collect();
                    (Colors::Rgb, None, rgb)
                }
                c => (c, None, data),
            };
            let samples = Samples {
                width: w,
                height: h,
                colors,
                palette,
                bits,
                data,
                lossy: false,
            };
            write_samples(samples, page.turn, invert)
        }
        Pic::Blank | Pic::Render { .. } => Err("drawn".into()),
    }
}

/// Draws page `index` with hayro, `scale` pixels per point, on white: a
/// PNG (gray when it is), unless a JPEG is much smaller.
fn drawn(
    pdf: &hayro::hayro_syntax::Pdf,
    index: usize,
    scale: f32,
) -> Result<(Vec<u8>, &'static str), String> {
    let broken = |e: &str| format!("archiveBroken: {e}");
    let page = pdf.pages().get(index).ok_or_else(|| broken("no page"))?;
    let (pw, ph) = page.render_dimensions();
    // Within what a picture may be.
    let most = 16_000.0 / pw.max(ph).max(1.0);
    let scale = scale.min(most).max(0.1);
    let settings = hayro::PixmapSettings {
        x_scale: scale,
        y_scale: scale,
        bg_color: hayro::vello_cpu::color::palette::css::WHITE,
    };
    let pixmap = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        hayro::render(
            page,
            &hayro::RenderCache::new(),
            &hayro::hayro_interpret::InterpreterSettings::default(),
            &hayro::RenderSettings::default(),
            &settings,
        )
    }))
    .map_err(|_| broken("page could not be drawn"))?;
    let (w, h) = (pixmap.width() as u32, pixmap.height() as u32);
    if w == 0 || h == 0 {
        return Err(broken("empty page"));
    }
    let px = pixmap.data();
    // Gray, give or take the tints smoothing leaves on edges.
    let gray = px.iter().all(|p| {
        let (lo, hi) = (p.r.min(p.g).min(p.b), p.r.max(p.g).max(p.b));
        hi - lo <= 8
    });
    let (rows, color): (Vec<u8>, _) = if gray {
        let rows = px
            .iter()
            .map(|p| ((p.r as u16 + p.g as u16 + p.b as u16) / 3) as u8)
            .collect();
        (rows, png::ColorType::Grayscale)
    } else {
        let rows = px.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
        (rows, png::ColorType::Rgb)
    };
    // Text and line art stay sharp (and small) in a PNG; photos and
    // shading are much smaller in a JPEG.
    let png = write_png(w, h, color, png::BitDepth::Eight, None, &rows)?;
    let jpeg = write_jpeg(w, h, gray, &rows, 92)?;
    if png.len() * 2 > jpeg.len() * 3 {
        Ok((jpeg, "jpg"))
    } else {
        Ok((png, "png"))
    }
}

/// Pages drawn at once, one per core; and how many pages are worked on
/// together (their files kept in memory until written, in order).
fn workers() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get().clamp(1, 8))
}

const BATCH: usize = 16;

/// A page as a file (its bytes and extension), or why it is not.
type PageFile = Result<(Vec<u8>, &'static str), String>;

/// Draws pages (index, scale) at once, each worker with its own reader of
/// the shared bytes.
fn draw_all(data: &Arc<Vec<u8>>, jobs: &[(usize, f32)]) -> Vec<PageFile> {
    let n = workers().min(jobs.len()).max(1);
    let mut out: Vec<Option<PageFile>> = (0..jobs.len()).map(|_| None).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(&mut out);
    std::thread::scope(|scope| {
        for _ in 0..n {
            scope.spawn(|| {
                let pdf = hayro_pdf(data);
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&(index, scale)) = jobs.get(k) else {
                        break;
                    };
                    let r = match &pdf {
                        Ok(pdf) => drawn(pdf, index, scale),
                        Err(e) => Err(e.clone()),
                    };
                    results.lock().expect("no panic holds it")[k] = Some(r);
                }
            });
        }
    });
    out.into_iter()
        .map(|r| r.unwrap_or_else(|| Err("archiveBroken: page not drawn".into())))
        .collect()
}

/// Every page of the PDF at `path` as a file, in order: `each` gets its
/// name, and `drawn_pages` counts those drawn (with `opts.draw` off, a
/// page to draw is an error, `pdfDrawn`). The file is read once; pages
/// to draw are drawn several at once.
fn pdf_files(
    path: &Path,
    each: &mut dyn FnMut(&Page, &mut dyn Read) -> Result<(), String>,
    drawn_pages: &mut usize,
    opts: PdfOptions,
) -> Result<(), String> {
    let data = Arc::new(fs::read(path).map_err(|e| e.to_string())?);
    let doc = match load_pdf_from(Ok(&data), false) {
        Ok(d) => Some(d),
        Err(e) if e == "pdfLocked" => return Err(e),
        // One the PDF library cannot read: drawn whole, if hayro can.
        Err(_) => None,
    };
    let mut put = |i: usize, digits: usize, file: (Vec<u8>, &'static str)| {
        let name = format!("{:0digits$}.{}", i + 1, file.1);
        each(
            &Page {
                name,
                modified: None,
            },
            &mut file.0.as_slice(),
        )
    };
    let Some(doc) = doc else {
        let count = hayro_pdf(&data)?.pages().len();
        if count == 0 {
            return Err("archiveBroken: no pages".into());
        }
        if !opts.draw {
            return Err("pdfDrawn".into());
        }
        let digits = count.to_string().len().max(3);
        let scale = opts.size as f32 / 842.0;
        for start in (0..count).step_by(BATCH) {
            let jobs: Vec<(usize, f32)> = (start..(start + BATCH).min(count))
                .map(|i| (i, scale))
                .collect();
            for ((i, _), file) in jobs.iter().zip(draw_all(&data, &jobs)) {
                *drawn_pages += 1;
                put(*i, digits, file?)?;
            }
        }
        return Ok(());
    };
    let pages = pdf_pages(&doc, opts.size);
    if pages.is_empty() {
        return Err("archiveBroken: no pages".into());
    }
    let blank = blank_size(&doc, &pages);
    let digits = pages.len().to_string().len().max(3);
    for start in (0..pages.len()).step_by(BATCH) {
        let end = (start + BATCH).min(pages.len());
        // Taken out when it can be; drawn as the last resort.
        let mut files: Vec<Option<(Vec<u8>, &'static str)>> = Vec::with_capacity(end - start);
        let mut jobs = Vec::new();
        for (i, page) in pages.iter().enumerate().take(end).skip(start) {
            match taken_out(&doc, page, blank) {
                Ok(f) => files.push(Some(f)),
                Err(_) => {
                    if !opts.draw {
                        return Err("pdfDrawn".into());
                    }
                    jobs.push((i, page.scale));
                    files.push(None);
                }
            }
        }
        let mut drawn_files = draw_all(&data, &jobs).into_iter();
        for (k, file) in files.into_iter().enumerate() {
            let file = match file {
                Some(f) => f,
                None => {
                    *drawn_pages += 1;
                    drawn_files
                        .next()
                        .ok_or("archiveBroken: page not drawn")??
                }
            };
            put(start + k, digits, file)?;
        }
    }
    Ok(())
}

/// The title and authors a PDF, EPUB or Kindle book has, as
/// `ComicInfo.xml` values.
fn own_info(path: &Path, kind: Kind) -> Vec<(&'static str, String)> {
    let book = match kind {
        Kind::Pdf => return pdf_info(path),
        Kind::Epub => crate::ebook::epub(path),
        Kind::Kindle => crate::ebook::mobi(path),
        _ => None,
    };
    let Some(m) = book else {
        return Vec::new();
    };
    let mut v = Vec::new();
    let title = m.title.trim();
    if !title.is_empty() {
        v.push(("Title", title.to_string()));
    }
    let authors: Vec<&str> = m
        .authors
        .iter()
        .map(|a| a.trim())
        .filter(|a| !a.is_empty())
        .collect();
    if !authors.is_empty() {
        v.push(("Writer", authors.join(", ")));
    }
    v
}

/// The PDF's title and author, as `ComicInfo.xml` values.
fn pdf_info(path: &Path) -> Vec<(&'static str, String)> {
    let Ok(m) = lopdf::Document::load_metadata(path) else {
        return Vec::new();
    };
    let clean = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let mut v = Vec::new();
    if let Some(t) = clean(m.title) {
        v.push(("Title", t));
    }
    if let Some(a) = clean(m.author) {
        v.push(("Writer", a));
    }
    v
}

/// Pictures (and archives) are not made smaller by compressing them.
fn stored(name: &str) -> bool {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "jxl" | "heic" | "heif" | "zip" | "rar"
    )
}

/// A zip entry's date, as seconds since 1970 (local time).
fn zip_secs(t: zip::DateTime) -> Option<i64> {
    let d = chrono::NaiveDate::from_ymd_opt(t.year() as i32, t.month() as u32, t.day() as u32)?
        .and_hms_opt(t.hour() as u32, t.minute() as u32, t.second() as u32)?;
    Local
        .from_local_datetime(&d)
        .single()
        .map(|t| t.timestamp())
}

fn zip_time(secs: Option<i64>) -> zip::DateTime {
    let t = secs
        .and_then(|s| Local.timestamp_opt(s, 0).single())
        .unwrap_or_else(Local::now)
        .naive_local();
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

/// A name as a zip keeps it: `/` between folders, none at the start, no
/// `.` or `..` parts. Empty: nothing usable.
fn clean_name(name: &str) -> String {
    name.replace('\\', "/")
        .split('/')
        .filter(|p| !p.is_empty() && *p != "." && *p != "..")
        .collect::<Vec<_>>()
        .join("/")
}

fn is_info(name: &str) -> bool {
    name.rsplit('/')
        .next()
        .is_some_and(|f| f.eq_ignore_ascii_case("ComicInfo.xml"))
}

/// Writes the CBZ of the archive at `path` (of `kind`) to `out`, with
/// `values` written into its `ComicInfo.xml` (see
/// [`crate::inside::comic_xml`]; made when it has none) unless empty.
/// Returns how many files it holds, and how many PDF pages were drawn.
fn write_zip(
    path: &Path,
    kind: Kind,
    out: &Path,
    values: &[(&'static str, String)],
    opts: PdfOptions,
) -> Result<(usize, usize), String> {
    let mut drawn_pages = 0;
    let file = File::create(out).map_err(|e| e.to_string())?;
    let mut zout = zip::ZipWriter::new(BufWriter::new(file));
    let mut seen = std::collections::HashSet::new();
    let mut info_done = false;
    let mut count = 0;
    let err = |e: zip::result::ZipError| e.to_string();
    each_file(
        path,
        kind,
        |page, data| {
            let name = clean_name(&page.name);
            // A name twice (an archive added to): the first stays.
            if name.is_empty() || !seen.insert(name.to_lowercase()) {
                return std::io::copy(data, &mut std::io::sink())
                    .map(|_| ())
                    .map_err(|e| format!("archiveBroken: {e}"));
            }
            let method = if stored(&name) {
                zip::CompressionMethod::Stored
            } else {
                zip::CompressionMethod::Deflated
            };
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(method)
                .last_modified_time(zip_time(page.modified))
                .large_file(false);
            if !values.is_empty() && !info_done && is_info(&name) {
                let mut bytes = Vec::new();
                data.take(8 << 20)
                    .read_to_end(&mut bytes)
                    .map_err(|e| format!("archiveBroken: {e}"))?;
                let old = String::from_utf8_lossy(&bytes).into_owned();
                let new = crate::inside::comic_xml(Some(&old), values).unwrap_or(old);
                zout.start_file(name, options).map_err(err)?;
                zout.write_all(new.as_bytes()).map_err(|e| e.to_string())?;
                info_done = true;
            } else {
                zout.start_file(name, options).map_err(err)?;
                std::io::copy(data, &mut zout).map_err(|e| format!("archiveBroken: {e}"))?;
            }
            count += 1;
            Ok(())
        },
        &mut drawn_pages,
        opts,
    )?;
    if count == 0 {
        return Err("archiveBroken: empty".into());
    }
    if !values.is_empty()
        && !info_done
        && let Some(new) = crate::inside::comic_xml(None, values)
    {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip_time(None));
        zout.start_file("ComicInfo.xml", options).map_err(err)?;
        zout.write_all(new.as_bytes()).map_err(|e| e.to_string())?;
        count += 1;
    }
    let w = zout.finish().map_err(err)?;
    let file = w.into_inner().map_err(|e| e.error().to_string())?;
    if let Ok(m) = fs::metadata(path)
        && !m.is_dir()
    {
        let _ = file.set_permissions(m.permissions());
    }
    file.sync_all().map_err(|e| e.to_string())?;
    // Read back: whole, with every file.
    let back = zip::ZipArchive::new(File::open(out).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if back.len() != count {
        return Err("archiveBroken: written short".into());
    }
    Ok((count, drawn_pages))
}

/// Where the original is left: beside, named as the comic with its own
/// extension (`.azw3` for a Kindle book of the newer kind only).
pub fn beside(path: &Path, kind: Kind) -> PathBuf {
    if kind == Kind::Kindle && crate::pages::kindle_is_kf8(path) {
        return path.with_extension("azw3");
    }
    path.with_extension(kind.ext())
}

/// Converts the comic at `path` (named `.cbz`, a RAR, 7z or tar inside)
/// into a CBZ in its place, with `values` in its `ComicInfo.xml` unless empty;
/// the original stays beside or goes to the trash. Errors:
/// `comicUnsupported` (already a zip, or not an archive), `archiveLocked`
/// (needs a password), `archiveBroken: …`, `convertTaken` (a file has the
/// original's name beside), or the system's.
pub fn convert(
    path: &Path,
    values: &[(&'static str, String)],
    original: Original,
) -> Result<Done, String> {
    convert_with(path, values, original, PdfOptions::default())
}

/// [`convert`], with how PDFs are converted (`pdfDrawn` when a page would
/// have to be drawn and `pdf.draw` is off).
pub fn convert_with(
    path: &Path,
    values: &[(&'static str, String)],
    original: Original,
    pdf: PdfOptions,
) -> Result<Done, String> {
    // Renamed to `.cbz` first (see the plan): the original then takes its
    // own extension back beside.
    let named_cbz = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cbz"));
    let kind = match kind(path) {
        Some(k) if named_cbz && k != Kind::Zip => k,
        _ => return Err("comicUnsupported".into()),
    };
    let left = beside(path, kind);
    if left == path || fs::symlink_metadata(&left).is_ok() {
        return Err("convertTaken".into());
    }
    let tmp = temp_of(path);
    // A PDF's own title and author, where the values have none.
    let mut values = values.to_vec();
    if pdf.info {
        for (k, v) in own_info(path, kind) {
            if !values.iter().any(|(key, _)| *key == k) {
                values.push((k, v));
            }
        }
    }
    let drawn_pages = match write_zip(path, kind, &tmp, &values, pdf) {
        Ok((_, drawn)) => drawn,
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
    };
    // The original steps aside, then the CBZ takes its name.
    if let Err(e) = fs::rename(path, &left) {
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::rename(&left, path);
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    let mut not_trashed = None;
    let kept = match original {
        Original::Keep => Some(left),
        Original::Delete => match crate::files::to_trash(&left) {
            Ok(()) => None,
            Err(e) => {
                not_trashed = Some(e);
                Some(left)
            }
        },
    };
    Ok(Done {
        backup: Backup {
            path: path.to_path_buf(),
            fields: Vec::new(),
            pictures: None,
            cleaned: false,
            removed: Vec::new(),
            raw: Vec::new(),
            kept_text: Vec::new(),
            entry: None,
            converted: Some(Converted {
                original: kept,
                folder: kind == Kind::Folder,
            }),
        },
        not_trashed,
        drawn_pages,
    })
}

/// Undo: a kept original takes the CBZ's place again (the CBZ goes); one
/// in the trash cannot come back, the CBZ stays.
pub fn restore(path: &Path, c: &Converted) -> Result<(), String> {
    let Some(original) = &c.original else {
        return Ok(());
    };
    let Ok(m) = fs::symlink_metadata(original) else {
        return Err("originalGone".into());
    };
    // A folder does not replace a file: the CBZ goes first.
    if m.is_dir() && fs::symlink_metadata(path).is_ok_and(|p| !p.is_dir()) {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    fs::rename(original, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    fn tar_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        for (name, data) in files {
            let mut h = tar::Header::new_ustar();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_mtime(1_600_000_000);
            h.set_cksum();
            b.append_data(&mut h, name, *data).unwrap();
        }
        b.into_inner().unwrap()
    }

    fn zip_names(path: &Path) -> Vec<String> {
        let z = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
        z.file_names()
            .filter_map(Result::ok)
            .map(|n| n.into_owned())
            .collect()
    }

    fn zip_read(path: &Path, name: &str) -> Vec<u8> {
        let mut z = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
        let mut f = z.by_name(name).unwrap();
        let mut v = Vec::new();
        f.read_to_end(&mut v).unwrap();
        v
    }

    #[test]
    fn a_tar_becomes_a_zip_and_comes_back() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("Some Hero 001.cbz");
        let tar = tar_of(&[
            ("pages/01.jpg", b"\xff\xd8one"),
            ("pages/02.png", b"\x89PNGtwo"),
            ("notes.txt", b"hello"),
        ]);
        fs::write(&path, &tar).unwrap();
        assert!(convertible(&path));
        let values = vec![
            ("Series", "Some Hero".to_string()),
            ("Number", "1".to_string()),
        ];
        let done = convert(&path, &values, Original::Keep).unwrap();
        assert!(done.not_trashed.is_none());
        assert_eq!(kind(&path), Some(Kind::Zip));
        assert_eq!(
            zip_names(&path),
            ["pages/01.jpg", "pages/02.png", "notes.txt", "ComicInfo.xml"]
        );
        assert_eq!(zip_read(&path, "pages/02.png"), b"\x89PNGtwo");
        let info = String::from_utf8(zip_read(&path, "ComicInfo.xml")).unwrap();
        assert!(info.contains("<Series>Some Hero</Series>"), "{info}");
        let left = d.path().join("Some Hero 001.cbt");
        assert_eq!(fs::read(&left).unwrap(), tar);
        assert!(!temp_of(&path).exists());
        // Undo: the original is back in the comic's place.
        let c = done.backup.converted.unwrap();
        assert_eq!(c.original.as_deref(), Some(left.as_path()));
        restore(&path, &c).unwrap();
        assert_eq!(fs::read(&path).unwrap(), tar);
        assert!(!left.exists());
    }

    #[test]
    fn its_comic_info_is_updated_in_place() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("a.cbz");
        let old = "<?xml version=\"1.0\"?>\n<ComicInfo>\n  <Series>Old</Series>\n  <Summary>Kept.</Summary>\n</ComicInfo>\n";
        fs::write(
            &path,
            tar_of(&[("ComicInfo.xml", old.as_bytes()), ("1.jpg", b"x")]),
        )
        .unwrap();
        let values = vec![("Series", "New".to_string())];
        convert(&path, &values, Original::Keep).unwrap();
        assert_eq!(zip_names(&path), ["ComicInfo.xml", "1.jpg"]);
        let info = String::from_utf8(zip_read(&path, "ComicInfo.xml")).unwrap();
        assert!(info.contains("<Series>New</Series>"), "{info}");
        assert!(info.contains("<Summary>Kept.</Summary>"), "{info}");
        // Without details, it is copied as it was.
        let other = d.path().join("b.cbz");
        fs::write(&other, tar_of(&[("ComicInfo.xml", old.as_bytes())])).unwrap();
        convert(&other, &[], Original::Keep).unwrap();
        assert_eq!(zip_read(&other, "ComicInfo.xml"), old.as_bytes());
    }

    #[test]
    fn what_cannot_be_converted_is_left_alone() {
        let d = tempfile::tempdir().unwrap();
        // Already a zip.
        let z = d.path().join("z.cbz");
        {
            let mut w = zip::ZipWriter::new(File::create(&z).unwrap());
            w.start_file("1.jpg", zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(b"x").unwrap();
            w.finish().unwrap();
        }
        assert!(!convertible(&z));
        assert_eq!(
            convert(&z, &[], Original::Keep).unwrap_err(),
            "comicUnsupported"
        );
        // A damaged RAR: an error, the file as it was, nothing left beside.
        let r = d.path().join("r.cbz");
        let junk = b"Rar!\x1a\x07\x00garbage that is not a rar archive".to_vec();
        fs::write(&r, &junk).unwrap();
        assert!(convertible(&r));
        let e = convert(&r, &[], Original::Keep).unwrap_err();
        assert!(e.starts_with("archiveBroken"), "{e}");
        assert_eq!(fs::read(&r).unwrap(), junk);
        assert!(!temp_of(&r).exists());
        assert!(!d.path().join("r.cbr").exists());
        // Not yet named `.cbz`.
        let n = d.path().join("n.cbt");
        fs::write(&n, tar_of(&[("1.jpg", b"x")])).unwrap();
        assert_eq!(
            convert(&n, &[], Original::Keep).unwrap_err(),
            "comicUnsupported"
        );
        // The original's name taken beside.
        let t = d.path().join("t.cbz");
        fs::write(&t, tar_of(&[("1.jpg", b"x")])).unwrap();
        fs::write(d.path().join("t.cbt"), b"mine").unwrap();
        assert_eq!(
            convert(&t, &[], Original::Keep).unwrap_err(),
            "convertTaken"
        );
        assert_eq!(kind(&t), Some(Kind::Tar));
    }

    // From libarchive's tests (BSD license).
    const RAR4: &[u8] = include_bytes!("../../renome-archive/tests/data/rar4.rar");
    const LOCKED: &[u8] = include_bytes!("../../renome-archive/tests/data/rar4-locked.rar");

    #[test]
    fn a_rar_becomes_a_zip() {
        let d = tempfile::tempdir().unwrap();
        // A CBR renamed to `.cbz` (by the plan) before.
        let path = d.path().join("x.cbz");
        fs::write(&path, RAR4).unwrap();
        assert!(convertible(&path));
        convert(&path, &[], Original::Keep).unwrap();
        // The link and the folders are left out.
        assert_eq!(zip_names(&path), ["test.txt", "testdir/test.txt"]);
        assert_eq!(
            zip_read(&path, "testdir/test.txt"),
            b"test text document\r\n"
        );
        assert_eq!(fs::read(d.path().join("x.cbr")).unwrap(), RAR4);
        // One with a password is left as it was.
        let locked = d.path().join("l.cbz");
        fs::write(&locked, LOCKED).unwrap();
        assert_eq!(
            convert(&locked, &[], Original::Keep).unwrap_err(),
            "archiveLocked"
        );
        assert_eq!(fs::read(&locked).unwrap(), LOCKED);
        assert!(!temp_of(&locked).exists());
        assert!(!d.path().join("l.cbr").exists());
    }

    /// A test page: its content, and its images (dictionary entries, data).
    type TestPage<'a> = (&'a str, Vec<(lopdf::Dictionary, Vec<u8>)>);

    /// A PDF with a page per entry of `pages`: its content and its images
    /// (dictionary entries, data).
    fn pdf_of(path: &Path, pages: &[TestPage]) {
        use lopdf::{Object, Stream};
        let mut doc = lopdf::Document::with_version("1.5");
        let tree = doc.new_object_id();
        let mut kids = Vec::new();
        for (content, images) in pages {
            let mut xobjects = lopdf::Dictionary::new();
            for (i, (dict, data)) in images.iter().enumerate() {
                let mut d = dict.clone();
                d.set("Type", "XObject");
                d.set("Subtype", "Image");
                let id = doc.add_object(Stream::new(d, data.clone()));
                xobjects.set(format!("Im{i}"), id);
            }
            let c = doc.add_object(Stream::new(dictionary! {}, content.as_bytes().to_vec()));
            let page = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => tree,
                "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
                "Contents" => c,
                "Resources" => dictionary! { "XObject" => xobjects },
            });
            kids.push(Object::Reference(page));
        }
        let count = kids.len() as i64;
        doc.objects.insert(
            tree,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => tree });
        doc.trailer.set("Root", catalog);
        doc.save(path).unwrap();
    }

    fn jpeg() -> (lopdf::Dictionary, Vec<u8>) {
        let d = dictionary! {
            "Width" => 2, "Height" => 2, "BitsPerComponent" => 8,
            "ColorSpace" => "DeviceRGB", "Filter" => "DCTDecode",
        };
        (d, b"\xff\xd8\xff\xe0 a jpeg".to_vec())
    }

    const SHOW: &str = "q 100 0 0 100 0 0 cm /Im0 Do Q";

    #[test]
    fn a_pdf_of_pictures_becomes_a_cbz() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("Hero.pdf");
        // Gray pixels, compressed; and a black and white page, 1 bit.
        let mut gray = lopdf::Stream::new(
            dictionary! {
                "Width" => 2, "Height" => 2, "BitsPerComponent" => 8, "ColorSpace" => "DeviceGray",
            },
            vec![0, 64, 128, 255],
        );
        gray.compress().unwrap();
        let bw = dictionary! {
            "Width" => 3, "Height" => 1, "BitsPerComponent" => 1, "ColorSpace" => "DeviceGray",
        };
        let indexed = dictionary! {
            "Width" => 3, "Height" => 1, "BitsPerComponent" => 1,
            "ColorSpace" => vec![
                "Indexed".into(), "DeviceRGB".into(), 1.into(),
                lopdf::Object::string_literal(vec![255u8, 0, 0, 0, 0, 255]),
            ],
        };
        pdf_of(
            &path,
            &[
                (SHOW, vec![jpeg()]),
                // Invisible text over the picture (text recognition).
                (
                    "q /Im0 Do Q BT 3 Tr (hidden) Tj ET",
                    vec![(gray.dict.clone(), gray.content.clone())],
                ),
                (SHOW, vec![(bw, vec![0b1010_0000])]),
                // Two colors of a palette, 1 bit each.
                (SHOW, vec![(indexed, vec![0b0100_0000])]),
            ],
        );
        assert!(convertible(&path));
        let cbz = d.path().join("Hero.cbz");
        fs::rename(&path, &cbz).unwrap();
        let values = vec![("Series", "Hero".to_string())];
        convert(&cbz, &values, Original::Keep).unwrap();
        assert_eq!(
            zip_names(&cbz),
            ["001.jpg", "002.png", "003.png", "004.png", "ComicInfo.xml"]
        );
        let png = image::load_from_memory(&zip_read(&cbz, "004.png"))
            .unwrap()
            .to_rgb8();
        assert_eq!(png.as_raw(), &[255, 0, 0, 0, 0, 255, 255, 0, 0]);
        assert_eq!(zip_read(&cbz, "001.jpg"), b"\xff\xd8\xff\xe0 a jpeg");
        let png = image::load_from_memory(&zip_read(&cbz, "002.png"))
            .unwrap()
            .to_luma8();
        assert_eq!(png.as_raw(), &[0, 64, 128, 255]);
        let png = image::load_from_memory(&zip_read(&cbz, "003.png"))
            .unwrap()
            .to_luma8();
        assert_eq!(png.as_raw(), &[255, 0, 255]);
        assert_eq!(kind(&d.path().join("Hero.pdf")), Some(Kind::Pdf));
    }

    /// A one-page PDF whose picture is inside two forms, the inner one
    /// drawing `inner` (with the picture as `/Im1`).
    fn pdf_in_forms(path: &Path, inner: &str) {
        use lopdf::{Object, Stream};
        let mut doc = lopdf::Document::with_version("1.5");
        let tree = doc.new_object_id();
        let (mut d, data) = jpeg();
        d.set("Type", "XObject");
        d.set("Subtype", "Image");
        let img = doc.add_object(Stream::new(d, data));
        let form = |doc: &mut lopdf::Document, content: &str, xobjects: lopdf::Dictionary| {
            doc.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject", "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
                    "Resources" => dictionary! { "XObject" => xobjects },
                },
                content.as_bytes().to_vec(),
            ))
        };
        let f2 = form(&mut doc, inner, dictionary! { "Im1" => img });
        let f1 = form(&mut doc, "q /Fm1 Do Q", dictionary! { "Fm1" => f2 });
        let c = doc.add_object(Stream::new(dictionary! {}, b"q /Fm1 Do Q".to_vec()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => tree,
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
            "Contents" => c,
            "Resources" => dictionary! { "XObject" => dictionary! { "Fm1" => f1 } },
        });
        doc.objects.insert(
            tree,
            Object::Dictionary(
                dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 },
            ),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => tree });
        doc.trailer.set("Root", catalog);
        doc.save(path).unwrap();
    }

    #[test]
    fn a_picture_inside_forms_is_found() {
        let d = tempfile::tempdir().unwrap();
        let ok = d.path().join("ok.pdf");
        pdf_in_forms(&ok, "q 100 0 0 100 0 0 cm /Im1 Do Q");
        assert!(convertible(&ok));
        let cbz = d.path().join("ok.cbz");
        fs::rename(&ok, &cbz).unwrap();
        convert(&cbz, &[], Original::Keep).unwrap();
        assert_eq!(zip_names(&cbz), ["001.jpg"]);
        // A form drawing more than the picture.
        let lines = d.path().join("lines.pdf");
        pdf_in_forms(&lines, "/Im1 Do 0 0 m 9 9 l S");
        assert_eq!(drawn_count(&lines), 1);
    }

    #[test]
    fn decoration_under_the_picture_is_left_out() {
        let d = tempfile::tempdir().unwrap();
        let (mut big, data) = jpeg();
        big.set("Width", 1000);
        big.set("Height", 1500);
        let (icon, icon_data) = jpeg();
        // A web page printed to PDF: a background, an icon, a card, then
        // the picture.
        let web = "0 0 100 100 re f /Im1 Do 0 0 m 9 0 l 9 9 l h f* BT (menu) Tj ET \
                   q 100 0 0 100 0 0 cm /Im0 Do Q 0 0 9 9 re W n";
        let ok = d.path().join("web.pdf");
        pdf_of(
            &ok,
            &[(
                web,
                vec![
                    (big.clone(), data.clone()),
                    (icon.clone(), icon_data.clone()),
                ],
            )],
        );
        assert!(convertible(&ok));
        let cbz = d.path().join("web.cbz");
        fs::rename(&ok, &cbz).unwrap();
        convert(&cbz, &[], Original::Keep).unwrap();
        assert_eq!(zip_names(&cbz), ["001.jpg"]);
        assert_eq!(zip_read(&cbz, "001.jpg"), data);
        // Anything over it is not left out: the page is drawn.
        for (name, content) in [
            ("shape.pdf", "/Im0 Do 0 0 9 9 re f"),
            ("icon.pdf", "/Im0 Do /Im1 Do"),
        ] {
            let p = d.path().join(name);
            pdf_of(
                &p,
                &[(
                    content,
                    vec![
                        (big.clone(), data.clone()),
                        (icon.clone(), icon_data.clone()),
                    ],
                )],
            );
            assert_eq!(drawn_count(&p), 1, "{name}");
        }
        // Something under it, the picture small on the page.
        let p = d.path().join("small.pdf");
        pdf_of(
            &p,
            &[(
                "0 0 100 100 re f q 50 0 0 50 0 0 cm /Im0 Do Q",
                vec![(big.clone(), data.clone())],
            )],
        );
        assert_eq!(drawn_count(&p), 1);
        // A big picture under it, which it covers: decoration too.
        let (mut half, _) = jpeg();
        half.set("Width", 500);
        half.set("Height", 750);
        let p = d.path().join("half.pdf");
        pdf_of(
            &p,
            &[(
                "/Im1 Do q 100 0 0 100 0 0 cm /Im0 Do Q",
                vec![(big, data.clone()), (half, data)],
            )],
        );
        assert_eq!(drawn_count(&p), 0);
    }

    /// The pages of a converted PDF, all taken out (none drawn).
    fn converted(d: &Path, name: &str, pages: &[TestPage]) -> PathBuf {
        let pdf = d.join(format!("{name}.pdf"));
        pdf_of(&pdf, pages);
        assert_eq!(drawn_count(&pdf), 0, "{name}");
        pdf.with_extension("cbz")
    }

    /// Converts the PDF at `pdf` (it goes); how many pages were drawn.
    fn drawn_count(pdf: &Path) -> usize {
        assert!(convertible(pdf), "{}", pdf.display());
        let cbz = pdf.with_extension("cbz");
        fs::rename(pdf, &cbz).unwrap();
        let done = convert(&cbz, &[], Original::Keep).unwrap();
        // The original, beside, is not needed.
        let _ = fs::remove_file(pdf);
        done.drawn_pages
    }

    fn gray(width: i64, height: i64, bits: i64, data: Vec<u8>) -> (lopdf::Dictionary, Vec<u8>) {
        let d = dictionary! {
            "Width" => width, "Height" => height, "BitsPerComponent" => bits,
            "ColorSpace" => "DeviceGray",
        };
        (d, data)
    }

    #[test]
    fn a_turned_picture_is_turned_back() {
        let d = tempfile::tempdir().unwrap();
        // 3 by 2: 1 2 3 over 4 5 6, shown with its rows going up.
        let pic = gray(3, 2, 8, vec![1, 2, 3, 4, 5, 6]);
        let cbz = converted(
            d.path(),
            "turned",
            &[("q 0 1 -1 0 100 0 cm /Im0 Do Q", vec![pic])],
        );
        let png = image::load_from_memory(&zip_read(&cbz, "001.png"))
            .unwrap()
            .to_luma8();
        assert_eq!((png.width(), png.height()), (2, 3));
        assert_eq!(png.as_raw(), &[3, 6, 2, 5, 1, 4]);
        // A JPEG shown upside down becomes a PNG.
        let mut buf = Vec::new();
        image::GrayImage::from_raw(16, 1, [vec![0u8; 8], vec![255u8; 8]].concat())
            .unwrap()
            .write_to(
                &mut std::io::Cursor::new(&mut buf),
                image::ImageFormat::Jpeg,
            )
            .unwrap();
        let (mut j, _) = jpeg();
        j.set("ColorSpace", "DeviceGray");
        j.set("Width", 16);
        j.set("Height", 1);
        let cbz = converted(
            d.path(),
            "upside",
            &[("q -100 0 0 -100 100 100 cm /Im0 Do Q", vec![(j, buf)])],
        );
        assert_eq!(zip_names(&cbz), ["001.png"]);
        let png = image::load_from_memory(&zip_read(&cbz, "001.png"))
            .unwrap()
            .to_luma8();
        let px = png.as_raw();
        assert!(px[0] > 200 && px[15] < 50, "{px:?}");
        // One of whole blocks is turned as a JPEG, without decoding it:
        // black on top, white below, shown upside down.
        let rows: Vec<u8> = (0..16)
            .flat_map(|y| [if y < 8 { 0u8 } else { 255 }; 16])
            .collect();
        let mut buf = Vec::new();
        image::GrayImage::from_raw(16, 16, rows)
            .unwrap()
            .write_to(
                &mut std::io::Cursor::new(&mut buf),
                image::ImageFormat::Jpeg,
            )
            .unwrap();
        let (mut j, _) = jpeg();
        j.set("ColorSpace", "DeviceGray");
        j.set("Width", 16);
        j.set("Height", 16);
        let cbz = converted(
            d.path(),
            "blocks",
            &[("q -100 0 0 -100 100 100 cm /Im0 Do Q", vec![(j, buf)])],
        );
        assert_eq!(zip_names(&cbz), ["001.jpg"]);
        let img = image::load_from_memory(&zip_read(&cbz, "001.jpg"))
            .unwrap()
            .to_luma8();
        let px = img.as_raw();
        assert!(px[0] > 200 && px[16 * 15] < 50, "{px:?}");
    }

    #[test]
    fn more_pictures_are_taken_out() {
        let d = tempfile::tempdir().unwrap();
        // A blank page between two: white, as big as they are.
        let cbz = converted(
            d.path(),
            "blank",
            &[(SHOW, vec![jpeg()]), ("", vec![]), (SHOW, vec![jpeg()])],
        );
        assert_eq!(zip_names(&cbz), ["001.jpg", "002.png", "003.jpg"]);
        let white = image::load_from_memory(&zip_read(&cbz, "002.png"))
            .unwrap()
            .to_luma8();
        assert_eq!((white.width(), white.height()), (2, 2));
        assert!(white.as_raw().iter().all(|&p| p == 255));
        // Black and white written with 1 for black; a `Decode` changing
        // nothing; 16 bits.
        let (mut inv, _) = gray(3, 1, 1, vec![]);
        inv.set("Decode", vec![1.into(), 0.into()]);
        let (mut plain, _) = gray(2, 1, 8, vec![]);
        plain.set("Decode", vec![0.into(), 1.into()]);
        let deep = gray(2, 1, 16, vec![0, 0, 0xff, 0xff]);
        let cbz = converted(
            d.path(),
            "kinds",
            &[
                (SHOW, vec![(inv, vec![0b1010_0000])]),
                (SHOW, vec![(plain, vec![7, 9])]),
                (SHOW, vec![deep]),
            ],
        );
        let png = image::load_from_memory(&zip_read(&cbz, "001.png"))
            .unwrap()
            .to_luma8();
        assert_eq!(png.as_raw(), &[0, 255, 0]);
        let png = image::load_from_memory(&zip_read(&cbz, "002.png"))
            .unwrap()
            .to_luma8();
        assert_eq!(png.as_raw(), &[7, 9]);
        let png = image::load_from_memory(&zip_read(&cbz, "003.png"))
            .unwrap()
            .to_luma16();
        assert_eq!(png.as_raw(), &[0, 0xffff]);
        // A JPEG compressed again.
        let (mut j, _) = jpeg();
        let data = [b"\xff\xd8\xff\xe0".to_vec(), vec![b'a'; 500]].concat();
        let mut s = lopdf::Stream::new(dictionary! {}, data.clone());
        s.compress().unwrap();
        assert!(s.dict.has(b"Filter"));
        j.set(
            "Filter",
            vec![
                lopdf::Object::Name(b"FlateDecode".to_vec()),
                lopdf::Object::Name(b"DCTDecode".to_vec()),
            ],
        );
        let cbz = converted(d.path(), "twice", &[(SHOW, vec![(j, s.content)])]);
        assert_eq!(zip_read(&cbz, "001.jpg"), data);
        // Text in rendering mode 7 (no ink); a picture in the content.
        let cbz = converted(
            d.path(),
            "inline",
            &[
                ("q /Im0 Do Q BT 7 Tr (ocr) Tj ET", vec![jpeg()]),
                (
                    "q 100 0 0 100 0 0 cm BI /W 2 /H 1 /BPC 8 /CS /G ID \x00\x7f EI Q",
                    vec![],
                ),
            ],
        );
        assert_eq!(zip_names(&cbz), ["001.jpg", "002.png"]);
        let png = image::load_from_memory(&zip_read(&cbz, "002.png"))
            .unwrap()
            .to_luma8();
        assert_eq!(png.as_raw(), &[0, 127]);
    }

    #[test]
    fn hidden_things_are_not_missed() {
        let d = tempfile::tempdir().unwrap();
        // Invisible text inside `q … Q`, visible text after it.
        let p = d.path().join("restored.pdf");
        pdf_of(
            &p,
            &[("q /Im0 Do Q q 3 Tr Q BT (visible) Tj ET", vec![jpeg()])],
        );
        assert_eq!(drawn_count(&p), 1);
        // A stamp over the page.
        let p = d.path().join("stamp.pdf");
        pdf_of(&p, &[(SHOW, vec![jpeg()])]);
        let mut doc = lopdf::Document::load(&p).unwrap();
        let page = *doc.get_pages().values().next().unwrap();
        let look = doc.add_object(lopdf::Stream::new(dictionary! {}, b"".to_vec()));
        let stamp = doc.add_object(dictionary! {
            "Type" => "Annot", "Subtype" => "Stamp",
            "Rect" => vec![0.into(), 0.into(), 50.into(), 50.into()],
            "AP" => dictionary! { "N" => look },
        });
        doc.get_dictionary_mut(page)
            .unwrap()
            .set("Annots", vec![lopdf::Object::Reference(stamp)]);
        doc.save(&p).unwrap();
        assert_eq!(drawn_count(&p), 1);
        // A slanted picture.
        let p = d.path().join("slanted.pdf");
        pdf_of(&p, &[("q 70 70 -70 70 50 0 cm /Im0 Do Q", vec![jpeg()])]);
        assert_eq!(drawn_count(&p), 1);
        // Only blank pages: white ones.
        let cbz = converted(d.path(), "empty", &[("", vec![])]);
        assert_eq!(zip_names(&cbz), ["001.png"]);
    }

    #[test]
    fn drawn_pages_are_drawn() {
        let d = tempfile::tempdir().unwrap();
        let count = |name: &str, pages: &[TestPage]| {
            let p = d.path().join(name);
            pdf_of(&p, pages);
            drawn_count(&p)
        };
        assert_eq!(count("text.pdf", &[("BT (Hi) Tj ET", vec![])]), 1);
        assert_eq!(
            count("over.pdf", &[("/Im0 Do BT (Hi) Tj ET", vec![jpeg()])]),
            1
        );
        assert_eq!(
            count("two.pdf", &[("/Im0 Do /Im1 Do", vec![jpeg(), jpeg()])]),
            1
        );
        assert_eq!(
            count("lines.pdf", &[("/Im0 Do 0 0 m 9 9 l S", vec![jpeg()])]),
            1
        );
        // Only the drawn page is drawn.
        assert_eq!(
            count(
                "mixed.pdf",
                &[(SHOW, vec![jpeg()]), ("0 0 9 9 re f", vec![])]
            ),
            1
        );
        // Pictures that cannot be read (made up here) are drawn as the
        // page shows them.
        let (mut cmyk, data) = jpeg();
        cmyk.set("ColorSpace", "DeviceCMYK");
        assert_eq!(count("cmyk.pdf", &[(SHOW, vec![(cmyk, data.clone())])]), 1);
        let (mut jpx, _) = jpeg();
        jpx.set("Filter", "JPXDecode");
        assert_eq!(count("jpx.pdf", &[(SHOW, vec![(jpx, data)])]), 1);
        // A drawn page comes out as a picture of the page.
        let p = d.path().join("lines.cbz");
        assert_eq!(zip_names(&p).len(), 1);
    }

    #[test]
    fn other_pictures_are_taken_out() {
        let d = tempfile::tempdir().unwrap();
        // CMYK pixels, made RGB; one ink, the other way round; a picture
        // in ASCIIHex.
        let cmyk = dictionary! {
            "Width" => 2, "Height" => 1, "BitsPerComponent" => 8, "ColorSpace" => "DeviceCMYK",
        };
        let ink = dictionary! {
            "Width" => 2, "Height" => 1, "BitsPerComponent" => 8,
            "ColorSpace" => vec![
                "Separation".into(), "Black".into(), "DeviceGray".into(),
                lopdf::Object::Null,
            ],
        };
        let (mut hex, _) = gray(2, 1, 8, vec![]);
        hex.set("Filter", "ASCIIHexDecode");
        let cbz = converted(
            d.path(),
            "colors",
            &[
                (SHOW, vec![(cmyk, vec![0, 0, 0, 0, 0, 0, 0, 255])]),
                (SHOW, vec![(ink, vec![0, 255])]),
                (SHOW, vec![(hex, b"10ff>".to_vec())]),
            ],
        );
        let png = image::load_from_memory(&zip_read(&cbz, "001.png"))
            .unwrap()
            .to_rgb8();
        assert_eq!(png.as_raw(), &[255, 255, 255, 0, 0, 0]);
        let png = image::load_from_memory(&zip_read(&cbz, "002.png"))
            .unwrap()
            .to_luma8();
        assert_eq!(png.as_raw(), &[255, 0]);
        let png = image::load_from_memory(&zip_read(&cbz, "003.png"))
            .unwrap()
            .to_luma8();
        assert_eq!(png.as_raw(), &[0x10, 0xff]);
        // An image mask painted black; one painted red is drawn.
        let mask = dictionary! { "Width" => 3, "Height" => 1, "ImageMask" => true };
        let cbz = converted(
            d.path(),
            "mask",
            &[(SHOW, vec![(mask.clone(), vec![0b0100_0000])])],
        );
        let png = image::load_from_memory(&zip_read(&cbz, "001.png"))
            .unwrap()
            .to_luma8();
        assert_eq!(png.as_raw(), &[0, 255, 0]);
        let p = d.path().join("red.pdf");
        pdf_of(
            &p,
            &[(
                "1 0 0 rg q 100 0 0 100 0 0 cm /Im0 Do Q",
                vec![(mask, vec![0b0100_0000])],
            )],
        );
        assert_eq!(drawn_count(&p), 1);
        // Fax (Group 4): black, white, black.
        let mut enc = fax::encoder::Encoder::new(fax::VecWriter::new());
        use fax::Color::{Black, White};
        enc.encode_line([Black, White, Black].into_iter(), 3)
            .unwrap();
        let g4 = enc.finish().unwrap().finish();
        let fx = dictionary! {
            "Width" => 3, "Height" => 1, "BitsPerComponent" => 1, "ColorSpace" => "DeviceGray",
            "Filter" => "CCITTFaxDecode",
            "DecodeParms" => dictionary! { "K" => -1, "Columns" => 3, "Rows" => 1 },
        };
        let cbz = converted(d.path(), "fax", &[(SHOW, vec![(fx, g4)])]);
        let png = image::load_from_memory(&zip_read(&cbz, "001.png"))
            .unwrap()
            .to_luma8();
        assert_eq!(png.as_raw(), &[0, 255, 0]);
    }

    #[test]
    fn jpeg_2000_is_decoded_without_loss() {
        // Made for these tests, lossless: gray 0 64 128 255 on two rows,
        // and two RGB pixels.
        let gray_jp2 = include_bytes!("../tests/fixtures/gray.jp2").to_vec();
        let color_jp2 = include_bytes!("../tests/fixtures/color.jp2").to_vec();
        let jpx = |w: i64, h: i64| {
            dictionary! { "Width" => w, "Height" => h, "BitsPerComponent" => 8, "Filter" => "JPXDecode" }
        };
        let d = tempfile::tempdir().unwrap();
        let cbz = converted(
            d.path(),
            "jpx",
            &[
                (SHOW, vec![(jpx(4, 2), gray_jp2)]),
                (SHOW, vec![(jpx(2, 1), color_jp2)]),
            ],
        );
        assert_eq!(zip_names(&cbz), ["001.png", "002.png"]);
        let png = image::load_from_memory(&zip_read(&cbz, "001.png"))
            .unwrap()
            .to_luma8();
        assert_eq!(png.as_raw(), &[0, 64, 128, 255, 0, 64, 128, 255]);
        let png = image::load_from_memory(&zip_read(&cbz, "002.png"))
            .unwrap()
            .to_rgb8();
        assert_eq!(png.as_raw(), &[255, 255, 0, 0, 0, 255]);
    }

    #[test]
    fn a_soft_mask_that_hides_nothing_is_ignored() {
        let d = tempfile::tempdir().unwrap();
        for (name, alpha, drawn) in [("opaque", vec![255, 255], 0), ("see", vec![255, 0], 1)] {
            let p = d.path().join(format!("{name}.pdf"));
            pdf_of(&p, &[(SHOW, vec![gray(2, 1, 8, vec![10, 20])])]);
            let mut doc = lopdf::Document::load(&p).unwrap();
            let m = doc.add_object(lopdf::Stream::new(
                dictionary! {
                    "Type" => "XObject", "Subtype" => "Image", "Width" => 2, "Height" => 1,
                    "BitsPerComponent" => 8, "ColorSpace" => "DeviceGray",
                },
                alpha,
            ));
            let ids: Vec<_> = doc.objects.keys().copied().collect();
            for id in ids {
                if let Ok(s) = doc.get_object_mut(id).and_then(|o| o.as_stream_mut())
                    && s.dict.get(b"Width").ok().and_then(num) == Some(2.0)
                    && id != m
                {
                    s.dict.set("SMask", m);
                }
            }
            doc.save(&p).unwrap();
            assert_eq!(drawn_count(&p), drawn, "{name}");
        }
    }

    #[test]
    fn pdf_options_are_followed() {
        let d = tempfile::tempdir().unwrap();
        // Not drawing: a PDF with a page to draw is left as it was.
        let p = d.path().join("text.pdf");
        pdf_of(&p, &[(SHOW, vec![jpeg()]), ("BT (Hi) Tj ET", vec![])]);
        assert_eq!(pdf_check(&p), Some(1));
        let cbz = d.path().join("text.cbz");
        fs::rename(&p, &cbz).unwrap();
        let before = fs::read(&cbz).unwrap();
        let no_draw = PdfOptions {
            draw: false,
            ..PdfOptions::default()
        };
        assert_eq!(
            convert_with(&cbz, &[], Original::Keep, no_draw).unwrap_err(),
            "pdfDrawn"
        );
        assert_eq!(fs::read(&cbz).unwrap(), before);
        assert!(!temp_of(&cbz).exists() && !p.exists());
        // Many pages drawn at once come out in order, at the size asked
        // for (no pictures: the long side).
        let pages: Vec<TestPage> = (0..20).map(|_| ("BT (Hi) Tj ET", vec![])).collect();
        let p = d.path().join("many.pdf");
        pdf_of(&p, &pages);
        let cbz = d.path().join("many.cbz");
        fs::rename(&p, &cbz).unwrap();
        let small = PdfOptions {
            size: 400,
            ..PdfOptions::default()
        };
        let done = convert_with(&cbz, &[], Original::Keep, small).unwrap();
        assert_eq!(done.drawn_pages, 20);
        let names = zip_names(&cbz);
        assert_eq!(names.len(), 20);
        for (i, n) in names.iter().enumerate() {
            assert!(n.starts_with(&format!("{:03}.", i + 1)), "{n}");
        }
        let first = image::load_from_memory(&zip_read(&cbz, &names[0])).unwrap();
        assert_eq!(first.width().max(first.height()), 400);
        // The PDF's own title and author, when asked for.
        let p = d.path().join("titled.pdf");
        pdf_of(&p, &[(SHOW, vec![jpeg()])]);
        let mut doc = lopdf::Document::load(&p).unwrap();
        let info = doc.add_object(dictionary! {
            "Title" => lopdf::Object::string_literal("A Title"),
            "Author" => lopdf::Object::string_literal("Someone"),
        });
        doc.trailer.set("Info", info);
        doc.save(&p).unwrap();
        let cbz = d.path().join("titled.cbz");
        fs::rename(&p, &cbz).unwrap();
        let with_info = PdfOptions {
            info: true,
            ..PdfOptions::default()
        };
        let values = vec![("Series", "Hero".to_string())];
        convert_with(&cbz, &values, Original::Keep, with_info).unwrap();
        let xml = String::from_utf8(zip_read(&cbz, "ComicInfo.xml")).unwrap();
        assert!(xml.contains("<Title>A Title</Title>"), "{xml}");
        assert!(xml.contains("<Writer>Someone</Writer>"), "{xml}");
        assert!(xml.contains("<Series>Hero</Series>"), "{xml}");
    }

    #[test]
    fn an_epub_of_pictures_becomes_a_cbz() {
        use crate::pages::tests::{epub_of, page};
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("Um Quadrinho.epub");
        let one: &[u8] = b"\xff\xd8\xff\xe0one";
        let two: &[u8] = b"\x89PNG\r\n\x1a\ntwo";
        let (p1, p2) = (page("../Images/1.jpeg"), page("../Images/2.png"));
        epub_of(
            &path,
            &[("Text/a.xhtml", &p1), ("Text/b.xhtml", &p2)],
            &[("Images/2.png", two), ("Images/1.jpeg", one)],
            "",
        );
        let epub = fs::read(&path).unwrap();
        assert_eq!(kind(&path), Some(Kind::Epub));
        assert_eq!(check(&path), Some(0));
        // Renamed `.cbz` first by the plan.
        let cbz = d.path().join("Um Quadrinho.cbz");
        fs::rename(&path, &cbz).unwrap();
        assert!(convertible(&cbz));
        let opts = PdfOptions {
            info: true,
            ..Default::default()
        };
        let values = vec![("Number", "1".to_string())];
        let done = convert_with(&cbz, &values, Original::Keep, opts).unwrap();
        assert_eq!(done.drawn_pages, 0);
        assert_eq!(zip_names(&cbz), ["001.jpg", "002.png", "ComicInfo.xml"]);
        assert_eq!(zip_read(&cbz, "001.jpg"), one);
        assert_eq!(zip_read(&cbz, "002.png"), two);
        let xml = String::from_utf8(zip_read(&cbz, "ComicInfo.xml")).unwrap();
        assert!(xml.contains("<Title>Um Quadrinho</Title>"), "{xml}");
        assert!(xml.contains("<Writer>Fulano de Tal</Writer>"), "{xml}");
        assert_eq!(fs::read(d.path().join("Um Quadrinho.epub")).unwrap(), epub);
        // A book of text is not one.
        let book = d.path().join("livro.epub");
        let text = "<html><body><p>Era uma vez um reino muito, muito distante.</p></body></html>";
        epub_of(&book, &[("a.xhtml", text)], &[], "");
        assert_eq!(check(&book), None);
        assert!(!convertible(&book));
    }

    #[test]
    fn a_kindle_comic_becomes_a_cbz() {
        use crate::pages::tests::kindle_of;
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("k.cbz");
        let a: &[u8] = b"\xff\xd8\xff\xe0aaaa";
        let b: &[u8] = b"\x89PNG\r\n\x1a\nbbbb";
        let text = r#"<html><body><img src="kindle:embed:0002"/><img src="kindle:embed:0001"/></body></html>"#;
        kindle_of(&path, text, &[a, b], true, false);
        assert_eq!(kind(&path), Some(Kind::Kindle));
        assert!(convertible(&path));
        convert(&path, &[], Original::Keep).unwrap();
        assert_eq!(zip_names(&path), ["001.png", "002.jpg"]);
        assert_eq!(zip_read(&path, "001.png"), b);
        assert_eq!(zip_read(&path, "002.jpg"), a);
        // The newer kind alone is an AZW3.
        assert!(d.path().join("k.azw3").exists());
    }

    #[test]
    fn a_folder_of_pictures_becomes_a_cbz_and_comes_back() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("Some Hero 001");
        fs::create_dir_all(dir.join("Cap 2")).unwrap();
        for (name, data) in [
            ("10.jpg", &b"ten"[..]),
            ("2.jpg", b"two"),
            ("1.png", b"one"),
            (
                "ComicInfo.xml",
                b"<ComicInfo><Series>Old</Series></ComicInfo>",
            ),
            ("Thumbs.db", b"junk"),
            (".hidden", b"junk"),
            ("Cap 2/3.jpg", b"three"),
        ] {
            fs::write(dir.join(name), data).unwrap();
        }
        assert_eq!(kind(&dir), Some(Kind::Folder));
        assert_eq!(check(&dir), Some(0));
        // Renamed `Name.cbz` first by the plan.
        let path = d.path().join("Some Hero 001.cbz");
        fs::rename(&dir, &path).unwrap();
        assert!(convertible(&path));
        let values = vec![("Series", "Some Hero".to_string())];
        let done = convert(&path, &values, Original::Keep).unwrap();
        assert!(path.is_file());
        assert_eq!(
            zip_names(&path),
            ["1.png", "2.jpg", "10.jpg", "Cap 2/3.jpg", "ComicInfo.xml"]
        );
        let xml = String::from_utf8(zip_read(&path, "ComicInfo.xml")).unwrap();
        assert!(xml.contains("<Series>Some Hero</Series>"), "{xml}");
        assert!(dir.join("Thumbs.db").exists());
        let c = done.backup.converted.unwrap();
        assert!(c.folder);
        assert_eq!(c.original.as_deref(), Some(dir.as_path()));
        // Undo: the folder takes the CBZ's place.
        restore(&path, &c).unwrap();
        assert!(path.is_dir() && !dir.exists());
        assert_eq!(fs::read(path.join("Cap 2/3.jpg")).unwrap(), b"three");
        // Anything but pictures: not a comic.
        fs::write(path.join("video.mp4"), b"x").unwrap();
        assert!(!convertible(&path));
        let empty = d.path().join("vazia");
        fs::create_dir(&empty).unwrap();
        assert_eq!(check(&empty), None);
    }

    #[test]
    fn names_sort_as_read() {
        let mut v = vec!["p10.jpg", "p2.jpg", "P1.jpg", "p02b.jpg", "a.jpg"];
        v.sort_by(|a, b| natural(a, b));
        assert_eq!(v, ["a.jpg", "P1.jpg", "p2.jpg", "p02b.jpg", "p10.jpg"]);
    }

    /// The comic EPUBs and Kindle books found on this computer (set
    /// `RENOME_EBOOKS` to a file listing their paths): none breaks, and
    /// the books of text are left alone.
    #[test]
    fn real_ebooks() {
        let Ok(list) = std::env::var("RENOME_EBOOKS") else {
            return;
        };
        for line in fs::read_to_string(list).unwrap().lines() {
            let p = Path::new(line);
            let k = kind(p);
            let c = std::panic::catch_unwind(|| check(p)).expect(line);
            println!("{k:?}\t{c:?}\t{line}");
        }
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("\\a\\b.jpg"), "a/b.jpg");
        assert_eq!(clean_name("/../x/./y.png"), "x/y.png");
        assert_eq!(clean_name("./"), "");
    }
}
