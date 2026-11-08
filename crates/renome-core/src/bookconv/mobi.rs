//! Kindle books read into a [`Book`]: the old format (MOBI, PRC, AZW) and
//! the newer one (KF8, in AZW3 files or after an old part in the same
//! file), and plain PalmDOC texts.
//!
//! An old Kindle book is one stream of HTML 3 with its own marks: links
//! name places by their byte in the text (`filepos`), pictures by their
//! record (`recindex`), page breaks are `<mbp:pagebreak/>`. A KF8 book is
//! its EPUB taken apart: each file is a skeleton with fragments to put
//! back in it (told by two indexes), its style sheets are other flows of
//! the text, links and pictures use `kindle:` addresses. Both are put
//! back together here. Fonts embedded in KF8 books are left out.

use std::collections::HashMap;
use std::path::Path;

use crate::pages::{Records, be16, be32, palmdoc, part, without_trailing};

use super::text_in::{add_document_with, decode_bytes, load_pictures};
use super::xhtml::{self, Out};
use super::{Book, Chapter, Toc};

/// The text records of a part, uncompressed, as bytes.
fn text_bytes(
    r: &Records,
    at: usize,
    count: usize,
    compression: u16,
    trailing: u16,
    huff: Option<&Huff>,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for i in at + 1..=at + count {
        let rec = r.read(i, 1 << 20).ok_or("bookBroken")?;
        let rec = without_trailing(&rec, trailing);
        match (compression, huff) {
            (1, _) => out.extend_from_slice(rec),
            (2, _) => palmdoc(rec, &mut out),
            (17480, Some(h)) => h.unpack(rec, &mut out, 0)?,
            _ => return Err("bookFormat".into()),
        }
    }
    Ok(out)
}

/// The details of a Kindle book.
fn details(path: &Path, book: &mut Book) {
    let own = crate::ebook::read(path);
    let m = &mut book.meta;
    m.title = own.title;
    m.authors = own.authors;
    m.language = own.language;
    m.publisher = own.publisher;
    m.isbn = own.isbns.first().cloned().unwrap_or_default();
    m.date = own.year.map(|y| y.to_string()).unwrap_or_default();
    m.series = own.series;
    m.series_index = own.series_index;
}

/// Picture record `i` (a large one is wrapped in a `CRES` record).
fn picture(r: &Records, i: usize) -> Option<Vec<u8>> {
    let d = r.read(i, 64 << 20)?;
    let d = if d.starts_with(b"CRES") {
        d.get(12..)?.to_vec()
    } else {
        d
    };
    super::media_of(&d, "")
        .filter(|(m, _)| m.starts_with("image/"))
        .map(|_| d.clone())
        .or_else(|| image::guess_format(&d).ok().map(|_| d))
}

pub fn read(path: &Path) -> Result<Book, String> {
    let r = Records::open(path).map_err(|_| "bookBroken".to_string())?;
    let Some(p0) = part(&r, 0) else {
        // A plain PalmDOC text.
        let h = r.read(0, 16).ok_or("bookBroken")?;
        let (comp, count) = (be16(&h, 0).unwrap_or(1), be16(&h, 8).unwrap_or(0) as usize);
        if be16(&h, 12).unwrap_or(0) != 0 {
            return Err("bookDrm".into());
        }
        let bytes = text_bytes(&r, 0, count, comp, 0, None)?;
        let mut book = super::text_in::txt(&bytes)?;
        details(path, &mut book);
        return Ok(book);
    };
    if p0.encrypted {
        return Err("bookDrm".into());
    }
    let mut book = Book::new();
    details(path, &mut book);
    // A file with both parts: the newer one, when it reads.
    let kf8_at = if p0.kf8 {
        Some(0)
    } else {
        p0.boundary.map(|b| b + 1)
    };
    if let Some(at) = kf8_at
        && let Some(p) = part(&r, at)
    {
        // Pages of pictures laid out as they are (a comic), and their
        // direction.
        if let Some(h) = r.read(at, 64 << 10) {
            book.fixed = exth(&h, 122).is_some_and(|v| v == b"true");
            book.rtl = exth(&h, 527).is_some_and(|v| v == b"rtl");
        }
        match kf8(&r, &p, at, &mut book) {
            Ok(()) => return finish(&r, book, &p),
            Err(e) if p0.kf8 => return Err(e),
            Err(_) => {
                book = Book::new();
                details(path, &mut book);
            }
        }
    }
    mobi6(&r, &p0, &mut book)?;
    finish(&r, book, &p0)
}

/// The data of the first EXTH record of type `ty` in a header record.
fn exth(h: &[u8], ty: u32) -> Option<&[u8]> {
    let be = |at: usize| Some(u32::from_be_bytes(h.get(at..at + 4)?.try_into().ok()?));
    let start = 16 + be(20)? as usize;
    if h.get(start..start + 4)? != b"EXTH" {
        return None;
    }
    let mut at = start + 12;
    for _ in 0..be(start + 8)? {
        let (t, len) = (be(at)?, be(at + 4)? as usize);
        if len < 8 {
            return None;
        }
        if t == ty {
            return h.get(at + 8..at + len);
        }
        at += len;
    }
    None
}

fn finish(r: &Records, mut book: Book, p: &crate::pages::Part) -> Result<Book, String> {
    if book.cover.is_none()
        && let Some(c) = p.cover
        && let Some(data) = picture(r, p.first_picture + c)
    {
        book.cover = book.add_file(data, "cover");
    }
    if book.chapters.is_empty() {
        return Err("bookBroken".into());
    }
    super::fix_links(&mut book);
    Ok(book)
}

/// The Huffman dictionary of the compression some Kindle books use.
fn huff_of(r: &Records, at: usize) -> Option<Huff> {
    let h = r.read(at, 64 << 10)?;
    let first = be32(&h, 0x70)? as usize;
    let count = be32(&h, 0x74)? as usize;
    if count < 2 {
        return None;
    }
    Huff::new(r, first, count)
}

/// An old Kindle book.
fn mobi6(r: &Records, p: &crate::pages::Part, book: &mut Book) -> Result<(), String> {
    let huff = if p.compression == 17480 {
        huff_of(r, p.at)
    } else {
        None
    };
    let mut raw = text_bytes(
        r,
        p.at,
        p.text_records,
        p.compression,
        p.trailing,
        huff.as_ref(),
    )?;
    let head = r.read(p.at, 64 << 10).unwrap_or_default();
    let utf8 = be32(&head, 0x1c) == Some(65001);
    // Places links point at, by byte: marked so the links can find them.
    let mut targets: Vec<usize> = Vec::new();
    {
        let s = &raw;
        let mut i = 0;
        while let Some(at) = find(&s[i..], b"filepos=") {
            let mut j = i + at + 8;
            while j < s.len() && (s[j] == b'"' || s[j] == b'\'') {
                j += 1;
            }
            let start = j;
            while j < s.len() && s[j].is_ascii_digit() {
                j += 1;
            }
            if let Ok(n) = std::str::from_utf8(&s[start..j])
                .unwrap_or("")
                .parse::<usize>()
            {
                targets.push(n);
            }
            i = j.max(i + at + 8);
        }
    }
    // The book's own table of contents: names and places.
    let mut toc_flat: Vec<(usize, String, usize)> = Vec::new();
    if let Some(i) = be32(&head, 0xF4).filter(|v| *v != u32::MAX)
        && let Some(ncx) = read_index(r, i as usize)
    {
        for (_, t) in &ncx.entries {
            let (Some(pos), Some(label)) = (
                t.get(&1).and_then(|v| v.first()).copied(),
                t.get(&3)
                    .and_then(|v| v.first())
                    .and_then(|o| ncx.strings.get(o))
                    .cloned(),
            ) else {
                continue;
            };
            let depth = t.get(&4).and_then(|v| v.first()).copied().unwrap_or(0) as usize;
            if !label.trim().is_empty() {
                targets.push(pos as usize);
                toc_flat.push((
                    depth,
                    crate::text::collapse_spaces(label.trim()),
                    pos as usize,
                ));
            }
        }
    }
    // Chapters start where the table of contents' top entries are.
    let top = toc_flat.iter().map(|(d, _, _)| *d).min();
    let starts: std::collections::HashSet<usize> = toc_flat
        .iter()
        .filter(|(d, _, _)| Some(*d) == top)
        .map(|(_, _, p)| *p)
        .collect();
    targets.sort_unstable();
    targets.dedup();
    for t in targets.iter().rev() {
        let mut at = (*t).min(raw.len());
        // Not inside a tag: before it.
        if let Some(lt) = raw[..at].iter().rposition(|b| *b == b'<')
            && !raw[lt..at].contains(&b'>')
        {
            at = lt;
        }
        let mark = if starts.contains(t) {
            format!("<mbp:pagebreak/><a id=\"filepos{t}\"></a>")
        } else {
            format!("<a id=\"filepos{t}\"></a>")
        };
        raw.splice(at..at, mark.into_bytes());
    }
    let text = if utf8 {
        String::from_utf8_lossy(&raw).into_owned()
    } else {
        decode_bytes(&raw, false)
    };
    let nodes = xhtml::parse(&text);
    let (body, _) = xhtml::body_of(nodes);
    let first = p.first_picture;
    let pics = load_pictures(book, &body, &|s: &str| {
        let n: usize = s.trim().parse().ok()?;
        picture(r, first + n - 1)
    });
    let href = |h: &str, pic: bool| -> Option<String> {
        if pic {
            return pics.get(h).cloned();
        }
        let digits = h.trim_start_matches('0');
        if !h.is_empty() && h.chars().all(|c| c.is_ascii_digit()) {
            return Some(format!(
                "#filepos{}",
                if digits.is_empty() { "0" } else { digits }
            ));
        }
        (h.starts_with('#') || h.starts_with("http") || h.starts_with("mailto:"))
            .then(|| h.to_string())
    };
    add_document_with(book, body, &href, false, true);
    if !toc_flat.is_empty() {
        book.toc = nest(
            toc_flat
                .into_iter()
                .map(|(d, title, pos)| {
                    (
                        d,
                        Toc {
                            title,
                            href: format!("#filepos{pos}"),
                            kids: vec![],
                        },
                    )
                })
                .collect(),
        );
    }
    Ok(())
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

// ---------------------------------------------------------------- KF8

/// A Kindle index: its entries (key, tag → values) and its strings.
struct Index {
    entries: Vec<(String, HashMap<u8, Vec<u32>>)>,
    strings: HashMap<u32, String>,
}

/// A number written in 7-bit groups, the last group's byte marked.
fn varlen(d: &[u8], at: &mut usize) -> Option<u32> {
    let mut v: u32 = 0;
    loop {
        let b = *d.get(*at)?;
        *at += 1;
        v = (v << 7) | (b & 0x7f) as u32;
        if b & 0x80 != 0 {
            return Some(v);
        }
    }
}

fn read_index(r: &Records, first: usize) -> Option<Index> {
    let meta = r.read(first, 1 << 20)?;
    if !meta.starts_with(b"INDX") {
        return None;
    }
    let header = be32(&meta, 4)? as usize;
    let records = be32(&meta, 24)? as usize;
    let strings_count = be32(&meta, 52).unwrap_or(0) as usize;
    // The tag table.
    let t = header;
    if meta.get(t..t + 4)? != b"TAGX" {
        return None;
    }
    let tlen = be32(&meta, t + 4)? as usize;
    let control_bytes = be32(&meta, t + 8)? as usize;
    let mut tags: Vec<(u8, u8, u8, bool)> = Vec::new();
    let mut i = t + 12;
    while i + 4 <= t + tlen {
        tags.push((meta[i], meta[i + 1], meta[i + 2], meta[i + 3] & 1 == 1));
        i += 4;
    }
    let mut out = Index {
        entries: Vec::new(),
        strings: HashMap::new(),
    };
    for k in 1..=records {
        let rec = r.read(first + k, 1 << 20)?;
        let idxt = be32(&rec, 20)? as usize;
        let n = be32(&rec, 24)? as usize;
        if rec.get(idxt..idxt + 4)? != b"IDXT" {
            return None;
        }
        let offs: Vec<usize> = (0..n)
            .filter_map(|j| be16(&rec, idxt + 4 + 2 * j).map(|v| v as usize))
            .collect();
        for j in 0..offs.len() {
            let start = offs[j];
            let end = offs.get(j + 1).copied().unwrap_or(idxt);
            let klen = *rec.get(start)? as usize;
            let key = String::from_utf8_lossy(rec.get(start + 1..start + 1 + klen)?).into_owned();
            let cb_at = start + 1 + klen;
            let cbs = rec.get(cb_at..cb_at + control_bytes)?;
            // First what each tag has (from the control bytes), then the
            // values, in the tags' order.
            enum Has {
                Count(u32),
                Bytes(u32),
            }
            let mut data = cb_at + control_bytes;
            let mut cbi = 0;
            let mut found: Vec<(u8, u8, Has)> = Vec::new();
            for &(tag, per, mask, end_flag) in &tags {
                if end_flag {
                    cbi += 1;
                    continue;
                }
                let Some(&cb) = cbs.get(cbi) else { break };
                let v = cb & mask;
                if v == 0 {
                    continue;
                }
                if v == mask && mask.count_ones() > 1 {
                    let bytes = varlen(&rec, &mut data)?;
                    found.push((tag, per, Has::Bytes(bytes)));
                } else {
                    found.push((tag, per, Has::Count((v >> mask.trailing_zeros()) as u32)));
                }
            }
            let mut map = HashMap::new();
            for (tag, per, has) in found {
                let mut vals = Vec::new();
                match has {
                    Has::Count(c) => {
                        for _ in 0..c * per.max(1) as u32 {
                            vals.push(varlen(&rec, &mut data)?);
                        }
                    }
                    Has::Bytes(b) => {
                        let stop = data + b as usize;
                        while data < stop {
                            vals.push(varlen(&rec, &mut data)?);
                        }
                    }
                }
                map.insert(tag, vals);
            }
            let _ = end;
            out.entries.push((key, map));
        }
    }
    // Strings: a length, then the text; addressed by record and offset.
    for k in 0..strings_count {
        let Some(rec) = r.read(first + records + 1 + k, 1 << 20) else {
            break;
        };
        let mut at = 0;
        while at < rec.len() {
            let here = at;
            let Some(len) = varlen(&rec, &mut at) else {
                break;
            };
            let len = len as usize;
            if at + len > rec.len() {
                break;
            }
            out.strings.insert(
                (k as u32) * 0x10000 + here as u32,
                String::from_utf8_lossy(&rec[at..at + len]).into_owned(),
            );
            at += len;
        }
    }
    Some(out)
}

/// Kindle's base-32 numbers (digits then `A`–`V`).
fn base32(s: &str) -> Option<u32> {
    let mut v: u32 = 0;
    for c in s.chars() {
        let d = c.to_digit(32)?;
        v = v.checked_mul(32)?.checked_add(d)?;
    }
    Some(v)
}

struct Frag {
    file: usize,
    /// Where its text starts in its file.
    at: usize,
}

fn kf8(r: &Records, p: &crate::pages::Part, at: usize, book: &mut Book) -> Result<(), String> {
    let head = r.read(at, 64 << 10).ok_or("bookBroken")?;
    let field = |o: usize| {
        be32(&head, o)
            .filter(|v| *v != u32::MAX)
            .map(|v| v as usize)
    };
    let huff = if p.compression == 17480 {
        huff_of(r, at)
    } else {
        None
    };
    let text = text_bytes(
        r,
        at,
        p.text_records,
        p.compression,
        p.trailing,
        huff.as_ref(),
    )?;
    // Flows: the HTML first, then style sheets and the like.
    let mut flows: Vec<(usize, usize)> = Vec::new();
    if let Some(f) = field(0xC0)
        && let Some(rec) = r.read(at + f, 1 << 20)
        && rec.starts_with(b"FDST")
    {
        let n = be32(&rec, 8).unwrap_or(0) as usize;
        for k in 0..n {
            if let (Some(a), Some(b)) = (be32(&rec, 12 + 8 * k), be32(&rec, 16 + 8 * k)) {
                flows.push((a as usize, b as usize));
            }
        }
    }
    if flows.is_empty() {
        flows.push((0, text.len()));
    }
    let flow = |i: usize| -> &[u8] {
        flows
            .get(i)
            .and_then(|(a, b)| text.get(*a..(*b).min(text.len())))
            .unwrap_or(&[])
    };
    let main = flow(0);
    let skel = field(0xFC)
        .and_then(|i| read_index(r, at + i))
        .ok_or("bookBroken")?;
    let frag = field(0xF8)
        .and_then(|i| read_index(r, at + i))
        .ok_or("bookBroken")?;
    // Files: each skeleton with its fragments put in.
    let mut files: Vec<Vec<u8>> = Vec::new();
    let mut frags: Vec<Frag> = Vec::new();
    let mut fi = 0;
    for (fnum, (_, tags)) in skel.entries.iter().enumerate() {
        let count = tags.get(&1).and_then(|v| v.first()).copied().unwrap_or(0) as usize;
        let (start, len) = match tags.get(&6).map(|v| v.as_slice()) {
            Some([s, l, ..]) => (*s as usize, *l as usize),
            _ => return Err("bookBroken".into()),
        };
        let mut file = main.get(start..start + len).ok_or("bookBroken")?.to_vec();
        let mut pos = start + len;
        for _ in 0..count {
            let (key, ft) = frag.entries.get(fi).ok_or("bookBroken")?;
            fi += 1;
            let insert: usize = key.trim().parse().map_err(|_| "bookBroken")?;
            let flen = match ft.get(&6).map(|v| v.as_slice()) {
                Some([_, l, ..]) => *l as usize,
                _ => return Err("bookBroken".into()),
            };
            let data = main.get(pos..pos + flen).ok_or("bookBroken")?;
            let off = insert
                .checked_sub(start)
                .filter(|o| *o <= file.len())
                .ok_or("bookBroken")?;
            frags.push(Frag {
                file: fnum,
                at: off,
            });
            file.splice(off..off, data.iter().copied());
            pos += flen;
        }
        files.push(file);
    }
    if files.is_empty() {
        return Err("bookBroken".into());
    }
    // Places links point at: `kindle:pos:fid:F:off:O`, marked in their
    // files.
    let mut marks: Vec<Vec<(usize, String)>> = vec![Vec::new(); files.len()];
    let pos_link = |s: &str| -> Option<(u32, u32)> {
        let rest = s.strip_prefix("kindle:pos:fid:")?;
        let (f, o) = rest.split_once(":off:")?;
        Some((base32(f)?, base32(o.split(['?', '"']).next()?)?))
    };
    for f in &files {
        let s = String::from_utf8_lossy(f);
        let mut i = 0;
        while let Some(a) = s[i..].find("kindle:pos:fid:") {
            let st = i + a;
            let end = s[st..].find(['"', '\'']).map(|e| st + e).unwrap_or(s.len());
            if let Some((fid, off)) = pos_link(&s[st..end])
                && let Some(fr) = frags.get(fid as usize)
            {
                marks[fr.file].push((fr.at + off as usize, format!("kp{fid}_{off}")));
            }
            i = end.max(st + 1);
        }
    }
    // And places the table of contents points at.
    let ncx = field(0xF4).and_then(|i| read_index(r, at + i));
    if let Some(ncx) = &ncx {
        for (_, t) in &ncx.entries {
            if let Some([fid, off, ..]) = t.get(&6).map(|v| v.as_slice())
                && let Some(fr) = frags.get(*fid as usize)
            {
                marks[fr.file].push((fr.at + *off as usize, format!("kp{fid}_{off}")));
            }
        }
    }
    for (k, m) in marks.iter_mut().enumerate() {
        m.sort();
        m.dedup();
        let f = &mut files[k];
        for (at, id) in m.iter().rev() {
            let mut at = (*at).min(f.len());
            if let Some(lt) = f[..at].iter().rposition(|b| *b == b'<')
                && !f[lt..at].contains(&b'>')
            {
                at = lt;
            }
            // Not inside the head.
            let body = find(f, b"<body")
                .map(|b| b + f[b..].iter().position(|c| *c == b'>').unwrap_or(0) + 1)
                .unwrap_or(0);
            let at = at.max(body);
            f.splice(at..at, format!("<a id=\"{id}\"></a>").into_bytes());
        }
    }
    // Pictures and style sheets.
    let first = p.first_picture;
    let mut pics: HashMap<String, String> = HashMap::new();
    let mut css = String::new();
    let mut css_done = std::collections::HashSet::new();
    let names: Vec<String> = (0..files.len())
        .map(|i| format!("c{:04}.xhtml", i + 1))
        .collect();
    for f in &files {
        let s = String::from_utf8_lossy(f);
        let mut i = 0;
        while let Some(a) = s[i..].find("kindle:") {
            let st = i + a;
            let end = s[st..]
                .find(['"', '\'', ')'])
                .map(|e| st + e)
                .unwrap_or(s.len());
            let link = &s[st..end];
            if let Some(rest) = link.strip_prefix("kindle:embed:") {
                let n = base32(rest.get(..4).unwrap_or(rest));
                if let Some(n) = n.filter(|n| *n > 0)
                    && !pics.contains_key(link)
                    && let Some(data) = picture(r, first + n as usize - 1)
                    && let Some(name) = book.add_file(data, "")
                {
                    pics.insert(link.to_string(), name);
                }
            } else if let Some(rest) = link.strip_prefix("kindle:flow:")
                && let Some(n) = base32(rest.get(..4).unwrap_or(rest))
                && css_done.insert(n)
            {
                css.push_str(&String::from_utf8_lossy(flow(n as usize)));
                css.push('\n');
            }
            i = end.max(st + 1);
        }
    }
    // Pictures named in the style sheets.
    let css = {
        let mut out = String::new();
        let mut rest = css.as_str();
        while let Some(a) = rest.find("kindle:embed:") {
            out.push_str(&rest[..a]);
            let tail = &rest[a..];
            let end = tail.find(['"', '\'', ')']).unwrap_or(tail.len());
            let link = &tail[..end];
            let n = base32(link["kindle:embed:".len()..].get(..4).unwrap_or(""));
            let name = n
                .filter(|n| *n > 0)
                .and_then(|n| picture(r, first + n as usize - 1))
                .and_then(|d| book.add_file(d, ""));
            out.push_str(name.as_deref().unwrap_or(""));
            rest = &tail[end..];
        }
        out.push_str(rest);
        out
    };
    // Fonts are left out: what used them falls back to the reader's.
    book.css = if css.trim().is_empty() {
        super::BASE_CSS.into()
    } else {
        strip_font_faces(&css)
    };
    for (k, f) in files.iter().enumerate() {
        let s = String::from_utf8_lossy(f).into_owned();
        let nodes = xhtml::parse(&s);
        let title = xhtml::find(&nodes, "title")
            .map(|t| crate::text::collapse_spaces(t.plain().trim()))
            .unwrap_or_default();
        let (mut body, attrs) = xhtml::body_of(nodes);
        xhtml::tidy(&mut body);
        let link = |h: &str, pic: bool| -> Option<String> {
            if pic {
                return pics
                    .get(h.split(['"', '\'']).next().unwrap_or(h))
                    .cloned()
                    .or_else(|| {
                        pics.iter()
                            .find(|(k, _)| h.starts_with(k.as_str()))
                            .map(|(_, v)| v.clone())
                    });
            }
            if let Some((fid, off)) = pos_link(h) {
                let fr = frags.get(fid as usize)?;
                return Some(format!("{}#kp{fid}_{off}", names[fr.file]));
            }
            (h.starts_with('#') || h.starts_with("http") || h.starts_with("mailto:"))
                .then(|| h.to_string())
        };
        let mut out = String::new();
        let class = attrs
            .iter()
            .find(|(a, _)| a == "class")
            .map(|(_, v)| v.clone());
        if let Some(c) = &class {
            out.push_str(&format!("<div class=\"{}\">", xhtml::esc_attr(c)));
        }
        xhtml::write(
            &body,
            &Out {
                href: &link,
                classes: true,
            },
            &mut out,
        );
        if class.is_some() {
            out.push_str("</div>");
        }
        book.chapters.push(Chapter {
            name: names[k].clone(),
            title,
            body: out,
            ..Default::default()
        });
    }
    // The table of contents.
    if let Some(ncx) = ncx {
        let mut flat: Vec<(usize, Toc)> = Vec::new();
        for (_, t) in &ncx.entries {
            let label = t
                .get(&3)
                .and_then(|v| v.first())
                .and_then(|o| ncx.strings.get(o))
                .cloned()
                .unwrap_or_default();
            let depth = t.get(&4).and_then(|v| v.first()).copied().unwrap_or(0) as usize;
            let href = match t.get(&6).map(|v| v.as_slice()) {
                Some([fid, off, ..]) => frags
                    .get(*fid as usize)
                    .map(|fr| format!("{}#kp{fid}_{off}", names[fr.file])),
                _ => None,
            };
            if let Some(href) = href
                && !label.trim().is_empty()
            {
                // The place is marked if it was linked; else the file.
                let file = href.split('#').next().unwrap_or_default().to_string();
                let marked = book.chapters.iter().any(|c| {
                    c.name == file
                        && c.body
                            .contains(&format!("id=\"{}\"", href.split('#').nth(1).unwrap_or("")))
                });
                flat.push((
                    depth,
                    Toc {
                        title: label.trim().to_string(),
                        href: if marked { href } else { file },
                        kids: vec![],
                    },
                ));
            }
        }
        book.toc = nest(flat);
    }
    Ok(())
}

/// A flat list of (depth, entry) as a tree.
fn nest(flat: Vec<(usize, Toc)>) -> Vec<Toc> {
    fn take(
        it: &mut std::iter::Peekable<std::vec::IntoIter<(usize, Toc)>>,
        depth: usize,
    ) -> Vec<Toc> {
        let mut out = Vec::new();
        while let Some((d, _)) = it.peek() {
            if *d < depth {
                break;
            }
            let (d, mut t) = it.next().unwrap();
            t.kids = take(it, d + 1);
            out.push(t);
        }
        out
    }
    let min = flat.iter().map(|(d, _)| *d).min().unwrap_or(0);
    take(&mut flat.into_iter().peekable(), min)
}

fn strip_font_faces(css: &str) -> String {
    let mut out = String::new();
    let mut rest = css;
    while let Some(a) = rest.find("@font-face") {
        out.push_str(&rest[..a]);
        let tail = &rest[a..];
        rest = match tail.find('}') {
            Some(e) => &tail[e + 1..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------- HUFF/CDIC

/// The dictionary compression of some Kindle books: Huffman codes for
/// phrases, which may themselves be compressed.
struct Huff {
    /// Per first byte of the code: its length, whether that is final, and
    /// the highest code.
    table1: Vec<(u8, bool, u64)>,
    min: [u64; 33],
    max: [u64; 33],
    phrases: Vec<(Vec<u8>, bool)>,
}

impl Huff {
    fn new(r: &Records, first: usize, count: usize) -> Option<Huff> {
        let h = r.read(first, 1 << 20)?;
        if !h.starts_with(b"HUFF") {
            return None;
        }
        let t1 = be32(&h, 8)? as usize;
        let t2 = be32(&h, 12)? as usize;
        let mut table1 = Vec::with_capacity(256);
        for i in 0..256 {
            let v = be32(&h, t1 + 4 * i)?;
            let len = (v & 0x1f) as u8;
            let term = v & 0x80 != 0;
            let max = (((v >> 8) as u64 + 1) << (32 - len as u32)) - 1;
            table1.push((len, term, max));
        }
        let mut min = [0u64; 33];
        let mut max = [u64::MAX; 33];
        for len in 1..=32usize {
            let lo = be32(&h, t2 + 8 * (len - 1))? as u64;
            let hi = be32(&h, t2 + 8 * (len - 1) + 4)? as u64;
            min[len] = lo << (32 - len);
            max[len] = ((hi + 1) << (32 - len)) - 1;
        }
        let mut phrases = Vec::new();
        for k in 1..count {
            let c = r.read(first + k, 1 << 20)?;
            if !c.starts_with(b"CDIC") {
                return None;
            }
            let hl = be32(&c, 4)? as usize;
            let total = be32(&c, 8)? as usize;
            let bits = be32(&c, 12)?;
            let n = (1usize << bits).min(total - phrases.len().min(total));
            for i in 0..n {
                let off = be16(&c, hl + 2 * i)? as usize;
                let at = hl + off;
                let w = be16(&c, at)?;
                let len = (w & 0x7fff) as usize;
                let done = w & 0x8000 != 0;
                phrases.push((c.get(at + 2..at + 2 + len)?.to_vec(), done));
            }
        }
        Some(Huff {
            table1,
            min,
            max,
            phrases,
        })
    }

    fn unpack(&self, data: &[u8], out: &mut Vec<u8>, depth: usize) -> Result<(), String> {
        if depth > 32 {
            return Err("bookBroken".into());
        }
        let total_bits = data.len() as u64 * 8;
        let mut pos: u64 = 0;
        // Bits from `pos`, 32 at a time (zeros past the end).
        let window = |pos: u64| -> u64 {
            let mut v: u64 = 0;
            for k in 0..5 {
                let byte = (pos / 8) as usize + k;
                v = (v << 8) | *data.get(byte).unwrap_or(&0) as u64;
            }
            (v >> (8 - (pos % 8))) & 0xffff_ffff
        };
        while pos < total_bits {
            let x = window(pos);
            let (mut len, term, mut max) = self.table1[(x >> 24) as usize];
            if len == 0 {
                return Err("bookBroken".into());
            }
            if !term {
                while (len as usize) < 32 && x < self.min[len as usize] {
                    len += 1;
                }
                max = self.max[len as usize];
            }
            pos += len as u64;
            if pos > total_bits {
                break;
            }
            let i = ((max - x) >> (32 - len as u32)) as usize;
            let (phrase, done) = self.phrases.get(i).ok_or("bookBroken")?;
            if *done {
                out.extend_from_slice(phrase);
            } else {
                self.unpack(phrase, out, depth + 1)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        let d = [0x01, 0x82, 0x85];
        let mut at = 0;
        assert_eq!(varlen(&d, &mut at), Some((1 << 7) | 2));
        assert_eq!(varlen(&d, &mut at), Some(5));
        assert_eq!(base32("0001"), Some(1));
        assert_eq!(base32("000V"), Some(31));
        assert_eq!(base32("0010"), Some(32));
    }
}
