use std::cmp::Ordering;
use std::fs::Metadata;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::music::AudioTags;

/// A file (or folder) in the list, with its name already split into stem
/// and extension.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: PathBuf,
    pub dir: PathBuf,
    /// Name without the extension (`photo.tar` for `photo.tar.gz`; `.bashrc` for `.bashrc`).
    pub stem: String,
    /// Extension without the dot; empty when there is none.
    pub ext: String,
    pub size: u64,
    /// Modification time in Unix seconds.
    pub modified: Option<i64>,
    /// A folder renamed as an item; its name has no extension.
    #[serde(default)]
    pub is_dir: bool,
    /// The folder it was added with (its own folder when added by itself):
    /// sent to another folder, it keeps the subfolders below this one.
    #[serde(default)]
    pub base: PathBuf,
    /// Date the photo was taken (EXIF), read on first use.
    #[serde(skip)]
    pub(crate) taken: OnceLock<Option<NaiveDateTime>>,
    /// Tags of an audio file, read on first use.
    #[serde(skip)]
    pub(crate) tags: OnceLock<Option<AudioTags>>,
    /// The `ComicInfo.xml` of a comic, read on first use.
    #[serde(skip)]
    pub(crate) comic: OnceLock<Option<crate::comics::ComicInfo>>,
    /// A video's picture size and codec, read on first use.
    #[serde(skip)]
    pub(crate) video: OnceLock<Option<crate::video::VideoInfo>>,
    /// What the content really is (see [`crate::kind`]), read on first use.
    #[serde(skip)]
    pub(crate) kind: OnceLock<Option<crate::kind::Kind>>,
    /// A comic the comic rule can convert, and how many of its pages would
    /// be drawn (see [`crate::convert::check`]); checked on first use.
    #[serde(skip)]
    pub(crate) convert_pages: OnceLock<Option<usize>>,
    /// A photo's or video's date, camera and size (see
    /// [`crate::photos::Inside`]), read on first use.
    #[serde(skip)]
    pub(crate) photo: OnceLock<Option<crate::photos::Inside>>,
}

impl FileEntry {
    /// Builds the entry from a path; `None` if the name is not valid UTF-8.
    pub fn from_path(path: &Path, meta: &Metadata) -> Option<Self> {
        let name = path.file_name()?.to_str()?;
        let is_dir = meta.is_dir();
        let (stem, ext) = if is_dir { (name, "") } else { split_name(name) };
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        Some(Self {
            path: path.to_path_buf(),
            base: dir.clone(),
            dir,
            stem: stem.to_string(),
            ext: ext.to_string(),
            size: if is_dir { 0 } else { meta.len() },
            modified,
            is_dir,
            taken: OnceLock::new(),
            tags: OnceLock::new(),
            comic: OnceLock::new(),
            video: OnceLock::new(),
            kind: OnceLock::new(),
            convert_pages: OnceLock::new(),
            photo: OnceLock::new(),
        })
    }

    /// Date the photo was taken, from its EXIF data (JPEG, HEIC, PNG, WebP,
    /// TIFF and RAW formats based on it). Read once and cached.
    pub fn taken(&self) -> Option<NaiveDateTime> {
        if self.is_dir {
            return None;
        }
        *self.taken.get_or_init(|| read_taken(&self.path))
    }

    /// A photo's or video's date, camera and size. Read once and cached.
    pub fn photo_inside(&self) -> Option<&crate::photos::Inside> {
        self.photo
            .get_or_init(|| crate::photos::read_inside(self))
            .as_ref()
    }

    /// Tags of an audio file (ID3, Vorbis comments, MP4…). Read once and
    /// cached.
    pub fn audio_tags(&self) -> Option<&AudioTags> {
        if self.is_dir || !crate::music::is_audio(&self.ext) {
            return None;
        }
        self.tags
            .get_or_init(|| crate::music::probe(&self.path))
            .as_ref()
    }

    /// A video's picture size and codec, from its headers. Read once and
    /// cached.
    pub fn video_info(&self) -> Option<&crate::video::VideoInfo> {
        if self.is_dir || !crate::tmdb::is_video(&self.ext) {
            return None;
        }
        self.video
            .get_or_init(|| crate::video::read(&self.path))
            .as_ref()
    }

    /// What the file's content really is. Read once and cached.
    pub fn kind(&self) -> Option<&crate::kind::Kind> {
        if self.is_dir {
            return None;
        }
        self.kind
            .get_or_init(|| crate::kind::read(&self.path))
            .as_ref()
    }

    /// A comic the comic rule can convert to CBZ (a PDF, EPUB or Kindle
    /// book): how many of its pages would be drawn; `None` when it cannot
    /// be. Checked once and cached.
    pub fn convert_check(&self) -> Option<usize> {
        *self
            .convert_pages
            .get_or_init(|| crate::convert::check(&self.path))
    }

    /// The `ComicInfo.xml` inside a comic file. Read once and cached.
    pub fn comic_info(&self) -> Option<&crate::comics::ComicInfo> {
        if !self.is_dir && !crate::comics::is_comic(&self.ext) {
            return None;
        }
        self.comic
            .get_or_init(|| crate::comics::read(&self.path))
            .as_ref()
    }

    /// After its tags (or a comic's details) were written, or the comic
    /// converted: they are read again, and the new size and date taken.
    pub fn forget_tags(&mut self) {
        self.tags = OnceLock::new();
        self.comic = OnceLock::new();
        // A comic converted to CBZ is now a zip.
        self.kind = OnceLock::new();
        self.convert_pages = OnceLock::new();
        if let Some(fresh) = std::fs::metadata(&self.path)
            .ok()
            .and_then(|m| Self::from_path(&self.path, &m))
        {
            // A folder of pictures converted is a file now.
            if fresh.is_dir != self.is_dir {
                *self = fresh;
                return;
            }
            self.size = fresh.size;
            self.modified = fresh.modified;
        }
    }

    /// Takes what `other` (a copy of this entry) read and this one has
    /// not, so reading done on a copy is not done again.
    pub fn keep_read(&mut self, other: &FileEntry) {
        if self.path != other.path || self.size != other.size || self.modified != other.modified {
            return;
        }
        if let (None, Some(t)) = (self.tags.get(), other.tags.get()) {
            let _ = self.tags.set(t.clone());
        }
        if let (None, Some(v)) = (self.video.get(), other.video.get()) {
            let _ = self.video.set(v.clone());
        }
        if let (None, Some(d)) = (self.taken.get(), other.taken.get()) {
            let _ = self.taken.set(*d);
        }
        if let (None, Some(p)) = (self.photo.get(), other.photo.get()) {
            let _ = self.photo.set(p.clone());
        }
        if let (None, Some(k)) = (self.kind.get(), other.kind.get()) {
            let _ = self.kind.set(k.clone());
        }
    }

    pub fn file_name(&self) -> String {
        join_name(&self.stem, &self.ext)
    }
}

/// Sends a file or folder to the system's trash. In a Flatpak, GLib's
/// `gio` asks the desktop's trash through its portal (the trash found
/// from inside would be the sandbox's own, which no file manager shows).
pub fn to_trash(path: &Path) -> Result<(), String> {
    if std::env::var_os("FLATPAK_ID").is_some() {
        let done = std::process::Command::new("gio")
            .arg("trash")
            .arg("--")
            .arg(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        return if done {
            Ok(())
        } else {
            Err("trashFailed: gio trash".into())
        };
    }
    trash::delete(path).map_err(|e| format!("trashFailed: {e}"))
}

fn read_taken(path: &Path) -> Option<NaiveDateTime> {
    taken_in(&read_exif(path)?)
}

/// A picture's EXIF data. JPEG and PNG are read up to their image data
/// only: the reader of the library goes through a whole JPEG without EXIF
/// looking for it, which on thousands of photos reads gigabytes.
pub(crate) fn read_exif(path: &Path) -> Option<exif::Exif> {
    let mut f = std::io::BufReader::new(std::fs::File::open(path).ok()?);
    let mut head = [0u8; 8];
    let n = f.read(&mut head).ok()?;
    f.rewind().ok()?;
    let raw = match &head[..n] {
        [0xff, 0xd8, ..] => jpeg_exif(&mut f),
        [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] => png_exif(&mut f),
        _ => return exif::Reader::new().read_from_container(&mut f).ok(),
    }?;
    exif::Reader::new().read_raw(raw).ok()
}

type Picture = std::io::BufReader<std::fs::File>;

/// A picture's width and height, from its header (the image library
/// reads a whole JPEG for them).
pub(crate) fn picture_size(path: &Path) -> Option<(u32, u32)> {
    let mut f = std::io::BufReader::new(std::fs::File::open(path).ok()?);
    let mut head = [0u8; 24];
    let n = f.read(&mut head).ok()?;
    let be = |i: usize| u32::from_be_bytes(head[i..i + 4].try_into().unwrap());
    match &head[..n] {
        [0xff, 0xd8, ..] => {
            // A frame's start: precision, height, width.
            let sof = |c: u8| matches!(c, 0xc0..=0xcf) && !matches!(c, 0xc4 | 0xc8 | 0xcc);
            jpeg_segment(&mut f, sof, |_, seg| {
                let at = |i: usize| {
                    Some(u32::from(u16::from_be_bytes([
                        *seg.get(i)?,
                        *seg.get(i + 1)?,
                    ])))
                };
                Some((at(3)?, at(1)?))
            })
        }
        [
            0x89,
            b'P',
            b'N',
            b'G',
            0x0d,
            0x0a,
            0x1a,
            0x0a,
            _,
            _,
            _,
            _,
            b'I',
            b'H',
            b'D',
            b'R',
            ..,
        ] if n == 24 => Some((be(16), be(20))),
        _ => image::ImageReader::new(f)
            .with_guessed_format()
            .ok()?
            .into_dimensions()
            .ok(),
    }
    .filter(|&(w, h)| w > 0 && h > 0)
}

/// The EXIF segment of a JPEG, looked for before the image data starts.
fn jpeg_exif(f: &mut Picture) -> Option<Vec<u8>> {
    jpeg_segment(
        f,
        |c| c == 0xe1,
        |_, seg| Some(seg.strip_prefix(b"Exif\0\0")?.to_vec()),
    )
}

/// Walks a JPEG's segments up to its image data, reading those `read`
/// accepts by their code, until `found` takes one.
fn jpeg_segment<T>(
    f: &mut Picture,
    read: impl Fn(u8) -> bool,
    mut found: impl FnMut(u8, &[u8]) -> Option<T>,
) -> Option<T> {
    let mut byte = [0u8; 1];
    let mut next = |f: &mut Picture| f.read_exact(&mut byte).ok().map(|()| byte[0]);
    f.seek(std::io::SeekFrom::Start(2)).ok()?;
    // Each segment: FF, its code, its length (with the two length bytes).
    for _ in 0..256 {
        if next(f)? != 0xff {
            return None;
        }
        let mut code = 0xff;
        while code == 0xff {
            code = next(f)?;
        }
        match code {
            0x01 | 0xd0..=0xd7 => continue,
            // The image data, or the end.
            0xd9 | 0xda => return None,
            _ => {}
        }
        let mut len = [0u8; 2];
        f.read_exact(&mut len).ok()?;
        let len = usize::from(u16::from_be_bytes(len)).checked_sub(2)?;
        if read(code) {
            let mut seg = vec![0; len];
            f.read_exact(&mut seg).ok()?;
            if let Some(t) = found(code, &seg) {
                return Some(t);
            }
        } else {
            f.seek_relative(len as i64).ok()?;
        }
    }
    None
}

/// The `eXIf` chunk of a PNG, looked for before the image data.
fn png_exif(f: &mut Picture) -> Option<Vec<u8>> {
    f.seek(std::io::SeekFrom::Start(8)).ok()?;
    for _ in 0..1024 {
        let mut head = [0u8; 8];
        f.read_exact(&mut head).ok()?;
        let len = u32::from_be_bytes(head[..4].try_into().ok()?);
        match &head[4..] {
            b"eXIf" => {
                let mut data = vec![0; len as usize];
                f.read_exact(&mut data).ok()?;
                return Some(data);
            }
            b"IDAT" | b"IEND" => return None,
            // The data and its check sum.
            _ => f.seek_relative(i64::from(len) + 4).ok()?,
        };
    }
    None
}

/// The date a photo was taken, from its EXIF data.
pub(crate) fn taken_in(data: &exif::Exif) -> Option<NaiveDateTime> {
    use exif::{In, Tag, Value};
    [Tag::DateTimeOriginal, Tag::DateTimeDigitized, Tag::DateTime]
        .iter()
        .find_map(|&tag| match &data.get_field(tag, In::PRIMARY)?.value {
            Value::Ascii(v) => {
                let d = exif::DateTime::from_ascii(v.first()?).ok()?;
                chrono::NaiveDate::from_ymd_opt(d.year.into(), d.month.into(), d.day.into())?
                    .and_hms_opt(d.hour.into(), d.minute.into(), d.second.into())
            }
            _ => None,
        })
}

/// Splits `name.ext` at the last dot. Hidden files (`.bashrc`) and names
/// ending in a dot have no extension.
pub fn split_name(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => (&name[..i], &name[i + 1..]),
        _ => (name, ""),
    }
}

pub fn join_name(stem: &str, ext: &str) -> String {
    if ext.is_empty() {
        stem.to_string()
    } else {
        format!("{stem}.{ext}")
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SortKey {
    Name,
    Ext,
    Dir,
    Modified,
    Size,
    // What the files hold (see [`crate::details`]).
    Artist,
    Album,
    Title,
    Track,
    Year,
    Genre,
    Length,
    Quality,
    Resolution,
    Codec,
    Taken,
}

pub fn sort_files(files: &mut [FileEntry], key: SortKey, descending: bool) {
    if crate::details::reads(key) {
        files.sort_by(|a, b| crate::details::compare(key, a, b, descending));
        return;
    }
    // sort_by is stable: ties keep their previous order.
    files.sort_by(|a, b| {
        let ord = match key {
            SortKey::Name => natural_cmp(&a.file_name(), &b.file_name()),
            SortKey::Ext => natural_cmp(&a.ext, &b.ext),
            SortKey::Dir => natural_cmp(&a.dir.to_string_lossy(), &b.dir.to_string_lossy()),
            SortKey::Modified => a.modified.cmp(&b.modified),
            SortKey::Size => a.size.cmp(&b.size),
            _ => Ordering::Equal,
        };
        if descending { ord.reverse() } else { ord }
    });
}

/// "Natural" comparison: runs of digits compare by value (`photo2 < photo10`)
/// and letters compare case-insensitively.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na = take_digits(&mut ai);
                let nb = take_digits(&mut bi);
                let ta = na.trim_start_matches('0');
                let tb = nb.trim_start_matches('0');
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                ai.next();
                bi.next();
            }
        }
    }
}

fn take_digits(it: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut s = String::new();
    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
        s.push(c);
        it.next();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picture_headers() {
        let d = tempfile::tempdir().unwrap();
        let img = image::RgbImage::new(40, 30);
        let mut jpg = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut jpg),
            image::ImageFormat::Jpeg,
        )
        .unwrap();
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        // EXIF after another segment, as cameras put a profile first.
        let mut tiff = b"II*\0\x08\0\0\0\x01\0\x32\x01\x02\0\x14\0\0\0\x1a\0\0\0\0\0\0\0".to_vec();
        tiff.extend_from_slice(b"2019:05:12 14:30:22\0");
        let mut with = vec![0xff, 0xd8, 0xff, 0xe2, 0, 6, 1, 2, 3, 4, 0xff, 0xe1];
        with.extend(((tiff.len() + 8) as u16).to_be_bytes());
        with.extend(b"Exif\0\0");
        with.extend(tiff);
        with.extend(&jpg[2..]);
        for (name, bytes) in [("a.jpg", &jpg), ("b.png", &png), ("c.jpg", &with)] {
            std::fs::write(d.path().join(name), bytes).unwrap();
        }
        let taken = |n: &str| read_exif(&d.path().join(n)).as_ref().and_then(taken_in);
        assert_eq!(taken("a.jpg"), None);
        assert_eq!(taken("b.png"), None);
        assert_eq!(taken("c.jpg").unwrap().to_string(), "2019-05-12 14:30:22");
        for n in ["a.jpg", "b.png", "c.jpg"] {
            assert_eq!(picture_size(&d.path().join(n)), Some((40, 30)), "{n}");
        }
    }

    #[test]
    fn split() {
        assert_eq!(split_name("a.tar.gz"), ("a.tar", "gz"));
        assert_eq!(split_name(".bashrc"), (".bashrc", ""));
        assert_eq!(split_name("semext"), ("semext", ""));
        assert_eq!(split_name("ponto."), ("ponto.", ""));
    }

    #[test]
    fn natural() {
        let mut v = vec!["foto10", "Foto2", "foto1", "foto02b", "abc"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["abc", "foto1", "Foto2", "foto02b", "foto10"]);
    }
}
