//! Books made into PDF: the book is written as one HTML page, ready to be
//! printed (A5 pages, each chapter on a new one, the reader's hyphenation
//! and justified text), which the app has the system's web view print to
//! a PDF file; the PDF then gets the book's details.

use std::path::{Path, PathBuf};

use super::xhtml::{esc, esc_attr};
use super::{Book, Source, Target};
use crate::convert::Original;

/// A book ready to be printed.
#[derive(Debug, Clone)]
pub struct PdfJob {
    /// The page to print.
    pub html: PathBuf,
    /// Where the PDF is printed (beside the book, hidden).
    pub pdf: PathBuf,
    /// The folder of the page and its pictures (removed after).
    pub dir: PathBuf,
    /// Where the original steps aside.
    pub left: PathBuf,
    pub title: String,
    pub authors: Vec<String>,
    pub language: String,
}

const PRINT_CSS: &str = "
@page { size: A5; margin: 16mm 14mm 18mm 14mm; }
html, body { margin: 0; padding: 0; }
body { font-family: serif; font-size: 11pt; line-height: 1.4; text-align: justify; hyphens: auto; -webkit-hyphens: auto; orphans: 2; widows: 2; }
.chapter { break-before: page; page-break-before: always; }
.chapter:first-child { break-before: auto; page-break-before: auto; }
h1, h2, h3, h4, h5, h6 { break-after: avoid; page-break-after: avoid; text-align: center; hyphens: manual; }
img { max-width: 100%; max-height: 165mm; break-inside: avoid; page-break-inside: avoid; }
.chapter > :first-child, .chapter > :first-child > :first-child { break-before: auto !important; page-break-before: auto !important; }
.chapter > :last-child, .chapter > :last-child > :last-child { break-after: auto !important; page-break-after: auto !important; }
a { color: inherit; text-decoration: none; }
.noteref { vertical-align: super; font-size: 0.75em; line-height: 0; }
.cover-page { break-after: page; page-break-after: always; text-align: center; }
.cover-page img { max-height: 165mm; }
.fixed { break-after: page; page-break-after: always; text-align: center; margin: 0; }
.fixed img { max-width: 100%; max-height: 165mm; }
";

/// The book as one HTML page in `dir`, with its pictures beside it.
pub fn print_page(book: &Book, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for r in &book.files {
        if r.media.starts_with("image/") {
            // Big pictures smaller: a page shows no more, and the PDF
            // would hold every pixel.
            let data = smaller(&r.data, &r.media).unwrap_or_else(|| r.data.clone());
            std::fs::write(dir.join(&r.name), data).map_err(|e| e.to_string())?;
        }
    }
    let id_re = regex::Regex::new(r#"\sid="([^"]+)""#).unwrap();
    let href_re = regex::Regex::new(r##"\shref="([^"#]*)(?:#([^"]*))?""##).unwrap();
    let files: std::collections::HashMap<&str, usize> = book
        .chapters
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name.as_str(), i))
        .collect();
    let mut body = String::new();
    // A cover page, unless the first chapter shows the cover.
    if let Some(c) = &book.cover
        && !book.fixed
        && !book
            .chapters
            .first()
            .is_some_and(|f| f.body.contains(&format!("\"{c}\"")))
    {
        body.push_str(&format!(
            "<div class=\"cover-page\"><img src=\"{}\" alt=\"\"/></div>\n",
            esc_attr(c)
        ));
    }
    for (i, c) in book.chapters.iter().enumerate() {
        // Ids are made unique across the chapters, and links follow.
        let b = id_re.replace_all(&c.body, |m: &regex::Captures| {
            format!(" id=\"c{i}-{}\"", &m[1])
        });
        let b = href_re.replace_all(&b, |m: &regex::Captures| {
            let file = &m[1];
            if file.starts_with("http") || file.starts_with("mailto:") {
                return m[0].to_string();
            }
            let n = if file.is_empty() {
                Some(i)
            } else {
                files.get(file).copied()
            };
            match (n, m.get(2)) {
                (Some(n), Some(f)) => format!(" href=\"#c{n}-{}\"", f.as_str()),
                (Some(n), None) => format!(" href=\"#c{n}\""),
                (None, _) => String::new(),
            }
        });
        // A break at the chapter's start: the chapter already starts a page.
        let b = b.trim_start();
        let b = b
            .strip_prefix("<div style=\"page-break-before: always\"></div>")
            .unwrap_or(b);
        let class = if book.fixed { "fixed" } else { "chapter" };
        let lang = if c.lang.is_empty() {
            String::new()
        } else {
            format!(" lang=\"{}\"", esc_attr(&c.lang))
        };
        body.push_str(&format!(
            "<section class=\"{class}\" id=\"c{i}\"{lang}>\n{b}\n</section>\n"
        ));
    }
    let lang = if book.meta.language.is_empty() || book.meta.language == "und" {
        "en"
    } else {
        &book.meta.language
    };
    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"{l}\">\n<head>\n<meta charset=\"utf-8\"/>\n<title>{t}</title>\n<style>\n{css}\n{print}\n</style>\n</head>\n<body>\n{body}</body>\n</html>\n",
        l = esc_attr(lang),
        t = esc(&book.meta.title),
        css = book.css.replace("</style", ""),
        print = PRINT_CSS,
    );
    let page = dir.join("book.html");
    std::fs::write(&page, html).map_err(|e| e.to_string())?;
    Ok(page)
}

/// A picture bigger than 1400 pixels made smaller (as JPEG, unless it has
/// see-through parts or few colours, then PNG).
fn smaller(data: &[u8], media: &str) -> Option<Vec<u8>> {
    if media == "image/svg+xml" || media == "image/gif" {
        return None;
    }
    let img = image::load_from_memory(data).ok()?;
    if img.width().max(img.height()) <= 1400 {
        return None;
    }
    let img = img.resize(1400, 1400, image::imageops::FilterType::Triangle);
    let mut out = std::io::Cursor::new(Vec::new());
    if media == "image/png" {
        img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    } else {
        img.to_rgb8()
            .write_to(&mut out, image::ImageFormat::Jpeg)
            .ok()?;
    }
    Some(out.into_inner())
}

/// Reads the book at `path` (named `.pdf`, its content another format)
/// and writes its page to print; the app prints it to `job.pdf`.
pub fn prepare(
    path: &Path,
    values: Option<&crate::inside::BookValues>,
    work: &Path,
) -> Result<PdfJob, String> {
    let source = super::source_of(path).ok_or("bookFormat")?;
    if !Target::Pdf.takes(source) || source == Source::Other {
        return Err("bookFormat".into());
    }
    if super::drm::locked(path, source) {
        return Err("bookDrm".into());
    }
    let left = path.with_extension(super::source_ext(path, source));
    if left == path || std::fs::symlink_metadata(&left).is_ok() {
        return Err("convertTaken".into());
    }
    let mut book = super::read(path, source)?;
    if let Some(v) = values {
        super::with_values(&mut book.meta, v);
    }
    super::fill_from_name(&mut book.meta, path);
    super::drop_dead_fragments(&mut book);
    let html = print_page(&book, work)?;
    Ok(PdfJob {
        html,
        pdf: crate::tagwrite::temp_of(path),
        dir: work.to_path_buf(),
        left,
        title: book.meta.title.clone(),
        authors: book.meta.authors.clone(),
        language: book.meta.language.clone(),
    })
}

/// For tests: [`photos_as_jpeg`] on a PDF file.
#[doc(hidden)]
pub fn compact(path: &Path) -> Result<(), String> {
    let mut doc = lopdf::Document::load(path).map_err(|e| e.to_string())?;
    photos_as_jpeg(&mut doc);
    doc.save(path).map(|_| ()).map_err(|e| e.to_string())
}

/// Pictures the web view stored as plain pixels, when they are photos
/// (many colours), stored as JPEG: the PDF gets many times smaller.
/// Drawings and text in pictures stay as they are.
fn photos_as_jpeg(doc: &mut lopdf::Document) {
    // Colour profiles: how many components each has.
    let icc: std::collections::HashMap<lopdf::ObjectId, i64> = doc
        .objects
        .iter()
        .filter_map(|(id, o)| match o {
            lopdf::Object::Stream(st) => st
                .dict
                .get(b"N")
                .ok()
                .and_then(|n| n.as_i64().ok())
                .map(|n| (*id, n)),
            _ => None,
        })
        .collect();
    for obj in doc.objects.values_mut() {
        let lopdf::Object::Stream(st) = obj else {
            continue;
        };
        let d = &st.dict;
        let name = |k: &[u8]| {
            d.get(k)
                .ok()
                .and_then(|o| o.as_name().ok())
                .map(|n| n.to_vec())
        };
        if name(b"Subtype").as_deref() != Some(b"Image")
            || name(b"Filter").as_deref() != Some(b"FlateDecode")
        {
            continue;
        }
        if d.get(b"DecodeParms").is_ok()
            || d.get(b"BitsPerComponent")
                .ok()
                .and_then(|o| o.as_i64().ok())
                != Some(8)
        {
            continue;
        }
        let comps = match d.get(b"ColorSpace") {
            Ok(lopdf::Object::Name(n)) if n == b"DeviceRGB" => 3usize,
            Ok(lopdf::Object::Name(n)) if n == b"DeviceGray" => 1,
            Ok(lopdf::Object::Array(a))
                if a.first().and_then(|o| o.as_name().ok()) == Some(b"ICCBased".as_slice()) =>
            {
                match a
                    .get(1)
                    .and_then(|o| o.as_reference().ok())
                    .and_then(|r| icc.get(&r))
                {
                    Some(3) => 3,
                    Some(1) => 1,
                    _ => continue,
                }
            }
            _ => continue,
        };
        let (Some(w), Some(h)) = (
            d.get(b"Width").ok().and_then(|o| o.as_i64().ok()),
            d.get(b"Height").ok().and_then(|o| o.as_i64().ok()),
        ) else {
            continue;
        };
        if w * h < 160 * 160 {
            continue;
        }
        let Ok(raw) = st.decompressed_content() else {
            continue;
        };
        if raw.len() != (w * h) as usize * comps {
            continue;
        }
        // A photo (many colours or grey levels); drawings stay.
        if !super::photo_like(&raw, comps) {
            continue;
        }
        let Some(jpeg) = super::jpeg_of(raw, w as u32, h as u32, comps) else {
            continue;
        };
        if jpeg.len() < st.content.len() {
            st.dict
                .set("Filter", lopdf::Object::Name(b"DCTDecode".to_vec()));
            st.set_content(jpeg);
        }
    }
}

/// A PDF text: plain when ASCII, else UTF-16 with its byte order mark.
fn text_string(s: &str) -> lopdf::Object {
    if s.is_ascii() {
        return lopdf::Object::string_literal(s);
    }
    let mut b = vec![0xfe, 0xff];
    for u in s.encode_utf16() {
        b.extend_from_slice(&u.to_be_bytes());
    }
    lopdf::Object::String(b, lopdf::StringFormat::Hexadecimal)
}

/// After printing: the PDF gets the book's details and takes its place;
/// the original steps aside (or goes to the trash).
pub fn finish(path: &Path, job: &PdfJob, original: Original) -> Result<super::Done, String> {
    let _ = std::fs::remove_dir_all(&job.dir);
    if std::fs::metadata(&job.pdf).map(|m| m.len()).unwrap_or(0) < 64 {
        let _ = std::fs::remove_file(&job.pdf);
        return Err("pdfPrint".into());
    }
    if let Ok(mut doc) = lopdf::Document::load(&job.pdf) {
        let mut info = lopdf::Dictionary::new();
        info.set("Title", text_string(&job.title));
        if !job.authors.is_empty() {
            info.set("Author", text_string(&job.authors.join(", ")));
        }
        info.set("Creator", text_string("Renome"));
        let id = doc.add_object(info);
        doc.trailer.set("Info", id);
        if !job.language.is_empty()
            && let Ok(root) = doc.trailer.get(b"Root").and_then(|r| r.as_reference())
            && let Ok(cat) = doc.get_object_mut(root).and_then(|o| o.as_dict_mut())
        {
            cat.set("Lang", text_string(&job.language));
        }
        photos_as_jpeg(&mut doc);
        let _ = doc.save(&job.pdf);
    }
    super::finish(path, &job.left, &job.pdf, original)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_page_with_unique_ids() {
        let d = tempfile::tempdir().unwrap();
        let mut b = super::super::epub_out::tests::sample();
        b.chapters.push(super::super::Chapter {
            name: "c0002.xhtml".into(),
            title: "Dois".into(),
            body: "<p id=\"x\">a <a href=\"c0001.xhtml\">um</a> <a href=\"#x\">aqui</a></p>".into(),
            ..Default::default()
        });
        let page = print_page(&b, d.path()).unwrap();
        let s = std::fs::read_to_string(page).unwrap();
        assert!(s.contains("<section class=\"chapter\" id=\"c1\">"));
        assert!(s.contains("id=\"c1-x\""));
        assert!(s.contains("href=\"#c0\""));
        assert!(s.contains("href=\"#c1-x\""));
        assert!(s.contains("class=\"cover-page\""));
        assert!(d.path().join("i0001.png").exists());
    }
}
