//! Comics made into e-books for readers: a fixed-layout EPUB (or Kobo's
//! KEPUB), a Kindle AZW3, a CBZ or a PDF, one page per picture, fit to a
//! reader's screen. Optionally the empty margins scanners leave are cut
//! (and the page numbers with them), double pages split in two (in a
//! manga's order, right page first) or turned sideways, a wide cover
//! narrowed to its front, long strips cut into pages, small scans made
//! larger, the tones stretched for ink screens, the greys reduced to the
//! 16 ink screens show, and fine dot patterns softened on colour ink
//! screens. The pages are read as the CBZ conversion reads them (CBR,
//! CB7, CBT, PDF), so anything that becomes a CBZ can become an e-book.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use image::{DynamicImage, GenericImageView, GrayImage};
use serde::{Deserialize, Serialize};

use crate::bookconv::{Book, Chapter, Toc};
use crate::convert::{Kind, Original, PdfOptions};
use crate::tagwrite::temp_of;

/// The reader the pages are made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Device {
    /// The pages as they are.
    #[default]
    Original,
    /// Kindle (2022), Paperwhite 3 and 4, Voyage, Oasis 1: 1072 × 1448.
    Kindle,
    /// Kindle Paperwhite 5 (2021): 1236 × 1648.
    KindlePw,
    /// Kindle Paperwhite (2024): 1272 × 1696.
    KindlePw2024,
    /// Kindle Oasis 2 and 3: 1264 × 1680.
    KindleOasis,
    /// Kindle Paperwhite 1 and 2: 758 × 1024.
    KindlePwOld,
    /// Kindle 4 to 10, Touch: 600 × 800.
    KindleBasic,
    /// Kindle DX: 824 × 1000.
    KindleDx,
    /// Kindle Scribe (2022, 2024): 1860 × 2480.
    KindleScribe,
    /// Kindle Scribe (2025): 1986 × 2648.
    KindleScribe2025,
    /// Kindle Colorsoft: 1272 × 1696, colour.
    KindleColor,
    /// Kindle Scribe Colorsoft: 1986 × 2648, colour.
    KindleScribeColor,
    /// Kobo Clara HD, 2E, BW, Glo HD: 1072 × 1448.
    Kobo,
    /// Kobo Clara Colour: 1072 × 1448, colour.
    KoboClaraColor,
    /// Kobo Libra 2: 1264 × 1680.
    KoboLibra,
    /// Kobo Libra Colour: 1264 × 1680, colour.
    KoboColor,
    /// Kobo Libra H2O, Aura H2O: 1080 × 1430.
    KoboH2o,
    /// Kobo Sage, Forma: 1440 × 1920.
    KoboSage,
    /// Kobo Elipsa, Aura One: 1404 × 1872.
    KoboElipsa,
    /// Kobo Nia, Aura: 758 × 1024.
    KoboNia,
    /// reMarkable 1 and 2: 1404 × 1872.
    Remarkable,
    /// reMarkable Paper Pro: 1620 × 2160, colour.
    RemarkablePro,
    /// reMarkable Paper Pro Move: 954 × 1696, colour.
    RemarkableMove,
    /// A tablet: 1600 × 2560, colour.
    Tablet,
    /// The options' width and height, colour.
    Custom,
}

impl Device {
    /// The screen pages are fit into, if any (a custom one: the options').
    pub fn size(self) -> Option<(u32, u32)> {
        use Device::*;
        Some(match self {
            Original | Custom => return None,
            Kindle | Kobo | KoboClaraColor => (1072, 1448),
            KindlePw => (1236, 1648),
            KindlePw2024 | KindleColor => (1272, 1696),
            KindleOasis | KoboLibra | KoboColor => (1264, 1680),
            KindlePwOld | KoboNia => (758, 1024),
            KindleBasic => (600, 800),
            KindleDx => (824, 1000),
            KindleScribe => (1860, 2480),
            KindleScribe2025 | KindleScribeColor => (1986, 2648),
            KoboH2o => (1080, 1430),
            KoboSage => (1440, 1920),
            KoboElipsa | Remarkable => (1404, 1872),
            RemarkablePro => (1620, 2160),
            RemarkableMove => (954, 1696),
            Tablet => (1600, 2560),
        })
    }

    /// A grey ink screen: pages saved in grey (smaller, the same on it).
    pub fn grey(self) -> bool {
        use Device::*;
        matches!(
            self,
            Kindle
                | KindlePw
                | KindlePw2024
                | KindleOasis
                | KindlePwOld
                | KindleBasic
                | KindleDx
                | KindleScribe
                | KindleScribe2025
                | Kobo
                | KoboLibra
                | KoboH2o
                | KoboSage
                | KoboElipsa
                | KoboNia
                | Remarkable
        )
    }

    /// A colour ink screen, where fine dot patterns shimmer in colour.
    pub fn colour_ink(self) -> bool {
        use Device::*;
        matches!(
            self,
            KindleColor | KindleScribeColor | KoboClaraColor | KoboColor | RemarkablePro | RemarkableMove
        )
    }

    /// An ink screen (grey or colour), which shows pages lighter than a
    /// tablet does.
    pub fn ink(self) -> bool {
        self.grey() || self.colour_ink()
    }

    /// A reader known to show pictures in PNG badly (blank pages): the
    /// Scribes and the colour Kindles get JPEG only.
    fn png_ok(self) -> bool {
        use Device::*;
        !matches!(self, KindleScribe | KindleScribe2025 | KindleScribeColor | KindleColor)
    }
}

/// What the e-book is made as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    /// A fixed-layout EPUB.
    Epub,
    /// The same, named for Kobo's own reader (`.kepub.epub`).
    Kepub,
    /// A Kindle book.
    Azw3,
    /// A CBZ with the pages made for the reader (KOReader, older Kindles).
    Cbz,
    /// A PDF, one page per picture (reMarkable, Kindle Scribe).
    Pdf,
}

impl Format {
    /// The extension the file gets (`kepub.epub` for Kobo's).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Epub => "epub",
            Format::Kepub => "kepub.epub",
            Format::Azw3 => "azw3",
            Format::Cbz => "cbz",
            Format::Pdf => "pdf",
        }
    }

    /// The format a comic rule's `ebook` names.
    pub fn from_name(s: &str) -> Option<Format> {
        Some(match s {
            "epub" => Format::Epub,
            "kepub" => Format::Kepub,
            "azw3" => Format::Azw3,
            "cbz" => Format::Cbz,
            "pdf" => Format::Pdf,
            _ => return None,
        })
    }
}

/// What double pages (wider than tall) become.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Spread {
    /// Left as they are (fit to the screen, small).
    #[default]
    Keep,
    /// Split in two pages.
    Split,
    /// Turned sideways, whole, to read with the reader turned.
    Rotate,
    /// Turned sideways whole, then split in two.
    Both,
}

/// How a comic becomes an e-book.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EbookOptions {
    pub device: Device,
    /// The screen of a custom device.
    pub width: u32,
    pub height: u32,
    /// The empty margins (white or black) cut away.
    pub crop: bool,
    /// With the margins, a page number alone at the top or bottom.
    pub numbers: bool,
    /// Double pages split in two (older presets; see `spread`).
    pub split: bool,
    pub spread: Spread,
    /// Tones stretched so the darkest grey is black and the lightest
    /// white, and the middle tones darkened a little on ink screens.
    pub contrast: bool,
    /// Pages smaller than the screen made larger to fill it.
    pub upscale: bool,
    /// On grey screens, pages reduced to the 16 greys they show (PNG).
    pub greys16: bool,
    /// On colour ink screens, fine dot patterns softened.
    pub soften: bool,
    /// Long vertical strips (webtoons) cut into pages.
    pub webtoon: bool,
    /// The comics of one series in one folder joined into one volume
    /// (done by the app with [`join`]).
    pub join: bool,
    /// Books larger than this many megabytes made in parts (0: no limit).
    pub max_mb: u32,
}

impl EbookOptions {
    /// The screen pages are fit into.
    pub fn screen(&self) -> Option<(u32, u32)> {
        if self.device == Device::Custom {
            return (self.width >= 100 && self.height >= 100).then_some((self.width, self.height));
        }
        self.device.size()
    }

    /// What double pages become (older presets said only "split").
    pub fn spread(&self) -> Spread {
        if self.spread == Spread::Keep && self.split {
            Spread::Split
        } else {
            self.spread
        }
    }
}

/// One page as written: its picture, size, and whether it is grey.
struct Out {
    data: Vec<u8>,
    w: u32,
    h: u32,
    grey: bool,
}

/// The JPEG quality of the pages, and of those whose scan was saved
/// small already.
const QUALITY: u8 = 85;
const QUALITY_LOW: u8 = 72;

/// The colour of a page's margins (white or black), as its edges are, if
/// they agree.
fn margin_colour(g: &GrayImage) -> Option<u8> {
    let (w, h) = g.dimensions();
    let mut sum = 0u64;
    let mut n = 0u64;
    for x in 0..w {
        sum += u64::from(g.get_pixel(x, 0)[0]) + u64::from(g.get_pixel(x, h - 1)[0]);
        n += 2;
    }
    for y in 0..h {
        sum += u64::from(g.get_pixel(0, y)[0]) + u64::from(g.get_pixel(w - 1, y)[0]);
        n += 2;
    }
    let mean = sum / n.max(1);
    if mean >= 200 {
        Some(255)
    } else if mean <= 55 {
        Some(0)
    } else {
        None
    }
}

/// The box of what a page shows, without the even margins around it
/// (white or black, as its edges are), and with `numbers` without a page
/// number standing alone at the top or bottom: `None` when there is
/// nothing to cut or the page seems empty.
fn content_box(g: &GrayImage, numbers: bool) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = g.dimensions();
    if w < 16 || h < 16 {
        return None;
    }
    let bg = margin_colour(g)?;
    // A pixel differs from the margin when far from its colour; a line
    // counts when a few of its pixels do (specks of dust do not).
    let ink = |v: u8| (i16::from(v) - i16::from(bg)).abs() > 48;
    let row_ink = |y: u32| (0..w).filter(|&x| ink(g.get_pixel(x, y)[0])).count() as u32;
    let col_ink = |x: u32, y0: u32, y1: u32| (y0..y1).filter(|&y| ink(g.get_pixel(x, y)[0])).count() as u32;
    let min_row = (w / 200).max(2);
    let rows: Vec<bool> = (0..h).map(|y| row_ink(y) >= min_row).collect();
    let mut top = rows.iter().position(|&r| r)? as u32;
    let mut bottom = rows.iter().rposition(|&r| r)? as u32 + 1;
    if numbers {
        // A small block alone near an edge, apart from the rest by a band
        // of margin: a page number (or a scanner's mark).
        let gap = (h / 100).max(2);
        let small = h / 25;
        let lone = |from: u32, to: u32| -> bool {
            // Its width: the columns with ink in its rows.
            let cols = (0..w).filter(|&x| col_ink(x, from, to) > 0).count() as u32;
            to - from <= small && cols <= w / 6
        };
        // Bottom: the last block of rows with ink, and the margin above it.
        let mut end = bottom;
        let mut start = end;
        while start > top && rows[start as usize - 1] {
            start -= 1;
        }
        let mut above = start;
        while above > top && !rows[above as usize - 1] {
            above -= 1;
        }
        if start - above >= gap && above > top && lone(start, end) {
            end = above;
            bottom = end;
        }
        // Top, the same way.
        let mut stop = top;
        while stop < bottom && rows[stop as usize] {
            stop += 1;
        }
        let mut below = stop;
        while below < bottom && !rows[below as usize] {
            below += 1;
        }
        if below - stop >= gap && below < bottom && lone(top, stop) {
            top = below;
        }
    }
    let cols: Vec<bool> = (0..w).map(|x| col_ink(x, top, bottom) >= (h / 200).max(2)).collect();
    let left = cols.iter().position(|&c| c)? as u32;
    let right = cols.iter().rposition(|&c| c)? as u32 + 1;
    let (cw, ch) = (right - left, bottom - top);
    // Too little left (an empty page), or too little to cut.
    if cw < w / 4 || ch < h / 4 || (cw + w / 50 >= w && ch + h / 50 >= h) {
        return None;
    }
    // A little margin kept around the drawing.
    let pad = (w.min(h) / 100).max(1);
    let x = left.saturating_sub(pad);
    let y = top.saturating_sub(pad);
    let cw = (right + pad).min(w) - x;
    let ch = (bottom + pad).min(h) - y;
    Some((x, y, cw, ch))
}

/// The page cut to [`content_box`], judged on a small grey copy.
fn cropped(img: DynamicImage, numbers: bool) -> DynamicImage {
    let small = img.thumbnail(800, 800).to_luma8();
    match content_box(&small, numbers) {
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
}

/// The tones stretched: the darkest few pixels black, the lightest white
/// (a grey scan's washed-out black made black), and with `gamma` above 1
/// the middle tones darkened. The same curve on each colour.
fn stretched(img: DynamicImage, gamma: f32) -> DynamicImage {
    let g = img.thumbnail(600, 600).to_luma8();
    let mut hist = [0u64; 256];
    for p in g.pixels() {
        hist[p[0] as usize] += 1;
    }
    let total: u64 = hist.iter().sum();
    let at = |share: f64| -> u8 {
        let want = (total as f64 * share) as u64;
        let mut seen = 0u64;
        for (v, n) in hist.iter().enumerate() {
            seen += n;
            if seen > want {
                return v as u8;
            }
        }
        255
    };
    let (lo, hi) = (at(0.005), at(0.995));
    // A page of one tone (a blank one): left alone.
    if hi <= lo || hi - lo < 32 {
        return img;
    }
    let lut: Vec<u8> = (0..=255u32)
        .map(|v| {
            let t = ((v as f32 - f32::from(lo)) / f32::from(hi - lo)).clamp(0.0, 1.0);
            (t.powf(gamma) * 255.0).round() as u8
        })
        .collect();
    match img {
        DynamicImage::ImageLuma8(mut p) => {
            for v in p.pixels_mut() {
                v[0] = lut[v[0] as usize];
            }
            DynamicImage::ImageLuma8(p)
        }
        other => {
            let mut p = other.to_rgb8();
            for v in p.pixels_mut() {
                for c in v.0.iter_mut() {
                    *c = lut[*c as usize];
                }
            }
            DynamicImage::ImageRgb8(p)
        }
    }
}

/// Fine dot patterns softened (a light blur, 1-4-6-4-1 each way): on a
/// colour ink screen they shimmer in colour.
fn softened(img: DynamicImage) -> DynamicImage {
    let src = img.to_rgb8();
    let (w, h) = src.dimensions();
    if w < 5 || h < 5 {
        return DynamicImage::ImageRgb8(src);
    }
    const K: [u32; 5] = [1, 4, 6, 4, 1];
    let pass = |src: &image::RgbImage, dx: i64, dy: i64| -> image::RgbImage {
        image::RgbImage::from_fn(w, h, |x, y| {
            let mut acc = [0u32; 3];
            for (i, k) in K.iter().enumerate() {
                let o = i as i64 - 2;
                let sx = (i64::from(x) + o * dx).clamp(0, i64::from(w) - 1) as u32;
                let sy = (i64::from(y) + o * dy).clamp(0, i64::from(h) - 1) as u32;
                let p = src.get_pixel(sx, sy);
                for c in 0..3 {
                    acc[c] += u32::from(p[c]) * k;
                }
            }
            image::Rgb([(acc[0] / 16) as u8, (acc[1] / 16) as u8, (acc[2] / 16) as u8])
        })
    };
    let across = pass(&src, 1, 0);
    DynamicImage::ImageRgb8(pass(&across, 0, 1))
}

/// A long strip cut into pages about as tall as the screen's shape asks,
/// each cut where a row is most even (between panels).
fn strips(img: DynamicImage, ratio: f32) -> Vec<DynamicImage> {
    let (w, h) = img.dimensions();
    let page_h = ((w as f32 * ratio) as u32).max(64);
    if h <= page_h * 3 / 2 {
        return vec![img];
    }
    let g = img.to_luma8();
    // How much a row looks like a gap: even, and white or black (a flat
    // colour inside a panel is not one).
    let busy = |y: u32| {
        let (mut lo, mut hi, mut sum, mut n) = (255u8, 0u8, 0u32, 0u32);
        for x in (0..w).step_by(2) {
            let v = g.get_pixel(x, y)[0];
            lo = lo.min(v);
            hi = hi.max(v);
            sum += u32::from(v);
            n += 1;
        }
        let mean = sum / n.max(1);
        let edge = if mean >= 230 || mean <= 25 { 0 } else { 100 };
        u32::from(hi - lo) + edge
    };
    let mut out = Vec::new();
    let mut at = 0u32;
    while h - at > page_h * 5 / 4 {
        let target = at + page_h;
        let from = target - page_h / 5;
        let to = (target + page_h / 10).min(h - 1);
        let cut = (from..to).min_by_key(|&y| (busy(y), y.abs_diff(target))).unwrap_or(target);
        out.push(img.crop_imm(0, at, w, cut - at));
        at = cut;
    }
    out.push(img.crop_imm(0, at, w, h - at));
    out
}

/// A page made ready: decoded, its wide cover narrowed to the front,
/// cut, split or turned, fit to the screen, its tones stretched, and
/// encoded. `first`: the cover. `rtl`: halves right first, and a cover's
/// front on the left.
fn prepare(data: &[u8], o: &EbookOptions, first: bool, rtl: bool) -> Result<Vec<Out>, String> {
    let img = image::load_from_memory(data).map_err(|e| format!("pageUnreadable: {e}"))?;
    let (w0, h0) = img.dimensions();
    let source_grey = matches!(img.color(), image::ColorType::L8 | image::ColorType::L16);
    let spread = o.spread();
    let mut img = img;
    let wide = |i: &DynamicImage| i.width() * 100 > i.height() * 115;
    // A cover scanned with its back: the front only.
    if first && spread != Spread::Keep && wide(&img) {
        let (w, h) = img.dimensions();
        let half = w / 2;
        img = if rtl { img.crop_imm(0, 0, half, h) } else { img.crop_imm(half, 0, w - half, h) };
    }
    if o.crop {
        img = cropped(img, o.numbers);
    }
    let screen = o.screen();
    let parts: Vec<DynamicImage> = if o.webtoon {
        let ratio = screen.map_or(1.5, |(sw, sh)| sh as f32 / sw as f32);
        strips(img, ratio)
    } else if !first && wide(&img) {
        let (w, h) = img.dimensions();
        let half = w / 2;
        let halves = || {
            let left = img.crop_imm(0, 0, half, h);
            let right = img.crop_imm(half, 0, w - half, h);
            if rtl { vec![right, left] } else { vec![left, right] }
        };
        match spread {
            Spread::Keep => vec![img.clone()],
            Spread::Split => halves(),
            Spread::Rotate => vec![img.rotate90()],
            Spread::Both => {
                let mut v = vec![img.rotate90()];
                v.extend(halves());
                v
            }
        }
    } else {
        vec![img]
    };
    let whole = parts.len() == 1;
    let grey = o.device.grey();
    parts
        .into_iter()
        .map(|p| {
            let p = match screen {
                Some((sw, sh)) if p.width() > sw || p.height() > sh => {
                    p.resize(sw, sh, image::imageops::FilterType::Lanczos3)
                }
                Some((sw, sh)) if o.upscale && p.width() < sw && p.height() < sh => {
                    p.resize(sw, sh, image::imageops::FilterType::CatmullRom)
                }
                _ => p,
            };
            let (w, h) = p.dimensions();
            // Not cut, split, turned, resized nor retouched: a JPEG stays
            // as it was (no second loss), unless a grey screen wants a
            // colour page grey.
            let same = whole && (w, h) == (w0, h0) && !o.contrast && !o.greys16 && !(o.soften && o.device.colour_ink());
            if same && data.starts_with(&[0xff, 0xd8]) && (source_grey || !grey) {
                return Ok(Out {
                    data: data.to_vec(),
                    w,
                    h,
                    grey: source_grey,
                });
            }
            let p = if grey { DynamicImage::ImageLuma8(p.to_luma8()) } else { p };
            let p = if o.contrast {
                stretched(p, if o.device.ink() { 1.2 } else { 1.0 })
            } else {
                p
            };
            let p = if o.soften && o.device.colour_ink() { softened(p) } else { p };
            let encode = |q: u8| -> Result<Vec<u8>, String> {
                let mut out = Vec::new();
                let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q);
                let r = if grey {
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
            // The 16 greys an ink screen shows, as a 4-bit PNG: kept when
            // smaller than the JPEG (drawings; rarely photos).
            if grey && o.greys16 && o.device.png_ok() {
                let png = png16(&p.to_luma8())?;
                if png.len() < out.len() {
                    out = png;
                }
            }
            Ok(Out { data: out, w, h, grey })
        })
        .collect()
}

/// A grey page in 16 greys, as a 4-bit PNG.
fn png16(g: &GrayImage) -> Result<Vec<u8>, String> {
    let (w, h) = g.dimensions();
    let row = (w as usize).div_ceil(2);
    let mut packed = vec![0u8; row * h as usize];
    for (y, line) in packed.chunks_mut(row).enumerate() {
        for x in 0..w {
            // The nearest of 16 levels (0, 17, … 255).
            let v = (u16::from(g.get_pixel(x, y as u32)[0]) + 8) / 17;
            let v = v.min(15) as u8;
            let b = &mut line[x as usize / 2];
            if x % 2 == 0 {
                *b |= v << 4;
            } else {
                *b |= v;
            }
        }
    }
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Grayscale);
        e.set_depth(png::BitDepth::Four);
        let mut wr = e.write_header().map_err(|e| e.to_string())?;
        wr.write_image_data(&packed).map_err(|e| e.to_string())?;
    }
    Ok(out)
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
    /// The book's other parts (made beside it when too large), which undo
    /// deletes.
    pub parts: Vec<std::path::PathBuf>,
}

/// The pages and details of a comic made into a book.
struct Made {
    pages: Vec<Out>,
    title: String,
    authors: Vec<String>,
    publisher: String,
    language: String,
    series: String,
    number: String,
    year: String,
    rtl: bool,
    /// Where each issue starts (page, title), for the table of contents
    /// (none: the book's title at its start).
    toc: Vec<(usize, String)>,
}

/// Makes the comic at `path` (renamed with the e-book's extension; its
/// content another kind, or the same kind for a CBZ or PDF made again)
/// into `to`; the original steps aside with its own extension (kept or
/// sent to the trash), or, when it had the same one, is kept in `store`
/// for undo. `values`: details for the book (a `ComicInfo.xml`'s keys);
/// `manga`: pages turn right to left (also when its `ComicInfo.xml`
/// says so).
#[allow(clippy::too_many_arguments)]
pub fn convert(
    path: &Path,
    to: Format,
    o: EbookOptions,
    values: &[(&'static str, String)],
    manga: bool,
    original: Original,
    pdf: PdfOptions,
    store: &Path,
) -> Result<Done, String> {
    let kind = crate::convert::kind(path).ok_or("comicUnsupported")?;
    if matches!(kind, Kind::Epub | Kind::Kindle | Kind::Folder) {
        return Err("comicUnsupported".into());
    }
    // Made again in the same kind (a CBZ to a CBZ, a PDF to a PDF): in
    // place, the old file kept for undo.
    let again = matches!((to, kind), (Format::Cbz, Kind::Zip) | (Format::Pdf, Kind::Pdf));
    let left = original_of(path, kind);
    if !again && (left == path || fs::symlink_metadata(&left).is_ok()) {
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
    let made = make(path, &pages, &info, &o, values, manga)?;
    let tmp = temp_of(path);
    let parts = write_all(&made, to, &info, values, &o, &tmp, &|k| part_path(path, to, k))?;
    let count = made.pages.len();
    let undo_parts = |e: String| {
        for p in &parts {
            let _ = fs::remove_file(p);
        }
        e
    };
    if again {
        let backup = crate::bookmeta::replace_keeping(path, &tmp, store).map_err(undo_parts)?;
        return Ok(Done {
            backup,
            not_trashed: None,
            pages: count,
            parts,
        });
    }
    let d = crate::bookconv::finish(path, &left, &tmp, original).map_err(undo_parts)?;
    Ok(Done {
        backup: d.backup,
        not_trashed: d.not_trashed,
        pages: count,
        parts,
    })
}

/// Where the original of the comic at `path` (named as the e-book, its
/// content `kind`) is left: named as it, with its own extension
/// (`Name.kepub.epub`: `Name.cbz`, not `Name.kepub.cbz`).
pub fn original_of(path: &Path, kind: Kind) -> std::path::PathBuf {
    let left = crate::convert::beside(path, kind);
    if let (Some(name), Some(ext)) = (path.file_name().and_then(|n| n.to_str()), left.extension())
        && let Some(stem) = name.strip_suffix(".kepub.epub")
    {
        return path.with_file_name(format!("{stem}.{}", ext.to_string_lossy()));
    }
    left
}

/// Where part `k` (from 2) of the book at `path` goes: `Name (2).azw3`.
fn part_path(path: &Path, to: Format, k: usize) -> std::path::PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = format!(".{}", to.ext());
    let stem = name.strip_suffix(&ext).unwrap_or(&name);
    path.with_file_name(format!("{stem} ({k}){ext}"))
}

/// Writes the book: at `main`, or, when larger than `o.max_mb`, in parts
/// of about even size, the first at `main` and the others at `part(2)`,
/// `part(3)`… (which must not exist; returned). Each part's title says
/// which it is (`(2/3)`). Anything written is removed on a failure.
fn write_all(
    m: &Made,
    to: Format,
    info: &str,
    values: &[(&'static str, String)],
    o: &EbookOptions,
    main: &Path,
    part: &dyn Fn(usize) -> std::path::PathBuf,
) -> Result<Vec<std::path::PathBuf>, String> {
    let total: u64 = m.pages.iter().map(|p| p.data.len() as u64).sum();
    // The pictures are most of a book: a little room for the rest.
    let room = (u64::from(o.max_mb) << 20) * 95 / 100;
    let one = |made: &Made, to_path: &Path| -> Result<(), String> {
        let r = match to {
            Format::Epub | Format::Kepub => crate::bookconv::epub_out::write(&book_of(made), to_path),
            Format::Azw3 => crate::bookconv::azw3_out::write(&book_of(made), to_path),
            Format::Cbz => write_cbz(made, info, values, to_path),
            Format::Pdf => write_pdf(made, to_path),
        };
        if r.is_err() {
            let _ = fs::remove_file(to_path);
        }
        r
    };
    if room == 0 || total <= room {
        return one(m, main).map(|_| Vec::new());
    }
    // A new part before a page would make this one too large (a page
    // larger than the limit is a part of its own).
    let mut cuts = vec![0usize];
    let mut run = 0u64;
    for (i, p) in m.pages.iter().enumerate() {
        let size = p.data.len() as u64;
        if run > 0 && run + size > room {
            cuts.push(i);
            run = 0;
        }
        run += size;
    }
    cuts.push(m.pages.len());
    let count = cuts.len() - 1;
    let mut made_parts: Vec<std::path::PathBuf> = Vec::new();
    let fail = |e: String, made_parts: &[std::path::PathBuf]| {
        for p in made_parts {
            let _ = fs::remove_file(p);
        }
        let _ = fs::remove_file(main);
        e
    };
    for k in 0..count {
        let (a, b) = (cuts[k], cuts[k + 1]);
        let piece = Made {
            pages: m.pages[a..b].iter().map(|p| Out { data: p.data.clone(), ..*p }).collect(),
            title: format!("{} ({}/{count})", m.title, k + 1),
            authors: m.authors.clone(),
            publisher: m.publisher.clone(),
            language: m.language.clone(),
            series: m.series.clone(),
            number: m.number.clone(),
            year: m.year.clone(),
            rtl: m.rtl,
            toc: m
                .toc
                .iter()
                .filter(|(p, _)| (a..b).contains(p))
                .map(|(p, t)| (p - a, t.clone()))
                .collect(),
        };
        let to_path = if k == 0 { main.to_path_buf() } else { part(k + 1) };
        if k > 0 && fs::symlink_metadata(&to_path).is_ok() {
            return Err(fail("convertTaken".into(), &made_parts));
        }
        one(&piece, &to_path).map_err(|e| fail(e, &made_parts))?;
        if k > 0 {
            made_parts.push(to_path);
        }
    }
    Ok(made_parts)
}

/// The issues to join into one volume, in reading order: each comic's
/// path (any kind [`convert`] takes, its content as it was) and details.
pub struct Issue {
    pub path: std::path::PathBuf,
    pub values: Vec<(&'static str, String)>,
}

/// What [`join`] made: the volume (and its other parts when too large),
/// all new files.
#[derive(Debug)]
pub struct Joined {
    pub files: Vec<std::path::PathBuf>,
    pub pages: usize,
}

/// Joins `issues` into one book at `out` (a new file), one entry per
/// issue in its table of contents; the comics are left as they are.
/// `title`: the volume's (else the first issue's series).
pub fn join(
    issues: &[Issue],
    to: Format,
    o: EbookOptions,
    manga: bool,
    pdf: PdfOptions,
    out: &Path,
    title: &str,
) -> Result<Joined, String> {
    if issues.is_empty() {
        return Err("comicUnsupported".into());
    }
    if fs::symlink_metadata(out).is_ok() {
        return Err("convertTaken".into());
    }
    let mut all: Option<Made> = None;
    let mut first_info = String::new();
    for (n, issue) in issues.iter().enumerate() {
        let kind = crate::convert::kind(&issue.path).ok_or("comicUnsupported")?;
        if matches!(kind, Kind::Epub | Kind::Kindle | Kind::Folder) {
            return Err("comicUnsupported".into());
        }
        let (pages, info) = if kind == Kind::Zip {
            zip_pages(&issue.path)?
        } else {
            let name = issue.path.file_name().unwrap_or_default().to_string_lossy();
            let cbz = issue.path.with_file_name(format!(".{name}.renome-pages"));
            let r = crate::convert::write_zip(&issue.path, kind, &cbz, &[], pdf).and_then(|_| zip_pages(&cbz));
            let _ = fs::remove_file(&cbz);
            r?
        };
        if pages.is_empty() {
            continue;
        }
        let made = make(&issue.path, &pages, &info, &o, &issue.values, manga)?;
        if n == 0 {
            first_info = info;
        }
        match &mut all {
            None => {
                let mut m = made;
                m.toc = vec![(0, m.title.clone())];
                all = Some(m);
            }
            Some(a) => {
                a.toc.push((a.pages.len(), made.title.clone()));
                for x in made.authors {
                    if !a.authors.contains(&x) {
                        a.authors.push(x);
                    }
                }
                a.pages.extend(made.pages);
            }
        }
    }
    let mut all = all.ok_or("comicUnsupported")?;
    all.title = if title.is_empty() { all.series.clone() } else { title.to_string() };
    if all.title.is_empty() {
        all.title = out.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    }
    let values = &issues[0].values;
    let tmp = temp_of(out);
    let parts = write_all(&all, to, &first_info, values, &o, &tmp, &|k| part_path(out, to, k))?;
    if let Err((_, e)) = crate::apply::rename_new(&tmp, out) {
        let _ = fs::remove_file(&tmp);
        for p in &parts {
            let _ = fs::remove_file(p);
        }
        return Err(e);
    }
    let mut files = vec![out.to_path_buf()];
    files.extend(parts);
    Ok(Joined {
        files,
        pages: all.pages.len(),
    })
}

/// The pages made ready (a few at a time on each core) and the book's
/// details.
fn make(
    path: &Path,
    pages: &[Vec<u8>],
    info: &str,
    o: &EbookOptions,
    values: &[(&'static str, String)],
    manga: bool,
) -> Result<Made, String> {
    let rtl = manga || field(values, info, "Manga").eq_ignore_ascii_case("YesAndRightToLeft");
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
                        .map(|(i, p)| prepare(p, o, c * chunk + i == 0, rtl))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_else(|_| vec![Err("pageUnreadable".into())]))
            .collect()
    });
    // A page that cannot be read is left out, as the CBZ would.
    let pages: Vec<Out> = done.into_iter().filter_map(Result::ok).flatten().collect();
    if pages.is_empty() {
        return Err("comicUnsupported".into());
    }
    let series = field(values, info, "Series");
    let number = field(values, info, "Number");
    let title = field(values, info, "Title");
    let title = match (series.is_empty(), number.is_empty(), title.is_empty()) {
        (false, false, false) => format!("{series} #{number} - {title}"),
        (false, false, true) => format!("{series} #{number}"),
        (false, true, _) => series.clone(),
        _ if !title.is_empty() => title.clone(),
        _ => path
            .file_stem()
            .map(|s| s.to_string_lossy().trim_end_matches(".kepub").to_string())
            .unwrap_or_default(),
    };
    Ok(Made {
        pages,
        title,
        authors: field(values, info, "Writer")
            .split(',')
            .map(|a| a.trim().to_string())
            .filter(|a| !a.is_empty())
            .collect(),
        publisher: field(values, info, "Publisher"),
        language: field(values, info, "LanguageISO"),
        series,
        number,
        year: field(values, info, "Year"),
        rtl,
        toc: Vec::new(),
    })
}

/// The fixed-layout book of the pages, one per page.
fn book_of(m: &Made) -> Book {
    let mut book = Book {
        fixed: true,
        rtl: m.rtl,
        css: crate::bookconv::BASE_CSS.into(),
        ..Default::default()
    };
    book.meta.title = m.title.clone();
    book.meta.authors = m.authors.clone();
    book.meta.publisher = m.publisher.clone();
    book.meta.language = m.language.clone();
    book.meta.series = m.series.clone();
    book.meta.series_index = m.number.clone();
    book.meta.date = m.year.clone();
    for out in &m.pages {
        let Some(name) = book.add_file(out.data.clone(), "p.jpg") else { continue };
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
    }
    let marks = if m.toc.is_empty() { vec![(0, m.title.clone())] } else { m.toc.clone() };
    for (page, title) in marks {
        if let Some(c) = book.chapters.get(page) {
            book.toc.push(Toc {
                title,
                href: c.name.clone(),
                kids: vec![],
            });
        }
    }
    book
}

/// A CBZ of the pages made ready, with the comic's `ComicInfo.xml` (and
/// `values` in it).
fn write_cbz(m: &Made, info: &str, values: &[(&'static str, String)], to: &Path) -> Result<(), String> {
    let err = |e: zip::result::ZipError| e.to_string();
    let file = File::create(to).map_err(|e| e.to_string())?;
    let mut z = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let digits = m.pages.len().to_string().len().max(3);
    for (i, p) in m.pages.iter().enumerate() {
        let ext = if p.data.starts_with(b"\x89PNG") { "png" } else { "jpg" };
        // Pictures are compressed already.
        let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        z.start_file(format!("{:0digits$}.{ext}", i + 1), o).map_err(err)?;
        z.write_all(&p.data).map_err(|e| e.to_string())?;
    }
    let old = (!info.is_empty()).then_some(info);
    let xml = if values.is_empty() {
        old.map(str::to_string)
    } else {
        crate::inside::comic_xml(old, values).or_else(|| old.map(str::to_string))
    };
    if let Some(xml) = xml {
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("ComicInfo.xml", o).map_err(err)?;
        z.write_all(xml.as_bytes()).map_err(|e| e.to_string())?;
    }
    let w = z.finish().map_err(err)?;
    let f = w.into_inner().map_err(|e| e.error().to_string())?;
    f.sync_all().map_err(|e| e.to_string())
}

/// A PDF of the pages, one picture per page, sized as printed at 300
/// dots per inch: JPEGs as they are, PNGs as compressed grey or colour
/// pixels.
fn write_pdf(m: &Made, to: &Path) -> Result<(), String> {
    use lopdf::{Dictionary, Document, Object, Stream, dictionary};
    let mut doc = Document::with_version("1.5");
    let tree = doc.new_object_id();
    let mut kids: Vec<Object> = Vec::new();
    for p in &m.pages {
        let (dict, data) = if p.data.starts_with(&[0xff, 0xd8]) {
            let d: Dictionary = dictionary! {
                "Type" => "XObject", "Subtype" => "Image",
                "Width" => p.w as i64, "Height" => p.h as i64, "BitsPerComponent" => 8,
                "ColorSpace" => if p.grey { "DeviceGray" } else { "DeviceRGB" },
                "Filter" => "DCTDecode",
            };
            (d, p.data.clone())
        } else {
            let img = image::load_from_memory(&p.data).map_err(|e| e.to_string())?;
            let (raw, space) = if p.grey {
                (img.to_luma8().into_raw(), "DeviceGray")
            } else {
                (img.to_rgb8().into_raw(), "DeviceRGB")
            };
            let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            z.write_all(&raw).map_err(|e| e.to_string())?;
            let d: Dictionary = dictionary! {
                "Type" => "XObject", "Subtype" => "Image",
                "Width" => p.w as i64, "Height" => p.h as i64, "BitsPerComponent" => 8,
                "ColorSpace" => space, "Filter" => "FlateDecode",
            };
            (d, z.finish().map_err(|e| e.to_string())?)
        };
        let image = doc.add_object(Stream::new(dict, data).with_compression(false));
        let (pw, ph) = (p.w as f32 * 72.0 / 300.0, p.h as f32 * 72.0 / 300.0);
        let content = format!("q {pw:.2} 0 0 {ph:.2} 0 0 cm /P Do Q");
        let c = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => tree,
            "MediaBox" => vec![0.into(), 0.into(), Object::Real(pw), Object::Real(ph)],
            "Contents" => c,
            "Resources" => dictionary! { "XObject" => dictionary! { "P" => image } },
        });
        kids.push(Object::Reference(page));
    }
    let count = kids.len() as i64;
    doc.objects.insert(
        tree,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let mut info = dictionary! {};
    // Text in UTF-16, as PDF readers take any language.
    let text = |s: &str| {
        let mut b = vec![0xfe, 0xff];
        for u in s.encode_utf16() {
            b.extend(u.to_be_bytes());
        }
        Object::String(b, lopdf::StringFormat::Hexadecimal)
    };
    if !m.title.is_empty() {
        info.set("Title", text(&m.title));
    }
    if !m.authors.is_empty() {
        info.set("Author", text(&m.authors.join(", ")));
    }
    let info = doc.add_object(info);
    doc.trailer.set("Info", info);
    let mut catalog = dictionary! { "Type" => "Catalog", "Pages" => tree };
    if m.rtl {
        catalog.set("ViewerPreferences", dictionary! { "Direction" => "R2L" });
    }
    let catalog = doc.add_object(catalog);
    doc.trailer.set("Root", catalog);
    let mut f = File::create(to).map_err(|e| e.to_string())?;
    doc.save_to(&mut f).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())
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
        let (x, y, w, h) = content_box(&g, false).unwrap();
        assert!(x > 40 && x < 52 && y > 50 && y < 62, "{x} {y}");
        assert!(w > 290 && w < 320 && h > 470 && h < 500, "{w} {h}");
        // A page all drawn: nothing cut.
        let full = DynamicImage::ImageRgb8(image::RgbImage::from_fn(300, 400, |x, _| {
            image::Rgb([(x % 255) as u8, 90, 90])
        }));
        assert!(content_box(&full.to_luma8(), false).is_none());
    }

    /// A page with a drawing and, apart below it, a small page number.
    fn numbered(w: u32, h: u32) -> GrayImage {
        GrayImage::from_fn(w, h, |x, y| {
            let drawing = x > w / 8 && x < w - w / 8 && y > h / 10 && y < h * 8 / 10;
            let number = x > w / 2 - 6 && x < w / 2 + 6 && y > h * 9 / 10 && y < h * 9 / 10 + 12;
            image::Luma([if drawing || number { 20 } else { 255 }])
        })
    }

    #[test]
    fn page_number_cut_too() {
        let g = numbered(400, 600);
        let (_, _, _, h_plain) = content_box(&g, false).unwrap();
        let (_, y, _, h) = content_box(&g, true).unwrap();
        // Without the number, the box ends under the drawing (y 480).
        assert!(h_plain > 470, "{h_plain}");
        assert!(y + h < 495 && y + h > 478, "{y} {h}");
        // A page whose last block is wide is not a number: kept.
        let wide = GrayImage::from_fn(400, 600, |x, y| {
            let a = x > 50 && x < 350 && y > 60 && y < 400;
            let b = x > 50 && x < 350 && y > 450 && y < 470;
            image::Luma([if a || b { 20 } else { 255 }])
        });
        let (_, y, _, h) = content_box(&wide, true).unwrap();
        assert!(y + h > 469, "{y} {h}");
    }

    #[test]
    fn tones_stretched() {
        // A washed-out scan: greys from 60 to 200.
        let img = DynamicImage::ImageLuma8(GrayImage::from_fn(100, 100, |x, _| image::Luma([60 + (x * 14 / 10) as u8])));
        let out = stretched(img, 1.2).to_luma8();
        let (lo, hi) = out.pixels().fold((255u8, 0u8), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
        assert_eq!((lo, hi), (0, 255));
        // A blank page stays blank.
        let blank = DynamicImage::ImageLuma8(GrayImage::from_pixel(50, 50, image::Luma([240])));
        assert!(stretched(blank, 1.2).to_luma8().pixels().all(|p| p[0] == 240));
    }

    #[test]
    fn sixteen_greys() {
        let g = GrayImage::from_fn(33, 7, |x, _| image::Luma([(x * 8) as u8]));
        let png = png16(&g).unwrap();
        let back = image::load_from_memory(&png).unwrap().to_luma8();
        assert_eq!(back.dimensions(), (33, 7));
        for (a, b) in g.pixels().zip(back.pixels()) {
            assert!(a[0].abs_diff(b[0]) <= 9, "{} {}", a[0], b[0]);
            assert_eq!(b[0] % 17, 0);
        }
    }

    #[test]
    fn long_strip_cut_between_panels() {
        // Panels 700 px tall with white gaps of 40 between them.
        let strip = DynamicImage::ImageLuma8(GrayImage::from_fn(500, 3700, |_, y| {
            image::Luma([if y % 740 < 700 { 30 } else { 255 }])
        }));
        let parts = strips(strip, 1.5);
        assert!(parts.len() >= 4, "{}", parts.len());
        assert_eq!(parts.iter().map(|p| p.height()).sum::<u32>(), 3700);
        // Every cut in a gap.
        let mut at = 0;
        for p in &parts[..parts.len() - 1] {
            at += p.height();
            assert!(at % 740 >= 700, "cut at {at}");
        }
    }

    fn opts(device: Device) -> EbookOptions {
        EbookOptions { device, crop: false, spread: Spread::Keep, ..Default::default() }
    }

    #[test]
    fn spreads_and_covers() {
        let wide = jpeg(page(1600, 1000));
        let o = |s| EbookOptions { spread: s, ..opts(Device::Kindle) };
        let sizes = |v: Vec<Out>| v.iter().map(|p| (p.w > p.h, p.w)).collect::<Vec<_>>();
        assert_eq!(prepare(&wide, &o(Spread::Keep), false, false).unwrap().len(), 1);
        assert_eq!(prepare(&wide, &o(Spread::Split), false, false).unwrap().len(), 2);
        let turned = prepare(&wide, &o(Spread::Rotate), false, false).unwrap();
        assert_eq!(sizes(turned).len(), 1);
        let both = prepare(&wide, &o(Spread::Both), false, false).unwrap();
        assert_eq!(both.len(), 3);
        assert!(both[0].h > both[0].w, "turned to stand");
        // A wide cover keeps its front: the right half (left in a manga).
        let cover = DynamicImage::ImageLuma8(GrayImage::from_fn(1600, 1000, |x, _| image::Luma([if x < 800 { 0 } else { 255 }])));
        let c = prepare(&jpeg(cover.clone()), &o(Spread::Split), true, false).unwrap();
        let px = image::load_from_memory(&c[0].data).unwrap().to_luma8();
        assert!(px.get_pixel(px.width() / 2, px.height() / 2)[0] > 200);
        let c = prepare(&jpeg(cover), &o(Spread::Split), true, true).unwrap();
        let px = image::load_from_memory(&c[0].data).unwrap().to_luma8();
        assert!(px.get_pixel(px.width() / 2, px.height() / 2)[0] < 50);
    }

    #[test]
    fn small_pages_enlarged_and_custom_screens() {
        let small = jpeg(page(500, 700));
        let o = EbookOptions { upscale: true, ..opts(Device::Kindle) };
        let p = &prepare(&small, &o, false, false).unwrap()[0];
        assert!(p.h == 1448 || p.w == 1072, "{}x{}", p.w, p.h);
        let o = EbookOptions { width: 600, height: 800, ..opts(Device::Custom) };
        let p = &prepare(&jpeg(page(1600, 2400)), &o, false, false).unwrap()[0];
        assert!(p.w <= 600 && p.h <= 800);
        // A custom size too small to be one: pages as they are.
        let o = EbookOptions { width: 10, height: 10, ..opts(Device::Custom) };
        assert_eq!(o.screen(), None);
    }

    #[test]
    fn comic_made_ebooks() {
        let d = tempfile::tempdir().unwrap();
        let pages = vec![page(1600, 2400), page(3200, 2400), page(1600, 2400)];
        let info = "<?xml version=\"1.0\"?><ComicInfo><Series>Hero</Series><Number>3</Number><Writer>Ana, Rui</Writer><Manga>YesAndRightToLeft</Manga></ComicInfo>";
        for to in [Format::Epub, Format::Kepub, Format::Azw3, Format::Pdf] {
            let src = d.path().join("Hero 003.cbz");
            cbz(&src, &pages, info);
            let old = fs::read(&src).unwrap();
            let named = src.with_extension(to.ext());
            fs::rename(&src, &named).unwrap();
            let o = EbookOptions {
                device: Device::Kindle,
                crop: true,
                split: true,
                ..Default::default()
            };
            let store = d.path().join("store");
            let done = convert(&named, to, o, &[], false, Original::Keep, PdfOptions::default(), &store)
                .unwrap();
            // Cover, the spread in two, the last page.
            assert_eq!(done.pages, 4);
            assert_eq!(fs::read(&src).unwrap(), old, "the original stays beside");
            if to == Format::Pdf {
                // A page per picture, one picture each, the title inside.
                let doc = lopdf::Document::load(&named).unwrap();
                assert_eq!(doc.get_pages().len(), 4);
                assert!(crate::convert::pdf_of_pictures(&named));
                crate::convert::restore(&named, done.backup.converted.as_ref().unwrap()).unwrap();
                fs::rename(&named, &src).unwrap();
                assert_eq!(fs::read(&src).unwrap(), old);
                continue;
            }
            let book = match to {
                Format::Epub | Format::Kepub => crate::bookconv::epub_in::read(&named).unwrap(),
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

    #[test]
    fn cbz_made_again_in_place() {
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("store");
        let src = d.path().join("Hero 003.cbz");
        let info = "<?xml version=\"1.0\"?><ComicInfo><Series>Hero</Series><Number>3</Number></ComicInfo>";
        cbz(&src, &[page(1600, 2400), page(3200, 2400)], info);
        let old = fs::read(&src).unwrap();
        let o = EbookOptions { device: Device::Kobo, crop: true, spread: Spread::Split, greys16: true, ..Default::default() };
        let done = convert(&src, Format::Cbz, o, &[], false, Original::Keep, PdfOptions::default(), &store).unwrap();
        assert_eq!(done.pages, 3);
        let mut z = zip::ZipArchive::new(File::open(&src).unwrap()).unwrap();
        let names: Vec<String> = z.file_names().filter_map(Result::ok).map(|n| n.into_owned()).collect();
        assert!(names.contains(&"ComicInfo.xml".to_string()), "{names:?}");
        assert_eq!(names.iter().filter(|n| n.ends_with(".jpg") || n.ends_with(".png")).count(), 3);
        let mut first = Vec::new();
        z.by_index(0).unwrap().read_to_end(&mut first).unwrap();
        let img = image::load_from_memory(&first).unwrap();
        assert!(img.width() <= 1072 && img.height() <= 1448);
        // Undo: the old file back from the history.
        let saved = done.backup.saved.clone().unwrap();
        assert_eq!(fs::read(store.join(saved)).unwrap(), old);
    }

    /// A page of noise (pictures that do not shrink much).
    fn noisy(w: u32, h: u32, seed: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| {
            let v = (x.wrapping_mul(2654435761) ^ y.wrapping_mul(40503) ^ seed).wrapping_mul(2246822519) >> 24;
            image::Rgb([v as u8, (v >> 1) as u8, (v >> 2) as u8])
        }))
    }

    #[test]
    fn large_book_made_in_parts() {
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("store");
        let src = d.path().join("Big.cbz");
        let pages: Vec<DynamicImage> = (0..6).map(|i| noisy(600, 800, i)).collect();
        cbz(&src, &pages, "<ComicInfo><Series>Big</Series></ComicInfo>");
        let named = d.path().join("Big.epub");
        fs::rename(&src, &named).unwrap();
        let o = EbookOptions { device: Device::Original, max_mb: 1, ..Default::default() };
        let done = convert(&named, Format::Epub, o, &[], false, Original::Keep, PdfOptions::default(), &store).unwrap();
        assert!(!done.parts.is_empty(), "made in parts");
        assert_eq!(done.parts[0], d.path().join("Big (2).epub"));
        let mut pages = 0;
        for (k, p) in std::iter::once(&named).chain(&done.parts).enumerate() {
            let b = crate::bookconv::epub_in::read(p).unwrap();
            assert!(b.meta.title.ends_with(&format!("({}/{})", k + 1, done.parts.len() + 1)), "{}", b.meta.title);
            pages += b.chapters.len();
            assert!(fs::metadata(p).unwrap().len() < 1_200_000);
        }
        assert_eq!(pages, 6);
    }

    #[test]
    fn issues_joined_into_a_volume() {
        let d = tempfile::tempdir().unwrap();
        let mut issues = Vec::new();
        for n in 1..=3 {
            let p = d.path().join(format!("Hero #{n}.cbz"));
            cbz(&p, &[page(800, 1200), page(800, 1200)], &format!("<ComicInfo><Series>Hero</Series><Number>{n}</Number><Writer>Ana</Writer></ComicInfo>"));
            issues.push(Issue { path: p, values: Vec::new() });
        }
        let before: Vec<Vec<u8>> = issues.iter().map(|i| fs::read(&i.path).unwrap()).collect();
        let out = d.path().join("Hero #1-3.epub");
        let o = EbookOptions { device: Device::Kobo, ..Default::default() };
        let j = join(&issues, Format::Epub, o, false, PdfOptions::default(), &out, "Hero #1-3").unwrap();
        assert_eq!(j.files, std::slice::from_ref(&out));
        assert_eq!(j.pages, 6);
        let b = crate::bookconv::epub_in::read(&out).unwrap();
        assert_eq!(b.meta.title, "Hero #1-3");
        assert_eq!(b.chapters.len(), 6);
        let toc: Vec<&str> = b.toc.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(toc, ["Hero #1", "Hero #2", "Hero #3"]);
        // The comics are as they were; a second volume there is refused.
        for (i, old) in issues.iter().zip(before) {
            assert_eq!(fs::read(&i.path).unwrap(), old);
        }
        assert_eq!(join(&issues, Format::Epub, o, false, PdfOptions::default(), &out, "").unwrap_err(), "convertTaken");
    }

    /// Makes e-books of the comics in `RENOME_COMIC_PROBE/src` (copies) in
    /// `…/out`, with several settings, printing time, pages and sizes, and
    /// saving a few pages of each as samples.
    #[test]
    #[ignore]
    fn probe_comics() {
        let Ok(dir) = std::env::var("RENOME_COMIC_PROBE") else { return };
        let dir = Path::new(&dir);
        let out = dir.join("out");
        let _ = fs::remove_dir_all(&out);
        fs::create_dir_all(out.join("samples")).unwrap();
        let store = out.join("store");
        let base = EbookOptions { device: Device::KindlePw2024, crop: true, spread: Spread::Split, contrast: true, ..Default::default() };
        let sets: Vec<(&str, Format, EbookOptions)> = vec![
            ("azw3", Format::Azw3, base),
            ("azw3-plain", Format::Azw3, EbookOptions { contrast: false, ..base }),
            ("azw3-16", Format::Azw3, EbookOptions { greys16: true, numbers: true, ..base }),
            ("kepub-color", Format::Kepub, EbookOptions { device: Device::KoboColor, soften: true, ..base }),
            ("cbz-both", Format::Cbz, EbookOptions { spread: Spread::Both, upscale: true, ..base }),
            ("pdf-scribe", Format::Pdf, EbookOptions { device: Device::KindleScribe2025, ..base }),
            ("epub-parts", Format::Epub, EbookOptions { max_mb: 50, ..base }),
        ];
        let mut srcs: Vec<_> = fs::read_dir(dir.join("src")).unwrap().flatten().map(|e| e.path()).collect();
        srcs.sort();
        for src in &srcs {
            let size = fs::metadata(src).unwrap().len();
            for (label, to, o) in &sets {
                let stem = src.file_stem().unwrap().to_string_lossy();
                let named = out.join(format!("{stem}-{label}.{}", to.ext()));
                // The comic copied under the e-book's name, as the rename leaves it.
                fs::copy(src, &named).unwrap();
                let t = std::time::Instant::now();
                match convert(&named, *to, *o, &[], stem.starts_with("frieren"), Original::Delete, PdfOptions::default(), &store) {
                    Ok(d) => {
                        let mut total = fs::metadata(&named).map(|m| m.len()).unwrap_or(0);
                        for p in &d.parts {
                            total += fs::metadata(p).unwrap().len();
                        }
                        println!(
                            "{stem:12} {label:12} {:>4} pages {:>6.1} MB -> {:>6.1} MB ({} parts) {:.1}s",
                            d.pages,
                            size as f64 / 1e6,
                            total as f64 / 1e6,
                            d.parts.len() + 1,
                            t.elapsed().as_secs_f32()
                        );
                    }
                    Err(e) => println!("{stem:12} {label:12} ERROR {e}"),
                }
                // Samples: pages 1, 5 and the middle, from what was made.
                let pics: Vec<Vec<u8>> = match to {
                    Format::Cbz => zip_pages(&named).map(|p| p.0).unwrap_or_default(),
                    Format::Pdf => Vec::new(),
                    Format::Azw3 => crate::bookconv::mobi::read(&named).map(|b| b.files.into_iter().map(|f| f.data).collect()).unwrap_or_default(),
                    _ => crate::bookconv::epub_in::read(&named).map(|b| b.files.into_iter().map(|f| f.data).collect()).unwrap_or_default(),
                };
                for i in [0usize, 4, pics.len() / 2] {
                    if let Some(p) = pics.get(i) {
                        let ext = if p.starts_with(b"\x89PNG") { "png" } else { "jpg" };
                        fs::write(out.join("samples").join(format!("{stem}-{label}-{i}.{ext}")), p).unwrap();
                    }
                }
            }
        }
        // Two issues of one series joined.
        let issues: Vec<Issue> = srcs
            .iter()
            .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("rei"))
            .map(|p| Issue { path: p.clone(), values: vec![("Series", "Rei".into())] })
            .collect();
        if issues.len() > 1 {
            let vol = out.join("Rei #3-4.azw3");
            let t = std::time::Instant::now();
            let j = join(&issues, Format::Azw3, base, false, PdfOptions::default(), &vol, "Rei #3-4").unwrap();
            let b = crate::bookconv::mobi::read(&vol).unwrap();
            println!("joined {} pages, toc {:?}, {:.1}s", j.pages, b.toc.iter().map(|t| t.title.clone()).collect::<Vec<_>>(), t.elapsed().as_secs_f32());
        }
    }

    /// Makes `RENOME_KF8_PROBE/src.cbz` into `ours.azw3` and `ours.epub`
    /// (Kindle Paperwhite 2024, defaults), for checking with other tools.
    #[test]
    #[ignore]
    fn probe_kf8() {
        let Ok(dir) = std::env::var("RENOME_KF8_PROBE") else { return };
        let dir = Path::new(&dir);
        let store = dir.join("store");
        let o = EbookOptions { device: Device::KindlePw2024, crop: true, spread: Spread::Split, contrast: true, ..Default::default() };
        for (to, name) in [(Format::Azw3, "ours.azw3"), (Format::Epub, "ours.epub"), (Format::Pdf, "ours.pdf")] {
            let p = dir.join(name);
            fs::copy(dir.join("src.cbz"), &p).unwrap();
            let label = match to {
                Format::Pdf => "Teste 3 - PDF",
                _ => "Teste 1 - AZW3 paginas fixas",
            };
            let values = vec![("Series", label.to_string()), ("Writer", "Autor".to_string())];
            convert(&p, to, o, &values, false, Original::Delete, PdfOptions::default(), &store).unwrap();
        }
    }
}
