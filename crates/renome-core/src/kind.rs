//! What a file really is, told by its first bytes rather than its name,
//! for the extension rule's "from the content": a `.jpg` that is a PNG,
//! an `.mp4` that is a Matroska video, a `.cbz` that is a RAR archive, a
//! file with no extension at all.
//!
//! Only formats with a signature that leaves no doubt are told; plain text
//! (subtitles, playlists, web pages) and containers whose contents cannot
//! be told apart from the start (Office 97 files, executables, databases)
//! are left alone, and so are split archive parts (`.r00`, `.001`).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// What a file's content was found to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kind {
    /// Extensions this content goes by (lowercase); the first is the one
    /// given to a file named otherwise.
    pub names: &'static [&'static str],
    /// For an archive: the comic book extension it takes when its name is
    /// a comic book's of another archive type (`.cbr` that is a zip →
    /// `.cbz`).
    pub comic: Option<&'static str>,
    /// Plain text: it replaces only an extension of a format that is not
    /// text (`.pdf`, `.jpg`), never a text one (`.csv`, `.srt`) or none.
    pub text: bool,
}

const COMIC: &[&str] = &["cbz", "cbr", "cb7", "cbt"];

/// Every kind of file built on a zip, kept as they are named.
const ZIP_BASED: &[&str] = &[
    "zip",
    "cbz",
    "epub",
    "kepub",
    "jar",
    "war",
    "ear",
    "apk",
    "aab",
    "xapk",
    "xpi",
    "kmz",
    "ipa",
    "whl",
    "egg",
    "nupkg",
    "vsix",
    "appx",
    "appxbundle",
    "msix",
    "3mf",
    "xps",
    "oxps",
    "ora",
    "kra",
    "sketch",
    "docx",
    "docm",
    "dotx",
    "dotm",
    "xlsx",
    "xlsm",
    "xltx",
    "xltm",
    "xlsb",
    "pptx",
    "pptm",
    "potx",
    "potm",
    "ppsx",
    "ppsm",
    "vsdx",
    "odt",
    "ott",
    "ods",
    "ots",
    "odp",
    "otp",
    "odg",
    "otg",
    "odf",
    "odb",
    "fodt",
    "sxw",
    "sxc",
    "sxi",
    "pages",
    "numbers",
    "key",
    "usdz",
    "aar",
    "zipx",
    "mcpack",
    "mcworld",
    "pk3",
    "pk4",
    "pak",
    "love",
];

/// TIFF files and the raw camera formats built on it.
const TIFF: &[&str] = &[
    "tif", "tiff", "dng", "cr2", "nef", "nrw", "arw", "srf", "sr2", "pef", "ptx", "erf", "3fr",
    "kdc", "dcr", "dcs", "drf", "k25", "mos", "mef", "mdc", "raw", "rwl", "iiq", "srw", "fff",
    "cap", "eip", "gpr", "orf", "rw2", "exif", "tif2",
];

/// The MPEG-4 family, kept as named whatever the brand says.
const MP4: &[&str] = &[
    "mp4", "m4v", "m4a", "m4b", "m4p", "m4r", "mov", "qt", "f4v", "f4a", "f4b", "f4p", "3gp",
    "3g2", "3gpp", "ismv", "isma", "mp4v", "dash", "lrv", "insv", "aax",
];

const MKV: &[&str] = &["mkv", "mka", "mks", "mk3d", "webm"];

/// Reads the file's kind; `None` when the content is not one told here.
pub fn read(path: &Path) -> Option<Kind> {
    let mut f = File::open(path).ok()?;
    let mut head = vec![0u8; 64 * 1024];
    let n = read_full(&mut f, &mut head);
    head.truncate(n);
    if head.is_empty() {
        return None;
    }
    of_head(&head, &mut f, path)
}

fn read_full(f: &mut File, buf: &mut [u8]) -> usize {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => n += k,
        }
    }
    n
}

fn at(f: &mut File, offset: u64, len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    if f.seek(SeekFrom::Start(offset)).is_err() {
        return Vec::new();
    }
    let n = read_full(f, &mut buf);
    buf.truncate(n);
    buf
}

fn kind(names: &'static [&'static str]) -> Option<Kind> {
    Some(Kind {
        names,
        comic: None,
        text: false,
    })
}

fn archive(names: &'static [&'static str], comic: &'static str) -> Option<Kind> {
    Some(Kind {
        names,
        comic: Some(comic),
        text: false,
    })
}

fn of_head(h: &[u8], f: &mut File, path: &Path) -> Option<Kind> {
    let starts = |sig: &[u8]| h.starts_with(sig);
    let at4 = |sig: &[u8]| h.len() >= 4 + sig.len() && &h[4..4 + sig.len()] == sig;
    let at8 = |sig: &[u8]| h.len() >= 8 + sig.len() && &h[8..8 + sig.len()] == sig;

    // Images.
    if starts(&[0xff, 0xd8, 0xff]) {
        return kind(&["jpg", "jpeg", "jpe", "jfif", "jif", "mpo", "thm"]);
    }
    if starts(b"\x89PNG\r\n\x1a\n") {
        return kind(&["png", "apng"]);
    }
    if starts(b"GIF87a") || starts(b"GIF89a") {
        return kind(&["gif"]);
    }
    if starts(b"RIFF") && at8(b"WEBP") {
        return kind(&["webp"]);
    }
    if starts(b"II*\0") || starts(b"MM\0*") || starts(b"II+\0") || starts(b"MM\0+") {
        return kind(TIFF);
    }
    if starts(b"IIRO") || starts(b"IIRS") {
        return kind(&["orf"]);
    }
    if starts(b"IIU\0") {
        return kind(&["rw2", "rwl", "raw"]);
    }
    if starts(b"FUJIFILMCCD-RAW") {
        return kind(&["raf"]);
    }
    if starts(b"8BPS") {
        return kind(&["psd", "psb"]);
    }
    if starts(&[0xff, 0x0a]) || starts(b"\0\0\0\x0cJXL \r\n\x87\n") {
        return kind(&["jxl"]);
    }
    if starts(b"\0\0\0\x0cjP  \r\n\x87\n") {
        return kind(&["jp2", "jpx", "jpf", "jpm", "j2k"]);
    }
    if starts(b"BM") && h.len() >= 14 && bmp(h, f) {
        return kind(&["bmp", "dib"]);
    }
    if (starts(&[0, 0, 1, 0]) || starts(&[0, 0, 2, 0])) && h.len() >= 22 && icon(h) {
        return if h[2] == 1 {
            kind(&["ico"])
        } else {
            kind(&["cur"])
        };
    }

    // ISO base media (MP4, MOV, HEIF, AVIF, CR3).
    if at4(b"ftyp") {
        return ftyp(h, path);
    }

    // Matroska.
    if starts(&[0x1a, 0x45, 0xdf, 0xa3]) {
        return if contains(&h[..h.len().min(64)], b"webm") {
            kind(&["webm", "mkv"])
        } else if crate::video::read(path).is_some() {
            kind(MKV)
        } else {
            kind(&["mka", "mkv", "mks"])
        };
    }

    // Other audio and video.
    if starts(b"RIFF") && at8(b"WAVE") {
        return kind(&["wav", "wave", "bwf"]);
    }
    if starts(b"RIFF") && at8(b"AVI ") {
        return kind(&["avi", "divx"]);
    }
    if starts(b"FORM") && (at8(b"AIFF") || at8(b"AIFC")) {
        return kind(&["aiff", "aif", "aifc"]);
    }
    if starts(b"fLaC") {
        return kind(&["flac"]);
    }
    if starts(b"ID3") && h.len() >= 10 {
        return after_id3(h, f);
    }
    if starts(b"OggS") {
        return ogg(h);
    }
    if starts(b"MAC ") {
        return kind(&["ape"]);
    }
    if starts(b"wvpk") {
        return kind(&["wv"]);
    }
    if starts(b"MThd") {
        return kind(&["mid", "midi", "kar", "rmi", "smf"]);
    }
    if starts(b"#!AMR") {
        return kind(&["amr", "awb"]);
    }
    if starts(b"DSD ") {
        return kind(&["dsf"]);
    }
    if starts(b"FRM8") {
        return kind(&["dff"]);
    }
    if starts(&[0x30, 0x26, 0xb2, 0x75, 0x8e, 0x66, 0xcf, 0x11]) {
        return kind(&["asf", "wma", "wmv"]);
    }
    if starts(b"FLV\x01") {
        return kind(&["flv"]);
    }
    if starts(&[0, 0, 1, 0xba]) {
        return kind(&[
            "mpg", "mpeg", "vob", "m2p", "mpe", "mod", "vro", "dat", "evo",
        ]);
    }
    if starts(&[0, 0, 1, 0xb3]) {
        return kind(&["mpg", "mpeg", "m1v", "m2v", "mpv"]);
    }
    if transport(h, 188, 0) {
        return kind(&["ts", "mts", "m2t", "tsv", "tsa", "trp", "tp"]);
    }
    if transport(h, 192, 4) {
        return kind(&["m2ts", "mts", "m2t", "ts"]);
    }
    if let Some(k) = mpeg_audio(h, 0) {
        return Some(k);
    }

    // Documents and books.
    if contains(&h[..h.len().min(1024)], b"%PDF-") {
        return kind(&["pdf", "ai"]);
    }
    if starts(b"%!PS") {
        return kind(&["ps", "eps", "epsf", "epsi"]);
    }
    if starts(&[0xc5, 0xd0, 0xd3, 0xc6]) {
        return kind(&["eps", "epsf"]);
    }
    if starts(b"{\\rtf") {
        return kind(&["rtf"]);
    }
    if starts(b"AT&TFORM") {
        return kind(&["djvu", "djv"]);
    }
    if h.len() >= 68 && &h[60..68] == b"BOOKMOBI" {
        return kind(&["mobi", "azw", "azw3", "prc", "azw4"]);
    }
    if fiction_book(h) {
        return kind(&["fb2"]);
    }

    // Fonts.
    if starts(b"OTTO") {
        return kind(&["otf"]);
    }
    if starts(b"ttcf") {
        return kind(&["ttc"]);
    }
    if starts(b"wOFF") {
        return kind(&["woff"]);
    }
    if starts(b"wOF2") {
        return kind(&["woff2"]);
    }
    if starts(&[0, 1, 0, 0]) && h.len() >= 12 && true_type(h) {
        return kind(&["ttf", "otf"]);
    }

    // Archives.
    if starts(b"PK\x03\x04") || starts(b"PK\x05\x06") {
        return zip(path);
    }
    if starts(b"Rar!\x1a\x07") {
        return archive(&["rar", "cbr"], "cbr");
    }
    if starts(&[0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c]) {
        return archive(&["7z", "cb7"], "cb7");
    }
    if h.len() >= 262 && &h[257..262] == b"ustar" {
        return archive(&["tar", "cbt"], "cbt");
    }
    if starts(&[0x1f, 0x8b, 0x08]) {
        return kind(&["gz", "tgz", "gzip", "svgz", "emz", "wmz", "z"]);
    }
    if starts(b"BZh") && h.len() > 3 && (b'1'..=b'9').contains(&h[3]) {
        return kind(&["bz2", "tbz2", "tbz", "bz"]);
    }
    if starts(&[0xfd, b'7', b'z', b'X', b'Z', 0]) {
        return kind(&["xz", "txz"]);
    }
    if starts(&[0x28, 0xb5, 0x2f, 0xfd]) {
        return kind(&["zst", "tzst", "zstd"]);
    }
    if starts(&[0x04, 0x22, 0x4d, 0x18]) {
        return kind(&["lz4"]);
    }
    if starts(b"LZIP") {
        return kind(&["lz", "tlz"]);
    }
    if starts(b"MSCF\0\0\0\0") {
        return kind(&["cab"]);
    }
    if starts(&[0xed, 0xab, 0xee, 0xdb]) {
        return kind(&["rpm"]);
    }
    if starts(b"!<arch>\ndebian") {
        return kind(&["deb", "udeb"]);
    }
    if at(f, 0x8001, 5) == b"CD001" {
        return kind(&["iso", "img", "udf"]);
    }
    text(h, f)
}

/// Extensions of formats that are not text: a text file named so gets a
/// text extension.
const BINARY: &[&str] = &[
    "pdf", "jpg", "jpeg", "png", "gif", "webp", "bmp", "tif", "tiff", "heic", "avif", "psd", "mp3",
    "m4a", "m4b", "flac", "ogg", "opus", "wav", "aac", "wma", "aiff", "ape", "mp4", "m4v", "mkv",
    "webm", "avi", "mov", "wmv", "flv", "mpg", "mpeg", "ts", "m2ts", "vob", "zip", "rar", "7z",
    "tar", "gz", "bz2", "xz", "zst", "cbz", "cbr", "cb7", "cbt", "epub", "mobi", "azw", "azw3",
    "djvu", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "iso", "exe", "dll",
    "bin", "dat", "ttf", "otf", "woff", "woff2", "apk", "jar", "deb", "rpm", "cab", "ico", "jxl",
    "dng", "cr2", "nef", "arw", "raw", "mid", "midi", "amr", "3gp", "dsf",
];

/// Plain text (UTF-8, UTF-16 with its mark, or bytes of a one-byte code
/// page with no controls), and what kind when its start tells.
fn text(h: &[u8], f: &mut File) -> Option<Kind> {
    let body = if h.starts_with(&[0xff, 0xfe]) || h.starts_with(&[0xfe, 0xff]) {
        // UTF-16: told by its mark; its kind is not looked into.
        return text_kind(&["txt"]);
    } else {
        h.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(h)
    };
    let whole = f.metadata().map(|m| m.len() as usize).unwrap_or(0) <= h.len();
    let s = match std::str::from_utf8(body) {
        Ok(s) => s.to_string(),
        // Cut in the middle of a letter at the end of what was read.
        Err(e) if e.error_len().is_none() && !whole => {
            String::from_utf8_lossy(&body[..e.valid_up_to()]).into_owned()
        }
        Err(_) => {
            // A one-byte code page: no control characters but tab and line
            // ends.
            if body
                .iter()
                .any(|&b| b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r' | 0x0c) || b == 0x7f)
            {
                return None;
            }
            encoding_rs::WINDOWS_1252
                .decode_without_bom_handling(body)
                .0
                .into_owned()
        }
    };
    if s.chars()
        .any(|c| c.is_control() && !matches!(c, '\t' | '\n' | '\r' | '\u{c}'))
    {
        return None;
    }
    let start = s.trim_start();
    let lower: String = start.chars().take(200).collect::<String>().to_lowercase();
    if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
        return text_kind(&["html", "htm", "xhtml"]);
    }
    if lower.starts_with("<svg") || (lower.starts_with("<?xml") && s.contains("<svg")) {
        return text_kind(&["svg"]);
    }
    if lower.starts_with("<?xml") {
        return text_kind(&["xml"]);
    }
    if start.starts_with("WEBVTT") {
        return text_kind(&["vtt"]);
    }
    if start.starts_with("[Script Info]") {
        return text_kind(&["ass", "ssa"]);
    }
    if start.starts_with("#EXTM3U") {
        return text_kind(&["m3u8", "m3u"]);
    }
    if srt(start) {
        return text_kind(&["srt"]);
    }
    if whole
        && (start.starts_with('{') || start.starts_with('['))
        && serde_json::from_str::<serde_json::Value>(&s).is_ok()
    {
        return text_kind(&["json"]);
    }
    text_kind(&["txt"])
}

fn text_kind(names: &'static [&'static str]) -> Option<Kind> {
    Some(Kind {
        names,
        comic: None,
        text: true,
    })
}

/// A SubRip subtitle: a number, then `00:00:01,000 --> 00:00:02,000`.
fn srt(s: &str) -> bool {
    static TIMES: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^\d+\s*\r?\n\d{1,2}:\d{2}:\d{2}[,.]\d{1,3}\s*-->").unwrap()
    });
    TIMES.is_match(s)
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn u16le(h: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([h[i], h[i + 1]])
}

fn u32le(h: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([h[i], h[i + 1], h[i + 2], h[i + 3]])
}

fn u32be(h: &[u8], i: usize) -> u32 {
    u32::from_be_bytes([h[i], h[i + 1], h[i + 2], h[i + 3]])
}

/// `BM` is common at the start of text; a bitmap's header gives its own
/// size and where its pixels start.
fn bmp(h: &[u8], f: &mut File) -> bool {
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let size = u64::from(u32le(h, 2));
    let reserved = u32le(h, 6);
    let offset = u64::from(u32le(h, 10));
    reserved == 0 && (size == len || size == 0) && offset >= 26 && offset < len
}

fn icon(h: &[u8]) -> bool {
    let count = u16le(h, 4);
    // First entry: reserved byte 0, planes 0/1 (icons), and its data
    // after the directory.
    let offset = u32le(h, 18) as usize;
    count > 0 && count < 256 && h[9] == 0 && offset >= 6 + 16 * count as usize
}

fn true_type(h: &[u8]) -> bool {
    let tables = u16::from_be_bytes([h[4], h[5]]);
    (4..=64).contains(&tables)
}

fn fiction_book(h: &[u8]) -> bool {
    let start = &h[..h.len().min(1024)];
    let s = String::from_utf8_lossy(start);
    s.trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with('<')
        && s.contains("<FictionBook")
}

fn transport(h: &[u8], packet: usize, offset: usize) -> bool {
    (0..4).all(|i| h.get(offset + i * packet) == Some(&0x47))
}

fn ftyp(h: &[u8], path: &Path) -> Option<Kind> {
    let size = (u32be(h, 0) as usize).min(h.len());
    let major = h.get(8..12)?;
    // Compatible brands follow the minor version.
    let brands: Vec<&[u8]> = std::iter::once(major)
        .chain(
            h.get(16..size)
                .unwrap_or(&[])
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| &c[..]),
        )
        .collect();
    let has = |b: &[u8; 4]| brands.iter().any(|x| *x == b);
    match major {
        b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" => kind(&["heic", "heif", "hif"]),
        b"avif" | b"avis" => kind(&["avif"]),
        b"crx " => kind(&["cr3"]),
        b"jp2 " | b"jpx " | b"jpm " => kind(&["jp2", "jpx", "jpf", "jpm"]),
        b"mif1" | b"msf1" if has(b"avif") => kind(&["avif"]),
        b"mif1" | b"msf1" => kind(&["heic", "heif", "hif", "avif"]),
        b"M4A " => kind(&["m4a", "m4b", "m4r", "m4p", "mp4", "aac"]),
        b"M4B " => kind(&["m4b", "m4a", "aax", "mp4"]),
        b"M4P " => kind(&["m4p", "m4a", "mp4"]),
        b"M4V " | b"M4VH" | b"M4VP" => kind(&["m4v", "mp4", "mov"]),
        b"qt  " => kind(&["mov", "qt", "mp4", "m4v"]),
        b"aax " => kind(&["aax", "m4b"]),
        _ if major.starts_with(b"3gp") || major.starts_with(b"3gg") => {
            kind(&["3gp", "3gpp", "mp4", "m4a", "3g2"])
        }
        _ if major.starts_with(b"3g2") => kind(&["3g2", "3gp", "mp4"]),
        _ => {
            if crate::video::read(path).is_some() {
                kind(MP4)
            } else {
                Some(Kind {
                    names: &[
                        "m4a", "m4b", "m4p", "m4r", "mp4", "mov", "f4a", "f4b", "3gp", "aac",
                        "isma", "aax",
                    ],
                    comic: None,
                    text: false,
                })
            }
        }
    }
}

/// An ID3 tag before the audio: FLAC, MP3 or AAC (and AIFF/WAV rarely).
fn after_id3(h: &[u8], f: &mut File) -> Option<Kind> {
    let syncsafe = |b: &[u8]| {
        b.iter()
            .fold(0usize, |acc, &x| (acc << 7) | usize::from(x & 0x7f))
    };
    let mut end = 10 + syncsafe(&h[6..10]);
    if h[5] & 0x10 != 0 {
        end += 10;
    }
    let tail = if end + 16 <= h.len() {
        h[end..].to_vec()
    } else {
        at(f, end as u64, 4096)
    };
    if tail.starts_with(b"fLaC") {
        return kind(&["flac"]);
    }
    // Some taggers pad with zeros past the tag's size.
    let skip = tail.iter().take(4096).take_while(|&&b| b == 0).count();
    mpeg_audio(&tail, skip).or_else(|| kind(&["mp3", "mp2", "mp1", "mpga", "aac"]))
}

/// MPEG audio frames (MP3, MP2) or AAC in ADTS, told by two frames in a
/// row so a stray `0xFF` is not taken for one.
fn mpeg_audio(h: &[u8], start: usize) -> Option<Kind> {
    let len = frame_len(h.get(start..start + 4)?)?;
    let next = h.get(start + len..start + len + 4)?;
    frame_len(next)?;
    let layer = (h[start + 1] >> 1) & 3;
    match layer {
        0 => kind(&["aac", "adts", "m4a"]),
        1 => kind(&["mp3", "mpga", "mp2"]),
        2 => kind(&["mp2", "mpa", "mpga", "mp3"]),
        _ => kind(&["mp1", "mpa", "mp3"]),
    }
}

/// The length of the MPEG audio or ADTS frame starting at `b`.
fn frame_len(b: &[u8]) -> Option<usize> {
    if b.len() < 4 || b[0] != 0xff || b[1] & 0xe0 != 0xe0 {
        return None;
    }
    let version = (b[1] >> 3) & 3; // 0: 2.5, 2: 2, 3: 1
    let layer = (b[1] >> 1) & 3; // 0: AAC (ADTS), 1: III, 2: II, 3: I
    if layer == 0 {
        // ADTS: 13-bit length from byte 3.
        if b[1] & 0xf6 != 0xf0 {
            return None;
        }
        return adts_len(b);
    }
    if version == 1 {
        return None;
    }
    let bitrate_index = usize::from(b[2] >> 4);
    let rate_index = usize::from((b[2] >> 2) & 3);
    if bitrate_index == 0 || bitrate_index == 15 || rate_index == 3 {
        return None;
    }
    let padding = usize::from((b[2] >> 1) & 1);
    const RATES: [[u32; 3]; 3] = [
        [44100, 48000, 32000],
        [22050, 24000, 16000],
        [11025, 12000, 8000],
    ];
    let rate = RATES[match version {
        3 => 0,
        2 => 1,
        _ => 2,
    }][rate_index];
    const V1: [[u32; 15]; 3] = [
        [
            0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
        ],
        [
            0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
        ],
        [
            0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
        ],
    ];
    const V2: [[u32; 15]; 2] = [
        [
            0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
        ],
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
    ];
    let kbps = if version == 3 {
        V1[usize::from(3 - layer)][bitrate_index]
    } else {
        V2[if layer == 3 { 0 } else { 1 }][bitrate_index]
    };
    let bits = kbps * 1000;
    let len = match layer {
        3 => (12 * bits / rate + padding as u32) * 4,
        2 => 144 * bits / rate + padding as u32,
        _ if version == 3 => 144 * bits / rate + padding as u32,
        _ => 72 * bits / rate + padding as u32,
    } as usize;
    (len >= 4).then_some(len)
}

fn adts_len(b: &[u8]) -> Option<usize> {
    if b.len() < 6 {
        return None;
    }
    let len = (usize::from(b[3] & 3) << 11) | (usize::from(b[4]) << 3) | (usize::from(b[5]) >> 5);
    (len >= 7).then_some(len)
}

fn ogg(h: &[u8]) -> Option<Kind> {
    let segments = usize::from(*h.get(26)?);
    let packet = h.get(27 + segments..).unwrap_or(&[]);
    let is = |sig: &[u8]| packet.starts_with(sig);
    if is(b"\x01vorbis") {
        kind(&["ogg", "oga"])
    } else if is(b"OpusHead") {
        kind(&["opus", "ogg", "oga"])
    } else if is(b"\x80theora") {
        kind(&["ogv", "ogg"])
    } else if is(b"\x7fFLAC") || is(b"fLaC") {
        kind(&["oga", "ogg"])
    } else if is(b"Speex   ") {
        kind(&["spx", "ogg", "oga"])
    } else {
        kind(&["ogg", "oga", "ogv", "ogx", "opus", "spx"])
    }
}

/// What a zip holds: an EPUB, an OpenDocument or Office Open XML file, an
/// Android package, a comic (pictures only) or a plain zip.
fn zip(path: &Path) -> Option<Kind> {
    let Ok(mut z) = zip::ZipArchive::new(File::open(path).ok()?) else {
        return kind(ZIP_BASED);
    };
    let names: Vec<String> = z
        .file_names()
        .filter_map(Result::ok)
        .map(|n| n.into_owned())
        .collect();
    let has = |n: &str| names.iter().any(|x| x == n);
    if has("mimetype") {
        let mut mime = String::new();
        if let Ok(e) = z.by_name("mimetype") {
            let _ = e.take(200).read_to_string(&mut mime);
        }
        match mime.trim() {
            "application/epub+zip" => return kind(&["epub", "kepub"]),
            "application/vnd.oasis.opendocument.text" => return kind(&["odt", "fodt"]),
            "application/vnd.oasis.opendocument.text-template" => return kind(&["ott"]),
            "application/vnd.oasis.opendocument.spreadsheet" => return kind(&["ods"]),
            "application/vnd.oasis.opendocument.spreadsheet-template" => return kind(&["ots"]),
            "application/vnd.oasis.opendocument.presentation" => return kind(&["odp"]),
            "application/vnd.oasis.opendocument.presentation-template" => return kind(&["otp"]),
            "application/vnd.oasis.opendocument.graphics" => return kind(&["odg"]),
            "image/openraster" => return kind(&["ora"]),
            "application/x-krita" => return kind(&["kra"]),
            _ => {}
        }
    }
    if has("[Content_Types].xml") {
        let under = |dir: &str| names.iter().any(|n| n.starts_with(dir));
        if under("word/") {
            return kind(&["docx", "docm", "dotx", "dotm"]);
        }
        if under("xl/") {
            return kind(&["xlsx", "xlsm", "xltx", "xltm", "xlsb"]);
        }
        if under("ppt/") {
            return kind(&["pptx", "pptm", "potx", "potm", "ppsx", "ppsm"]);
        }
        if under("visio/") {
            return kind(&["vsdx"]);
        }
    }
    if has("AndroidManifest.xml") {
        return kind(&["apk", "aab", "xapk", "zip"]);
    }
    // A comic is pictures with, at most, notes beside them.
    let files: Vec<&String> = names.iter().filter(|n| !n.ends_with('/')).collect();
    let picture = |n: &str| {
        let ext = n.rsplit('.').next().unwrap_or("").to_lowercase();
        matches!(
            ext.as_str(),
            "jpg"
                | "jpeg"
                | "png"
                | "gif"
                | "webp"
                | "bmp"
                | "avif"
                | "jxl"
                | "heic"
                | "tif"
                | "tiff"
        )
    };
    let note = |n: &str| {
        let ext = n.rsplit('.').next().unwrap_or("").to_lowercase();
        matches!(
            ext.as_str(),
            "xml" | "txt" | "nfo" | "sfv" | "json" | "db" | "url" | "ini"
        )
    };
    let pictures = files.iter().filter(|n| picture(n)).count();
    if pictures > 0 && files.iter().all(|n| picture(n) || note(n)) {
        return Some(Kind {
            names: &["cbz", "zip"],
            comic: Some("cbz"),
            text: false,
        });
    }
    Some(Kind {
        names: ZIP_BASED,
        comic: Some("cbz"),
        text: false,
    })
}

/// Parts of split archives, named by their number: never changed.
fn split_part(ext: &str) -> bool {
    let b = ext.as_bytes();
    let digits = |s: &[u8]| !s.is_empty() && s.iter().all(u8::is_ascii_digit);
    (b.len() == 3 && matches!(b[0], b'r' | b'z' | b's') && digits(&b[1..]))
        || (b.len() == 3 && digits(b))
}

/// Whether the text after a name's last dot is an extension (`jpg`,
/// `mp3`, `torrent`) rather than part of the name (`0_x86[0]`, `Jr`
/// would pass too, but is then replaced only when the content is told).
pub fn looks_like_extension(ext: &str) -> bool {
    !ext.is_empty() && ext.len() <= 10 && ext.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Extensions of text formats, which a text file's name may already
/// carry before a wrong one (`movie.nfo.pdf`).
const TEXT: &[&str] = &[
    "txt", "text", "nfo", "xml", "html", "htm", "xhtml", "svg", "json", "srt", "ass", "ssa", "vtt",
    "sub", "sbv", "lrc", "csv", "tsv", "md", "markdown", "log", "ini", "cfg", "conf", "yml",
    "yaml", "toml", "m3u", "m3u8", "pls", "cue", "sfv", "md5", "sha1", "sha256", "url", "opf",
    "ncx", "css", "js", "py", "sh", "bat", "ps1", "tex", "bib", "diz", "rst", "adoc",
];

impl Kind {
    /// Whether text of this kind is valid under the text extension `ext`:
    /// formats with a structure (JSON, XML, web pages, subtitles) need
    /// content told as that; free ones (`txt`, `nfo`, `csv`) take any.
    fn text_fits(&self, ext: &str) -> bool {
        const XML: &[&str] = &["xml", "opf", "ncx", "xhtml", "svg"];
        const STRICT: &[&str] = &[
            "json", "xml", "opf", "ncx", "xhtml", "svg", "html", "htm", "srt", "vtt", "ass", "ssa",
            "m3u8", "m3u",
        ];
        if !self.text || !TEXT.contains(&ext) {
            return false;
        }
        if !STRICT.contains(&ext) {
            return true;
        }
        let is = |k: &str| self.names.first() == Some(&k);
        match ext {
            "opf" | "ncx" => is("xml"),
            "xhtml" => is("xml") || is("html"),
            // An `.m3u` may be a bare list of paths.
            "m3u" | "m3u8" => is("m3u8") || is("txt"),
            _ if XML.contains(&ext) => self.names.contains(&ext),
            _ => self.names.contains(&ext),
        }
    }

    /// The name (stem, extension) a file with this content and that name
    /// should have; `None` when it already has a fitting one. When the
    /// name carries a fitting extension before the wrong one
    /// (`movie.nfo.pdf` that is text), the wrong one is only taken off;
    /// after a dot that starts no extension (`v1.0_x86[0]`), the new one
    /// is added.
    pub fn fix_name(&self, stem: &str, ext: &str) -> Option<(String, String)> {
        let new = self.fix(ext)?;
        if !ext.is_empty() && !looks_like_extension(ext) {
            return Some((format!("{stem}.{ext}"), new));
        }
        if let Some((before, inner)) = stem.rsplit_once('.')
            && !before.is_empty()
            && looks_like_extension(inner)
        {
            let lower = inner.to_lowercase();
            let fits = self.names.contains(&lower.as_str()) || self.text_fits(&lower);
            if fits {
                return Some((before.to_string(), inner.to_string()));
            }
        }
        Some((stem.to_string(), new))
    }

    /// The extension a file with this content and extension `ext` should
    /// have; `None` when it already has a fitting one.
    pub fn fix(&self, ext: &str) -> Option<String> {
        let lower = ext.to_lowercase();
        if split_part(&lower) || self.names.contains(&lower.as_str()) {
            return None;
        }
        if self.text && !BINARY.contains(&lower.as_str()) {
            return None;
        }
        let new = match self.comic {
            Some(c) if COMIC.contains(&lower.as_str()) => c,
            _ => self.names[0],
        };
        if new == lower {
            return None;
        }
        let upper = looks_like_extension(ext)
            && ext.bytes().any(|b| b.is_ascii_alphabetic())
            && ext.chars().all(|c| !c.is_lowercase());
        Some(if upper {
            new.to_uppercase()
        } else {
            new.to_string()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn kind_of(bytes: &[u8]) -> Option<Kind> {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        std::fs::write(&p, bytes).unwrap();
        read(&p)
    }

    fn fix(bytes: &[u8], ext: &str) -> Option<String> {
        kind_of(bytes)?.fix(ext)
    }

    #[test]
    fn images() {
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
        assert_eq!(fix(png, "jpg").as_deref(), Some("png"));
        assert_eq!(fix(png, "JPG").as_deref(), Some("PNG"));
        assert_eq!(fix(png, "png"), None);
        assert_eq!(fix(png, "PNG"), None);
        assert_eq!(fix(png, "").as_deref(), Some("png"));
        assert_eq!(fix(&[0xff, 0xd8, 0xff, 0xe0, 0, 0x10], "jpeg"), None);
        assert_eq!(
            fix(&[0xff, 0xd8, 0xff, 0xe0, 0, 0x10], "png").as_deref(),
            Some("jpg")
        );
        // Raw camera files are TIFF inside and keep their names.
        assert_eq!(fix(b"II*\0\x08\0\0\0", "nef"), None);
        assert_eq!(fix(b"II*\0\x08\0\0\0", "jpg").as_deref(), Some("tif"));
    }

    #[test]
    fn text() {
        let srt = b"1\n00:00:01,000 --> 00:00:02,000\nHello\n";
        // Text only replaces the extension of a format that is not text.
        assert_eq!(fix(srt, "srt"), None);
        assert_eq!(fix(srt, "txt"), None);
        assert_eq!(fix(srt, ""), None);
        assert_eq!(fix(srt, "pdf").as_deref(), Some("srt"));
        let plain = "Ol\u{e1}, mundo\r\n".as_bytes();
        assert_eq!(fix(plain, "pdf").as_deref(), Some("txt"));
        assert_eq!(fix(plain, "PDF").as_deref(), Some("TXT"));
        assert_eq!(fix(plain, "csv"), None);
        assert_eq!(fix(plain, "md"), None);
        // Latin-1 text.
        assert_eq!(fix(b"Ol\xe1 \x93x\x94\n", "jpg").as_deref(), Some("txt"));
        assert_eq!(
            fix(b"BM is not a bitmap here", "bmp").as_deref(),
            Some("txt")
        );
        assert_eq!(
            fix(b"<!DOCTYPE html><html>", "pdf").as_deref(),
            Some("html")
        );
        assert_eq!(fix(b"{\"a\": [1, 2]}", "zip").as_deref(), Some("json"));
        assert_eq!(
            fix(b"#EXTM3U\n#EXTINF:1,x\na.mp3\n", "mp3").as_deref(),
            Some("m3u8")
        );
        let k = kind_of(b"<?xml version=\"1.0\"?><movie/>").unwrap();
        let name = |s: &str, e: &str| k.fix_name(s, e).map(|(s, e)| format!("{s}.{e}"));
        assert_eq!(
            name("movie (Copy).nfo", "pdf").as_deref(),
            Some("movie (Copy).nfo")
        );
        assert_eq!(name("movie", "pdf").as_deref(), Some("movie.xml"));
        assert_eq!(
            name("report.2024", "pdf").as_deref(),
            Some("report.2024.xml")
        );
        assert_eq!(name("movie.nfo", "xml"), None);
        assert_eq!(name("book.opf", "pdf").as_deref(), Some("book.opf"));
        // Structured formats need content of that kind.
        assert_eq!(name("data.json", "pdf").as_deref(), Some("data.json.xml"));
        assert_eq!(name("sub.srt", "pdf").as_deref(), Some("sub.srt.xml"));
        let plain = kind_of(b"just some words\n").unwrap();
        let name = |s: &str, e: &str| plain.fix_name(s, e).map(|(s, e)| format!("{s}.{e}"));
        assert_eq!(name("notes.csv", "pdf").as_deref(), Some("notes.csv"));
        assert_eq!(name("data.json", "pdf").as_deref(), Some("data.json.txt"));
        assert_eq!(name("page.html", "pdf").as_deref(), Some("page.html.txt"));
        // Binary junk is no text.
        assert_eq!(kind_of(b"\x00\x01\x02garbage"), None);
        assert_eq!(kind_of(b""), None);
    }

    #[test]
    fn audio() {
        // Two MPEG-1 layer III frames, 128 kbps at 44.1 kHz: 417 bytes.
        let mut mp3 = vec![0u8; 417 * 2 + 4];
        for i in [0, 417, 834] {
            mp3[i..i + 4].copy_from_slice(&[0xff, 0xfb, 0x90, 0x00]);
        }
        assert_eq!(fix(&mp3, "m4a").as_deref(), Some("mp3"));
        assert_eq!(fix(&mp3, "mp3"), None);
        let mut tagged = b"ID3\x04\0\0\0\0\0\x0a".to_vec();
        tagged.extend([0u8; 10]);
        tagged.extend(b"fLaC\0\0\0\x22");
        assert_eq!(fix(&tagged, "mp3").as_deref(), Some("flac"));
        let mut ogg = b"OggS\0\x02".to_vec();
        ogg.resize(26, 0);
        ogg.push(1);
        ogg.push(19);
        ogg.extend(b"OpusHead\x01\x02");
        assert_eq!(fix(&ogg, "ogg"), None);
        assert_eq!(fix(&ogg, "mp3").as_deref(), Some("opus"));
    }

    #[test]
    fn mp4_brands() {
        let mut m4a = vec![0, 0, 0, 0x20];
        m4a.extend(b"ftypM4A \0\0\0\0M4A mp42isom\0\0\0\0");
        assert_eq!(fix(&m4a, "mp3").as_deref(), Some("m4a"));
        assert_eq!(fix(&m4a, "m4b"), None);
        let mut heic = vec![0, 0, 0, 0x18];
        heic.extend(b"ftypheic\0\0\0\0mif1heic");
        assert_eq!(fix(&heic, "jpg").as_deref(), Some("heic"));
    }

    fn zipped(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (name, data) in files {
                z.start_file(*name, opts).unwrap();
                z.write_all(data).unwrap();
            }
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn archives() {
        let comic = zipped(&[
            ("001.jpg", b"x"),
            ("002.jpg", b"y"),
            ("ComicInfo.xml", b"<a/>"),
        ]);
        assert_eq!(fix(&comic, "cbr").as_deref(), Some("cbz"));
        assert_eq!(fix(&comic, "rar").as_deref(), Some("cbz"));
        assert_eq!(fix(&comic, "zip"), None);
        let epub = zipped(&[
            ("mimetype", b"application/epub+zip"),
            ("OEBPS/a.xhtml", b""),
        ]);
        assert_eq!(fix(&epub, "cbz").as_deref(), Some("epub"));
        assert_eq!(fix(&epub, "zip").as_deref(), Some("epub"));
        let docx = zipped(&[("[Content_Types].xml", b""), ("word/document.xml", b"")]);
        assert_eq!(fix(&docx, "doc").as_deref(), Some("docx"));
        let plain = zipped(&[("notes.txt", b""), ("setup.exe", b"")]);
        assert_eq!(fix(&plain, "jar"), None);
        assert_eq!(fix(&plain, "cbr").as_deref(), Some("cbz"));
        assert_eq!(fix(&plain, "rar").as_deref(), Some("zip"));
        let rar = b"Rar!\x1a\x07\x01\0";
        assert_eq!(fix(rar, "cbz").as_deref(), Some("cbr"));
        assert_eq!(fix(rar, "zip").as_deref(), Some("rar"));
        assert_eq!(fix(rar, "r00"), None);
        assert_eq!(fix(rar, "001"), None);
    }
}
