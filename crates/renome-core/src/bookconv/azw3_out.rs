//! A [`Book`] written as AZW3 (KF8), the format Kindle e-readers show
//! books in when they are copied to them. Each chapter becomes a skeleton
//! (its XHTML file with an empty body) and one fragment (the body's
//! content) put back in it; the style sheet is a second flow of the text;
//! pictures are resources; three indexes say where skeletons, fragments
//! and table-of-contents entries are. Links and pictures use `kindle:`
//! addresses. The text is compressed the PalmDOC way, in records of
//! 4096 bytes.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use super::xhtml::{esc, esc_attr};
use super::{Book, Toc};

fn put16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn put32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn set32(v: &mut [u8], at: usize, x: u32) {
    v[at..at + 4].copy_from_slice(&x.to_be_bytes());
}

/// Kindle's base-32 numbers, `width` digits.
fn base32(mut n: u32, width: usize) -> String {
    const D: &[u8; 32] = b"0123456789ABCDEFGHIJKLMNOPQRSTUV";
    let mut s = vec![b'0'; width];
    for i in (0..width).rev() {
        s[i] = D[(n % 32) as usize];
        n /= 32;
    }
    String::from_utf8(s).unwrap()
}

/// A number in 7-bit groups, the last byte marked (as indexes have them).
fn varlen(mut n: u32, out: &mut Vec<u8>) {
    let mut groups = vec![(n & 0x7f) as u8 | 0x80];
    n >>= 7;
    while n > 0 {
        groups.push((n & 0x7f) as u8);
        n >>= 7;
    }
    groups.reverse();
    out.extend(groups);
}

/// PalmDOC compression: repeats of 3 to 10 bytes found in the last 2047
/// bytes, a space before a plain letter in one byte, other bytes as they
/// are (those that would be read as codes, in runs of up to 8).
fn palmdoc_pack(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        // A repeat.
        if i >= 3 {
            let start = i.saturating_sub(2047);
            let mut best = (0usize, 0usize);
            let max_len = 10.min(input.len() - i);
            if max_len >= 3 {
                let mut j = start;
                while j + 3 <= i {
                    if input[j] == input[i]
                        && input[j + 1] == input[i + 1]
                        && input[j + 2] == input[i + 2]
                    {
                        let mut l = 3;
                        while l < max_len && j + l < i && input[j + l] == input[i + l] {
                            l += 1;
                        }
                        if l > best.1 {
                            best = (i - j, l);
                            if l == max_len {
                                break;
                            }
                        }
                    }
                    j += 1;
                }
            }
            if best.1 >= 3 {
                let code = 0x8000u16 | ((best.0 as u16) << 3) | (best.1 as u16 - 3);
                put16(&mut out, code);
                i += best.1;
                continue;
            }
        }
        let b = input[i];
        if b == b' ' && i + 1 < input.len() && (0x40..0x80).contains(&input[i + 1]) {
            out.push(input[i + 1] ^ 0x80);
            i += 2;
            continue;
        }
        if b == 0 || (0x09..0x80).contains(&b) {
            out.push(b);
            i += 1;
            continue;
        }
        // Bytes that would read as codes: a run with its length first.
        let mut n = 1;
        while n < 8 && i + n < input.len() {
            let c = input[i + n];
            if c == 0 || (0x09..0x80).contains(&c) {
                break;
            }
            n += 1;
        }
        out.push(n as u8);
        out.extend_from_slice(&input[i..i + n]);
        i += n;
    }
    out
}

/// A skeleton and its fragment, and where they go in the text.
struct File {
    skeleton: Vec<u8>,
    insert: usize,
    fragment: Vec<u8>,
}

/// An index entry: its key and its tags' values.
type Entry = (String, Vec<(u8, Vec<u32>)>);

/// The index records of one index: its header (with its tag table and
/// the last key of each record), the records of entries, and strings.
fn index(
    tags: &[(u8, u8, u8, u8)],
    control_bytes: u8,
    entries: &[Entry],
    strings: Option<&[u8]>,
) -> Vec<Vec<u8>> {
    // Entries record.
    let mut body = Vec::new();
    let mut offs = Vec::new();
    for (key, vals) in entries {
        offs.push(192 + body.len());
        body.push(key.len() as u8);
        body.extend_from_slice(key.as_bytes());
        // Control byte: the count of each tag present.
        let mut cb = 0u8;
        for (tag, values) in vals {
            if let Some((_, per, mask, _)) = tags.iter().find(|t| t.0 == *tag) {
                let count = (values.len() / (*per as usize).max(1)) as u8;
                cb |= (count << mask.trailing_zeros()) & mask;
            }
        }
        body.push(cb);
        body.extend(std::iter::repeat_n(
            0u8,
            control_bytes.saturating_sub(1) as usize,
        ));
        for (tag, values) in vals {
            if tags.iter().any(|t| t.0 == *tag) {
                for v in values {
                    varlen(*v, &mut body);
                }
            }
        }
    }
    let mut rec = vec![0u8; 192];
    rec[..4].copy_from_slice(b"INDX");
    set32(&mut rec, 4, 192);
    set32(&mut rec, 12, 1);
    rec.extend_from_slice(&body);
    while !rec.len().is_multiple_of(4) {
        rec.push(0);
    }
    let idxt = rec.len();
    rec.extend_from_slice(b"IDXT");
    for o in &offs {
        put16(&mut rec, *o as u16);
    }
    while !rec.len().is_multiple_of(4) {
        rec.push(0);
    }
    set32(&mut rec, 20, idxt as u32);
    set32(&mut rec, 24, entries.len() as u32);
    for k in [28usize, 32] {
        set32(&mut rec, k, u32::MAX);
    }

    // Header record.
    let mut meta = vec![0u8; 192];
    meta[..4].copy_from_slice(b"INDX");
    set32(&mut meta, 4, 192);
    set32(&mut meta, 24, 1);
    set32(&mut meta, 28, 65001);
    set32(&mut meta, 32, u32::MAX);
    set32(&mut meta, 36, entries.len() as u32);
    set32(&mut meta, 52, u32::from(strings.is_some()));
    meta.extend_from_slice(b"TAGX");
    put32(&mut meta, 12 + 4 * tags.len() as u32);
    put32(&mut meta, control_bytes as u32);
    for t in tags {
        meta.extend_from_slice(&[t.0, t.1, t.2, t.3]);
    }
    // The last key of the entries record, and its count.
    let geo = meta.len();
    let last = entries.last().map(|e| e.0.as_str()).unwrap_or("");
    meta.push(last.len() as u8);
    meta.extend_from_slice(last.as_bytes());
    put16(&mut meta, entries.len() as u16);
    while !meta.len().is_multiple_of(4) {
        meta.push(0);
    }
    let midxt = meta.len();
    meta.extend_from_slice(b"IDXT");
    put16(&mut meta, geo as u16);
    while !meta.len().is_multiple_of(4) {
        meta.push(0);
    }
    set32(&mut meta, 20, midxt as u32);
    let mut out = vec![meta, rec];
    if let Some(s) = strings {
        out.push(s.to_vec());
    }
    out
}

/// Strings for an index, each with its length first; their offsets.
fn strings_of(list: &[String]) -> (Vec<u8>, Vec<u32>) {
    let mut out = Vec::new();
    let mut offs = Vec::new();
    for s in list {
        offs.push(out.len() as u32);
        let b = s.as_bytes();
        let b = &b[..b.len().min(500)];
        varlen(b.len() as u32, &mut out);
        out.extend_from_slice(b);
    }
    (out, offs)
}

fn locale(lang: &str) -> u32 {
    match lang
        .split('-')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "en" => 9,
        "pt" => 22,
        "es" => 10,
        "fr" => 12,
        "de" => 7,
        "it" => 16,
        "nl" => 19,
        "ru" => 25,
        "ja" => 17,
        "zh" => 4,
        _ => 0,
    }
}

/// Writes `book` as an AZW3 file.
pub fn write(book: &Book, to: &Path) -> Result<(), String> {
    let data = build(book)?;
    let mut f = std::fs::File::create(to).map_err(|e| e.to_string())?;
    f.write_all(&data).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())
}

pub fn build(book: &Book) -> Result<Vec<u8>, String> {
    // Pictures: their number as resources.
    let pics: Vec<&super::Res> = book
        .files
        .iter()
        .filter(|r| r.media.starts_with("image/") && r.media != "image/svg+xml")
        .collect();
    let pic_no: HashMap<&str, u32> = pics
        .iter()
        .enumerate()
        .map(|(i, r)| (r.name.as_str(), i as u32 + 1))
        .collect();
    let mime: HashMap<&str, &str> = pics
        .iter()
        .map(|r| (r.name.as_str(), r.media.as_str()))
        .collect();

    // Each chapter's body, with `kindle:` addresses; where its ids are.
    let mut bodies: Vec<String> = Vec::new();
    for c in &book.chapters {
        let mut body = c.body.clone();
        for (name, n) in &pic_no {
            body = body.replace(
                &format!("src=\"{}\"", esc_attr(name)),
                &format!("src=\"kindle:embed:{}?mime={}\"", base32(*n, 4), mime[name]),
            );
        }
        bodies.push(body);
    }
    // Ids: (chapter, id) → offset of their element in the fragment.
    let id_re = regex::Regex::new(r#"<[^<>]*\sid="([^"]+)""#).unwrap();
    let mut ids: HashMap<(usize, String), usize> = HashMap::new();
    for (i, b) in bodies.iter().enumerate() {
        for m in id_re.captures_iter(b) {
            ids.insert((i, m[1].to_string()), m.get(0).unwrap().start());
        }
    }
    let file_no: HashMap<&str, usize> = book
        .chapters
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name.as_str(), i))
        .collect();
    // Links: `file#id` → `kindle:pos:fid:F:off:O`. Offsets do not move,
    // as each address has the same length.
    let href_re = regex::Regex::new(r##"href="([^"#]*)(?:#([^"]*))?""##).unwrap();
    let place = |file: usize, id: Option<&str>| -> String {
        let off = id
            .and_then(|i| ids.get(&(file, i.to_string())))
            .copied()
            .unwrap_or(0);
        format!(
            "kindle:pos:fid:{}:off:{}",
            base32(file as u32, 4),
            base32(off as u32, 10)
        )
    };
    let bodies: Vec<String> = bodies
        .iter()
        .enumerate()
        .map(|(i, b)| {
            href_re
                .replace_all(b, |c: &regex::Captures| {
                    let file = &c[1];
                    if file.starts_with("http") || file.starts_with("mailto:") {
                        return c[0].to_string();
                    }
                    let target = if file.is_empty() {
                        Some(i)
                    } else {
                        file_no.get(file).copied()
                    };
                    match target {
                        Some(t) => format!("href=\"{}\"", place(t, c.get(2).map(|m| m.as_str()))),
                        None => c[0].to_string(),
                    }
                })
                .into_owned()
        })
        .collect();

    // Skeletons and fragments.
    let lang = if book.meta.language.is_empty() {
        "und".to_string()
    } else {
        book.meta.language.clone()
    };
    let mut files = Vec::new();
    for (c, body) in book.chapters.iter().zip(&bodies) {
        let title = if c.title.is_empty() {
            &book.meta.title
        } else {
            &c.title
        };
        // A fixed page says its size, as the Kindle lays it out by.
        let viewport = c
            .page
            .map(|(w, h)| format!("<meta name=\"viewport\" content=\"width={w}, height={h}\"/>\n"))
            .unwrap_or_default();
        let head = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"{l}\" lang=\"{l}\">\n<head>\n{viewport}<title>{t}</title>\n<link href=\"kindle:flow:0001?mime=text/css\" rel=\"stylesheet\" type=\"text/css\"/>\n</head>\n<body aid=\"0\">",
            l = esc_attr(if c.lang.is_empty() { &lang } else { &c.lang }),
            t = esc(title)
        );
        let tail = "</body>\n</html>\n";
        let mut skeleton = head.clone().into_bytes();
        let insert = skeleton.len();
        skeleton.extend_from_slice(tail.as_bytes());
        files.push(File {
            skeleton,
            insert,
            fragment: body.clone().into_bytes(),
        });
    }
    if files.is_empty() {
        return Err("bookBroken".into());
    }
    let mut text: Vec<u8> = Vec::new();
    let mut skel_entries = Vec::new();
    let mut frag_entries = Vec::new();
    let mut frag_starts = Vec::new();
    for (i, f) in files.iter().enumerate() {
        let start = text.len() as u32;
        text.extend_from_slice(&f.skeleton);
        frag_starts.push(text.len());
        text.extend_from_slice(&f.fragment);
        skel_entries.push((
            format!("SKEL{i:07}"),
            vec![(1u8, vec![1u32]), (6, vec![start, f.skeleton.len() as u32])],
        ));
        frag_entries.push((
            format!("{:010}", start as usize + f.insert),
            vec![
                (2u8, vec![0u32]),
                (3, vec![i as u32]),
                (4, vec![i as u32]),
                (6, vec![0, f.fragment.len() as u32]),
            ],
        ));
    }
    let flow0 = text.len();
    let mut css = book.css.clone();
    // Pictures in the style sheet.
    for (name, n) in &pic_no {
        css = css.replace(
            &format!("url(\"{name}\")"),
            &format!("url(kindle:embed:{}?mime={})", base32(*n, 4), mime[name]),
        );
    }
    text.extend_from_slice(css.as_bytes());
    let flows = vec![(0usize, flow0), (flow0, text.len())];

    // Text records: 4096 bytes each, compressed; a character cut at the
    // end is completed after it, with how many bytes that took.
    let mut text_recs: Vec<Vec<u8>> = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let end = (at + 4096).min(text.len());
        let mut rec = palmdoc_pack(&text[at..end]);
        let mut extra = 0;
        while end + extra < text.len() && (text[end + extra] & 0xc0) == 0x80 && extra < 3 {
            extra += 1;
        }
        rec.extend_from_slice(&text[end..end + extra]);
        rec.push(extra as u8);
        text_recs.push(rec);
        at = end;
    }

    // The table of contents: its entries' labels, places and depths.
    let mut flat: Vec<(String, usize, u32, u32)> = Vec::new();
    fn walk(t: &[Toc], d: u32, out: &mut Vec<(String, String, u32)>) {
        for e in t {
            out.push((e.title.clone(), e.href.clone(), d));
            walk(&e.kids, d + 1, out);
        }
    }
    let mut toc_list = Vec::new();
    walk(&book.toc, 0, &mut toc_list);
    if toc_list.is_empty() {
        for c in &book.chapters {
            toc_list.push((
                if c.title.is_empty() {
                    book.meta.title.clone()
                } else {
                    c.title.clone()
                },
                c.name.clone(),
                0,
            ));
        }
    }
    for (title, href, depth) in toc_list {
        let (file, id) = match href.split_once('#') {
            Some((f, i)) => (f.to_string(), Some(i.to_string())),
            None => (href.clone(), None),
        };
        if let Some(&n) = file_no.get(file.as_str()) {
            let off = id.and_then(|i| ids.get(&(n, i))).copied().unwrap_or(0);
            flat.push((title, n, off as u32, depth));
        }
    }
    let labels: Vec<String> = flat.iter().map(|f| f.0.clone()).collect();
    let (ncx_strings, label_offs) = strings_of(&labels);
    let ncx_entries: Vec<Entry> = flat
        .iter()
        .enumerate()
        .map(|(i, (_, file, off, depth))| {
            let pos = (frag_starts[*file] + *off as usize) as u32;
            (
                format!("{i:03}"),
                vec![
                    (1u8, vec![pos]),
                    (2, vec![0]),
                    (3, vec![label_offs[i]]),
                    (4, vec![*depth]),
                    (6, vec![*file as u32, *off]),
                ],
            )
        })
        .collect();
    let (frag_strings, _) = strings_of(&["P-//*[@aid='0']".to_string()]);

    let skel_idx = index(
        &[(1, 1, 0x03, 0), (6, 2, 0x0C, 0), (0, 0, 0, 1)],
        1,
        &skel_entries,
        None,
    );
    let frag_idx = index(
        &[
            (2, 1, 0x01, 0),
            (3, 1, 0x02, 0),
            (4, 1, 0x04, 0),
            (6, 2, 0x08, 0),
            (0, 0, 0, 1),
        ],
        1,
        &frag_entries,
        Some(&frag_strings),
    );
    let ncx_idx = index(
        &[
            (1, 1, 0x01, 0),
            (2, 1, 0x02, 0),
            (3, 1, 0x04, 0),
            (4, 1, 0x08, 0),
            (6, 2, 0x10, 0),
            (0, 0, 0, 1),
        ],
        1,
        &ncx_entries,
        Some(&ncx_strings),
    );

    // Records in order.
    let mut records: Vec<Vec<u8>> = vec![Vec::new()];
    records.extend(text_recs.iter().cloned());
    let first_nonbook = records.len();
    let first_res = records.len();
    for p in &pics {
        records.push(p.data.clone());
    }
    let fdst_at = records.len();
    let mut fdst = b"FDST".to_vec();
    put32(&mut fdst, 12);
    put32(&mut fdst, flows.len() as u32);
    for (a, b) in &flows {
        put32(&mut fdst, *a as u32);
        put32(&mut fdst, *b as u32);
    }
    records.push(fdst);
    let skel_at = records.len();
    records.extend(skel_idx);
    let frag_at = records.len();
    records.extend(frag_idx);
    let ncx_at = records.len();
    records.extend(ncx_idx);
    let flis_at = records.len();
    let mut flis = b"FLIS".to_vec();
    for v in [8u32, 0x41, 0, 0, u32::MAX] {
        put32(&mut flis, v);
    }
    put16(&mut flis, 1);
    put16(&mut flis, 3);
    put32(&mut flis, 3);
    put32(&mut flis, 1);
    put32(&mut flis, u32::MAX);
    records.push(flis);
    let fcis_at = records.len();
    let mut fcis = b"FCIS".to_vec();
    for v in [0x14u32, 0x10, 2, 0, text.len() as u32, 0, 0x28, 0, 0x28, 8] {
        put32(&mut fcis, v);
    }
    put16(&mut fcis, 1);
    put16(&mut fcis, 1);
    put32(&mut fcis, 0);
    records.push(fcis);
    records.push(vec![0xe9, 0x8e, 0x0d, 0x0a]);

    // Record 0: the PalmDOC and MOBI headers, the details, the title.
    let m = &book.meta;
    let mut exth: Vec<(u32, Vec<u8>)> = Vec::new();
    for a in &m.authors {
        exth.push((100, a.as_bytes().to_vec()));
    }
    if !m.publisher.is_empty() {
        exth.push((101, m.publisher.as_bytes().to_vec()));
    }
    if !m.description.is_empty() {
        exth.push((103, m.description.as_bytes().to_vec()));
    }
    if !m.isbn.is_empty() {
        exth.push((104, m.isbn.as_bytes().to_vec()));
    }
    for s in &m.subjects {
        exth.push((105, s.as_bytes().to_vec()));
    }
    if !m.date.is_empty() {
        exth.push((106, m.date.as_bytes().to_vec()));
    }
    exth.push((503, m.title.as_bytes().to_vec()));
    exth.push((524, lang.as_bytes().to_vec()));
    exth.push((501, b"EBOK".to_vec()));
    // Pages of pictures (a comic): laid out as they are, at their size,
    // turning right to left when the book does.
    if book.fixed {
        exth.push((122, b"true".to_vec()));
        exth.push((123, b"comic".to_vec()));
        exth.push((124, b"none".to_vec()));
        if let Some((w, h)) = book.chapters.iter().find_map(|c| c.page) {
            exth.push((126, format!("{w}x{h}").into_bytes()));
        }
        exth.push((132, b"false".to_vec()));
        if book.rtl {
            exth.push((525, b"horizontal-rl".to_vec()));
            exth.push((527, b"rtl".to_vec()));
        }
    }
    if let Some(c) = &book.cover
        && let Some(n) = pic_no.get(c.as_str())
    {
        exth.push((201, (n - 1).to_be_bytes().to_vec()));
        exth.push((129, format!("kindle:embed:{}", base32(*n, 4)).into_bytes()));
    }
    let mut ex = Vec::new();
    for (k, v) in &exth {
        put32(&mut ex, *k);
        put32(&mut ex, v.len() as u32 + 8);
        ex.extend_from_slice(v);
    }
    let pad = (4 - ex.len() % 4) % 4;
    let mut exth_rec = b"EXTH".to_vec();
    put32(&mut exth_rec, (12 + ex.len()) as u32);
    put32(&mut exth_rec, exth.len() as u32);
    exth_rec.extend_from_slice(&ex);
    exth_rec.extend(std::iter::repeat_n(0u8, pad));

    let header_len = 264usize;
    let mut r0 = vec![0u8; 16 + header_len];
    // PalmDOC.
    r0[0..2].copy_from_slice(&2u16.to_be_bytes());
    set32(&mut r0, 4, text.len() as u32);
    r0[8..10].copy_from_slice(&(text_recs.len() as u16).to_be_bytes());
    r0[10..12].copy_from_slice(&4096u16.to_be_bytes());
    // MOBI.
    r0[16..20].copy_from_slice(b"MOBI");
    set32(&mut r0, 0x14, header_len as u32);
    set32(&mut r0, 0x18, 2);
    set32(&mut r0, 0x1C, 65001);
    set32(
        &mut r0,
        0x20,
        xxhash_rust::xxh3::xxh3_64(m.title.as_bytes()) as u32,
    );
    set32(&mut r0, 0x24, 8);
    for o in (0x28..0x50).step_by(4) {
        set32(&mut r0, o, u32::MAX);
    }
    set32(&mut r0, 0x50, first_nonbook as u32);
    let name_at = 16 + header_len + exth_rec.len();
    set32(&mut r0, 0x54, name_at as u32);
    set32(&mut r0, 0x58, m.title.len() as u32);
    set32(&mut r0, 0x5C, locale(&lang));
    set32(&mut r0, 0x68, 8);
    set32(&mut r0, 0x6C, first_res as u32);
    set32(&mut r0, 0x80, 0x50);
    set32(&mut r0, 0xA4, u32::MAX);
    set32(&mut r0, 0xA8, u32::MAX);
    set32(&mut r0, 0xC0, fdst_at as u32);
    set32(&mut r0, 0xC4, flows.len() as u32);
    set32(&mut r0, 0xC8, fcis_at as u32);
    set32(&mut r0, 0xCC, 1);
    set32(&mut r0, 0xD0, flis_at as u32);
    set32(&mut r0, 0xD4, 1);
    set32(&mut r0, 0xE0, u32::MAX);
    set32(&mut r0, 0xE8, u32::MAX);
    set32(&mut r0, 0xEC, u32::MAX);
    set32(&mut r0, 0xF0, 1);
    set32(&mut r0, 0xF4, ncx_at as u32);
    set32(&mut r0, 0xF8, frag_at as u32);
    set32(&mut r0, 0xFC, skel_at as u32);
    set32(&mut r0, 0x100, u32::MAX);
    set32(&mut r0, 0x104, u32::MAX);
    for o in (0x108..16 + header_len).step_by(4) {
        set32(&mut r0, o, u32::MAX);
    }
    r0.extend_from_slice(&exth_rec);
    r0.extend_from_slice(m.title.as_bytes());
    r0.extend_from_slice(&[0, 0]);
    while !r0.len().is_multiple_of(4) {
        r0.push(0);
    }
    records[0] = r0;

    // The Palm database around them.
    let n = records.len();
    let mut out = Vec::new();
    let mut name: Vec<u8> = m
        .title
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ')
        .collect::<String>()
        .replace(' ', "_")
        .into_bytes();
    name.truncate(31);
    if name.is_empty() {
        name = b"book".to_vec();
    }
    name.resize(32, 0);
    out.extend_from_slice(&name);
    put16(&mut out, 0);
    put16(&mut out, 0);
    let now = chrono::Utc::now().timestamp() as u32;
    put32(&mut out, now);
    put32(&mut out, now);
    put32(&mut out, 0);
    put32(&mut out, 0);
    put32(&mut out, 0);
    put32(&mut out, 0);
    out.extend_from_slice(b"BOOKMOBI");
    put32(&mut out, (2 * n - 1) as u32);
    put32(&mut out, 0);
    put16(&mut out, n as u16);
    let mut offset = 78 + 8 * n + 2;
    for (i, r) in records.iter().enumerate() {
        put32(&mut out, offset as u32);
        let id = (2 * i) as u32;
        out.extend_from_slice(&[0, (id >> 16) as u8, (id >> 8) as u8, id as u8]);
        offset += r.len();
    }
    put16(&mut out, 0);
    for r in &records {
        out.extend_from_slice(r);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn palmdoc_both_ways() {
        let text = "Era uma vez uma casa, uma casa muito antiga — não é? Era uma vez.".repeat(30);
        let packed = super::palmdoc_pack(text.as_bytes());
        assert!(packed.len() < text.len() / 2);
        let mut out = Vec::new();
        crate::pages::palmdoc(&packed, &mut out);
        assert_eq!(String::from_utf8(out).unwrap(), text);
    }

    #[test]
    fn azw3_reads_back() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("b.azw3");
        let mut b = epub_out::tests::sample();
        // A long chapter (many records, characters cut between them) and
        // links between chapters.
        let long: String = (0..400)
            .map(|i| format!("<p>Parágrafo número {i}, com acentos: ação, coração, João.</p>"))
            .collect();
        b.chapters.push(Chapter {
            name: "c0002.xhtml".into(),
            title: "Dois".into(),
            body: format!("<h1 id=\"top\">Dois</h1>{long}<p id=\"end\">Fim. <a href=\"c0001.xhtml\">Volta</a> <a href=\"#top\">topo</a></p>"),
            ..Default::default()
        });
        b.toc = vec![
            Toc {
                title: "Capítulo 1".into(),
                href: "c0001.xhtml".into(),
                kids: vec![],
            },
            Toc {
                title: "Dois".into(),
                href: "c0002.xhtml#end".into(),
                kids: vec![],
            },
        ];
        azw3_out::write(&b, &p).unwrap();
        assert_eq!(source_of(&p), Some(Source::Kindle));
        assert!(!drm::locked(&p, Source::Kindle));
        assert!(crate::pages::kindle_is_kf8(&p));
        let m = crate::ebook::read(&p);
        assert_eq!(m.title, "Um Livro & Outro");
        assert_eq!(m.authors, vec!["Lia Marta"]);
        let r = mobi::read(&p).unwrap();
        assert_eq!(r.chapters.len(), 2);
        assert!(
            r.chapters[0].body.contains("Texto &amp; mais."),
            "{}",
            r.chapters[0].body
        );
        let two = &r.chapters[1].body;
        assert!(
            two.contains("Parágrafo número 399, com acentos: ação, coração, João."),
            "{}",
            &two[..300]
        );
        assert!(
            two.contains("href=\"c0001.xhtml\"") || two.contains("href=\"c0001.xhtml#"),
            "{}",
            &two[two.len() - 400..]
        );
        assert!(r.cover.is_some());
        assert_eq!(r.toc.len(), 2);
        assert!(r.toc[1].href.starts_with("c0002.xhtml#kp1_"), "{:?}", r.toc);
        assert!(r.css.contains("text-indent"));
    }
}
