//! Pictures that look alike: the same photo resized, recompressed, saved
//! in another format or lightly edited, and shots taken a moment apart.
//!
//! Each picture is reduced to a 64-bit fingerprint of its shapes: shrunk
//! to 32×32 greys, turned the way it is shown (EXIF orientation), its
//! lowest 8×8 frequencies taken (a discrete cosine transform), and one
//! bit per frequency for being above their median. Pictures whose
//! fingerprints differ in few bits look alike.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::files::{FileEntry, natural_cmp};
use crate::identical::{Stamp, on_disk};

/// Fingerprints this many bits apart, or fewer, look alike.
pub const NEAR: u32 = 8;

/// Pictures larger than this are not read.
const MAX_BYTES: u64 = 256 << 20;

/// Side of the grid the picture is shrunk to.
const N: usize = 32;

/// Pictures decoded here.
const DECODED: [&str; 8] = ["jpg", "jpeg", "jpe", "png", "webp", "gif", "bmp", "tif"];

/// Pictures compared by the small copy their EXIF data carries, when there
/// is one (camera raw files; HEIC rarely has one).
const BY_THUMBNAIL: [&str; 12] = [
    "heic", "heif", "dng", "cr2", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "pef",
];

/// Whether `ext` (lower case) is a picture this module can fingerprint.
pub fn is_picture(ext: &str) -> bool {
    let ext = if ext == "tiff" { "tif" } else { ext };
    DECODED.contains(&ext) || BY_THUMBNAIL.contains(&ext)
}

/// Fingerprints by path, while the file's size and modification time stay
/// the same. Kept in a file between sessions when made with
/// [`PictureCache::in_file`]; one not used for [`FORGET_DAYS`] is dropped.
#[derive(Default)]
pub struct PictureCache {
    entries: HashMap<PathBuf, Known>,
    file: Option<PathBuf>,
    loaded: bool,
    changed: bool,
}

/// Days a fingerprint not used is kept in the file.
pub const FORGET_DAYS: u32 = 90;

#[derive(Clone, Copy)]
struct Known {
    stamp: Stamp,
    hash: Option<u64>,
    /// Day it was last used (days since 1970).
    used: u32,
}

fn today() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| (d.as_secs() / 86_400) as u32)
}

impl PictureCache {
    /// A cache kept in `file` (read when first needed).
    pub fn in_file(file: PathBuf) -> Self {
        Self {
            file: Some(file),
            ..Self::default()
        }
    }

    /// One line per picture: hash (or `-`), size, modification time
    /// (seconds, nanoseconds), day last used, path.
    fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let Some(text) = self
            .file
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
        else {
            return;
        };
        let oldest = today().saturating_sub(FORGET_DAYS);
        for line in text.lines() {
            let mut parts = line.splitn(6, '\t');
            let (Some(hash), Some(size), Some(secs), Some(nanos), Some(used), Some(path)) = (
                parts.next(),
                parts.next(),
                parts.next(),
                parts.next(),
                parts.next(),
                parts.next(),
            ) else {
                continue;
            };
            let (Ok(size), Ok(secs), Ok(nanos), Ok(used)) = (
                size.parse(),
                secs.parse(),
                nanos.parse(),
                used.parse::<u32>(),
            ) else {
                continue;
            };
            let hash = match hash {
                "-" => None,
                h => match u64::from_str_radix(h, 16) {
                    Ok(h) => Some(h),
                    Err(_) => continue,
                },
            };
            if used < oldest {
                self.changed = true;
                continue;
            }
            let modified = UNIX_EPOCH.checked_add(Duration::new(secs, nanos));
            let stamp = Stamp { size, modified };
            self.entries
                .insert(PathBuf::from(path), Known { stamp, hash, used });
        }
    }

    /// Writes the cache to its file, when it changed.
    pub fn save(&mut self) {
        let Some(file) = self.file.clone() else {
            return;
        };
        if !self.changed {
            return;
        }
        let mut out = String::new();
        for (path, k) in &self.entries {
            let (Some(path), Some(t)) = (
                path.to_str(),
                k.stamp
                    .modified
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok()),
            ) else {
                continue;
            };
            if path.contains(['\n', '\r']) {
                continue;
            }
            let hash = k.hash.map_or("-".to_string(), |h| format!("{h:016x}"));
            out.push_str(&format!(
                "{hash}\t{}\t{}\t{}\t{}\t{path}\n",
                k.stamp.size,
                t.as_secs(),
                t.subsec_nanos(),
                k.used
            ));
        }
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let part = file.with_extension("part");
        if std::fs::write(&part, out).is_ok() && std::fs::rename(&part, &file).is_ok() {
            self.changed = false;
        }
    }

    fn get(&mut self, path: &Path, stamp: Stamp) -> Option<Option<u64>> {
        let day = today();
        let k = self.entries.get_mut(path).filter(|k| k.stamp == stamp)?;
        if k.used != day {
            k.used = day;
            self.changed = true;
        }
        Some(k.hash)
    }

    fn put(&mut self, path: PathBuf, stamp: Stamp, hash: Option<u64>) {
        let used = today();
        self.entries.insert(path, Known { stamp, hash, used });
        self.changed = true;
    }
}

/// Reports progress: pictures read and pictures to read.
pub type Progress<'a> = &'a mut dyn FnMut(u64, u64);

/// How many bits two fingerprints differ in.
pub fn distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Groups of pictures that look alike (two or more each, sorted by path;
/// the groups by their first path). A group made only of identical copies
/// of one file is left out (they are shown as copies already). `None`
/// when `cancelled` returned true.
pub fn similar_pictures(
    files: &[FileEntry],
    identical: &HashMap<PathBuf, usize>,
    cache: &mut PictureCache,
    progress: Progress,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Option<Vec<Vec<PathBuf>>> {
    cache.load();
    let mut known: Vec<(&FileEntry, u64)> = Vec::new();
    let mut todo: Vec<(&FileEntry, Stamp)> = Vec::new();
    for f in files {
        if f.is_dir || f.size == 0 || f.size > MAX_BYTES || !is_picture(&f.ext.to_lowercase()) {
            continue;
        }
        let Some((stamp, _)) = on_disk(&f.path) else {
            continue;
        };
        match cache.get(&f.path, stamp) {
            Some(Some(h)) => known.push((f, h)),
            Some(None) => {}
            None => todo.push((f, stamp)),
        }
    }

    // Read what is not known yet, a few at a time.
    let total = todo.len() as u64;
    if total > 0 {
        progress(0, total);
        let next = AtomicU64::new(0);
        let workers = std::thread::available_parallelism()
            .map_or(2, |n| n.get() / 2)
            .clamp(1, 6)
            .min(todo.len());
        let (tx, rx) = std::sync::mpsc::channel();
        let found = std::thread::scope(|scope| {
            for _ in 0..workers {
                let tx = tx.clone();
                let (todo, next) = (&todo, &next);
                scope.spawn(move || {
                    loop {
                        let i = next.fetch_add(1, Ordering::SeqCst) as usize;
                        if i >= todo.len() || cancelled() {
                            break;
                        }
                        let f = todo[i].0;
                        let hash = std::fs::read(&f.path)
                            .ok()
                            .and_then(|b| fingerprint(&b, &f.ext.to_lowercase()));
                        if tx.send((i, hash)).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(tx);
            let mut found = Vec::with_capacity(todo.len());
            for (i, hash) in rx {
                found.push((i, hash));
                progress(found.len() as u64, total);
            }
            found
        });
        if cancelled() {
            return None;
        }
        for (i, hash) in found {
            let (f, stamp) = todo[i];
            cache.put(f.path.clone(), stamp, hash);
            if let Some(h) = hash {
                known.push((f, h));
            }
        }
    }

    // Join each pair that is near (a picture alike two others joins all
    // three).
    let mut parent: Vec<usize> = (0..known.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for i in 0..known.len() {
        if i % 256 == 0 && cancelled() {
            return None;
        }
        for j in i + 1..known.len() {
            if distance(known[i].1, known[j].1) <= NEAR {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                if a != b {
                    parent[b] = a;
                }
            }
        }
    }
    let mut by_root: HashMap<usize, Vec<&FileEntry>> = HashMap::new();
    for (i, (f, _)) in known.iter().enumerate() {
        let r = root(&mut parent, i);
        by_root.entry(r).or_default().push(*f);
    }
    let mut groups: Vec<Vec<PathBuf>> = Vec::new();
    for g in by_root.into_values().filter(|g| g.len() > 1) {
        let first = identical.get(&g[0].path);
        if first.is_some() && g.iter().all(|f| identical.get(&f.path) == first) {
            continue;
        }
        let mut paths: Vec<PathBuf> = g.iter().map(|f| f.path.clone()).collect();
        paths.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
        groups.push(paths);
    }
    groups.sort_by(|a, b| natural_cmp(&a[0].to_string_lossy(), &b[0].to_string_lossy()));
    Some(groups)
}

/// The fingerprint of a picture's bytes, `ext` telling its kind (lower
/// case). `None` when it cannot be read, or is one flat colour (nothing
/// to compare).
pub fn fingerprint(bytes: &[u8], ext: &str) -> Option<u64> {
    let ext = if ext == "tiff" { "tif" } else { ext };
    let turn = orientation(bytes);
    let grid = if BY_THUMBNAIL.contains(&ext) {
        greys(thumbnail(bytes)?.as_slice())?
    } else {
        greys(bytes)?
    };
    hash(&turned(&grid, turn))
}

/// The fingerprint of a picture already decoded (pages of comics, covers).
pub fn fingerprint_image(img: &image::DynamicImage) -> Option<u64> {
    let g = img.to_luma8();
    let (w, h) = (g.width() as usize, g.height() as usize);
    hash(&shrink(g.as_raw(), w, h)?)
}

/// The picture shrunk to N×N greys (0–255), as stored (not turned).
fn greys(bytes: &[u8]) -> Option<Vec<f32>> {
    if bytes.starts_with(&[0xFF, 0xD8])
        && let Some(g) = jpeg_greys(bytes)
    {
        return Some(g);
    }
    let img = image::load_from_memory(bytes).ok()?;
    let g = img.to_luma8();
    let (w, h) = (g.width() as usize, g.height() as usize);
    shrink(g.as_raw(), w, h)
}

/// A JPEG decoded at the smallest scale that still gives N pixels a side
/// (much faster than decoding it whole).
fn jpeg_greys(bytes: &[u8]) -> Option<Vec<f32>> {
    std::panic::catch_unwind(|| {
        let mut d = turbojpeg::Decompressor::new().ok()?;
        let head = d.read_header(bytes).ok()?;
        let side = head.width.min(head.height);
        let scale = [8, 4, 2].into_iter().find(|&k| side / k >= N).unwrap_or(1);
        d.set_scaling_factor(turbojpeg::ScalingFactor::new(1, scale))
            .ok()?;
        let head = head.scaled(turbojpeg::ScalingFactor::new(1, scale));
        let (w, h) = (head.width, head.height);
        let mut pixels = vec![0u8; w * h];
        let out = turbojpeg::Image {
            pixels: pixels.as_mut_slice(),
            width: w,
            pitch: w,
            height: h,
            format: turbojpeg::PixelFormat::GRAY,
        };
        d.decompress(bytes, out).ok()?;
        shrink(&pixels, w, h)
    })
    .ok()
    .flatten()
}

/// Greys `w`×`h` averaged into N×N cells.
fn shrink(pixels: &[u8], w: usize, h: usize) -> Option<Vec<f32>> {
    if w == 0 || h == 0 || pixels.len() < w * h {
        return None;
    }
    let mut out = vec![0f32; N * N];
    for cy in 0..N {
        let (y0, y1) = (cy * h / N, ((cy + 1) * h / N).max(cy * h / N + 1).min(h));
        for cx in 0..N {
            let (x0, x1) = (cx * w / N, ((cx + 1) * w / N).max(cx * w / N + 1).min(w));
            let mut sum = 0u64;
            for y in y0..y1 {
                let row = &pixels[y * w..y * w + w];
                sum += row[x0..x1].iter().map(|&p| u64::from(p)).sum::<u64>();
            }
            out[cy * N + cx] = sum as f32 / ((y1 - y0) * (x1 - x0)) as f32;
        }
    }
    Some(out)
}

/// The grid turned the way the EXIF orientation (1–8) says it is shown.
fn turned(g: &[f32], orientation: u16) -> Vec<f32> {
    let m = N - 1;
    let mut out = vec![0f32; N * N];
    for y in 0..N {
        for x in 0..N {
            let (sy, sx) = match orientation {
                2 => (y, m - x),
                3 => (m - y, m - x),
                4 => (m - y, x),
                5 => (x, y),
                6 => (m - x, y),
                7 => (m - x, m - y),
                8 => (x, m - y),
                _ => (y, x),
            };
            out[y * N + x] = g[sy * N + sx];
        }
    }
    out
}

/// cos((2x+1)uπ/2N) for the 8 lowest frequencies `u`.
static COS: LazyLock<[[f32; N]; 8]> = LazyLock::new(|| {
    let mut c = [[0f32; N]; 8];
    for (u, row) in c.iter_mut().enumerate() {
        for (x, v) in row.iter_mut().enumerate() {
            *v = ((2 * x + 1) as f32 * u as f32 * PI / (2 * N) as f32).cos();
        }
    }
    c
});

/// One bit per low frequency (but the average) above their median.
fn hash(g: &[f32]) -> Option<u64> {
    // Rows first, then columns: only the 8×8 lowest frequencies.
    let mut rows = [[0f32; 8]; N];
    for (y, r) in rows.iter_mut().enumerate() {
        for (u, v) in r.iter_mut().enumerate() {
            *v = (0..N).map(|x| g[y * N + x] * COS[u][x]).sum();
        }
    }
    let mut f = [0f32; 64];
    for v in 0..8 {
        for u in 0..8 {
            f[v * 8 + u] = (0..N).map(|y| rows[y][u] * COS[v][y]).sum();
        }
    }
    let mut ac: Vec<f32> = f[1..].to_vec();
    // A flat picture: its frequencies are rounding noise.
    if ac.iter().all(|v| v.abs() < (N * N) as f32 * 0.5) {
        return None;
    }
    ac.sort_by(|a, b| a.total_cmp(b));
    let median = ac[ac.len() / 2];
    Some(
        f.iter()
            .enumerate()
            .skip(1)
            .filter(|(_, v)| **v > median)
            .fold(0u64, |h, (i, _)| h | 1 << i),
    )
}

/// The EXIF orientation (1 when none).
fn orientation(bytes: &[u8]) -> u16 {
    exif_of(bytes)
        .and_then(|e| {
            e.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?
                .value
                .get_uint(0)
        })
        .and_then(|v| u16::try_from(v).ok())
        .unwrap_or(1)
}

fn exif_of(bytes: &[u8]) -> Option<exif::Exif> {
    exif::Reader::new()
        .read_from_container(&mut std::io::Cursor::new(bytes))
        .ok()
}

/// The small JPEG copy kept in the EXIF data.
fn thumbnail(bytes: &[u8]) -> Option<Vec<u8>> {
    let e = exif_of(bytes)?;
    let at = |tag| {
        e.get_field(tag, exif::In::THUMBNAIL)?
            .value
            .get_uint(0)
            .map(|v| v as usize)
    };
    let start = at(exif::Tag::JPEGInterchangeFormat)?;
    let len = at(exif::Tag::JPEGInterchangeFormatLength)?;
    let jpeg = e.buf().get(start..start.checked_add(len)?)?;
    jpeg.starts_with(&[0xFF, 0xD8]).then(|| jpeg.to_vec())
}

/// Width and height in pixels.
pub type Size = (u32, u32);

/// A smaller copy of a picture to show: JPEG, its longest side at most
/// `max`, turned the way the picture is shown; and the picture's own
/// width and height as shown, when known. Camera raw and HEIC files show
/// the small copy in their EXIF data.
pub fn shown_copy(bytes: &[u8], ext: &str, max: u32) -> Option<(Vec<u8>, Option<Size>)> {
    let ext = if ext == "tiff" { "tif" } else { ext };
    let turn = orientation(bytes);
    let (img, dims) = if BY_THUMBNAIL.contains(&ext) {
        let small = thumbnail(bytes)?;
        let dims = exif_of(bytes).and_then(|e| {
            let get = |tag| e.get_field(tag, exif::In::PRIMARY)?.value.get_uint(0);
            Some((
                get(exif::Tag::PixelXDimension)?,
                get(exif::Tag::PixelYDimension)?,
            ))
        });
        (image::load_from_memory(&small).ok()?.to_rgb8(), dims)
    } else if bytes.starts_with(&[0xFF, 0xD8])
        && let Some(found) = jpeg_rgb(bytes, max)
    {
        found
    } else {
        let img = image::load_from_memory(bytes).ok()?;
        let dims = (img.width(), img.height());
        (img.to_rgb8(), Some(dims))
    };
    let (w, h) = img.dimensions();
    let scale = (max as f32 / w.max(h) as f32).min(1.0);
    let small = if scale < 1.0 {
        let (nw, nh) = (
            ((w as f32 * scale).round() as u32).max(1),
            ((h as f32 * scale).round() as u32).max(1),
        );
        image::imageops::thumbnail(&img, nw, nh)
    } else {
        img
    };
    use image::imageops::{flip_horizontal, flip_vertical, rotate90, rotate180, rotate270};
    let shown = match turn {
        2 => flip_horizontal(&small),
        3 => rotate180(&small),
        4 => flip_vertical(&small),
        5 => flip_horizontal(&rotate90(&small)),
        6 => rotate90(&small),
        7 => flip_horizontal(&rotate270(&small)),
        8 => rotate270(&small),
        _ => small,
    };
    let dims = dims.map(|(w, h)| {
        if (5..=8).contains(&turn) {
            (h, w)
        } else {
            (w, h)
        }
    });
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82)
        .encode_image(&shown)
        .ok()?;
    Some((out, dims))
}

/// A JPEG decoded at the smallest scale that still gives `max` pixels on
/// its longest side, and its full size.
fn jpeg_rgb(bytes: &[u8], max: u32) -> Option<(image::RgbImage, Option<Size>)> {
    std::panic::catch_unwind(|| {
        let mut d = turbojpeg::Decompressor::new().ok()?;
        let head = d.read_header(bytes).ok()?;
        let full = (head.width as u32, head.height as u32);
        let side = head.width.max(head.height);
        let scale = [8, 4, 2]
            .into_iter()
            .find(|&k| side / k >= max as usize)
            .unwrap_or(1);
        let factor = turbojpeg::ScalingFactor::new(1, scale);
        d.set_scaling_factor(factor).ok()?;
        let head = head.scaled(factor);
        let (w, h) = (head.width, head.height);
        let mut pixels = vec![0u8; w * h * 3];
        let out = turbojpeg::Image {
            pixels: pixels.as_mut_slice(),
            width: w,
            pitch: w * 3,
            height: h,
            format: turbojpeg::PixelFormat::RGB,
        };
        d.decompress(bytes, out).ok()?;
        let img = image::RgbImage::from_raw(w as u32, h as u32, pixels)?;
        Some((img, Some(full)))
    })
    .ok()
    .flatten()
}

/// Path of a picture, for tests and callers that hold only a path.
pub fn fingerprint_file(path: &Path) -> Option<u64> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    fingerprint(&std::fs::read(path).ok()?, &ext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, ImageFormat, Rgb, RgbImage};

    /// A made-up scene: soft shapes, different for each `seed`.
    fn scene(w: u32, h: u32, seed: u32) -> RgbImage {
        ImageBuffer::from_fn(w, h, |x, y| {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let s = seed as f32;
            let a = ((fx * (3.0 + s) + s).sin() * (fy * (2.0 + s * 0.7)).cos() + 1.0) * 110.0;
            let b = if (fx - 0.3 - s * 0.05).powi(2) + (fy - 0.6).powi(2) < 0.04 {
                60.0
            } else {
                0.0
            };
            let v = (a + b).min(255.0) as u8;
            Rgb([v, v / 2 + 40, 255 - v])
        })
    }

    fn encode(img: &RgbImage, format: ImageFormat) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, format).unwrap();
        out.into_inner()
    }

    fn jpeg(img: &RgbImage, quality: u8) -> Vec<u8> {
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality)
            .encode_image(img)
            .unwrap();
        out
    }

    #[test]
    fn the_same_picture_resized_and_recompressed_is_near() {
        let big = scene(1600, 1200, 1);
        let small = image::imageops::resize(&big, 400, 300, image::imageops::FilterType::Triangle);
        let a = fingerprint(&jpeg(&big, 92), "jpg").unwrap();
        let b = fingerprint(&jpeg(&small, 40), "jpg").unwrap();
        let c = fingerprint(&encode(&small, ImageFormat::Png), "png").unwrap();
        assert!(distance(a, b) <= NEAR, "{}", distance(a, b));
        assert!(distance(a, c) <= NEAR, "{}", distance(a, c));
        // Somewhat brighter.
        let lighter = ImageBuffer::from_fn(400, 300, |x, y| {
            let p = small.get_pixel(x, y);
            Rgb(p.0.map(|v| v.saturating_add(25)))
        });
        let d = fingerprint(&jpeg(&lighter, 85), "jpg").unwrap();
        assert!(distance(a, d) <= NEAR, "{}", distance(a, d));
    }

    #[test]
    fn different_pictures_are_far() {
        let hashes: Vec<u64> = (1..6)
            .map(|s| fingerprint(&jpeg(&scene(800, 600, s * 3), 90), "jpg").unwrap())
            .collect();
        for i in 0..hashes.len() {
            for j in i + 1..hashes.len() {
                assert!(distance(hashes[i], hashes[j]) > NEAR, "{i} {j}");
            }
        }
    }

    #[test]
    fn a_flat_picture_has_no_fingerprint() {
        let flat: RgbImage = ImageBuffer::from_pixel(300, 200, Rgb([30, 30, 30]));
        assert_eq!(fingerprint(&encode(&flat, ImageFormat::Png), "png"), None);
    }

    #[test]
    fn turning_follows_the_orientation() {
        let g: Vec<f32> = (0..N * N).map(|i| i as f32).collect();
        // Turned a quarter clockwise: the first row is the first column,
        // from the bottom up.
        let t = turned(&g, 6);
        assert_eq!(t[0], g[(N - 1) * N]);
        assert_eq!(t[N - 1], g[0]);
        // Turned back by 8.
        assert_eq!(turned(&t, 8), g);
    }

    #[test]
    fn groups_join_near_pictures_only() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, bytes: Vec<u8>| {
            let p = dir.path().join(name);
            std::fs::write(&p, bytes).unwrap();
            let meta = std::fs::symlink_metadata(&p).unwrap();
            FileEntry::from_path(&p, &meta).unwrap()
        };
        let big = scene(1200, 900, 2);
        let small = image::imageops::resize(&big, 300, 225, image::imageops::FilterType::Triangle);
        let files = vec![
            write("a.jpg", jpeg(&big, 90)),
            write("b.png", encode(&small, ImageFormat::Png)),
            write("c.jpg", jpeg(&scene(1200, 900, 11), 90)),
            write("d.jpg", jpeg(&big, 90)),
        ];
        let mut cache = PictureCache::default();
        let groups = similar_pictures(&files, &HashMap::new(), &mut cache, &mut |_, _| {}, &|| {
            false
        })
        .unwrap();
        assert_eq!(groups.len(), 1);
        let names: Vec<_> = groups[0]
            .iter()
            .map(|p| p.file_name().unwrap().to_owned())
            .collect();
        assert_eq!(names, ["a.jpg", "b.png", "d.jpg"]);
        // Only identical copies (a, d): left out.
        let identical: HashMap<PathBuf, usize> =
            [(files[0].path.clone(), 0), (files[3].path.clone(), 0)].into();
        let only = similar_pictures(
            &files[..1]
                .iter()
                .chain(&files[3..])
                .cloned()
                .collect::<Vec<_>>(),
            &identical,
            &mut cache,
            &mut |_, _| {},
            &|| false,
        )
        .unwrap();
        assert!(only.is_empty());
    }

    #[test]
    fn the_shown_copy_is_small_and_turned() {
        let big = scene(1600, 1200, 5);
        let (copy, dims) = shown_copy(&jpeg(&big, 90), "jpg", 400).unwrap();
        assert_eq!(dims, Some((1600, 1200)));
        let shown = image::load_from_memory(&copy).unwrap();
        assert_eq!((shown.width(), shown.height()), (400, 300));
        let (copy, dims) =
            shown_copy(&encode(&scene(300, 200, 5), ImageFormat::Png), "png", 400).unwrap();
        assert_eq!(dims, Some((300, 200)));
        assert_eq!(image::load_from_memory(&copy).unwrap().width(), 300);
    }

    #[test]
    fn the_cache_is_kept_in_a_file() {
        let dir = tempfile::tempdir().unwrap();
        // A tab in the name tests the cache's line format; Windows refuses it.
        let pic = dir
            .path()
            .join(if cfg!(windows) { "a b.png" } else { "a\tb.png" });
        std::fs::write(&pic, encode(&scene(200, 150, 4), ImageFormat::Png)).unwrap();
        let flat = dir.path().join("flat.png");
        let grey: RgbImage = ImageBuffer::from_pixel(50, 50, Rgb([9, 9, 9]));
        std::fs::write(&flat, encode(&grey, ImageFormat::Png)).unwrap();
        let entry =
            |p: &Path| FileEntry::from_path(p, &std::fs::symlink_metadata(p).unwrap()).unwrap();
        let files = [entry(&pic), entry(&flat)];
        let file = dir.path().join("cache").join("pictures");
        let mut cache = PictureCache::in_file(file.clone());
        similar_pictures(&files, &HashMap::new(), &mut cache, &mut |_, _| {}, &|| {
            false
        })
        .unwrap();
        cache.save();
        let mut again = PictureCache::in_file(file.clone());
        again.load();
        let stamp = on_disk(&pic).unwrap().0;
        assert_eq!(again.get(&pic, stamp), Some(fingerprint_file(&pic)));
        assert_eq!(again.get(&flat, on_disk(&flat).unwrap().0), Some(None));
        // Changed since: read again.
        std::fs::write(&pic, encode(&scene(200, 150, 9), ImageFormat::Png)).unwrap();
        assert_eq!(again.get(&pic, on_disk(&pic).unwrap().0), None);
        // Not used for long: dropped when read.
        let old = std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .map(|l| {
                let mut p: Vec<&str> = l.splitn(6, '\t').collect();
                p[4] = "1";
                p.join("\t") + "\n"
            })
            .collect::<String>();
        std::fs::write(&file, old).unwrap();
        let mut stale = PictureCache::in_file(file);
        stale.load();
        assert!(stale.entries.is_empty());
    }
}
