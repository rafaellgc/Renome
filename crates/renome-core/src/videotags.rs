//! Title, date, description, genres, show, season, episode and cover
//! written inside Matroska (`.mkv`, `.webm`) and MP4 (`.mp4`, `.m4v`,
//! `.mov`) videos, where players and media servers show them.
//!
//! A video is gigabytes: it is never rewritten. Matroska gets its new
//! tags and attachments at the end of the file, the old ones turned into
//! empty space (Void) and its index (SeekHead) and segment size updated
//! where they are; MP4 gets its new index (`moov`) where the old one was
//! when it fits, else at the end with the old one turned into a `free`
//! box (the media data never moves, so its offsets stay right). Every
//! byte changed is saved first, so undo puts them back and cuts the file
//! to its old length.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::video::{ebml, ebml_uint, vint};

/// What is written into a video.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VideoMeta {
    /// The movie's title, or the episode's.
    pub title: String,
    /// Release (or air) date, `2010-07-16` or `2010`.
    pub date: String,
    pub description: String,
    pub genres: Vec<String>,
    /// For an episode: its show, season and number.
    pub show: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    /// `tt1375666`.
    pub imdb: String,
    /// `movie/27205`, `tv/1399`.
    pub tmdb: String,
    /// A JPEG or PNG picture.
    #[serde(skip)]
    pub cover: Option<Vec<u8>>,
}

impl VideoMeta {
    fn is_episode(&self) -> bool {
        !self.show.is_empty()
    }
}

/// What a write changed in a video, to put it back (see [`restore`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoEdit {
    pub old_len: u64,
    pub new_len: u64,
    /// The file (in the history's folder) with the old bytes, one range
    /// after another.
    pub store: String,
    pub patches: Vec<Patch>,
    /// Bytes from here to `old_len` were cut (MP4 whose index was last):
    /// they follow the patches' in the store file.
    pub cut: u64,
}

/// One range of bytes changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Patch {
    pub offset: u64,
    pub len: u64,
    /// The written bytes' hash: undo checks the file still has them.
    pub hash: u64,
}

/// The videos whose details can be written: Matroska and MP4, told by
/// their first bytes.
pub fn writable(path: &Path) -> bool {
    kind(path).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Matroska,
    Mp4,
}

fn kind(path: &Path) -> Option<Kind> {
    let mut head = [0u8; 12];
    File::open(path).ok()?.read_exact(&mut head).ok()?;
    if head[..4] == [0x1a, 0x45, 0xdf, 0xa3] {
        return Some(Kind::Matroska);
    }
    let boxes: [&[u8; 4]; 6] = [b"ftyp", b"moov", b"mdat", b"free", b"wide", b"skip"];
    boxes.iter().any(|k| head[4..8] == k[..]).then_some(Kind::Mp4)
}

/// The changes to make: same-length patches inside the file, and what
/// replaces it from `cut` on.
struct Plan {
    patches: Vec<(u64, Vec<u8>)>,
    cut: u64,
    tail: Vec<u8>,
}

/// Writes `meta` into the video at `path`; the old bytes go to a new file
/// in `store`. `None` when there was nothing to change.
pub fn write(path: &Path, meta: &VideoMeta, store: &Path) -> Result<Option<VideoEdit>, String> {
    let k = kind(path).ok_or("videoUnsupported")?;
    let mut f = File::options()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    let plan = match k {
        Kind::Matroska => mkv_plan(&mut f, len, meta)?,
        Kind::Mp4 => mp4_plan(&mut f, len, meta)?,
    };
    let Some(plan) = plan else { return Ok(None) };
    apply(&mut f, path, len, plan, store).map(Some)
}

fn hash(b: &[u8]) -> u64 {
    xxhash_rust::xxh3::xxh3_64(b)
}

fn read_at(f: &mut File, at: u64, len: usize) -> Result<Vec<u8>, String> {
    let mut b = vec![0u8; len];
    f.seek(SeekFrom::Start(at)).map_err(|e| e.to_string())?;
    f.read_exact(&mut b).map_err(|e| e.to_string())?;
    Ok(b)
}

fn write_at(f: &mut File, at: u64, b: &[u8]) -> std::io::Result<()> {
    f.seek(SeekFrom::Start(at))?;
    f.write_all(b)
}

/// Saves what `plan` changes, then makes the changes: the new tail first
/// (beyond what any reader of the old file looks at), then the patches.
/// A failure puts the file back.
fn apply(f: &mut File, path: &Path, len: u64, plan: Plan, store: &Path) -> Result<VideoEdit, String> {
    let mut patches = plan.patches;
    patches.sort_by_key(|p| p.0);
    for w in patches.windows(2) {
        if w[0].0 + w[0].1.len() as u64 > w[1].0 {
            return Err("videoUnsupported".into());
        }
    }
    if patches.iter().any(|(at, b)| at + b.len() as u64 > plan.cut) || plan.cut > len {
        return Err("videoUnsupported".into());
    }
    // The old bytes, saved before anything changes.
    let mut old = Vec::new();
    for (at, b) in &patches {
        old.extend(read_at(f, *at, b.len())?);
    }
    old.extend(read_at(f, plan.cut, (len - plan.cut) as usize)?);
    fs::create_dir_all(store).map_err(|e| e.to_string())?;
    let name = {
        let base = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut n = 0;
        loop {
            let s = format!("video-{n}-{base}.bin");
            if fs::symlink_metadata(store.join(&s)).is_err() {
                break s;
            }
            n += 1;
        }
    };
    let saved = store.join(&name);
    fs::write(&saved, &old)
        .and_then(|_| File::open(&saved)?.sync_all())
        .map_err(|e| e.to_string())?;
    let new_len = plan.cut + plan.tail.len() as u64;
    let done = (|| -> std::io::Result<()> {
        // The tail first: past the old content, or over an index the
        // patches no longer point to.
        if !plan.tail.is_empty() || plan.cut < len {
            write_at(f, plan.cut, &plan.tail)?;
            f.set_len(new_len)?;
            f.sync_data()?;
        }
        for (at, b) in &patches {
            write_at(f, *at, b)?;
        }
        f.sync_all()
    })();
    let edit = VideoEdit {
        old_len: len,
        new_len,
        store: name,
        patches: patches
            .iter()
            .map(|(at, b)| Patch {
                offset: *at,
                len: b.len() as u64,
                hash: hash(b),
            })
            .collect(),
        cut: plan.cut,
    };
    if let Err(e) = done {
        // Put back what was written, as far as it goes.
        let _ = put_back(f, &edit, &old);
        let _ = fs::remove_file(&saved);
        return Err(e.to_string());
    }
    Ok(edit)
}

fn put_back(f: &mut File, edit: &VideoEdit, old: &[u8]) -> std::io::Result<()> {
    let mut at = 0usize;
    for p in &edit.patches {
        let n = p.len as usize;
        write_at(f, p.offset, &old[at..at + n])?;
        at += n;
    }
    f.set_len(edit.cut)?;
    write_at(f, edit.cut, &old[at..])?;
    f.set_len(edit.old_len)?;
    f.sync_all()
}

/// Puts back what [`write`] changed. The file must still be as it was
/// left (its length and the bytes written), or it is not touched.
pub fn restore(path: &Path, edit: &VideoEdit, store: &Path) -> Result<(), String> {
    let old = fs::read(store.join(&edit.store)).map_err(|e| e.to_string())?;
    let expected: u64 = edit.patches.iter().map(|p| p.len).sum::<u64>() + (edit.old_len - edit.cut);
    if old.len() as u64 != expected {
        return Err("videoChanged".into());
    }
    let mut f = File::options()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    if len != edit.new_len {
        return Err("videoChanged".into());
    }
    for p in &edit.patches {
        if hash(&read_at(&mut f, p.offset, p.len as usize)?) != p.hash {
            return Err("videoChanged".into());
        }
    }
    put_back(&mut f, edit, &old).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- EBML

const SEGMENT: u64 = 0x1853_8067;
const SEEKHEAD: u64 = 0x114d_9b74;
const INFO: u64 = 0x1549_a966;
const TAGS: u64 = 0x1254_c367;
const ATTACHMENTS: u64 = 0x1941_a469;
#[cfg(test)]
const CLUSTER: u64 = 0x1f43_b675;
const VOID: u64 = 0xec;
const CRC32: u64 = 0xbf;
/// The largest attachments copied to make room for a cover (fonts of
/// SubStation subtitles, usually a few megabytes).
const MAX_ATTACHMENTS: u64 = 64 << 20;

fn id_bytes(id: u64) -> Vec<u8> {
    let b = id.to_be_bytes();
    let skip = b.iter().position(|x| *x != 0).unwrap_or(7);
    b[skip..].to_vec()
}

/// A size as an EBML number of exactly `len` bytes (`None` if it does not
/// fit).
fn size_vint(n: u64, len: usize) -> Option<Vec<u8>> {
    if !(1..=8).contains(&len) || n >= (1u64 << (7 * len)) - 1 {
        return None;
    }
    let mut v = n.to_be_bytes()[8 - len..].to_vec();
    v[0] |= 0x80 >> (len - 1);
    Some(v)
}

fn min_size_len(n: u64) -> usize {
    (1..=8)
        .find(|&l| n < (1u64 << (7 * l)) - 1)
        .unwrap_or(8)
}

/// An element: id, size (as short as it can be, or `size_len` bytes)
/// and content.
fn el_len(id: u64, data: &[u8], size_len: Option<usize>) -> Vec<u8> {
    let len = size_len.unwrap_or_else(|| min_size_len(data.len() as u64));
    let mut v = id_bytes(id);
    v.extend(size_vint(data.len() as u64, len).expect("size fits"));
    v.extend_from_slice(data);
    v
}

fn el(id: u64, data: &[u8]) -> Vec<u8> {
    el_len(id, data, None)
}

fn uint_el(id: u64, n: u64) -> Vec<u8> {
    let b = n.to_be_bytes();
    let skip = b.iter().position(|x| *x != 0).unwrap_or(7);
    el(id, &b[skip..])
}

fn str_el(id: u64, s: &str) -> Vec<u8> {
    el(id, s.as_bytes())
}

/// A Void element exactly `n` bytes long (`n` ≥ 2).
fn void(n: u64) -> Option<Vec<u8>> {
    if n < 2 {
        return None;
    }
    let mut v = vec![0xec];
    if n - 2 < 127 {
        v.push(0x80 | (n - 2) as u8);
    } else if n >= 9 {
        v.extend(size_vint(n - 9, 8)?);
    } else {
        return None;
    }
    v.resize(n as usize, 0);
    Some(v)
}

/// The header of a Void element `n` bytes long, all that needs writing
/// to turn an element of that length into one.
fn void_head(n: u64) -> Option<Vec<u8>> {
    let v = void(n.min(200))?;
    if n < 129 {
        return Some(v[..2].to_vec());
    }
    let mut h = vec![0xec];
    h.extend(size_vint(n - 9, 8)?);
    Some(h)
}

/// `bytes` padded with a Void to fill `room` exactly; when one byte
/// would be left (too little for a Void), `rebuild` makes the element
/// with a size field one byte longer.
fn fill(room: u64, bytes: Vec<u8>, rebuild: impl Fn(usize) -> Option<Vec<u8>>) -> Option<Vec<u8>> {
    let n = bytes.len() as u64;
    if n > room {
        return None;
    }
    if room - n == 1 {
        let wider = rebuild(1)?;
        return (wider.len() as u64 == room).then_some(wider);
    }
    let mut out = bytes;
    if room > n {
        out.extend(void(room - n)?);
    }
    Some(out)
}

/// A top-level element of the segment.
#[derive(Debug, Clone, Copy)]
struct Top {
    id: u64,
    at: u64,
    head: u64,
    size: u64,
}

impl Top {
    fn end(&self) -> u64 {
        self.at + self.head + self.size
    }
}

/// Reads an element header at `at`: id, size (`None`: unknown), header
/// length.
fn header_at(f: &mut File, at: u64) -> Option<(u64, Option<u64>, u64)> {
    let mut b = [0u8; 12];
    f.seek(SeekFrom::Start(at)).ok()?;
    let n = f.read(&mut b).ok()?;
    let (id, a) = vint(&b[..n], true)?;
    if a > 4 {
        return None;
    }
    let (size, s) = vint(b.get(a..n)?, false)?;
    Some((id, (size != u64::MAX).then_some(size), (a + s) as u64))
}

/// The Matroska changes: new Tags (and Attachments with the cover) at
/// the end, the old ones made Void, the SeekHead pointing to the new
/// ones, the segment's size, and the title in Info when it fits.
fn mkv_plan(f: &mut File, len: u64, meta: &VideoMeta) -> Result<Option<Plan>, String> {
    let bad = || "videoUnsupported".to_string();
    let (id, size, h) = header_at(f, 0).ok_or_else(bad)?;
    if id != 0x1a45_dfa3 {
        return Err(bad());
    }
    let seg_at = h + size.ok_or_else(bad)?;
    let (id, seg_size, seg_head) = header_at(f, seg_at).ok_or_else(bad)?;
    if id != SEGMENT {
        return Err(bad());
    }
    // A live recording's unknown size: its end is not known.
    let seg_size = seg_size.ok_or_else(bad)?;
    let data = seg_at + seg_head;
    let seg_end = data + seg_size;
    // Something after the segment: appending would not be inside it.
    if seg_end != len {
        return Err(bad());
    }
    let mut tops: Vec<Top> = Vec::new();
    let mut at = data;
    while at < seg_end {
        let (id, size, head) = header_at(f, at).ok_or_else(bad)?;
        let size = size.ok_or_else(bad)?;
        let t = Top { id, at, head, size };
        if t.end() > seg_end {
            return Err(bad());
        }
        tops.push(t);
        at = t.end();
    }
    let find = |id: u64| tops.iter().filter(move |t| t.id == id).copied();
    let body = |f: &mut File, t: &Top| read_at(f, t.at + t.head, t.size as usize);

    let mut patches: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut tail: Vec<u8> = Vec::new();
    let mut moved: Vec<(u64, u64)> = Vec::new();

    // Tags: the old ones' tags of tracks and the like kept, ours added.
    let old_tags = find(TAGS).next();
    let mut tags = Vec::new();
    if let Some(t) = old_tags {
        let b = body(f, &t)?;
        for (id, tag) in ebml(&b) {
            if id == 0x7373
                && let Some(kept) = keep_tag(tag, meta)
            {
                tags.extend(kept);
            }
        }
    }
    tags.extend(our_tags(meta));
    let tags_el = el(TAGS, &tags);
    moved.push((TAGS, seg_end + tail.len() as u64 - data));
    tail.extend(&tags_el);

    // Attachments: a cover named `cover.jpg` (as players look for), in
    // place of an older cover; the other files (fonts) kept.
    let old_att = find(ATTACHMENTS).next();
    if let Some(cover) = &meta.cover {
        if old_att.is_some_and(|t| t.size > MAX_ATTACHMENTS) {
            return Err("videoNoRoom".into());
        }
        let mut files = Vec::new();
        if let Some(t) = old_att {
            let b = body(f, &t)?;
            for (id, file) in ebml(&b) {
                if id == 0x61a7 && !is_cover(file) {
                    files.extend(el(id, file));
                }
            }
        }
        let png = cover.starts_with(b"\x89PNG");
        let mut file = str_el(0x466e, if png { "cover.png" } else { "cover.jpg" });
        file.extend(str_el(0x4660, if png { "image/png" } else { "image/jpeg" }));
        file.extend(el(0x465c, cover));
        file.extend(uint_el(0x46ae, hash(cover) | 1));
        files.extend(el(0x61a7, &file));
        moved.push((ATTACHMENTS, seg_end + tail.len() as u64 - data));
        tail.extend(el(ATTACHMENTS, &files));
    }

    // The old ones become empty space.
    for t in [old_tags, old_att.filter(|_| meta.cover.is_some())].into_iter().flatten() {
        // Only the header changes: the rest is ignored as a Void's.
        patches.push((t.at, void_head(t.end() - t.at).ok_or_else(bad)?));
    }

    // Room after an element: the Voids right after it.
    let room_after = |t: &Top| -> u64 {
        let mut end = t.end();
        // The elements are in file order: the ones after it, while Void.
        for v in tops.iter().skip_while(|v| v.at < t.end()) {
            if v.at == end && v.id == VOID {
                end = v.end();
            } else {
                break;
            }
        }
        end - t.at
    };
    let taken = |patches: &[(u64, Vec<u8>)], at: u64, n: u64| {
        patches
            .iter()
            .any(|(p, b)| *p < at + n && at < p + b.len() as u64)
    };

    // The SeekHeads: the entries of the moved elements changed, or added
    // to the first one.
    let heads: Vec<Top> = find(SEEKHEAD).collect();
    let first = heads.first().copied().ok_or("videoNoRoom")?;
    let mut done: Vec<u64> = Vec::new();
    for sh in &heads {
        let b = body(f, sh)?;
        let mut entries: Vec<(Vec<u8>, u64)> = ebml(&b)
            .filter(|(id, _)| *id == 0x4dbb)
            .filter_map(|(_, e)| {
                let mut sid = None;
                let mut pos = None;
                for (id, v) in ebml(e) {
                    match id {
                        0x53ab => sid = Some(v.to_vec()),
                        0x53ac => pos = Some(ebml_uint(v)),
                        _ => {}
                    }
                }
                Some((sid?, pos?))
            })
            .collect();
        let mut changed = false;
        for (id, pos) in &moved {
            let want = id_bytes(*id);
            for e in entries.iter_mut().filter(|e| e.0 == want) {
                e.1 = *pos;
                changed = true;
                done.push(*id);
            }
        }
        if !changed {
            continue;
        }
        entries.dedup();
        patches.push(seekhead_patch(sh, &entries, room_after(sh))?);
    }
    let missing: Vec<(u64, u64)> = moved
        .iter()
        .filter(|(id, _)| !done.contains(id))
        .copied()
        .collect();
    if !missing.is_empty() {
        // Rebuilt from the first SeekHead's entries (changed already if
        // it held some), plus the missing ones.
        let b = body(f, &first)?;
        let mut entries: Vec<(Vec<u8>, u64)> = ebml(&b)
            .filter(|(id, _)| *id == 0x4dbb)
            .filter_map(|(_, e)| {
                let mut sid = None;
                let mut pos = None;
                for (id, v) in ebml(e) {
                    match id {
                        0x53ab => sid = Some(v.to_vec()),
                        0x53ac => pos = Some(ebml_uint(v)),
                        _ => {}
                    }
                }
                Some((sid?, pos?))
            })
            .collect();
        for (id, pos) in &moved {
            let want = id_bytes(*id);
            match entries.iter_mut().find(|e| e.0 == want) {
                Some(e) => e.1 = *pos,
                None => entries.push((want, *pos)),
            }
        }
        patches.retain(|(at, _)| *at != first.at);
        patches.push(seekhead_patch(&first, &entries, room_after(&first))?);
    }

    // The title in Info, when it fits where Info is.
    if let Some(info) = find(INFO).next()
        && !meta.title.is_empty()
    {
        let title = if meta.is_episode() {
            episode_title(meta)
        } else {
            meta.title.clone()
        };
        let b = body(f, &info)?;
        let had_title = ebml(&b).any(|(id, _)| id == 0x7ba9);
        let mut rest: Vec<u8> = Vec::new();
        for (id, v) in ebml(&b) {
            if id != 0x7ba9 && id != CRC32 {
                rest.extend(el(id, v));
            }
        }
        let mut with = rest.clone();
        with.extend(str_el(0x7ba9, &title));
        let room = room_after(&info);
        let place = |kids: &Vec<u8>| {
            fill(room, el(INFO, kids), |extra| {
                Some(el_len(INFO, kids, Some(min_size_len(kids.len() as u64) + extra)))
            })
        };
        if !taken(&patches, info.at, room) {
            // Too long to fit: an old title (often a site's name) goes,
            // and players show the tags' title instead.
            let bytes = place(&with).or_else(|| had_title.then(|| place(&rest)).flatten());
            if let Some(bytes) = bytes {
                patches.push((info.at, bytes));
            }
        }
    }

    // The segment's size, in as many bytes as it has.
    let new_size = seg_size + tail.len() as u64;
    let size_len = (seg_head - 4) as usize;
    let size = size_vint(new_size, size_len).ok_or("videoNoRoom")?;
    patches.push((seg_at + 4, size));
    Ok(Some(Plan {
        patches,
        cut: len,
        tail,
    }))
}

/// A SeekHead rebuilt with `entries` where it is (with the room after
/// it), or an error when it does not fit.
fn seekhead_patch(sh: &Top, entries: &[(Vec<u8>, u64)], room: u64) -> Result<(u64, Vec<u8>), String> {
    let mut kids = Vec::new();
    for (sid, pos) in entries {
        let mut e = el(0x53ab, sid);
        e.extend(uint_el(0x53ac, *pos));
        kids.extend(el(0x4dbb, &e));
    }
    let bytes = fill(room, el(SEEKHEAD, &kids), |extra| {
        Some(el_len(SEEKHEAD, &kids, Some(min_size_len(kids.len() as u64) + extra)))
    })
    .ok_or("videoNoRoom")?;
    Ok((sh.at, bytes))
}

/// A file attached as a cover (`cover.jpg`, `small_cover.png`…).
fn is_cover(file: &[u8]) -> bool {
    ebml(file).any(|(id, v)| {
        id == 0x466e && {
            let n = String::from_utf8_lossy(v).to_lowercase();
            n.starts_with("cover.") || n.starts_with("small_cover.") || n.starts_with("cover_land.")
        }
    })
}

/// The names of the simple tags written, per target level.
fn our_names(level: u64, meta: &VideoMeta) -> &'static [&'static str] {
    match (meta.is_episode(), level) {
        (false, 50) => &["TITLE", "DATE_RELEASED", "DESCRIPTION", "SYNOPSIS", "SUMMARY", "GENRE", "IMDB", "TMDB"],
        (true, 70) => &["TITLE", "IMDB", "TMDB", "GENRE"],
        (true, 60) => &["PART_NUMBER"],
        (true, 50) => &["TITLE", "PART_NUMBER", "DATE_RELEASED", "DESCRIPTION", "SYNOPSIS", "SUMMARY"],
        _ => &[],
    }
}

/// An old Tag kept: those of a track, chapter or attachment as they are;
/// a whole-file one without the simple tags written anew (`None` when
/// nothing is left of it).
fn keep_tag(tag: &[u8], meta: &VideoMeta) -> Option<Vec<u8>> {
    let mut level = 50;
    let mut uids = false;
    for (id, v) in ebml(tag) {
        if id == 0x63c0 {
            for (tid, tv) in ebml(v) {
                match tid {
                    0x68ca => level = ebml_uint(tv),
                    0x63c5 | 0x63c9 | 0x63c4 | 0x63c6 if ebml_uint(tv) != 0 => uids = true,
                    _ => {}
                }
            }
        }
    }
    if uids {
        return Some(el(0x7373, tag));
    }
    let ours = our_names(level, meta);
    let mut out = Vec::new();
    let mut simple = 0;
    for (id, v) in ebml(tag) {
        if id == 0x67c8 {
            let name = ebml(v)
                .find(|(i, _)| *i == 0x45a3)
                .map(|(_, n)| String::from_utf8_lossy(n).to_uppercase())
                .unwrap_or_default();
            if ours.contains(&name.as_str()) {
                continue;
            }
            simple += 1;
        }
        out.extend(el(id, v));
    }
    (simple > 0).then(|| el(0x7373, &out))
}

fn simple(name: &str, value: &str) -> Vec<u8> {
    let mut s = str_el(0x45a3, name);
    s.extend(str_el(0x4487, value));
    el(0x67c8, &s)
}

fn tag(level: u64, kind: &str, simples: &[(&str, String)]) -> Vec<u8> {
    let simples: Vec<&(&str, String)> = simples.iter().filter(|(_, v)| !v.is_empty()).collect();
    if simples.is_empty() {
        return Vec::new();
    }
    let mut targets = uint_el(0x68ca, level);
    targets.extend(str_el(0x63ca, kind));
    let mut t = el(0x63c0, &targets);
    for (n, v) in simples {
        t.extend(simple(n, v));
    }
    el(0x7373, &t)
}

/// The tags written: a movie's at level 50; an episode's show at 70, its
/// season at 60 and itself at 50.
fn our_tags(meta: &VideoMeta) -> Vec<u8> {
    let genre = meta.genres.join(", ");
    if !meta.is_episode() {
        return tag(
            50,
            "MOVIE",
            &[
                ("TITLE", meta.title.clone()),
                ("DATE_RELEASED", meta.date.clone()),
                ("SYNOPSIS", meta.description.clone()),
                ("GENRE", genre),
                ("IMDB", meta.imdb.clone()),
                ("TMDB", meta.tmdb.clone()),
            ],
        );
    }
    let mut out = tag(
        70,
        "COLLECTION",
        &[
            ("TITLE", meta.show.clone()),
            ("GENRE", genre),
            ("IMDB", meta.imdb.clone()),
            ("TMDB", meta.tmdb.clone()),
        ],
    );
    out.extend(tag(
        60,
        "SEASON",
        &[("PART_NUMBER", meta.season.map(|s| s.to_string()).unwrap_or_default())],
    ));
    out.extend(tag(
        50,
        "EPISODE",
        &[
            ("TITLE", meta.title.clone()),
            ("PART_NUMBER", meta.episode.map(|e| e.to_string()).unwrap_or_default()),
            ("DATE_RELEASED", meta.date.clone()),
            ("SYNOPSIS", meta.description.clone()),
        ],
    ));
    out
}

/// `Show - S01E02 - Title`, as a segment title.
fn episode_title(meta: &VideoMeta) -> String {
    let mut s = meta.show.clone();
    if let (Some(se), Some(ep)) = (meta.season, meta.episode) {
        s.push_str(&format!(" - S{se:02}E{ep:02}"));
    }
    if !meta.title.is_empty() {
        s.push_str(" - ");
        s.push_str(&meta.title);
    }
    s
}

// ----------------------------------------------------------------- MP4

/// A top-level box: kind, where, header length, whole length (`0` in its
/// header: to the end).
#[derive(Debug, Clone, Copy)]
struct Bx {
    kind: [u8; 4],
    at: u64,
    head: u64,
    len: u64,
    to_end: bool,
}

/// The boxes inside `data`: kind, header length, content.
fn kids(data: &[u8]) -> Vec<([u8; 4], &[u8], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 8 <= data.len() {
        let mut len = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        let mut head = 8;
        if len == 1 {
            let Some(b) = data.get(at + 8..at + 16) else { break };
            len = u64::from_be_bytes(b.try_into().unwrap()) as usize;
            head = 16;
        } else if len == 0 {
            len = data.len() - at;
        }
        if len < head || at + len > data.len() {
            break;
        }
        let kind: [u8; 4] = data[at + 4..at + 8].try_into().unwrap();
        out.push((kind, &data[at..at + head], &data[at + head..at + len]));
        at += len;
    }
    out
}

fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    v.extend_from_slice(kind);
    v.extend_from_slice(body);
    v
}

/// An `ilst` item with one `data` box: type (1 text, 13 JPEG, 14 PNG, 21
/// integer) and value.
fn item(kind: &[u8; 4], ty: u32, value: &[u8]) -> Vec<u8> {
    let mut d = ty.to_be_bytes().to_vec();
    d.extend([0, 0, 0, 0]);
    d.extend_from_slice(value);
    mp4_box(kind, &mp4_box(b"data", &d))
}

/// The items written: kinds and boxes.
fn our_items(meta: &VideoMeta) -> Vec<([u8; 4], Vec<u8>)> {
    let mut out: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    let mut text = |k: &[u8; 4], v: &str| {
        if !v.is_empty() {
            out.push((*k, item(k, 1, v.as_bytes())));
        }
    };
    text(b"\xa9nam", &meta.title);
    text(b"\xa9day", &meta.date);
    // The short description holds at most 255 characters.
    let short: String = meta.description.chars().take(255).collect();
    text(b"desc", &short);
    text(b"ldes", &meta.description);
    text(b"\xa9gen", &meta.genres.join(", "));
    if meta.is_episode() {
        text(b"tvsh", &meta.show);
        if let (Some(s), Some(e)) = (meta.season, meta.episode) {
            text(b"tven", &format!("S{s:02}E{e:02}"));
        }
    }
    if meta.is_episode() {
        if let Some(s) = meta.season {
            out.push((*b"tvsn", item(b"tvsn", 21, &s.to_be_bytes())));
        }
        if let Some(e) = meta.episode {
            out.push((*b"tves", item(b"tves", 21, &e.to_be_bytes())));
        }
    }
    // Media kind: 9 movie, 10 TV show.
    let stik = if meta.is_episode() { 10u8 } else { 9 };
    out.push((*b"stik", item(b"stik", 21, &[stik])));
    if let Some(c) = &meta.cover {
        let ty = if c.starts_with(b"\x89PNG") { 14 } else { 13 };
        out.push((*b"covr", item(b"covr", ty, c)));
    }
    out
}

/// `moov` with the items of `meta` in `udta/meta/ilst` (others kept).
fn new_moov(moov: &[u8], meta: &VideoMeta) -> Option<Vec<u8>> {
    let ours = our_items(meta);
    let mut body = Vec::new();
    let mut had_udta = false;
    for (kind, head, content) in kids(moov) {
        if &kind == b"mvex" {
            // A fragmented file: its index is not the whole story.
            return None;
        }
        if &kind != b"udta" {
            body.extend_from_slice(head);
            body.extend_from_slice(content);
            continue;
        }
        had_udta = true;
        body.extend(mp4_box(b"udta", &new_udta(content, &ours)));
    }
    if !had_udta {
        body.extend(mp4_box(b"udta", &new_udta(&[], &ours)));
    }
    Some(mp4_box(b"moov", &body))
}

fn new_udta(udta: &[u8], ours: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut had_meta = false;
    for (kind, head, content) in kids(udta) {
        if &kind != b"meta" || had_meta {
            out.extend_from_slice(head);
            out.extend_from_slice(content);
            continue;
        }
        had_meta = true;
        // ISO's meta has a version and flags; QuickTime's does not.
        let full = content.len() >= 4 && content[..4] == [0, 0, 0, 0] && {
            kids(&content[4..]).first().is_some_and(|k| &k.0 == b"hdlr")
        };
        let inner = if full { &content[4..] } else { content };
        let mut m = if full { vec![0, 0, 0, 0] } else { Vec::new() };
        let mut had_ilst = false;
        let mut had_hdlr = false;
        for (k, h, c) in kids(inner) {
            if &k == b"hdlr" {
                had_hdlr = true;
            }
            if &k == b"ilst" {
                had_ilst = true;
                m.extend(new_ilst(c, ours));
            } else {
                m.extend_from_slice(h);
                m.extend_from_slice(c);
            }
        }
        if !had_hdlr {
            let mut with = handler();
            with.extend(m.split_off(if full { 4 } else { 0 }));
            m.extend(with);
        }
        if !had_ilst {
            m.extend(new_ilst(&[], ours));
        }
        out.extend(mp4_box(b"meta", &m));
    }
    if !had_meta {
        let mut m = vec![0, 0, 0, 0];
        m.extend(handler());
        m.extend(new_ilst(&[], ours));
        out.extend(mp4_box(b"meta", &m));
    }
    out
}

/// The handler that says the metadata is iTunes-style.
fn handler() -> Vec<u8> {
    let mut h = vec![0u8; 8];
    h.extend_from_slice(b"mdirappl");
    h.extend([0u8; 9]);
    mp4_box(b"hdlr", &h)
}

fn new_ilst(ilst: &[u8], ours: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (kind, head, content) in kids(ilst) {
        // Ours replace theirs; a description or TV detail not written
        // this time goes too (it would belong to something else).
        let replaced = ours.iter().any(|(k, _)| *k == kind)
            || [*b"desc", *b"ldes", *b"tvsh", *b"tven", *b"tvsn", *b"tves", *b"stik"].contains(&kind);
        if !replaced {
            out.extend_from_slice(head);
            out.extend_from_slice(content);
        }
    }
    for (_, b) in ours {
        out.extend_from_slice(b);
    }
    mp4_box(b"ilst", &out)
}

fn mp4_tops(f: &mut File, len: u64) -> Result<Vec<Bx>, String> {
    let bad = || "videoUnsupported".to_string();
    let mut out = Vec::new();
    let mut at = 0u64;
    while at < len {
        if len - at < 8 {
            return Err(bad());
        }
        let h = read_at(f, at, (len - at).min(16) as usize)?;
        let mut size = u64::from(u32::from_be_bytes(h[..4].try_into().unwrap()));
        let kind: [u8; 4] = h[4..8].try_into().unwrap();
        let mut head = 8;
        let mut to_end = false;
        if size == 1 {
            size = u64::from_be_bytes(h.get(8..16).ok_or_else(bad)?.try_into().unwrap());
            head = 16;
        } else if size == 0 {
            size = len - at;
            to_end = true;
        }
        if size < head || at + size > len {
            return Err(bad());
        }
        out.push(Bx {
            kind,
            at,
            head,
            len: size,
            to_end,
        });
        at += size;
    }
    Ok(out)
}

/// The MP4 changes: the new `moov` where the old one is, when it fits
/// (with the `free` boxes after it), else at the end with the old one
/// turned into `free`.
fn mp4_plan(f: &mut File, len: u64, meta: &VideoMeta) -> Result<Option<Plan>, String> {
    let bad = || "videoUnsupported".to_string();
    let tops = mp4_tops(f, len)?;
    let i = tops
        .iter()
        .position(|b| &b.kind == b"moov")
        .ok_or_else(bad)?;
    if tops.iter().filter(|b| &b.kind == b"moov").count() != 1 || tops.iter().any(|b| &b.kind == b"moof") {
        return Err(bad());
    }
    let mv = tops[i];
    if mv.len > 256 << 20 {
        return Err(bad());
    }
    let old = read_at(f, mv.at + mv.head, (mv.len - mv.head) as usize)?;
    let new = new_moov(&old, meta).ok_or_else(bad)?;
    // The room it has: itself and the free boxes right after it.
    let mut room = mv.len;
    for b in &tops[i + 1..] {
        if (&b.kind == b"free" || &b.kind == b"skip") && !b.to_end {
            room += b.len;
        } else {
            break;
        }
    }
    let n = new.len() as u64;
    let last = i == tops.len() - 1;
    if n == room || (n < room && room - n >= 8) {
        let mut bytes = new;
        if room > n {
            bytes.extend(mp4_box(b"free", &vec![0u8; (room - n - 8) as usize]));
        }
        return Ok(Some(Plan {
            patches: vec![(mv.at, bytes)],
            cut: len,
            tail: Vec::new(),
        }));
    }
    if last {
        // The index is the end of the file: rewritten there.
        return Ok(Some(Plan {
            patches: Vec::new(),
            cut: mv.at,
            tail: new,
        }));
    }
    // Moved to the end: the old one becomes free space. A box that runs
    // "to the end" must say its length first.
    let mut patches = vec![(mv.at + 4, b"free".to_vec())];
    if let Some(end) = tops.last().filter(|b| b.to_end) {
        if end.len > u64::from(u32::MAX) {
            return Err("videoNoRoom".into());
        }
        patches.push((end.at, (end.len as u32).to_be_bytes().to_vec()));
    }
    Ok(Some(Plan {
        patches,
        cut: len,
        tail: new,
    }))
}

// ------------------------------------------------------------- reading

/// What a video carries (for tests and checks): title, date, show,
/// season, episode, and whether it has a cover.
pub fn read(path: &Path) -> Option<(VideoMeta, bool)> {
    match kind(path)? {
        Kind::Matroska => read_mkv(path),
        Kind::Mp4 => read_mp4(path),
    }
}

fn read_mkv(path: &Path) -> Option<(VideoMeta, bool)> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let (_, size, h) = header_at(&mut f, 0)?;
    let seg_at = h + size?;
    let (_, seg_size, head) = header_at(&mut f, seg_at)?;
    let data = seg_at + head;
    let end = (data + seg_size?).min(len);
    let mut at = data;
    let mut meta = VideoMeta::default();
    let mut cover = false;
    while at < end {
        let (id, size, head) = header_at(&mut f, at)?;
        let size = size?;
        if id == TAGS || id == ATTACHMENTS {
            let b = read_at(&mut f, at + head, size as usize).ok()?;
            if id == ATTACHMENTS {
                cover |= ebml(&b).any(|(i, v)| i == 0x61a7 && is_cover(v));
            }
            for (_, t) in ebml(&b).filter(|(i, _)| *i == 0x7373 && id == TAGS) {
                let mut level = 50;
                for (i, v) in ebml(t) {
                    if i == 0x63c0 {
                        for (ti, tv) in ebml(v) {
                            if ti == 0x68ca {
                                level = ebml_uint(tv);
                            }
                        }
                    }
                    if i == 0x67c8 {
                        let mut name = String::new();
                        let mut value = String::new();
                        for (si, sv) in ebml(v) {
                            match si {
                                0x45a3 => name = String::from_utf8_lossy(sv).into_owned(),
                                0x4487 => value = String::from_utf8_lossy(sv).into_owned(),
                                _ => {}
                            }
                        }
                        match (level, name.as_str()) {
                            (70, "TITLE") => meta.show = value,
                            (60, "PART_NUMBER") => meta.season = value.parse().ok(),
                            (50, "PART_NUMBER") => meta.episode = value.parse().ok(),
                            (50, "TITLE") => meta.title = value,
                            (50, "DATE_RELEASED") => meta.date = value,
                            _ => {}
                        }
                    }
                }
            }
        }
        at += head + size;
    }
    Some((meta, cover))
}

fn read_mp4(path: &Path) -> Option<(VideoMeta, bool)> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let tops = mp4_tops(&mut f, len).ok()?;
    let mv = tops.iter().find(|b| &b.kind == b"moov")?;
    let moov = read_at(&mut f, mv.at + mv.head, (mv.len - mv.head) as usize).ok()?;
    let (_, _, udta) = kids(&moov).into_iter().find(|k| &k.0 == b"udta")?;
    let (_, _, meta_box) = kids(udta).into_iter().find(|k| &k.0 == b"meta")?;
    let inner = if meta_box.get(..4) == Some(&[0, 0, 0, 0]) {
        &meta_box[4..]
    } else {
        meta_box
    };
    let (_, _, ilst) = kids(inner).into_iter().find(|k| &k.0 == b"ilst")?;
    let mut meta = VideoMeta::default();
    let mut cover = false;
    for (kind, _, content) in kids(ilst) {
        let Some((_, _, data)) = kids(content).into_iter().find(|k| &k.0 == b"data") else {
            continue;
        };
        let value = data.get(8..).unwrap_or(&[]);
        let text = || String::from_utf8_lossy(value).into_owned();
        let num = || {
            value
                .iter()
                .fold(0u32, |n, b| (n << 8) | u32::from(*b))
        };
        match &kind {
            b"\xa9nam" => meta.title = text(),
            b"\xa9day" => meta.date = text(),
            b"tvsh" => meta.show = text(),
            b"tvsn" => meta.season = Some(num()),
            b"tves" => meta.episode = Some(num()),
            b"covr" => cover = true,
            _ => {}
        }
    }
    Some((meta, cover))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small Matroska file: SeekHead (with room after it), Info, Tracks,
    /// a Cluster, and track tags as mkvmerge writes them.
    fn mkv(with_tags: bool) -> Vec<u8> {
        let info = el(INFO, &[uint_el(0x2ad7b1, 1_000_000), str_el(0x4d80, "muxer")].concat());
        let tracks = el(0x1654_ae6b, &el(0xae, &[uint_el(0xd7, 1), uint_el(0x83, 1)].concat()));
        let cluster = el(CLUSTER, &[uint_el(0xe7, 0), el(0xa3, &[0x81, 0, 0, 0x80, 1, 2, 3])].concat());
        let track_tag = {
            let mut t = el(0x63c0, &uint_el(0x63c5, 7));
            t.extend(simple("BPS", "1000"));
            el(0x7373, &t)
        };
        let old_global = tag(50, "MOVIE", &[("TITLE", "Old".into()), ("COMMENT", "keep me".into())]);
        let tags = el(TAGS, &[track_tag, old_global].concat());
        // Positions inside the segment: SeekHead (fixed 64 bytes with its
        // Void), then Info, Tracks, Cluster, Tags.
        let head_room = 64usize;
        let info_at = head_room as u64;
        let tracks_at = info_at + info.len() as u64;
        let cluster_at = tracks_at + tracks.len() as u64;
        let tags_at = cluster_at + cluster.len() as u64;
        let mut entries = Vec::new();
        for (id, pos) in [(INFO, info_at), (0x1654_ae6b, tracks_at)]
            .into_iter()
            .chain(with_tags.then_some((TAGS, tags_at)))
        {
            let mut e = el(0x53ab, &id_bytes(id));
            e.extend(uint_el(0x53ac, pos));
            entries.extend(el(0x4dbb, &e));
        }
        let mut sh = el(SEEKHEAD, &entries);
        let pad = head_room - sh.len();
        sh.extend(void(pad as u64).unwrap());
        let mut seg = sh;
        seg.extend(info);
        seg.extend(tracks);
        seg.extend(cluster);
        if with_tags {
            seg.extend(tags);
        }
        let mut file = el(0x1a45_dfa3, &str_el(0x4282, "matroska"));
        file.extend(el_len(SEGMENT, &seg, Some(8)));
        file
    }

    fn meta() -> VideoMeta {
        VideoMeta {
            title: "Some Movie".into(),
            date: "2010-07-16".into(),
            description: "A story.".into(),
            genres: vec!["Drama".into()],
            tmdb: "movie/1".into(),
            cover: Some(b"\xFF\xD8\xFFjpeg".to_vec()),
            ..Default::default()
        }
    }

    #[test]
    fn matroska_written_and_put_back() {
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("store");
        for with_tags in [true, false] {
            let p = d.path().join("m.mkv");
            let old = mkv(with_tags);
            fs::write(&p, &old).unwrap();
            let edit = write(&p, &meta(), &store).unwrap().unwrap();
            let (got, cover) = read(&p).unwrap();
            assert_eq!(got.title, "Some Movie");
            assert_eq!(got.date, "2010-07-16");
            assert!(cover);
            let bytes = fs::read(&p).unwrap();
            // The old tags of the track and the old comment stay.
            let text = String::from_utf8_lossy(&bytes);
            if with_tags {
                assert!(text.contains("BPS") && text.contains("keep me"));
                assert_eq!(text.matches("Old").count(), 1, "old tags left as Void");
            }
            // The title in Info, where it was (room in its Void? none:
            // Info is followed by Tracks, so only when it fits).
            // Walked again: every top element valid and the SeekHead
            // pointing to the new Tags.
            let mut f = File::open(&p).unwrap();
            let plan = mkv_plan(&mut f, bytes.len() as u64, &meta()).unwrap();
            assert!(plan.is_some());
            // Written twice: still one cover.
            let edit2 = write(&p, &meta(), &store).unwrap().unwrap();
            restore(&p, &edit2, &store).unwrap();
            restore(&p, &edit, &store).unwrap();
            assert_eq!(fs::read(&p).unwrap(), old);
        }
    }

    #[test]
    fn matroska_episode() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("e.mkv");
        fs::write(&p, mkv(true)).unwrap();
        let m = VideoMeta {
            title: "Pilot".into(),
            show: "Some Show".into(),
            season: Some(1),
            episode: Some(2),
            ..Default::default()
        };
        write(&p, &m, &d.path().join("s")).unwrap().unwrap();
        let (got, cover) = read(&p).unwrap();
        assert_eq!(
            (got.show.as_str(), got.season, got.episode, got.title.as_str()),
            ("Some Show", Some(1), Some(2), "Pilot")
        );
        assert!(!cover);
    }

    /// A small MP4: ftyp, moov (mvhd, an old udta), mdat; or with moov
    /// after mdat.
    fn mp4(moov_last: bool, free_after: usize) -> Vec<u8> {
        let ftyp = mp4_box(b"ftyp", b"isom\0\0\0\0isom");
        let ilst = mp4_box(b"ilst", &[item(b"\xa9too", 1, b"encoder"), item(b"\xa9nam", 1, b"Old")].concat());
        let mut meta = vec![0, 0, 0, 0];
        meta.extend(handler());
        meta.extend(ilst);
        let udta = mp4_box(b"udta", &mp4_box(b"meta", &meta));
        let moov = mp4_box(b"moov", &[mp4_box(b"mvhd", &[0u8; 100]), udta].concat());
        let mdat = mp4_box(b"mdat", &[7u8; 1000]);
        let free = if free_after >= 8 {
            mp4_box(b"free", &vec![0u8; free_after - 8])
        } else {
            Vec::new()
        };
        if moov_last {
            [ftyp, mdat, moov].concat()
        } else {
            [ftyp, moov, free, mdat].concat()
        }
    }

    #[test]
    fn mp4_written_and_put_back() {
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("store");
        for (last, free) in [(false, 0), (false, 4096), (true, 0)] {
            let p = d.path().join("v.mp4");
            let old = mp4(last, free);
            fs::write(&p, &old).unwrap();
            let edit = write(&p, &meta(), &store).unwrap().unwrap();
            let (got, cover) = read(&p).unwrap();
            assert_eq!((got.title.as_str(), got.date.as_str()), ("Some Movie", "2010-07-16"));
            assert!(cover);
            let bytes = fs::read(&p).unwrap();
            // The media data is where it was.
            let mdat = old.windows(4).position(|w| w == b"mdat").unwrap();
            assert_eq!(&bytes[mdat..mdat + 4], b"mdat");
            assert_eq!(&bytes[mdat + 4..mdat + 1004], &[7u8; 1000][..]);
            // The encoder item stays.
            assert!(String::from_utf8_lossy(&bytes).contains("encoder"));
            if free == 4096 {
                assert_eq!(bytes.len(), old.len(), "fits in the free box");
            }
            restore(&p, &edit, &store).unwrap();
            assert_eq!(fs::read(&p).unwrap(), old, "last={last} free={free}");
        }
    }

    #[test]
    fn changed_file_is_not_put_back() {
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("store");
        let p = d.path().join("v.mp4");
        fs::write(&p, mp4(false, 0)).unwrap();
        let edit = write(&p, &meta(), &store).unwrap().unwrap();
        let mut f = File::options().append(true).open(&p).unwrap();
        f.write_all(b"more").unwrap();
        assert_eq!(restore(&p, &edit, &store).unwrap_err(), "videoChanged");
    }

    #[test]
    fn voids_of_any_size() {
        for n in 2..300u64 {
            assert_eq!(void(n).unwrap().len() as u64, n);
        }
        assert!(void(1).is_none());
        let big = void(100_000).unwrap();
        assert_eq!(big.len(), 100_000);
        let (id, a) = vint(&big, true).unwrap();
        let (size, b) = vint(&big[a..], false).unwrap();
        assert_eq!((id, size + (a + b) as u64), (VOID, 100_000));
    }
}
