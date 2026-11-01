//! What the optional columns of the list show about each file: a song's
//! tags and audio, a video's picture, a photo's date. Read from the
//! file's cached details (see [`FileEntry`]), so a column costs one
//! reading per file.

use std::cmp::Ordering;

use serde::Serialize;

use crate::files::{FileEntry, SortKey, natural_cmp};

/// A file's values for the optional columns; empty when it has none.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    pub artist: String,
    pub album: String,
    pub title: String,
    /// `5`, or `2-05` on an album of several discs.
    pub track: String,
    pub year: Option<u16>,
    pub genre: String,
    /// Seconds.
    pub length: Option<u32>,
    /// `MP3 · 320 kbps`, `FLAC · 24/96`.
    pub quality: String,
    pub resolution: String,
    pub codec: String,
    /// When the photo was taken, as Unix seconds of its local time.
    pub taken: Option<i64>,
}

pub fn of(f: &FileEntry) -> Details {
    let mut d = Details::default();
    if let Some(t) = f.audio_tags() {
        d.artist = t.artist.clone();
        d.album = t.album.clone();
        d.title = t.title.clone();
        d.track = match (t.disc, t.track) {
            (Some(disc), Some(n)) if disc > 1 || t.disc_total.is_some_and(|n| n > 1) => {
                format!("{disc}-{n:02}")
            }
            (_, Some(n)) => n.to_string(),
            _ => String::new(),
        };
        d.year = t.year;
        d.genre = t.genre.clone();
        d.length = t.duration;
        d.quality = quality(&t.audio);
    }
    if let Some(v) = f.video_info() {
        d.resolution = v.resolution();
        d.codec = v.codec.clone();
    }
    d.taken = taken(f);
    d
}

/// When the photo was taken; other files are not opened for it.
fn taken(f: &FileEntry) -> Option<i64> {
    const PHOTOS: [&str; 18] = [
        "jpg", "jpeg", "heic", "heif", "png", "webp", "tif", "tiff", "dng", "cr2", "cr3", "nef",
        "arw", "orf", "rw2", "raf", "pef", "srw",
    ];
    let ext = f.ext.to_lowercase();
    if !PHOTOS.contains(&ext.as_str()) {
        return None;
    }
    f.taken().map(|t| t.and_utc().timestamp())
}

/// The format with its bitrate (lossy) or its bits and sample rate
/// (lossless, when the file says them).
fn quality(a: &crate::music::AudioInfo) -> String {
    let khz = a.sample_rate.map(|r| {
        let k = r as f64 / 1000.0;
        if k.fract() == 0.0 {
            format!("{k}")
        } else {
            format!("{k:.1}")
        }
    });
    let detail = match (a.bits, khz, a.bitrate) {
        (Some(bits), Some(k), _) => format!("{bits}/{k}"),
        (_, _, Some(b)) => format!("{b} kbps"),
        _ => String::new(),
    };
    match (a.format.is_empty(), detail.is_empty()) {
        (_, true) => a.format.clone(),
        (true, false) => detail,
        _ => format!("{} · {detail}", a.format),
    }
}

/// Whether the key sorts by what the files hold, which reads each file.
pub fn reads(key: SortKey) -> bool {
    !matches!(
        key,
        SortKey::Name | SortKey::Ext | SortKey::Dir | SortKey::Modified | SortKey::Size
    )
}

/// Compares two files by a column of details. Files without the value
/// go last either way; ties go by album and track, so sorting by artist
/// or year keeps each album in order.
pub fn compare(key: SortKey, a: &FileEntry, b: &FileEntry, descending: bool) -> Ordering {
    let (x, y) = (Sortable::of(key, a), Sortable::of(key, b));
    let main = match (&x.main, &y.main) {
        (Value::None, Value::None) => Ordering::Equal,
        (Value::None, _) => return Ordering::Greater,
        (_, Value::None) => return Ordering::Less,
        (p, q) => {
            let o = p.cmp(q);
            if descending { o.reverse() } else { o }
        }
    };
    main.then_with(|| natural_cmp(&x.album, &y.album))
        .then(x.disc.cmp(&y.disc))
        .then(x.track.cmp(&y.track))
}

#[derive(PartialEq, Eq)]
enum Value {
    None,
    Text(String),
    Number(u64),
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Value::Text(a), Value::Text(b)) => natural_cmp(a, b),
            (Value::Number(a), Value::Number(b)) => a.cmp(b),
            _ => Ordering::Equal,
        }
    }
}

struct Sortable {
    main: Value,
    album: String,
    disc: u32,
    track: u32,
}

impl Sortable {
    fn of(key: SortKey, f: &FileEntry) -> Self {
        let tags = f.audio_tags();
        let text = |s: Option<&str>| match s {
            Some(s) if !s.is_empty() => Value::Text(s.to_string()),
            _ => Value::None,
        };
        let number = |n: Option<u64>| n.map_or(Value::None, Value::Number);
        let main = match key {
            SortKey::Artist => text(tags.map(|t| t.artist.as_str())),
            SortKey::Album => text(tags.map(|t| t.album.as_str())),
            SortKey::Title => text(tags.map(|t| t.title.as_str())),
            SortKey::Genre => text(tags.map(|t| t.genre.as_str())),
            SortKey::Track => number(tags.and_then(|t| {
                t.track
                    .map(|n| u64::from(t.disc.unwrap_or(1)) << 32 | u64::from(n))
            })),
            SortKey::Year => number(tags.and_then(|t| t.year).map(u64::from)),
            SortKey::Length => number(tags.and_then(|t| t.duration).map(u64::from)),
            SortKey::Quality => match tags {
                // Lossless (bits) above lossy, then by bitrate.
                Some(t) if t.audio.bits.is_some() || t.audio.bitrate.is_some() => Value::Number(
                    u64::from(t.audio.bits.unwrap_or(0)) << 48
                        | u64::from(t.audio.sample_rate.unwrap_or(0)) << 24
                        | u64::from(t.audio.bitrate.unwrap_or(0)),
                ),
                _ => Value::None,
            },
            SortKey::Resolution => number(
                f.video_info()
                    .map(|v| u64::from(v.width) * u64::from(v.height))
                    .filter(|&n| n > 0),
            ),
            SortKey::Codec => text(f.video_info().map(|v| v.codec.as_str())),
            SortKey::Taken => number(taken(f).map(|t| t.max(0) as u64)),
            _ => Value::None,
        };
        // The ties: by album, then disc and track (by disc and track
        // only when sorting by album; none when by track or title).
        let (album, disc, track) = match (key, tags) {
            (SortKey::Track | SortKey::Title, _) | (_, None) => (String::new(), 0, 0),
            (_, Some(t)) => (
                if key == SortKey::Album {
                    String::new()
                } else {
                    t.album.clone()
                },
                t.disc.unwrap_or(1),
                t.track.unwrap_or(0),
            ),
        };
        Sortable {
            main,
            album,
            disc,
            track,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::AudioInfo;

    #[test]
    fn qualities() {
        let q = |format: &str, bitrate, sample_rate, bits| {
            quality(&AudioInfo {
                format: format.into(),
                bitrate,
                sample_rate,
                bits,
            })
        };
        assert_eq!(q("MP3", Some(320), Some(44100), None), "MP3 · 320 kbps");
        assert_eq!(q("FLAC", Some(900), Some(96000), Some(24)), "FLAC · 24/96");
        assert_eq!(
            q("FLAC", Some(900), Some(44100), Some(16)),
            "FLAC · 16/44.1"
        );
        assert_eq!(q("OGG", None, None, None), "OGG");
    }

    #[test]
    fn sorts_by_tags() {
        let song = |name: &str, artist: &str, album: &str, track| {
            let f = FileEntry {
                path: name.into(),
                stem: name.into(),
                ext: "mp3".into(),
                ..FileEntry::default()
            };
            let t = (!artist.is_empty()).then(|| crate::music::AudioTags {
                artist: artist.into(),
                album: album.into(),
                track: Some(track),
                ..Default::default()
            });
            f.tags.set(t).unwrap();
            f
        };
        let mut files = vec![
            song("a", "Zeta", "One", 2),
            song("b", "Alfa", "Two", 1),
            song("c", "Zeta", "One", 1),
            song("d", "", "", 0),
        ];
        let names = |files: &[FileEntry]| files.iter().map(|f| f.stem.clone()).collect::<Vec<_>>();
        crate::sort_files(&mut files, SortKey::Artist, false);
        assert_eq!(names(&files), ["b", "c", "a", "d"]);
        crate::sort_files(&mut files, SortKey::Artist, true);
        assert_eq!(names(&files), ["c", "a", "b", "d"]);
        crate::sort_files(&mut files, SortKey::Track, false);
        assert_eq!(names(&files), ["c", "b", "a", "d"]);
        assert_eq!(of(&files[0]).track, "1");
        assert_eq!(of(&files[3]), Details::default());
    }
}
