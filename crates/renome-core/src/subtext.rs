//! Text subtitles: read in their formats (SubRip, WebVTT, SubStation,
//! MicroDVD, SubViewer, SAMI) and encodings, cleaned (credits and
//! advertisements, marks for the hard of hearing), retimed (a delay, a
//! frame rate), written back in their own format or made into SubRip.
//!
//! Edits keep everything the format holds besides the lines touched
//! (styles, positions, comments): only the times and texts of the cues
//! are written again. A file whose format is not recognised is not
//! touched. Picture subtitles (VobSub, PGS) cannot be read as text.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use encoding_rs::Encoding;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::convert::Original;
use crate::tagwrite::Backup;

/// A text subtitle's format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    /// SubRip (`.srt`).
    Srt,
    /// WebVTT (`.vtt`).
    Vtt,
    /// SubStation Alpha and Advanced SubStation (`.ssa`, `.ass`).
    Ass,
    /// MicroDVD (`.sub`): times in frames.
    MicroDvd,
    /// SubViewer 2 (`.sub`).
    SubViewer,
    /// SAMI (`.smi`): only made into SubRip.
    Sami,
}

impl Format {
    /// The extension a file in this format has.
    pub fn ext(self) -> &'static str {
        match self {
            Format::Srt => "srt",
            Format::Vtt => "vtt",
            Format::Ass => "ass",
            Format::MicroDvd | Format::SubViewer => "sub",
            Format::Sami => "smi",
        }
    }
}

/// The subtitle extensions whose files may hold text.
pub fn is_text_ext(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "srt" | "vtt" | "ass" | "ssa" | "sub" | "smi" | "sami"
    )
}

/// Subtitles made of pictures (VobSub's `.sub`, Blu-ray's `.sup`): no text
/// to read without recognising the letters.
pub fn is_picture(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0, 0, 1, 0xba]) || bytes.starts_with(b"PG")
}

/// The largest subtitle read: a film's is well under a megabyte.
const MAX_SIZE: u64 = 32 << 20;

// ------------------------------------------------------------- encodings

/// How a subtitle's bytes are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enc {
    Utf8 { bom: bool },
    Utf16 { little: bool },
    Legacy(&'static Encoding),
}

impl Enc {
    /// The name shown for it (`windows-1252`).
    pub fn name(self) -> &'static str {
        match self {
            Enc::Utf8 { .. } => "UTF-8",
            Enc::Utf16 { little: true } => "UTF-16LE",
            Enc::Utf16 { .. } => "UTF-16BE",
            Enc::Legacy(e) => e.name(),
        }
    }

    pub fn is_utf8(self) -> bool {
        matches!(self, Enc::Utf8 { .. })
    }

    /// `text` in this encoding; characters it cannot hold make it UTF-8.
    pub fn encode(self, text: &str) -> Vec<u8> {
        match self {
            Enc::Utf8 { bom } => {
                let mut out = Vec::with_capacity(text.len() + 3);
                if bom {
                    out.extend_from_slice(b"\xEF\xBB\xBF");
                }
                out.extend_from_slice(text.as_bytes());
                out
            }
            Enc::Utf16 { little } => {
                let mut out = Vec::with_capacity(text.len() * 2 + 2);
                out.extend_from_slice(if little { b"\xFF\xFE" } else { b"\xFE\xFF" });
                for u in text.encode_utf16() {
                    out.extend_from_slice(&if little {
                        u.to_le_bytes()
                    } else {
                        u.to_be_bytes()
                    });
                }
                out
            }
            Enc::Legacy(e) => {
                let (bytes, _, lost) = e.encode(text);
                if lost {
                    text.as_bytes().to_vec()
                } else {
                    bytes.into_owned()
                }
            }
        }
    }
}

/// The language a subtitle's name gives (`Movie.ru.srt` → `ru`), to read
/// an old encoding by.
pub fn lang_of_name(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().to_string();
    let (base, ext) = name.rsplit_once('.')?;
    let tags = crate::tmdb::subtitle_tags(base, ext);
    tags.split('.')
        .filter(|t| !t.is_empty())
        .find(|t| !crate::subtitles::langs(t).is_empty())
        .map(str::to_string)
}

/// The old (single or double byte) encoding subtitles in a language were
/// usually written in, before UTF-8.
fn legacy_for(lang: &str) -> Option<&'static Encoding> {
    use encoding_rs::*;
    let (code, region) = lang.split_once(['-', '_']).unwrap_or((lang, ""));
    let two = crate::subtitles::langs(code)
        .first()
        .map(|l| l.tag.split('-').next().unwrap_or("").to_string())
        .unwrap_or_else(|| code.to_ascii_lowercase());
    Some(match two.as_str() {
        "ru" | "uk" | "bg" | "sr" | "mk" | "be" => WINDOWS_1251,
        "el" => WINDOWS_1253,
        "tr" => WINDOWS_1254,
        "he" => WINDOWS_1255,
        "ar" | "fa" => WINDOWS_1256,
        "pl" | "cs" | "sk" | "hu" | "ro" | "hr" | "sl" | "bs" | "sq" => WINDOWS_1250,
        "et" | "lv" | "lt" => WINDOWS_1257,
        "vi" => WINDOWS_1258,
        "th" => WINDOWS_874,
        "ja" => SHIFT_JIS,
        "ko" => EUC_KR,
        "zh" if ["tw", "hk", "mo"].contains(&region.to_ascii_lowercase().as_str()) => BIG5,
        "zh" => GBK,
        "pt" | "en" | "es" | "fr" | "de" | "it" | "nl" | "sv" | "no" | "da" | "fi" | "ca" | "id" => {
            WINDOWS_1252
        }
        _ => return None,
    })
}

/// Of the Western, Central European and Cyrillic encodings, the one an
/// unmarked text most likely is. Cyrillic text is mostly bytes above 127
/// in runs (whole words); Western text has them alone among plain
/// letters; Central European text has some that Western text hardly ever
/// uses (`ł` is `³` there, `ą` is `¹`) or that it leaves undefined.
fn guess_legacy(bytes: &[u8]) -> &'static Encoding {
    let high = bytes.iter().filter(|&&b| b >= 0x80).count();
    if high == 0 {
        return encoding_rs::WINDOWS_1252;
    }
    let paired = bytes
        .windows(2)
        .filter(|w| w[0] >= 0xc0 && w[1] >= 0xc0)
        .count();
    let letters = bytes.iter().filter(|b| b.is_ascii_alphabetic()).count();
    if paired * 2 > high && high * 2 > letters {
        return encoding_rs::WINDOWS_1251;
    }
    let central = bytes
        .iter()
        .filter(|&&b| {
            matches!(
                b,
                0x81 | 0x8c | 0x8d | 0x8f | 0x90 | 0x9c | 0x9d | 0x9f | 0xa3 | 0xa5 | 0xb3 | 0xb9
            )
        })
        .count();
    if central >= 3 && central * 20 >= high {
        encoding_rs::WINDOWS_1250
    } else {
        encoding_rs::WINDOWS_1252
    }
}

/// A subtitle's text and how it was written: UTF-8 or UTF-16 when it is
/// (by its mark, or valid as UTF-8), else the old encoding of its
/// language (`lang`, from its name), else a guess between the Western,
/// Central European and Cyrillic ones.
pub fn decode(bytes: &[u8], lang: Option<&str>) -> (String, Enc) {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return (
            String::from_utf8_lossy(rest).into_owned(),
            Enc::Utf8 { bom: true },
        );
    }
    for (mark, little) in [(b"\xFF\xFE", true), (b"\xFE\xFF", false)] {
        if let Some(rest) = bytes.strip_prefix(mark) {
            let e = if little {
                encoding_rs::UTF_16LE
            } else {
                encoding_rs::UTF_16BE
            };
            return (
                e.decode_without_bom_handling(rest).0.into_owned(),
                Enc::Utf16 { little },
            );
        }
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return (s.to_string(), Enc::Utf8 { bom: false });
    }
    let e = lang
        .and_then(legacy_for)
        .unwrap_or_else(|| guess_legacy(bytes));
    (
        e.decode_without_bom_handling(bytes).0.into_owned(),
        Enc::Legacy(e),
    )
}

/// A subtitle's text as UTF-8 (without a byte order mark), read as
/// [`decode`] does.
pub fn to_utf8(bytes: &[u8], lang: Option<&str>) -> Vec<u8> {
    decode(bytes, lang).0.into_bytes()
}

// ------------------------------------------------------------------ times

fn hms(ms: i64) -> (i64, i64, i64, i64) {
    let ms = ms.max(0);
    (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
}

/// `01:02:03,456`.
fn srt_time(ms: i64) -> String {
    let (h, m, s, f) = hms(ms);
    format!("{h:02}:{m:02}:{s:02},{f:03}")
}

/// `01:02:03.456`.
fn vtt_time(ms: i64) -> String {
    let (h, m, s, f) = hms(ms);
    format!("{h:02}:{m:02}:{s:02}.{f:03}")
}

/// `1:02:03.45` (hundredths, as SubStation writes them).
fn ass_time(ms: i64) -> String {
    let cs = (ms.max(0) + 5) / 10;
    let (h, m, s) = (cs / 360_000, cs / 6000 % 60, cs / 100 % 60);
    format!("{h}:{m:02}:{s:02}.{:02}", cs % 100)
}

/// `01:02:03.45` (SubViewer's).
fn sv_time(ms: i64) -> String {
    let cs = (ms.max(0) + 5) / 10;
    let (h, m, s) = (cs / 360_000, cs / 6000 % 60, cs / 100 % 60);
    format!("{h:02}:{m:02}:{s:02}.{:02}", cs % 100)
}

/// A fraction of a second written with `digits` digits, in milliseconds.
fn fraction(digits: &str) -> i64 {
    let n: i64 = digits.parse().unwrap_or(0);
    match digits.len() {
        0 => 0,
        1 => n * 100,
        2 => n * 10,
        3 => n,
        k => n / 10i64.pow(k as u32 - 3),
    }
}

fn to_ms(h: &str, m: &str, s: &str, f: &str) -> Option<i64> {
    let h: i64 = if h.is_empty() { 0 } else { h.parse().ok()? };
    let m: i64 = m.parse().ok()?;
    let s: i64 = s.parse().ok()?;
    Some(((h * 60 + m) * 60 + s) * 1000 + fraction(f))
}

static SRT_TIMING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(\d+):(\d{1,2}):(\d{1,2})(?:[,.:](\d{1,4}))?\s*-->\s*(\d+):(\d{1,2}):(\d{1,2})(?:[,.:](\d{1,4}))?(.*)$",
    )
    .unwrap()
});

static VTT_TIMING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(?:(\d+):)?(\d{1,2}):(\d{2})\.(\d{1,3})\s+-->\s+(?:(\d+):)?(\d{1,2}):(\d{2})\.(\d{1,3})(.*)$",
    )
    .unwrap()
});

static ASS_TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\d+):(\d{1,2}):(\d{1,2})(?:[.:](\d{1,3}))?\s*$").unwrap());

static SV_TIMING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(\d{1,2}):(\d{2}):(\d{2})[.,](\d{1,3})\s*,\s*(\d{1,2}):(\d{2}):(\d{2})[.,](\d{1,3})\s*$")
        .unwrap()
});

static MDVD_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\{(\d+)\}\{(\d*)\}(.*)$").unwrap());

/// The time of an inline WebVTT timestamp (`<00:01.500>`) inside a cue.
static VTT_INLINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<(?:(\d+):)?(\d{1,2}):(\d{2})\.(\d{3})>").unwrap()
});

fn ass_ms(text: &str) -> Option<i64> {
    let c = ASS_TIME.captures(text)?;
    to_ms(&c[1], &c[2], &c[3], c.get(4).map_or("", |m| m.as_str()))
}

// -------------------------------------------------------------- documents

/// One subtitle line: when it is shown and its text in the format's own
/// markup (lines joined as the format joins them: `\n`, SubStation's
/// `\N`, MicroDVD's `|`, SubViewer's `[br]`).
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub start: i64,
    pub end: i64,
    pub text: String,
}

#[derive(Debug, Clone)]
enum Item {
    /// Kept as it was (with its line ends as the document writes them).
    Raw(String),
    Cue(Cue, Shape),
}

/// What a cue keeps of how its format wrote it.
#[derive(Debug, Clone)]
enum Shape {
    /// Whatever follows the times on the timing line (positions).
    Srt { extra: String },
    Vtt { id: Option<String>, settings: String },
    /// A `Dialogue:` or `Comment:` line split by its `Format:`.
    Ass {
        kind: String,
        fields: Vec<String>,
        start: usize,
        end: usize,
        text: usize,
        style: String,
    },
    MicroDvd,
    SubViewer,
    Sami,
}

/// A subtitle file read.
#[derive(Debug, Clone)]
pub struct Doc {
    pub format: Format,
    /// The line end it uses.
    nl: &'static str,
    items: Vec<Item>,
    /// MicroDVD: frames per second.
    pub fps: Option<f64>,
    /// SubStation: styles written in italics.
    italic_styles: Vec<String>,
}

/// The format of a subtitle's text, by what it holds.
pub fn detect(text: &str) -> Option<Format> {
    let head: String = text.chars().take(8192).collect();
    let first = head.lines().map(str::trim).find(|l| !l.is_empty())?;
    let lower = head.to_ascii_lowercase();
    if first.starts_with("WEBVTT") {
        return Some(Format::Vtt);
    }
    if lower.contains("[script info]") || lower.contains("\ndialogue:") && lower.contains("[events]")
    {
        return Some(Format::Ass);
    }
    if MDVD_LINE.is_match(first) {
        return Some(Format::MicroDvd);
    }
    if lower.contains("<sami") || lower.contains("<sync start") {
        return Some(Format::Sami);
    }
    if head.lines().any(|l| SRT_TIMING.is_match(l) && l.contains("-->")) {
        return Some(Format::Srt);
    }
    if lower.contains("[information]") || head.lines().any(|l| SV_TIMING.is_match(l)) {
        return Some(Format::SubViewer);
    }
    None
}

impl Doc {
    /// Reads `text` (already decoded); `fps` is used for MicroDVD files
    /// that do not say theirs.
    pub fn parse(text: &str, fps: Option<f64>) -> Option<Doc> {
        let format = detect(text)?;
        // Mixed line ends: the most used.
        let crlf = text.matches("\r\n").count();
        let nl = if crlf * 2 > text.matches('\n').count() {
            "\r\n"
        } else {
            "\n"
        };
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut doc = Doc {
            format,
            nl,
            items: Vec::new(),
            fps: None,
            italic_styles: Vec::new(),
        };
        match format {
            Format::Srt => doc.read_srt(&text),
            Format::Vtt => doc.read_vtt(&text),
            Format::Ass => doc.read_ass(&text),
            Format::MicroDvd => doc.read_microdvd(&text, fps),
            Format::SubViewer => doc.read_subviewer(&text),
            Format::Sami => doc.read_sami(&text),
        }
        Some(doc)
    }

    /// The cues, in file order (SubStation comments left out).
    pub fn cues(&self) -> impl Iterator<Item = &Cue> {
        self.items.iter().filter_map(|i| match i {
            Item::Cue(_, Shape::Ass { kind, .. }) if !kind.eq_ignore_ascii_case("dialogue") => None,
            Item::Cue(c, _) => Some(c),
            Item::Raw(_) => None,
        })
    }

    /// MicroDVD without its frame rate: its times cannot be known.
    pub fn needs_fps(&self) -> bool {
        self.format == Format::MicroDvd && self.fps.is_none()
    }

    fn read_srt(&mut self, text: &str) {
        let lines: Vec<&str> = text.split('\n').collect();
        let timings: Vec<usize> = (0..lines.len())
            .filter(|&i| lines[i].contains("-->") && SRT_TIMING.is_match(lines[i]))
            .collect();
        // What comes before the first cue (its number aside) stays.
        let first = timings.first().copied().unwrap_or(lines.len());
        let mut head_end = first;
        if head_end > 0 && is_number(lines[head_end - 1]) {
            head_end -= 1;
        }
        let head = lines[..head_end].join("\n");
        if !head.trim().is_empty() {
            self.items
                .push(Item::Raw(format!("{}\n\n", head.trim_end())));
        }
        for (k, &at) in timings.iter().enumerate() {
            let c = SRT_TIMING.captures(lines[at]).unwrap();
            let get = |i: usize| c.get(i).map_or("", |m| m.as_str());
            let start = to_ms(get(1), get(2), get(3), get(4)).unwrap_or(0);
            let end = to_ms(get(5), get(6), get(7), get(8)).unwrap_or(start);
            let stop = timings.get(k + 1).copied().unwrap_or(lines.len());
            let mut body: Vec<&str> = lines[at + 1..stop].to_vec();
            // The next cue's number, after a blank line.
            while body.last().is_some_and(|l| l.trim().is_empty()) {
                body.pop();
            }
            // The next cue's number: after a blank line, alone, or the
            // number that comes next.
            if k + 1 < timings.len()
                && let Some(last) = body.last()
                && is_number(last)
                && (body.len() == 1
                    || body[body.len() - 2].trim().is_empty()
                    || last.trim().parse::<usize>().ok() == Some(k + 2))
            {
                body.pop();
            }
            while body.last().is_some_and(|l| l.trim().is_empty()) {
                body.pop();
            }
            // An empty line inside a cue would end it when written; one
            // with spaces stays as it was.
            let text = body
                .iter()
                .copied()
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            self.items.push(Item::Cue(
                Cue { start, end, text },
                Shape::Srt {
                    extra: get(9).trim_end().to_string(),
                },
            ));
        }
    }

    fn read_vtt(&mut self, text: &str) {
        let mut blocks: Vec<Vec<&str>> = Vec::new();
        let mut cur: Vec<&str> = Vec::new();
        for line in text.split('\n') {
            if line.trim().is_empty() {
                if !cur.is_empty() {
                    blocks.push(std::mem::take(&mut cur));
                }
            } else {
                cur.push(line);
            }
        }
        if !cur.is_empty() {
            blocks.push(cur);
        }
        for block in blocks {
            let timing = block
                .iter()
                .take(2)
                .position(|l| l.contains("-->") && VTT_TIMING.is_match(l));
            let header = block[0].starts_with("WEBVTT")
                || block[0].starts_with("NOTE")
                || block[0].starts_with("STYLE")
                || block[0].starts_with("REGION");
            match timing {
                Some(at) if !header => {
                    let c = VTT_TIMING.captures(block[at]).unwrap();
                    let get = |i: usize| c.get(i).map_or("", |m| m.as_str());
                    let start = to_ms(get(1), get(2), get(3), get(4)).unwrap_or(0);
                    let end = to_ms(get(5), get(6), get(7), get(8)).unwrap_or(start);
                    let id = (at == 1).then(|| block[0].to_string());
                    self.items.push(Item::Cue(
                        Cue {
                            start,
                            end,
                            text: block[at + 1..]
                                .iter()
                                .map(|l| l.trim_end())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        },
                        Shape::Vtt {
                            id,
                            settings: get(9).trim().to_string(),
                        },
                    ));
                }
                _ => self.items.push(Item::Raw(block.join("\n"))),
            }
        }
    }

    fn read_ass(&mut self, text: &str) {
        let mut format: Vec<String> = Vec::new();
        let mut style_format: Vec<String> = Vec::new();
        let mut section = String::new();
        for line in text.split('\n') {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                section = trimmed.to_ascii_lowercase();
            }
            let (key, rest) = trimmed.split_once(':').unwrap_or(("", ""));
            let key_l = key.trim().to_ascii_lowercase();
            if section.contains("styles") && key_l == "format" {
                style_format = rest.split(',').map(|s| s.trim().to_ascii_lowercase()).collect();
            } else if section.contains("styles") && key_l == "style" {
                let f: Vec<&str> = rest.split(',').map(str::trim).collect();
                let at = |n: &str| style_format.iter().position(|x| x == n);
                if let (Some(name), Some(it)) = (at("name"), at("italic"))
                    && f.get(it).is_some_and(|v| *v == "-1" || *v == "1")
                    && let Some(n) = f.get(name)
                {
                    self.italic_styles.push(n.to_string());
                }
            }
            if section == "[events]" && key_l == "format" {
                format = rest.split(',').map(|s| s.trim().to_ascii_lowercase()).collect();
            }
            let event = section == "[events]" && (key_l == "dialogue" || key_l == "comment");
            if event && !format.is_empty() {
                let at = |n: &str| format.iter().position(|x| x == n);
                if let (Some(s), Some(e), Some(t)) = (at("start"), at("end"), at("text")) {
                    // The text is the last field and may hold commas.
                    let body = rest.strip_prefix(' ').unwrap_or(rest);
                    let fields: Vec<String> =
                        body.splitn(format.len(), ',').map(str::to_string).collect();
                    if fields.len() == format.len()
                        && t == format.len() - 1
                        && let (Some(start), Some(end)) = (ass_ms(&fields[s]), ass_ms(&fields[e]))
                    {
                        let style = at("style")
                            .and_then(|i| fields.get(i))
                            .map(|s| s.trim().to_string())
                            .unwrap_or_default();
                        self.items.push(Item::Cue(
                            Cue {
                                start,
                                end,
                                text: fields[t].clone(),
                            },
                            Shape::Ass {
                                kind: key.trim().to_string(),
                                fields,
                                start: s,
                                end: e,
                                text: t,
                                style,
                            },
                        ));
                        continue;
                    }
                }
            }
            self.items.push(Item::Raw(line.to_string()));
        }
        // The split leaves an empty piece after the last line end.
        if matches!(self.items.last(), Some(Item::Raw(l)) if l.is_empty()) {
            self.items.pop();
        }
    }

    fn read_microdvd(&mut self, text: &str, fps: Option<f64>) {
        let mut own = None;
        for line in text.split('\n') {
            let Some(c) = MDVD_LINE.captures(line.trim_end()) else {
                self.items.push(Item::Raw(line.to_string()));
                continue;
            };
            let (a, b) = (&c[1], &c[2]);
            // `{1}{1}23.976`: the frame rate, not a cue.
            if own.is_none()
                && self.items.iter().all(|i| matches!(i, Item::Raw(_)))
                && (a == b || b.is_empty())
                && let Ok(f) = c[3].trim().replace(',', ".").parse::<f64>()
                && (1.0..=200.0).contains(&f)
            {
                own = Some(f);
                self.items.push(Item::Raw(line.to_string()));
                continue;
            }
            let start: i64 = a.parse().unwrap_or(0);
            let end: i64 = b.parse().unwrap_or(-1);
            // Frames for now: made times below, once the rate is known.
            self.items.push(Item::Cue(
                Cue {
                    start,
                    end,
                    text: c[3].to_string(),
                },
                Shape::MicroDvd,
            ));
        }
        if matches!(self.items.last(), Some(Item::Raw(l)) if l.is_empty()) {
            self.items.pop();
        }
        self.fps = own.or(fps);
        let Some(fps) = self.fps else { return };
        for it in &mut self.items {
            if let Item::Cue(c, _) = it {
                let start = frames_to_ms(c.start, fps);
                c.end = if c.end < 0 {
                    start + 3000
                } else {
                    frames_to_ms(c.end, fps)
                };
                c.start = start;
            }
        }
    }

    fn read_subviewer(&mut self, text: &str) {
        let lines: Vec<&str> = text.split('\n').collect();
        let mut i = 0;
        while i < lines.len() {
            if let Some(c) = SV_TIMING.captures(lines[i]) {
                let start = to_ms(&c[1], &c[2], &c[3], &c[4]).unwrap_or(0);
                let end = to_ms(&c[5], &c[6], &c[7], &c[8]).unwrap_or(start);
                let mut body = Vec::new();
                i += 1;
                while i < lines.len() && !lines[i].trim().is_empty() {
                    body.push(lines[i].trim_end());
                    i += 1;
                }
                self.items.push(Item::Cue(
                    Cue {
                        start,
                        end,
                        text: body.join("[br]"),
                    },
                    Shape::SubViewer,
                ));
                // The blank line after it is written with it.
                i += 1;
                continue;
            }
            self.items.push(Item::Raw(lines[i].to_string()));
            i += 1;
        }
        if matches!(self.items.last(), Some(Item::Raw(l)) if l.is_empty()) {
            self.items.pop();
        }
    }

    fn read_sami(&mut self, text: &str) {
        static SYNC: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r#"(?is)<sync\s[^>]*?start\s*=\s*"?(\d+)[^>]*>"#).unwrap());
        static P: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r#"(?is)<p(?:\s[^>]*?class\s*=\s*"?([\w-]+))?[^>]*>"#).unwrap()
        });
        let syncs: Vec<(usize, usize, i64)> = SYNC
            .captures_iter(text)
            .map(|c| {
                let m = c.get(0).unwrap();
                (m.start(), m.end(), c[1].parse().unwrap_or(0))
            })
            .collect();
        let body_end = text
            .to_ascii_lowercase()
            .rfind("</body")
            .unwrap_or(text.len());
        // (time, class, text) of every paragraph.
        let mut paras: Vec<(i64, String, String)> = Vec::new();
        for (k, &(_, end, time)) in syncs.iter().enumerate() {
            let stop = syncs.get(k + 1).map_or(body_end, |s| s.0).max(end);
            let chunk = &text[end..stop];
            let ps: Vec<_> = P.captures_iter(chunk).collect();
            if ps.is_empty() {
                paras.push((time, String::new(), sami_text(chunk)));
            }
            for (j, c) in ps.iter().enumerate() {
                let m = c.get(0).unwrap();
                let to = ps.get(j + 1).map_or(chunk.len(), |n| n.get(0).unwrap().start());
                let class = c.get(1).map_or("", |m| m.as_str()).to_ascii_lowercase();
                paras.push((time, class, sami_text(&chunk[m.end()..to])));
            }
        }
        // Several languages: the first one with text.
        let class = paras
            .iter()
            .find(|p| !p.2.is_empty())
            .map(|p| p.1.clone())
            .unwrap_or_default();
        let mine: Vec<&(i64, String, String)> = paras.iter().filter(|p| p.1 == class).collect();
        for (k, p) in mine.iter().enumerate() {
            if p.2.is_empty() {
                continue;
            }
            let end = mine
                .get(k + 1)
                .map(|n| n.0)
                .filter(|&e| e > p.0)
                .unwrap_or(p.0 + 4000);
            self.items.push(Item::Cue(
                Cue {
                    start: p.0,
                    end,
                    text: p.2.clone(),
                },
                Shape::Sami,
            ));
        }
    }

    // ---------------------------------------------------------- edits

    fn cues_mut(&mut self) -> impl Iterator<Item = (&mut Cue, &Shape)> {
        self.items.iter_mut().filter_map(|i| match i {
            Item::Cue(c, s) => Some((c, &*s)),
            Item::Raw(_) => None,
        })
    }

    /// Moves every cue: times scaled by `scale` (a subtitle timed for
    /// 23.976 frames per second shown at 25: 23.976 / 25), then `shift_ms`
    /// added. Cues that end up before the start are dropped.
    pub fn retime(&mut self, shift_ms: i64, scale: f64) {
        let fix = |t: i64| ((t as f64) * scale).round() as i64 + shift_ms;
        let vtt = self.format == Format::Vtt;
        for (c, _) in self.cues_mut() {
            c.start = fix(c.start);
            c.end = fix(c.end);
            if vtt && c.text.contains('<') {
                c.text = VTT_INLINE
                    .replace_all(&c.text, |m: &regex::Captures| {
                        let get = |i: usize| m.get(i).map_or("", |m| m.as_str());
                        let t = to_ms(get(1), get(2), get(3), get(4)).unwrap_or(0);
                        format!("<{}>", vtt_time(fix(t)))
                    })
                    .into_owned();
            }
        }
        self.items.retain(|i| match i {
            Item::Cue(c, _) => c.end > 0,
            Item::Raw(_) => true,
        });
        for (c, _) in self.cues_mut() {
            c.start = c.start.max(0);
        }
    }

    /// Takes out the lines that are credits or advertisements; the lines
    /// taken out.
    pub fn drop_ads(&mut self) -> usize {
        let marks = self.ad_marks();
        let format = self.format;
        let mut dropped = 0;
        let mut at = 0;
        let mut emptied = Vec::new();
        for it in &mut self.items {
            let Item::Cue(c, shape) = it else { continue };
            if is_comment(shape) {
                continue;
            }
            let lines = split_lines(format, &c.text);
            let kept: Vec<String> = lines
                .iter()
                .filter(|l| !marks.get(at).is_some_and(|m| m.contains(&visible(format, l))))
                .cloned()
                .collect();
            dropped += lines.len() - kept.len();
            if kept.len() != lines.len() {
                c.text = join_lines(format, &kept);
                emptied.push(at);
            }
            at += 1;
        }
        self.drop_emptied(&emptied);
        dropped
    }

    /// For each cue, the visible text of its lines that are advertisements.
    fn ad_marks(&self) -> Vec<Vec<String>> {
        let cues: Vec<&Cue> = self.cues().collect();
        let n = cues.len();
        let last = cues.iter().map(|c| c.start).max().unwrap_or(0);
        cues.iter()
            .enumerate()
            .map(|(i, c)| {
                // Credits stand at the start and the end of a film.
                let edge = i < 6 || i + 6 >= n || c.start < 120_000 || c.start + 180_000 > last;
                let lines: Vec<String> = split_lines(self.format, &c.text)
                    .iter()
                    .map(|l| visible(self.format, l))
                    .collect();
                // `Legendas:` over the names: the whole cue.
                if edge && lines.iter().any(|l| CREDIT_HEAD.is_match(l.trim())) {
                    return lines;
                }
                lines.into_iter().filter(|v| is_ad(v, edge)).collect()
            })
            .collect()
    }

    /// Takes out what is there for the hard of hearing: sounds in
    /// brackets (and in parentheses when written in capitals or standing
    /// alone), speakers' names in capitals before a colon, lines of only
    /// music notes. The cues changed.
    pub fn drop_hearing(&mut self) -> usize {
        let format = self.format;
        let caps = self.mostly_capitals();
        let mut changed = 0;
        let mut at = 0;
        let mut emptied = Vec::new();
        for it in &mut self.items {
            let Item::Cue(c, shape) = it else { continue };
            if is_comment(shape) {
                continue;
            }
            if let Some(t) = clean_hearing(format, &c.text, caps) {
                c.text = t;
                changed += 1;
                emptied.push(at);
            }
            at += 1;
        }
        self.drop_emptied(&emptied);
        changed
    }

    /// Subtitles written all in capitals: there, capitals do not tell a
    /// speaker's name or a sound apart.
    fn mostly_capitals(&self) -> bool {
        let (mut up, mut all) = (0usize, 0usize);
        for c in self.cues() {
            for ch in visible(self.format, &c.text).chars().filter(|c| c.is_alphabetic()) {
                all += 1;
                if ch.is_uppercase() {
                    up += 1;
                }
            }
        }
        all > 0 && up * 10 >= all * 8
    }

    /// Takes out the cues (numbered as [`cues`](Self::cues) counts them)
    /// in `edited` that were left with nothing to show; cues that were
    /// empty already stay.
    fn drop_emptied(&mut self, edited: &[usize]) {
        let format = self.format;
        let mut at = 0;
        self.items.retain(|i| match i {
            Item::Cue(_, s) if is_comment(s) => true,
            Item::Cue(c, _) => {
                let n = at;
                at += 1;
                !(edited.binary_search(&n).is_ok() && visible(format, &c.text).trim().is_empty())
            }
            Item::Raw(_) => true,
        });
    }

    // --------------------------------------------------------- writing

    /// The file again, in its own format.
    pub fn write(&self) -> String {
        let nl = self.nl;
        let mut out = String::new();
        match self.format {
            Format::Srt => {
                let mut n = 0;
                for it in &self.items {
                    match it {
                        Item::Raw(r) => out.push_str(&r.replace('\n', nl)),
                        Item::Cue(c, Shape::Srt { extra }) => {
                            n += 1;
                            push_srt(&mut out, n, c, extra, nl);
                        }
                        Item::Cue(..) => {}
                    }
                }
            }
            Format::Vtt => {
                let blocks: Vec<String> = self
                    .items
                    .iter()
                    .map(|it| match it {
                        Item::Raw(r) => r.clone(),
                        Item::Cue(c, Shape::Vtt { id, settings }) => {
                            let mut b = String::new();
                            if let Some(id) = id {
                                b.push_str(id);
                                b.push('\n');
                            }
                            b.push_str(&vtt_time(c.start));
                            b.push_str(" --> ");
                            b.push_str(&vtt_time(c.end));
                            if !settings.is_empty() {
                                b.push(' ');
                                b.push_str(settings);
                            }
                            b.push('\n');
                            b.push_str(&c.text);
                            b
                        }
                        Item::Cue(c, _) => c.text.clone(),
                    })
                    .collect();
                out = blocks.join("\n\n").replace('\n', nl);
                out.push_str(nl);
            }
            Format::Ass | Format::MicroDvd | Format::SubViewer => {
                for it in &self.items {
                    match it {
                        Item::Raw(r) => out.push_str(r),
                        Item::Cue(
                            c,
                            Shape::Ass {
                                kind,
                                fields,
                                start,
                                end,
                                text,
                                ..
                            },
                        ) => {
                            let mut f = fields.clone();
                            f[*start] = ass_time(c.start);
                            f[*end] = ass_time(c.end);
                            f[*text] = c.text.clone();
                            out.push_str(kind);
                            out.push_str(": ");
                            out.push_str(&f.join(","));
                        }
                        Item::Cue(c, Shape::MicroDvd) => {
                            // Without a rate the times are still frames.
                            let (a, b) = match self.fps {
                                Some(fps) => (ms_to_frames(c.start, fps), ms_to_frames(c.end, fps)),
                                None => (c.start, c.end),
                            };
                            let b = if b < 0 { String::new() } else { b.to_string() };
                            out.push_str(&format!("{{{a}}}{{{b}}}{}", c.text));
                        }
                        Item::Cue(c, _) => {
                            out.push_str(&format!("{},{}", sv_time(c.start), sv_time(c.end)));
                            out.push_str(nl);
                            out.push_str(&c.text);
                        }
                    }
                    out.push_str(nl);
                    // SubViewer's cues end with a blank line, but the last.
                    if self.format == Format::SubViewer
                        && matches!(it, Item::Cue(..))
                        && !std::ptr::eq(it, self.items.last().unwrap())
                    {
                        out.push_str(nl);
                    }
                }
            }
            // Not written in its own format: made into SubRip.
            Format::Sami => return self.to_srt(),
        }
        out
    }

    /// The cues as SubRip, in time order: styles that SubRip has
    /// (italics, bold, underline, a line on top) kept, the rest left out.
    pub fn to_srt(&self) -> String {
        let nl = "\r\n";
        let mut cues: Vec<Cue> = Vec::new();
        for it in &self.items {
            let Item::Cue(c, shape) = it else { continue };
            if is_comment(shape) {
                continue;
            }
            let text = match shape {
                Shape::Srt { .. } => c.text.clone(),
                Shape::Vtt { settings, .. } => vtt_to_srt(&c.text, settings),
                Shape::Ass { style, .. } => {
                    ass_to_srt(&c.text, self.italic_styles.iter().any(|s| s == style))
                }
                Shape::MicroDvd => microdvd_to_srt(&c.text),
                Shape::SubViewer => split_lines(Format::SubViewer, &c.text).join("\n"),
                Shape::Sami => c.text.clone(),
            };
            let text = text
                .split('\n')
                .map(str::trim_end)
                .filter(|l| !l.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if strip_srt_tags(&text).trim().is_empty() {
                continue;
            }
            let cue = Cue {
                start: c.start,
                end: c.end.max(c.start),
                text,
            };
            // The same line drawn twice (a shadow on another layer).
            if !cues.iter().rev().take(8).any(|x| *x == cue) {
                cues.push(cue);
            }
        }
        cues.sort_by_key(|c| c.start);
        let mut out = String::new();
        for (n, c) in cues.iter().enumerate() {
            push_srt(&mut out, n + 1, c, "", nl);
        }
        out
    }
}

fn is_number(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())
}

fn is_comment(shape: &Shape) -> bool {
    matches!(shape, Shape::Ass { kind, .. } if !kind.eq_ignore_ascii_case("dialogue"))
}

fn frames_to_ms(frames: i64, fps: f64) -> i64 {
    ((frames as f64) * 1000.0 / fps).round() as i64
}

fn ms_to_frames(ms: i64, fps: f64) -> i64 {
    ((ms as f64) * fps / 1000.0).round() as i64
}

fn push_srt(out: &mut String, n: usize, c: &Cue, extra: &str, nl: &str) {
    out.push_str(&n.to_string());
    out.push_str(nl);
    out.push_str(&srt_time(c.start));
    out.push_str(" --> ");
    out.push_str(&srt_time(c.end));
    if !extra.is_empty() {
        out.push(' ');
        out.push_str(extra.trim());
    }
    out.push_str(nl);
    out.push_str(&c.text.replace('\n', nl));
    out.push_str(nl);
    out.push_str(nl);
}

// ----------------------------------------------------------------- markup

fn split_lines(format: Format, text: &str) -> Vec<String> {
    let parts: Vec<&str> = match format {
        Format::Srt | Format::Vtt | Format::Sami => text.split('\n').collect(),
        Format::Ass => text.split("\\N").collect(),
        Format::MicroDvd => text.split('|').collect(),
        Format::SubViewer => return split_ci(text, "[br]"),
    };
    parts.into_iter().map(str::to_string).collect()
}

fn split_ci(text: &str, sep: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find(sep) {
        out.push(text[from..from + i].to_string());
        from += i + sep.len();
    }
    out.push(text[from..].to_string());
    out
}

fn join_lines(format: Format, lines: &[String]) -> String {
    let sep = match format {
        Format::Srt | Format::Vtt | Format::Sami => "\n",
        Format::Ass => "\\N",
        Format::MicroDvd => "|",
        Format::SubViewer => "[br]",
    };
    lines.join(sep)
}

/// Where the markup is in a line: byte ranges of tags.
fn tag_ranges(format: Format, line: &str) -> Vec<(usize, usize)> {
    static ANGLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^<>]*>").unwrap());
    static CURLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{[^{}]*\}").unwrap());
    static SRT_CURLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\\[^{}]*\}").unwrap());
    let mut out: Vec<(usize, usize)> = Vec::new();
    match format {
        Format::Srt | Format::Sami => {
            out.extend(ANGLE.find_iter(line).map(|m| (m.start(), m.end())));
            out.extend(SRT_CURLY.find_iter(line).map(|m| (m.start(), m.end())));
        }
        Format::Vtt => out.extend(ANGLE.find_iter(line).map(|m| (m.start(), m.end()))),
        Format::Ass | Format::MicroDvd => {
            out.extend(CURLY.find_iter(line).map(|m| (m.start(), m.end())))
        }
        Format::SubViewer => {}
    }
    out.sort_unstable();
    out
}

/// A line's text as shown (markup left out, entities read).
fn visible(format: Format, line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut at = 0;
    for (s, e) in tag_ranges(format, line) {
        if s >= at {
            out.push_str(&line[at..s]);
            at = e;
        }
    }
    out.push_str(&line[at..]);
    match format {
        Format::Ass => out.replace("\\h", " ").replace("\\n", " "),
        Format::Vtt | Format::Srt | Format::Sami => entities(&out),
        _ => out,
    }
}

fn entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&nbsp;", "\u{a0}")
        .replace("&lrm;", "\u{200e}")
        .replace("&rlm;", "\u{200f}")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

static URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:https?://|www\.)\S+|\b[a-z0-9][a-z0-9-]*\.(?:com|net|org|info|tv|io|me|cc|ws|biz|to|xyz)(?:\.[a-z]{2})?\b|\b[a-z0-9][a-z0-9-]*\.com?\.br\b",
    )
    .unwrap()
});

/// Who made, timed or translated the subtitle (`Subtitles by…`,
/// `Legendas por…`, `Tradução de…`): the word, then at most two more,
/// then who.
static CREDIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:subtitles?|subtitled|subs|captions?|captioned|captioning|synced|sync|synchroni[sz]ed|resynced?|corrected|corrections?|encoded|ripped|translated|translation|transcript|legendas?|legendad[oa]s?|legendagem|tradu[çc](?:[ãa]o|[õo]es)|traduzid[oa]s?|revis[ãa]o|revisad[oa]|sincronia|sincroni[az]a[çc][ãa]o|sincronizad[oa]s?|subt[ií]tulos?|subtitulad[oa]s?|traducci[óo]n|traducid[oa]s?)(?:\s+(?:[\w&]+|&)){0,2}?\s*(?:\b(?:by|por|from)\b|:)|\b(?:legendas?|tradu[çc][ãa]o|traducci[óo]n|subt[ií]tulos)\s+de\b",
    )
    .unwrap()
});

/// Words that make a web address an advertisement (`Visit…`, `Acesse…`).
static AD_WORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:visit[ea]?|acesse|acessem|baixe|baixar|download|downloads|site|subs?|legendas?|subt[ií]tulos|torrents?|follow|siga|sigam|twitter|facebook|instagram|telegram|discord)\b",
    )
    .unwrap()
});

/// A credit's heading alone on its line, the names under it.
static CREDIT_HEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^[\W_]*(?:subtitles?|subtitled\s+by|captions?|synced\s+by|sync|translation|translated\s+by|legendas?|legendado\s+por|tradu[çc][ãa]o|revis[ãa]o|sincronia|sincroniza[çc][ãa]o|subt[ií]tulos?|traducci[óo]n|ajustes?)\s*:[\W_]*$",
    )
    .unwrap()
});

/// Subtitle sites and their notices, anywhere in the film.
static SERVICE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)opensubtitles|open\s*subtitles|subdl\b|addic7ed|podnapisi|subscene|yifysubtitles|become\s+(?:a\s+)?vip\s+member|advertise\s+your\s+product|rate\s+this\s+subtitle|remove\s+all\s+ads",
    )
    .unwrap()
});

/// A line that is an advertisement or a credit: a subtitle site's notice
/// anywhere; a web address or a credit only near the start or the end
/// (`edge`), where they stand, so a film that speaks of a site keeps it.
fn is_ad(line: &str, edge: bool) -> bool {
    let line = line.trim();
    if line.is_empty() {
        return false;
    }
    if SERVICE.is_match(line) {
        return true;
    }
    if !edge || line.chars().count() > 90 {
        return false;
    }
    if CREDIT.is_match(line) {
        return true;
    }
    // A web address alone (or nearly), or with words that sell it; a
    // sentence that speaks of a site stays.
    URL.is_match(line) && {
        let rest = URL.replace_all(line, " ");
        let words = rest.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count();
        words <= 2 || AD_WORDS.is_match(&rest)
    }
}

/// `line` with the visible characters in `cut` (byte ranges of the
/// visible text) taken out; markup is kept.
fn cut_visible(format: Format, line: &str, cut: &[(usize, usize)]) -> String {
    if cut.is_empty() {
        return line.to_string();
    }
    let tags = tag_ranges(format, line);
    let mut out = String::with_capacity(line.len());
    // The position in the visible text as the line is walked (markup
    // takes none; entities are cut whole with what they stand for).
    let mut vis = 0usize;
    let mut i = 0usize;
    let mut t = 0usize;
    while i < line.len() {
        if t < tags.len() && tags[t].0 == i {
            out.push_str(&line[tags[t].0..tags[t].1]);
            i = tags[t].1;
            t += 1;
            continue;
        }
        let ch = line[i..].chars().next().unwrap();
        let len = ch.len_utf8();
        let inside = cut.iter().any(|&(s, e)| vis >= s && vis < e);
        if !inside {
            out.push(ch);
        }
        vis += len;
        i += len;
    }
    out
}

/// The visible text of a line where `cut_visible`'s ranges refer: tags
/// taken out, nothing else changed (so byte positions match).
fn plain(format: Format, line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut at = 0;
    for (s, e) in tag_ranges(format, line) {
        if s >= at {
            out.push_str(&line[at..s]);
            at = e;
        }
    }
    out.push_str(&line[at..]);
    out
}

static BRACKETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\[\]]*\]|［[^［］]*］").unwrap());
static PARENS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(([^()]*)\)").unwrap());
static SPEAKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\s*[-–—]?\s*)([\p{Lu}][\p{Lu}\p{Nd} .'’&#-]{0,30}?)\s*:(?:\s|$)").unwrap()
});
static EMPTY_TAGS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<(i|b|u)>\s*</(?:i|b|u)>|<font[^>]*>\s*</font>|\{\\[ibu]1\}\s*\{\\[ibu]0\}")
        .unwrap()
});

/// A cue's text without what is there for the hard of hearing; `None`
/// when nothing is. `caps`: the subtitles are all in capitals.
fn clean_hearing(format: Format, text: &str, caps: bool) -> Option<String> {
    let lines = split_lines(format, text);
    let mut out: Vec<String> = Vec::new();
    let mut changed = false;
    let dashed = lines
        .iter()
        .filter(|l| visible(format, l).trim_start().starts_with(['-', '–', '—']))
        .count();
    for line in &lines {
        let p = plain(format, line);
        let mut cut: Vec<(usize, usize)> = BRACKETS
            .find_iter(&p)
            .map(|m| (m.start(), m.end()))
            .collect();
        let core = p.trim().trim_start_matches(['-', '–', '—']).trim();
        let core_at = p.len() - p.trim_start().trim_start_matches(['-', '–', '—']).trim_start().len();
        for c in PARENS.captures_iter(&p) {
            let m = c.get(0).unwrap();
            let inner = c[1].trim();
            let words = inner.split_whitespace().count();
            let capital = inner.chars().next().is_some_and(char::is_uppercase);
            // Alone on its line and short (a sound: longer is speech, a
            // song's second voice), or before what is said and naming a
            // sound or a speaker (`(Sighs)`, `(Man on radio)`); a remark in
            // parentheses (`(which is 2) and…`) stays.
            let alone = m.as_str() == core && words <= 4;
            let before = m.start() == core_at
                && m.as_str() != core
                && (words == 1 || (capital && words <= 3));
            let shouting = !caps
                && inner.chars().any(char::is_alphabetic)
                && !inner.chars().any(char::is_lowercase);
            if inner.chars().any(char::is_alphabetic) && (alone || before || shouting) {
                cut.push((m.start(), m.end()));
            }
        }
        if !caps && let Some(c) = SPEAKER.captures(&p) {
            let name = c.get(2).unwrap();
            let m = c.get(0).unwrap();
            // A name before what is said; capitals and a colon over
            // capitals (`WANTED: OGRES`) or alone are a sign's words.
            let said = p[m.end()..].chars().any(char::is_lowercase);
            if said && name.as_str().chars().filter(|c| c.is_alphabetic()).count() >= 2 {
                // The dash before the name stays.
                cut.push((name.start(), m.end()));
            }
        }
        // Only music notes (and spaces, dashes): the line goes.
        let notes = core.chars().any(|c| matches!(c, '♪' | '♫' | '♬' | '♩'))
            && core
                .chars()
                .all(|c| matches!(c, '♪' | '♫' | '♬' | '♩' | '#' | '*' | '-' | '–') || c.is_whitespace());
        if notes {
            changed = true;
            continue;
        }
        if cut.is_empty() {
            out.push(line.clone());
            continue;
        }
        changed = true;
        let cut = widen(&p, cut);
        let cleaned = tidy(&cut_visible(format, line, &cut));
        let v = visible(format, &cleaned);
        let left = v.trim().trim_matches(['-', '–', '—', ':', '.', ',', ' ']);
        if !left.is_empty() {
            out.push(cleaned);
        }
    }
    if !changed {
        return None;
    }
    // One line left of a dialogue: its dash goes too.
    if out.len() == 1 && dashed >= 2 {
        let v = plain(format, &out[0]);
        if let Some(pos) = v.find(|c: char| !c.is_whitespace())
            && v[pos..].starts_with(['-', '–', '—'])
        {
            let dash = v[pos..].chars().next().unwrap().len_utf8();
            let spaces = v[pos + dash..].len() - v[pos + dash..].trim_start().len();
            out[0] = cut_visible(format, &out[0], &[(pos, pos + dash + spaces)]);
        }
    }
    Some(join_lines(format, &out))
}

/// Cuts taking the spaces after them (or, at the line's end, before
/// them) so no gap is left.
fn widen(p: &str, mut cut: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    for (s, e) in &mut cut {
        let after = p[*e..].len() - p[*e..].trim_start().len();
        if after > 0 && *e + after < p.len() {
            *e += after;
        } else if *e + after >= p.len() {
            *e = p.len();
            *s -= p[..*s].len() - p[..*s].trim_end().len();
        }
    }
    cut
}

/// Spaces left by what was cut, and empty style marks.
fn tidy(line: &str) -> String {
    static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]{2,}").unwrap());
    static DASH_GAP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*[-–—])\s{2,}").unwrap());
    let mut s = line.to_string();
    for _ in 0..3 {
        let next = EMPTY_TAGS.replace_all(&s, "").into_owned();
        if next == s {
            break;
        }
        s = next;
    }
    let s = SPACES.replace_all(&s, " ");
    let s = DASH_GAP.replace(&s, "$1 ");
    s.trim().to_string()
}

fn strip_srt_tags(text: &str) -> String {
    visible(Format::Srt, text)
}

/// WebVTT's markup as SubRip's: italics, bold and underline kept; voices,
/// classes, timestamps and ruby text left out; a cue placed on top gets
/// `{\an8}`.
fn vtt_to_srt(text: &str, settings: &str) -> String {
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<(/?)([a-zA-Z]+)?[^<>]*>").unwrap());
    static RT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<rt>.*?</rt>").unwrap());
    let t = RT.replace_all(text, "");
    let t = TAG.replace_all(&t, |c: &regex::Captures| {
        let close = &c[1];
        match c.get(2).map(|m| m.as_str().to_ascii_lowercase()).as_deref() {
            Some(k @ ("i" | "b" | "u")) => format!("<{close}{k}>"),
            _ => String::new(),
        }
    });
    let t = entities(&t);
    let top = settings.split_whitespace().any(|s| {
        s.strip_prefix("line:").is_some_and(|v| {
            let v = v.split(',').next().unwrap_or("");
            match v.strip_suffix('%') {
                Some(p) => p.parse::<f64>().is_ok_and(|p| p < 40.0),
                None => v.parse::<i64>().is_ok_and(|n| (0..=3).contains(&n)),
            }
        })
    });
    if top {
        format!("{{\\an8}}{t}")
    } else {
        t
    }
}

/// SubStation's text as SubRip's: line breaks, hard spaces, italics, bold,
/// underline and placement on top kept; drawings and other overrides left
/// out. `italic`: its style is in italics.
fn ass_to_srt(text: &str, italic: bool) -> String {
    static BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([^{}]*)\}").unwrap());
    static OVERRIDE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\\(an|a|i|b|u|p|r)(\d*)([a-zA-Z]*)").unwrap());
    let mut out = String::new();
    let (mut i_on, mut b_on, mut u_on) = (italic, false, false);
    let mut drawing = false;
    let mut top = false;
    if italic {
        out.push_str("<i>");
    }
    let mut at = 0;
    let mut pieces: Vec<(usize, usize, String)> = BLOCK
        .captures_iter(text)
        .map(|c| {
            let m = c.get(0).unwrap();
            (m.start(), m.end(), c[1].to_string())
        })
        .collect();
    pieces.push((text.len(), text.len(), String::new()));
    for (s, e, codes) in pieces {
        if !drawing {
            let piece = &text[at..s];
            out.push_str(
                &piece
                    .replace("\\N", "\n")
                    .replace("\\n", " ")
                    .replace("\\h", "\u{a0}"),
            );
        }
        for c in OVERRIDE.captures_iter(&codes) {
            // `\iclip`, `\bord`, `\blur`: other overrides.
            if !c[3].is_empty() && &c[1] != "r" {
                continue;
            }
            let n: i64 = c[2].parse().unwrap_or(0);
            let mut flip = |on: &mut bool, tag: &str, want: bool| {
                if *on != want {
                    out.push_str(&format!("<{}{tag}>", if want { "" } else { "/" }));
                    *on = want;
                }
            };
            match &c[1] {
                "i" => flip(&mut i_on, "i", n != 0),
                "b" => flip(&mut b_on, "b", n != 0),
                "u" => flip(&mut u_on, "u", n != 0),
                "an" => top = (7..=9).contains(&n),
                // The old numbering: 5 to 7 and 9 to 11 are on top.
                "a" => top = (5..=11).contains(&n) && n != 8,
                "p" => drawing = n != 0,
                "r" => {
                    flip(&mut b_on, "b", false);
                    flip(&mut u_on, "u", false);
                    flip(&mut i_on, "i", italic);
                }
                _ => {}
            }
        }
        at = e;
    }
    for (on, tag) in [(u_on, "u"), (b_on, "b"), (i_on, "i")] {
        if on {
            out.push_str(&format!("</{tag}>"));
        }
    }
    let out = tidy_lines(&out);
    if top && !out.trim().is_empty() {
        format!("{{\\an8}}{out}")
    } else {
        out
    }
}

fn tidy_lines(text: &str) -> String {
    text.split('\n')
        .map(tidy)
        .collect::<Vec<_>>()
        .join("\n")
}

/// MicroDVD's text as SubRip's: `|` breaks lines, `{y:i}` (that line) and
/// `{Y:i}` (all its lines) make italics, bold and underline; colours and
/// fonts are left out.
fn microdvd_to_srt(text: &str) -> String {
    static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([a-zA-Z]):([^{}]*)\}").unwrap());
    let mut all: Vec<char> = Vec::new();
    let lines: Vec<String> = text.split('|').map(str::to_string).collect();
    let mut styled: Vec<(Vec<char>, String)> = Vec::new();
    for line in &lines {
        let mut mine = Vec::new();
        for c in CODE.captures_iter(line) {
            if c[1].eq_ignore_ascii_case("y") {
                let styles: Vec<char> = c[2]
                    .chars()
                    .map(|c| c.to_ascii_lowercase())
                    .filter(|c| matches!(c, 'i' | 'b' | 'u'))
                    .collect();
                if &c[1] == "Y" {
                    all.extend(styles);
                } else {
                    mine.extend(styles);
                }
            }
        }
        styled.push((mine, CODE.replace_all(line, "").trim().to_string()));
    }
    styled
        .into_iter()
        .map(|(mine, t)| {
            let mut s: Vec<char> = all.iter().chain(mine.iter()).copied().collect();
            s.dedup();
            let open: String = s.iter().map(|c| format!("<{c}>")).collect();
            let close: String = s.iter().rev().map(|c| format!("</{c}>")).collect();
            if t.is_empty() {
                t
            } else {
                format!("{open}{t}{close}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A SAMI paragraph's text: line breaks kept, italics, bold and underline
/// kept, other markup left out, entities read; `&nbsp;` alone is empty
/// (it clears the screen).
fn sami_text(html: &str) -> String {
    static BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").unwrap());
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<(/?)([a-zA-Z]+)[^<>]*>").unwrap());
    static WS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t\r\n]+").unwrap());
    let t = WS.replace_all(html, " ");
    let t = BR.replace_all(&t, "\n");
    let t = TAG.replace_all(&t, |c: &regex::Captures| {
        match c[2].to_ascii_lowercase().as_str() {
            k @ ("i" | "b" | "u") => format!("<{}{k}>", &c[1]),
            _ => String::new(),
        }
    });
    let t = entities(&t);
    let t = t
        .split('\n')
        .map(|l| l.trim_matches(|c: char| c.is_whitespace()))
        .collect::<Vec<_>>()
        .join("\n");
    t.trim().to_string()
}

// --------------------------------------------------------------- the rule

/// What the subtitle rule does to subtitles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tools {
    /// Other text formats are made into SubRip (`.srt`).
    pub to_srt: bool,
    /// Old encodings are written as UTF-8.
    pub utf8: bool,
    /// Credits and advertisements are taken out.
    pub ads: bool,
    /// Marks for the hard of hearing are taken out.
    pub hearing: bool,
    /// Milliseconds added to every time (negative: earlier).
    pub shift_ms: i64,
    /// The frame rate the subtitle was timed for and the video's: times
    /// scaled from one to the other (0: not).
    pub fps_from: f64,
    pub fps_to: f64,
}

impl Tools {
    fn scale(&self) -> f64 {
        if self.fps_from > 0.0 && self.fps_to > 0.0 && (self.fps_from - self.fps_to).abs() > 1e-6 {
            self.fps_from / self.fps_to
        } else {
            1.0
        }
    }

    fn retimes(&self) -> bool {
        self.shift_ms != 0 || self.scale() != 1.0
    }
}

/// What a subtitle file is, read once for the preview.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub format: Option<Format>,
    /// Pictures, not text.
    pub picture: bool,
    /// The encoding it is written in (`windows-1252`).
    pub encoding: &'static str,
    pub utf8: bool,
    pub cues: usize,
    /// Lines that are credits or advertisements.
    pub ads: usize,
    /// Cues with marks for the hard of hearing.
    pub hearing: usize,
    /// MicroDVD without a frame rate (in it or from the video beside).
    pub no_fps: bool,
}

/// What the rule does to one subtitle, shown in the preview.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    /// Made into SubRip (named `.srt` already).
    pub convert: bool,
    /// Written as UTF-8, read from this encoding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<&'static str>,
    pub ads: usize,
    pub hearing: usize,
    pub retime: bool,
    /// MicroDVD whose times cannot be known: left as it is.
    pub no_fps: bool,
}

impl Fix {
    pub fn is_empty(&self) -> bool {
        !self.convert && self.encoding.is_none() && self.ads == 0 && self.hearing == 0 && !self.retime
    }
}

/// The frame rate of the video a subtitle belongs to: the one beside it
/// whose name begins its name.
fn video_fps(sub: &Path) -> Option<f64> {
    let dir = sub.parent()?;
    let stem = sub.file_stem()?.to_string_lossy().to_lowercase();
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| crate::tmdb::is_video(&e.to_string_lossy()))
                && p.file_stem()
                    .is_some_and(|s| stem.starts_with(&s.to_string_lossy().to_lowercase()))
        })
        .collect();
    // The longest name is the closest.
    found.sort_by_key(|p| std::cmp::Reverse(p.as_os_str().len()));
    found.iter().find_map(|p| crate::video::frame_rate(p))
}

/// Reads a subtitle file: its text, encoding and document (`None` for a
/// format not known or pictures).
fn load(path: &Path) -> Result<(Vec<u8>, String, Enc, Option<Doc>), String> {
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_SIZE {
        return Err("subTooBig".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if is_picture(&bytes) {
        return Ok((bytes, String::new(), Enc::Utf8 { bom: false }, None));
    }
    let lang = lang_of_name(path);
    let (text, enc) = decode(&bytes, lang.as_deref());
    let doc = match detect(&text) {
        Some(Format::MicroDvd) => Doc::parse(&text, None).map(|d| {
            if d.needs_fps() {
                Doc::parse(&text, video_fps(path)).unwrap_or(d)
            } else {
                d
            }
        }),
        Some(_) => Doc::parse(&text, None),
        None => None,
    };
    Ok((bytes, text, enc, doc))
}

/// What a subtitle file is (for the preview).
pub fn summary(path: &Path) -> Option<Summary> {
    let (bytes, _, enc, doc) = load(path).ok()?;
    if is_picture(&bytes) {
        return Some(Summary {
            picture: true,
            ..Default::default()
        });
    }
    let mut s = Summary {
        encoding: enc.name(),
        utf8: enc.is_utf8() || bytes.is_ascii(),
        ..Default::default()
    };
    if let Some(doc) = doc {
        s.format = Some(doc.format);
        s.cues = doc.cues().count();
        s.no_fps = doc.needs_fps();
        s.ads = doc.ad_marks().iter().map(Vec::len).sum();
        let caps = doc.mostly_capitals();
        s.hearing = doc
            .cues()
            .filter(|c| clean_hearing(doc.format, &c.text, caps).is_some())
            .count();
    }
    Some(s)
}

/// What `tools` would do to a subtitle like `s`, now named with `ext`.
pub fn plan(s: &Summary, tools: &Tools, ext: &str) -> Option<Fix> {
    if s.picture || s.format.is_none() {
        return None;
    }
    let mut fix = Fix {
        encoding: (tools.utf8 && !s.utf8).then_some(s.encoding),
        ..Default::default()
    };
    if let Some(format) = s.format {
        fix.convert = tools.to_srt && format != Format::Srt && ext.eq_ignore_ascii_case("srt");
        // SAMI is only edited when made into SubRip.
        let editable = format != Format::Sami || fix.convert;
        if s.no_fps && (fix.convert || tools.retimes()) {
            return Some(Fix {
                no_fps: true,
                ..Default::default()
            });
        }
        if editable {
            fix.ads = if tools.ads { s.ads } else { 0 };
            fix.hearing = if tools.hearing { s.hearing } else { 0 };
            fix.retime = tools.retimes() && s.cues > 0;
        }
    }
    (!fix.is_empty()).then_some(fix)
}

/// Whether the subtitle rule makes this file (`ext`, its extension) a
/// SubRip: a text subtitle in another format whose times can be known.
pub fn converts(ext: &str, summary: Option<&Summary>) -> bool {
    !ext.eq_ignore_ascii_case("srt")
        && is_text_ext(ext)
        && summary.is_some_and(|s| s.format.is_some_and(|f| f != Format::Srt) && !s.no_fps)
}

/// The text `tools` make of a document (in its format, or SubRip).
fn edited(mut doc: Doc, tools: &Tools, to_srt: bool) -> String {
    let editable = doc.format != Format::Sami || to_srt;
    if editable {
        if tools.ads {
            doc.drop_ads();
        }
        if tools.hearing {
            doc.drop_hearing();
        }
        if tools.retimes() {
            doc.retime(tools.shift_ms, tools.scale());
        }
    }
    if to_srt { doc.to_srt() } else { doc.write() }
}

/// The text a subtitle file becomes: (`text`, encoding to write it in),
/// or `None` when nothing changes.
fn rewrite(
    (bytes, text, enc, doc): &(Vec<u8>, String, Enc, Option<Doc>),
    tools: &Tools,
    to_srt: bool,
) -> Result<Option<Vec<u8>>, String> {
    let (bytes, text, enc) = (bytes.as_slice(), text, *enc);
    // Pictures, or not a subtitle known (a `.sub` can be anything): not
    // touched, not even its encoding.
    if is_picture(bytes) || doc.is_none() {
        return Ok(None);
    }
    let out_enc = if tools.utf8 && !enc.is_utf8() {
        Enc::Utf8 { bom: false }
    } else {
        enc
    };
    let new_text = match doc.clone() {
        Some(d) if d.needs_fps() && (to_srt || tools.retimes()) => return Err("subNoFps".into()),
        Some(d) if to_srt || tools.ads || tools.hearing || tools.retimes() => {
            let format = d.format;
            let before = d.cues().count();
            let out = edited(d, tools, to_srt);
            // Only credits or advertisements: an empty file would be no
            // subtitle at all.
            if before > 0 && Doc::parse(&out, Some(25.0)).is_none_or(|d| d.cues().count() == 0) {
                return Err("subNothingLeft".into());
            }
            // Nothing edited: the file as it was (its own spacing kept).
            let same = || doc.as_ref().is_some_and(|d0| d0.write() == out);
            if !to_srt && format != Format::Sami && same() {
                text.clone()
            } else {
                out
            }
        }
        _ => text.clone(),
    };
    let out = out_enc.encode(&new_text);
    // Read as UTF-8 but not quite (a few bad bytes): written as it was,
    // rather than with replacement marks.
    if enc.is_utf8() && std::str::from_utf8(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)).is_err() {
        return Err("subEncoding".into());
    }
    Ok((out != bytes).then_some(out))
}

/// Writes `bytes` to a copy beside `path` that then takes its place.
fn replace_with(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write as _;
    let tmp = crate::tagwrite::temp_of(path);
    let written = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    if let Ok(m) = fs::metadata(path) {
        let _ = fs::set_permissions(&tmp, m.permissions());
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

/// A store file name for `path`'s old content, not taken yet.
fn store_name(store: &Path, path: &Path) -> String {
    let base = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "subtitle".into());
    let mut n = 0;
    loop {
        let name = if n == 0 {
            format!("sub-{base}")
        } else {
            format!("sub-{n}-{base}")
        };
        if fs::symlink_metadata(store.join(&name)).is_err() {
            return name;
        }
        n += 1;
    }
}

/// The outcome of [`fix_file`].
#[derive(Debug)]
pub struct Fixed {
    pub backup: Option<Backup>,
    /// The original was to go to the trash but could not: kept beside.
    pub not_trashed: Option<String>,
}

/// Applies `tools` to the subtitle at `path` (renamed already). One named
/// `.srt` but in another format is made into SubRip: the original steps
/// aside with its own extension (kept, or sent to the trash). Otherwise it
/// is edited where it is, its old content saved in `store` for undo.
pub fn fix_file(path: &Path, tools: &Tools, original: Original, store: &Path) -> Result<Fixed, String> {
    let loaded = load(path)?;
    let (bytes, text) = (&loaded.0, &loaded.1);
    let format = if is_picture(bytes) { None } else { detect(text) };
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let convert = tools.to_srt && ext == "srt" && format.is_some_and(|f| f != Format::Srt);
    let Some(out) = rewrite(&loaded, tools, convert)? else {
        return Ok(Fixed {
            backup: None,
            not_trashed: None,
        });
    };
    if convert {
        // SubStation's first version is `.ssa`.
        let lower = text.to_ascii_lowercase();
        let own = match format {
            Some(Format::Ass) if !lower.contains("v4.00+") && lower.contains("[v4 styles]") => "ssa",
            f => f.map_or("sub", Format::ext),
        };
        let left = path.with_extension(own);
        if fs::symlink_metadata(&left).is_ok() {
            return Err("convertTaken".into());
        }
        let tmp = crate::tagwrite::temp_of(path);
        if let Err(e) = fs::write(&tmp, &out).and_then(|_| crate::sync_file(&tmp)) {
            let _ = fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        let done = crate::bookconv::finish(path, &left, &tmp, original)?;
        return Ok(Fixed {
            backup: Some(done.backup),
            not_trashed: done.not_trashed,
        });
    }
    fs::create_dir_all(store).map_err(|e| e.to_string())?;
    let name = store_name(store, path);
    let saved = store.join(&name);
    fs::write(&saved, bytes)
        .and_then(|_| crate::sync_file(&saved))
        .map_err(|e| e.to_string())?;
    if let Err(e) = replace_with(path, &out) {
        let _ = fs::remove_file(&saved);
        return Err(e);
    }
    Ok(Fixed {
        backup: Some(Backup {
            saved: Some(name),
            ..Backup::empty(path)
        }),
        not_trashed: None,
    })
}

/// Puts back a file's old content saved whole in `store` (a subtitle
/// edited by [`fix_file`], a book rewritten by [`crate::bookmeta`]):
/// copied beside it, then taking its place.
pub fn restore(path: &Path, saved: &str, store: &Path) -> Result<(), String> {
    let tmp = crate::tagwrite::temp_of(path);
    let copied = fs::copy(store.join(saved), &tmp).and_then(|_| crate::sync_file(&tmp));
    if let Err(e) = copied {
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    if let Ok(m) = fs::metadata(path) {
        let _ = fs::set_permissions(&tmp, m.permissions());
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

/// The text of a subtitle made from `text` (SubRip, WebVTT or SubStation,
/// as taken out of a video) with `tools`' cleaning, in its own format or
/// SubRip. Times are not moved: a track inside a video is in time.
pub fn clean_text(text: &str, tools: &Tools) -> Option<String> {
    let doc = Doc::parse(text, None)?;
    let only = Tools {
        shift_ms: 0,
        fps_from: 0.0,
        fps_to: 0.0,
        ..*tools
    };
    Some(edited(doc, &only, tools.to_srt))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT: &str = "1\r\n00:00:01,000 --> 00:00:02,500\r\nHello there.\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000 X1:10 X2:20\r\n<i>[DOOR SLAMS]</i>\r\n\r\n3\r\n00:00:05,000 --> 00:00:06,000\r\nJOHN: Where were you?\r\n- (sighs) Out.\r\n\r\n";

    #[test]
    fn subrip_read_and_written_the_same() {
        let d = Doc::parse(SRT, None).unwrap();
        assert_eq!(d.format, Format::Srt);
        assert_eq!(d.cues().count(), 3);
        assert_eq!(d.write(), SRT);
    }

    #[test]
    fn subrip_forgiving() {
        let t = "\u{feff}1\n00:00:01.5 --> 00:00:02,000\nOne\n\n\n2\n0:00:03,000-->0:00:04,000\nTwo\n\nstray\n3\n00:00:05,000 --> 00:00:06,000\nThree";
        let d = Doc::parse(t, None).unwrap();
        let c: Vec<&Cue> = d.cues().collect();
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].start, 1500);
        assert_eq!(c[1].text, "Two\nstray");
        assert_eq!(c[2].text, "Three");
    }

    #[test]
    fn hearing_marks_out() {
        let mut d = Doc::parse(SRT, None).unwrap();
        assert_eq!(d.drop_hearing(), 2);
        let c: Vec<&Cue> = d.cues().collect();
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].text, "Where were you?\n- Out.");
        let out = d.write();
        assert!(out.starts_with("1\r\n00:00:01,000"));
        assert!(out.contains("2\r\n00:00:05,000 --> 00:00:06,000\r\nWhere were you?"));
    }

    #[test]
    fn hearing_keeps_dialogue() {
        let c = |t: &str| clean_hearing(Format::Srt, t, false);
        assert_eq!(c("I said (quietly) no."), None);
        assert_eq!(c("OK, let's go."), None);
        assert_eq!(c("At 10:30 we leave."), None);
        assert_eq!(c("♪ la la la ♪"), None);
        assert_eq!(c("♪ ♪").as_deref(), Some(""));
        assert_eq!(c("- [gasps]\n- What?").as_deref(), Some("What?"));
        assert_eq!(c("<i>(LAUGHING) Stop it.</i>").as_deref(), Some("<i>Stop it.</i>"));
        assert_eq!(c("MAN #2: Run!").as_deref(), Some("Run!"));
        // All in capitals: names and words cannot be told apart.
        assert_eq!(clean_hearing(Format::Srt, "JOHN: RUN!", true), None);
    }

    #[test]
    fn ads_out_at_the_edges() {
        let t = "1\n00:00:01,000 --> 00:00:02,000\nSubtitles by Someone\nwww.example.com\n\n2\n00:10:00,000 --> 00:10:02,000\nVisit www.example.com tonight.\n\n3\n00:20:00,000 --> 00:20:02,000\nSupport us and become VIP member\n\n4\n00:30:00,000 --> 00:30:02,000\nBye.\n\n";
        let mut d = Doc::parse(t, None).unwrap();
        // Few cues: all are at the edges.
        assert_eq!(d.drop_ads(), 4);
        assert_eq!(d.cues().count(), 1);
        assert!(is_ad("Legendas por Fulano", true));
        assert!(!is_ad("Legendas por Fulano", false));
        assert!(is_ad("Advertise your product or brand here", false));
        assert!(!is_ad("Visit www.example.com tonight.", false));
    }

    #[test]
    fn retimed() {
        let mut d = Doc::parse(SRT, None).unwrap();
        d.retime(-1500, 1.0);
        let c: Vec<&Cue> = d.cues().collect();
        assert_eq!((c[0].start, c[0].end), (0, 1000));
        d.retime(0, 25.0 / 23.976);
        let c: Vec<&Cue> = d.cues().collect();
        assert_eq!(c[2].start, ((3500.0f64) * 25.0 / 23.976).round() as i64);
    }

    const ASS: &str = "[Script Info]\nTitle: x\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, Bold, Italic\nStyle: Default,Arial,20,0,0\nStyle: Thought,Arial,20,0,-1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:02.50,Default,,0,0,0,,Hello, {\\i1}world{\\i0}\\Nsecond line\nComment: 0,0:00:01.00,0:00:02.50,Default,,0,0,0,,note\nDialogue: 0,0:00:03.00,0:00:04.00,Thought,,0,0,0,,{\\an8}thinking\nDialogue: 1,0:00:05.00,0:00:06.00,Default,,0,0,0,,{\\p1}m 0 0 l 10 10{\\p0}\n";

    #[test]
    fn substation_kept_and_made_subrip() {
        let d = Doc::parse(ASS, None).unwrap();
        assert_eq!(d.format, Format::Ass);
        assert_eq!(d.write(), ASS);
        let srt = d.to_srt();
        assert_eq!(
            srt,
            "1\r\n00:00:01,000 --> 00:00:02,500\r\nHello, <i>world</i>\r\nsecond line\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\n{\\an8}<i>thinking</i>\r\n\r\n"
        );
        let mut d = Doc::parse(ASS, None).unwrap();
        d.retime(1000, 1.0);
        assert!(d.write().contains("Dialogue: 0,0:00:02.00,0:00:03.50,Default"));
        assert!(d.write().contains("Comment: 0,0:00:02.00"));
    }

    #[test]
    fn webvtt_kept_and_made_subrip() {
        let t = "WEBVTT\n\nNOTE made by hand\n\nintro\n00:01.000 --> 00:02.000 line:0\n<v Bob>Hi <i>there</i></v> &amp; you\n\n00:00:03.000 --> 00:00:04.000\nA <00:00:03.500>B\n";
        let d = Doc::parse(t, None).unwrap();
        assert_eq!(d.cues().count(), 2);
        assert_eq!(
            d.write(),
            "WEBVTT\n\nNOTE made by hand\n\nintro\n00:00:01.000 --> 00:00:02.000 line:0\n<v Bob>Hi <i>there</i></v> &amp; you\n\n00:00:03.000 --> 00:00:04.000\nA <00:00:03.500>B\n"
        );
        let srt = d.to_srt();
        assert!(srt.contains("{\\an8}Hi <i>there</i> & you"));
        assert!(srt.contains("\r\nA B\r\n"));
        let mut d = Doc::parse(t, None).unwrap();
        d.retime(1000, 1.0);
        assert!(d.write().contains("A <00:00:04.500>B"));
    }

    #[test]
    fn microdvd_by_its_rate() {
        let t = "{1}{1}25\n{25}{50}{y:i}Hello|world\n{75}{100}{Y:b}Bold|too\n";
        let d = Doc::parse(t, None).unwrap();
        assert_eq!(d.fps, Some(25.0));
        let c: Vec<&Cue> = d.cues().collect();
        assert_eq!((c[0].start, c[0].end), (1000, 2000));
        assert_eq!(d.write(), t);
        let srt = d.to_srt();
        assert!(srt.contains("<i>Hello</i>\r\nworld"));
        assert!(srt.contains("<b>Bold</b>\r\n<b>too</b>"));
        let none = Doc::parse("{25}{50}Hi\n", None).unwrap();
        assert!(none.needs_fps());
        let given = Doc::parse("{25}{50}Hi\n", Some(25.0)).unwrap();
        assert_eq!(given.cues().next().unwrap().start, 1000);
    }

    #[test]
    fn subviewer_and_sami() {
        let sv = "[INFORMATION]\n[TITLE]x\n[END INFORMATION]\n\n00:00:01.00,00:00:02.00\nOne[br]Two\n\n00:00:03.00,00:00:04.00\nThree\n";
        let d = Doc::parse(sv, None).unwrap();
        assert_eq!(d.format, Format::SubViewer);
        assert_eq!(d.write(), sv);
        assert!(d.to_srt().contains("One\r\nTwo"));
        let smi = "<SAMI><BODY>\n<SYNC Start=1000><P Class=ENCC>Hello<br>there\n<SYNC Start=2500><P Class=ENCC>&nbsp;\n<SYNC Start=3000><P Class=ENCC><i>Bye</i>\n</BODY></SAMI>";
        let d = Doc::parse(smi, None).unwrap();
        assert_eq!(d.format, Format::Sami);
        assert_eq!(
            d.to_srt(),
            "1\r\n00:00:01,000 --> 00:00:02,500\r\nHello\r\nthere\r\n\r\n2\r\n00:00:03,000 --> 00:00:07,000\r\n<i>Bye</i>\r\n\r\n"
        );
    }

    #[test]
    fn encodings() {
        let (t, e) = decode(b"ol\xE1 a\xE7\xE3o", None);
        assert_eq!((t.as_str(), e), ("olá ação", Enc::Legacy(encoding_rs::WINDOWS_1252)));
        // "Привет мир" in windows-1251.
        let ru = b"\xCF\xF0\xE8\xE2\xE5\xF2 \xEC\xE8\xF0";
        assert_eq!(decode(ru, None).0, "Привет мир");
        assert_eq!(decode(ru, Some("ru")).0, "Привет мир");
        // "Zażółć gęślą" in windows-1250, told by its name.
        let pl = b"Za\xBF\xF3\xB3\xE6 g\xEA\x9Cl\xB9";
        assert_eq!(decode(pl, Some("pl")).0, "Zażółć gęślą");
        assert_eq!(decode(pl, None).0, "Zażółć gęślą");
        assert_eq!(to_utf8(b"\xEF\xBB\xBFol\xC3\xA1", None), "olá".as_bytes());
        assert_eq!(to_utf8(b"\xFF\xFEo\x00l\x00\xE1\x00", None), "olá".as_bytes());
        let w = Enc::Legacy(encoding_rs::WINDOWS_1252);
        assert_eq!(w.encode("olá"), b"ol\xE1");
        assert_eq!(w.encode("ол"), "ол".as_bytes());
    }

    #[test]
    fn files_fixed_and_put_back() {
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("store");
        let p = d.path().join("Movie.pt-BR.srt");
        let old = b"1\r\n00:00:01,000 --> 00:00:02,000\r\n[M\xDASICA]\r\nOl\xE1!\r\n\r\n".to_vec();
        fs::write(&p, &old).unwrap();
        let s = summary(&p).unwrap();
        assert_eq!(s.encoding, "windows-1252");
        assert_eq!(s.hearing, 1);
        let tools = Tools {
            utf8: true,
            hearing: true,
            ..Default::default()
        };
        let fix = plan(&s, &tools, "srt").unwrap();
        assert_eq!(fix.encoding, Some("windows-1252"));
        let done = fix_file(&p, &tools, Original::Keep, &store).unwrap();
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "1\r\n00:00:01,000 --> 00:00:02,000\r\nOlá!\r\n\r\n"
        );
        let b = done.backup.unwrap();
        restore(&p, b.saved.as_ref().unwrap(), &store).unwrap();
        assert_eq!(fs::read(&p).unwrap(), old);
        // Nothing to do: not touched.
        let none = fix_file(&p, &Tools::default(), Original::Keep, &store).unwrap();
        assert!(none.backup.is_none());
    }

    #[test]
    fn files_made_subrip() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("Movie.en.srt");
        fs::write(&p, ASS).unwrap();
        let tools = Tools {
            to_srt: true,
            ..Default::default()
        };
        let s = summary(&p).unwrap();
        assert!(plan(&s, &tools, "srt").unwrap().convert);
        let done = fix_file(&p, &tools, Original::Keep, &d.path().join("s")).unwrap();
        assert!(fs::read_to_string(&p).unwrap().starts_with("1\r\n00:00:01,000"));
        assert_eq!(fs::read_to_string(d.path().join("Movie.en.ass")).unwrap(), ASS);
        let c = done.backup.unwrap().converted.unwrap();
        crate::convert::restore(&p, &c).unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), ASS);
    }

    #[test]
    fn microdvd_without_rate_left_alone() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("Movie.srt");
        fs::write(&p, "{25}{50}Hi\n").unwrap();
        let s = summary(&p).unwrap();
        assert!(s.no_fps);
        let tools = Tools {
            to_srt: true,
            ..Default::default()
        };
        assert!(plan(&s, &tools, "srt").unwrap().no_fps);
        assert_eq!(
            fix_file(&p, &tools, Original::Keep, &d.path().join("s")).unwrap_err(),
            "subNoFps"
        );
    }
}
