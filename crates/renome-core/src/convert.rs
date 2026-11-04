//! Comics turned into CBZ: a CBR (RAR), CB7 (7z) or CBT (tar), whatever its
//! name says, is written again as a zip with the same pages, in the same
//! order, and the details of [`crate::inside`] when asked. Pictures are
//! stored as they are (they do not get smaller compressed); other files
//! are compressed. RAR archives are read with libarchive (see
//! `renome-archive`); they cannot be written, so a CBR's details can only
//! be written by converting it.
//!
//! The zip is written beside the comic; when whole, the original takes a
//! name with its own extension beside it (`Name.cbr`) and the zip the
//! comic's name. The original then stays there, or goes to the system's
//! trash. Undo puts a kept original back in the comic's place.

use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, TimeZone, Timelike};
use serde::{Deserialize, Serialize};

use crate::tagwrite::{Backup, temp_of};

/// What a comic archive really is, by its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Zip,
    Rar,
    SevenZip,
    Tar,
}

impl Kind {
    /// The comic extension of the kind.
    pub fn ext(self) -> &'static str {
        match self {
            Kind::Zip => "cbz",
            Kind::Rar => "cbr",
            Kind::SevenZip => "cb7",
            Kind::Tar => "cbt",
        }
    }
}

/// The kind of the archive at `path`, by its first bytes.
pub fn kind(path: &Path) -> Option<Kind> {
    let mut head = [0u8; 262];
    let mut f = File::open(path).ok()?;
    let mut n = 0;
    while n < head.len() {
        match f.read(&mut head[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(_) => return None,
        }
    }
    let h = &head[..n];
    if h.starts_with(b"PK\x03\x04") || h.starts_with(b"PK\x05\x06") {
        Some(Kind::Zip)
    } else if renome_archive::is_rar(h) {
        Some(Kind::Rar)
    } else if h.starts_with(&[0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c]) {
        Some(Kind::SevenZip)
    } else if n >= 262 && &h[257..262] == b"ustar" {
        Some(Kind::Tar)
    } else {
        None
    }
}

/// Comics that can be converted: a comic's extension, a RAR, 7z or tar
/// inside.
pub fn convertible(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    matches!(ext.as_str(), "cbz" | "cbr" | "cb7" | "cbt")
        && matches!(kind(path), Some(Kind::Rar | Kind::SevenZip | Kind::Tar))
}

/// What becomes of the original once converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Original {
    /// Stays beside the CBZ, with its own extension.
    Keep,
    /// Goes to the system's trash.
    Delete,
}

/// A conversion, for undo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Converted {
    /// Where the original was left (beside the CBZ); undo puts it back in
    /// the CBZ's place. `None`: it went to the trash.
    pub original: Option<PathBuf>,
}

/// The outcome of [`convert`].
#[derive(Debug)]
pub struct Done {
    pub backup: Backup,
    /// The original was to go to the trash but could not: it was kept
    /// beside (the reason).
    pub not_trashed: Option<String>,
}

/// One file of the archive, as read.
struct Page {
    name: String,
    modified: Option<i64>,
}

/// Reads each file (not folder) of the archive in order: `each` gets its
/// name, date and content.
fn each_file(
    path: &Path,
    kind: Kind,
    mut each: impl FnMut(&Page, &mut dyn Read) -> Result<(), String>,
) -> Result<(), String> {
    let broken = |e: String| format!("archiveBroken: {e}");
    match kind {
        Kind::Rar => renome_archive::read(path, |e, data| {
            if e.dir || e.other {
                return Ok(true);
            }
            if e.encrypted {
                return Err("archiveLocked".into());
            }
            each(
                &Page {
                    name: e.name.clone(),
                    modified: e.modified,
                },
                data,
            )?;
            Ok(true)
        })
        .map_err(|e| {
            if e == "archiveLocked" || e.starts_with("archiveBroken") {
                e
            } else if e.to_lowercase().contains("encrypt") || e.to_lowercase().contains("password")
            {
                "archiveLocked".into()
            } else {
                broken(e)
            }
        }),
        Kind::SevenZip => {
            let mut reader =
                sevenz_rust2::ArchiveReader::open(path, sevenz_rust2::Password::empty()).map_err(
                    |e| {
                        let e = e.to_string();
                        if e.to_lowercase().contains("password") {
                            "archiveLocked".to_string()
                        } else {
                            broken(e)
                        }
                    },
                )?;
            let mut failed = None;
            reader
                .for_each_entries(|entry, data| {
                    if entry.is_directory() {
                        return Ok(true);
                    }
                    let page = Page {
                        name: entry.name().to_string(),
                        modified: None,
                    };
                    match each(&page, data) {
                        Ok(()) => Ok(true),
                        Err(e) => {
                            failed = Some(e);
                            Ok(false)
                        }
                    }
                })
                .map_err(|e| broken(e.to_string()))?;
            failed.map_or(Ok(()), Err)
        }
        Kind::Tar => {
            let mut archive = tar::Archive::new(File::open(path).map_err(|e| e.to_string())?);
            for entry in archive.entries().map_err(|e| broken(e.to_string()))? {
                let mut entry = entry.map_err(|e| broken(e.to_string()))?;
                if !entry.header().entry_type().is_file() {
                    continue;
                }
                let name = entry
                    .path()
                    .map_err(|e| broken(e.to_string()))?
                    .to_string_lossy()
                    .into_owned();
                let modified = entry.header().mtime().ok().map(|t| t as i64);
                each(&Page { name, modified }, &mut entry)?;
            }
            Ok(())
        }
        Kind::Zip => Err("comicUnsupported".into()),
    }
}

/// Pictures (and archives) are not made smaller by compressing them.
fn stored(name: &str) -> bool {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "jxl" | "heic" | "heif" | "zip" | "rar"
    )
}

fn zip_time(secs: Option<i64>) -> zip::DateTime {
    let t = secs
        .and_then(|s| Local.timestamp_opt(s, 0).single())
        .unwrap_or_else(Local::now)
        .naive_local();
    zip::DateTime::from_date_and_time(
        t.year().clamp(1980, 2107) as u16,
        t.month() as u8,
        t.day() as u8,
        t.hour() as u8,
        t.minute() as u8,
        t.second() as u8,
    )
    .unwrap_or_default()
}

/// A name as a zip keeps it: `/` between folders, none at the start, no
/// `.` or `..` parts. Empty: nothing usable.
fn clean_name(name: &str) -> String {
    name.replace('\\', "/")
        .split('/')
        .filter(|p| !p.is_empty() && *p != "." && *p != "..")
        .collect::<Vec<_>>()
        .join("/")
}

fn is_info(name: &str) -> bool {
    name.rsplit('/')
        .next()
        .is_some_and(|f| f.eq_ignore_ascii_case("ComicInfo.xml"))
}

/// Writes the CBZ of the archive at `path` (of `kind`) to `out`, with
/// `values` written into its `ComicInfo.xml` (see
/// [`crate::inside::comic_xml`]; made when it has none) unless empty.
/// Returns how many files it holds.
fn write_zip(
    path: &Path,
    kind: Kind,
    out: &Path,
    values: &[(&'static str, String)],
) -> Result<usize, String> {
    let file = File::create(out).map_err(|e| e.to_string())?;
    let mut zout = zip::ZipWriter::new(BufWriter::new(file));
    let mut seen = std::collections::HashSet::new();
    let mut info_done = false;
    let mut count = 0;
    let err = |e: zip::result::ZipError| e.to_string();
    each_file(path, kind, |page, data| {
        let name = clean_name(&page.name);
        // A name twice (an archive added to): the first stays.
        if name.is_empty() || !seen.insert(name.to_lowercase()) {
            return std::io::copy(data, &mut std::io::sink())
                .map(|_| ())
                .map_err(|e| format!("archiveBroken: {e}"));
        }
        let method = if stored(&name) {
            zip::CompressionMethod::Stored
        } else {
            zip::CompressionMethod::Deflated
        };
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(method)
            .last_modified_time(zip_time(page.modified))
            .large_file(false);
        if !values.is_empty() && !info_done && is_info(&name) {
            let mut bytes = Vec::new();
            data.take(8 << 20)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("archiveBroken: {e}"))?;
            let old = String::from_utf8_lossy(&bytes).into_owned();
            let new = crate::inside::comic_xml(Some(&old), values).unwrap_or(old);
            zout.start_file(name, options).map_err(err)?;
            zout.write_all(new.as_bytes()).map_err(|e| e.to_string())?;
            info_done = true;
        } else {
            zout.start_file(name, options).map_err(err)?;
            std::io::copy(data, &mut zout).map_err(|e| format!("archiveBroken: {e}"))?;
        }
        count += 1;
        Ok(())
    })?;
    if count == 0 {
        return Err("archiveBroken: empty".into());
    }
    if !values.is_empty()
        && !info_done
        && let Some(new) = crate::inside::comic_xml(None, values)
    {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip_time(None));
        zout.start_file("ComicInfo.xml", options).map_err(err)?;
        zout.write_all(new.as_bytes()).map_err(|e| e.to_string())?;
        count += 1;
    }
    let w = zout.finish().map_err(err)?;
    let file = w.into_inner().map_err(|e| e.error().to_string())?;
    if let Ok(m) = fs::metadata(path) {
        let _ = file.set_permissions(m.permissions());
    }
    file.sync_all().map_err(|e| e.to_string())?;
    // Read back: whole, with every file.
    let back = zip::ZipArchive::new(File::open(out).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if back.len() != count {
        return Err("archiveBroken: written short".into());
    }
    Ok(count)
}

/// Where the original is left: beside, named as the comic with its own
/// extension.
pub fn beside(path: &Path, kind: Kind) -> PathBuf {
    path.with_extension(kind.ext())
}

/// Converts the comic at `path` (named `.cbz`, a RAR, 7z or tar inside)
/// into a CBZ in its place, with `values` in its `ComicInfo.xml` unless empty;
/// the original stays beside or goes to the trash. Errors:
/// `comicUnsupported` (already a zip, or not an archive), `archiveLocked`
/// (needs a password), `archiveBroken: …`, `convertTaken` (a file has the
/// original's name beside), or the system's.
pub fn convert(
    path: &Path,
    values: &[(&'static str, String)],
    original: Original,
) -> Result<Done, String> {
    // Renamed to `.cbz` first (see the plan): the original then takes its
    // own extension back beside.
    let named_cbz = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cbz"));
    let kind = match kind(path) {
        Some(k @ (Kind::Rar | Kind::SevenZip | Kind::Tar)) if named_cbz => k,
        _ => return Err("comicUnsupported".into()),
    };
    let left = beside(path, kind);
    if left == path || fs::symlink_metadata(&left).is_ok() {
        return Err("convertTaken".into());
    }
    let tmp = temp_of(path);
    if let Err(e) = write_zip(path, kind, &tmp, values) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    // The original steps aside, then the CBZ takes its name.
    if let Err(e) = fs::rename(path, &left) {
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::rename(&left, path);
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    let mut not_trashed = None;
    let kept = match original {
        Original::Keep => Some(left),
        Original::Delete => match to_trash(&left) {
            Ok(()) => None,
            Err(e) => {
                not_trashed = Some(e);
                Some(left)
            }
        },
    };
    Ok(Done {
        backup: Backup {
            path: path.to_path_buf(),
            fields: Vec::new(),
            pictures: None,
            cleaned: false,
            removed: Vec::new(),
            raw: Vec::new(),
            kept_text: Vec::new(),
            entry: None,
            converted: Some(Converted { original: kept }),
        },
        not_trashed,
    })
}

fn to_trash(path: &Path) -> Result<(), String> {
    // In a Flatpak the trash found would be the sandbox's own, which the
    // file manager does not show.
    if std::env::var_os("FLATPAK_ID").is_some() {
        return Err("trashFlatpak".into());
    }
    trash::delete(path).map_err(|e| format!("trashFailed: {e}"))
}

/// Undo: a kept original takes the CBZ's place again (the CBZ goes); one
/// in the trash cannot come back, the CBZ stays.
pub fn restore(path: &Path, c: &Converted) -> Result<(), String> {
    let Some(original) = &c.original else {
        return Ok(());
    };
    if fs::symlink_metadata(original).is_err() {
        return Err("originalGone".into());
    }
    fs::rename(original, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tar_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        for (name, data) in files {
            let mut h = tar::Header::new_ustar();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_mtime(1_600_000_000);
            h.set_cksum();
            b.append_data(&mut h, name, *data).unwrap();
        }
        b.into_inner().unwrap()
    }

    fn zip_names(path: &Path) -> Vec<String> {
        let z = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
        z.file_names()
            .filter_map(Result::ok)
            .map(|n| n.into_owned())
            .collect()
    }

    fn zip_read(path: &Path, name: &str) -> Vec<u8> {
        let mut z = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
        let mut f = z.by_name(name).unwrap();
        let mut v = Vec::new();
        f.read_to_end(&mut v).unwrap();
        v
    }

    #[test]
    fn a_tar_becomes_a_zip_and_comes_back() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("Some Hero 001.cbz");
        let tar = tar_of(&[
            ("pages/01.jpg", b"\xff\xd8one"),
            ("pages/02.png", b"\x89PNGtwo"),
            ("notes.txt", b"hello"),
        ]);
        fs::write(&path, &tar).unwrap();
        assert!(convertible(&path));
        let values = vec![
            ("Series", "Some Hero".to_string()),
            ("Number", "1".to_string()),
        ];
        let done = convert(&path, &values, Original::Keep).unwrap();
        assert!(done.not_trashed.is_none());
        assert_eq!(kind(&path), Some(Kind::Zip));
        assert_eq!(
            zip_names(&path),
            ["pages/01.jpg", "pages/02.png", "notes.txt", "ComicInfo.xml"]
        );
        assert_eq!(zip_read(&path, "pages/02.png"), b"\x89PNGtwo");
        let info = String::from_utf8(zip_read(&path, "ComicInfo.xml")).unwrap();
        assert!(info.contains("<Series>Some Hero</Series>"), "{info}");
        let left = d.path().join("Some Hero 001.cbt");
        assert_eq!(fs::read(&left).unwrap(), tar);
        assert!(!temp_of(&path).exists());
        // Undo: the original is back in the comic's place.
        let c = done.backup.converted.unwrap();
        assert_eq!(c.original.as_deref(), Some(left.as_path()));
        restore(&path, &c).unwrap();
        assert_eq!(fs::read(&path).unwrap(), tar);
        assert!(!left.exists());
    }

    #[test]
    fn its_comic_info_is_updated_in_place() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("a.cbz");
        let old = "<?xml version=\"1.0\"?>\n<ComicInfo>\n  <Series>Old</Series>\n  <Summary>Kept.</Summary>\n</ComicInfo>\n";
        fs::write(
            &path,
            tar_of(&[("ComicInfo.xml", old.as_bytes()), ("1.jpg", b"x")]),
        )
        .unwrap();
        let values = vec![("Series", "New".to_string())];
        convert(&path, &values, Original::Keep).unwrap();
        assert_eq!(zip_names(&path), ["ComicInfo.xml", "1.jpg"]);
        let info = String::from_utf8(zip_read(&path, "ComicInfo.xml")).unwrap();
        assert!(info.contains("<Series>New</Series>"), "{info}");
        assert!(info.contains("<Summary>Kept.</Summary>"), "{info}");
        // Without details, it is copied as it was.
        let other = d.path().join("b.cbz");
        fs::write(&other, tar_of(&[("ComicInfo.xml", old.as_bytes())])).unwrap();
        convert(&other, &[], Original::Keep).unwrap();
        assert_eq!(zip_read(&other, "ComicInfo.xml"), old.as_bytes());
    }

    #[test]
    fn what_cannot_be_converted_is_left_alone() {
        let d = tempfile::tempdir().unwrap();
        // Already a zip.
        let z = d.path().join("z.cbz");
        {
            let mut w = zip::ZipWriter::new(File::create(&z).unwrap());
            w.start_file("1.jpg", zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(b"x").unwrap();
            w.finish().unwrap();
        }
        assert!(!convertible(&z));
        assert_eq!(
            convert(&z, &[], Original::Keep).unwrap_err(),
            "comicUnsupported"
        );
        // A damaged RAR: an error, the file as it was, nothing left beside.
        let r = d.path().join("r.cbz");
        let junk = b"Rar!\x1a\x07\x00garbage that is not a rar archive".to_vec();
        fs::write(&r, &junk).unwrap();
        assert!(convertible(&r));
        let e = convert(&r, &[], Original::Keep).unwrap_err();
        assert!(e.starts_with("archiveBroken"), "{e}");
        assert_eq!(fs::read(&r).unwrap(), junk);
        assert!(!temp_of(&r).exists());
        assert!(!d.path().join("r.cbr").exists());
        // Not yet named `.cbz`.
        let n = d.path().join("n.cbt");
        fs::write(&n, tar_of(&[("1.jpg", b"x")])).unwrap();
        assert_eq!(
            convert(&n, &[], Original::Keep).unwrap_err(),
            "comicUnsupported"
        );
        // The original's name taken beside.
        let t = d.path().join("t.cbz");
        fs::write(&t, tar_of(&[("1.jpg", b"x")])).unwrap();
        fs::write(d.path().join("t.cbt"), b"mine").unwrap();
        assert_eq!(
            convert(&t, &[], Original::Keep).unwrap_err(),
            "convertTaken"
        );
        assert_eq!(kind(&t), Some(Kind::Tar));
    }

    // From libarchive's tests (BSD license).
    const RAR4: &[u8] = include_bytes!("../../renome-archive/tests/data/rar4.rar");
    const LOCKED: &[u8] = include_bytes!("../../renome-archive/tests/data/rar4-locked.rar");

    #[test]
    fn a_rar_becomes_a_zip() {
        let d = tempfile::tempdir().unwrap();
        // A CBR renamed to `.cbz` (by the plan) before.
        let path = d.path().join("x.cbz");
        fs::write(&path, RAR4).unwrap();
        assert!(convertible(&path));
        convert(&path, &[], Original::Keep).unwrap();
        // The link and the folders are left out.
        assert_eq!(zip_names(&path), ["test.txt", "testdir/test.txt"]);
        assert_eq!(
            zip_read(&path, "testdir/test.txt"),
            b"test text document\r\n"
        );
        assert_eq!(fs::read(d.path().join("x.cbr")).unwrap(), RAR4);
        // One with a password is left as it was.
        let locked = d.path().join("l.cbz");
        fs::write(&locked, LOCKED).unwrap();
        assert_eq!(
            convert(&locked, &[], Original::Keep).unwrap_err(),
            "archiveLocked"
        );
        assert_eq!(fs::read(&locked).unwrap(), LOCKED);
        assert!(!temp_of(&locked).exists());
        assert!(!d.path().join("l.cbr").exists());
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("\\a\\b.jpg"), "a/b.jpg");
        assert_eq!(clean_name("/../x/./y.png"), "x/y.png");
        assert_eq!(clean_name("./"), "");
    }
}
