//! Text subtitles taken out of Matroska videos (`.mkv`, `.webm`): SubRip,
//! SubStation and WebVTT tracks, written beside the video in their own
//! format or as SubRip. Picture tracks (VobSub, PGS, DVB) are counted but
//! not taken out: their letters would have to be recognised.
//!
//! The whole file is walked once, cluster by cluster: only the headers of
//! the other tracks' blocks are read, the rest skipped.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::subtext::Tools;
use crate::subtitles::Lang;
use crate::video::{ebml, ebml_uint, matroska_element, vint};

const SEGMENT: u64 = 0x1853_8067;
const CLUSTER: u64 = 0x1f43_b675;
const INFO: u64 = 0x1549_a966;
const TRACKS: u64 = 0x1654_ae6b;
/// The largest block of text read: a subtitle line is a few hundred bytes.
const MAX_BLOCK: u64 = 4 << 20;
/// The most text taken out of one track.
const MAX_TRACK: usize = 64 << 20;

/// What a subtitle track holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Codec {
    Srt,
    Ass,
    Vtt,
    /// Pictures (or text this module does not read).
    Other,
}

impl Codec {
    fn of(id: &str) -> Self {
        match id {
            "S_TEXT/UTF8" | "S_TEXT/ASCII" => Codec::Srt,
            "S_TEXT/ASS" | "S_TEXT/SSA" | "S_ASS" | "S_SSA" => Codec::Ass,
            "S_TEXT/WEBVTT" | "D_WEBVTT/SUBTITLES" | "D_WEBVTT/CAPTIONS" | "D_WEBVTT/DESCRIPTIONS" => {
                Codec::Vtt
            }
            _ => Codec::Other,
        }
    }

    fn ext(self) -> &'static str {
        match self {
            Codec::Srt | Codec::Other => "srt",
            Codec::Ass => "ass",
            Codec::Vtt => "vtt",
        }
    }
}

/// How a track's blocks are stored.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Coding {
    Plain,
    Zlib,
    /// Bytes taken off the start of every block.
    Stripped(Vec<u8>),
    /// Encrypted or compressed in a way not read here.
    Unreadable,
}

/// A subtitle track inside a video.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub number: u64,
    pub codec: Codec,
    /// As the track says (`por`, `pt-BR`; `eng` when it says nothing).
    pub lang: String,
    pub name: String,
    pub forced: bool,
    pub hearing: bool,
    #[serde(skip)]
    private: Vec<u8>,
    #[serde(skip)]
    coding: Coding,
    /// The codec's header is zlib-compressed.
    #[serde(skip)]
    private_zlib: bool,
    /// Nanoseconds a block lasts when it does not say.
    #[serde(skip)]
    default_ns: u64,
}

impl Track {
    /// A text track that can be taken out.
    pub fn readable(&self) -> bool {
        self.codec != Codec::Other && self.coding != Coding::Unreadable
    }
}

fn is_matroska(path: &Path) -> bool {
    let mut head = [0u8; 4];
    File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok()
        && head == [0x1a, 0x45, 0xdf, 0xa3]
}

fn text(data: &[u8]) -> String {
    String::from_utf8_lossy(data).trim_end_matches('\0').to_string()
}

/// The subtitle tracks of a Matroska video (none for other files).
pub fn tracks(path: &Path) -> Vec<Track> {
    if !is_matroska(path) {
        return Vec::new();
    }
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let mut f = BufReader::new(file);
    let Some(body) = matroska_element(&mut f, TRACKS) else {
        return Vec::new();
    };
    ebml(&body)
        .filter(|(id, _)| *id == 0xae)
        .filter_map(|(_, entry)| {
            let mut kind = 0;
            let mut t = Track {
                number: 0,
                codec: Codec::Other,
                lang: String::new(),
                name: String::new(),
                forced: false,
                hearing: false,
                private: Vec::new(),
                coding: Coding::Plain,
                private_zlib: false,
                default_ns: 0,
            };
            let (mut lang, mut bcp47) = (None, None);
            for (id, data) in ebml(entry) {
                match id {
                    0xd7 => t.number = ebml_uint(data),
                    0x83 => kind = ebml_uint(data),
                    0x86 => t.codec = Codec::of(&text(data)),
                    0x63a2 => t.private = data.to_vec(),
                    0x22_b59c => lang = Some(text(data)),
                    0x22_b59d => bcp47 = Some(text(data)),
                    0x536e => t.name = text(data),
                    0x55aa => t.forced = ebml_uint(data) == 1,
                    0x55ab => t.hearing = ebml_uint(data) == 1,
                    0x23_e383 => t.default_ns = ebml_uint(data),
                    0x6d80 => (t.coding, t.private_zlib) = coding(data),
                    _ => {}
                }
            }
            t.lang = bcp47
                .or(lang)
                .filter(|l| !l.is_empty())
                .unwrap_or_else(|| "eng".into());
            // The codec's header may be compressed too.
            if t.private_zlib {
                match decode_block(&Coding::Zlib, &t.private) {
                    Some(p) => t.private = p,
                    None => t.coding = Coding::Unreadable,
                }
            }
            (kind == 0x11 && t.number > 0).then_some(t)
        })
        .collect()
}

/// How a track's content encodings store its blocks, and whether its
/// codec's header is zlib-compressed.
fn coding(encodings: &[u8]) -> (Coding, bool) {
    let mut out = Coding::Plain;
    let mut private = false;
    for (id, enc) in ebml(encodings) {
        if id != 0x6240 {
            continue;
        }
        let mut scope = 1;
        let mut kind = 0;
        let (mut algo, mut settings) = (0, Vec::new());
        let mut encrypted = false;
        for (id, data) in ebml(enc) {
            match id {
                0x5032 => scope = ebml_uint(data),
                0x5033 => kind = ebml_uint(data),
                0x5034 => {
                    for (id, data) in ebml(data) {
                        match id {
                            0x4254 => algo = ebml_uint(data),
                            0x4255 => settings = data.to_vec(),
                            _ => {}
                        }
                    }
                }
                0x5035 => encrypted = true,
                _ => {}
            }
        }
        if kind == 1 || encrypted {
            return (Coding::Unreadable, false);
        }
        if scope & 2 != 0 {
            if algo != 0 {
                return (Coding::Unreadable, false);
            }
            private = true;
        }
        // Only the codec's header: the blocks are plain.
        if scope & 1 == 0 {
            continue;
        }
        out = match algo {
            0 => Coding::Zlib,
            3 => Coding::Stripped(settings),
            _ => return (Coding::Unreadable, false),
        };
    }
    (out, private)
}

/// A block's bytes as stored before encoding (`None`: as they are).
fn decode_block(coding: &Coding, data: &[u8]) -> Option<Vec<u8>> {
    match coding {
        Coding::Plain | Coding::Unreadable => None,
        Coding::Stripped(head) => {
            let mut v = head.clone();
            v.extend_from_slice(data);
            Some(v)
        }
        Coding::Zlib => {
            if data.is_empty() {
                return Some(Vec::new());
            }
            let mut out = Vec::new();
            flate2::read::ZlibDecoder::new(data)
                .take(MAX_BLOCK)
                .read_to_end(&mut out)
                .ok()?;
            Some(out)
        }
    }
}

/// One subtitle line taken out: when (nanoseconds) and its bytes.
#[derive(Debug, Clone)]
struct Event {
    start: i64,
    duration: Option<i64>,
    data: Vec<u8>,
}

/// Reads an element's id and size from the stream: id, size (`None`:
/// unknown) and the header's length.
fn header<R: Read>(r: &mut R) -> Option<(u64, Option<u64>, u64)> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b[..1]).ok()?;
    let len = b[0].leading_zeros() as usize + 1;
    if len > 4 {
        return None;
    }
    r.read_exact(&mut b[1..len]).ok()?;
    let (id, _) = vint(&b[..len], true)?;
    let mut s = [0u8; 8];
    r.read_exact(&mut s[..1]).ok()?;
    let slen = s[0].leading_zeros() as usize + 1;
    if slen > 8 {
        return None;
    }
    r.read_exact(&mut s[1..slen]).ok()?;
    let (size, _) = vint(&s[..slen], false)?;
    Some((id, (size != u64::MAX).then_some(size), (len + slen) as u64))
}

/// The frames of a block (`Block` or `SimpleBlock` content): track,
/// time relative to its cluster, and frames (several when laced).
fn frames(data: &[u8]) -> Option<(u64, i16, Vec<&[u8]>)> {
    let (track, n) = vint(data, false)?;
    let rel = i16::from_be_bytes(data.get(n..n + 2)?.try_into().ok()?);
    let flags = *data.get(n + 2)?;
    let body = data.get(n + 3..)?;
    let lacing = (flags >> 1) & 3;
    if lacing == 0 {
        return Some((track, rel, vec![body]));
    }
    let count = usize::from(*body.first()?) + 1;
    let mut at = 1;
    let mut sizes: Vec<usize> = Vec::with_capacity(count);
    match lacing {
        // Xiph: each size as bytes of 255 and the rest.
        1 => {
            for _ in 0..count - 1 {
                let mut size = 0usize;
                loop {
                    let b = *body.get(at)?;
                    at += 1;
                    size += usize::from(b);
                    if b != 255 {
                        break;
                    }
                }
                sizes.push(size);
            }
        }
        // EBML: the first size, then signed differences.
        3 => {
            let (first, k) = vint(body.get(at..)?, false)?;
            at += k;
            let mut size = i64::try_from(first).ok()?;
            sizes.push(usize::try_from(size).ok()?);
            for _ in 1..count - 1 {
                let (raw, k) = vint(body.get(at..)?, false)?;
                at += k;
                let bias = (1i64 << (7 * k - 1)) - 1;
                size += i64::try_from(raw).ok()? - bias;
                sizes.push(usize::try_from(size).ok()?);
            }
        }
        // Fixed: all the same size.
        _ => {
            let each = body.len().checked_sub(1)? / count;
            sizes = vec![each; count - 1];
        }
    }
    let used: usize = sizes.iter().sum();
    sizes.push(body.len().checked_sub(at + used)?);
    let mut out = Vec::with_capacity(count);
    for s in sizes {
        out.push(body.get(at..at + s)?);
        at += s;
    }
    Some((track, rel, out))
}

/// Walks the file's clusters and keeps the blocks of the tracks in
/// `wanted`. `stop` is asked between clusters; `progress` is told the
/// bytes walked.
fn scan(
    path: &Path,
    wanted: &[u64],
    stop: &dyn Fn() -> bool,
    progress: &dyn Fn(u64, u64),
) -> Result<HashMap<u64, Vec<Event>>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    let mut f = BufReader::with_capacity(1 << 20, file);
    let scale = matroska_element(&mut f, INFO)
        .and_then(|info| {
            ebml(&info)
                .find(|(id, _)| *id == 0x2a_d7b1)
                .map(|(_, d)| ebml_uint(d))
        })
        .filter(|s| *s > 0)
        .unwrap_or(1_000_000) as i64;
    f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut out: HashMap<u64, Vec<Event>> = wanted.iter().map(|n| (*n, Vec::new())).collect();
    let mut kept: HashMap<u64, usize> = HashMap::new();
    let mut pos = 0u64;
    // The EBML header, then the segment's.
    let (id, len, h) = header(&mut f).ok_or("subMkv")?;
    if id != 0x1a45_dfa3 {
        return Err("subMkv".into());
    }
    pos += h + len.ok_or("subMkv")?;
    f.seek(SeekFrom::Start(pos)).map_err(|e| e.to_string())?;
    let (id, len, h) = header(&mut f).ok_or("subMkv")?;
    if id != SEGMENT {
        return Err("subMkv".into());
    }
    pos += h;
    let end = len.map_or(size, |l| (pos + l).min(size));
    let mut told = 0u64;
    // A header read ahead (an element after a cluster of unknown size).
    let mut pending: Option<(u64, Option<u64>, u64)> = None;
    while pos < end {
        if stop() {
            return Err("stopped".into());
        }
        if pos - told >= 64 << 20 {
            progress(pos, size);
            told = pos;
        }
        let Some((id, len, h)) = pending.take().or_else(|| header(&mut f)) else {
            break;
        };
        let start = pos + h;
        if id != CLUSTER {
            let Some(len) = len else { break };
            pos = start + len;
            if f.seek_relative(len as i64).is_err() {
                break;
            }
            continue;
        }
        let cluster_end = len.map(|l| (start + l).min(end));
        pos = start;
        let mut cluster_ts = 0i64;
        loop {
            if cluster_end.is_some_and(|e| pos >= e) || pos >= end {
                break;
            }
            let Some((cid, clen, ch)) = header(&mut f) else {
                pos = end;
                break;
            };
            // A cluster of unknown size ends at the next element that is
            // not one of its own.
            if cluster_end.is_none() && !matches!(cid, 0xe7 | 0xa7 | 0xab | 0xa3 | 0xa0 | 0xaf | 0xbf | 0xec)
            {
                pending = Some((cid, clen, ch));
                break;
            }
            let Some(clen) = clen else {
                pos = end;
                break;
            };
            let body_at = pos + ch;
            pos = body_at + clen;
            match cid {
                0xe7 => {
                    let mut b = vec![0u8; clen.min(8) as usize];
                    if f.read_exact(&mut b).is_err() {
                        pos = end;
                        break;
                    }
                    cluster_ts = ebml_uint(&b) as i64;
                    if clen > 8 {
                        let _ = f.seek_relative((clen - 8) as i64);
                    }
                }
                0xa3 | 0xa0 => {
                    let simple = cid == 0xa3;
                    // The track number comes first (inside the Block of a
                    // BlockGroup, after its header).
                    let mut peek = [0u8; 16];
                    let n = clen.min(16) as usize;
                    if f.read_exact(&mut peek[..n]).is_err() {
                        pos = end;
                        break;
                    }
                    let track = if simple {
                        vint(&peek[..n], false).map(|t| t.0)
                    } else {
                        group_track(&peek[..n])
                    };
                    let want = track.is_some_and(|t| out.contains_key(&t));
                    if !want || clen > MAX_BLOCK {
                        if f.seek_relative(clen as i64 - n as i64).is_err() {
                            pos = end;
                            break;
                        }
                        continue;
                    }
                    let mut data = peek[..n].to_vec();
                    data.resize(clen as usize, 0);
                    if f.read_exact(&mut data[n..]).is_err() {
                        pos = end;
                        break;
                    }
                    let (block, duration) = if simple {
                        (Some(data.as_slice()), None)
                    } else {
                        let mut b = None;
                        let mut d = None;
                        for (id, v) in ebml(&data) {
                            match id {
                                0xa1 => b = Some(v),
                                0x9b => d = Some(ebml_uint(v) as i64),
                                _ => {}
                            }
                        }
                        (b, d)
                    };
                    let Some((t, rel, parts)) = block.and_then(frames) else {
                        continue;
                    };
                    let Some(list) = out.get_mut(&t) else {
                        continue;
                    };
                    let used = kept.entry(t).or_default();
                    for p in parts {
                        if *used > MAX_TRACK {
                            break;
                        }
                        *used += p.len();
                        list.push(Event {
                            start: (cluster_ts + i64::from(rel)) * scale,
                            duration: duration.map(|d| d * scale),
                            data: p.to_vec(),
                        });
                    }
                }
                _ => {
                    if f.seek_relative(clen as i64).is_err() {
                        pos = end;
                        break;
                    }
                }
            }
        }
        if cluster_end.is_some_and(|e| pos != e) {
            pos = cluster_end.unwrap();
            if f.seek(SeekFrom::Start(pos)).is_err() {
                break;
            }
        }
    }
    progress(size, size);
    Ok(out)
}

/// The track number of a BlockGroup's Block, from the group's first bytes.
fn group_track(head: &[u8]) -> Option<u64> {
    let (id, a) = vint(head, true)?;
    if id != 0xa1 {
        return None;
    }
    let (_, b) = vint(head.get(a..)?, false)?;
    vint(head.get(a + b..)?, false).map(|t| t.0)
}

/// A track's lines as the text of a subtitle file in its own format.
fn file_text(track: &Track, events: &[Event]) -> String {
    let mut ev: Vec<(i64, i64, String)> = Vec::with_capacity(events.len());
    let decoded: Vec<(i64, Option<i64>, String)> = events
        .iter()
        .map(|e| {
            let data = decode_block(&track.coding, &e.data).unwrap_or_else(|| e.data.clone());
            (e.start, e.duration, String::from_utf8_lossy(&data).replace("\r\n", "\n"))
        })
        .collect();
    let mut sorted = decoded;
    sorted.sort_by_key(|e| e.0);
    for (i, (start, dur, t)) in sorted.iter().enumerate() {
        let next = sorted.get(i + 1).map(|n| n.0).filter(|n| n > start);
        let fallback = if track.default_ns > 0 {
            track.default_ns as i64
        } else {
            next.map_or(4_000_000_000, |n| (n - start).min(5_000_000_000))
        };
        let end = start + dur.filter(|d| *d > 0).unwrap_or(fallback);
        ev.push((start / 1_000_000, end / 1_000_000, t.clone()));
    }
    let ms = |t: i64, sep: char| {
        let t = t.max(0);
        format!(
            "{:02}:{:02}:{:02}{sep}{:03}",
            t / 3_600_000,
            t / 60_000 % 60,
            t / 1000 % 60,
            t % 1000
        )
    };
    match track.codec {
        Codec::Srt | Codec::Other => {
            let mut s = String::new();
            let mut n = 0;
            for (a, b, t) in &ev {
                let t = t.trim_matches('\n');
                if t.trim().is_empty() {
                    continue;
                }
                n += 1;
                s.push_str(&format!("{n}\n{} --> {}\n{t}\n\n", ms(*a, ','), ms(*b, ',')));
            }
            s
        }
        Codec::Vtt => {
            let head = String::from_utf8_lossy(&track.private).replace("\r\n", "\n");
            let head = head.trim();
            let mut s = if head.starts_with("WEBVTT") {
                head.to_string()
            } else {
                "WEBVTT".to_string()
            };
            s.push_str("\n\n");
            for (a, b, t) in &ev {
                let t = t.trim_matches('\n');
                if t.trim().is_empty() {
                    continue;
                }
                s.push_str(&format!("{} --> {}\n{t}\n\n", ms(*a, '.'), ms(*b, '.')));
            }
            s
        }
        Codec::Ass => {
            let cs = |t: i64| {
                let c = (t.max(0) + 5) / 10;
                format!(
                    "{}:{:02}:{:02}.{:02}",
                    c / 360_000,
                    c / 6000 % 60,
                    c / 100 % 60,
                    c % 100
                )
            };
            let mut head = String::from_utf8_lossy(&track.private)
                .replace("\r\n", "\n")
                .trim_end()
                .to_string();
            if !head.to_ascii_lowercase().contains("[events]") {
                head.push_str(
                    "\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text",
                );
            }
            // Each block: ReadOrder, Layer, Style, Name, margins, Effect,
            // Text; written back in the order they were read in.
            let mut lines: Vec<(i64, i64, String)> = ev
                .iter()
                .filter_map(|(a, b, t)| {
                    let mut p = t.splitn(3, ',');
                    let order: i64 = p.next()?.trim().parse().ok()?;
                    let layer = p.next()?.trim();
                    let rest = p.next()?.trim_end_matches('\n');
                    Some((order, *a, format!("Dialogue: {layer},{},{},{rest}", cs(*a), cs(*b))))
                })
                .collect();
            lines.sort_by_key(|l| (l.0, l.1));
            let mut s = head;
            s.push('\n');
            for (_, _, l) in lines {
                s.push_str(&l);
                s.push('\n');
            }
            s
        }
    }
}

/// The language tag a file taken out of a track gets: the usual one
/// (`pt-BR`, `en`), else the track's own code (`eu`, `fil`); none for
/// "undetermined".
fn lang_tag(lang: &str) -> Option<String> {
    let lang = lang.trim();
    if ["und", "zxx", "mis", "mul", ""].contains(&lang.to_ascii_lowercase().as_str()) {
        return None;
    }
    if let Some(l) = crate::subtitles::langs(lang).first() {
        return Some(l.tag.clone());
    }
    // As written, if it is a code (letters, digits and dashes).
    (lang.len() <= 16 && lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .then(|| lang.to_string())
}

/// The files the text tracks of `video` would be written to: those in
/// `langs` (all when empty), named after the video with their language,
/// `.forced` and `.sdh`; those taken already left out.
pub fn targets(video: &Path, tracks: &[Track], langs: &[Lang], to_srt: bool) -> Vec<(u64, PathBuf)> {
    let stem = video
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut out: Vec<(u64, PathBuf)> = Vec::new();
    for t in tracks.iter().filter(|t| t.readable()) {
        if !langs.is_empty() && !langs.iter().any(|l| crate::subtitles::same_lang(&t.lang, l)) {
            continue;
        }
        let ext = if to_srt { "srt" } else { t.codec.ext() };
        let mut base = stem.clone();
        if let Some(tag) = lang_tag(&t.lang) {
            base.push('.');
            base.push_str(&tag);
        }
        if t.forced {
            base.push_str(".forced");
        }
        if t.hearing {
            base.push_str(".sdh");
        }
        let mut name = video.with_file_name(format!("{base}.{ext}"));
        let mut n = 2;
        while out.iter().any(|(_, p)| *p == name) {
            name = video.with_file_name(format!("{base}.{n}.{ext}"));
            n += 1;
        }
        if std::fs::symlink_metadata(&name).is_err() {
            out.push((t.number, name));
        }
    }
    out
}

/// What [`extract`] wrote.
#[derive(Debug, Default)]
pub struct Extracted {
    pub created: Vec<PathBuf>,
    /// File name and reason.
    pub errors: Vec<(String, String)>,
}

/// Takes the text subtitles of `video` out into files beside it (see
/// [`targets`]), cleaned as `tools` say (times are not moved: the tracks
/// are in time with their video). A name taken is never replaced.
pub fn extract(
    video: &Path,
    tools: &Tools,
    langs: &[Lang],
    stop: &dyn Fn() -> bool,
    progress: &dyn Fn(u64, u64),
) -> Result<Extracted, String> {
    let all = tracks(video);
    let targets = targets(video, &all, langs, tools.to_srt);
    let mut out = Extracted::default();
    if targets.is_empty() {
        return Ok(out);
    }
    let numbers: Vec<u64> = targets.iter().map(|t| t.0).collect();
    let events = scan(video, &numbers, stop, progress)?;
    for (number, to) in targets {
        let Some(track) = all.iter().find(|t| t.number == number) else {
            continue;
        };
        let name = to
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let list = events.get(&number).map(Vec::as_slice).unwrap_or(&[]);
        if list.is_empty() {
            out.errors.push((name, "subTrackEmpty".into()));
            continue;
        }
        let raw = file_text(track, list);
        let text = crate::subtext::clean_text(&raw, tools).unwrap_or(raw);
        // SubRip is written with Windows line ends, as players expect.
        let text = if to.extension().is_some_and(|e| e == "srt") && !text.contains("\r\n") {
            text.replace('\n', "\r\n")
        } else {
            text
        };
        match write_new(&to, text.as_bytes()) {
            Ok(()) => out.created.push(to),
            Err(e) => out.errors.push((name, e)),
        }
    }
    Ok(out)
}

/// Writes a file that must not be there yet.
fn write_new(to: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write as _;
    let mut f = File::options()
        .write(true)
        .create_new(true)
        .open(to)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                "err.targetExists".to_string()
            } else {
                format!("err.io: {e}")
            }
        })?;
    let r = f.write_all(bytes).and_then(|_| f.sync_all());
    if let Err(e) = r {
        drop(f);
        let _ = std::fs::remove_file(to);
        return Err(format!("err.io: {e}"));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// An EBML element: id (with its marker) and content.
    pub fn el(id: u64, body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        let id_bytes = id.to_be_bytes();
        let skip = id_bytes.iter().position(|b| *b != 0).unwrap_or(7);
        v.extend_from_slice(&id_bytes[skip..]);
        // Sizes as 8 bytes: 0x01 then 7 bytes.
        let n = body.len() as u64;
        v.push(0x01);
        v.extend_from_slice(&n.to_be_bytes()[1..]);
        v.extend_from_slice(body);
        v
    }

    fn uint(id: u64, n: u64) -> Vec<u8> {
        el(id, &n.to_be_bytes())
    }

    fn simple(track: u8, rel: i16, data: &[u8]) -> Vec<u8> {
        let mut b = vec![0x80 | track];
        b.extend_from_slice(&rel.to_be_bytes());
        b.push(0x80);
        b.extend_from_slice(data);
        el(0xa3, &b)
    }

    fn group(track: u8, rel: i16, dur: u64, data: &[u8]) -> Vec<u8> {
        let mut b = vec![0x80 | track];
        b.extend_from_slice(&rel.to_be_bytes());
        b.push(0);
        b.extend_from_slice(data);
        let mut g = el(0xa1, &b);
        g.extend(uint(0x9b, dur));
        el(0xa0, &g)
    }

    /// A small Matroska file: a video track (1), a SubRip track in
    /// Portuguese (2), a SubStation track (3, zlib-compressed) and a
    /// picture track (4).
    pub fn sample() -> Vec<u8> {
        use std::io::Write as _;
        let mut tracks = Vec::new();
        let mut v = uint(0xd7, 1);
        v.extend(uint(0x83, 1));
        v.extend(el(0x86, b"V_MPEG4/ISO/AVC"));
        tracks.extend(el(0xae, &v));
        let mut s = uint(0xd7, 2);
        s.extend(uint(0x83, 0x11));
        s.extend(el(0x86, b"S_TEXT/UTF8"));
        s.extend(el(0x22_b59c, b"por"));
        tracks.extend(el(0xae, &s));
        let mut a = uint(0xd7, 3);
        a.extend(uint(0x83, 0x11));
        a.extend(el(0x86, b"S_TEXT/ASS"));
        a.extend(el(0x22_b59c, b"eng"));
        a.extend(uint(0x55aa, 1));
        a.extend(el(
            0x63a2,
            b"[Script Info]\nScriptType: v4.00+\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
        ));
        let mut comp = uint(0x4254, 0);
        comp = el(0x5034, &comp);
        let mut enc = uint(0x5032, 1);
        enc.extend(comp);
        a.extend(el(0x6d80, &el(0x6240, &enc)));
        tracks.extend(el(0xae, &a));
        let mut p = uint(0xd7, 4);
        p.extend(uint(0x83, 0x11));
        p.extend(el(0x86, b"S_HDMV/PGS"));
        tracks.extend(el(0xae, &p));
        let zl = |t: &[u8]| {
            let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(t).unwrap();
            e.finish().unwrap()
        };
        let mut c1 = uint(0xe7, 1000);
        c1.extend(simple(1, 0, &[0u8; 300]));
        c1.extend(group(2, 0, 1500, "Olá, mundo".as_bytes()));
        c1.extend(group(3, 500, 1000, &zl(b"0,0,Default,,0,0,0,,{\\i1}Hi{\\i0}")));
        c1.extend(simple(1, 40, &[0u8; 300]));
        let mut c2 = uint(0xe7, 5000);
        c2.extend(group(2, 0, 2000, b"<i>[MUSIC]</i>\r\nSecond"));
        c2.extend(simple(4, 0, b"PGpicture"));
        let mut seg = el(INFO, &uint(0x2a_d7b1, 1_000_000));
        seg.extend(el(TRACKS, &tracks));
        seg.extend(el(CLUSTER, &c1));
        seg.extend(el(CLUSTER, &c2));
        let mut file = el(0x1a45_dfa3, &el(0x4282, b"matroska"));
        file.extend(el(SEGMENT, &seg));
        file
    }

    #[test]
    fn tracks_listed() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("Movie.mkv");
        std::fs::write(&p, sample()).unwrap();
        let t = tracks(&p);
        assert_eq!(t.len(), 3);
        assert_eq!((t[0].codec, t[0].lang.as_str()), (Codec::Srt, "por"));
        assert_eq!(t[1].coding, Coding::Zlib);
        assert!(t[1].forced);
        assert!(!t[2].readable());
        let names: Vec<String> = targets(&p, &t, &[], false)
            .iter()
            .map(|(_, p)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["Movie.pt.srt", "Movie.en.forced.ass"]);
        let only = crate::subtitles::langs("pt-BR");
        assert_eq!(targets(&p, &t, &only, true).len(), 1);
    }

    #[test]
    fn subtitles_taken_out() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("Movie.mkv");
        std::fs::write(&p, sample()).unwrap();
        let tools = Tools {
            hearing: true,
            ..Default::default()
        };
        let got = extract(&p, &tools, &[], &|| false, &|_, _| {}).unwrap();
        assert_eq!(got.created.len(), 2, "{:?}", got.errors);
        let srt = std::fs::read_to_string(d.path().join("Movie.pt.srt")).unwrap();
        assert_eq!(
            srt,
            "1\r\n00:00:01,000 --> 00:00:02,500\r\nOlá, mundo\r\n\r\n2\r\n00:00:05,000 --> 00:00:07,000\r\nSecond\r\n\r\n"
        );
        let ass = std::fs::read_to_string(d.path().join("Movie.en.forced.ass")).unwrap();
        assert!(ass.contains("Dialogue: 0,0:00:01.50,0:00:02.50,Default,,0,0,0,,{\\i1}Hi{\\i0}"));
        // Taken already: not again.
        let again = extract(&p, &tools, &[], &|| false, &|_, _| {}).unwrap();
        assert!(again.created.is_empty());
        // As SubRip.
        std::fs::remove_file(d.path().join("Movie.en.forced.ass")).unwrap();
        let srt_tools = Tools {
            to_srt: true,
            ..Default::default()
        };
        extract(&p, &srt_tools, &crate::subtitles::langs("en"), &|| false, &|_, _| {}).unwrap();
        let s = std::fs::read_to_string(d.path().join("Movie.en.forced.srt")).unwrap();
        assert!(s.contains("00:00:01,500 --> 00:00:02,500\r\n<i>Hi</i>"));
    }

    #[test]
    fn laced_frames() {
        // Xiph lacing: 3 frames of 2, 3 and 1 bytes.
        let data = [0x81, 0, 0, 0x02, 2, 2, 3, b'a', b'b', b'c', b'd', b'e', b'f'];
        let (t, rel, parts) = frames(&data).unwrap();
        assert_eq!((t, rel), (1, 0));
        assert_eq!(parts, [&b"ab"[..], b"cde", b"f"]);
        // EBML lacing: 2 + (3) + rest.
        let data = [0x81, 0, 0, 0x06, 2, 0x82, 0xbf, b'a', b'b', b'c', b'd', b'e', b'f'];
        let (_, _, parts) = frames(&data).unwrap();
        assert_eq!(parts, [&b"ab"[..], b"cd", b"ef"]);
    }
}
