//! Comics made into e-books for readers: a fixed-layout EPUB or a Kindle
//! AZW3, one page per picture, fit to a reader's screen. Optionally the
//! empty margins scanners leave are cut and double pages split in two (in
//! a manga's order, right page first). The pages are read as the CBZ
//! conversion reads them (CBR, CB7, CBT, PDF), so anything that becomes a
//! CBZ can become an e-book.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use image::{DynamicImage, GenericImageView, GrayImage};
use serde::{Deserialize, Serialize};

use crate::bookconv::{Book, Chapter, Target, Toc};
use crate::convert::{Kind, Original, PdfOptions};
use crate::tagwrite::temp_of;

/// The reader the pages are made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Device {
    /// The pages as they are.
    #[default]
    Original,
    /// Kindle and Kindle Paperwhite (6", 300 ppi): 1072 × 1448, grey.
    Kindle,
    /// Kindle Paperwhite 11th generation and later: 1236 × 1648, grey.
    KindlePw,
    /// Kindle Scribe: 1860 × 2480, grey.
    KindleScribe,
    /// Kindle Colorsoft: 1264 × 1680, colour.
    KindleColor,
    /// Kobo Clara and similar (6"): 1072 × 1448, grey.
    Kobo,
    /// Kobo Libra Colour and similar (7"): 1264 × 1680, colour.
    KoboColor,
    /// A tablet: 1600 × 2560, colour.
    Tablet,
}

impl Device {
    /// The screen pages are fit into, if any.
    pub fn size(self) -> Option<(u32, u32)> {
        match self {
            Device::Original => None,
            Device::Kindle | Device::Kobo => Some((1072, 1448)),
            Device::KindlePw => Some((1236, 1648)),
            Device::KindleScribe => Some((1860, 2480)),
            Device::KindleColor | Device::KoboColor => Some((1264, 1680)),
            Device::Tablet => Some((1600, 2560)),
        }
    }

    /// A grey screen: pages saved in grey (smaller, the same on it).
    pub fn grey(self) -> bool {
        matches!(
            self,
            Device::Kindle | Device::KindlePw | Device::KindleScribe | Device::Kobo
        )
    }
}

/// How a comic becomes an e-book.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EbookOptions {
    pub device: Device,
    /// The empty margins (white or black) cut away.
    pub crop: bool,
    /// Double pages (wider than tall) split in two.
    pub split: bool,
}

/// One page as written: its picture and size.
struct Out {
    data: Vec<u8>,
    w: u32,
    h: u32,
}

/// The JPEG quality of the pages, and of those whose scan was saved
/// small already.
const QUALITY: u8 = 85;
const QUALITY_LOW: u8 = 72;

/// The box of what a page shows, without the even margins around it
/// (white or black, as its edges are): `None` when there is nothing to cut
/// or the page seems empty.
fn content_box(g: &GrayImage) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = g.dimensions();
    if w < 16 || h < 16 {
        return None;
    }
    // The colour of the margins: what the edges are, if they agree.
    let mut edge = Vec::new();
    for x in 0..w {
        edge.push(g.get_pixel(x, 0)[0]);
        edge.push(g.get_pixel(x, h - 1)[0]);
    }
    for y in 0..h {
        edge.push(g.get_pixel(0, y)[0]);
        edge.push(g.get_pixel(w - 1, y)[0]);
    }
    let mean = edge.iter().map(|&v| u32::from(v)).sum::<u32>() / edge.len() as u32;
    let bg: u8 = if mean >= 200 {
        255
    } else if mean <= 55 {
        0
    } else {
        return None;
    };
    let ink = |v: u8| v.abs_diff(bg) > 48;
    // A line counts when more than a speck is drawn on it.
    let row = |y: u32| (0..w).filter(|&x| ink(g.get_pixel(x, y)[0])).count() as u32 > (w / 200).max(2);
    let col = |x: u32| (0..h).filter(|&y| ink(g.get_pixel(x, y)[0])).count() as u32 > (h / 200).max(2);
    let top = (0..h).find(|&y| row(y))?;
    let bottom = (0..h).rev().find(|&y| row(y))?;
    let left = (0..w).find(|&x| col(x))?;
    let right = (0..w).rev().find(|&x| col(x))?;
    if bottom <= top || right <= left {
        return None;
    }
    // A little air kept around the drawing.
    let (mx, my) = (w / 100, h / 100);
    let (x0, y0) = (left.saturating_sub(mx), top.saturating_sub(my));
    let (x1, y1) = ((right + mx + 1).min(w), (bottom + my + 1).min(h));
    let (cw, ch) = (x1 - x0, y1 - y0);
    // Nothing worth cutting, or so much that the page may be mostly blank
    // (a title page): left whole.
    let kept = u64::from(cw) * u64::from(ch) * 100 / (u64::from(w) * u64::from(h));
    if !(40..=97).contains(&kept) {
        return None;
    }
    Some((x0, y0, cw, ch))
}

/// A page made ready: cut, split, fit and saved as JPEG (grey or colour).
/// `first`: the cover, never split. `rtl`: halves right first.
fn prepare(data: &[u8], o: &EbookOptions, first: bool, rtl: bool) -> Result<Vec<Out>, String> {
    let img = image::load_from_memory(data).map_err(|e| format!("pageUnreadable: {e}"))?;
    let (w0, h0) = img.dimensions();
    let source_grey = matches!(img.color(), image::ColorType::L8 | image::ColorType::L16);
    let img = if o.crop {
        // Judged on a small grey copy, cut on the page itself.
        let small = img.thumbnail(800, 800).to_luma8();
        match content_box(&small) {
            Some((x, y, w, h)) => {
                let (sw, sh) = small.dimensions();
                let (fw, fh) = img.dimensions();
                let sx = |v: u32| (u64::from(v) * u64::from(fw) / u64::from(sw)) as u32;
                let sy = |v: u32| (u64::from(v) * u64::from(fh) / u64::from(sh)) as u32;
                let (x0, y0) = (sx(x), sy(y));
                let (cw, ch) = (sx(w).min(fw - x0), sy(h).min(fh - y0));
                img.crop_imm(x0, y0, cw.max(1), ch.max(1))
            }
            None => img,
        }
    } else {
        img
    };
    let (w, h) = img.dimensions();
    let halves: Vec<DynamicImage> = if o.split && !first && w * 100 > h * 115 {
        let half = w / 2;
        let left = img.crop_imm(0, 0, half, h);
        let right = img.crop_imm(half, 0, w - half, h);
        if rtl { vec![right, left] } else { vec![left, right] }
    } else {
        vec![img]
    };
    let whole = halves.len() == 1;
    halves
        .into_iter()
        .map(|p| {
            let p = match o.device.size() {
                Some((sw, sh)) if p.width() > sw || p.height() > sh => {
                    p.resize(sw, sh, image::imageops::FilterType::Lanczos3)
                }
                _ => p,
            };
            let (w, h) = p.dimensions();
            // Not cut, split nor made smaller: a JPEG stays as it was (no
            // second loss), unless a grey screen wants a colour page grey.
            let same = whole && (w, h) == (w0, h0);
            if same && data.starts_with(&[0xff, 0xd8]) && (source_grey || !o.device.grey()) {
                return Ok(Out {
                    data: data.to_vec(),
                    w,
                    h,
                });
            }
            let encode = |q: u8| -> Result<Vec<u8>, String> {
                let mut out = Vec::new();
                let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q);
                let r = if o.device.grey() {
                    p.to_luma8().write_with_encoder(enc)
                } else {
                    p.to_rgb8().write_with_encoder(enc)
                };
                r.map_err(|e| e.to_string())?;
                Ok(out)
            };
            let mut out = encode(QUALITY)?;
            // A scan saved small already would grow: its share of the
            // original's bytes (by area) is the budget, met with a lower
            // quality when it can be (its own detail is all there is).
            let share = data.len() as u64 * u64::from(w) * u64::from(h)
                / (u64::from(w0) * u64::from(h0)).max(1);
            if out.len() as u64 > share + share / 10 {
                let lower = encode(QUALITY_LOW)?;
                if lower.len() < out.len() {
                    out = lower;
                }
            }
            Ok(Out { data: out, w, h })
        })
        .collect()
}

/// The pages of a CBZ in reading order, and its `ComicInfo.xml`.
fn zip_pages(path: &Path) -> Result<(Vec<Vec<u8>>, String), String> {
    let mut z = zip::ZipArchive::new(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("archiveBroken: {e}"))?;
    let mut names: Vec<String> = z
        .file_names()
        .filter_map(Result::ok)
        .map(|n| n.into_owned())
        .collect();
    let info_name = names
        .iter()
        .find(|n| n.rsplit('/').next().is_some_and(|b| b.eq_ignore_ascii_case("ComicInfo.xml")))
        .cloned();
    names.retain(|n| crate::convert::is_page_name(n) && !n.contains("__MACOSX"));
    names.sort_by(|a, b| crate::convert::natural_order(a, b));
    let mut pages = Vec::with_capacity(names.len());
    for n in names {
        let mut f = z.by_name(&n).map_err(|e| e.to_string())?;
        let mut b = Vec::new();
        f.read_to_end(&mut b).map_err(|e| e.to_string())?;
        pages.push(b);
    }
    let mut info = String::new();
    if let Some(n) = info_name
        && let Ok(f) = z.by_name(&n)
    {
        let _ = f.take(8 << 20).read_to_string(&mut info);
    }
    Ok((pages, info))
}

/// The value of a `ComicInfo.xml` field (or of `values`, which win).
fn field(values: &[(&'static str, String)], info: &str, key: &str) -> String {
    if let Some((_, v)) = values.iter().find(|(k, v)| *k == key && !v.is_empty()) {
        return v.clone();
    }
    roxmltree::Document::parse(info.trim_start_matches('\u{feff}'))
        .ok()
        .and_then(|d| {
            d.descendants()
                .find(|n| n.has_tag_name(key))
                .and_then(|n| n.text())
                .map(|t| t.trim().to_string())
        })
        .unwrap_or_default()
}

/// What [`convert`] did.
#[derive(Debug)]
pub struct Done {
    pub backup: crate::tagwrite::Backup,
    pub not_trashed: Option<String>,
    pub pages: usize,
}

/// Makes the comic at `path` (renamed with the e-book's extension; its
/// content another kind) into an e-book in `to`'s format; the original
/// steps aside with its own extension (kept or sent to the trash).
/// `values`: details for the book (a `ComicInfo.xml`'s keys); `manga`:
/// pages turn right to left (also when its `ComicInfo.xml` says so).
pub fn convert(
    path: &Path,
    to: Target,
    o: EbookOptions,
    values: &[(&'static str, String)],
    manga: bool,
    original: Original,
    pdf: PdfOptions,
) -> Result<Done, String> {
    if !matches!(to, Target::Epub | Target::Azw3) {
        return Err("comicUnsupported".into());
    }
    let kind = crate::convert::kind(path).ok_or("comicUnsupported")?;
    if matches!(kind, Kind::Epub | Kind::Kindle | Kind::Folder) {
        return Err("comicUnsupported".into());
    }
    let left = crate::convert::beside(path, kind);
    if left == path || fs::symlink_metadata(&left).is_ok() {
        return Err("convertTaken".into());
    }
    // Anything but a zip is made a CBZ first (beside, then removed).
    let (pages, info) = if kind == Kind::Zip {
        zip_pages(path)?
    } else {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let cbz = path.with_file_name(format!(".{name}.renome-pages"));
        let r = crate::convert::write_zip(path, kind, &cbz, &[], pdf).and_then(|_| zip_pages(&cbz));
        let _ = fs::remove_file(&cbz);
        r?
    };
    if pages.is_empty() {
        return Err("comicUnsupported".into());
    }
    let rtl = manga
        || field(values, &info, "Manga").eq_ignore_ascii_case("YesAndRightToLeft");
    // The pages, a few at a time on each core.
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let chunk = pages.len().div_ceil(threads).max(1);
    let done: Vec<Result<Vec<Out>, String>> = std::thread::scope(|s| {
        let handles: Vec<_> = pages
            .chunks(chunk)
            .enumerate()
            .map(|(c, part)| {
                s.spawn(move || {
                    part.iter()
                        .enumerate()
                        .map(|(i, p)| prepare(p, &o, c * chunk + i == 0, rtl))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_else(|_| vec![Err("pageUnreadable".into())]))
            .collect()
    });
    let mut book = Book {
        fixed: true,
        rtl,
        css: crate::bookconv::BASE_CSS.into(),
        ..Default::default()
    };
    let series = field(values, &info, "Series");
    let number = field(values, &info, "Number");
    let title = field(values, &info, "Title");
    book.meta.title = match (series.is_empty(), number.is_empty(), title.is_empty()) {
        (false, false, false) => format!("{series} #{number} - {title}"),
        (false, false, true) => format!("{series} #{number}"),
        (false, true, _) => series.clone(),
        _ if !title.is_empty() => title.clone(),
        _ => path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };
    book.meta.authors = field(values, &info, "Writer")
        .split(',')
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect();
    book.meta.publisher = field(values, &info, "Publisher");
    book.meta.language = field(values, &info, "LanguageISO");
    book.meta.series = series;
    book.meta.series_index = number;
    book.meta.date = field(values, &info, "Year");
    let mut count = 0;
    for page in done {
        // A page that cannot be read is left out, as the CBZ would.
        let Ok(outs) = page else { continue };
        for out in outs {
            let Some(name) = book.add_file(out.data, "p.jpg") else { continue };
            if book.cover.is_none() {
                book.cover = Some(name.clone());
            }
            book.chapters.push(Chapter {
                name: book.next_name(),
                title: String::new(),
                body: crate::bookconv::epub_out::picture_page(&name),
                lang: String::new(),
                page: Some((out.w, out.h)),
            });
            count += 1;
        }
    }
    if count == 0 {
        return Err("comicUnsupported".into());
    }
    book.toc.push(Toc {
        title: book.meta.title.clone(),
        href: book.chapters[0].name.clone(),
        kids: vec![],
    });
    let tmp = temp_of(path);
    let written = match to {
        Target::Epub => crate::bookconv::epub_out::write(&book, &tmp),
        _ => crate::bookconv::azw3_out::write(&book, &tmp),
    };
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    let d = crate::bookconv::finish(path, &left, &tmp, original)?;
    Ok(Done {
        backup: d.backup,
        not_trashed: d.not_trashed,
        pages: count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn jpeg(img: DynamicImage) -> Vec<u8> {
        let mut b = Vec::new();
        img.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(&mut b, 90))
            .unwrap();
        b
    }

    /// A page: white margins around a dark drawing.
    fn page(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| {
            let inside = x > w / 8 && x < w - w / 8 && y > h / 10 && y < h - h / 10;
            if inside {
                image::Rgb([(x % 200) as u8, 40, (y % 200) as u8])
            } else {
                image::Rgb([255, 255, 255])
            }
        }))
    }

    fn cbz(path: &Path, pages: &[DynamicImage], info: &str) {
        let mut z = zip::ZipWriter::new(File::create(path).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        // Out of order on purpose: read in natural order.
        for (i, p) in pages.iter().enumerate().rev() {
            z.start_file(format!("p{}.jpg", i + 1), o).unwrap();
            z.write_all(&jpeg(p.clone())).unwrap();
        }
        z.start_file("ComicInfo.xml", o).unwrap();
        z.write_all(info.as_bytes()).unwrap();
        z.finish().unwrap();
    }

    #[test]
    fn margins_found() {
        let g = page(400, 600).to_luma8();
        let (x, y, w, h) = content_box(&g).unwrap();
        assert!(x > 40 && x < 52 && y > 50 && y < 62, "{x} {y}");
        assert!(w > 290 && w < 320 && h > 470 && h < 500, "{w} {h}");
        // A page all drawn: nothing cut.
        let full = DynamicImage::ImageRgb8(image::RgbImage::from_fn(300, 400, |x, _| {
            image::Rgb([(x % 255) as u8, 90, 90])
        }));
        assert!(content_box(&full.to_luma8()).is_none());
    }

    #[test]
    fn comic_made_ebooks() {
        let d = tempfile::tempdir().unwrap();
        let pages = vec![page(1600, 2400), page(3200, 2400), page(1600, 2400)];
        let info = "<?xml version=\"1.0\"?><ComicInfo><Series>Hero</Series><Number>3</Number><Writer>Ana, Rui</Writer><Manga>YesAndRightToLeft</Manga></ComicInfo>";
        for to in [Target::Epub, Target::Azw3] {
            let src = d.path().join("Hero 003.cbz");
            cbz(&src, &pages, info);
            let old = fs::read(&src).unwrap();
            let named = src.with_extension(to.ext());
            fs::rename(&src, &named).unwrap();
            let o = EbookOptions {
                device: Device::Kindle,
                crop: true,
                split: true,
            };
            let done = convert(&named, to, o, &[], false, Original::Keep, PdfOptions::default())
                .unwrap();
            // Cover, the spread in two, the last page.
            assert_eq!(done.pages, 4);
            assert_eq!(fs::read(&src).unwrap(), old, "the original stays beside");
            let book = match to {
                Target::Epub => crate::bookconv::epub_in::read(&named).unwrap(),
                _ => crate::bookconv::mobi::read(&named).unwrap(),
            };
            assert!(book.fixed, "{to:?}");
            assert_eq!(book.meta.title, "Hero #3");
            assert_eq!(book.meta.authors, ["Ana", "Rui"]);
            assert_eq!(book.chapters.len(), 4);
            // Fit to the reader, in grey.
            for f in &book.files {
                let img = image::load_from_memory(&f.data).unwrap();
                assert!(img.width() <= 1072 && img.height() <= 1448);
                assert!(matches!(img, DynamicImage::ImageLuma8(_)));
            }
            assert!(book.rtl, "{to:?}");
            crate::convert::restore(&named, done.backup.converted.as_ref().unwrap()).unwrap();
            fs::rename(&named, &src).unwrap();
            assert_eq!(fs::read(&src).unwrap(), old);
        }
    }
}
