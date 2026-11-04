//! Reads RAR archives (RAR 4 and RAR 5) with libarchive, an open source
//! library under the BSD license whose RAR readers are its own work. A
//! copy is built into the program (see `build.rs`), so nothing has to be
//! installed. Only reading: RAR archives cannot be written.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[repr(C)]
struct Archive {
    _p: [u8; 0],
}

#[repr(C)]
struct ArchiveEntry {
    _p: [u8; 0],
}

#[cfg(any(windows, target_os = "macos"))]
type Mode = u16;
#[cfg(not(any(windows, target_os = "macos")))]
type Mode = u32;

type ReadCb = unsafe extern "C" fn(*mut Archive, *mut c_void, *mut *const c_void) -> isize;
type SeekCb = unsafe extern "C" fn(*mut Archive, *mut c_void, i64, c_int) -> i64;
type SkipCb = unsafe extern "C" fn(*mut Archive, *mut c_void, i64) -> i64;

unsafe extern "C" {
    fn archive_read_new() -> *mut Archive;
    fn archive_read_support_format_rar(a: *mut Archive) -> c_int;
    fn archive_read_support_format_rar5(a: *mut Archive) -> c_int;
    fn archive_read_set_read_callback(a: *mut Archive, f: ReadCb) -> c_int;
    fn archive_read_set_seek_callback(a: *mut Archive, f: SeekCb) -> c_int;
    fn archive_read_set_skip_callback(a: *mut Archive, f: SkipCb) -> c_int;
    fn archive_read_set_callback_data(a: *mut Archive, data: *mut c_void) -> c_int;
    fn archive_read_open1(a: *mut Archive) -> c_int;
    fn archive_read_next_header(a: *mut Archive, e: *mut *mut ArchiveEntry) -> c_int;
    fn archive_read_data(a: *mut Archive, buf: *mut c_void, len: usize) -> isize;
    fn archive_error_string(a: *mut Archive) -> *const c_char;
    fn archive_read_free(a: *mut Archive) -> c_int;
    fn archive_entry_pathname_utf8(e: *mut ArchiveEntry) -> *const c_char;
    fn archive_entry_pathname(e: *mut ArchiveEntry) -> *const c_char;
    fn archive_entry_filetype(e: *mut ArchiveEntry) -> Mode;
    fn archive_entry_size(e: *mut ArchiveEntry) -> i64;
    fn archive_entry_size_is_set(e: *mut ArchiveEntry) -> c_int;
    fn archive_entry_mtime(e: *mut ArchiveEntry) -> i64;
    fn archive_entry_mtime_is_set(e: *mut ArchiveEntry) -> c_int;
    fn archive_entry_is_encrypted(e: *mut ArchiveEntry) -> c_int;
}

const OK: c_int = 0;
const EOF: c_int = 1;
const WARN: c_int = -20;
const FATAL: i64 = -30;
const IFMT: u32 = 0o170000;
const IFDIR: u32 = 0o040000;
const IFREG: u32 = 0o100000;

/// The first bytes of every RAR archive (RAR 4 and RAR 5).
pub fn is_rar(head: &[u8]) -> bool {
    head.starts_with(b"Rar!\x1a\x07")
}

/// A file or folder in the archive.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Its path in the archive, `/` between folders.
    pub name: String,
    pub dir: bool,
    /// Neither a file nor a folder (a link): skipped by readers.
    pub other: bool,
    pub size: Option<u64>,
    /// Seconds since 1970, when told.
    pub modified: Option<i64>,
    /// Needs a password: its content cannot be read.
    pub encrypted: bool,
}

struct Source {
    file: File,
    buf: Vec<u8>,
}

unsafe extern "C" fn read_cb(_: *mut Archive, data: *mut c_void, out: *mut *const c_void) -> isize {
    // SAFETY: `data` is the `Source` given to `archive_read_set_callback_data`,
    // alive until the archive is freed.
    let src = unsafe { &mut *(data as *mut Source) };
    match src.file.read(&mut src.buf) {
        Ok(n) => {
            // SAFETY: `out` is a valid pointer from libarchive.
            unsafe { *out = src.buf.as_ptr() as *const c_void };
            n as isize
        }
        Err(_) => -1,
    }
}

unsafe extern "C" fn seek_cb(
    _: *mut Archive,
    data: *mut c_void,
    offset: i64,
    whence: c_int,
) -> i64 {
    // SAFETY: as in `read_cb`.
    let src = unsafe { &mut *(data as *mut Source) };
    let to = match whence {
        0 => SeekFrom::Start(offset.max(0) as u64),
        1 => SeekFrom::Current(offset),
        _ => SeekFrom::End(offset),
    };
    src.file.seek(to).map_or(FATAL, |p| p as i64)
}

unsafe extern "C" fn skip_cb(_: *mut Archive, data: *mut c_void, n: i64) -> i64 {
    // SAFETY: as in `read_cb`.
    let src = unsafe { &mut *(data as *mut Source) };
    match src.file.seek(SeekFrom::Current(n)) {
        Ok(_) => n,
        Err(_) => 0,
    }
}

/// An open archive; freed when dropped.
struct Handle {
    a: *mut Archive,
    // Boxed so its address stays put for the callbacks.
    _src: Box<Source>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: `a` came from `archive_read_new` and is freed once.
        unsafe { archive_read_free(self.a) };
    }
}

impl Handle {
    fn error(&self) -> String {
        // SAFETY: `a` is valid; the string, when set, is NUL-terminated.
        let s = unsafe { archive_error_string(self.a) };
        if s.is_null() {
            "unreadable".into()
        } else {
            // SAFETY: checked not null above.
            unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()
        }
    }
}

/// The content of the entry being read.
pub struct Data<'a> {
    h: &'a Handle,
}

impl Read for Data<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // SAFETY: `buf` is valid for `buf.len()` bytes.
        let n = unsafe { archive_read_data(self.h.a, buf.as_mut_ptr() as *mut c_void, buf.len()) };
        if n < 0 {
            Err(std::io::Error::other(self.h.error()))
        } else {
            Ok(n as usize)
        }
    }
}

/// Reads the RAR archive at `path` entry by entry, in the archive's order:
/// `each` gets each entry and its content (read it or not), and returns
/// `false` to stop there. Errors: the archive is not a RAR one, is
/// damaged, or the reading of a content failed (as `each` returns it).
pub fn read(
    path: &Path,
    mut each: impl FnMut(&Entry, &mut Data) -> Result<bool, String>,
) -> Result<(), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut src = Box::new(Source {
        file,
        buf: vec![0; 1 << 16],
    });
    // SAFETY: plain calls on a fresh archive; `src` outlives it (both in
    // `Handle`, the archive freed first in `drop`).
    let h = unsafe {
        let a = archive_read_new();
        if a.is_null() {
            return Err("out of memory".into());
        }
        archive_read_support_format_rar(a);
        archive_read_support_format_rar5(a);
        archive_read_set_read_callback(a, read_cb);
        archive_read_set_seek_callback(a, seek_cb);
        archive_read_set_skip_callback(a, skip_cb);
        archive_read_set_callback_data(a, &mut *src as *mut Source as *mut c_void);
        Handle { a, _src: src }
    };
    // SAFETY: callbacks and data are set.
    if unsafe { archive_read_open1(h.a) } != OK {
        return Err(h.error());
    }
    loop {
        let mut e: *mut ArchiveEntry = std::ptr::null_mut();
        // SAFETY: `h.a` is open; `e` is written by libarchive.
        let r = unsafe { archive_read_next_header(h.a, &mut e) };
        if r == EOF {
            return Ok(());
        }
        if (r != OK && r != WARN) || e.is_null() {
            return Err(h.error());
        }
        // SAFETY: `e` is the current entry, valid until the next header.
        let entry = unsafe {
            let mut name = archive_entry_pathname_utf8(e);
            if name.is_null() {
                name = archive_entry_pathname(e);
            }
            let name = if name.is_null() {
                String::new()
            } else {
                CStr::from_ptr(name).to_string_lossy().replace('\\', "/")
            };
            let kind = archive_entry_filetype(e) as u32 & IFMT;
            Entry {
                name,
                dir: kind == IFDIR,
                other: kind != IFDIR && kind != IFREG && kind != 0,
                size: (archive_entry_size_is_set(e) != 0)
                    .then(|| archive_entry_size(e).max(0) as u64),
                modified: (archive_entry_mtime_is_set(e) != 0).then(|| archive_entry_mtime(e)),
                encrypted: archive_entry_is_encrypted(e) != 0,
            }
        };
        if !each(&entry, &mut Data { h: &h })? {
            return Ok(());
        }
    }
}
