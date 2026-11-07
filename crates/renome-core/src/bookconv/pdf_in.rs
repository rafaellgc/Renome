//! A PDF read into a [`Book`] whose text flows again.
//!
//! A PDF only says where each letter is drawn. The text is rebuilt from
//! that: letters become words and lines (by their baselines and the gaps
//! between them), lines become blocks (lines close together, one above
//! the other), blocks are put in reading order (columns found by the
//! empty strips between them), page headers, footers and numbers that
//! repeat are left out, lines become paragraphs again (by indents, short
//! last lines and spacing), words broken with a hyphen at a line's end
//! are joined, and a paragraph cut by a page or column goes on. Headings
//! are told by their size (and the PDF's bookmarks, which also cut the
//! chapters), italics and bold by their fonts, note marks by their size
//! and height, notes at a page's foot by their size and place. Pictures
//! stay where they were in the text.
//!
//! A book with no text (scanned, without a text layer) cannot flow: its
//! pages become pictures, in a book of fixed pages.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use hayro::hayro_interpret::{
    self as hi, BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, InterpreterSettings, SoftMask, font::GlyphRun,
};
use hi::TransformExt;
use hi::kurbo::{Affine, BezPath, Point, Rect};
use unicode_normalization::UnicodeNormalization;

use super::xhtml::{Elem, Node};
use super::{Book, Chapter};

// ---------------------------------------------------------------- reading

/// One letter (or a few, for a ligature), in page space with y down.
#[derive(Debug, Clone)]
struct Glyph {
    text: String,
    x0: f32,
    x1: f32,
    base: f32,
    size: f32,
    bold: bool,
    italic: bool,
    invisible: bool,
}

#[derive(Debug, Clone)]
struct Pic {
    name: String,
    r: Rect,
}

#[derive(Default)]
struct PageData {
    w: f32,
    h: f32,
    glyphs: Vec<Glyph>,
    pics: Vec<Pic>,
}

#[derive(Clone, Copy, Default)]
struct FontStyle {
    bold: bool,
    italic: bool,
}

/// Collects what a page draws: letters and pictures.
struct Collector<'b> {
    page: PageData,
    fonts: HashMap<u128, FontStyle>,
    /// Per font: for its plain letters that say what they are, how far
    /// each is from its code (a subset font numbers its letters in order:
    /// then a letter that does not say is its code moved as much).
    codes: HashMap<u128, HashMap<i64, usize>>,
    book: &'b mut Book,
    /// Pictures already taken (the same picture on many pages).
    seen_pics: &'b mut HashMap<u128, Option<String>>,
}

fn style_of(name: &str, weight: Option<u32>, italic: bool) -> FontStyle {
    let n = name.to_ascii_lowercase();
    // `ABCDEF+Name-BoldItalic`: the part after the subset tag.
    let n = n.split_once('+').map(|(_, r)| r).unwrap_or(&n);
    let bold = weight.is_some_and(|w| w >= 600)
        || [
            "bold",
            "black",
            "heavy",
            "semibold",
            "demibold",
            "demi",
            "extrabold",
            "ultrabold",
            ",bd",
            "-bd",
        ]
        .iter()
        .any(|k| n.contains(k));
    let italic = italic
        || [
            "italic", "oblique", "ital", "-it", ",it", "kursiv", "slanted",
        ]
        .iter()
        .any(|k| n.contains(k));
    FontStyle { bold, italic }
}

impl<'a> Device<'a> for Collector<'_> {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}

    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, props: DrawProps<'a>, mode: &DrawMode) {
        if matches!(
            run.writing_mode(),
            hayro::hayro_interpret::hayro_cmap::WritingMode::Vertical
        ) {
            return;
        }
        let invisible = matches!(mode, DrawMode::Invisible);
        let mut style = FontStyle::default();
        let mut font_key = 0u128;
        if let Some(first) = run.glyphs().first()
            && let hi::font::Glyph::Outline(o) = &**first
            && let Some(fd) = o.font_data()
        {
            font_key = fd.cache_key;
            style = *self.fonts.entry(fd.cache_key).or_insert_with(|| {
                style_of(
                    fd.postscript_name.as_deref().unwrap_or(""),
                    fd.weight,
                    fd.is_italic,
                )
            });
        }
        {
            let st = self.codes.entry(font_key).or_default();
            for g in run.glyphs() {
                if let Some(hayro::hayro_interpret::hayro_cmap::BfString::Char(c)) = g.as_unicode()
                    && (33..127).contains(&(c as u32))
                {
                    *st.entry(c as i64 - g.char_code() as i64).or_default() += 1;
                }
            }
        }
        let shift = self.codes.get(&font_key).and_then(|st| {
            let total: usize = st.values().sum();
            let (d, n) = st.iter().max_by_key(|(_, n)| **n)?;
            (total >= 8 && n * 10 >= total * 9).then_some(*d)
        });
        for g in run.glyphs() {
            let t: Affine = props.transform * g.transform();
            let o = t * Point::new(0.0, 0.0);
            let up = t * Point::new(0.0, 1000.0);
            let size = ((up.x - o.x).powi(2) + (up.y - o.y).powi(2)).sqrt() as f32;
            // Only text that runs left to right, upright.
            let [a, b, _, d, _, _] = t.as_coeffs();
            if size < 0.5 || a <= 0.0 || d >= 0.0 || b.abs() > a.abs() * 0.1 {
                continue;
            }
            let adv = match &**g {
                hi::font::Glyph::Outline(o) => o.advance_width(),
                hi::font::Glyph::Type3(t) => t.advance_width(),
            }
            .filter(|w| *w > 0.0 && *w < 5000.0)
            .unwrap_or(500.0) as f64;
            let end = t * Point::new(adv, 0.0);
            let text = match g.as_unicode() {
                Some(hayro::hayro_interpret::hayro_cmap::BfString::Char(c)) => c.to_string(),
                Some(hayro::hayro_interpret::hayro_cmap::BfString::String(s)) => s,
                // A font whose plain letters are their codes moved by the
                // same amount: this one too.
                None if shift.is_some_and(|d| (33..127).contains(&(g.char_code() as i64 + d))) => {
                    match char::from_u32((g.char_code() as i64 + shift.unwrap()) as u32) {
                        Some(c) => c.to_string(),
                        None => continue,
                    }
                }
                None => continue,
            };
            let text = normalize(&text);
            if text.is_empty() {
                continue;
            }
            self.page.glyphs.push(Glyph {
                text,
                x0: o.x as f32,
                x1: (end.x as f32).max(o.x as f32 + size * 0.1),
                base: o.y as f32,
                size,
                bold: style.bold,
                italic: style.italic,
                invisible,
            });
        }
    }

    fn draw_image(&mut self, image: Image<'a, '_>, props: ImageDrawProps<'a>) {
        let Image::Raster(r) = image else { return };
        // The transform takes the picture's pixels to the page.
        let rect = props.transform.transform_rect_bbox(Rect::new(
            0.0,
            0.0,
            r.width() as f64,
            r.height() as f64,
        ));
        let (pw, ph) = (self.page.w as f64, self.page.h as f64);
        // Drawn outside the page (copies a clip hides): not shown.
        let rect = rect.intersect(Rect::new(0.0, 0.0, pw, ph));
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        // Small marks, rules and dots are decoration.
        if rect.width() < pw * 0.06
            || rect.height() < ph * 0.03
            || r.width() < 32
            || r.height() < 32
        {
            return;
        }
        let key = hi::CacheKey::cache_key(&r);
        let name = match self.seen_pics.get(&key) {
            Some(n) => n.clone(),
            None => {
                let n = picture_of(&r)
                    .and_then(|(data, ext)| self.book.add_file(data, &format!("x.{ext}")));
                self.seen_pics.insert(key, n.clone());
                n
            }
        };
        if let Some(name) = name {
            self.page.pics.push(Pic { name, r: rect });
        }
    }
}

/// A picture's file: a JPEG as it is when it can be; else its pixels as
/// PNG (or JPEG when much smaller, for photos).
fn picture_of(r: &hi::RasterImage) -> Option<(Vec<u8>, &'static str)> {
    let s = r.stream();
    let filters = s.filters();
    if filters.len() == 1 && format!("{:?}", filters[0]).contains("Dct") {
        let raw = s.raw_data();
        // Not CMYK (readers show Adobe's CMYK JPEGs wrongly): three or one
        // components in the frame header.
        if jpeg_components(&raw).is_some_and(|c| c == 1 || c == 3) {
            return Some((raw.into_owned(), "jpg"));
        }
    }
    let mut out = None;
    r.with_rgba(
        |data, alpha| {
            let (w, h) = (data.width(), data.height());
            let (mut px, color) = match data {
                hi::ImageData::Rgb(d) => (d.data, png::ColorType::Rgb),
                hi::ImageData::Luma(d) => (d.data, png::ColorType::Grayscale),
            };
            // Transparency over white: readers' pages are light.
            if let Some(a) = alpha
                && a.data.len() == (w * h) as usize
                && a.data.iter().any(|v| *v < 255)
            {
                let k = if color == png::ColorType::Rgb { 3 } else { 1 };
                for (i, al) in a.data.iter().enumerate() {
                    for c in 0..k {
                        let v = &mut px[i * k + c];
                        *v = ((*v as u32 * *al as u32 + 255 * (255 - *al as u32)) / 255) as u8;
                    }
                }
            }
            // Photos as JPEG (much smaller), drawings and text as PNG.
            let comps = if color == png::ColorType::Rgb { 3 } else { 1 };
            if super::photo_like(&px, comps)
                && let Some(j) = super::jpeg_of(px.clone(), w, h, comps)
            {
                out = Some((j, "jpg"));
                return;
            }
            let mut buf = Vec::new();
            {
                let mut enc = png::Encoder::new(&mut buf, w, h);
                enc.set_color(color);
                enc.set_depth(png::BitDepth::Eight);
                enc.set_compression(png::Compression::Fast);
                if let Ok(mut wr) = enc.write_header()
                    && wr.write_image_data(&px).is_ok()
                {
                    let _ = wr.finish();
                }
            }
            if !buf.is_empty() {
                out = Some((buf, "png"));
            }
        },
        None,
    );
    out
}

/// The number of components of a JPEG, from its frame header.
fn jpeg_components(j: &[u8]) -> Option<u8> {
    let mut i = 2;
    while i + 9 < j.len() {
        if j[i] != 0xff {
            return None;
        }
        let m = j[i + 1];
        let len = u16::from_be_bytes([j[i + 2], j[i + 3]]) as usize;
        if (0xc0..=0xcf).contains(&m) && m != 0xc4 && m != 0xc8 && m != 0xcc {
            return j.get(i + 9).copied();
        }
        i += 2 + len;
    }
    None
}

/// Ligatures and the like as plain letters; soft hyphens and controls out.
fn normalize(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{fb00}' => o.push_str("ff"),
            '\u{fb01}' => o.push_str("fi"),
            '\u{fb02}' => o.push_str("fl"),
            '\u{fb03}' => o.push_str("ffi"),
            '\u{fb04}' => o.push_str("ffl"),
            '\u{fb05}' | '\u{fb06}' => o.push_str("st"),
            '\u{ad}' => o.push('-'),
            '\u{a0}' | '\u{2002}'..='\u{200a}' | '\u{202f}' => o.push(' '),
            '\u{2010}' | '\u{2011}' => o.push('-'),
            c if c.is_control() => {}
            '\u{fffd}' | '\u{feff}' | '\u{200b}' => {}
            c => o.push(c),
        }
    }
    o
}

/// A page's text is symbols, not words: most of what is drawn is not a
/// letter or digit.
fn garbage(glyphs: &[Glyph]) -> bool {
    let mut total = 0usize;
    let mut letters = 0usize;
    for g in glyphs {
        for c in g.text.chars().filter(|c| !c.is_whitespace()) {
            total += 1;
            if c.is_alphanumeric() {
                letters += 1;
            }
        }
    }
    total >= 40 && letters * 2 < total
}

/// The pages' letters and pictures.
fn collect(data: &Arc<Vec<u8>>, book: &mut Book) -> Result<Vec<PageData>, String> {
    let pdf = crate::convert::hayro_pdf(data)?;
    let cache = InterpreterCache::new();
    let mut seen = HashMap::new();
    let mut out = Vec::new();
    for page in pdf.pages().iter() {
        let (w, h) = page.render_dimensions();
        let transform = page.initial_transform(true).to_kurbo();
        let mut ctx = Context::new(
            transform,
            Rect::new(0.0, 0.0, w as f64, h as f64),
            &cache,
            page.xref(),
            InterpreterSettings::default(),
        );
        let mut dev = Collector {
            page: PageData {
                w,
                h,
                ..Default::default()
            },
            fonts: HashMap::new(),
            codes: HashMap::new(),
            book,
            seen_pics: &mut seen,
        };
        let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            hi::interpret_page(page, &mut ctx, &mut dev);
        }));
        let mut p = dev.page;
        if done.is_err() {
            p.glyphs.clear();
            p.pics.clear();
        }
        out.push(p);
    }
    Ok(out)
}

// ---------------------------------------------------------------- layout

/// Letters on one baseline, close together: a piece of a line.
#[derive(Debug, Clone)]
struct Chunk {
    x0: f32,
    x1: f32,
    base: f32,
    size: f32,
    /// Its words: (text, bold, italic, superscript).
    runs: Vec<Run>,
}

#[derive(Debug, Clone, PartialEq)]
struct Run {
    text: String,
    bold: bool,
    italic: bool,
    sup: bool,
    /// The page it is on (a note mark links to its page's note).
    page: usize,
}

impl Chunk {
    fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    fn top(&self) -> f32 {
        self.base - self.size * 0.8
    }
}

/// A block: lines one under the other.
#[derive(Debug, Clone)]
struct Block {
    lines: Vec<Chunk>,
    x0: f32,
    x1: f32,
    pic: Option<String>,
    top: f32,
    bottom: f32,
}

impl Block {
    fn size(&self) -> f32 {
        // The size most letters have.
        let mut by: HashMap<i32, usize> = HashMap::new();
        for l in &self.lines {
            *by.entry((l.size * 2.0).round() as i32).or_default() += l.text().chars().count();
        }
        by.into_iter()
            .max_by_key(|(_, n)| *n)
            .map(|(s, _)| s as f32 / 2.0)
            .unwrap_or(0.0)
    }

    fn text(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.text())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Spacing accents drawn as their own letters over a letter.
fn combining(c: char) -> Option<char> {
    Some(match c {
        '´' | '\u{2b9}' | '\u{2ca}' => '\u{301}',
        '`' | '\u{2cb}' => '\u{300}',
        '^' | 'ˆ' => '\u{302}',
        '~' | '˜' => '\u{303}',
        '¨' => '\u{308}',
        '¸' => '\u{327}',
        '˚' => '\u{30a}',
        'ˇ' => '\u{30c}',
        '˝' => '\u{30b}',
        '\u{300}'..='\u{36f}' => c,
        _ => return None,
    })
}

/// A page's letters as chunks of lines.
fn chunks_of(glyphs: &[Glyph]) -> Vec<Chunk> {
    if glyphs.is_empty() {
        return Vec::new();
    }
    let mut g: Vec<&Glyph> = glyphs.iter().collect();
    // Lines: letters whose baselines are close.
    g.sort_by(|a, b| a.base.total_cmp(&b.base).then(a.x0.total_cmp(&b.x0)));
    let mut lines: Vec<Vec<&Glyph>> = Vec::new();
    for x in g {
        match lines.last_mut() {
            Some(l) if (x.base - l[0].base).abs() <= l[0].size.min(x.size) * 0.3 => l.push(x),
            _ => lines.push(vec![x]),
        }
    }
    let merged: Vec<(Vec<&Glyph>, Vec<&Glyph>)> =
        lines.into_iter().map(|l| (l, Vec::new())).collect();
    // A line of small letters lying within the upper part of
    // the following line of bigger letters, overlapping it, is its marks.
    let mut i = 0;
    let mut out_lines: Vec<(Vec<&Glyph>, Vec<&Glyph>)> = Vec::new();
    while i < merged.len() {
        let (l, _) = &merged[i];
        let size = median(l.iter().map(|x| x.size));
        if i + 1 < merged.len() {
            let (next, _) = &merged[i + 1];
            let nsize = median(next.iter().map(|x| x.size));
            let lift = next[0].base - l[0].base;
            if size < nsize * 0.85 && lift > nsize * 0.15 && lift < nsize * 0.75 && l.len() <= 6 {
                let mut n = merged[i + 1].0.clone();
                let sups = l.clone();
                n.extend(sups.iter().copied());
                out_lines.push((n, sups));
                i += 2;
                continue;
            }
        }
        // Or of the line before (marks drawn after their line).
        if let Some((prev, psups)) = out_lines.last_mut() {
            let psize = median(prev.iter().map(|x| x.size));
            let lift = prev[0].base - l[0].base;
            if size < psize * 0.85 && lift > psize * 0.15 && lift < psize * 0.75 && l.len() <= 6 {
                psups.extend(l.iter().copied());
                prev.extend(l.iter().copied());
                i += 1;
                continue;
            }
        }
        out_lines.push((l.clone(), Vec::new()));
        i += 1;
    }

    let mut out = Vec::new();
    for (l, sups) in out_lines {
        let sup_set: HashSet<*const Glyph> = sups.iter().map(|g| *g as *const Glyph).collect();
        let mut l: Vec<(&Glyph, bool)> = l
            .into_iter()
            .map(|g| (g, sup_set.contains(&(g as *const Glyph))))
            .collect();
        l.sort_by(|a, b| a.0.x0.total_cmp(&b.0.x0));
        // The same letter drawn twice in place (fake bold): once.
        l.dedup_by(|b, a| {
            a.0.text == b.0.text
                && (a.0.x0 - b.0.x0).abs() < a.0.size * 0.08
                && (a.0.base - b.0.base).abs() < 0.5
        });
        let line_size = median(l.iter().filter(|g| !g.1).map(|g| g.0.size));
        let base = median(l.iter().filter(|g| !g.1).map(|g| g.0.base));
        let mut cur: Option<Chunk> = None;
        let mut prev_x1 = f32::MIN;
        let mut word_gap = line_size * 0.2;
        // The typical gap between words on this line (wide in justified
        // text).
        let line_gap = {
            let mut gaps: Vec<f32> = l
                .windows(2)
                .map(|w| w[1].0.x0 - w[0].0.x1)
                .filter(|g| *g > line_size * 0.1)
                .collect();
            gaps.sort_by(f32::total_cmp);
            gaps.get(gaps.len() / 2).copied().unwrap_or(0.0)
        };
        {
            let mut gaps: Vec<f32> = l
                .windows(2)
                .map(|w| w[1].0.x0 - w[0].0.x1)
                .filter(|g| *g > line_size * 0.1)
                .collect();
            gaps.sort_by(f32::total_cmp);
            if let Some(m) = gaps.get(gaps.len() / 2) {
                word_gap = word_gap.max(m * 0.5).min(line_size * 0.4);
            }
        }
        for (gl, is_sup) in l {
            let gap = gl.x0 - prev_x1;
            // A wide gap starts another chunk (another column, a tab).
            // A wide gap starts another chunk (another column, a tab) —
            // unless the line's words are all that far apart (justified
            // text without hyphens), or a space was drawn there.
            let spaced = cur
                .as_ref()
                .is_some_and(|c| c.runs.last().is_some_and(|r| r.text.ends_with(' ')));
            let wide = gap > line_size.max(gl.size) * 1.6 && gap > line_gap * 2.5;
            if cur.is_some() && wide && !is_sup && (!spaced || gap > line_size * 4.0) {
                out.push(cur.take().unwrap());
            }
            let c = cur.get_or_insert_with(|| Chunk {
                x0: gl.x0,
                x1: gl.x1,
                base,
                size: line_size,
                runs: Vec::new(),
            });
            let mut text = gl.text.clone();
            // A spacing accent over the letter before it: combined.
            if let Some(comb) = text
                .chars()
                .next()
                .and_then(combining)
                .filter(|_| text.chars().count() == 1)
                && let Some(last) = c.runs.last_mut()
                && gl.x0 < prev_x1 + gl.size * 0.05
                && last
                    .text
                    .chars()
                    .last()
                    .is_some_and(|ch| ch.is_alphabetic())
            {
                last.text.push(comb);
                last.text = last.text.nfc().collect();
                prev_x1 = prev_x1.max(gl.x1);
                c.x1 = c.x1.max(gl.x1);
                continue;
            }
            let gap_needed = if is_sup { line_size * 0.35 } else { word_gap };
            if gap > gap_needed && !c.runs.is_empty() && !text.starts_with(' ') {
                let ends_space = c.runs.last().is_some_and(|r| r.text.ends_with(' '));
                if !ends_space {
                    text.insert(0, ' ');
                }
            }
            let (bold, italic) = (gl.bold, gl.italic);
            match c.runs.last_mut() {
                Some(r) if r.bold == bold && r.italic == italic && r.sup == is_sup => {
                    r.text.push_str(&text)
                }
                _ => {
                    // A style change after a space: the space goes before.
                    let (lead, rest) = match text.strip_prefix(' ') {
                        Some(r) => (" ", r.to_string()),
                        None => ("", text),
                    };
                    if !lead.is_empty()
                        && let Some(r) = c.runs.last_mut()
                    {
                        r.text.push(' ');
                    }
                    c.runs.push(Run {
                        text: rest,
                        bold,
                        italic,
                        sup: is_sup,
                        page: 0,
                    });
                }
            }
            prev_x1 = prev_x1.max(gl.x1);
            c.x1 = c.x1.max(gl.x1);
        }
        if let Some(c) = cur {
            out.push(c);
        }
    }
    // Spaces drawn as letters are kept once (a run's edges too: they part
    // it from its neighbours).
    for c in &mut out {
        for r in &mut c.runs {
            let mut t = String::with_capacity(r.text.len());
            for ch in r.text.chars() {
                if ch.is_whitespace() {
                    if !t.ends_with(' ') {
                        t.push(' ');
                    }
                } else {
                    t.push(ch);
                }
            }
            r.text = t;
        }
        // A space at both sides of a style change: once.
        for i in 1..c.runs.len() {
            if c.runs[i - 1].text.ends_with(' ') && c.runs[i].text.starts_with(' ') {
                c.runs[i].text.remove(0);
            }
        }
        c.runs.retain(|r| !r.text.is_empty());
        if let Some(f) = c.runs.first_mut() {
            f.text = f.text.trim_start().to_string();
        }
        if let Some(l) = c.runs.last_mut() {
            l.text = l.text.trim_end().to_string();
        }
    }
    out.retain(|c| !c.text().trim().is_empty());
    out
}

fn median(v: impl Iterator<Item = f32>) -> f32 {
    let mut v: Vec<f32> = v.collect();
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f32::total_cmp);
    v[v.len() / 2]
}

/// A big letter that starts a paragraph (a drop cap) joins its text.
fn join_drop_caps(chunks: &mut Vec<Chunk>, body: f32) {
    let mut i = 0;
    while i < chunks.len() {
        let c = &chunks[i];
        let t = c.text();
        if t.chars().count() <= 2
            && t.chars().next().is_some_and(|ch| ch.is_alphabetic())
            && c.size >= body * 1.9
        {
            let (top, base, x1) = (c.top(), c.base, c.x1);
            // The first line beside it, at its top.
            let target = chunks
                .iter()
                .enumerate()
                .filter(|(j, o)| {
                    *j != i
                        && o.x0 >= x1 - body * 0.2
                        && o.x0 <= x1 + body * 2.0
                        && o.base > top
                        && o.base <= base + body * 0.3
                })
                .min_by(|a, b| a.1.base.total_cmp(&b.1.base))
                .map(|(j, _)| j);
            if let Some(j) = target {
                let letter = chunks[i].runs.clone();
                let x0 = chunks[i].x0;
                let o = &mut chunks[j];
                let mut runs = letter;
                for r in &mut runs {
                    r.sup = false;
                }
                if let (Some(l), Some(f)) = (runs.last_mut(), o.runs.first())
                    && l.bold == f.bold
                    && l.italic == f.italic
                {
                    l.text.push_str(&f.text);
                    o.runs.remove(0);
                }
                runs.append(&mut o.runs);
                o.runs = runs;
                o.x0 = x0;
                chunks.remove(i);
                continue;
            }
        }
        i += 1;
    }
}

/// Chunks into blocks: a chunk goes under the block above it when they
/// overlap sideways, are close and have about the same size.
fn blocks_of(mut chunks: Vec<Chunk>, pics: &[Pic], body: f32) -> Vec<Block> {
    chunks.sort_by(|a, b| a.base.total_cmp(&b.base).then(a.x0.total_cmp(&b.x0)));
    let mut blocks: Vec<Block> = Vec::new();
    for c in chunks {
        let fit = blocks.iter().rposition(|b| {
            if b.pic.is_some() {
                return false;
            }
            let last = b.lines.last().unwrap();
            let gap = c.base - last.base;
            let size = last.size.max(c.size);
            let similar = c.size / last.size > 0.8 && c.size / last.size < 1.25;
            let overlap =
                (c.x1.min(b.x1) - c.x0.max(b.x0)) / (c.x1 - c.x0).min(b.x1 - b.x0).max(1.0);
            gap > 0.0
                && gap <= size * 1.75
                && similar
                && (overlap > 0.4 || (c.x0 - b.x0).abs() < size)
        });
        match fit {
            // Only the nearest block above may take it.
            Some(i)
                if !blocks[i + 1..].iter().any(|o| {
                    let ov = c.x1.min(o.x1) - c.x0.max(o.x0);
                    ov > 0.0 && o.top < c.base && o.bottom > blocks[i].bottom
                }) =>
            {
                let b = &mut blocks[i];
                b.x0 = b.x0.min(c.x0);
                b.x1 = b.x1.max(c.x1);
                b.bottom = c.base + c.size * 0.25;
                b.lines.push(c);
            }
            _ => blocks.push(Block {
                x0: c.x0,
                x1: c.x1,
                top: c.top(),
                bottom: c.base + c.size * 0.25,
                lines: vec![c],
                pic: None,
            }),
        }
    }
    let _ = body;
    for p in pics {
        blocks.push(Block {
            lines: Vec::new(),
            x0: p.r.x0 as f32,
            x1: p.r.x1 as f32,
            top: p.r.y0 as f32,
            bottom: p.r.y1 as f32,
            pic: Some(p.name.clone()),
        });
    }
    blocks
}

/// Blocks in reading order: the page cut where nothing is drawn — first
/// into columns when an empty strip runs its whole height, else at the
/// first empty band across it — over and over.
fn reading_order(mut blocks: Vec<Block>, body: f32) -> Vec<Block> {
    if blocks.len() <= 1 {
        return blocks;
    }
    // Columns: a vertical strip no block crosses.
    let mut xs: Vec<(f32, f32)> = blocks.iter().map(|b| (b.x0, b.x1)).collect();
    xs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut cut = None;
    let mut reach = xs[0].1;
    for w in xs.iter().skip(1) {
        if w.0 > reach + body * 0.8 {
            // Both sides must hold text, not a margin note.
            cut = Some((reach + w.0) / 2.0);
            break;
        }
        reach = reach.max(w.1);
    }
    if let Some(x) = cut {
        let (left, right): (Vec<Block>, Vec<Block>) = blocks.into_iter().partition(|b| b.x1 <= x);
        if !left.is_empty() && !right.is_empty() {
            let mut out = reading_order(left, body);
            out.extend(reading_order(right, body));
            return out;
        }
        blocks = left.into_iter().chain(right).collect();
    }
    // Bands: the first empty horizontal band from the top.
    blocks.sort_by(|a, b| a.top.total_cmp(&b.top));
    let mut bottom = blocks[0].bottom;
    for i in 1..blocks.len() {
        if blocks[i].top >= bottom - body * 0.1 {
            let rest = blocks.split_off(i);
            let mut out = sort_band(blocks);
            out.extend(reading_order(rest, body));
            return out;
        }
        bottom = bottom.max(blocks[i].bottom);
    }
    sort_band(blocks)
}

/// A band no cut divides: top to bottom, left to right.
fn sort_band(mut b: Vec<Block>) -> Vec<Block> {
    b.sort_by(|a, c| a.top.total_cmp(&c.top).then(a.x0.total_cmp(&c.x0)));
    b
}

// ---------------------------------------------------------------- book

/// A page's number, header or footer text, with digits as `#`.
fn furniture_key(t: &str) -> String {
    t.chars()
        .map(|c| {
            if c.is_ascii_digit() {
                '#'
            } else {
                c.to_lowercase().next().unwrap_or(c)
            }
        })
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn is_page_number(t: &str) -> bool {
    let t = t.trim().trim_matches(|c: char| {
        c == '-' || c == '—' || c == '–' || c == '|' || c == '[' || c == ']' || c.is_whitespace()
    });
    let t = t
        .strip_prefix("Página")
        .or_else(|| t.strip_prefix("página"))
        .or_else(|| t.strip_prefix("Page"))
        .or_else(|| t.strip_prefix("Pág."))
        .unwrap_or(t)
        .trim();
    !t.is_empty()
        && t.len() <= 8
        && (t.chars().all(|c| c.is_ascii_digit())
            || t.chars()
                .all(|c| matches!(c.to_ascii_lowercase(), 'i' | 'v' | 'x' | 'l' | 'c')))
}

/// A paragraph being built.
#[derive(Default)]
struct Para {
    runs: Vec<Run>,
    size: f32,
    center: bool,
    heading: Option<usize>,
    /// Ended where text ends a sentence.
    ends: bool,
}

/// The characters that end a paragraph.
fn ends_sentence(t: &str) -> bool {
    let t = t.trim_end();
    t.ends_with(['.', '!', '?', ':', '…', '"', '”', '»', '’', ')', ';'])
}

/// A paragraph of prose that stops in the middle of a sentence: on a
/// letter, a digit or a comma, after more than a line.
fn open_end(p: &Para) -> bool {
    let t: String = p.runs.iter().map(|r| r.text.as_str()).collect();
    let t = t.trim_end();
    t.chars().count() > 80
        && t.chars()
            .last()
            .is_some_and(|c| c.is_alphanumeric() || c == ',')
}

/// The characters that end a sentence for sure (not `:` or `;`).
fn ends_strongly(t: &str) -> bool {
    let t = t.trim_end();
    t.ends_with(['.', '!', '?', '…', '"', '”', '»', '’', ')'])
}

fn starts_lower(t: &str) -> bool {
    t.trim_start()
        .chars()
        .next()
        .is_some_and(|c| c.is_lowercase())
}

/// The book's words, and the parts it writes with hyphens (`-se`, `-lhe`,
/// `sul-`, `pré-`), to tell a hyphen that belongs to a word from one
/// that only breaks it at a line's end.
#[derive(Default)]
struct Vocab {
    words: HashSet<String>,
    heads: HashSet<String>,
    tails: HashSet<String>,
}

impl Vocab {
    fn add(&mut self, w: &str) {
        let w: String = w
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '-')
            .to_lowercase();
        if w.is_empty() || w.ends_with('-') || w.starts_with('-') {
            return;
        }
        let parts: Vec<&str> = w.split('-').collect();
        if parts.len() > 1
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_alphabetic()))
        {
            self.heads.insert(parts[0].to_string());
            self.tails.insert(parts[parts.len() - 1].to_string());
        }
        if w.chars().count() >= 2 {
            self.words.insert(w);
        }
    }

    /// Whether `head-` at a line's end and `tail` on the next are one
    /// word with its hyphen.
    fn keep_hyphen(&self, head: &str, tail: &str) -> bool {
        let (h, t) = (head.to_lowercase(), tail.to_lowercase());
        if h.is_empty() || t.is_empty() {
            return false;
        }
        if self.words.contains(&format!("{h}-{t}")) {
            return true;
        }
        if self.words.contains(&format!("{h}{t}")) {
            return false;
        }
        let vowel = |c: char| "aeiouáéíóúâêôãõàü".contains(c);
        self.tails.contains(&t)
            || self.heads.contains(&h)
            || h == t
            || (h.chars().last() == t.chars().next() && h.chars().last().is_some_and(vowel))
            || (h.chars().count() >= 2
                && t.chars().count() >= 2
                && self.words.contains(&h)
                && self.words.contains(&t))
    }
}

/// Joins `next` to `p`; a word broken with a hyphen at the line's end is
/// joined back when the whole word is in the book (or no form is).
fn push_line(p: &mut Para, next: &[Run], words: &Vocab, lined: bool) {
    let mut next: Vec<Run> = next.to_vec();
    if lined && !p.runs.is_empty() {
        if let Some(l) = p.runs.last_mut() {
            l.text = l.text.trim_end().to_string();
        }
        p.runs.push(Run {
            text: "\n".into(),
            bold: false,
            italic: false,
            sup: false,
            page: 0,
        });
    } else if let (Some(last), Some(first)) = (p.runs.last_mut(), next.first_mut()) {
        let lt = last.text.trim_end();
        let broken = lt.ends_with('-')
            && !lt.ends_with("--")
            && lt[..lt.len() - 1]
                .chars()
                .last()
                .is_some_and(|c| c.is_alphabetic());
        if broken && starts_lower(&first.text) {
            let head: String = lt[..lt.len() - 1]
                .chars()
                .rev()
                .take_while(|c| c.is_alphabetic())
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let tail: String = first
                .text
                .chars()
                .take_while(|c| c.is_alphabetic())
                .collect();
            let keep_hyphen = words.keep_hyphen(&head, &tail);
            last.text = lt.to_string();
            if !keep_hyphen {
                last.text.pop();
            }
        } else if !lt.ends_with('/') {
            last.text = format!("{lt} ");
        }
    }
    for r in next.drain(..) {
        match p.runs.last_mut() {
            Some(l)
                if l.bold == r.bold
                    && l.italic == r.italic
                    && l.sup == r.sup
                    && !r.sup
                    && l.text != "\n" =>
            {
                l.text.push_str(&r.text)
            }
            _ => p.runs.push(r),
        }
    }
}

/// Reads the PDF at `path`.
pub fn read(path: &Path) -> Result<Book, String> {
    let data = Arc::new(std::fs::read(path).map_err(|e| e.to_string())?);
    let mut book = Book::new();
    book.css.push_str(PDF_CSS);
    let own = crate::ebook::read(path);
    book.meta.title = own.title.clone();
    book.meta.authors = own.authors.clone();
    book.meta.language = own.language.clone();
    book.meta.publisher = own.publisher.clone();
    book.meta.isbn = own.isbns.first().cloned().unwrap_or_default();
    book.meta.date = own.year.map(|y| y.to_string()).unwrap_or_default();
    book.meta.series = own.series.clone();
    book.meta.series_index = own.series_index.clone();

    let pages = collect(&data, &mut book)?;
    if pages.is_empty() {
        return Err("bookBroken".into());
    }
    // Text drawn invisibly (the text layer of a scanned book) counts only
    // on pages with no visible text.
    let pages: Vec<PageData> = pages
        .into_iter()
        .map(|mut p| {
            let visible = p
                .glyphs
                .iter()
                .filter(|g| !g.invisible && !g.text.trim().is_empty())
                .count();
            if visible > 20 {
                p.glyphs.retain(|g| !g.invisible);
            } else if p.glyphs.iter().any(|g| g.invisible) {
                // A scanned page with its text: the text, not the picture.
                p.pics
                    .retain(|pic| (pic.r.width() * pic.r.height()) < (p.w * p.h) as f64 * 0.5);
            }
            p
        })
        .collect();
    // Text whose fonts do not say which letters they draw comes out as
    // symbols: such a page is shown as its picture.
    let mut pages = pages;
    let mut garbled = Vec::new();
    for (i, p) in pages.iter_mut().enumerate() {
        if garbage(&p.glyphs) {
            p.glyphs.clear();
            p.pics.clear();
            garbled.push(i);
        }
    }
    let letters: usize = pages.iter().map(|p| p.glyphs.len()).sum();
    if letters < pages.len() * 30 {
        return scanned(path, book);
    }
    for i in &garbled {
        if let Some((img, ext)) = crate::convert::draw_page(&data, *i, 1600.0)
            && let Some(name) = book.add_file(img, &format!("p.{ext}"))
        {
            let (w, h) = (pages[*i].w as f64, pages[*i].h as f64);
            pages[*i].pics.push(Pic {
                name,
                r: Rect::new(0.0, 0.0, w, h),
            });
        }
    }
    // Where (nearly) everything is bold or italic, that is the font, not
    // emphasis.
    let total: usize = pages.iter().map(|p| p.glyphs.len()).sum();
    let bold = pages
        .iter()
        .flat_map(|p| p.glyphs.iter())
        .filter(|g| g.bold)
        .count();
    let italic = pages
        .iter()
        .flat_map(|p| p.glyphs.iter())
        .filter(|g| g.italic)
        .count();
    if bold * 10 > total * 6 || italic * 10 > total * 6 {
        let (nb, ni) = (bold * 10 > total * 6, italic * 10 > total * 6);
        for p in pages.iter_mut() {
            for g in p.glyphs.iter_mut() {
                if nb {
                    g.bold = false;
                }
                if ni {
                    g.italic = false;
                }
            }
        }
    }

    // The size of the body text: the one most letters have.
    let mut sizes: HashMap<i32, usize> = HashMap::new();
    for p in &pages {
        for g in &p.glyphs {
            *sizes.entry((g.size * 2.0).round() as i32).or_default() += 1;
        }
    }
    let body = sizes
        .iter()
        .max_by_key(|(_, n)| **n)
        .map(|(s, _)| *s as f32 / 2.0)
        .unwrap_or(10.0);

    // Blocks per page, in reading order.
    let mut page_blocks: Vec<Vec<Block>> = Vec::new();
    for p in &pages {
        let mut ch = chunks_of(&p.glyphs);
        join_drop_caps(&mut ch, body);
        let bl = blocks_of(ch, &p.pics, body);
        page_blocks.push(reading_order(bl, body));
    }

    for (pi, bl) in page_blocks.iter_mut().enumerate() {
        for b in bl.iter_mut() {
            for l in b.lines.iter_mut() {
                for r in l.runs.iter_mut() {
                    r.page = pi;
                }
            }
        }
    }

    // Headers, footers and page numbers: small blocks near the top or
    // bottom whose text (digits aside) repeats, or that are a number.
    let n_pages = pages.len();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (pi, bl) in page_blocks.iter().enumerate() {
        let h = pages[pi].h;
        let mut seen = HashSet::new();
        for b in bl.iter().filter(|b| {
            b.pic.is_none() && b.lines.len() <= 2 && (b.bottom < h * 0.12 || b.top > h * 0.88)
        }) {
            let k = furniture_key(&b.text());
            if seen.insert(k.clone()) {
                *counts.entry(k).or_default() += 1;
            }
        }
    }
    let repeated =
        |k: &str| counts.get(k).copied().unwrap_or(0) >= 3.max(n_pages / 5) && n_pages >= 4;
    for (pi, bl) in page_blocks.iter_mut().enumerate() {
        let h = pages[pi].h;
        bl.retain(|b| {
            if b.pic.is_some() || b.lines.len() > 2 || !(b.bottom < h * 0.12 || b.top > h * 0.88) {
                return true;
            }
            let t = b.text();
            !(is_page_number(&t) || repeated(&furniture_key(&t)))
        });
    }

    // The width of a full line of text (of a column, in two columns).
    let col_width = {
        let mut w: Vec<f32> = page_blocks
            .iter()
            .flatten()
            .flat_map(|b| b.lines.iter())
            .filter(|l| (l.size - body).abs() < body * 0.1)
            .map(|l| l.x1 - l.x0)
            .collect();
        w.sort_by(f32::total_cmp);
        w.get(w.len() * 9 / 10).copied().unwrap_or(400.0)
    };

    // Words of the book, for hyphens at line ends.
    let mut words = Vocab::default();
    for bl in &page_blocks {
        for b in bl {
            for l in &b.lines {
                for w in l.text().split_whitespace() {
                    words.add(w);
                }
            }
        }
    }

    // Heading sizes: bigger than the body, from the biggest down.
    let mut hsizes: Vec<f32> = Vec::new();
    for bl in &page_blocks {
        for b in bl {
            let s = b.size();
            if b.pic.is_none()
                && s >= body * 1.15
                && b.lines.len() <= 4
                && b.text().chars().count() < 200
            {
                hsizes.push((s * 2.0).round() / 2.0);
            }
        }
    }
    hsizes.sort_by(|a, b| b.total_cmp(a));
    hsizes.dedup_by(|a, b| (*a - *b).abs() < body * 0.08);
    hsizes.truncate(3);
    let level_of = |s: f32| -> Option<usize> {
        if s < body * 1.15 {
            return None;
        }
        hsizes
            .iter()
            .position(|h| (s - h).abs() < body * 0.08 || s > *h)
            .map(|i| i + 1)
    };

    // Bookmarks: chapter titles and where they start.
    let outline = outline_of(&data);

    // The note marks each page raises (small, above the line).
    let marks: Vec<HashSet<String>> = page_blocks
        .iter()
        .map(|bl| {
            bl.iter()
                .flat_map(|b| b.lines.iter())
                .flat_map(|l| l.runs.iter())
                .filter(|r| r.sup)
                .map(|r| r.text.trim().to_string())
                .filter(|t| !t.is_empty() && t.len() <= 3)
                .collect()
        })
        .collect();

    // Whether the book starts its paragraphs indented: many lines inside
    // blocks start further in than the block.
    let indents = {
        let (mut ind, mut all) = (0usize, 0usize);
        for bl in &page_blocks {
            for b in bl.iter().filter(|b| b.pic.is_none() && b.lines.len() >= 3) {
                let size = b.size();
                for l in &b.lines[1..] {
                    all += 1;
                    if l.x0 > b.x0 + size * 0.6 && l.x0 < b.x0 + size * 4.0 {
                        ind += 1;
                    }
                }
            }
        }
        all >= 50 && ind * 20 >= all
    };

    // Paragraphs.
    let mut paras: Vec<(usize, Para)> = Vec::new();
    let mut notes: Vec<(usize, String, Vec<Run>)> = Vec::new();
    let mut pending: Option<Para> = None;
    let mut pics_at: Vec<(usize, usize, String)> = Vec::new();
    for (pi, bl) in page_blocks.iter().enumerate() {
        let (pw, ph) = (pages[pi].w, pages[pi].h);
        // The text area: where most lines start and end.
        let left = median(bl.iter().filter(|b| b.pic.is_none()).map(|b| b.x0));
        let right = median(bl.iter().filter(|b| b.pic.is_none()).map(|b| b.x1));
        for b in bl {
            if let Some(name) = &b.pic {
                if let Some(p) = pending.take() {
                    paras.push((pi, p));
                }
                pics_at.push((paras.len(), pi, name.clone()));
                continue;
            }
            let bsize = b.size();
            // Notes at the foot: smaller letters, low on the page, starting
            // with a number or mark that the page's text has raised.
            let first = b.lines[0].text();
            let mark_of = |t: &str| -> String {
                t.chars()
                    .take_while(|c| c.is_ascii_digit() || matches!(c, '*' | '†' | '‡'))
                    .collect()
            };
            let mark = mark_of(&first);
            if bsize < body * 0.92
                && b.top > ph * 0.55
                && !mark.is_empty()
                && mark.len() <= 3
                && marks[pi].contains(&mark)
            {
                // One note per line starting with a mark the page raised.
                let mut cur: Option<(String, Para)> = None;
                for l in &b.lines {
                    let m = mark_of(&l.text());
                    if !m.is_empty() && marks[pi].contains(&m) {
                        if let Some((m, p)) = cur.take() {
                            notes.push((pi, m, p.runs));
                        }
                        let mut p = Para {
                            runs: l.runs.clone(),
                            ..Default::default()
                        };
                        if let Some(f) = p.runs.first_mut() {
                            f.text = f.text[m.len()..]
                                .trim_start_matches(['.', ')', ' '])
                                .to_string();
                        }
                        cur = Some((m, p));
                    } else if let Some((_, p)) = cur.as_mut() {
                        push_line(p, &l.runs, &words, false);
                    }
                }
                if let Some((m, p)) = cur.take() {
                    notes.push((pi, m, p.runs));
                }
                continue;
            }
            let level =
                level_of(bsize).filter(|_| b.lines.len() <= 4 && b.text().chars().count() < 200);
            let justified = b.lines.len() >= 3 && {
                let full = b.lines.iter().filter(|l| l.x1 > b.x1 - bsize * 1.2).count();
                full * 2 >= b.lines.len()
            };
            // Lines kept as they are (a list, a poem, an address): most of
            // them well short of a full line, and not justified.
            let lined = !justified
                && b.lines.len() >= 2
                && b.lines
                    .iter()
                    .filter(|l| l.x1 - l.x0 < col_width * 0.72)
                    .count()
                    * 10
                    >= b.lines.len() * 7;
            let typical = {
                let gaps: Vec<f32> = b.lines.windows(2).map(|w| w[1].base - w[0].base).collect();
                median(gaps.into_iter())
            };
            // A line bigger than the next, or bold over plain lines, that
            // does not end a sentence, after one that does (or first): a
            // heading over its paragraph.
            let heading_line = |li: usize| -> bool {
                let l0 = &b.lines[li];
                let Some(next) = b.lines.get(li + 1) else {
                    return false;
                };
                let t0 = l0.text();
                let bold = |l: &Chunk| l.runs.iter().all(|r| r.bold || r.text.trim().is_empty());
                level.is_none()
                    && (li == 0 || ends_sentence(&b.lines[li - 1].text()))
                    && !ends_strongly(&t0)
                    && !t0.trim_end().ends_with([',', ';', '-'])
                    && t0.chars().count() < 150
                    && !starts_lower(&next.text())
                    && (l0.size > next.size * 1.045 || (bold(l0) && !bold(next)))
            };
            let mut after_heading = false;
            for (li, l) in b.lines.iter().enumerate() {
                let t = l.text();
                if heading_line(li) {
                    if let Some(p) = pending.take() {
                        paras.push((pi, p));
                    }
                    paras.push((
                        pi,
                        Para {
                            runs: l.runs.clone(),
                            size: l.size,
                            heading: Some(level_of(l.size).unwrap_or(LOWEST)),
                            ..Default::default()
                        },
                    ));
                    after_heading = true;
                    continue;
                }
                if after_heading {
                    after_heading = false;
                    pending = Some(Para {
                        size: bsize,
                        ..Default::default()
                    });
                    let p = pending.as_mut().unwrap();
                    p.runs = l.runs.clone();
                    p.ends = ends_sentence(&t);
                    continue;
                }
                let centered = (l.x0 - left) > bsize * 2.0
                    && ((l.x0 - left) - (right - l.x1)).abs() < bsize * 1.5
                    && (l.x1 - l.x0) < (right - left) * 0.8;
                let indented = l.x0 > b.x0 + bsize * 0.6;
                let new_para = if li == 0 {
                    // A block's start: a new paragraph, unless the last one
                    // goes on (cut by a page or column).
                    // In a book whose paragraphs start indented, a block
                    // starting at the margin goes on the paragraph before.
                    let at_margin = (l.x0 - left).abs() < bsize * 0.6;
                    !matches!(&pending, Some(p) if level.is_none()
                        && p.heading.is_none()
                        && !p.center
                        && !indented
                        && !centered
                        // Same letter size, or stopped right in a word's
                        // place (small capitals go on in smaller letters).
                        && ((p.size - bsize).abs() < body * 0.15
                            || p.runs.last().is_some_and(|r| r.text.trim_end().chars().last().is_some_and(|c| c.is_alphanumeric())))
                        && ((indents && at_margin && !p.runs.is_empty())
                            || !p.runs.last().is_some_and(|r| ends_strongly(&r.text))
                                && (starts_lower(&t)
                                    || p.runs.last().is_some_and(|r| r.text.trim_end().ends_with('-'))
                                    // A long paragraph stopped without any
                                    // mark (by the page's end): it goes on.
                                    || open_end(p))))
                } else {
                    let prev = &b.lines[li - 1];
                    let gap = l.base - prev.base;
                    let short = justified && prev.x1 < b.x1 - bsize * 1.5;
                    // A line that does not end a sentence, followed by one
                    // starting in lower case: the same paragraph.
                    let pt = prev.text();
                    let goes_on = !ends_strongly(&pt)
                        && (starts_lower(&t)
                            // Stopped on a word or a comma: the sentence
                            // goes on (a name in capitals, a year).
                            || (prev.x1 > b.x1 - bsize * 1.5
                                && pt.trim_end().chars().last().is_some_and(|c| c.is_alphabetic() || c == ',')));
                    level.is_none()
                        && !goes_on
                        && (indented
                            && !b.lines[li - 1..].iter().all(|x| x.x0 > b.x0 + bsize * 0.6)
                            || short && ends_sentence(&prev.text())
                            || gap > typical * 1.45 && typical > 0.0
                            || centered)
                };
                if new_para {
                    if let Some(p) = pending.take() {
                        paras.push((pi, p));
                    }
                    pending = Some(Para {
                        size: bsize,
                        center: centered && level.is_none(),
                        heading: level,
                        ..Default::default()
                    });
                }
                let p = pending.as_mut().unwrap();
                if p.runs.is_empty() {
                    p.runs = l.runs.clone();
                } else {
                    push_line(p, &l.runs, &words, lined);
                }
                p.ends = ends_sentence(&t);
            }
            // A heading or a centred line ends with its block.
            if pending
                .as_ref()
                .is_some_and(|p| p.heading.is_some() || p.center)
                && let Some(p) = pending.take()
            {
                paras.push((pi, p));
            }
            let _ = pw;
        }
    }
    if let Some(p) = pending.take() {
        paras.push((page_blocks.len().saturating_sub(1), p));
    }

    // A bold short line alone, at the body's size, after bigger headings
    // exist: the lowest heading level.
    let bold_level = hsizes.len() + 1;
    for (_, p) in paras.iter_mut() {
        if p.heading == Some(LOWEST) {
            p.heading = Some(bold_level.min(6));
        }
    }
    for (_, p) in paras.iter_mut() {
        let t: String = p.runs.iter().map(|r| r.text.as_str()).collect();
        if p.heading.is_none()
            && p.runs.iter().all(|r| r.bold || r.text.trim().is_empty())
            && t.chars().count() < 90
            && !t.trim_end().ends_with(['.', ','])
            && !t.trim().is_empty()
        {
            p.heading = Some(bold_level.min(6));
        }
    }

    // Bookmarks pick the chapter headings: on its page, the first
    // paragraph whose text is the bookmark's.
    let mut toc_marks: Vec<(usize, usize, String)> = Vec::new();
    if outline.len() >= 2 {
        let norm = |s: &str| -> String {
            s.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(|c| c.to_lowercase())
                .collect()
        };
        // The bookmarks' levels become the headings' levels.
        for (level, title, page) in &outline {
            let want = norm(title);
            if want.is_empty() {
                continue;
            }
            let hit = paras.iter().position(|(pi, p)| {
                *pi + 1 >= *page && *pi <= *page + 1 && {
                    let t: String = p.runs.iter().map(|r| r.text.as_str()).collect();
                    let n = norm(&t);
                    !n.is_empty()
                        && n.len() < want.len() * 2 + 10
                        && (n.contains(&want) || want.contains(&n))
                }
            });
            if let Some(i) = hit {
                // The title starts a longer paragraph (no break after it
                // in the PDF): the title alone is the heading.
                let p = &paras[i].1;
                let t: String = p.runs.iter().map(|r| r.text.as_str()).collect();
                let n = norm(&t);
                if n.starts_with(&want) && n.len() > want.len() + 12 {
                    let (head, rest) = split_runs(&p.runs, want.chars().count());
                    let pi = paras[i].0;
                    let size = p.size;
                    paras[i].1.runs = rest;
                    paras.insert(
                        i,
                        (
                            pi,
                            Para {
                                runs: head,
                                size,
                                heading: Some((*level).clamp(1, 6)),
                                ..Default::default()
                            },
                        ),
                    );
                } else {
                    paras[i].1.heading = Some((*level).clamp(1, 6));
                }
            } else {
                toc_marks.push((*page, *level, title.clone()));
            }
        }
    }

    // Write the paragraphs as nodes; notes of each page after its text
    // (collected at the end of each chapter).
    let mut nodes: Vec<Node> = Vec::new();
    let note_key = |pi: usize, m: &str| format!("fn{}_{}", pi + 1, m.replace(['*', '†', '‡'], "x"));
    let note_ids: HashSet<String> = notes.iter().map(|(pi, m, _)| note_key(*pi, m)).collect();
    let split_level = paras.iter().filter_map(|(_, p)| p.heading).min();
    let mut chapter_notes: Vec<Node> = Vec::new();
    let flush_notes = |nodes: &mut Vec<Node>, chapter_notes: &mut Vec<Node>| {
        if !chapter_notes.is_empty() {
            let mut sec = Elem::new("section");
            sec.set("class", "notes");
            sec.set("epub:type", "footnotes");
            sec.kids = std::mem::take(chapter_notes);
            nodes.push(Node::Elem(sec));
        }
    };
    let mut notes_by_page: HashMap<usize, Vec<(String, Vec<Run>)>> = HashMap::new();
    for (pi, m, runs) in notes {
        notes_by_page.entry(pi).or_default().push((m, runs));
    }
    let mut last_page = usize::MAX;
    let mut pic_i = 0;
    let mut mark_i = 0;
    toc_marks.sort_by_key(|m| m.0);
    for (idx, (pi, p)) in paras.into_iter().enumerate() {
        while pic_i < pics_at.len() && pics_at[pic_i].0 <= idx {
            let mut d = Elem::new("div");
            d.set("class", "center");
            d.push(Node::Elem(
                Elem::new("img")
                    .with("src", pics_at[pic_i].2.clone())
                    .with("alt", ""),
            ));
            nodes.push(Node::Elem(d));
            pic_i += 1;
        }
        if pi != last_page {
            if last_page != usize::MAX
                && let Some(ns) = notes_by_page.remove(&last_page)
            {
                for (m, runs) in ns {
                    chapter_notes.push(Node::Elem(note_node(&note_key(last_page, &m), &m, &runs)));
                }
            }
            // Bookmarks that matched no heading: a heading of their own.
            while mark_i < toc_marks.len() && toc_marks[mark_i].0 <= pi + 1 {
                let (_, level, title) = &toc_marks[mark_i];
                if Some(*level) <= split_level.or(Some(*level)) {
                    flush_notes(&mut nodes, &mut chapter_notes);
                }
                let mut h = Elem::new(&format!("h{}", (*level).clamp(1, 6)));
                h.set("class", "mark");
                h.text(title.clone());
                nodes.push(Node::Elem(h));
                mark_i += 1;
            }
            last_page = pi;
        }
        if p.heading.is_some_and(|l| Some(l) <= split_level) {
            flush_notes(&mut nodes, &mut chapter_notes);
        }
        let tag = match p.heading {
            Some(l) => format!("h{}", l.clamp(1, 6)),
            None => "p".into(),
        };
        let mut e = Elem::new(&tag);
        if p.center && p.heading.is_none() {
            e.set("class", "center");
        }
        for r in &p.runs {
            if r.sup {
                let m = r.text.trim();
                let id = note_key(r.page, m);
                if note_ids.contains(&id) {
                    let mut a = Elem::new("a")
                        .with("href", format!("#{id}"))
                        .with("id", format!("r{id}"))
                        .with("class", "noteref")
                        .with("epub:type", "noteref");
                    a.text(m.to_string());
                    if r.text.starts_with(' ') {
                        e.text(" ");
                    }
                    e.push(Node::Elem(a));
                    if r.text.ends_with(' ') {
                        e.text(" ");
                    }
                    continue;
                }
            }
            push_run(&mut e, r);
        }
        if !e.plain().trim().is_empty() || p.heading.is_none() && !e.kids.is_empty() {
            nodes.push(Node::Elem(e));
        }
    }
    while pic_i < pics_at.len() {
        let mut d = Elem::new("div");
        d.set("class", "center");
        d.push(Node::Elem(
            Elem::new("img")
                .with("src", pics_at[pic_i].2.clone())
                .with("alt", ""),
        ));
        nodes.push(Node::Elem(d));
        pic_i += 1;
    }
    if let Some(ns) = notes_by_page.remove(&last_page) {
        for (m, runs) in ns {
            chapter_notes.push(Node::Elem(note_node(&note_key(last_page, &m), &m, &runs)));
        }
    }
    flush_notes(&mut nodes, &mut chapter_notes);

    // A bookmark's heading put in where its title was not found, with a
    // heading right after it: that one is the chapter's.
    let is_heading =
        |n: &Node| matches!(n, Node::Elem(e) if e.name.len() == 2 && e.name.starts_with('h'));
    let mut i = 0;
    while i < nodes.len() {
        let mark = matches!(&nodes[i], Node::Elem(e) if e.attr("class") == Some("mark"));
        if mark && nodes.get(i + 1).is_some_and(is_heading) {
            nodes.remove(i);
            continue;
        }
        if let Node::Elem(e) = &mut nodes[i]
            && e.attr("class") == Some("mark")
        {
            e.attrs.retain(|(k, _)| k != "class");
        }
        i += 1;
    }
    if nodes.is_empty() {
        return scanned(path, book);
    }
    let keep = |h: &str, _pic: bool| Some(h.to_string());
    super::text_in::add_document(&mut book, nodes, &keep, true);
    // The first picture of the first page is the cover, when the page is
    // mostly it.
    if book.cover.is_none()
        && let Some(first) = pages.first()
        && let Some(p) = first
            .pics
            .iter()
            .find(|p| p.r.width() * p.r.height() > (first.w * first.h) as f64 * 0.5)
    {
        book.cover = Some(p.name.clone());
    }
    super::fix_links(&mut book);
    Ok(book)
}

/// Runs cut after `n` letters and digits: (before, after).
fn split_runs(runs: &[Run], n: usize) -> (Vec<Run>, Vec<Run>) {
    let mut head = Vec::new();
    let mut tail = Vec::new();
    let mut count = 0;
    for r in runs {
        if count >= n {
            tail.push(r.clone());
            continue;
        }
        let mut cut = r.text.len();
        for (i, c) in r.text.char_indices() {
            if count >= n {
                cut = i;
                break;
            }
            if c.is_alphanumeric() {
                count += c.to_lowercase().count();
            }
        }
        // Up to the end of the word and its closing marks.
        let rest = &r.text[cut..];
        let more = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
        let (a, b) = r.text.split_at(cut + more.min(rest.len()));
        let a = a.trim_end_matches(|c: char| {
            c == ':' || c == '.' || c == '–' || c == '—' || c.is_whitespace()
        });
        if !a.is_empty() {
            head.push(Run {
                text: a.to_string(),
                ..r.clone()
            });
        }
        if !b.trim().is_empty() {
            tail.push(Run {
                text: b.trim_start().to_string(),
                ..r.clone()
            });
        }
    }
    (head, tail)
}

fn push_run(e: &mut Elem, r: &Run) {
    if r.text.is_empty() {
        return;
    }
    if r.text == "\n" {
        e.push(Node::Elem(Elem::new("br")));
        return;
    }
    let mut node = Node::Text(r.text.clone());
    for (on, tag) in [(r.sup, "sup"), (r.italic, "em"), (r.bold, "strong")] {
        if on {
            let mut x = Elem::new(tag);
            x.push(node);
            node = Node::Elem(x);
        }
    }
    // Spaces at a styled run's edges go outside it.
    match &mut node {
        Node::Elem(x) => {
            let lead = r.text.starts_with(' ');
            let trail = r.text.ends_with(' ') && r.text.trim().len() < r.text.len();
            fn trim_inner(x: &mut Elem) {
                for k in x.kids.iter_mut() {
                    match k {
                        Node::Text(t) => *t = t.trim().to_string(),
                        Node::Elem(y) => trim_inner(y),
                    }
                }
            }
            trim_inner(x);
            if lead {
                e.text(" ");
            }
            e.push(node);
            if trail {
                e.text(" ");
            }
        }
        Node::Text(t) => e.text(t.clone()),
    }
}

fn note_node(id: &str, mark: &str, runs: &[Run]) -> Elem {
    let mut aside = Elem::new("aside")
        .with("id", id)
        .with("class", "note")
        .with("epub:type", "footnote");
    let mut p = Elem::new("p");
    let mut back = Elem::new("a")
        .with("href", format!("#r{id}"))
        .with("class", "back");
    back.text(format!("{mark}."));
    p.push(Node::Elem(back));
    p.text(" ");
    for r in runs {
        push_run(&mut p, r);
    }
    aside.push(Node::Elem(p));
    aside
}

/// A heading whose level is the lowest (set once the levels are known).
const LOWEST: usize = 99;

const PDF_CSS: &str = "
.noteref { vertical-align: super; font-size: 0.75em; line-height: 0; text-decoration: none; }
.back { text-decoration: none; }
.notes h2 { font-size: 1em; }
";

/// The PDF's bookmarks: (level, title, page number from 1).
fn outline_of(data: &[u8]) -> Vec<(usize, String, usize)> {
    let Ok(doc) = lopdf::Document::load_mem(data) else {
        return Vec::new();
    };
    match doc.get_toc() {
        Ok(t) => t
            .toc
            .into_iter()
            .map(|e| {
                (
                    e.level,
                    crate::text::collapse_spaces(e.title.trim()),
                    e.page,
                )
            })
            .filter(|(_, t, _)| !t.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// A book without text: each page a picture, in a book of fixed pages.
fn scanned(path: &Path, mut book: Book) -> Result<Book, String> {
    book.files.clear();
    book.chapters.clear();
    book.toc.clear();
    book.css = super::BASE_CSS.into();
    book.fixed = true;
    let pages = crate::convert::pdf_pictures(path)?;
    if pages.is_empty() {
        return Err("bookBroken".into());
    }
    for (i, (data, ext)) in pages.into_iter().enumerate() {
        let (w, h) = image::ImageReader::new(std::io::Cursor::new(&data))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_dimensions().ok())
            .unwrap_or((1200, 1800));
        let Some(name) = book.add_file(data, &format!("p.{ext}")) else {
            continue;
        };
        if i == 0 {
            book.cover = Some(name.clone());
        }
        let ch = Chapter {
            name: book.next_name(),
            title: String::new(),
            body: super::epub_out::picture_page(&name),
            lang: String::new(),
            page: Some((w, h)),
        };
        book.chapters.push(ch);
    }
    // The bookmarks still lead to their pages.
    if let Ok(data) = std::fs::read(path) {
        for (level, title, page) in outline_of(&data) {
            if level == 1
                && let Some(c) = book.chapters.get(page.saturating_sub(1))
            {
                book.toc.push(super::Toc {
                    title,
                    href: c.name.clone(),
                    kids: vec![],
                });
            }
        }
    }
    Ok(book)
}

/// For tests: a page's chunks and blocks, with their places.
#[doc(hidden)]
pub fn dump_page(path: &Path, page: usize) -> String {
    let Ok(data) = std::fs::read(path) else {
        return String::new();
    };
    let data = Arc::new(data);
    let mut book = Book::new();
    let Ok(pages) = collect(&data, &mut book) else {
        return String::new();
    };
    let mut sizes: HashMap<i32, usize> = HashMap::new();
    for p in &pages {
        for g in &p.glyphs {
            *sizes.entry((g.size * 2.0).round() as i32).or_default() += 1;
        }
    }
    let body = sizes
        .iter()
        .max_by_key(|(_, n)| **n)
        .map(|(s, _)| *s as f32 / 2.0)
        .unwrap_or(10.0);
    let Some(p) = pages.get(page) else {
        return String::new();
    };
    let mut ch = chunks_of(&p.glyphs);
    join_drop_caps(&mut ch, body);
    let bl = reading_order(blocks_of(ch, &p.pics, body), body);
    let mut s = format!("page {page} {}x{} body={body}\n", p.w, p.h);
    for (i, b) in bl.iter().enumerate() {
        s.push_str(&format!(
            "block {i} x={:.0}-{:.0} y={:.0}-{:.0} size={} pic={:?}\n",
            b.x0,
            b.x1,
            b.top,
            b.bottom,
            b.size(),
            b.pic
        ));
        for l in &b.lines {
            s.push_str(&format!(
                "   x={:6.1}-{:6.1} base={:6.1} sz={:4.1} | {}\n",
                l.x0,
                l.x1,
                l.base,
                l.size,
                l.text()
            ));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Object, Stream, dictionary};

    /// A PDF of pages, each a list of (x, y from the top, size, bold, text).
    type Line<'a> = (f32, f32, f32, bool, &'a str);

    fn pdf_of(pages: &[Vec<Line>]) -> Vec<u8> {
        let mut doc = lopdf::Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let regular = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Times-Roman", "Encoding" => "WinAnsiEncoding",
        });
        let bold = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Times-Bold", "Encoding" => "WinAnsiEncoding",
        });
        let mut kids = Vec::new();
        for lines in pages {
            let mut ops = String::new();
            for (x, y, size, b, text) in lines {
                let (enc, _, _) = encoding_rs::WINDOWS_1252.encode(text);
                let esc: String = enc
                    .iter()
                    .map(|c| match c {
                        b'(' | b')' | b'\\' => format!("\\{}", *c as char),
                        c if *c < 128 => (*c as char).to_string(),
                        c => format!("\\{:o}", c),
                    })
                    .collect();
                ops.push_str(&format!(
                    "BT /{} {size} Tf {x} {} Td ({esc}) Tj ET\n",
                    if *b { "F2" } else { "F1" },
                    792.0 - y
                ));
            }
            let content = doc.add_object(Stream::new(dictionary! {}, ops.into_bytes()));
            let page = doc.add_object(dictionary! {
                "Type" => "Page", "Parent" => pages_id, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Contents" => content,
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => regular, "F2" => bold } },
            });
            kids.push(Object::Reference(page));
        }
        let count = kids.len() as i64;
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    #[test]
    fn text_flows_again() {
        let mut pages = Vec::new();
        for n in 1..=4 {
            let mut l: Vec<Line> = vec![(250.0, 40.0, 9.0, false, "A Casa Vazia")];
            match n {
                1 => {
                    l.push((200.0, 120.0, 20.0, true, "Capítulo Um"));
                    l.push((
                        72.0,
                        170.0,
                        11.0,
                        false,
                        "Era uma vez uma casa muito antiga que ficava no alto de uma",
                    ));
                    l.push((
                        72.0,
                        184.0,
                        11.0,
                        false,
                        "colina, longe de tudo e de todos, onde o vento sempre so-",
                    ));
                    l.push((
                        72.0,
                        198.0,
                        11.0,
                        false,
                        "prava forte durante as noites mais frias do longo inverno.",
                    ));
                    l.push((
                        90.0,
                        212.0,
                        11.0,
                        false,
                        "Outro parágrafo começa aqui com um recuo e segue por",
                    ));
                    l.push((
                        72.0,
                        226.0,
                        11.0,
                        false,
                        "mais uma linha completa que chega até o fim da página e",
                    ));
                }
                2 => {
                    l.push((
                        72.0,
                        100.0,
                        11.0,
                        false,
                        "continua na página seguinte sem parar no meio da frase.",
                    ));
                    l.push((
                        90.0,
                        114.0,
                        11.0,
                        false,
                        "Uma nota aparece aqui no texto deste parágrafo final",
                    ));
                    l.push((341.0, 110.0, 7.0, false, "1"));
                    l.push((72.0, 128.0, 11.0, false, "que termina nesta linha curta."));
                    l.push((
                        72.0,
                        700.0,
                        8.0,
                        false,
                        "1 Esta é a nota de rodapé da página.",
                    ));
                }
                _ => {
                    l.push((
                        200.0,
                        120.0,
                        20.0,
                        true,
                        if n == 3 {
                            "Capítulo Dois"
                        } else {
                            "Capítulo Três"
                        },
                    ));
                    l.push((
                        72.0,
                        170.0,
                        11.0,
                        false,
                        "Texto curto do capítulo, que termina aqui.",
                    ));
                }
            }
            l.push((
                300.0,
                760.0,
                9.0,
                false,
                Box::leak(n.to_string().into_boxed_str()),
            ));
            pages.push(l);
        }
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.pdf");
        std::fs::write(&p, pdf_of(&pages)).unwrap();
        let b = read(&p).unwrap();
        let all: String = b.chapters.iter().map(|c| c.body.clone()).collect();
        assert_eq!(
            b.chapters.len(),
            3,
            "{:?}",
            b.chapters
                .iter()
                .map(|c| (&c.name, &c.title, c.body.len()))
                .collect::<Vec<_>>()
        );
        let c1 = &b.chapters[0].body;
        assert!(c1.starts_with("<h1>Capítulo Um</h1>"), "{c1}");
        assert!(c1.contains("o vento sempre soprava forte"), "{c1}");
        assert!(
            c1.contains("até o fim da página e continua na página seguinte"),
            "{c1}"
        );
        assert!(!all.contains("A Casa Vazia"), "header left out: {all}");
        assert!(!all.contains(">1<") || all.contains("noteref"), "{all}");
        assert!(c1.contains("epub:type=\"noteref\""), "{c1}");
        assert!(c1.contains("Esta é a nota de rodapé da página."), "{c1}");
        assert!(c1.contains("<p>Outro parágrafo"), "{c1}");
        assert!(c1.contains("epub:type=\"noteref\">1</a> que"), "{c1}");
        assert_eq!(b.toc.len(), 3);
    }
}
