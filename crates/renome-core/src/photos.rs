//! The photo rule: photos and phone videos named and put in folders by
//! when they were taken. The date comes from inside the file (EXIF, or a
//! video's header), else from its name (`IMG_20190512_143022`), else (when
//! asked) from when the file was last changed. A photo's other files (its
//! RAW, edits, a Live Photo's video) go with it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::{Datelike, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use regex::Regex;
use serde::Serialize;

use crate::files::FileEntry;
use crate::text;

/// Pictures read for a date.
const PHOTO: [&str; 11] = [
    "jpg", "jpeg", "jpe", "heic", "heif", "avif", "png", "webp", "tif", "tiff", "gif",
];
/// Cameras' raw files (most are TIFF inside, with EXIF).
const RAW: [&str; 22] = [
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef",
    "srw", "x3f", "3fr", "erf", "kdc", "mrw", "rwl", "iiq", "raw",
];
/// Videos a phone or camera records.
const CLIP: [&str; 7] = ["mp4", "mov", "m4v", "3gp", "mts", "m2ts", "avi"];
/// Files that only go with a photo: edits (`.xmp`, Apple's `.aae`).
const SIDECAR: [&str; 2] = ["xmp", "aae"];

fn among(list: &[&str], ext: &str) -> bool {
    list.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

pub fn is_photo(ext: &str) -> bool {
    among(&PHOTO, ext)
}

pub fn is_raw(ext: &str) -> bool {
    among(&RAW, ext)
}

pub fn is_clip(ext: &str) -> bool {
    among(&CLIP, ext)
}

pub fn is_sidecar(ext: &str) -> bool {
    among(&SIDECAR, ext)
}

/// What the photo rule names: photos, raw files, videos and edits.
pub fn is_shot(ext: &str) -> bool {
    is_photo(ext) || is_raw(ext) || is_clip(ext) || is_sidecar(ext)
}

/// What a photo or video says about itself.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inside {
    pub taken: Option<NaiveDateTime>,
    pub camera: String,
    pub width: u32,
    pub height: u32,
}

/// Reads [`Inside`]: EXIF from photos and raw files, the header from MP4
/// and QuickTime videos (the size from any video).
/// Reads what is inside these photos and videos, a few at a time: on a
/// slow disk each file costs a seek, and the disk can serve several.
/// Files read before are skipped; `progress(done, total)` as they go.
pub fn read_all(files: &[&FileEntry], progress: &(dyn Fn(usize, usize) + Sync)) {
    let todo: Vec<&FileEntry> = files
        .iter()
        .copied()
        .filter(|f| is_shot(&f.ext) && !is_sidecar(&f.ext) && f.photo.get().is_none())
        .collect();
    if todo.is_empty() {
        return;
    }
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get().clamp(2, 8));
    progress(0, todo.len());
    std::thread::scope(|s| {
        for _ in 0..workers.min(todo.len()) {
            s.spawn(|| {
                while let Some(f) = todo.get(next.fetch_add(1, Ordering::Relaxed)) {
                    f.photo_inside();
                    progress(done.fetch_add(1, Ordering::Relaxed) + 1, todo.len());
                }
            });
        }
    });
}

pub(crate) fn read_inside(file: &FileEntry) -> Option<Inside> {
    if file.is_dir {
        return None;
    }
    if is_clip(&file.ext) {
        // MP4 and QuickTime: the index read once for both.
        let rec = match crate::video::clip(&file.path) {
            Some((rec, info)) => {
                let _ = file.video.set(info);
                rec
            }
            None => Default::default(),
        };
        let (width, height) = file.video_info().map_or((0, 0), |v| (v.width, v.height));
        return Some(Inside {
            taken: rec.at,
            camera: camera(&rec.make, &rec.model),
            width,
            height,
        });
    }
    if !(is_photo(&file.ext) || is_raw(&file.ext)) {
        return None;
    }
    let mut out = Inside::default();
    let exif = crate::files::read_exif(&file.path);
    // Read once for both: the date is the one the insert rule shows too.
    let taken = exif.as_ref().and_then(crate::files::taken_in);
    let _ = file.taken.set(taken);
    out.taken = file.taken();
    if let Some(exif) = exif {
        use exif::{In, Tag};
        let text = |tag| {
            exif.get_field(tag, In::PRIMARY)
                .map(|f| {
                    f.display_value()
                        .to_string()
                        .trim_matches('"')
                        .trim()
                        .to_string()
                })
                .unwrap_or_default()
        };
        out.camera = camera(&text(Tag::Make), &text(Tag::Model));
        let number = |tag| {
            exif.get_field(tag, In::PRIMARY)
                .and_then(|f| f.value.get_uint(0))
                .unwrap_or(0)
        };
        out.width = number(Tag::PixelXDimension);
        out.height = number(Tag::PixelYDimension);
    }
    if (out.width == 0 || out.height == 0)
        && is_photo(&file.ext)
        && let Some((w, h)) = crate::files::picture_size(&file.path)
    {
        (out.width, out.height) = (w, h);
    }
    Some(out)
}

/// `Canon EOS 80D`, `Apple iPhone 12`: the make before the model, unless
/// the model already says it.
fn camera(make: &str, model: &str) -> String {
    let (make, model) = (make.trim(), model.trim());
    let first = make.split_whitespace().next().unwrap_or("");
    if model.is_empty() {
        make.to_string()
    } else if first.is_empty() || model.to_lowercase().starts_with(&first.to_lowercase()) {
        model.to_string()
    } else {
        format!("{first} {model}")
    }
}

/// Where a date came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DateFrom {
    /// The file's own data (EXIF, the video's header).
    Inside,
    /// Its name.
    Name,
    /// When the file was last changed.
    Modified,
    /// Nowhere: the file stays as it is.
    None,
}

/// When and with what a photo was taken, as the rule names it.
#[derive(Debug, Clone, PartialEq)]
pub struct Shot {
    pub taken: NaiveDateTime,
    /// A name with the day alone (`IMG-20190512-WA0001`) gives no time.
    pub has_time: bool,
    pub from: DateFrom,
    pub camera: String,
    pub width: u32,
    pub height: u32,
}

/// What the rule names a file by: its group's shot, and what of its own
/// name is kept after the new one (`.CR2` for `IMG_0001.CR2.xmp`).
#[derive(Debug, Clone, PartialEq)]
pub struct Named {
    pub shot: Shot,
    pub kept: String,
}

/// How the rule reads dates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// The date in the name, when the file has none inside.
    pub from_name: bool,
    /// Then when the file was last changed.
    pub modified: bool,
    /// Hours added to the camera's time (its clock was wrong, or set to
    /// another time zone); not to the file's change date, nor to a name's
    /// day without a time.
    pub shift_hours: i32,
}

/// The date in a name: `IMG_20190512_143022`, `PXL_20190512_143022123`,
/// `IMG-20190512-WA0001` (the day alone), `Screenshot_2019-05-12-14-30-22`,
/// `WhatsApp Image 2019-05-12 at 14.30.22`, `2019-05-12 14.30.22`.
static NAME_DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        (?:^|[^0-9])
        ((?:19|20)\d{2}) [-_.]? (0[1-9]|1[0-2]) [-_.]? (0[1-9]|[12]\d|3[01])
        (?:
            [-_.\sT]* (?:at\s*)?
            ([01]\d|2[0-3]) [-_.:h]? ([0-5]\d) [-_.:m]? ([0-5]\d) (?:\d{3})?
        )?
        (?:[^0-9]|$)",
    )
    .unwrap()
});

/// See [`NAME_DATE`]; `None` when the name has no date (or an impossible
/// one), and for years before 1970.
pub fn date_in_name(stem: &str) -> Option<(NaiveDateTime, bool)> {
    let c = NAME_DATE.captures(stem)?;
    let n = |i: usize| c.get(i).and_then(|m| m.as_str().parse::<u32>().ok());
    let year = i32::try_from(n(1)?).ok().filter(|y| *y >= 1970)?;
    let day = NaiveDate::from_ymd_opt(year, n(2)?, n(3)?)?;
    match (n(4), n(5), n(6)) {
        (Some(h), Some(m), Some(s)) => Some((day.and_hms_opt(h, m, s)?, true)),
        _ => Some((day.and_hms_opt(0, 0, 0)?, false)),
    }
}

/// A file's [`Shot`]: its own date, else its name's, else (when asked) its
/// change date. `None` for files the rule does not read, and for those
/// without a date.
pub fn shot(file: &FileEntry, opts: &Options) -> Option<Shot> {
    if file.is_dir || !is_shot(&file.ext) {
        return None;
    }
    let inside = file.photo_inside().cloned().unwrap_or_default();
    let shift = |t: NaiveDateTime| t + chrono::Duration::hours(i64::from(opts.shift_hours));
    let named = opts.from_name.then(|| date_in_name(&file.stem)).flatten();
    // A video's name with the time (`20260913_185946`, `VID_…`) says when
    // the phone started recording, in its own time; its header often holds
    // when the file was finished, in UTC.
    let (taken, has_time, from) = if is_clip(&file.ext)
        && let Some((t, true)) = named
    {
        (shift(t), true, DateFrom::Name)
    } else if let Some(t) = inside.taken {
        (shift(t), true, DateFrom::Inside)
    } else if let Some((t, has_time)) = named {
        // A day without its time is not moved to another day.
        (
            if has_time { shift(t) } else { t },
            has_time,
            DateFrom::Name,
        )
    } else if opts.modified
        && !is_sidecar(&file.ext)
        && let Some(t) = file
            .modified
            .and_then(|s| chrono::Local.timestamp_opt(s, 0).single())
    {
        (t.naive_local(), true, DateFrom::Modified)
    } else {
        return None;
    };
    Some(Shot {
        taken,
        has_time,
        from,
        camera: inside.camera,
        width: inside.width,
        height: inside.height,
    })
}

/// Expands a template: `{year}`, `{month}`, `{day}`, `{hour}`, `{minute}`,
/// `{second}`, `{date}`, `{month name}`, `{camera}`, `{width}`,
/// `{height}`, `{name}` (the name it has), also in Portuguese and Spanish.
/// A date without a time leaves the time's keys empty; what is left
/// around them is tidied away.
pub fn expand(template: &str, s: &Shot, name: &str, folder: bool) -> String {
    let two = |n: u32| format!("{n:02}");
    let year = format!("{:04}", s.taken.year());
    let (hour, minute, second) = if s.has_time {
        (
            two(s.taken.hour()),
            two(s.taken.minute()),
            two(s.taken.second()),
        )
    } else {
        Default::default()
    };
    let size = |n: u32| if n > 0 { n.to_string() } else { String::new() };
    let (width, height) = (size(s.width), size(s.height));
    let camera = crate::tmdb::safe_title(&s.camera, " - ");
    let name = crate::tmdb::safe_text(name, " - ");
    let mut extra = crate::fields::Extra::new();
    crate::fields::add_ymd(
        &mut extra,
        u16::try_from(s.taken.year()).ok(),
        u8::try_from(s.taken.month()).ok(),
        u8::try_from(s.taken.day()).ok(),
    );
    let out = crate::rules::expand(template, |key| match key {
        "year" | "ano" | "año" => Some(year.as_str()),
        "hour" | "hora" => Some(hour.as_str()),
        "minute" | "minuto" => Some(minute.as_str()),
        "second" | "segundo" => Some(second.as_str()),
        "camera" | "câmera" | "cámara" | "camara" => Some(camera.as_str()),
        "width" | "largura" | "ancho" => Some(width.as_str()),
        "height" | "altura" | "alto" => Some(height.as_str()),
        "name" | "nome" | "nombre" => Some(name.as_str()),
        key => crate::fields::get(&extra, key),
    });
    let out = out.replace("()", "").replace("[]", "");
    let out = text::collapse_spaces(&out);
    let edge = |c: char| c.is_whitespace() || "-–_.,".contains(c);
    let out = out.trim_start_matches(edge).trim_end_matches(edge);
    // A time left empty leaves its separators behind (`2019-05-12 ..`).
    let out = TIDY.replace_all(out, |c: &regex::Captures| {
        let run = &c[0];
        if run.chars().any(char::is_whitespace) {
            " ".to_string()
        } else {
            run.chars().next().map(String::from).unwrap_or_default()
        }
    });
    let out = out.trim_end_matches(|c: char| c.is_whitespace() || "-–_.".contains(c));
    if folder {
        out.trim_end_matches('.').to_string()
    } else {
        out.to_string()
    }
}

/// Separators with nothing between them (`a .. b`, `a_._b`): two marks
/// or more in a row become a space (or the first mark, without spaces).
static TIDY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[-_.:\s]*[-_.:][-_.:\s]*[-_.:][-_.:\s]*").unwrap());

/// The folders for a photo, one per `/` in the template; empty levels are
/// left out.
pub fn folders(template: &str, s: &Shot) -> Vec<String> {
    template
        .split('/')
        .map(|level| expand(level, s, "", true))
        .filter(|l| !l.is_empty())
        .collect()
}

/// Where a photo's folders start: the folder it was added with, above the
/// date folders it may be in already (`Photos/2019/2019-05` added itself:
/// `Photos`), so photos already sorted are not sorted again inside.
pub fn library_root(base: &Path, levels: usize) -> PathBuf {
    let mut root = base;
    for _ in 0..levels {
        let name = root.file_name().unwrap_or_default().to_string_lossy();
        match root.parent() {
            Some(up) if dated(&name) => root = up,
            _ => break,
        }
    }
    root.to_path_buf()
}

/// A folder named by a date: digits, separators and month names alone
/// (`2019`, `2019-05`, `05 - maio`, `2019-05-12`).
fn dated(name: &str) -> bool {
    let lower = text::strip_accents(&name.to_lowercase());
    let mut rest = lower.clone();
    for month in crate::fields::MONTHS.iter().flatten() {
        rest = rest.replace(&text::strip_accents(&month.to_lowercase()), "");
    }
    lower.chars().any(|c| c.is_ascii_digit())
        && rest
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_whitespace() || "-_.".contains(c))
}

/// The files that are one photo: in one folder, with one name before
/// their extension (`IMG_0001.JPG`, `IMG_0001.CR2`, `IMG_0001.xmp`,
/// `IMG_0001.CR2.xmp`, a Live Photo's `IMG_0001.HEIC` and `IMG_0001.MOV`).
/// Each file's group id and the part of its name kept after the new name
/// (`.CR2` for `IMG_0001.CR2.xmp`).
pub fn groups<'a>(files: &[&'a FileEntry]) -> HashMap<&'a Path, (usize, String)> {
    let mut ids: HashMap<(PathBuf, String), usize> = HashMap::new();
    let mut out = HashMap::new();
    for f in files.iter().filter(|f| !f.is_dir && is_shot(&f.ext)) {
        let (base, kept) = match crate::files::split_name(&f.stem) {
            (b, inner) if is_sidecar(&f.ext) && (is_photo(inner) || is_raw(inner)) => {
                (b.to_string(), format!(".{inner}"))
            }
            _ => (f.stem.clone(), String::new()),
        };
        let next = ids.len();
        let id = *ids
            .entry((f.dir.clone(), base.to_lowercase()))
            .or_insert(next);
        out.insert(f.path.as_path(), (id, kept));
    }
    out
}

/// Which file of a group gives the date: the one with a date inside, a
/// photo before a raw file before a video; then one dated by its name or
/// change date, in the same order.
pub fn lead_rank(file: &FileEntry, s: &Shot) -> (u8, u8) {
    let from = match s.from {
        DateFrom::Inside => 0,
        DateFrom::Name => 1,
        DateFrom::Modified => 2,
        DateFrom::None => 3,
    };
    let kind = if is_photo(&file.ext) {
        0
    } else if is_raw(&file.ext) {
        1
    } else if is_clip(&file.ext) {
        2
    } else {
        3
    };
    (from, kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot_at(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32, has_time: bool) -> Shot {
        Shot {
            taken: NaiveDate::from_ymd_opt(y, mo, d)
                .unwrap()
                .and_hms_opt(h, mi, s)
                .unwrap(),
            has_time,
            from: DateFrom::Inside,
            camera: "Canon EOS 80D".into(),
            width: 6000,
            height: 4000,
        }
    }

    #[test]
    fn dates_in_names() {
        let at = |s: &str| date_in_name(s).map(|(t, h)| (t.to_string(), h));
        let full = |t: &str| Some((t.to_string(), true));
        assert_eq!(at("IMG_20190512_143022"), full("2019-05-12 14:30:22"));
        assert_eq!(at("PXL_20190512_143022123"), full("2019-05-12 14:30:22"));
        assert_eq!(at("VID_20190512_143022"), full("2019-05-12 14:30:22"));
        assert_eq!(
            at("Screenshot_2019-05-12-14-30-22-123_com.app"),
            full("2019-05-12 14:30:22")
        );
        assert_eq!(
            at("WhatsApp Image 2019-05-12 at 14.30.22"),
            full("2019-05-12 14:30:22")
        );
        assert_eq!(at("2019-05-12 14.30.22"), full("2019-05-12 14:30:22"));
        assert_eq!(
            at("IMG-20190512-WA0001"),
            Some(("2019-05-12 00:00:00".into(), false))
        );
        assert_eq!(
            at("Viagem 2019-05-12 (3)"),
            Some(("2019-05-12 00:00:00".into(), false))
        );
        // Not dates: other numbers, impossible days, long runs of digits.
        assert_eq!(at("IMG_0001"), None);
        assert_eq!(at("DSC_20191345"), None);
        assert_eq!(at("1557671422123"), None);
        assert_eq!(at("120190512"), None);
        assert_eq!(at("Ferias 1960-05-12"), None);
    }

    #[test]
    fn templates() {
        let s = shot_at(2019, 5, 12, 14, 30, 22, true);
        assert_eq!(
            expand(
                "{ano}-{mês}-{dia} {hora}.{minuto}.{segundo}",
                &s,
                "IMG_0001",
                false
            ),
            "2019-05-12 14.30.22"
        );
        assert_eq!(
            expand(
                "{year}{month}{day}_{hour}{minute}{second} ({camera})",
                &s,
                "x",
                false
            ),
            "20190512_143022 (Canon EOS 80D)"
        );
        assert_eq!(
            expand("{año}-{mes}-{día} - {nombre}", &s, "Praia", false),
            "2019-05-12 - Praia"
        );
        assert_eq!(expand("{largura}×{altura}", &s, "", false), "6000×4000");
        // No time: what was around it goes.
        let day = shot_at(2019, 5, 12, 0, 0, 0, false);
        assert_eq!(
            expand(
                "{ano}-{mês}-{dia} {hora}.{minuto}.{segundo}",
                &day,
                "",
                false
            ),
            "2019-05-12"
        );
        assert_eq!(
            expand(
                "{ano}-{mês}-{dia} {hora}{minuto}{segundo} ({câmera})",
                &day,
                "",
                false
            ),
            "2019-05-12 (Canon EOS 80D)"
        );
        // No camera: its brackets go.
        let mut bare = s.clone();
        bare.camera.clear();
        assert_eq!(
            expand("{ano}-{mês}-{dia} ({câmera})", &bare, "", false),
            "2019-05-12"
        );
        assert_eq!(
            folders("{ano}/{mês} - {nome do mês}", &s),
            vec!["2019".to_string(), "05 - maio".to_string()]
        );
    }

    #[test]
    fn roots_above_date_folders() {
        let p = Path::new("/f/Fotos/2019/2019-05");
        assert_eq!(library_root(p, 2), PathBuf::from("/f/Fotos"));
        assert_eq!(library_root(p, 1), PathBuf::from("/f/Fotos/2019"));
        assert_eq!(
            library_root(Path::new("/f/Fotos/Viagem"), 2),
            PathBuf::from("/f/Fotos/Viagem")
        );
        assert_eq!(
            library_root(Path::new("/f/Fotos/2019/05 - Maio"), 2),
            PathBuf::from("/f/Fotos")
        );
    }

    #[test]
    fn clock_shift() {
        let d = tempfile::tempdir().unwrap();
        let file = |name: &str| {
            let p = d.path().join(name);
            std::fs::write(&p, "x").unwrap();
            FileEntry::from_path(&p, &std::fs::metadata(&p).unwrap()).unwrap()
        };
        let opts = Options {
            from_name: true,
            modified: false,
            shift_hours: -3,
        };
        let at = |name: &str| shot(&file(name), &opts).unwrap().taken.to_string();
        assert_eq!(at("IMG_20190512_013022.jpg"), "2019-05-11 22:30:22");
        // The day alone stays that day.
        assert_eq!(at("IMG-20190512-WA0001.jpg"), "2019-05-12 00:00:00");
    }

    #[test]
    fn cameras() {
        assert_eq!(camera("Canon", "Canon EOS 80D"), "Canon EOS 80D");
        assert_eq!(camera("Apple", "iPhone 12"), "Apple iPhone 12");
        assert_eq!(camera("NIKON CORPORATION", "NIKON D750"), "NIKON D750");
        assert_eq!(camera("samsung", "SM-G991B"), "samsung SM-G991B");
        assert_eq!(camera("", ""), "");
    }

    #[test]
    fn files_of_one_photo() {
        let f = |name: &str| {
            let (stem, ext) = crate::files::split_name(name);
            FileEntry {
                path: PathBuf::from(format!("/p/{name}")),
                dir: PathBuf::from("/p"),
                stem: stem.into(),
                ext: ext.into(),
                ..Default::default()
            }
        };
        let all = [
            f("IMG_0001.JPG"),
            f("IMG_0001.CR2"),
            f("IMG_0001.CR2.xmp"),
            f("img_0001.xmp"),
            f("IMG_0002.HEIC"),
            f("IMG_0002.MOV"),
            f("notes.txt"),
        ];
        let refs: Vec<&FileEntry> = all.iter().collect();
        let g = groups(&refs);
        let id = |n: &str| g[Path::new(&format!("/p/{n}"))].0;
        assert_eq!(id("IMG_0001.JPG"), id("IMG_0001.CR2"));
        assert_eq!(id("IMG_0001.JPG"), id("IMG_0001.CR2.xmp"));
        assert_eq!(id("IMG_0001.JPG"), id("img_0001.xmp"));
        assert_eq!(id("IMG_0002.HEIC"), id("IMG_0002.MOV"));
        assert_ne!(id("IMG_0001.JPG"), id("IMG_0002.HEIC"));
        assert_eq!(g[Path::new("/p/IMG_0001.CR2.xmp")].1, ".CR2");
        assert!(!g.contains_key(Path::new("/p/notes.txt")));
    }
}
