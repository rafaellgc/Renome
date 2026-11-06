//! What a video file holds: its picture's size and codec, read from the
//! container's headers (Matroska/WebM, MP4/QuickTime, AVI, MPEG, FLV); no
//! video is decoded and the media data is skipped, not read. Each reader
//! looks only for what it needs and stops there, so a file cut short or
//! with odd parts elsewhere (an index past its end) is still read.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use serde::Serialize;

/// The main video track's frame size, in pixels, and codec.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    /// `H.264`, `HEVC`, `AV1`…; empty when unknown.
    pub codec: String,
}

impl VideoInfo {
    /// The usual name for the picture's size: `2160p`, `1080p`, `720p`,
    /// `576p`, `480p`. A film cropped to a wider shape (`1920×800`) keeps
    /// the name of its width; other sizes give their height (`360p`).
    pub fn resolution(&self) -> String {
        let (w, h) = (self.width, self.height);
        if w == 0 || h == 0 {
            return String::new();
        }
        let name = if w >= 3200 || h >= 1800 {
            "2160p"
        } else if w >= 2200 || h >= 1300 {
            "1440p"
        } else if w >= 1700 || h >= 1000 {
            "1080p"
        } else if w >= 1100 || h >= 700 {
            "720p"
        } else if h >= 540 {
            "576p"
        } else if h >= 460 {
            "480p"
        } else {
            return format!("{h}p");
        };
        name.into()
    }
}

/// The template keys that read the video (resolution and codec).
const KEYS: [&str; 10] = [
    "resolution",
    "resolução",
    "resolucao",
    "resolución",
    "video codec",
    "codec de vídeo",
    "codec de video",
    "códec de vídeo",
    "códec de video",
    "codec",
];

/// Whether a template has a key that needs the video read.
pub fn wanted(template: &str) -> bool {
    KEYS.iter().any(|k| crate::rules::uses_key(template, k))
}

/// The video's values for templates: resolution and codec, empty when
/// the video is unknown.
#[derive(Default)]
pub struct Fields {
    resolution: String,
    codec: String,
}

impl Fields {
    pub fn of(video: Option<&VideoInfo>) -> Self {
        video
            .map(|v| Fields {
                resolution: v.resolution(),
                codec: v.codec.clone(),
            })
            .unwrap_or_default()
    }

    /// The value of a template key about the video; `None` for other keys.
    pub fn get(&self, key: &str) -> Option<&str> {
        match KEYS.iter().position(|k| *k == key)? {
            0..=3 => Some(&self.resolution),
            _ => Some(&self.codec),
        }
    }
}

/// Reads the main video track of a video file; `None` when the format is
/// not one of those read or the file has no video track. The format is
/// told by the file's first bytes, not its extension (an `.avi` is
/// sometimes an MP4).
pub fn read(path: &Path) -> Option<VideoInfo> {
    let mut f = BufReader::new(File::open(path).ok()?);
    let mut head = [0u8; 12];
    f.read_exact(&mut head).ok()?;
    f.seek(SeekFrom::Start(0)).ok()?;
    let mp4_box = [
        b"ftyp", b"moov", b"mdat", b"free", b"wide", b"skip", b"pnot",
    ];
    match head {
        [0x1a, 0x45, 0xdf, 0xa3, ..] => matroska_video(&mut f),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'A', b'V', b'I', b' '] => avi_video(&mut f),
        // A video CD's MPEG wrapped in RIFF.
        [b'R', b'I', b'F', b'F', _, _, _, _, b'C', b'D', b'X', b'A'] => mpeg_video(&mut f),
        [0, 0, 1, 0xba, ..] | [0, 0, 1, 0xb3, ..] => mpeg_video(&mut f),
        [b'F', b'L', b'V', 1, ..] => flv_video(&mut f),
        _ if mp4_box.iter().any(|k| head[4..8] == k[..]) => mp4_video(&mut f),
        _ => None,
    }
    .filter(|v| v.width > 0 && v.height > 0)
}

/// An EBML number: the element ids keep their length marker, sizes lose
/// it (all ones: unknown). Returns the value and its length.
fn vint(data: &[u8], keep_marker: bool) -> Option<(u64, usize)> {
    let first = *data.first()?;
    let len = first.leading_zeros() as usize + 1;
    if len > 8 {
        return None;
    }
    let mut value = u64::from(if keep_marker {
        first
    } else {
        first & (0xffu16 >> len) as u8
    });
    for &b in data.get(1..len)? {
        value = (value << 8) | u64::from(b);
    }
    if !keep_marker && value == (1u64 << (7 * len)) - 1 {
        value = u64::MAX;
    }
    Some((value, len))
}

/// The EBML elements in `data`: id and content.
fn ebml(data: &[u8]) -> impl Iterator<Item = (u64, &[u8])> {
    let mut at = 0usize;
    std::iter::from_fn(move || {
        let (id, a) = vint(data.get(at..)?, true)?;
        let (size, b) = vint(data.get(at + a..)?, false)?;
        let start = at + a + b;
        let end = start
            .checked_add(usize::try_from(size).ok()?)?
            .min(data.len());
        at = end;
        Some((id, data.get(start..end)?))
    })
}

fn ebml_uint(data: &[u8]) -> u64 {
    data.iter().fold(0, |n, &b| (n << 8) | u64::from(b))
}

/// The languages of the subtitle tracks inside a Matroska video (`por`,
/// `pt-BR`; `eng` when a track does not say), forced ones (only the parts
/// in another language) left out. Other formats: none.
pub fn subtitle_tracks(path: &Path) -> Vec<String> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let mut f = BufReader::new(file);
    let mut head = [0u8; 4];
    if f.read_exact(&mut head).is_err() || head != [0x1a, 0x45, 0xdf, 0xa3] {
        return Vec::new();
    }
    let Some(tracks) = matroska_tracks(&mut f) else {
        return Vec::new();
    };
    ebml(&tracks)
        .filter(|(id, _)| *id == 0xae)
        .filter_map(|(_, entry)| {
            let (mut kind, mut forced) = (0, false);
            let (mut lang, mut bcp47) = (None, None);
            for (id, data) in ebml(entry) {
                let text = || {
                    Some(
                        std::str::from_utf8(data)
                            .ok()?
                            .trim_end_matches('\0')
                            .to_string(),
                    )
                };
                match id {
                    0x83 => kind = ebml_uint(data),
                    0x55aa => forced = ebml_uint(data) == 1,
                    0x22_b59c => lang = text(),
                    0x22_b59d => bcp47 = text(),
                    _ => {}
                }
            }
            (kind == 0x11 && !forced).then(|| bcp47.or(lang).unwrap_or_else(|| "eng".into()))
        })
        .collect()
}

/// Matroska and WebM: the segment's elements are walked by their headers
/// until its track list, which is returned; the clusters before it, if
/// any, are skipped.
fn matroska_tracks<R: Read + Seek>(f: &mut R) -> Option<Vec<u8>> {
    const SEGMENT: u64 = 0x1853_8067;
    const TRACKS: u64 = 0x1654_ae6b;
    let size = f.seek(SeekFrom::End(0)).ok()?;
    let mut pos = 0u64;
    let mut head = [0u8; 16];
    loop {
        if pos >= size {
            return None;
        }
        f.seek(SeekFrom::Start(pos)).ok()?;
        let n = f.read(&mut head).ok()?;
        let (id, a) = vint(&head[..n], true)?;
        let (len, b) = vint(head.get(a..n)?, false)?;
        let start = pos + (a + b) as u64;
        if id == SEGMENT {
            // Its children follow.
            pos = start;
        } else if id == TRACKS {
            let len = len.min(size - start).min(16 << 20);
            let mut body = vec![0u8; len as usize];
            f.seek(SeekFrom::Start(start)).ok()?;
            f.read_exact(&mut body).ok()?;
            return Some(body);
        } else if len == u64::MAX {
            return None;
        } else {
            pos = start.checked_add(len)?;
        }
    }
}

/// Matroska and WebM: the largest video track.
fn matroska_video<R: Read + Seek>(f: &mut R) -> Option<VideoInfo> {
    let tracks = matroska_tracks(f)?;
    ebml(&tracks)
        .filter(|(id, _)| *id == 0xae)
        .filter_map(|(_, entry)| {
            let (mut video, mut kind, mut codec) = (None, 0, "");
            for (id, data) in ebml(entry) {
                match id {
                    0x83 => kind = ebml_uint(data),
                    0x86 => {
                        codec =
                            matroska_codec(std::str::from_utf8(data).ok()?.trim_end_matches('\0'))
                    }
                    0xe0 => video = Some(data),
                    _ => {}
                }
            }
            if kind != 1 {
                return None;
            }
            let (mut width, mut height) = (0, 0);
            for (id, data) in ebml(video?) {
                match id {
                    0xb0 => width = ebml_uint(data),
                    0xba => height = ebml_uint(data),
                    _ => {}
                }
            }
            Some(VideoInfo {
                width: u32::try_from(width).ok()?,
                height: u32::try_from(height).ok()?,
                codec: codec.into(),
            })
        })
        .max_by_key(|v| u64::from(v.width) * u64::from(v.height))
}

fn matroska_codec(id: &str) -> &'static str {
    match id {
        "V_MPEG4/ISO/AVC" => "H.264",
        "V_MPEGH/ISO/HEVC" => "HEVC",
        "V_AV1" => "AV1",
        "V_VP9" => "VP9",
        "V_VP8" => "VP8",
        "V_MPEG2" | "V_MPEG1" => "MPEG-2",
        id if id.starts_with("V_MPEG4/ISO/") || id.starts_with("V_MPEG4/MS/") => "MPEG-4",
        _ => "",
    }
}

/// The codec of a four-character code, as MP4 sample descriptions and AVI
/// stream headers give it.
fn fourcc_codec(code: &[u8]) -> &'static str {
    match code.to_ascii_lowercase().as_slice() {
        b"avc1" | b"avc3" | b"h264" | b"x264" => "H.264",
        b"hvc1" | b"hev1" | b"hevc" | b"h265" | b"x265" => "HEVC",
        b"av01" => "AV1",
        b"vp09" => "VP9",
        b"vp08" => "VP8",
        b"mp4v" | b"xvid" | b"divx" | b"dx50" | b"fmp4" | b"div3" | b"div4" | b"mp43" | b"mp42"
        | b"mp41" => "MPEG-4",
        b"mjpg" | b"jpeg" | b"mjpa" | b"mjpb" => "MJPEG",
        b"apch" | b"apcn" | b"apcs" | b"apco" | b"ap4h" | b"ap4x" => "ProRes",
        _ => "",
    }
}

/// The boxes inside `data` (an MP4 box's content): kind and content.
fn children(data: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    let mut at = 0usize;
    std::iter::from_fn(move || {
        let head = data.get(at..at + 8)?;
        let mut len = u32::from_be_bytes(head[..4].try_into().ok()?) as usize;
        let mut start = 8;
        if len == 1 {
            len = usize::try_from(u64::from_be_bytes(
                data.get(at + 8..at + 16)?.try_into().ok()?,
            ))
            .ok()?;
            start = 16;
        } else if len == 0 {
            len = data.len() - at;
        }
        let body = data.get(at + start..at.checked_add(len)?)?;
        let kind = &head[4..8];
        at += len;
        Some((kind, body))
    })
}

fn child<'a>(data: &'a [u8], kind: &[u8]) -> Option<&'a [u8]> {
    children(data).find(|(k, _)| *k == kind).map(|(_, b)| b)
}

/// MP4 and QuickTime: the `moov` box is found by the top-level headers,
/// the media data skipped (a film's index is a few megabytes, its data
/// gigabytes); of its tracks, the video ones (`hdlr` `vide`) give their
/// size from the sample description (`stsd`), else the track header
/// (`tkhd`), and the largest is taken.
fn mp4_video<R: Read + Seek>(f: &mut R) -> Option<VideoInfo> {
    mp4_info(&moov(f)?)
}

fn mp4_info(moov: &[u8]) -> Option<VideoInfo> {
    children(moov)
        .filter(|(k, _)| *k == b"trak")
        .filter_map(|(_, trak)| {
            let mdia = child(trak, b"mdia")?;
            // hdlr: version and flags, pre-defined, then the handler type.
            if child(mdia, b"hdlr")?.get(8..12)? != b"vide" {
                return None;
            }
            let stsd = child(child(child(mdia, b"minf")?, b"stbl")?, b"stsd")?;
            // stsd: version and flags, entry count, then the first entry
            // (size, format, 6 reserved, data reference, 16 bytes of
            // version and the like, width and height).
            let entry = stsd.get(8..)?;
            let codec = fourcc_codec(entry.get(4..8)?);
            let be16 =
                |at: usize| Some(u16::from_be_bytes(entry.get(at..at + 2)?.try_into().ok()?));
            let (mut width, mut height) = (u32::from(be16(32)?), u32::from(be16(34)?));
            if width == 0 || height == 0 {
                // tkhd: the size in 16.16 fixed point at its end.
                let tkhd = child(trak, b"tkhd")?;
                let n = tkhd.len();
                let be32 =
                    |at: usize| Some(u32::from_be_bytes(tkhd.get(at..at + 4)?.try_into().ok()?));
                width = be32(n.checked_sub(8)?)? >> 16;
                height = be32(n - 4)? >> 16;
            }
            Some(VideoInfo {
                width,
                height,
                codec: codec.into(),
            })
        })
        .max_by_key(|v| u64::from(v.width) * u64::from(v.height))
}

/// When a phone's or camera's MP4 or QuickTime video was recorded, in the
/// local time where it was (Apple's `creationdate`, which says it), else
/// in this computer's time zone (the movie header's time, in UTC as the
/// format wants); and the device (Apple's `make` and `model`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Recorded {
    pub at: Option<chrono::NaiveDateTime>,
    pub make: String,
    pub model: String,
}

/// See [`Recorded`]; `None` for other formats.
pub fn recorded(path: &Path) -> Option<Recorded> {
    recorded_in(&mp4_moov(path)?)
}

/// When it was recorded and its picture, from one read of the index
/// (it is a megabyte or so on a phone's video); `None` for other formats.
pub fn clip(path: &Path) -> Option<(Recorded, Option<VideoInfo>)> {
    let moov = mp4_moov(path)?;
    let info = mp4_info(&moov).filter(|v| v.width > 0 && v.height > 0);
    Some((recorded_in(&moov).unwrap_or_default(), info))
}

/// An MP4's or QuickTime's index; `None` for other formats.
fn mp4_moov(path: &Path) -> Option<Vec<u8>> {
    let mut f = BufReader::new(File::open(path).ok()?);
    let mut head = [0u8; 12];
    f.read_exact(&mut head).ok()?;
    f.seek(SeekFrom::Start(0)).ok()?;
    if !matches!(
        &head[4..8],
        b"ftyp" | b"moov" | b"mdat" | b"free" | b"wide" | b"skip"
    ) {
        return None;
    }
    moov(&mut f)
}

fn recorded_in(moov: &[u8]) -> Option<Recorded> {
    let mut out = Recorded::default();
    // Apple's keys (`com.apple.quicktime.…`), in moov/meta or moov/udta/meta.
    let metas = child(moov, b"meta")
        .into_iter()
        .chain(child(moov, b"udta").and_then(|u| child(u, b"meta")));
    for meta in metas {
        // QuickTime's meta holds its boxes; MP4's starts with version and flags.
        let meta = if child(meta, b"hdlr").is_some() {
            meta
        } else {
            meta.get(4..).unwrap_or_default()
        };
        let (Some(keys), Some(ilst)) = (child(meta, b"keys"), child(meta, b"ilst")) else {
            continue;
        };
        // keys: version and flags, count, then (size, namespace, name).
        let mut names = Vec::new();
        let mut at = 8usize;
        while let Some(head) = keys.get(at..at + 8) {
            let len = u32::from_be_bytes(head[..4].try_into().ok()?) as usize;
            if len < 8 {
                break;
            }
            names.push(String::from_utf8_lossy(keys.get(at + 8..at + len)?).into_owned());
            at += len;
        }
        // ilst: one box per key, its type the key's index (from 1), holding
        // a `data` box (type, locale, then the value).
        for (kind, item) in children(ilst) {
            let index = u32::from_be_bytes(kind.try_into().ok()?) as usize;
            let Some(name) = index.checked_sub(1).and_then(|i| names.get(i)) else {
                continue;
            };
            let Some(value) = child(item, b"data").and_then(|d| d.get(8..)) else {
                continue;
            };
            let value = String::from_utf8_lossy(value).trim().to_string();
            match name.as_str() {
                "com.apple.quicktime.creationdate" => {
                    // `2019-05-12T14:30:22-0300`: the time where it was.
                    out.at = out.at.or_else(|| {
                        chrono::NaiveDateTime::parse_from_str(value.get(..19)?, "%Y-%m-%dT%H:%M:%S")
                            .ok()
                    });
                }
                "com.apple.quicktime.make" => out.make = value,
                "com.apple.quicktime.model" => out.model = value,
                _ => {}
            }
        }
    }
    if out.at.is_none()
        && let Some(mvhd) = child(moov, b"mvhd")
    {
        // Seconds since 1904 in UTC: 4 bytes (version 0) or 8 (version 1)
        // after version and flags. Zero: not set.
        let secs = match mvhd.first()? {
            1 => u64::from_be_bytes(mvhd.get(4..12)?.try_into().ok()?),
            _ => u64::from(u32::from_be_bytes(mvhd.get(4..8)?.try_into().ok()?)),
        };
        const FROM_1904: i64 = 2_082_844_800;
        if secs > 0 {
            let unix = i64::try_from(secs).ok()? - FROM_1904;
            use chrono::TimeZone;
            out.at = chrono::Local
                .timestamp_opt(unix, 0)
                .single()
                .map(|t| t.naive_local())
                // Before 1990: a camera without its clock set.
                .filter(|t| chrono::Datelike::year(t) >= 1990);
        }
    }
    Some(out)
}

/// The `moov` box's content, found by the top-level headers: the media
/// data is skipped (a film's index is a few megabytes, its data gigabytes).
fn moov<R: Read + Seek>(f: &mut R) -> Option<Vec<u8>> {
    // A moov bigger than this is no sane file's.
    const MOOV_MAX: u64 = 256 << 20;
    let size = f.seek(SeekFrom::End(0)).ok()?;
    let mut pos = 0u64;
    loop {
        if pos + 8 > size {
            return None;
        }
        f.seek(SeekFrom::Start(pos)).ok()?;
        let mut head = [0u8; 16];
        f.read_exact(&mut head[..8]).ok()?;
        let (mut len, mut start) = (u64::from(u32::from_be_bytes(head[..4].try_into().ok()?)), 8);
        if len == 1 {
            f.read_exact(&mut head[8..]).ok()?;
            len = u64::from_be_bytes(head[8..].try_into().ok()?);
            start = 16;
        } else if len == 0 {
            len = size - pos;
        }
        if len < start {
            return None;
        }
        if &head[4..8] == b"moov" {
            let len = len.min(size - pos);
            if len > MOOV_MAX {
                return None;
            }
            let mut body = vec![0u8; (len - start) as usize];
            f.read_exact(&mut body).ok()?;
            return Some(body);
        }
        pos += len;
    }
}

/// AVI: the first video stream's format (`strf`, a bitmap header) gives
/// its size and codec, the main header (`avih`) the size when it lacks it.
fn avi_video<R: Read>(f: &mut R) -> Option<VideoInfo> {
    let mut head = vec![0u8; 64 * 1024];
    let mut n = 0;
    while n < head.len() {
        match f.read(&mut head[n..]).ok()? {
            0 => break,
            k => n += k,
        }
    }
    let head = &head[..n];
    let le32 = |at: usize| Some(u32::from_le_bytes(head.get(at..at + 4)?.try_into().ok()?));
    let find = |tag: &[u8], from: usize| {
        head.get(from..)?
            .windows(4)
            .position(|w| w == tag)
            .map(|p| p + from)
    };
    // The video stream header (`strh` of type `vids`), and the format
    // after it.
    let mut at = 0;
    let (handler, strf) = loop {
        let strh = find(b"strh", at)?;
        if head.get(strh + 8..strh + 12)? == b"vids" {
            let handler = head.get(strh + 12..strh + 16)?;
            break (handler, find(b"strf", strh));
        }
        at = strh + 4;
    };
    // strf: size, then the bitmap header's size, width, height, planes and
    // bit count, and the compression.
    let (mut width, mut height, mut codec) = (0, 0, "");
    if let Some(strf) = strf {
        width = le32(strf + 12)?;
        height = (le32(strf + 16)? as i32).unsigned_abs();
        codec = fourcc_codec(head.get(strf + 24..strf + 28)?);
    }
    if width == 0 || height == 0 {
        let avih = find(b"avih", 0)? + 8;
        (width, height) = (le32(avih + 32)?, le32(avih + 36)?);
    }
    if codec.is_empty() {
        codec = fourcc_codec(handler);
    }
    Some(VideoInfo {
        width,
        height,
        codec: codec.into(),
    })
}

/// Flash video: the size from the metadata (`onMetaData`'s `width` and
/// `height`), the codec from the first video tag.
fn flv_video<R: Read>(f: &mut R) -> Option<VideoInfo> {
    let mut head = Vec::new();
    f.take(256 * 1024).read_to_end(&mut head).ok()?;
    let number = |key: &[u8]| {
        let mut pattern = (key.len() as u16).to_be_bytes().to_vec();
        pattern.extend_from_slice(key);
        pattern.push(0);
        let at = head.windows(pattern.len()).position(|w| w == pattern)? + pattern.len();
        let n = f64::from_be_bytes(head.get(at..at + 8)?.try_into().ok()?);
        (n.is_finite() && n > 0.0 && n < 100_000.0).then_some(n as u32)
    };
    let (width, height) = (number(b"width")?, number(b"height")?);
    // Tags after the header: previous tag size, type, data size, time,
    // stream id, data.
    let mut at = u32::from_be_bytes(head.get(5..9)?.try_into().ok()?) as usize + 4;
    let mut codec = "";
    while let Some(tag) = head.get(at..at + 12) {
        let len = u32::from_be_bytes([0, tag[1], tag[2], tag[3]]) as usize;
        if tag[0] == 9 {
            codec = match tag[11] & 0x0f {
                2 => "H.263",
                4 | 5 => "VP6",
                7 => "H.264",
                12 => "HEVC",
                _ => "",
            };
            break;
        }
        at += 11 + len + 4;
    }
    Some(VideoInfo {
        width,
        height,
        codec: codec.into(),
    })
}

/// MPEG program streams (`.mpg`, `.vob`): the first sequence header gives
/// the size; an extension after it marks MPEG-2.
fn mpeg_video<R: Read>(f: &mut R) -> Option<VideoInfo> {
    let mut head = Vec::new();
    f.take(4 << 20).read_to_end(&mut head).ok()?;
    let seq = head.windows(4).position(|w| w == [0, 0, 1, 0xb3])?;
    let b = head.get(seq + 4..seq + 7)?;
    let width = (u32::from(b[0]) << 4) | (u32::from(b[1]) >> 4);
    let height = ((u32::from(b[1]) & 0xf) << 8) | u32::from(b[2]);
    let mpeg2 = head
        .get(seq..(seq + 200).min(head.len()))?
        .windows(4)
        .any(|w| w == [0, 0, 1, 0xb5]);
    Some(VideoInfo {
        width,
        height,
        codec: if mpeg2 { "MPEG-2" } else { "MPEG-1" }.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }

    #[test]
    fn when_recorded() {
        let d = tempfile::tempdir().unwrap();
        // The movie header alone: UTC, shown in this computer's time zone.
        let secs: u32 = 1_557_671_422 + 2_082_844_800; // 2019-05-12 14:30:22 UTC
        let mut mvhd = vec![0u8; 4];
        mvhd.extend_from_slice(&secs.to_be_bytes());
        mvhd.extend_from_slice(&[0u8; 92]);
        let plain = [
            mp4_box(b"ftyp", b"isom\0\0\0\0"),
            mp4_box(b"moov", &mp4_box(b"mvhd", &mvhd)),
        ]
        .concat();
        let p = d.path().join("a.mp4");
        std::fs::write(&p, &plain).unwrap();
        use chrono::TimeZone;
        let utc = chrono::Local
            .timestamp_opt(1_557_671_422, 0)
            .unwrap()
            .naive_local();
        assert_eq!(recorded(&p).unwrap().at, Some(utc));
        // Apple's keys: the local time where it was, and the phone.
        let key = |name: &str| mp4_box(b"mdta", name.as_bytes());
        let mut keys = vec![0, 0, 0, 0, 0, 0, 0, 3];
        for k in [
            "com.apple.quicktime.make",
            "com.apple.quicktime.model",
            "com.apple.quicktime.creationdate",
        ] {
            keys.extend(key(k));
        }
        let item = |i: u32, v: &str| {
            let mut data = vec![0, 0, 0, 1, 0, 0, 0, 0];
            data.extend_from_slice(v.as_bytes());
            mp4_box(&i.to_be_bytes(), &mp4_box(b"data", &data))
        };
        let ilst = [
            item(1, "Apple"),
            item(2, "iPhone 12"),
            item(3, "2019-05-12T11:30:22-0300"),
        ]
        .concat();
        let meta = [
            mp4_box(b"hdlr", &[0u8; 24]),
            mp4_box(b"keys", &keys),
            mp4_box(b"ilst", &ilst),
        ]
        .concat();
        let apple = [
            mp4_box(b"ftyp", b"qt  \0\0\0\0"),
            mp4_box(
                b"moov",
                &[mp4_box(b"mvhd", &mvhd), mp4_box(b"meta", &meta)].concat(),
            ),
        ]
        .concat();
        let p = d.path().join("b.mov");
        std::fs::write(&p, &apple).unwrap();
        let r = recorded(&p).unwrap();
        assert_eq!(r.at.unwrap().to_string(), "2019-05-12 11:30:22");
        assert_eq!((r.make.as_str(), r.model.as_str()), ("Apple", "iPhone 12"));
    }

    #[test]
    fn resolutions() {
        let r = |width, height| {
            VideoInfo {
                width,
                height,
                codec: String::new(),
            }
            .resolution()
        };
        assert_eq!(r(3840, 2160), "2160p");
        assert_eq!(r(3840, 1600), "2160p");
        assert_eq!(r(1920, 1080), "1080p");
        assert_eq!(r(1920, 800), "1080p");
        assert_eq!(r(1440, 1080), "1080p");
        assert_eq!(r(1280, 720), "720p");
        assert_eq!(r(1280, 536), "720p");
        assert_eq!(r(720, 576), "576p");
        assert_eq!(r(720, 480), "480p");
        assert_eq!(r(640, 360), "360p");
        assert_eq!(r(0, 0), "");
    }

    #[test]
    fn reads_containers() {
        let dir = Path::new("tests/fixtures");
        for (name, codec) in [
            ("video.mkv", "H.264"),
            ("video.webm", "VP9"),
            ("video.mp4", "HEVC"),
            ("video-faststart.mp4", "H.264"),
            ("video.mov", "H.264"),
            ("video.avi", "MPEG-4"),
            ("video.mpg", "MPEG-2"),
            ("video.flv", "H.263"),
        ] {
            let v = read(&dir.join(name)).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(
                (v.width, v.height, v.codec.as_str()),
                (320, 136, codec),
                "{name}"
            );
            assert_eq!(v.resolution(), "136p");
        }
        // Not a video.
        assert_eq!(read(&dir.join("song.flac")), None);
        // Told by its content, not its extension.
        let d = tempfile::tempdir().unwrap();
        let odd = d.path().join("really an mp4.avi");
        std::fs::copy(dir.join("video.mp4"), &odd).unwrap();
        assert_eq!(read(&odd).map(|v| v.codec), Some("HEVC".into()));
        // Cut short (a download stopped halfway): its tracks are at the start.
        let mkv = std::fs::read(dir.join("video.mkv")).unwrap();
        let cut = d.path().join("cut.mkv");
        std::fs::write(&cut, &mkv[..mkv.len() / 2]).unwrap();
        assert_eq!(read(&cut).map(|v| v.height), Some(136));
    }

    #[test]
    fn subtitle_tracks_inside() {
        let dir = Path::new("tests/fixtures");
        // por, eng (forced: left out), spa.
        assert_eq!(subtitle_tracks(&dir.join("video-subs.mkv")), ["por", "spa"]);
        assert!(subtitle_tracks(&dir.join("video.mkv")).is_empty());
        assert!(subtitle_tracks(&dir.join("video.mp4")).is_empty());
    }

    /// The boxes of an MP4 file: (kind, bytes with the header).
    fn boxes(data: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
        let mut out = Vec::new();
        let mut at = 0;
        while at + 8 <= data.len() {
            let len = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
            out.push((
                data[at + 4..at + 8].try_into().unwrap(),
                data[at..at + len].to_vec(),
            ));
            at += len;
        }
        out
    }

    #[test]
    fn old_and_huge_mp4() {
        let d = tempfile::tempdir().unwrap();
        let mov = std::fs::read("tests/fixtures/video.mov").unwrap();
        // QuickTime without ftyp.
        let bare: Vec<u8> = boxes(&mov)
            .into_iter()
            .filter(|(k, _)| k != b"ftyp")
            .flat_map(|(_, b)| b)
            .collect();
        let old = d.path().join("old.mov");
        std::fs::write(&old, bare).unwrap();
        assert_eq!(read(&old).map(|v| v.height), Some(136));
        // 5 GB of media data (a sparse file) before the moov, its size in
        // 64 bits: skipped, not read. (Windows would fill the file.)
        if cfg!(windows) {
            return;
        }
        let mp4 = std::fs::read("tests/fixtures/video.mp4").unwrap();
        let parts = boxes(&mp4);
        let huge = d.path().join("huge.mp4");
        let mut f = File::create(&huge).unwrap();
        use std::io::Write;
        let mdat: u64 = 5 << 30;
        for (kind, bytes) in &parts {
            if kind == b"moov" {
                let mut head = 1u32.to_be_bytes().to_vec();
                head.extend_from_slice(b"mdat");
                head.extend_from_slice(&mdat.to_be_bytes());
                f.write_all(&head).unwrap();
                f.seek(SeekFrom::Current(mdat as i64 - 16)).unwrap();
            }
            if kind != b"mdat" {
                f.write_all(bytes).unwrap();
            }
        }
        drop(f);
        let start = std::time::Instant::now();
        let v = read(&huge).unwrap();
        assert_eq!((v.width, v.codec.as_str()), (320, "HEVC"));
        assert!(start.elapsed().as_secs() < 2);
    }
}
