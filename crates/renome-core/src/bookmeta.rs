//! A book's details written into FB2 and Kindle (MOBI, AZW3) files, and
//! EPUBs polished: brought to EPUB 3 (a navigation page made from the old
//! table of contents) and their large pictures made smaller. EPUB details
//! are written by [`crate::inside`].
//!
//! These files are small (a book is a few megabytes): each is written
//! anew beside itself and takes its place, the old one kept whole in the
//! history's folder for undo ([`Backup::saved`]).

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::inside::BookValues;
use crate::tagwrite::{Backup, temp_of};
use crate::tmdb::xml as esc;

/// The largest book rewritten (an EPUB of scans can be hundreds of
/// megabytes; it is still copied whole for undo).
const MAX_BOOK: u64 = 1 << 30;

fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Books whose details this module writes: FB2 and unlocked Kindle files.
pub fn writable(path: &Path) -> bool {
    match ext_of(path).as_str() {
        "fb2" => true,
        "mobi" | "azw3" | "azw" | "prc" => {
            fs::read(path).is_ok_and(|b| pdb_records(&b).is_some() && !kindle_locked(&b))
        }
        _ => false,
    }
}

/// Writes `v` into the book at `path`; its old content is kept in
/// `store`. `None`: nothing changed.
pub fn write(path: &Path, v: &BookValues, store: &Path) -> Result<Option<Backup>, String> {
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_BOOK {
        return Err("bookTooBig".into());
    }
    let old = fs::read(path).map_err(|e| e.to_string())?;
    let new = match ext_of(path).as_str() {
        "fb2" => fb2_write(&old, v)?,
        "mobi" | "azw3" | "azw" | "prc" => kindle_write(&old, v)?,
        _ => return Err("bookUnsupported".into()),
    };
    let Some(new) = new.filter(|n| *n != old) else {
        return Ok(None);
    };
    let tmp = temp_of(path);
    write_file(&tmp, &new)?;
    replace_keeping(path, &tmp, store).map(Some)
}

fn write_file(to: &Path, bytes: &[u8]) -> Result<(), String> {
    let r = File::create(to).and_then(|mut f| {
        f.write_all(bytes)?;
        f.sync_all()
    });
    r.map_err(|e| {
        let _ = fs::remove_file(to);
        e.to_string()
    })
}

/// Keeps `path`'s file in `store` (copied) and puts `tmp` in its place.
fn replace_keeping(path: &Path, tmp: &Path, store: &Path) -> Result<Backup, String> {
    let fail = |e: String| {
        let _ = fs::remove_file(tmp);
        e
    };
    fs::create_dir_all(store).map_err(|e| fail(e.to_string()))?;
    let base = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "book".into());
    let mut n = 0;
    let name = loop {
        let s = format!("book-{n}-{base}");
        if fs::symlink_metadata(store.join(&s)).is_err() {
            break s;
        }
        n += 1;
    };
    let kept = store.join(&name);
    let copied = fs::copy(path, &kept).and_then(|_| crate::sync_file(&kept));
    if let Err(e) = copied {
        let _ = fs::remove_file(&kept);
        return Err(fail(e.to_string()));
    }
    if let Ok(m) = fs::metadata(path) {
        let _ = fs::set_permissions(tmp, m.permissions());
    }
    if let Err(e) = fs::rename(tmp, path) {
        let _ = fs::remove_file(&kept);
        return Err(fail(e.to_string()));
    }
    Ok(Backup {
        saved: Some(name),
        ..Backup::empty(path)
    })
}

// ------------------------------------------------------------------ FB2

/// The encoding an XML file says it is in (UTF-8 when it does not).
fn xml_encoding(bytes: &[u8]) -> &'static encoding_rs::Encoding {
    if bytes.starts_with(b"\xEF\xBB\xBF") {
        return encoding_rs::UTF_8;
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]).into_owned();
    head.split_once("encoding=")
        .and_then(|(_, r)| {
            let q = r.chars().next()?;
            let r = &r[1..];
            r.split(q).next()
        })
        .and_then(|label| encoding_rs::Encoding::for_label(label.trim().as_bytes()))
        .unwrap_or(encoding_rs::UTF_8)
}

fn author_xml(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    match parts.as_slice() {
        [] => String::new(),
        [one] => format!("<author><nickname>{}</nickname></author>", esc(one)),
        [first @ .., last] => format!(
            "<author><first-name>{}</first-name><last-name>{}</last-name></author>",
            esc(&first.join(" ")),
            esc(last)
        ),
    }
}

/// An element of `parent` rebuilt in its schema's order: for each name in
/// `order`, ours when given, else the old ones as they were; old children
/// with other names after.
fn rebuild(
    text: &str,
    parent: roxmltree::Node,
    order: &[&str],
    ours: &HashMap<&str, Vec<String>>,
    indent: &str,
) -> String {
    let kids: Vec<roxmltree::Node> = parent.children().filter(|n| n.is_element()).collect();
    let mut out = Vec::new();
    for name in order {
        match ours.get(name) {
            Some(list) => out.extend(list.iter().cloned()),
            None => out.extend(
                kids.iter()
                    .filter(|k| k.tag_name().name() == *name)
                    .map(|k| text[k.range()].to_string()),
            ),
        }
    }
    out.extend(
        kids.iter()
            .filter(|k| !order.contains(&k.tag_name().name()))
            .map(|k| text[k.range()].to_string()),
    );
    let tag = parent.tag_name().name();
    let open_end = text[parent.range()].find('>').map_or(0, |i| i + 1);
    let open = &text[parent.range()][..open_end];
    let close_indent = indent.strip_suffix("  ").unwrap_or("");
    format!(
        "{open}{}\n{close_indent}</{tag}>",
        out.iter()
            .map(|e| format!("\n{indent}{e}"))
            .collect::<String>()
    )
}

/// An FB2 with `v` in its `title-info` (authors, title, date, language,
/// series) and `publish-info` (publisher, year, ISBN).
fn fb2_write(bytes: &[u8], v: &BookValues) -> Result<Option<Vec<u8>>, String> {
    let enc = xml_encoding(bytes);
    let (text, _, bad) = enc.decode(bytes);
    if bad {
        return Err("bookUnreadable".into());
    }
    let opts = || roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(&text, opts()).map_err(|e| e.to_string())?;
    let desc = doc
        .descendants()
        .find(|n| n.has_tag_name("description"))
        .ok_or("bookUnreadable")?;
    let title_info = desc
        .children()
        .find(|n| n.has_tag_name("title-info"))
        .ok_or("bookUnreadable")?;
    let mut ours: HashMap<&str, Vec<String>> = HashMap::new();
    if !v.authors.is_empty() {
        ours.insert("author", v.authors.iter().map(|a| author_xml(a)).collect());
    }
    if !v.title.is_empty() {
        ours.insert("book-title", vec![format!("<book-title>{}</book-title>", esc(&v.title))]);
    }
    if !v.year.is_empty() {
        ours.insert("date", vec![format!("<date>{}</date>", esc(&v.year))]);
    }
    if !v.language.is_empty() {
        ours.insert("lang", vec![format!("<lang>{}</lang>", esc(&v.language))]);
    }
    if !v.series.is_empty() {
        // Its number must be a whole number.
        let number = v
            .series_index
            .trim()
            .parse::<u32>()
            .map(|n| format!(" number=\"{n}\""))
            .unwrap_or_default();
        ours.insert("sequence", vec![format!("<sequence name=\"{}\"{number}/>", esc(&v.series))]);
    }
    const TITLE: &[&str] = &[
        "genre", "author", "book-title", "annotation", "keywords", "date", "coverpage", "lang",
        "src-lang", "translator", "sequence",
    ];
    let new_title = rebuild(&text, title_info, TITLE, &ours, "      ");
    let mut edits: Vec<(std::ops::Range<usize>, String)> = vec![(title_info.range(), new_title)];

    let mut pub_ours: HashMap<&str, Vec<String>> = HashMap::new();
    if !v.publisher.is_empty() {
        pub_ours.insert("publisher", vec![format!("<publisher>{}</publisher>", esc(&v.publisher))]);
    }
    if !v.year.is_empty() {
        pub_ours.insert("year", vec![format!("<year>{}</year>", esc(&v.year))]);
    }
    if !v.isbn.is_empty() {
        pub_ours.insert("isbn", vec![format!("<isbn>{}</isbn>", esc(&v.isbn))]);
    }
    const PUBLISH: &[&str] = &["book-name", "publisher", "city", "year", "isbn", "sequence"];
    match desc.children().find(|n| n.has_tag_name("publish-info")) {
        Some(p) => edits.push((p.range(), rebuild(&text, p, PUBLISH, &pub_ours, "      "))),
        None if !pub_ours.is_empty() => {
            // After document-info (else title-info), as the schema wants.
            let after = desc
                .children()
                .rfind(|n| n.has_tag_name("document-info") || n.has_tag_name("title-info"))
                .unwrap_or(title_info);
            let items: String = PUBLISH
                .iter()
                .filter_map(|k| pub_ours.get(k))
                .flatten()
                .map(|e| format!("\n      {e}"))
                .collect();
            let at = after.range().end;
            edits.push((at..at, format!("\n    <publish-info>{items}\n    </publish-info>")));
        }
        None => {}
    }
    edits.sort_by_key(|e| e.0.start);
    let mut out = String::with_capacity(text.len() + 1024);
    let mut at = 0;
    for (r, s) in &edits {
        out.push_str(&text[at..r.start]);
        out.push_str(s);
        at = r.end;
    }
    out.push_str(&text[at..]);
    // Checked before it replaces the book.
    roxmltree::Document::parse_with_options(&out, opts()).map_err(|e| e.to_string())?;
    if out == text {
        return Ok(None);
    }
    let mut bytes_out = Vec::new();
    if bytes.starts_with(b"\xEF\xBB\xBF") {
        bytes_out.extend_from_slice(b"\xEF\xBB\xBF");
    }
    // Characters the old encoding lacks become numeric references.
    bytes_out.extend_from_slice(&enc.encode(&out).0);
    Ok(Some(bytes_out))
}

// --------------------------------------------------------------- Kindle

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The records of a Palm database: their start offsets (and the end).
fn pdb_records(b: &[u8]) -> Option<Vec<usize>> {
    if b.get(60..68)? != b"BOOKMOBI" {
        return None;
    }
    let n = usize::from(be16(b, 76)?);
    let mut offs = Vec::with_capacity(n + 1);
    for i in 0..n {
        offs.push(be32(b, 78 + i * 8)? as usize);
    }
    offs.push(b.len());
    offs.windows(2).all(|w| w[0] <= w[1] && w[1] <= b.len()).then_some(offs)
}

/// Encrypted (DRM): never touched.
fn kindle_locked(b: &[u8]) -> bool {
    pdb_records(b)
        .and_then(|o| be16(b, o[0] + 12))
        .is_some_and(|e| e != 0)
}

/// A Kindle header record (`MOBI` at 16) with `v` in its EXTH records and
/// full name; what else it held kept.
fn kindle_header(r0: &[u8], v: &BookValues) -> Option<Vec<u8>> {
    if r0.get(16..20)? != b"MOBI" {
        return None;
    }
    let mobi_len = be32(r0, 20)? as usize;
    let utf8 = be32(r0, 28)? == 65001;
    let enc = |s: &str| -> Vec<u8> {
        if utf8 {
            s.as_bytes().to_vec()
        } else {
            encoding_rs::WINDOWS_1252.encode(s).0.into_owned()
        }
    };
    let head_end = 16 + mobi_len;
    let name_at = be32(r0, 84)? as usize;
    let name_len = be32(r0, 88)? as usize;
    let flags = be32(r0, 128)?;
    // The old EXTH records.
    let mut records: Vec<(u32, Vec<u8>)> = Vec::new();
    if flags & 0x40 != 0 && r0.get(head_end..head_end + 4)? == b"EXTH" {
        let count = be32(r0, head_end + 8)? as usize;
        let mut at = head_end + 12;
        for _ in 0..count {
            let ty = be32(r0, at)?;
            let len = be32(r0, at + 4)? as usize;
            if len < 8 {
                return None;
            }
            records.push((ty, r0.get(at + 8..at + len)?.to_vec()));
            at += len;
        }
    }
    let mut ours: Vec<(u32, Vec<u8>)> = Vec::new();
    for a in &v.authors {
        ours.push((100, enc(a)));
    }
    if !v.publisher.is_empty() {
        ours.push((101, enc(&v.publisher)));
    }
    if !v.isbn.is_empty() {
        ours.push((104, enc(&v.isbn)));
    }
    if !v.year.is_empty() {
        ours.push((106, enc(&v.year)));
    }
    if !v.title.is_empty() {
        ours.push((503, enc(&v.title)));
    }
    if !v.language.is_empty() {
        ours.push((524, enc(&v.language)));
    }
    let replaced: Vec<u32> = ours.iter().map(|o| o.0).collect();
    records.retain(|(t, _)| !replaced.contains(t));
    records.extend(ours);
    let mut body = Vec::new();
    for (t, d) in &records {
        body.extend(t.to_be_bytes());
        body.extend(((d.len() + 8) as u32).to_be_bytes());
        body.extend(d);
    }
    let pad = (4 - (body.len() + 12) % 4) % 4;
    let mut exth = b"EXTH".to_vec();
    exth.extend(((12 + body.len()) as u32).to_be_bytes());
    exth.extend((records.len() as u32).to_be_bytes());
    exth.extend(body);
    exth.extend(std::iter::repeat_n(0u8, pad));
    let name = if v.title.is_empty() {
        r0.get(name_at..name_at + name_len)?.to_vec()
    } else {
        enc(&v.title)
    };
    // What followed the old name (padding a Kindle may use) stays.
    let rest = r0.get((name_at + name_len).min(r0.len())..)?;
    let rest_trimmed = rest.iter().position(|b| *b != 0).map_or(&[][..], |_| rest);
    let mut out = r0.get(..head_end)?.to_vec();
    out.extend(&exth);
    let new_name_at = out.len();
    out.extend(&name);
    // Two zeros after the name, then four-byte alignment; the old padding
    // length (at least) kept, since some readers want room there.
    out.extend([0, 0]);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    if rest_trimmed.is_empty() {
        let keep = rest.len().saturating_sub(2);
        out.extend(std::iter::repeat_n(0u8, keep));
    } else {
        out.extend(rest_trimmed);
    }
    out[84..88].copy_from_slice(&(new_name_at as u32).to_be_bytes());
    out[88..92].copy_from_slice(&(name.len() as u32).to_be_bytes());
    out[128..132].copy_from_slice(&(flags | 0x40).to_be_bytes());
    Some(out)
}

/// A Kindle book with `v` in its header (and in the KF8 part's header of
/// a book holding both formats); the other records as they were.
fn kindle_write(b: &[u8], v: &BookValues) -> Result<Option<Vec<u8>>, String> {
    let offs = pdb_records(b).ok_or("bookUnreadable")?;
    if kindle_locked(b) {
        return Err("bookDrm".into());
    }
    let rec = |i: usize| &b[offs[i]..offs[i + 1]];
    let n = offs.len() - 1;
    let mut records: Vec<Vec<u8>> = (0..n).map(|i| rec(i).to_vec()).collect();
    let first = kindle_header(&records[0], v).ok_or("bookUnreadable")?;
    // The KF8 part of a combined book: EXTH 121 says where it starts.
    let boundary = exth_value(&records[0], 121)
        .and_then(|d| d.get(..4).map(|x| u32::from_be_bytes(x.try_into().unwrap()) as usize))
        .filter(|&i| i > 0 && i < n);
    records[0] = first;
    if let Some(i) = boundary
        && let Some(h) = kindle_header(&records[i], v)
    {
        records[i] = h;
    }
    // The database again: its header and record list, the old bytes
    // between the list and the first record, the records.
    let list_end = 78 + n * 8;
    let gap = b.get(list_end..offs[0]).unwrap_or(&[]);
    let mut out = b[..list_end].to_vec();
    let mut at = (list_end + gap.len()) as u32;
    for (i, r) in records.iter().enumerate() {
        out[78 + i * 8..82 + i * 8].copy_from_slice(&at.to_be_bytes());
        at += r.len() as u32;
    }
    out.extend(gap);
    for r in &records {
        out.extend(r);
    }
    Ok(Some(out))
}

/// The data of the first EXTH record of type `ty` in a header record.
fn exth_value(r0: &[u8], ty: u32) -> Option<Vec<u8>> {
    let head_end = 16 + be32(r0, 20)? as usize;
    if r0.get(head_end..head_end + 4)? != b"EXTH" {
        return None;
    }
    let count = be32(r0, head_end + 8)? as usize;
    let mut at = head_end + 12;
    for _ in 0..count {
        let t = be32(r0, at)?;
        let len = be32(r0, at + 4)? as usize;
        if len < 8 {
            return None;
        }
        if t == ty {
            return r0.get(at + 8..at + len).map(<[u8]>::to_vec);
        }
        at += len;
    }
    None
}

// ---------------------------------------------------------- EPUB polish

/// What polishing an EPUB does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Polish {
    /// EPUB 2 brought to EPUB 3: a navigation page made from the NCX.
    pub epub3: bool,
    /// Pictures larger than `MAX_SIDE` made that size; large ones
    /// compressed again when that saves much.
    pub shrink: bool,
}

/// `%20` and the like in a link, read.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(h) = b.get(i + 1..i + 3)
            && let Ok(v) = u8::from_str_radix(std::str::from_utf8(h).unwrap_or("zz"), 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// The longest side a picture keeps: sharp on any reader's screen.
const MAX_SIDE: u32 = 1600;

/// What [`polish`] did.
#[derive(Debug, Default)]
pub struct Polished {
    pub backup: Option<Backup>,
    /// Bytes saved by making pictures smaller.
    pub saved_bytes: u64,
}

/// Polishes the EPUB at `path` (and writes `values` into it, when given,
/// in the same pass); the old file is kept whole in `store`.
pub fn polish(
    path: &Path,
    p: Polish,
    values: Option<&BookValues>,
    store: &Path,
) -> Result<Polished, String> {
    if !crate::inside::book_writable(path) {
        return Err("bookUnsupported".into());
    }
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_BOOK {
        return Err("bookTooBig".into());
    }
    let mut zip = zip::ZipArchive::new(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let names: Vec<String> = zip
        .file_names()
        .filter_map(Result::ok)
        .map(|n| n.into_owned())
        .collect();
    // Locked books are never touched (fonts merely obfuscated are fine:
    // the book's own identifier, their key, stays).
    if crate::bookconv::drm::epub_locked(path) {
        return Err("epubDrm".into());
    }
    let read = |zip: &mut zip::ZipArchive<File>, name: &str| -> Result<Vec<u8>, String> {
        let mut f = zip.by_name(name).map_err(|e| e.to_string())?;
        let mut b = Vec::new();
        f.read_to_end(&mut b).map_err(|e| e.to_string())?;
        Ok(b)
    };
    let container = String::from_utf8(read(&mut zip, "META-INF/container.xml")?)
        .map_err(|_| "notUtf8".to_string())?;
    let opf_name = {
        let doc = roxmltree::Document::parse(container.trim_start_matches('\u{feff}'))
            .map_err(|e| e.to_string())?;
        doc.descendants()
            .find(|n| n.has_tag_name("rootfile"))
            .and_then(|n| n.attribute("full-path"))
            .ok_or("epubNoOpf")?
            .to_string()
    };
    let opf_dir = opf_name.rsplit_once('/').map_or("", |(d, _)| d).to_string();
    let joined = |href: &str| {
        let href = percent_decode(href);
        if opf_dir.is_empty() {
            href
        } else {
            format!("{opf_dir}/{href}")
        }
    };
    let old_opf = String::from_utf8(read(&mut zip, &opf_name)?).map_err(|_| "notUtf8".to_string())?;
    let mut opf = match values {
        Some(v) => crate::inside::book_opf(&old_opf, v).unwrap_or_else(|| old_opf.clone()),
        None => old_opf.clone(),
    };
    // New entries (the navigation page) and replaced ones (pictures).
    let mut added: Vec<(String, Vec<u8>)> = Vec::new();
    let mut replaced: HashMap<String, Vec<u8>> = HashMap::new();
    if p.epub3
        && let Some((new_opf, nav)) = to_epub3(&opf, &mut |href| read(&mut zip, &joined(href)).ok(), &names, &joined)
    {
        opf = new_opf;
        added.push(nav);
    }
    let mut saved_bytes = 0u64;
    if p.shrink {
        let doc = roxmltree::Document::parse(opf.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
        let pictures: Vec<String> = doc
            .descendants()
            .filter(|n| n.has_tag_name("item"))
            .filter(|n| matches!(n.attribute("media-type"), Some("image/jpeg" | "image/png")))
            .filter_map(|n| n.attribute("href").map(&joined))
            .collect();
        for name in pictures {
            let Ok(old) = read(&mut zip, &name) else { continue };
            if let Some(new) = smaller(&old) {
                saved_bytes += (old.len() - new.len()) as u64;
                replaced.insert(name, new);
            }
        }
    }
    if opf == old_opf && added.is_empty() && replaced.is_empty() {
        return Ok(Polished::default());
    }
    // The book written again: the same entries in the same order (the
    // `mimetype` first, stored), the changed ones replaced.
    let tmp = temp_of(path);
    let written = (|| -> Result<(), String> {
        let out = File::create(&tmp).map_err(|e| e.to_string())?;
        let mut zout = zip::ZipWriter::new(BufWriter::new(out));
        let deflated = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for i in 0..zip.len() {
            let f = zip.by_index_raw(i).map_err(|e| e.to_string())?;
            let name = f.name().map_err(|e| e.to_string())?.into_owned();
            if name == opf_name {
                drop(f);
                zout.start_file(&name, deflated).map_err(|e| e.to_string())?;
                zout.write_all(opf.as_bytes()).map_err(|e| e.to_string())?;
            } else if let Some(b) = replaced.get(&name) {
                drop(f);
                // Pictures are compressed already.
                zout.start_file(&name, stored).map_err(|e| e.to_string())?;
                zout.write_all(b).map_err(|e| e.to_string())?;
            } else {
                zout.raw_copy_file(f).map_err(|e| e.to_string())?;
            }
        }
        for (name, b) in &added {
            zout.start_file(name, deflated).map_err(|e| e.to_string())?;
            zout.write_all(b).map_err(|e| e.to_string())?;
        }
        let w = zout.finish().map_err(|e| e.to_string())?;
        let file = w.into_inner().map_err(|e| e.error().to_string())?;
        file.sync_all().map_err(|e| e.to_string())
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    // Read back before it replaces the book.
    if zip::ZipArchive::new(File::open(&tmp).map_err(|e| e.to_string())?).is_err() {
        let _ = fs::remove_file(&tmp);
        return Err("bookUnreadable".into());
    }
    let backup = replace_keeping(path, &tmp, store)?;
    Ok(Polished {
        backup: Some(backup),
        saved_bytes,
    })
}

/// A picture made smaller: at most `MAX_SIDE` on its longest side; a
/// JPEG over 300 KB compressed again. `None` when that does not save at
/// least a tenth.
fn smaller(old: &[u8]) -> Option<Vec<u8>> {
    let png = old.starts_with(b"\x89PNG");
    let fmt = if png {
        image::ImageFormat::Png
    } else if old.starts_with(&[0xff, 0xd8]) {
        image::ImageFormat::Jpeg
    } else {
        return None;
    };
    let (w, h) = image::ImageReader::with_format(std::io::Cursor::new(old), fmt)
        .into_dimensions()
        .ok()?;
    let big = w.max(h) > MAX_SIDE;
    if !big && (png || old.len() < 300 << 10) {
        return None;
    }
    let img = image::load_from_memory_with_format(old, fmt).ok()?;
    let img = if big {
        img.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    let mut out = Vec::new();
    if png {
        let enc = image::codecs::png::PngEncoder::new_with_quality(
            &mut out,
            image::codecs::png::CompressionType::Best,
            image::codecs::png::FilterType::Adaptive,
        );
        img.write_with_encoder(enc).ok()?;
    } else {
        // A JPEG has no transparency: drawn on white if it had any.
        let rgb = img.to_rgb8();
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85);
        rgb.write_with_encoder(enc).ok()?;
    }
    (out.len() * 10 < old.len() * 9).then_some(out)
}

/// An EPUB 2 package as EPUB 3: version 3.0, a modification date, the
/// cover picture marked, and a navigation page made from the NCX (kept
/// for older readers). The new OPF and the page (its name and bytes).
fn to_epub3(
    opf: &str,
    read: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
    names: &[String],
    joined: &dyn Fn(&str) -> String,
) -> Option<(String, (String, Vec<u8>))> {
    let doc = roxmltree::Document::parse(opf.trim_start_matches('\u{feff}')).ok()?;
    let pkg = doc.root_element();
    if !pkg.attribute("version").is_some_and(|v| v.starts_with('2')) {
        return None;
    }
    let manifest = pkg.children().find(|n| n.has_tag_name("manifest"))?;
    let metadata = pkg.children().find(|n| n.has_tag_name("metadata"))?;
    let items: Vec<roxmltree::Node> = manifest.children().filter(|n| n.has_tag_name("item")).collect();
    let ncx = items
        .iter()
        .find(|n| n.attribute("media-type") == Some("application/x-dtbncx+xml"))?;
    let ncx_href = ncx.attribute("href")?;
    let ncx_bytes = read(ncx_href)?;
    let ncx_text = String::from_utf8_lossy(&ncx_bytes).into_owned();
    let ncx_doc = roxmltree::Document::parse(ncx_text.trim_start_matches('\u{feff}')).ok()?;
    let title = ncx_doc
        .descendants()
        .find(|n| n.has_tag_name("docTitle"))
        .and_then(|n| n.descendants().find(|t| t.has_tag_name("text")))
        .and_then(|t| t.text())
        .unwrap_or("")
        .trim()
        .to_string();
    // Links in the NCX are relative to it; the page sits beside the OPF.
    let ncx_dir = ncx_href.rsplit_once('/').map_or("", |(d, _)| d);
    fn points(n: roxmltree::Node, dir: &str, depth: usize, out: &mut String) {
        let kids: Vec<roxmltree::Node> = n.children().filter(|c| c.has_tag_name("navPoint")).collect();
        if kids.is_empty() || depth > 8 {
            return;
        }
        out.push_str("<ol>");
        for p in kids {
            let label = p
                .children()
                .find(|c| c.has_tag_name("navLabel"))
                .and_then(|l| l.descendants().find(|t| t.has_tag_name("text")))
                .and_then(|t| t.text())
                .unwrap_or("")
                .trim();
            let src = p
                .children()
                .find(|c| c.has_tag_name("content"))
                .and_then(|c| c.attribute("src"))
                .unwrap_or("");
            let href = if dir.is_empty() || src.is_empty() {
                src.to_string()
            } else {
                format!("{dir}/{src}")
            };
            let label = if label.is_empty() { "—" } else { label };
            out.push_str(&format!("<li><a href=\"{}\">{}</a>", esc(&href), esc(label)));
            points(p, dir, depth + 1, out);
            out.push_str("</li>");
        }
        out.push_str("</ol>");
    }
    let map = ncx_doc.descendants().find(|n| n.has_tag_name("navMap"))?;
    let mut list = String::new();
    points(map, ncx_dir, 0, &mut list);
    if list.is_empty() {
        list.push_str("<ol><li><a href=\"\">—</a></li></ol>");
    }
    let page = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\">\n<head><meta charset=\"utf-8\"/><title>{}</title></head>\n<body><nav epub:type=\"toc\" id=\"toc\"><h1>{}</h1>{list}</nav></body>\n</html>\n",
        esc(&title),
        esc(&title)
    );
    // A name not taken, an id not taken.
    let mut href = "nav.xhtml".to_string();
    let mut k = 1;
    while names.iter().any(|n| *n == joined(&href)) {
        href = format!("nav{k}.xhtml");
        k += 1;
    }
    let ids: Vec<&str> = items.iter().filter_map(|n| n.attribute("id")).collect();
    let mut id = "nav".to_string();
    let mut k = 1;
    while ids.contains(&id.as_str()) {
        id = format!("nav{k}");
        k += 1;
    }
    let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    let base = opf.len() - opf.trim_start_matches('\u{feff}').len();
    let at = |r: std::ops::Range<usize>| (r.start + base)..(r.end + base);
    // version="2.0" → 3.0.
    let pkg_open = at(pkg.range());
    let open_text = &opf[pkg_open.start..];
    let open_end = open_text.find('>')?;
    let open_tag = &open_text[..open_end];
    let vpos = open_tag.find("version")?;
    let q = open_tag[vpos..].find(['"', '\''])? + vpos;
    let quote = &open_tag[q..q + 1];
    let vend = open_tag[q + 1..].find(quote)? + q + 1;
    edits.push(((pkg_open.start + q + 1)..(pkg_open.start + vend), "3.0".into()));
    // The navigation page in the manifest.
    let man_end = at(manifest.range()).end;
    let close = opf[..man_end].rfind("</")?;
    edits.push((
        close..close,
        format!(
            "  <item id=\"{id}\" href=\"{}\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n  ",
            esc(&href)
        ),
    ));
    // The cover picture, as EPUB 3 marks it.
    let cover_id = metadata
        .children()
        .find(|n| n.has_tag_name("meta") && n.attribute("name") == Some("cover"))
        .and_then(|n| n.attribute("content"));
    if let Some(cid) = cover_id
        && let Some(item) = items.iter().find(|n| n.attribute("id") == Some(cid))
        && item.attribute("properties").is_none()
    {
        let r = at(item.range());
        let tag = &opf[r.clone()];
        let end = if tag.ends_with("/>") { r.end - 2 } else { r.start + tag.find('>')? };
        edits.push((end..end, " properties=\"cover-image\"".into()));
    }
    // EPUB 3 wants a modification date.
    let has_modified = metadata
        .children()
        .any(|n| n.has_tag_name("meta") && n.attribute("property") == Some("dcterms:modified"));
    if !has_modified {
        let meta_end = at(metadata.range()).end;
        let close = opf[..meta_end].rfind("</")?;
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
        edits.push((close..close, format!("  <meta property=\"dcterms:modified\">{now}</meta>\n  ")));
    }
    edits.sort_by_key(|e| e.0.start);
    let mut out = String::with_capacity(opf.len() + 512);
    let mut pos = 0;
    for (r, s) in &edits {
        out.push_str(&opf[pos..r.start]);
        out.push_str(s);
        pos = r.end;
    }
    out.push_str(&opf[pos..]);
    roxmltree::Document::parse(out.trim_start_matches('\u{feff}')).ok()?;
    Some((out, (joined(&href), page.into_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> BookValues {
        BookValues {
            title: "Título Novo".into(),
            authors: vec!["Ana Maria Souza".into(), "Rui".into()],
            year: "2001".into(),
            publisher: "Editora X".into(),
            language: "pt".into(),
            isbn: "9780000000002".into(),
            series: "Saga".into(),
            series_index: "2".into(),
        }
    }

    const FB2: &str = r#"<?xml version="1.0" encoding="windows-1251"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">
  <description>
    <title-info>
      <genre>prose</genre>
      <author><first-name>Old</first-name><last-name>Name</last-name></author>
      <book-title>Old</book-title>
      <annotation><p>Keep me</p></annotation>
      <lang>ru</lang>
    </title-info>
    <document-info><id>1</id></document-info>
  </description>
  <body><section><p>Текст</p></section></body>
</FictionBook>
"#;

    #[test]
    fn fb2_details_written() {
        let old = encoding_rs::WINDOWS_1251.encode(FB2).0.into_owned();
        let new = fb2_write(&old, &values()).unwrap().unwrap();
        let (text, _, bad) = encoding_rs::WINDOWS_1251.decode(&new);
        assert!(!bad);
        assert!(text.contains("<genre>prose</genre>"));
        assert!(text.contains("<annotation><p>Keep me</p></annotation>"));
        assert!(text.contains("<first-name>Ana Maria</first-name><last-name>Souza</last-name>"));
        assert!(text.contains("<nickname>Rui</nickname>"));
        // Not in windows-1251: a numeric reference.
        assert!(text.contains("<book-title>T&#237;tulo Novo</book-title>") || text.contains("Título Novo"));
        assert!(text.contains("<sequence name=\"Saga\" number=\"2\"/>"));
        assert!(text.contains("<publish-info>"));
        assert!(text.contains("<isbn>9780000000002</isbn>"));
        assert!(text.contains("Текст"));
        // In schema order: author before book-title, lang before sequence.
        let pos = |s: &str| text.find(s).unwrap();
        assert!(pos("<author>") < pos("<book-title>"));
        assert!(pos("<lang>") < pos("<sequence"));
        assert!(pos("</document-info>") < pos("<publish-info>"));
        // Again: nothing more to change.
        assert_eq!(fb2_write(&new, &values()).unwrap(), Some(new.clone()));
    }

    #[test]
    fn kindle_details_written() {
        let book = crate::bookconv::Book {
            meta: crate::bookconv::Meta {
                title: "Old".into(),
                authors: vec!["Someone".into()],
                language: "en".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut book = book;
        book.chapters.push(crate::bookconv::Chapter {
            title: "One".into(),
            body: "<p>Hello there.</p>".into(),
            ..Default::default()
        });
        let old = crate::bookconv::azw3_out::build(&book).unwrap();
        let new = kindle_write(&old, &values()).unwrap().unwrap();
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("b.azw3");
        fs::write(&p, &new).unwrap();
        let back = crate::bookconv::mobi::read(&p).unwrap();
        assert_eq!(back.meta.title, "Título Novo");
        assert_eq!(back.meta.authors, ["Ana Maria Souza", "Rui"]);
        assert!(back.chapters.iter().any(|c| c.body.contains("Hello there.")));
        // The text records are where they were, shifted as a whole.
        let ro = pdb_records(&old).unwrap();
        let rn = pdb_records(&new).unwrap();
        assert_eq!(ro.len(), rn.len());
        assert_eq!(&old[ro[1]..ro[2]], &new[rn[1]..rn[2]]);
    }

    #[test]
    fn epub_polished_and_put_back() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("b.epub");
        // An EPUB 2 with a large picture.
        let big = {
            let img = image::RgbImage::from_fn(3000, 2000, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]));
            let mut b = Vec::new();
            image::DynamicImage::ImageRgb8(img)
                .write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(&mut b, 95))
                .unwrap();
            b
        };
        let opf = r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Old</dc:title><dc:identifier id="id">x</dc:identifier><dc:language>en</dc:language>
    <meta name="cover" content="cov"/>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="c1" href="Text/c1.xhtml" media-type="application/xhtml+xml"/>
    <item id="cov" href="Images/cover.jpg" media-type="image/jpeg"/>
  </manifest>
  <spine toc="ncx"><itemref idref="c1"/></spine>
</package>"#;
        let ncx = r#"<?xml version="1.0" encoding="utf-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><docTitle><text>Old</text></docTitle>
<navMap><navPoint id="p1" playOrder="1"><navLabel><text>Chapter &amp; One</text></navLabel><content src="Text/c1.xhtml"/>
<navPoint id="p2" playOrder="2"><navLabel><text>Part</text></navLabel><content src="Text/c1.xhtml#a"/></navPoint></navPoint></navMap></ncx>"#;
        {
            let mut z = zip::ZipWriter::new(File::create(&p).unwrap());
            let st = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            z.start_file("mimetype", st).unwrap();
            z.write_all(b"application/epub+zip").unwrap();
            for (n, b) in [
                ("META-INF/container.xml", br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_vec()),
                ("OEBPS/content.opf", opf.as_bytes().to_vec()),
                ("OEBPS/toc.ncx", ncx.as_bytes().to_vec()),
                ("OEBPS/Text/c1.xhtml", b"<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>Hi</p></body></html>".to_vec()),
                ("OEBPS/Images/cover.jpg", big.clone()),
            ] {
                z.start_file(n, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(&b).unwrap();
            }
            z.finish().unwrap();
        }
        let old = fs::read(&p).unwrap();
        let store = d.path().join("store");
        let done = polish(&p, Polish { epub3: true, shrink: true }, Some(&values()), &store).unwrap();
        assert!(done.saved_bytes > 0);
        let mut z = zip::ZipArchive::new(File::open(&p).unwrap()).unwrap();
        assert_eq!(z.by_index(0).unwrap().name().unwrap(), "mimetype");
        let mut s = String::new();
        z.by_name("OEBPS/content.opf").unwrap().read_to_string(&mut s).unwrap();
        assert!(s.contains("version=\"3.0\""));
        assert!(s.contains("properties=\"nav\""));
        assert!(s.contains("properties=\"cover-image\""));
        assert!(s.contains("dcterms:modified"));
        assert!(s.contains("Título Novo"));
        let mut nav = String::new();
        z.by_name("OEBPS/nav.xhtml").unwrap().read_to_string(&mut nav).unwrap();
        assert!(nav.contains("<a href=\"Text/c1.xhtml\">Chapter &amp; One</a><ol><li><a href=\"Text/c1.xhtml#a\">Part</a>"));
        let mut pic = Vec::new();
        z.by_name("OEBPS/Images/cover.jpg").unwrap().read_to_end(&mut pic).unwrap();
        let (w, h) = image::ImageReader::new(std::io::Cursor::new(&pic))
            .with_guessed_format()
            .unwrap()
            .into_dimensions()
            .unwrap();
        assert_eq!((w, h), (1600, 1067));
        // Read by the book reader as EPUB 3, with its chapter.
        let book = crate::bookconv::epub_in::read(&p).unwrap();
        assert!(book.chapters.iter().any(|c| c.body.contains("Hi")));
        // Undo.
        let b = done.backup.unwrap();
        crate::subtext::restore(&p, b.saved.as_ref().unwrap(), &store).unwrap();
        assert_eq!(fs::read(&p).unwrap(), old);
        // Nothing to do on an EPUB 3 without large pictures.
        let none = polish(&p, Polish { epub3: false, shrink: false }, None, &store).unwrap();
        assert!(none.backup.is_none());
    }
}
