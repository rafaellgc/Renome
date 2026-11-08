//! Books converted between formats, with EPUB in the middle: every format
//! is read into one [`Book`] (chapters of clean XHTML, pictures, a table
//! of contents and the book's details), and a [`Book`] is written as
//! EPUB (or, from it, as AZW3 or FB2). Books locked by DRM are never
//! converted: [`drm`] tells them apart and conversion refuses them.
//!
//! Like comics turned into CBZ (see [`crate::convert`]), a book to
//! convert is first renamed with the new format's extension; the new file
//! is written beside, the original takes back its own extension beside
//! it (or goes to the trash), and the new file takes the name. Undo puts
//! a kept original back.

pub mod azw3_out;
pub mod drm;
pub mod epub_in;
pub mod epub_out;
pub mod fb2;
pub mod mobi;
pub mod office;
pub mod pdf_in;
pub mod print;
pub mod text_in;
pub mod xhtml;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::convert::{Converted, Original};
use crate::tagwrite::{Backup, temp_of};

/// A book's details.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Meta {
    pub title: String,
    pub authors: Vec<String>,
    /// Translators, illustrators and the like: (name, MARC role such as
    /// `trl`, `ill`, `edt`).
    pub contributors: Vec<(String, String)>,
    /// `pt`, `en-US`…
    pub language: String,
    pub publisher: String,
    /// `2019`, `2019-05` or `2019-05-12`.
    pub date: String,
    pub isbn: String,
    pub series: String,
    pub series_index: String,
    pub description: String,
    pub subjects: Vec<String>,
    /// The identifier the book had (kept when there is no ISBN).
    pub id: String,
}

/// One file of the reading order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chapter {
    /// Its file name in the book (`c0003.xhtml`).
    pub name: String,
    pub title: String,
    /// The body's content, clean XHTML.
    pub body: String,
    /// Language of this chapter when not the book's.
    pub lang: String,
    /// A fixed-layout page: its size in pixels.
    pub page: Option<(u32, u32)>,
}

/// A picture, font or style sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct Res {
    /// Its file name in the book (`i0003.jpg`).
    pub name: String,
    pub media: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Toc {
    pub title: String,
    /// `c0003.xhtml` or `c0003.xhtml#n12`.
    pub href: String,
    pub kids: Vec<Toc>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Book {
    pub meta: Meta,
    pub chapters: Vec<Chapter>,
    pub files: Vec<Res>,
    /// The style sheet every chapter uses.
    pub css: String,
    pub toc: Vec<Toc>,
    /// The cover picture's name (one of `files`).
    pub cover: Option<String>,
    /// Pages are pictures of fixed size (a scanned book).
    pub fixed: bool,
    /// Pages turn right to left.
    pub rtl: bool,
}

/// The style every converted book starts with: plain, for the reader to
/// adjust. Each format adds what it needs.
pub const BASE_CSS: &str = "\
body { margin: 0 2%; }
p { margin: 0; text-indent: 1.5em; }
h1, h2, h3, h4, h5, h6 { text-align: center; text-indent: 0; margin: 1.5em 0 1em; page-break-after: avoid; }
h1 { font-size: 1.6em; } h2 { font-size: 1.35em; } h3 { font-size: 1.15em; } h4, h5, h6 { font-size: 1em; }
img { max-width: 100%; }
blockquote { margin: 1em 2em; }
pre { white-space: pre-wrap; font-size: 0.9em; }
table { border-collapse: collapse; margin: 1em 0; }
td, th { border: 1px solid #888; padding: 0.2em 0.4em; vertical-align: top; }
.center { text-align: center; text-indent: 0; }
.right { text-align: right; text-indent: 0; }
.noindent { text-indent: 0; }
.space { margin-top: 1em; }
.cover { text-align: center; margin: 0; padding: 0; }
.cover img { max-height: 100%; }
.note { font-size: 0.9em; }
.notes { margin-top: 2em; border-top: 1px solid #888; }
.poem { margin: 1em 0 1em 2em; }
.poem p { text-indent: 0; }
.stanza { margin-bottom: 1em; }
.epigraph { margin: 1em 0 1em 30%; font-style: italic; }
.epigraph p { text-indent: 0; }
.author { text-align: right; font-style: normal; }
.break { text-align: center; text-indent: 0; margin: 1em 0; }
";

impl Book {
    pub fn new() -> Self {
        Book {
            css: BASE_CSS.into(),
            ..Default::default()
        }
    }

    /// Adds a picture or other file; gives its name in the book. Pictures
    /// readers may not show (BMP, TIFF) become PNG.
    pub fn add_file(&mut self, data: Vec<u8>, hint: &str) -> Option<String> {
        let (data, media, ext) = match media_of(&data, hint) {
            Some((m, e)) => (data, m, e),
            None => {
                let png = to_png(&data)?;
                (png, "image/png", "png")
            }
        };
        // The same picture twice is one file.
        if let Some(r) = self.files.iter().find(|r| r.data == data) {
            return Some(r.name.clone());
        }
        let name = format!("i{:04}.{ext}", self.files.len() + 1);
        self.files.push(Res {
            name: name.clone(),
            media: media.into(),
            data,
        });
        Some(name)
    }

    /// A new chapter's file name.
    pub fn next_name(&self) -> String {
        format!("c{:04}.xhtml", self.chapters.len() + 1)
    }

    /// A table of contents made of the chapters' titles, when the format
    /// gave none.
    pub fn toc_from_chapters(&mut self) {
        if self.toc.is_empty() {
            self.toc = self
                .chapters
                .iter()
                .filter(|c| !c.title.trim().is_empty())
                .map(|c| Toc {
                    title: c.title.trim().into(),
                    href: c.name.clone(),
                    kids: vec![],
                })
                .collect();
        }
    }
}

/// The type of a picture, from its first bytes: (media type, extension).
pub fn media_of(data: &[u8], hint: &str) -> Option<(&'static str, &'static str)> {
    if data.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("image/jpeg", "jpg"))
    } else if data.starts_with(b"\x89PNG") {
        Some(("image/png", "png"))
    } else if data.starts_with(b"GIF8") {
        Some(("image/gif", "gif"))
    } else if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else if hint.ends_with(".svg") || {
        let head = String::from_utf8_lossy(&data[..data.len().min(512)]);
        head.contains("<svg")
    } {
        Some(("image/svg+xml", "svg"))
    } else if hint.ends_with(".css") {
        Some(("text/css", "css"))
    } else if hint.ends_with(".otf") || data.starts_with(b"OTTO") {
        Some(("font/otf", "otf"))
    } else if hint.ends_with(".ttf") || data.starts_with(&[0, 1, 0, 0]) {
        Some(("font/ttf", "ttf"))
    } else if hint.ends_with(".woff") || data.starts_with(b"wOFF") {
        Some(("font/woff", "woff"))
    } else if hint.ends_with(".woff2") || data.starts_with(b"wOF2") {
        Some(("font/woff2", "woff2"))
    } else {
        None
    }
}

/// Whether pixels (`comps` 1 or 3 per pixel) are a photo or shading —
/// better kept as JPEG — rather than a drawing or text with few colours:
/// many different colours, or, in a grey picture, many grey levels.
pub(crate) fn photo_like(px: &[u8], comps: usize) -> bool {
    let n = px.len() / comps.max(1);
    if n < 160 * 160 {
        return false;
    }
    let mut seen = std::collections::HashSet::new();
    let mut grey = true;
    for p in px.chunks(comps).step_by((n / 6000).max(1)) {
        if comps == 3 && (p[0].abs_diff(p[1]) > 12 || p[1].abs_diff(p[2]) > 12) {
            grey = false;
        }
        seen.insert(p.to_vec());
        if seen.len() > 600 {
            return true;
        }
    }
    grey && seen.len() > 128
}

/// Pixels as a JPEG file.
pub(crate) fn jpeg_of(px: Vec<u8>, w: u32, h: u32, comps: usize) -> Option<Vec<u8>> {
    let img: image::DynamicImage = if comps == 3 {
        image::RgbImage::from_raw(w, h, px)?.into()
    } else {
        image::GrayImage::from_raw(w, h, px)?.into()
    };
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88)
        .encode_image(&img)
        .ok()?;
    Some(out.into_inner())
}

/// A BMP, TIFF or other picture the image library reads, as PNG.
fn to_png(data: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(data).ok()?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

// ---------------------------------------------------------------- formats

/// What a book file is, from its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Epub,
    /// MOBI, AZW, AZW3, PRC, and PalmDOC.
    Kindle,
    Fb2,
    /// FB2 in a zip (`.fbz`, `.fb2.zip`).
    Fbz,
    Docx,
    Odt,
    Rtf,
    Pdf,
    Html,
    /// HTML in a zip (`.htmlz`).
    Htmlz,
    Txt,
    /// A format books come in that cannot be read (DjVu, KFX…).
    Other,
}

/// The formats books are converted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Epub,
    Azw3,
    Fb2,
    /// Written by the app (the system's web view prints it).
    Pdf,
}

impl Target {
    pub fn ext(self) -> &'static str {
        match self {
            Target::Epub => "epub",
            Target::Azw3 => "azw3",
            Target::Fb2 => "fb2",
            Target::Pdf => "pdf",
        }
    }

    pub fn from_ext(s: &str) -> Option<Target> {
        match s.to_ascii_lowercase().as_str() {
            "epub" => Some(Target::Epub),
            "azw3" => Some(Target::Azw3),
            "fb2" => Some(Target::Fb2),
            "pdf" => Some(Target::Pdf),
            _ => None,
        }
    }

    /// Whether a book in `s` becomes a new file in this format.
    pub fn takes(self, s: Source) -> bool {
        match (self, s) {
            (_, Source::Other) => false,
            (Target::Epub, Source::Epub) => false,
            (Target::Fb2, Source::Fb2) => false,
            (Target::Pdf, Source::Pdf) => false,
            // An AZW3 is only made again from another format; a MOBI
            // becomes AZW3.
            _ => true,
        }
    }
}

/// Whether a book in `s` can be read to make one in `t` (formats whose
/// reader is not there yet are left alone).
pub fn readable(s: Source, t: Target) -> bool {
    let _ = t;
    matches!(
        s,
        Source::Epub
            | Source::Fb2
            | Source::Fbz
            | Source::Docx
            | Source::Odt
            | Source::Html
            | Source::Htmlz
            | Source::Txt
            | Source::Pdf
            | Source::Kindle
    )
}

/// [`Target::takes`], for a file: a Kindle book already in KF8 is not made
/// into AZW3 again.
pub fn takes_file(t: Target, s: Source, path: &Path) -> bool {
    t.takes(s) && !(t == Target::Azw3 && s == Source::Kindle && crate::pages::kindle_is_kf8(path))
}

/// Extensions of the files the book rule converts besides its books.
pub fn is_document(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "docx" | "odt" | "rtf" | "txt" | "html" | "htm" | "xhtml" | "htmlz" | "fbz"
    )
}

/// What the file at `path` is, from its first bytes (and, for zips, the
/// names inside).
pub fn source_of(path: &Path) -> Option<Source> {
    let mut f = fs::File::open(path).ok()?;
    let mut head = [0u8; 80];
    let n = f.read(&mut head).ok()?;
    let head = &head[..n];
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if head.starts_with(b"%PDF")
        || (head.len() > 8 && head[..n.min(1024)].windows(4).any(|w| w == b"%PDF") && ext == "pdf")
    {
        return Some(Source::Pdf);
    }
    if head.len() >= 68 && (&head[60..68] == b"BOOKMOBI" || &head[60..68] == b"TEXtREAd") {
        return Some(Source::Kindle);
    }
    if head.starts_with(b"PK\x03\x04") {
        return zip_source(path);
    }
    if head.starts_with(b"{\\rtf") {
        return Some(Source::Rtf);
    }
    if head.starts_with(b"AT&TFORM")
        || head.starts_with(b"\xeaDRMION")
        || head.starts_with(b"CONT\x02")
    {
        return Some(Source::Other);
    }
    // Text: by its start, else by its name.
    let start = String::from_utf8_lossy(head).to_ascii_lowercase();
    let mut more = String::new();
    if start.contains("<?xml") || start.trim_start_matches('\u{feff}').starts_with('<') {
        let mut buf = vec![0u8; 4096];
        let _ = fs::File::open(path)
            .and_then(|mut f| f.read(&mut buf))
            .map(|n| buf.truncate(n));
        more = String::from_utf8_lossy(&buf).to_ascii_lowercase();
    }
    if more.contains("<fictionbook") {
        return Some(Source::Fb2);
    }
    if more.contains("<html") || more.contains("<!doctype html") {
        return Some(Source::Html);
    }
    match ext.as_str() {
        "txt" | "text" => Some(Source::Txt),
        "html" | "htm" | "xhtml" => Some(Source::Html),
        "fb2" => Some(Source::Fb2),
        "djvu" | "djv" | "kfx" => Some(Source::Other),
        _ => None,
    }
}

fn zip_source(path: &Path) -> Option<Source> {
    let z = zip::ZipArchive::new(fs::File::open(path).ok()?).ok()?;
    let names: Vec<String> = z
        .file_names()
        .filter_map(Result::ok)
        .map(|n| n.to_ascii_lowercase())
        .collect();
    let has = |n: &str| names.iter().any(|x| x == n);
    if has("meta-inf/container.xml") || has("mimetype") && names.iter().any(|n| n.ends_with(".opf"))
    {
        return Some(Source::Epub);
    }
    if has("word/document.xml") {
        return Some(Source::Docx);
    }
    if has("content.xml") && has("mimetype") {
        return Some(Source::Odt);
    }
    if names.iter().any(|n| n.ends_with(".fb2")) {
        return Some(Source::Fbz);
    }
    if has("index.html") || has("index.htm") {
        return Some(Source::Htmlz);
    }
    None
}

/// The extension a file in `s` takes back when it steps aside.
pub fn source_ext(path: &Path, s: Source) -> String {
    let own = path
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();
    let fits = |exts: &[&str]| exts.iter().any(|e| e.eq_ignore_ascii_case(&own));
    match s {
        Source::Epub if fits(&["epub", "kepub"]) => own,
        Source::Epub => "epub".into(),
        // What it is: an old Kindle book is not an AZW3 (the name a
        // conversion to AZW3 gave it).
        Source::Kindle => {
            let kf8 = crate::pages::kindle_is_kf8(path);
            if fits(&["mobi", "azw", "prc", "pdb"]) && !kf8 || fits(&["azw3"]) && kf8 {
                own
            } else if kf8 {
                "azw3".into()
            } else {
                "mobi".into()
            }
        }
        Source::Fb2 => "fb2".into(),
        Source::Fbz if fits(&["fbz", "zip"]) => own,
        Source::Fbz => "fbz".into(),
        Source::Docx => "docx".into(),
        Source::Odt => "odt".into(),
        Source::Rtf => "rtf".into(),
        Source::Pdf => "pdf".into(),
        Source::Html if fits(&["html", "htm", "xhtml"]) => own,
        Source::Html => "html".into(),
        Source::Htmlz => "htmlz".into(),
        Source::Txt if fits(&["txt", "text"]) => own,
        Source::Txt => "txt".into(),
        Source::Other => own,
    }
}

/// Reads the book at `path` (in format `s`).
pub fn read(path: &Path, s: Source) -> Result<Book, String> {
    let path = path.to_path_buf();
    // A broken file must never take the program down.
    std::panic::catch_unwind(move || match s {
        Source::Epub => epub_in::read(&path),
        Source::Fb2 => fb2::read(&fs::read(&path).map_err(|e| e.to_string())?),
        Source::Fbz => fb2::read_zip(&path),
        Source::Docx => office::docx(&path),
        Source::Odt => office::odt(&path),
        Source::Html => text_in::html_file(&path),
        Source::Htmlz => text_in::htmlz(&path),
        Source::Txt => text_in::txt(&fs::read(&path).map_err(|e| e.to_string())?),
        Source::Pdf => pdf_in::read(&path),
        Source::Kindle => mobi::read(&path),
        _ => Err("bookFormat".into()),
    })
    .unwrap_or_else(|_| Err("bookBroken".into()))
}

/// What the conversion gives for undo and the report.
#[derive(Debug)]
pub struct Done {
    pub backup: Backup,
    /// The original was to go to the trash but could not (the reason).
    pub not_trashed: Option<String>,
}

/// Details chosen for the book (from a catalogue or typed): they take the
/// place of the book's own.
pub fn with_values(meta: &mut Meta, v: &crate::inside::BookValues) {
    let set = |to: &mut String, from: &str| {
        if !from.trim().is_empty() {
            *to = from.trim().to_string();
        }
    };
    set(&mut meta.title, &v.title);
    if !v.authors.is_empty() {
        meta.authors = v.authors.clone();
    }
    set(&mut meta.publisher, &v.publisher);
    set(&mut meta.language, &v.language);
    set(&mut meta.isbn, &v.isbn);
    set(&mut meta.series, &v.series);
    set(&mut meta.series_index, &v.series_index);
    if !v.year.is_empty() && !meta.date.starts_with(&v.year) {
        meta.date = v.year.clone();
    }
}

/// Fills what the book does not say with what its name says.
pub(crate) fn fill_from_name(meta: &mut Meta, path: &Path) {
    if meta.title.trim().is_empty() {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        match stem.split_once(" - ") {
            Some((a, t)) if meta.authors.is_empty() => {
                meta.authors = vec![a.trim().into()];
                meta.title = t.trim().into();
            }
            _ => meta.title = stem,
        }
    }
    if meta.language.is_empty() {
        meta.language = "und".into();
    }
}

/// Converts the book at `path` (named with `target`'s extension, its
/// content still the old format) into `target` in its place; the original
/// stays beside with its own extension, or goes to the trash. `values`:
/// details to write in it. Errors: `bookUnsupported`, `bookDrm`,
/// `bookBroken`, `convertTaken`, or the system's.
pub fn convert(
    path: &Path,
    target: Target,
    original: Original,
    values: Option<&crate::inside::BookValues>,
) -> Result<Done, String> {
    let source = source_of(path).ok_or("bookFormat")?;
    if !takes_file(target, source, path) || target == Target::Pdf {
        return Err("bookFormat".into());
    }
    if drm::locked(path, source) {
        return Err("bookDrm".into());
    }
    let left = path.with_extension(source_ext(path, source));
    if left == path || fs::symlink_metadata(&left).is_ok() {
        return Err("convertTaken".into());
    }
    let mut book = read(path, source)?;
    if let Some(v) = values {
        with_values(&mut book.meta, v);
    }
    fill_from_name(&mut book.meta, path);
    drop_dead_fragments(&mut book);
    titles_from_toc(&mut book);
    let tmp = temp_of(path);
    let written = match target {
        Target::Epub => epub_out::write(&book, &tmp),
        Target::Fb2 => fb2::write(&book, &tmp),
        Target::Azw3 => azw3_out::write(&book, &tmp),
        Target::Pdf => Err("bookFormat".into()),
    };
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    finish(path, &left, &tmp, original)
}

/// The original steps aside, the new file takes its name.
pub fn finish(path: &Path, left: &Path, tmp: &Path, original: Original) -> Result<Done, String> {
    if let Err(e) = fs::rename(path, left) {
        let _ = fs::remove_file(tmp);
        return Err(e.to_string());
    }
    if let Err(e) = fs::rename(tmp, path) {
        let _ = fs::rename(left, path);
        let _ = fs::remove_file(tmp);
        return Err(e.to_string());
    }
    let mut not_trashed = None;
    let kept = match original {
        Original::Keep => Some(left.to_path_buf()),
        Original::Delete => match crate::files::to_trash(left) {
            Ok(()) => None,
            Err(e) => {
                not_trashed = Some(e);
                Some(left.to_path_buf())
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
            saved: None,
            video: None,
            entry: None,
            converted: Some(Converted {
                original: kept,
                folder: false,
            }),
        },
        not_trashed,
    })
}

/// Where the original of a converted book is left (for the preview).
pub fn beside(path: &Path) -> Option<PathBuf> {
    let s = source_of(path)?;
    Some(path.with_extension(source_ext(path, s)))
}

/// Links inside the book written `#id` point at the chapter holding that
/// id; links to ids no chapter has lose their address (a reader would
/// show an error).
pub fn fix_links(book: &mut Book) {
    use regex::Regex;
    use std::sync::LazyLock;
    static ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\sid="([^"]+)""#).unwrap());
    static HREF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r##"\shref="#([^"]*)""##).unwrap());
    let mut at: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for c in &book.chapters {
        for cap in ID.captures_iter(&c.body) {
            at.entry(cap[1].to_string())
                .or_insert_with(|| c.name.clone());
        }
    }
    for c in &mut book.chapters {
        let name = c.name.clone();
        c.body = HREF
            .replace_all(&c.body, |cap: &regex::Captures| match at.get(&cap[1]) {
                Some(file) if *file == name => format!(" href=\"#{}\"", &cap[1]),
                Some(file) => format!(" href=\"{file}#{}\"", &cap[1]),
                None => String::new(),
            })
            .into_owned();
    }
    fn toc(t: &mut [Toc], at: &std::collections::HashMap<String, String>) {
        for e in t {
            if let Some(id) = e.href.strip_prefix('#')
                && let Some(file) = at.get(id)
            {
                e.href = format!("{file}#{id}");
            }
            toc(&mut e.kids, at);
        }
    }
    toc(&mut book.toc, &at);
}

/// Chapters without a title take the one the table of contents gives
/// their file.
pub fn titles_from_toc(book: &mut Book) {
    fn walk(t: &[Toc], m: &mut std::collections::HashMap<String, String>) {
        for e in t {
            let file = e.href.split('#').next().unwrap_or_default().to_string();
            m.entry(file).or_insert_with(|| e.title.clone());
            walk(&e.kids, m);
        }
    }
    let mut m = std::collections::HashMap::new();
    walk(&book.toc, &mut m);
    for c in &mut book.chapters {
        if c.title.trim().is_empty()
            && let Some(t) = m.get(&c.name)
        {
            c.title = t.clone();
        }
    }
}

/// Links to a place that does not exist in the chapter they name lose
/// the place and keep the chapter (some books come with such links).
pub fn drop_dead_fragments(book: &mut Book) {
    use regex::Regex;
    use std::collections::{HashMap, HashSet};
    use std::sync::LazyLock;
    static ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\sid="([^"]+)""#).unwrap());
    static HREF: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r##"\shref="([^"#]*)#([^"]*)""##).unwrap());
    let ids: HashMap<String, HashSet<String>> = book
        .chapters
        .iter()
        .map(|c| {
            (
                c.name.clone(),
                ID.captures_iter(&c.body)
                    .map(|x| x[1].to_string())
                    .collect(),
            )
        })
        .collect();
    let has = |file: &str, id: &str| ids.get(file).is_some_and(|s| s.contains(id));
    for c in &mut book.chapters {
        let name = c.name.clone();
        c.body = HREF
            .replace_all(&c.body, |cap: &regex::Captures| {
                let file = if cap[1].is_empty() {
                    name.as_str()
                } else {
                    &cap[1]
                };
                if has(file, &cap[2]) || !ids.contains_key(file) {
                    cap[0].to_string()
                } else if cap[1].is_empty() {
                    String::new()
                } else {
                    format!(" href=\"{}\"", &cap[1])
                }
            })
            .into_owned();
    }
    fn toc(t: &mut [Toc], has: &dyn Fn(&str, &str) -> bool) {
        for e in t {
            if let Some((f, id)) = e.href.split_once('#')
                && !has(f, id)
            {
                e.href = f.to_string();
            }
            toc(&mut e.kids, has);
        }
    }
    toc(&mut book.toc, &has);
}

#[cfg(test)]
mod tests {
    use super::*;

    const FB2: &str = r#"<?xml version="1.0" encoding="utf-8"?><FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0"><description><title-info><author><first-name>Lia</first-name><last-name>Marta</last-name></author><book-title>Terra Seca</book-title><lang>pt</lang></title-info></description><body><section><title><p>Um</p></title><p>Texto.</p></section></body></FictionBook>"#;

    #[test]
    fn converts_and_undoes() {
        let d = tempfile::tempdir().unwrap();
        // Renamed with the new extension first, as the plan does.
        let path = d.path().join("Lia Marta - Terra Seca.epub");
        fs::write(&path, FB2).unwrap();
        assert_eq!(source_of(&path), Some(Source::Fb2));
        let values = crate::inside::BookValues {
            title: "Terra Seca".into(),
            authors: vec!["Lia Marta".into()],
            year: "2019".into(),
            language: "pt".into(),
            ..Default::default()
        };
        let done = convert(&path, Target::Epub, Original::Keep, Some(&values)).unwrap();
        assert_eq!(source_of(&path), Some(Source::Epub));
        let left = d.path().join("Lia Marta - Terra Seca.fb2");
        assert_eq!(fs::read_to_string(&left).unwrap(), FB2);
        let m = crate::ebook::read(&path);
        assert_eq!(m.title, "Terra Seca");
        assert_eq!(m.year, Some(2019));
        // Undo: the original back in the book's place.
        crate::tagwrite::restore(&done.backup, d.path()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), FB2);
        assert!(!left.exists());
        // Converting again with the original's name taken: refused.
        fs::write(&left, "x").unwrap();
        assert_eq!(
            convert(&path, Target::Epub, Original::Keep, None).unwrap_err(),
            "convertTaken"
        );
    }

    #[test]
    fn locked_books_are_refused() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("a.fb2");
        let mut z = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        use std::io::Write;
        z.start_file("mimetype", o).unwrap();
        z.write_all(b"application/epub+zip").unwrap();
        z.start_file("META-INF/container.xml", o).unwrap();
        z.write_all(b"<container/>").unwrap();
        z.start_file("META-INF/rights.xml", o).unwrap();
        z.write_all(b"<rights/>").unwrap();
        z.finish().unwrap();
        assert_eq!(source_of(&path), Some(Source::Epub));
        assert!(drm::locked(&path, Source::Epub));
        assert_eq!(
            convert(&path, Target::Fb2, Original::Keep, None).unwrap_err(),
            "bookDrm"
        );
    }
}
