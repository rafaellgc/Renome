//! Subtitles to download for movies and episodes: the languages asked
//! for, those a video has already (beside it or inside it), the video's
//! fingerprint the services match exact copies by, how the subtitles
//! found rank, and the name each one gets. Searching and downloading are
//! the app's.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Two-letter codes with their three-letter ones (bibliographic and
/// terminology), to tell what a subtitle already there or a track holds.
const CODES: &[(&str, &[&str])] = &[
    ("pt", &["por", "pob", "ptb"]),
    ("en", &["eng"]),
    ("es", &["spa", "spl", "esp"]),
    ("fr", &["fre", "fra"]),
    ("de", &["ger", "deu"]),
    ("it", &["ita"]),
    ("ja", &["jpn"]),
    ("ko", &["kor"]),
    ("zh", &["chi", "zho", "chs", "cht"]),
    ("ru", &["rus"]),
    ("nl", &["dut", "nld"]),
    ("sv", &["swe"]),
    ("no", &["nor", "nob", "nb"]),
    ("da", &["dan"]),
    ("fi", &["fin"]),
    ("pl", &["pol"]),
    ("tr", &["tur"]),
    ("ar", &["ara"]),
    ("he", &["heb"]),
    ("el", &["gre", "ell"]),
    ("cs", &["cze", "ces"]),
    ("hu", &["hun"]),
    ("ro", &["rum", "ron"]),
    ("uk", &["ukr"]),
    ("th", &["tha"]),
    ("vi", &["vie"]),
    ("id", &["ind"]),
    ("ca", &["cat"]),
    ("hr", &["hrv"]),
    ("sr", &["srp"]),
    ("bg", &["bul"]),
];

/// A language asked for, as the options write it (`pt-BR`, `en`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lang {
    /// What the subtitle's name ends with: `pt-BR`, `en`.
    pub tag: String,
    /// OpenSubtitles' code: `pt-br`, `en`.
    pub opensubtitles: String,
    /// SubDL's: `BR_PT`, `PT`, `EN`.
    pub subdl: String,
    /// SubSource's: `brazilian_portuguese`, `english`; empty when it has
    /// none for it.
    pub subsource: String,
}

/// SubSource's name of a language (two letters and region).
fn subsource_code(two: &str, region: &str) -> &'static str {
    match (two, region) {
        ("pt", "BR") => return "brazilian_portuguese",
        ("es", "ES") => return "spanish_spain",
        ("es", r) if !r.is_empty() => return "spanish_latin_america",
        ("fr", "CA") => return "french_canada",
        ("zh", "TW" | "HK") => return "chinese_traditional",
        ("zh", "CN" | "SG") => return "chinese_simplified",
        _ => {}
    }
    const NAMES: [(&str, &str); 72] = [
        ("af", "afrikaans"), ("am", "amharic"), ("ar", "arabic"), ("as", "assamese"),
        ("az", "azerbaijani"), ("be", "belarusian"), ("bg", "bulgarian"), ("bn", "bengali"),
        ("br", "breton"), ("bs", "bosnian"), ("ca", "catalan"), ("cs", "czech"),
        ("cy", "welsh"), ("da", "danish"), ("de", "german"), ("el", "greek"),
        ("en", "english"), ("eo", "espranto"), ("es", "spanish"), ("et", "estonian"),
        ("eu", "basque"), ("fa", "farsi_persian"), ("fi", "finnish"), ("fr", "french"),
        ("ga", "irish"), ("gd", "gaelic"), ("gl", "gaelician"), ("he", "hebrew"),
        ("hi", "hindi"), ("hr", "croatian"), ("hu", "hungarian"), ("hy", "armenian"),
        ("id", "indonesian"), ("is", "icelandic"), ("it", "italian"), ("ja", "japanese"),
        ("ka", "georgian"), ("kk", "kazakh"), ("km", "khmer"), ("kn", "kannada"),
        ("ko", "korean"), ("ku", "kurdish"), ("ky", "kyrgyz"), ("lb", "luxembourgish"),
        ("lt", "lithuanian"), ("lv", "latvian"), ("mk", "macedonian"), ("ml", "malayalam"),
        ("mn", "mongolian"), ("mr", "marathi"), ("ms", "malay"), ("my", "burmese"),
        ("ne", "nepali"), ("nl", "dutch"), ("no", "norwegian"), ("pl", "polish"),
        ("ps", "pashto"), ("pt", "portuguese"), ("ro", "romanian"), ("ru", "russian"),
        ("si", "sinhala"), ("sk", "slovak"), ("sl", "slovenian"), ("sq", "albanian"),
        ("sr", "serbian"), ("sv", "swedish"), ("sw", "swahili"), ("ta", "tamil"),
        ("te", "telugu"), ("th", "thai"), ("tr", "turkish"), ("uk", "ukrainian"),
    ];
    const MORE: [(&str, &str); 6] = [
        ("ur", "urdu"), ("uz", "uzbek"), ("vi", "vietnamese"), ("zh", "chinese"),
        ("tl", "tagalog"), ("tt", "tatar"),
    ];
    NAMES
        .iter()
        .chain(MORE.iter())
        .find(|(t, _)| *t == two)
        .map_or("", |(_, n)| *n)
}

/// The two-letter code of a language written as two or three letters.
fn two_letters(code: &str) -> Option<&'static str> {
    let code = code.to_ascii_lowercase();
    CODES
        .iter()
        .find(|(two, threes)| *two == code || threes.contains(&code.as_str()))
        .map(|(two, _)| *two)
}

/// The languages in the options' text (`pt-BR, en`), in order, each once.
/// Unknown codes are left out; `pob` and `pt-br` are `pt-BR`.
pub fn langs(text: &str) -> Vec<Lang> {
    let mut out: Vec<Lang> = Vec::new();
    for word in text.split([',', ';', ' ']).filter(|w| !w.is_empty()) {
        let (code, region) = word.split_once(['-', '_']).unwrap_or((word, ""));
        let brazil = region.eq_ignore_ascii_case("br")
            || ["pob", "ptb"].contains(&code.to_ascii_lowercase().as_str());
        let Some(two) = two_letters(code) else {
            continue;
        };
        let region = if two == "pt" && brazil {
            "BR".to_string()
        } else {
            region.to_ascii_uppercase()
        };
        let tag = if region.is_empty() {
            two.to_string()
        } else {
            format!("{two}-{region}")
        };
        let subdl = match (two, region.as_str()) {
            ("pt", "BR") => "BR_PT".to_string(),
            ("zh", "TW" | "HK") => "ZH_BG".to_string(),
            _ => two.to_ascii_uppercase(),
        };
        let lang = Lang {
            opensubtitles: tag.to_ascii_lowercase(),
            subsource: subsource_code(two, &region).to_string(),
            tag,
            subdl,
        };
        if !out.contains(&lang) {
            out.push(lang);
        }
    }
    out
}

/// Whether a language written on a subtitle or a track (`pt-BR`, `por`,
/// `eng`, `pt`) is the one asked for. One without a region counts for
/// every region (a `.pt.srt` beside a Brazilian video is usually
/// Brazilian); one with another region does not.
pub fn same_lang(found: &str, wanted: &Lang) -> bool {
    let (code, region) = found.split_once(['-', '_']).unwrap_or((found, ""));
    let Some(two) = two_letters(code) else {
        return false;
    };
    let (want_two, want_region) = wanted.tag.split_once('-').unwrap_or((&wanted.tag, ""));
    if two != want_two {
        return false;
    }
    let region = if ["pob", "ptb"].contains(&code.to_ascii_lowercase().as_str()) {
        "BR"
    } else {
        region
    };
    region.is_empty() || want_region.is_empty() || region.eq_ignore_ascii_case(want_region)
}

/// The languages of the subtitles already beside a video: files named
/// after it with a subtitle extension (`Movie.pt-BR.srt`, `Movie.eng.forced.srt`;
/// `Movie.srt`, without one, is not counted).
pub fn beside(video: &Path) -> Vec<String> {
    let (Some(dir), Some(stem)) = (video.parent(), video.file_stem()) else {
        return Vec::new();
    };
    let stem = stem.to_string_lossy();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some((base, ext)) = name.rsplit_once('.') else {
            continue;
        };
        if !crate::tmdb::is_subtitle(ext) {
            continue;
        }
        let Some(rest) = base.strip_prefix(&*stem) else {
            continue;
        };
        let tags = crate::tmdb::subtitle_tags(base, ext);
        if tags.len() != rest.len() {
            continue;
        }
        if let Some(lang) = tags
            .split('.')
            .find(|t| two_letters(t.split(['-', '_']).next().unwrap_or("")).is_some())
        {
            out.push(lang.to_string());
        }
    }
    out
}

/// The languages a video has: subtitles beside it and, for Matroska, the
/// subtitle tracks inside it.
pub fn present(video: &Path) -> Vec<String> {
    let mut out = beside(video);
    out.extend(crate::video::subtitle_tracks(video));
    out
}

/// The OpenSubtitles fingerprint of a video: its size plus the sum of its
/// first and last 64 KiB read as little-endian 64-bit numbers, in hex. A
/// subtitle with the same one was timed for this very file.
pub fn hash(path: &Path) -> Option<String> {
    const CHUNK: u64 = 65536;
    let mut f = File::open(path).ok()?;
    let size = f.metadata().ok()?.len();
    if size < CHUNK * 2 {
        return None;
    }
    let mut sum = size;
    let mut buf = vec![0u8; CHUNK as usize];
    for start in [0, size - CHUNK] {
        f.seek(SeekFrom::Start(start)).ok()?;
        f.read_exact(&mut buf).ok()?;
        for w in buf.as_chunks::<8>().0 {
            sum = sum.wrapping_add(u64::from_le_bytes(*w));
        }
    }
    Some(format!("{sum:016x}"))
}

/// Where a subtitle comes from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    #[default]
    OpenSubtitles,
    Subdl,
    Subsource,
}

/// A subtitle found for a video.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub source: Source,
    /// What the download asks for: OpenSubtitles' file number, SubDL's
    /// path.
    pub id: String,
    /// The language asked for that it is in (`pt-BR`).
    pub lang: String,
    /// The release it was made for, as its uploader wrote it.
    pub release: String,
    pub downloads: u64,
    /// Made for this very file (same fingerprint).
    pub exact: bool,
    /// For the hard of hearing (sounds described).
    pub hearing_impaired: bool,
    /// From an uploader the service trusts.
    pub trusted: bool,
    /// The page to see it on.
    pub url: String,
}

/// Words of a release name, lower case (`Movie.2010.1080p.WEB-DL.x264-GRP`
/// → `movie 2010 1080p web dl x264 grp`).
fn words(name: &str) -> Vec<String> {
    name.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// The kind of source a release came from, by family: a subtitle for one
/// WEB release is usually in time with another, not with a Blu-ray.
fn origin(words: &[String]) -> Option<&'static str> {
    let has = |w: &str| words.iter().any(|x| x == w);
    let joined = words.join(" ");
    if has("bluray") || has("bdrip") || has("brrip") || has("bdremux") || joined.contains("blu ray")
    {
        Some("bluray")
    } else if has("webdl")
        || has("webrip")
        || has("web")
        || has("amzn")
        || has("nf")
        || has("dsnp")
        || has("hmax")
    {
        Some("web")
    } else if has("hdtv") || has("pdtv") {
        Some("hdtv")
    } else if has("dvdrip") || has("dvd") || has("dvdscr") {
        Some("dvd")
    } else {
        None
    }
}

/// The release group: the word after the last dash (`…x264-GRP`).
fn group(name: &str) -> Option<String> {
    let stem = name
        .rsplit_once('.')
        .map_or(name, |(s, e)| if e.len() <= 4 { s } else { name });
    let g = stem.rsplit_once('-')?.1.trim();
    (!g.is_empty() && g.len() <= 20 && g.chars().all(|c| c.is_alphanumeric()))
        .then(|| g.to_lowercase())
}

/// How well a subtitle fits a video file named `video`; higher is
/// better. An exact one (same fingerprint) first; then the same release
/// group, the same kind of source, resolution, codec and edition; then
/// how many downloaded it. One for the hard of hearing is a little behind.
pub fn score(video: &str, c: &Candidate) -> f64 {
    let v = words(video);
    let r = words(&c.release);
    let mut s = 0.0;
    if c.exact {
        s += 1000.0;
    }
    if let (Some(a), Some(b)) = (group(video), group(&c.release))
        && a == b
    {
        s += 40.0;
    }
    if let (Some(a), Some(b)) = (origin(&v), origin(&r)) {
        s += if a == b { 30.0 } else { -10.0 };
    }
    const TAGS: &[&[&str]] = &[
        &["2160p", "4k", "uhd"],
        &["1080p"],
        &["720p"],
        &["480p", "576p"],
        &["x264", "h264", "avc"],
        &["x265", "h265", "hevc"],
        &["xvid", "divx"],
        &["extended"],
        &["unrated"],
        &["directors", "dc"],
        &["remastered"],
        &["imax"],
        &["proper"],
        &["repack"],
    ];
    for set in TAGS {
        let a = v.iter().any(|w| set.contains(&w.as_str()));
        let b = r.iter().any(|w| set.contains(&w.as_str()));
        if a && b {
            s += 8.0;
        } else if a != b
            && set.len() == 1
            && ["extended", "unrated", "remastered", "imax"].contains(&set[0])
        {
            // Another cut: other scenes, other times.
            s -= 40.0;
        }
    }
    // Words in common, as a share of the subtitle's.
    if !r.is_empty() {
        let common = r.iter().filter(|w| v.contains(w)).count();
        s += 20.0 * common as f64 / r.len() as f64;
    }
    s += 2.0 * (1.0 + c.downloads as f64).ln();
    if c.trusted {
        s += 3.0;
    }
    if c.hearing_impaired {
        s -= 12.0;
    }
    s
}

/// The candidates best first (see [`score`]).
pub fn rank(video: &str, cands: &mut [Candidate]) {
    cands.sort_by(|a, b| score(video, b).total_cmp(&score(video, a)));
}

/// Which of an archive's subtitles (index, name, extension, size) is the
/// video's: see [`from_archive`].
fn pick_in_archive<'a>(
    found: &'a [(usize, String, String, u64)],
    episode: Option<(u32, u32)>,
    video: &str,
) -> Option<&'a (usize, String, String, u64)> {
    let mut pool: Vec<&(usize, String, String, u64)> = found.iter().collect();
    if let Some((s, e)) = episode.filter(|_| pool.len() > 1) {
        let read: Vec<_> = pool
            .iter()
            .map(|f| {
                let stem = f.1.rsplit_once('.').map_or(f.1.as_str(), |(a, _)| a);
                crate::tv::parse(stem, "")
            })
            .collect();
        let has = |g: &crate::tv::EpisodeGuess| {
            g.episodes.iter().any(|&n| u32::from(n) == e) || g.number == Some(e)
        };
        let exact: Vec<_> = (0..pool.len())
            .filter(|&i| read[i].season.map(u32::from) == Some(s) && has(&read[i]))
            .collect();
        let loose: Vec<_> = (0..pool.len())
            .filter(|&i| read[i].season.is_none() && has(&read[i]))
            .collect();
        let keep = if exact.is_empty() { loose } else { exact };
        if !keep.is_empty() {
            pool = keep.into_iter().map(|i| pool[i]).collect();
        }
    }
    let words = |s: &str| -> std::collections::HashSet<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 1)
            .map(str::to_string)
            .collect()
    };
    let of_video = words(video);
    pool.into_iter()
        .max_by_key(|f| (words(&f.1).intersection(&of_video).count(), f.3))
}

/// Whether a downloaded subtitle can be for a video that plays `video`
/// seconds: one whose last line (ads left out) comes over a minute after
/// the video ends, or before its middle, is for another cut or another
/// frame rate. `None` when it cannot be told (unreadable, no lines, a
/// MicroDVD file without its frame rate).
pub fn fits(bytes: &[u8], lang: Option<&str>, video: f64, fps: Option<f64>) -> Option<bool> {
    let (text, _) = crate::subtext::decode(bytes, lang);
    let mut doc = crate::subtext::Doc::parse(&text, fps)?;
    if doc.needs_fps() {
        return None;
    }
    doc.drop_ads();
    let last = doc.cues().map(|c| c.end).max()? as f64 / 1000.0;
    Some(last <= video + 60.0 && last >= video / 2.0)
}

/// The name a subtitle gets: the video's, with its language and `.sdh`
/// for the hard of hearing (`Movie (2010).pt-BR.srt`).
pub fn target(video: &Path, lang: &str, hearing_impaired: bool, ext: &str) -> PathBuf {
    let stem = video
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = if crate::tmdb::is_subtitle(ext) {
        ext.to_lowercase()
    } else {
        "srt".into()
    };
    let flag = if hearing_impaired { ".sdh" } else { "" };
    video.with_file_name(format!("{stem}.{lang}{flag}.{ext}"))
}

/// The subtitle in a downloaded archive (SubDL sends zips): the one file
/// with a subtitle extension (macOS' `__MACOSX` copies left out); when
/// there are several, the one for the episode (its number read as in
/// file names: `S01E02`, `1x02`, `E02`, `- 02`…), then the one sharing
/// most words with the video's name (`video`), then the largest. Its
/// extension and bytes. An archive that is not a zip is taken for the
/// subtitle itself (`srt`).
pub fn from_archive(bytes: &[u8], episode: Option<(u32, u32)>, video: &str) -> Option<(String, Vec<u8>)> {
    if !bytes.starts_with(b"PK") {
        return (!bytes.is_empty()).then(|| ("srt".to_string(), bytes.to_vec()));
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    let mut found: Vec<(usize, String, String, u64)> = Vec::new();
    for i in 0..zip.len() {
        let Ok(f) = zip.by_index(i) else { continue };
        let Ok(name) = f.name().map(|n| n.to_string()) else {
            continue;
        };
        let base = name.rsplit(['/', '\\']).next().unwrap_or(&name).to_string();
        if f.is_dir() || name.contains("__MACOSX") || base.starts_with("._") {
            continue;
        }
        let Some((_, ext)) = base.rsplit_once('.') else {
            continue;
        };
        if ["srt", "ass", "ssa", "vtt", "sub"].contains(&ext.to_lowercase().as_str()) {
            found.push((i, base.clone(), ext.to_lowercase(), f.size()));
        }
    }
    let pick = pick_in_archive(&found, episode, video)?;
    let mut out = Vec::new();
    zip.by_index(pick.0).ok()?.read_to_end(&mut out).ok()?;
    Some((pick.2.clone(), out))
}

#[cfg(test)]
mod tests {
    #[test]
    fn subtitle_fits_the_video_length() {
        // Two lines, the last one at `t`.
        let at = |t: &str| {
            format!("1\n00:00:05,000 --> 00:00:07,000\nOlá.\n\n2\n{t},000 --> {t},500\nFim.\n")
        };
        // A 100-minute video.
        let v = 6000.0;
        assert_eq!(super::fits(at("01:38:00").as_bytes(), None, v, None), Some(true));
        assert_eq!(super::fits(at("01:40:50").as_bytes(), None, v, None), Some(true));
        assert_eq!(super::fits(at("01:45:00").as_bytes(), None, v, None), Some(false));
        assert_eq!(super::fits(at("00:40:00").as_bytes(), None, v, None), Some(false));
        assert_eq!(super::fits(b"nada", None, v, None), None);
    }

    use super::*;

    fn cand(release: &str, downloads: u64) -> Candidate {
        Candidate {
            source: Source::OpenSubtitles,
            id: release.into(),
            lang: "pt-BR".into(),
            release: release.into(),
            downloads,
            exact: false,
            hearing_impaired: false,
            trusted: false,
            url: String::new(),
        }
    }

    #[test]
    fn languages_as_written() {
        let l = langs("pt-br, EN; pob es-MX xx");
        let tags: Vec<&str> = l.iter().map(|l| l.tag.as_str()).collect();
        assert_eq!(tags, ["pt-BR", "en", "es-MX"]);
        assert_eq!(l[0].opensubtitles, "pt-br");
        assert_eq!(l[0].subdl, "BR_PT");
        assert_eq!(l[1].subdl, "EN");
        assert_eq!(langs("pt")[0].subdl, "PT");
        assert_eq!(langs("por")[0].tag, "pt");
        assert_eq!(l[0].subsource, "brazilian_portuguese");
        assert_eq!(l[1].subsource, "english");
        assert_eq!(l[2].subsource, "spanish_latin_america");
        assert_eq!(langs("pt")[0].subsource, "portuguese");
    }

    #[test]
    fn languages_already_there() {
        let br = &langs("pt-BR")[0];
        let pt = &langs("pt-PT")[0];
        assert!(same_lang("pt-BR", br));
        assert!(same_lang("pob", br));
        assert!(same_lang("por", br));
        assert!(same_lang("pt", pt));
        assert!(!same_lang("pt-BR", pt));
        assert!(!same_lang("eng", br));
        assert!(same_lang("eng", &langs("en")[0]));
    }

    #[test]
    fn subtitles_beside_a_video() {
        let d = tempfile::tempdir().unwrap();
        let v = d.path().join("Movie (2010).mkv");
        for f in [
            "Movie (2010).mkv",
            "Movie (2010).pt-BR.srt",
            "Movie (2010).eng.forced.srt",
            "Movie (2010).srt",
            "Other.en.srt",
        ] {
            std::fs::write(d.path().join(f), "x").unwrap();
        }
        let mut got = beside(&v);
        got.sort();
        assert_eq!(got, ["eng", "pt-BR"]);
    }

    #[test]
    fn fingerprint_of_a_file() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("v.bin");
        // 131072 + 8 bytes: 1 at the start, 2 at the end.
        let mut b = vec![0u8; 131_080];
        b[0] = 1;
        let n = b.len();
        b[n - 8] = 2;
        std::fs::write(&p, &b).unwrap();
        assert_eq!(hash(&p).unwrap(), format!("{:016x}", 131_080u64 + 1 + 2));
        std::fs::write(&p, b"small").unwrap();
        assert_eq!(hash(&p), None);
    }

    #[test]
    fn ranks_by_fit() {
        let video = "Movie.2010.1080p.WEB-DL.DDP5.1.H.264-GRP.mkv";
        let mut c = vec![
            cand("Movie.2010.1080p.BluRay.x264-OTHER", 90000),
            cand("Movie.2010.1080p.WEB-DL.H264-GRP", 50),
            cand("Movie.2010.EXTENDED.1080p.WEB-DL-NOPE", 5000),
            {
                let mut e = cand("whatever", 0);
                e.exact = true;
                e
            },
        ];
        rank(video, &mut c);
        let order: Vec<&str> = c.iter().map(|c| c.release.as_str()).collect();
        assert_eq!(order[0], "whatever");
        assert_eq!(order[1], "Movie.2010.1080p.WEB-DL.H264-GRP");
        assert_eq!(order[3], "Movie.2010.EXTENDED.1080p.WEB-DL-NOPE");
    }

    #[test]
    fn subtitle_names() {
        let v = Path::new("/m/Movie (2010)/Movie (2010).mkv");
        assert_eq!(
            target(v, "pt-BR", false, "SRT"),
            Path::new("/m/Movie (2010)/Movie (2010).pt-BR.srt")
        );
        assert_eq!(
            target(v, "en", true, "ass"),
            Path::new("/m/Movie (2010)/Movie (2010).en.sdh.ass")
        );
        assert_eq!(
            target(v, "en", false, "zip"),
            Path::new("/m/Movie (2010)/Movie (2010).en.srt")
        );
    }

    #[test]
    fn subtitle_out_of_a_zip() {
        use std::io::Write;
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default();
            for (n, body) in [
                ("__MACOSX/._a.srt", "junk"),
                ("Show.S01E01.srt", "one"),
                ("Show.S01E02.srt", "two!"),
                ("readme.txt", "x"),
            ] {
                z.start_file(n, o).unwrap();
                z.write_all(body.as_bytes()).unwrap();
            }
            z.finish().unwrap();
        }
        let zip = buf.into_inner();
        assert_eq!(from_archive(&zip, Some((1, 1)), "").unwrap().1, b"one");
        assert_eq!(from_archive(&zip, None, "").unwrap().1, b"two!");
        assert_eq!(
            from_archive(b"1\n00:00", None, "").unwrap(),
            ("srt".into(), b"1\n00:00".to_vec())
        );
    }

    #[test]
    fn episode_and_release_found_in_a_zip() {
        use std::io::Write;
        let zip_of = |names: &[(&str, &str)]| {
            let mut buf = std::io::Cursor::new(Vec::new());
            {
                let mut z = zip::ZipWriter::new(&mut buf);
                let o = zip::write::SimpleFileOptions::default();
                for (n, body) in names {
                    z.start_file(*n, o).unwrap();
                    z.write_all(body.as_bytes()).unwrap();
                }
                z.finish().unwrap();
            }
            buf.into_inner()
        };
        // Numbered without the season.
        let z = zip_of(&[("Show - 03.srt", "three"), ("Show - 04.srt", "four"), ("Show - 05 long.srt", "five!!")]);
        assert_eq!(from_archive(&z, Some((1, 4)), "Show S01E04").unwrap().1, b"four");
        let z = zip_of(&[("Show.E03.srt", "three"), ("Show.E04.srt", "four")]);
        assert_eq!(from_archive(&z, Some((2, 3)), "").unwrap().1, b"three");
        // The release of the video, not the largest.
        let z = zip_of(&[
            ("Movie.2010.1080p.BluRay.x264-GRP.srt", "blu"),
            ("Movie.2010.720p.WEB-DL-OTHER.srt", "web, longer"),
        ]);
        assert_eq!(from_archive(&z, None, "Movie.2010.1080p.BluRay.x264-GRP").unwrap().1, b"blu");
    }
}
