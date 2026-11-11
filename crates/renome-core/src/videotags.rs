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
    /// The original title (an episode's: its show's).
    pub original_title: String,
    pub tagline: String,
    /// The age rating as media servers write it: `PG-13` in the US,
    /// `BR-14` elsewhere.
    pub certification: String,
    /// TMDB's average, 0 to 10, as written (`7.8`).
    pub rating: String,
    pub keywords: Vec<String>,
    pub countries: Vec<String>,
    pub studios: Vec<String>,
    /// An episode's channel.
    pub network: String,
    pub directors: Vec<String>,
    pub writers: Vec<String>,
    pub producers: Vec<String>,
    pub composers: Vec<String>,
    /// A movie's cast, or a show's main cast.
    pub cast: Vec<Person>,
    /// An episode's guest stars.
    pub guests: Vec<Person>,
    /// The collection a movie belongs to, and its place in it.
    pub collection: String,
    pub collection_part: Option<u32>,
    /// A YouTube address.
    pub trailer: String,
    /// A wide picture (the movie's or show's background).
    #[serde(skip)]
    pub backdrop: Option<Vec<u8>>,
}

/// An actor and the part played.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Person {
    pub name: String,
    pub character: String,
}

/// How a write treats what the video had.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Options {
    /// Whatever details and pictures came in the video go (sites'
    /// names, comments, old covers); the fonts its subtitles use, the
    /// details of each track (bit rate, frame count) and the chapters
    /// stay.
    pub clean: bool,
    /// The audio and subtitle tracks are named after their language and
    /// what tells them apart (see [`crate::tracknames`]).
    pub tracks: bool,
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
pub fn write(path: &Path, meta: &VideoMeta, opt: &Options, store: &Path) -> Result<Option<VideoEdit>, String> {
    let k = kind(path).ok_or("videoUnsupported")?;
    let mut f = File::options()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    let plan = match k {
        Kind::Matroska => mkv_plan(&mut f, len, meta, opt)?,
        Kind::Mp4 => mp4_plan(&mut f, len, meta, opt)?,
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
        .and_then(|_| crate::sync_file(&saved))
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
const TRACKS: u64 = 0x1654_ae6b;
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
fn mkv_plan(f: &mut File, len: u64, meta: &VideoMeta, opt: &Options) -> Result<Option<Plan>, String> {
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
    // Elements no longer there, whose SeekHead entries go.
    let mut gone: Vec<u64> = Vec::new();
    // Old elements made empty space (where they start).
    let mut freed: Vec<u64> = Vec::new();

    // Tags: the old ones' tags of tracks and the like kept, ours added
    // (cleaning: only those of tracks, chapters and attachments).
    let old_tags = find(TAGS).next();
    let mut tags = Vec::new();
    if let Some(t) = old_tags {
        let b = body(f, &t)?;
        for (id, tag) in ebml(&b) {
            if id == 0x7373
                && let Some(kept) = keep_tag(tag, meta, opt.clean)
            {
                tags.extend(kept);
            }
        }
        freed.push(t.at);
    }
    tags.extend(our_tags(meta));
    let tags_el = el(TAGS, &tags);
    moved.push((TAGS, seg_end + tail.len() as u64 - data));
    tail.extend(&tags_el);

    // Attachments: the covers (`cover.jpg`, `cover_land.jpg`, as players
    // look for them) in place of older ones; the other files (fonts)
    // kept, and when cleaning only the fonts.
    let old_att = find(ATTACHMENTS).next();
    let old_files: Vec<Vec<u8>> = match old_att {
        Some(t) if t.size <= MAX_ATTACHMENTS => ebml(&body(f, &t)?)
            .filter(|(id, _)| *id == 0x61a7)
            .map(|(_, v)| v.to_vec())
            .collect(),
        _ => Vec::new(),
    };
    let covers: Vec<(&str, &Vec<u8>)> = [("cover", &meta.cover), ("cover_land", &meta.backdrop)]
        .into_iter()
        .filter_map(|(n, c)| c.as_ref().map(|c| (n, c)))
        .collect();
    let unwanted = |file: &[u8]| is_cover(file) || (opt.clean && !is_font(file));
    if old_att.is_some_and(|t| t.size > MAX_ATTACHMENTS) && (!covers.is_empty() || opt.clean) {
        return Err("videoNoRoom".into());
    }
    if !covers.is_empty() || old_files.iter().any(|f| unwanted(f)) {
        let mut files = Vec::new();
        for file in old_files.iter().filter(|f| !unwanted(f)) {
            files.extend(el(0x61a7, file));
        }
        for (name, cover) in &covers {
            let png = cover.starts_with(b"\x89PNG");
            let mut file = str_el(0x466e, &format!("{name}.{}", if png { "png" } else { "jpg" }));
            file.extend(str_el(0x4660, if png { "image/png" } else { "image/jpeg" }));
            file.extend(el(0x465c, cover));
            // The UID from the picture and its name: the same picture
            // as poster and background still gets two.
            file.extend(uint_el(0x46ae, (hash(cover) ^ hash(name.as_bytes())) | 1));
            files.extend(el(0x61a7, &file));
        }
        if files.is_empty() {
            gone.push(ATTACHMENTS);
        } else {
            moved.push((ATTACHMENTS, seg_end + tail.len() as u64 - data));
            tail.extend(el(ATTACHMENTS, &files));
        }
        if let Some(t) = old_att {
            freed.push(t.at);
        }
    }

    // Info (with the title) and Tracks (with the tracks' names) rewritten
    // where they are, in the room up to the next element kept: each with
    // its choices, the best first.
    let mut choices: Vec<(Top, Vec<Vec<u8>>)> = Vec::new();
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
        // Too long to fit: an old title (often a site's name) goes, and
        // players show the tags' title instead.
        let mut opts = vec![with];
        if had_title {
            opts.push(rest);
        }
        opts.push(b);
        choices.push((info, opts));
    }
    if opt.tracks
        && let Some(tr) = find(TRACKS).next()
    {
        let b = body(f, &tr)?;
        if let Some(named) = named_tracks(&b) {
            choices.push((tr, vec![named, b]));
        }
    }
    choices.sort_by_key(|c| c.0.at);
    let free_at = |t: &Top| t.id == VOID || freed.contains(&t.at);
    let mut covered: Vec<u64> = Vec::new();
    let mut i = 0;
    while i < choices.len() {
        // The run: from this element on, while the next ones are this
        // list's, Voids or old elements made empty.
        let start = choices[i].0;
        let first = tops.iter().position(|t| t.at == start.at).ok_or_else(bad)?;
        let mut members: Vec<usize> = Vec::new();
        let mut end = start.at;
        let mut run: Vec<u64> = Vec::new();
        for t in tops.iter().skip(first) {
            let chosen = choices.iter().position(|c| c.0.at == t.at);
            if chosen.is_none() && !free_at(t) {
                break;
            }
            if let Some(c) = chosen {
                members.push(c);
            }
            run.push(t.at);
            end = t.end();
        }
        i = members.iter().max().map_or(i + 1, |m| m + 1);
        let room = end - start.at;
        let mut placed = None;
        for combo in combos(&members.iter().map(|&m| choices[m].1.len()).collect::<Vec<_>>()) {
            // The last choice of each is the element as it was.
            let same = members.iter().zip(&combo).all(|(&m, &c)| c == choices[m].1.len() - 1);
            let parts: Vec<(u64, &Vec<u8>)> = members.iter().zip(&combo).map(|(&m, &c)| (choices[m].0.id, &choices[m].1[c])).collect();
            if same {
                break;
            }
            if let Some(bytes) = lay_out(&parts, room) {
                placed = Some((parts, bytes));
                break;
            }
        }
        let Some((parts, bytes)) = placed else { continue };
        covered.extend(run);
        let mut at = start.at;
        for (&m, (id, kids)) in members.iter().zip(&parts) {
            if at != choices[m].0.at {
                moved.push((*id, at - data));
            }
            at += el(*id, kids).len() as u64;
        }
        patches.push((start.at, bytes));
    }

    // The old ones become empty space (unless inside a run rewritten).
    for at in &freed {
        if covered.contains(at) {
            continue;
        }
        let t = tops.iter().find(|t| t.at == *at).ok_or_else(bad)?;
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

    // The SeekHeads: the entries of the moved elements changed (those
    // gone left out), or added to the first one.
    let heads: Vec<Top> = find(SEEKHEAD).collect();
    let first = heads.first().copied().ok_or("videoNoRoom")?;
    let entries_of = |b: &[u8]| -> Vec<(Vec<u8>, u64)> {
        ebml(b)
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
            .collect()
    };
    let mut done: Vec<u64> = Vec::new();
    for sh in &heads {
        let b = body(f, sh)?;
        let mut entries = entries_of(&b);
        let mut changed = false;
        for (id, pos) in &moved {
            let want = id_bytes(*id);
            for e in entries.iter_mut().filter(|e| e.0 == want) {
                e.1 = *pos;
                changed = true;
                done.push(*id);
            }
        }
        let before = entries.len();
        entries.retain(|e| !gone.iter().any(|g| id_bytes(*g) == e.0));
        changed |= entries.len() != before;
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
        let mut entries = entries_of(&b);
        entries.retain(|e| !gone.iter().any(|g| id_bytes(*g) == e.0));
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

/// A font attached for the subtitles (by its type or its name).
fn is_font(file: &[u8]) -> bool {
    ebml(file).any(|(id, v)| {
        let s = String::from_utf8_lossy(v).to_ascii_lowercase();
        match id {
            0x4660 => {
                s.starts_with("font/")
                    || s.starts_with("application/x-font")
                    || s.starts_with("application/font-")
                    || ["application/x-truetype-font", "application/vnd.ms-opentype", "application/vnd.ms-fontobject"]
                        .contains(&s.as_str())
            }
            0x466e => [".ttf", ".otf", ".ttc", ".woff", ".woff2", ".pfb", ".fon"]
                .iter()
                .any(|e| s.ends_with(e)),
            _ => false,
        }
    })
}

/// Every way of picking one choice of each element (`lens`: how many
/// each has), those with the earliest choices first.
fn combos(lens: &[usize]) -> Vec<Vec<usize>> {
    let mut all: Vec<Vec<usize>> = vec![Vec::new()];
    for &n in lens {
        all = all
            .into_iter()
            .flat_map(|c| {
                (0..n).map(move |k| {
                    let mut c = c.clone();
                    c.push(k);
                    c
                })
            })
            .collect();
    }
    all.sort_by_key(|c| (c.iter().sum::<usize>(), c.clone()));
    all
}

/// The elements one after another in `room` bytes, the rest a Void (only
/// its header written); `None` when they do not fit.
fn lay_out(parts: &[(u64, &Vec<u8>)], room: u64) -> Option<Vec<u8>> {
    let mut bytes: Vec<u8> = parts.iter().flat_map(|(id, kids)| el(*id, kids)).collect();
    let n = bytes.len() as u64;
    if n > room {
        return None;
    }
    match room - n {
        0 => {}
        // Too little for a Void: the last element's size a byte longer.
        1 => {
            let (id, kids) = parts.last()?;
            let last = el(*id, kids);
            bytes.truncate(bytes.len() - last.len());
            bytes.extend(el_len(*id, kids, Some(min_size_len(kids.len() as u64) + 1)));
            if bytes.len() as u64 != room {
                return None;
            }
        }
        left => bytes.extend(void_head(left)?),
    }
    Some(bytes)
}

/// Short names of Matroska codecs.
fn mkv_codec(id: &str) -> String {
    let s = match id {
        "A_AC3" => "AC3",
        "A_EAC3" => "E-AC3",
        "A_DTS" | "A_DTS/EXPRESS" | "A_DTS/LOSSLESS" => "DTS",
        "A_TRUEHD" | "A_MLP" => "TrueHD",
        "A_FLAC" => "FLAC",
        "A_OPUS" => "Opus",
        "A_VORBIS" => "Vorbis",
        "A_MPEG/L3" => "MP3",
        "A_MPEG/L2" => "MP2",
        "A_ALAC" => "ALAC",
        "S_TEXT/UTF8" | "S_TEXT/ASCII" => "SRT",
        "S_TEXT/ASS" | "S_ASS" => "ASS",
        "S_TEXT/SSA" | "S_SSA" => "SSA",
        "S_TEXT/WEBVTT" => "WebVTT",
        "S_HDMV/PGS" => "PGS",
        "S_HDMV/TEXTST" => "TextST",
        "S_VOBSUB" => "VobSub",
        "S_DVBSUB" => "DVB",
        _ if id.starts_with("A_AAC") => "AAC",
        _ if id.starts_with("A_PCM") => "PCM",
        _ => "",
    };
    s.to_string()
}

/// The track entries of Tracks, and each one as [`crate::tracknames`]
/// reads it.
fn mkv_tracks(tracks: &[u8]) -> (Vec<&[u8]>, Vec<crate::tracknames::TrackInfo>) {
    use crate::tracknames::{TrackInfo, TrackKind};
    let entries: Vec<&[u8]> = ebml(tracks).filter(|(id, _)| *id == 0xae).map(|(_, v)| v).collect();
    let infos: Vec<TrackInfo> = entries
        .iter()
        .map(|e| {
            let mut t = TrackInfo::default();
            let (mut lang, mut bcp) = (None, None);
            for (id, v) in ebml(e) {
                let text = || String::from_utf8_lossy(v).trim_end_matches('\0').to_string();
                match id {
                    0x83 => {
                        t.kind = match ebml_uint(v) {
                            1 => TrackKind::Video,
                            2 => TrackKind::Audio,
                            17 => TrackKind::Subtitle,
                            _ => TrackKind::Other,
                        }
                    }
                    0x536e => t.name = text(),
                    0x22b59c => lang = Some(text()),
                    0x22b59d => bcp = Some(text()),
                    0x86 => t.codec = mkv_codec(&text()),
                    0x55aa => t.forced = ebml_uint(v) != 0,
                    0x55ab => t.hearing = ebml_uint(v) != 0,
                    0x55ac => t.described = ebml_uint(v) != 0,
                    0x55af => t.commentary = ebml_uint(v) != 0,
                    0xe1 => {
                        for (aid, av) in ebml(v) {
                            if aid == 0x9f {
                                t.channels = ebml_uint(av) as u32;
                            }
                        }
                    }
                    _ => {}
                }
            }
            // Matroska's default language is English.
            t.lang = bcp.filter(|s| !s.is_empty()).or(lang).unwrap_or_else(|| "eng".into());
            t
        })
        .collect();
    (entries, infos)
}

/// The children of Tracks with each track's new name (and the flags and
/// language its old name told); `None` when nothing changes.
fn named_tracks(tracks: &[u8]) -> Option<Vec<u8>> {
    let (entries, infos) = mkv_tracks(tracks);
    let new = crate::tracknames::names(&infos);    let mut out = Vec::new();
    let mut changed = false;
    for ((e, old), n) in entries.iter().zip(&infos).zip(&new) {
        let mut kids = Vec::new();
        let flags = [(0x55aa, n.forced && !old.forced), (0x55ab, n.hearing && !old.hearing), (0x55ac, n.described && !old.described), (0x55af, n.commentary && !old.commentary)];
        for (id, v) in ebml(e) {
            match id {
                0x536e | CRC32 => continue,
                _ if flags.iter().any(|(f, set)| *f == id && *set) => continue,
                0x22b59c if n.lang.is_some() => kids.extend(str_el(id, n.lang.unwrap_or_default())),
                // The language tag of a track that said none: the two
                // letters (Matroska keeps both in step).
                0x22b59d if n.lang.is_some() => {
                    let two = crate::subtitles::langs(n.lang.unwrap_or_default());
                    match two.first() {
                        Some(l) => kids.extend(str_el(id, &l.tag)),
                        None => kids.extend(el(id, v)),
                    }
                }
                _ => kids.extend(el(id, v)),
            }
        }
        if !n.name.is_empty() {
            kids.extend(str_el(0x536e, &n.name));
        }
        for (id, set) in flags {
            if set {
                kids.extend(uint_el(id, 1));
            }
        }
        changed |= n.name != old.name || n.lang.is_some() || flags.iter().any(|f| f.1);
        out.extend(el(0xae, &kids));
    }
    // What is not a track (a CRC, a Void) is left out.
    changed.then_some(out)
}

/// The names of the simple tags written, per target level.
fn our_names(level: u64, meta: &VideoMeta) -> &'static [&'static str] {
    const WORK: &[&str] = &[
        "TITLE", "DATE_RELEASED", "DESCRIPTION", "SYNOPSIS", "SUMMARY", "GENRE", "IMDB", "TMDB", "ORIGINAL_TITLE",
        "TAGLINE", "LAW_RATING", "RATING", "KEYWORDS", "COUNTRY", "PRODUCTION_STUDIO", "DIRECTOR", "WRITTEN_BY",
        "PRODUCER", "COMPOSER", "ACTOR", "PART_NUMBER", "TRAILER",
    ];
    const SHOW: &[&str] = &[
        "TITLE", "IMDB", "TMDB", "GENRE", "ORIGINAL_TITLE", "TAGLINE", "LAW_RATING", "KEYWORDS", "PRODUCTION_STUDIO",
        "ACTOR", "TRAILER",
    ];
    const EPISODE: &[&str] = &[
        "TITLE", "PART_NUMBER", "DATE_RELEASED", "DESCRIPTION", "SYNOPSIS", "SUMMARY", "DIRECTOR", "WRITTEN_BY",
        "ACTOR", "RATING",
    ];
    match (meta.is_episode(), level) {
        (false, 50) => WORK,
        (false, 70) => &["TITLE"],
        (true, 70) => SHOW,
        (true, 60) => &["PART_NUMBER"],
        (true, 50) => EPISODE,
        _ => &[],
    }
}

/// An old Tag kept: those of a track, chapter or attachment as they are;
/// a whole-file one without the simple tags written anew, or none at all
/// when cleaning (`None` when nothing is left of it).
fn keep_tag(tag: &[u8], meta: &VideoMeta, clean: bool) -> Option<Vec<u8>> {
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
    if clean {
        return None;
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

/// A simple tag holding others (an actor and the part played).
fn simple_with(name: &str, value: &str, inner: &[(&str, &str)]) -> Vec<u8> {
    let mut s = str_el(0x45a3, name);
    s.extend(str_el(0x4487, value));
    for (n, v) in inner.iter().filter(|(_, v)| !v.is_empty()) {
        s.extend(simple(n, v));
    }
    el(0x67c8, &s)
}

/// A Tag at `level`: its simple tags, those empty left out (none: no Tag).
fn tag_of(level: u64, kind: &str, simples: Vec<Vec<u8>>) -> Vec<u8> {
    let simples: Vec<Vec<u8>> = simples.into_iter().filter(|s| !s.is_empty()).collect();
    if simples.is_empty() {
        return Vec::new();
    }
    let mut targets = uint_el(0x68ca, level);
    targets.extend(str_el(0x63ca, kind));
    let mut t = el(0x63c0, &targets);
    for s in simples {
        t.extend(s);
    }
    el(0x7373, &t)
}

fn tag(level: u64, kind: &str, simples: &[(&str, String)]) -> Vec<u8> {
    tag_of(level, kind, simples.iter().filter(|(_, v)| !v.is_empty()).map(|(n, v)| simple(n, v)).collect())
}

/// Simple tags of one name, one per value.
fn each(name: &str, values: &[String]) -> Vec<Vec<u8>> {
    values.iter().filter(|v| !v.is_empty()).map(|v| simple(name, v)).collect()
}

fn one(name: &str, value: &str) -> Vec<u8> {
    if value.is_empty() {
        Vec::new()
    } else {
        simple(name, value)
    }
}

fn actors(people: &[Person]) -> Vec<Vec<u8>> {
    people
        .iter()
        .filter(|p| !p.name.is_empty())
        .map(|p| simple_with("ACTOR", &p.name, &[("CHARACTER", &p.character)]))
        .collect()
}

/// TMDB's 0–10 as Matroska's 0–5.
fn five(rating: &str) -> String {
    rating
        .parse::<f64>()
        .ok()
        .filter(|r| *r > 0.0)
        .map(|r| format!("{:.1}", r / 2.0))
        .unwrap_or_default()
}

/// The tags written: a movie's at level 50 (its collection at 70); an
/// episode's show at 70, its season at 60 and itself at 50.
fn our_tags(meta: &VideoMeta) -> Vec<u8> {
    let genre = meta.genres.join(", ");
    let keywords = meta.keywords.join(", ");
    if !meta.is_episode() {
        let mut out = tag(70, "COLLECTION", &[("TITLE", meta.collection.clone())]);
        let mut s = vec![
            one("TITLE", &meta.title),
            one("ORIGINAL_TITLE", if meta.original_title == meta.title { "" } else { &meta.original_title }),
            one("TAGLINE", &meta.tagline),
            one("DATE_RELEASED", &meta.date),
            one("SYNOPSIS", &meta.description),
            one("GENRE", &genre),
            one("KEYWORDS", &keywords),
            one("LAW_RATING", &meta.certification),
            one("RATING", &five(&meta.rating)),
        ];
        if !meta.collection.is_empty() {
            s.push(one("PART_NUMBER", &meta.collection_part.map(|n| n.to_string()).unwrap_or_default()));
        }
        s.extend(each("COUNTRY", &meta.countries));
        s.extend(each("PRODUCTION_STUDIO", &meta.studios));
        s.extend(each("DIRECTOR", &meta.directors));
        s.extend(each("WRITTEN_BY", &meta.writers));
        s.extend(each("PRODUCER", &meta.producers));
        s.extend(each("COMPOSER", &meta.composers));
        s.extend(actors(&meta.cast));
        s.push(one("TRAILER", &meta.trailer));
        s.push(one("IMDB", &meta.imdb));
        s.push(one("TMDB", &meta.tmdb));
        out.extend(tag_of(50, "MOVIE", s));
        return out;
    }
    let mut show = vec![
        one("TITLE", &meta.show),
        one("ORIGINAL_TITLE", if meta.original_title == meta.show { "" } else { &meta.original_title }),
        one("TAGLINE", &meta.tagline),
        one("GENRE", &genre),
        one("KEYWORDS", &keywords),
        one("LAW_RATING", &meta.certification),
    ];
    let mut studios = Vec::new();
    if !meta.network.is_empty() {
        studios.push(meta.network.clone());
    }
    studios.extend(meta.studios.iter().filter(|s| **s != meta.network).cloned());
    show.extend(each("PRODUCTION_STUDIO", &studios));
    show.extend(actors(&meta.cast));
    show.push(one("TRAILER", &meta.trailer));
    show.push(one("IMDB", &meta.imdb));
    show.push(one("TMDB", &meta.tmdb));
    let mut out = tag_of(70, "COLLECTION", show);
    out.extend(tag(
        60,
        "SEASON",
        &[("PART_NUMBER", meta.season.map(|s| s.to_string()).unwrap_or_default())],
    ));
    let mut ep = vec![
        one("TITLE", &meta.title),
        one("PART_NUMBER", &meta.episode.map(|e| e.to_string()).unwrap_or_default()),
        one("DATE_RELEASED", &meta.date),
        one("SYNOPSIS", &meta.description),
        one("RATING", &five(&meta.rating)),
    ];
    ep.extend(each("DIRECTOR", &meta.directors));
    ep.extend(each("WRITTEN_BY", &meta.writers));
    ep.extend(actors(&meta.guests));
    out.extend(tag_of(50, "EPISODE", ep));
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

/// An iTunes-style free-form item (`----`): its name under
/// `com.apple.iTunes`, and a text.
fn freeform(name: &str, value: &str) -> Vec<u8> {
    let mut mean = vec![0u8; 4];
    mean.extend_from_slice(b"com.apple.iTunes");
    let mut n = vec![0u8; 4];
    n.extend_from_slice(name.as_bytes());
    let mut d = 1u32.to_be_bytes().to_vec();
    d.extend([0, 0, 0, 0]);
    d.extend_from_slice(value.as_bytes());
    mp4_box(b"----", &[mp4_box(b"mean", &mean), mp4_box(b"name", &n), mp4_box(b"data", &d)].concat())
}

/// What tells items apart: the kind, and a free-form item's name too.
fn item_key(kind: &[u8; 4], content: &[u8]) -> Vec<u8> {
    let mut k = kind.to_vec();
    if kind == b"----"
        && let Some((_, _, n)) = kids(content).into_iter().find(|k| &k.0 == b"name")
    {
        k.extend_from_slice(n.get(4..).unwrap_or_default());
    }
    k
}

fn plist_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The people list players read from MP4 movies (`iTunMOVI`).
fn people_plist(meta: &VideoMeta) -> String {
    let list = |key: &str, names: &[&str]| {
        let names: Vec<&&str> = names.iter().filter(|n| !n.is_empty()).collect();
        if names.is_empty() {
            return String::new();
        }
        let mut s = format!("<key>{key}</key><array>");
        for n in names {
            s.push_str(&format!("<dict><key>name</key><string>{}</string></dict>", plist_text(n)));
        }
        s.push_str("</array>");
        s
    };
    fn names(v: &[String]) -> Vec<&str> {
        v.iter().map(String::as_str).collect()
    }
    let cast: Vec<&str> = meta.cast.iter().chain(&meta.guests).map(|p| p.name.as_str()).collect();
    let mut body = [
        list("cast", &cast),
        list("directors", &names(&meta.directors)),
        list("screenwriters", &names(&meta.writers)),
        list("producers", &names(&meta.producers)),
    ]
    .concat();
    let studio = if meta.network.is_empty() { meta.studios.first().cloned().unwrap_or_default() } else { meta.network.clone() };
    if !studio.is_empty() {
        body.push_str(&format!("<key>studio</key><string>{}</string>", plist_text(&studio)));
    }
    if body.is_empty() {
        return String::new();
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>{body}</dict></plist>\n"
    )
}

/// The US age rating in Apple's form (`mpaa|PG-13|300|`); other
/// countries have none there.
fn apple_rating(meta: &VideoMeta) -> String {
    let c = meta.certification.as_str();
    let (system, score) = if meta.is_episode() {
        ("us-tv", ["TV-Y", "TV-Y7", "TV-G", "TV-PG", "TV-14", "TV-MA"].iter().position(|x| *x == c))
    } else {
        ("mpaa", ["G", "PG", "PG-13", "R", "NC-17"].iter().position(|x| *x == c))
    };
    score.map_or_else(String::new, |i| format!("{system}|{c}|{}|", (i + 1) * 100))
}

/// The items written: what tells them apart, and the boxes.
fn our_items(meta: &VideoMeta) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut out: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut text = |k: &[u8; 4], v: &str| {
        if !v.is_empty() {
            out.push((k.to_vec(), item(k, 1, v.as_bytes())));
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
        text(b"tvnn", &meta.network);
        if let (Some(s), Some(e)) = (meta.season, meta.episode) {
            text(b"tven", &format!("S{s:02}E{e:02}"));
        }
    }
    if meta.is_episode() {
        if let Some(s) = meta.season {
            out.push((b"tvsn".to_vec(), item(b"tvsn", 21, &s.to_be_bytes())));
        }
        if let Some(e) = meta.episode {
            out.push((b"tves".to_vec(), item(b"tves", 21, &e.to_be_bytes())));
        }
    }
    // Media kind: 9 movie, 10 TV show.
    let stik = if meta.is_episode() { 10u8 } else { 9 };
    out.push((b"stik".to_vec(), item(b"stik", 21, &[stik])));
    if let Some(c) = &meta.cover {
        let ty = if c.starts_with(b"\x89PNG") { 14 } else { 13 };
        out.push((b"covr".to_vec(), item(b"covr", ty, c)));
    }
    let original = if meta.original_title == meta.title || meta.original_title == meta.show {
        ""
    } else {
        meta.original_title.as_str()
    };
    let part = if meta.collection.is_empty() { None } else { meta.collection_part };
    let free = [
        ("iTunMOVI", people_plist(meta)),
        ("iTunEXTC", apple_rating(meta)),
        ("ORIGINAL_TITLE", original.to_string()),
        ("TAGLINE", meta.tagline.clone()),
        ("LAW_RATING", meta.certification.clone()),
        ("RATING", meta.rating.clone()),
        ("KEYWORDS", meta.keywords.join(", ")),
        ("COUNTRY", meta.countries.join(", ")),
        ("PRODUCTION_STUDIO", meta.studios.join(", ")),
        ("COLLECTION", meta.collection.clone()),
        ("COLLECTION_PART", part.map(|n| n.to_string()).unwrap_or_default()),
        ("TRAILER", meta.trailer.clone()),
        ("IMDB", meta.imdb.clone()),
        ("TMDB", meta.tmdb.clone()),
    ];
    for (name, value) in free {
        if !value.is_empty() {
            let b = freeform(name, &value);
            let mut key = b"----".to_vec();
            key.extend_from_slice(name.as_bytes());
            out.push((key, b));
        }
    }
    out
}

/// The free-form items written: a later write replaces them even when it
/// has none of its own (they would belong to something else).
const OUR_FREE: &[&str] = &[
    "iTunMOVI", "iTunEXTC", "ORIGINAL_TITLE", "TAGLINE", "LAW_RATING", "RATING", "KEYWORDS", "COUNTRY",
    "PRODUCTION_STUDIO", "COLLECTION", "COLLECTION_PART", "TRAILER", "IMDB", "TMDB",
];

/// QuickTime and 3GPP texts kept beside the items (`©cmt`, `titl`…):
/// what cleaning takes out of `udta`.
fn loose_text(kind: &[u8; 4]) -> bool {
    kind[0] == 0xa9
        || [
            b"name", b"Xtra", b"cprt", b"titl", b"dscp", b"perf", b"auth", b"gnre", b"albm", b"yrrc", b"rtng", b"clsf",
            b"kywd", b"loci", b"desc",
        ]
        .contains(&kind)
}

/// A track of an MP4, read for its name.
fn mp4_track(trak: &[u8]) -> crate::tracknames::TrackInfo {
    use crate::tracknames::{TrackInfo, TrackKind};
    let mut t = TrackInfo::default();
    let mut udta_name = String::new();
    let mut hdlr_name = String::new();
    let mut elng = String::new();
    for (k, _, c) in kids(trak) {
        if &k == b"udta" {
            for (k2, _, c2) in kids(c) {
                if &k2 == b"name" {
                    udta_name = String::from_utf8_lossy(c2).trim_matches('\0').trim().to_string();
                }
            }
        }
        if &k != b"mdia" {
            continue;
        }
        for (k2, _, c2) in kids(c) {
            match &k2 {
                b"mdhd" => t.lang = mdhd_lang(c2).unwrap_or_default(),
                b"elng" => elng = String::from_utf8_lossy(c2.get(4..).unwrap_or_default()).trim_matches('\0').to_string(),
                b"hdlr" => {
                    t.kind = match c2.get(8..12) {
                        Some(b"vide") => TrackKind::Video,
                        Some(b"soun") => TrackKind::Audio,
                        Some(b"sbtl" | b"subt" | b"clcp") => TrackKind::Subtitle,
                        _ => TrackKind::Other,
                    };
                    hdlr_name = hdlr_text(c2);
                }
                b"minf" => {
                    let stsd = kids(c2)
                        .into_iter()
                        .find(|k| &k.0 == b"stbl")
                        .and_then(|(_, _, stbl)| kids(stbl).into_iter().find(|k| &k.0 == b"stsd"));
                    if let Some((_, _, stsd)) = stsd
                        && let Some(entry) = stsd.get(8..)
                    {
                        t.codec = match entry.get(4..8) {
                            Some(b"mp4a") => "AAC",
                            Some(b"ac-3") => "AC3",
                            Some(b"ec-3") => "E-AC3",
                            Some(b"dtsc" | b"dtsh" | b"dtsl" | b"dtse") => "DTS",
                            Some(b"mlpa") => "TrueHD",
                            Some(b"Opus") => "Opus",
                            Some(b"fLaC") => "FLAC",
                            Some(b"alac") => "ALAC",
                            Some(b"lpcm" | b"sowt" | b"twos") => "PCM",
                            Some(b"tx3g") => "Text",
                            Some(b"wvtt") => "WebVTT",
                            Some(b"stpp") => "TTML",
                            _ => "",
                        }
                        .to_string();
                        if t.kind == TrackKind::Audio {
                            t.channels = entry.get(24..26).map_or(0, |b| u32::from(u16::from_be_bytes([b[0], b[1]])));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if !elng.is_empty() {
        t.lang = elng;
    }
    t.name = if !udta_name.is_empty() {
        udta_name
    } else if is_default_handler(&hdlr_name) {
        String::new()
    } else {
        hdlr_name
    };
    t
}

/// A handler's name as tools write it by default (`SoundHandler`, `Core
/// Media Audio`): no name of the track's.
fn is_default_handler(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.is_empty() || n.contains("handler") || n.contains("media") || n.starts_with("gpac") || n.contains("produced by")
}

/// The name in a `hdlr` box: after version, flags, a field, the type and
/// 12 reserved bytes; a C string, or QuickTime's counted one.
fn hdlr_text(c: &[u8]) -> String {
    let name = c.get(24..).unwrap_or_default();
    let name = match name.first() {
        Some(&n) if n as usize == name.len() - 1 && n > 0 => &name[1..],
        _ => name,
    };
    String::from_utf8_lossy(name).trim_matches('\0').trim().to_string()
}

fn mdhd_lang(c: &[u8]) -> Option<String> {
    let at = if c.first() == Some(&1) { 32 } else { 20 };
    let v = u16::from_be_bytes(c.get(at..at + 2)?.try_into().ok()?);
    if v == 0 || v == 0x7fff {
        return Some("und".into());
    }
    let s: String = [(v >> 10) & 31, (v >> 5) & 31, v & 31]
        .iter()
        .map(|x| char::from((*x as u8).wrapping_add(0x60)))
        .collect();
    s.chars().all(|c| c.is_ascii_lowercase()).then_some(s)
}

/// A track with its new name (and language, when its old name told it):
/// the handler's name and `udta/name`, as players read them.
fn named_trak(trak: &[u8], info: &crate::tracknames::TrackInfo, n: &crate::tracknames::NewName) -> Vec<u8> {
    use crate::tracknames::TrackKind;
    let mut body = Vec::new();
    let mut had_udta = false;
    for (k, head, c) in kids(trak) {
        match &k {
            b"mdia" => {
                let mut m = Vec::new();
                for (k2, h2, c2) in kids(c) {
                    match &k2 {
                        b"hdlr" if c2.len() >= 24 => {
                            let name = if !n.name.is_empty() {
                                n.name.clone()
                            } else {
                                let old = hdlr_text(c2);
                                match info.kind {
                                    TrackKind::Video => "VideoHandler".into(),
                                    TrackKind::Audio => "SoundHandler".into(),
                                    TrackKind::Subtitle => "SubtitleHandler".into(),
                                    TrackKind::Other => old,
                                }
                            };
                            let counted = matches!(c2.get(24), Some(&l) if l as usize == c2.len() - 25 && l > 0);
                            let mut b = c2[..24].to_vec();
                            if counted && name.len() < 256 {
                                b.push(name.len() as u8);
                                b.extend_from_slice(name.as_bytes());
                            } else {
                                b.extend_from_slice(name.as_bytes());
                                b.push(0);
                            }
                            m.extend(mp4_box(b"hdlr", &b));
                        }
                        b"mdhd" if n.lang.is_some() => {
                            let code = n.lang.unwrap_or_default().as_bytes();
                            let at = if c2.first() == Some(&1) { 32 } else { 20 };
                            let mut b = c2.to_vec();
                            if code.len() == 3 && b.len() >= at + 2 {
                                let v = (u16::from(code[0] - 0x60) << 10) | (u16::from(code[1] - 0x60) << 5) | u16::from(code[2] - 0x60);
                                b[at..at + 2].copy_from_slice(&v.to_be_bytes());
                            }
                            m.extend(mp4_box(b"mdhd", &b));
                        }
                        b"elng" if n.lang.is_some() => {
                            let tag = crate::subtitles::langs(n.lang.unwrap_or_default())
                                .first()
                                .map(|l| l.tag.clone())
                                .unwrap_or_default();
                            let mut b = c2.get(..4).unwrap_or(&[0, 0, 0, 0]).to_vec();
                            b.extend_from_slice(tag.as_bytes());
                            b.push(0);
                            m.extend(mp4_box(b"elng", &b));
                        }
                        _ => {
                            m.extend_from_slice(h2);
                            m.extend_from_slice(c2);
                        }
                    }
                }
                body.extend(mp4_box(b"mdia", &m));
            }
            b"udta" => {
                had_udta = true;
                let mut u = Vec::new();
                for (k2, h2, c2) in kids(c) {
                    if &k2 != b"name" {
                        u.extend_from_slice(h2);
                        u.extend_from_slice(c2);
                    }
                }
                if !n.name.is_empty() {
                    u.extend(mp4_box(b"name", n.name.as_bytes()));
                }
                if !u.is_empty() {
                    body.extend(mp4_box(b"udta", &u));
                }
            }
            _ => {
                body.extend_from_slice(head);
                body.extend_from_slice(c);
            }
        }
    }
    if !had_udta && !n.name.is_empty() {
        body.extend(mp4_box(b"udta", &mp4_box(b"name", n.name.as_bytes())));
    }
    mp4_box(b"trak", &body)
}

/// `moov` with the items of `meta` in `udta/meta/ilst` (others kept,
/// unless cleaning) and, with `opt.tracks`, the tracks named.
fn new_moov(moov: &[u8], meta: &VideoMeta, opt: &Options) -> Option<Vec<u8>> {
    let ours = our_items(meta);
    let parts = kids(moov);
    if parts.iter().any(|k| &k.0 == b"mvex") {
        // A fragmented file: its index is not the whole story.
        return None;
    }
    let traks: Vec<&[u8]> = parts.iter().filter(|k| &k.0 == b"trak").map(|k| k.2).collect();
    let infos: Vec<crate::tracknames::TrackInfo> = traks.iter().map(|t| mp4_track(t)).collect();
    let names = if opt.tracks { crate::tracknames::names(&infos) } else { Vec::new() };
    let mut body = Vec::new();
    let mut had_udta = false;
    let mut nth = 0;
    for (kind, head, content) in parts {
        if &kind == b"trak" && opt.tracks {
            let (info, n) = (&infos[nth], &names[nth]);
            nth += 1;
            if n.name != info.name || n.lang.is_some() {
                body.extend(named_trak(content, info, n));
                continue;
            }
        }
        if &kind != b"udta" {
            body.extend_from_slice(head);
            body.extend_from_slice(content);
            continue;
        }
        had_udta = true;
        body.extend(mp4_box(b"udta", &new_udta(content, &ours, opt.clean)));
    }
    if !had_udta {
        body.extend(mp4_box(b"udta", &new_udta(&[], &ours, opt.clean)));
    }
    Some(mp4_box(b"moov", &body))
}

fn new_udta(udta: &[u8], ours: &[(Vec<u8>, Vec<u8>)], clean: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut had_meta = false;
    for (kind, head, content) in kids(udta) {
        if clean && loose_text(&kind) {
            continue;
        }
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
                m.extend(new_ilst(if clean { &[] } else { c }, ours));
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

fn new_ilst(ilst: &[u8], ours: &[(Vec<u8>, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (kind, head, content) in kids(ilst) {
        // Ours replace theirs; a description or TV detail not written
        // this time goes too (it would belong to something else).
        let key = item_key(&kind, content);
        let replaced = ours.iter().any(|(k, _)| *k == key)
            || [*b"desc", *b"ldes", *b"tvsh", *b"tven", *b"tvsn", *b"tves", *b"stik", *b"tvnn"].contains(&kind)
            || (&kind == b"----" && OUR_FREE.iter().any(|n| key[4..] == *n.as_bytes()));
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
fn mp4_plan(f: &mut File, len: u64, meta: &VideoMeta, opt: &Options) -> Result<Option<Plan>, String> {
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
    let new = new_moov(&old, meta, opt).ok_or_else(bad)?;
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
            let edit = write(&p, &meta(), &Options::default(), &store).unwrap().unwrap();
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
            let plan = mkv_plan(&mut f, bytes.len() as u64, &meta(), &Options::default()).unwrap();
            assert!(plan.is_some());
            // Written twice: still one cover.
            let edit2 = write(&p, &meta(), &Options::default(), &store).unwrap().unwrap();
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
        write(&p, &m, &Options::default(), &d.path().join("s")).unwrap().unwrap();
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
            let edit = write(&p, &meta(), &Options::default(), &store).unwrap().unwrap();
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
        let edit = write(&p, &meta(), &Options::default(), &store).unwrap().unwrap();
        let mut f = File::options().append(true).open(&p).unwrap();
        f.write_all(b"more").unwrap();
        assert_eq!(restore(&p, &edit, &store).unwrap_err(), "videoChanged");
    }

    fn entry(num: u64, kind: u64, lang: &str, name: &str, codec: &str, channels: u64) -> Vec<u8> {
        let mut e = [uint_el(0xd7, num), uint_el(0x73c5, 100 + num), uint_el(0x83, kind), str_el(0x86, codec)].concat();
        if !lang.is_empty() {
            e.extend(str_el(0x22b59c, lang));
        }
        if !name.is_empty() {
            e.extend(str_el(0x536e, name));
        }
        if channels > 0 {
            e.extend(el(0xe1, &uint_el(0x9f, channels)));
        }
        el(0xae, &e)
    }

    fn attached(name: &str, mime: &str) -> Vec<u8> {
        el(0x61a7, &[str_el(0x466e, name), str_el(0x4660, mime), el(0x465c, b"data"), uint_el(0x46ae, 9)].concat())
    }

    /// A Matroska file with named tracks, attachments and site tags, the
    /// tags right after the tracks (as some tools write them).
    fn mkv_dirty() -> Vec<u8> {
        let info = el(INFO, &[uint_el(0x2ad7b1, 1_000_000), str_el(0x7ba9, "Some.Movie.2010 - www.site.com")].concat());
        let tracks = el(
            TRACKS,
            &[
                entry(1, 1, "und", "x264 by GROUP", "V_MPEG4/ISO/AVC", 0),
                entry(2, 2, "por", "Dublagem Clássica [site.com]", "A_AC3", 2),
                entry(3, 2, "por", "Redublagem", "A_AC3", 2),
                entry(4, 17, "por", "Legendas por Fulano", "S_TEXT/UTF8", 0),
                entry(5, 17, "und", "Brazilian Portuguese Forced", "S_TEXT/UTF8", 0),
            ]
            .concat(),
        );
        let track_tag = {
            let mut t = el(0x63c0, &uint_el(0x63c5, 102));
            t.extend(simple("BPS", "1000"));
            el(0x7373, &t)
        };
        let junk = tag(50, "MOVIE", &[("COMMENT", "Downloaded from somewhere".into())]);
        let tags = el(TAGS, &[track_tag, junk].concat());
        let att = el(
            ATTACHMENTS,
            &[attached("font.ttf", "application/x-truetype-font"), attached("poster.jpg", "image/jpeg"), attached("cover.jpg", "image/jpeg")].concat(),
        );
        let cluster = el(CLUSTER, &[uint_el(0xe7, 0), el(0xa3, &[0x81, 0, 0, 0x80, 1, 2, 3])].concat());
        mkv_from(vec![(INFO, info), (TRACKS, tracks), (TAGS, tags), (ATTACHMENTS, att), (CLUSTER, cluster)])
    }

    /// A Matroska file of these elements, after a SeekHead to them with
    /// room after it.
    fn mkv_from(parts: Vec<(u64, Vec<u8>)>) -> Vec<u8> {
        let head_room = 96usize;
        let mut at = head_room as u64;
        let mut entries = Vec::new();
        for (id, b) in &parts {
            if *id != CLUSTER && *id != VOID {
                let mut e = el(0x53ab, &id_bytes(*id));
                e.extend(uint_el(0x53ac, at));
                entries.extend(el(0x4dbb, &e));
            }
            at += b.len() as u64;
        }
        let mut seg = el(SEEKHEAD, &entries);
        let pad = head_room - seg.len();
        seg.extend(void(pad as u64).unwrap());
        for (_, b) in parts {
            seg.extend(b);
        }
        let mut file = el(0x1a45_dfa3, &str_el(0x4282, "matroska"));
        file.extend(el_len(SEGMENT, &seg, Some(8)));
        file
    }

    /// The top elements of a Matroska file (checking each one and that
    /// the SeekHead points at what it says), and its tracks' names.
    fn walk(p: &Path) -> (Vec<u64>, Vec<String>) {
        let mut f = File::open(p).unwrap();
        let len = f.metadata().unwrap().len();
        let (_, size, h) = header_at(&mut f, 0).unwrap();
        let seg_at = h + size.unwrap();
        let (_, seg_size, head) = header_at(&mut f, seg_at).unwrap();
        let data = seg_at + head;
        assert_eq!(data + seg_size.unwrap(), len);
        let mut at = data;
        let mut ids = Vec::new();
        let mut names = Vec::new();
        while at < len {
            let (id, size, head) = header_at(&mut f, at).unwrap();
            let size = size.unwrap();
            assert!(at + head + size <= len);
            let b = read_at(&mut f, at + head, size as usize).unwrap();
            if id == SEEKHEAD {
                for (_, e) in ebml(&b).filter(|(i, _)| *i == 0x4dbb) {
                    let sid = ebml(e).find(|(i, _)| *i == 0x53ab).unwrap().1.to_vec();
                    let pos = ebml_uint(ebml(e).find(|(i, _)| *i == 0x53ac).unwrap().1);
                    let (got, _, _) = header_at(&mut f, data + pos).unwrap();
                    assert_eq!(id_bytes(got), sid, "SeekHead entry");
                }
            }
            if id == TRACKS {
                for (_, t) in ebml(&b).filter(|(i, _)| *i == 0xae) {
                    names.push(
                        ebml(t)
                            .find(|(i, _)| *i == 0x536e)
                            .map(|(_, v)| String::from_utf8_lossy(v).into_owned())
                            .unwrap_or_default(),
                    );
                }
            }
            if id != VOID {
                ids.push(id);
            }
            at += head + size;
        }
        (ids, names)
    }

    fn full_meta() -> VideoMeta {
        VideoMeta {
            original_title: "Original".into(),
            tagline: "A line.".into(),
            certification: "BR-14".into(),
            rating: "7.8".into(),
            keywords: vec!["heist".into()],
            countries: vec!["Brazil".into()],
            studios: vec!["Studio".into()],
            directors: vec!["Director A".into()],
            writers: vec!["Writer B".into()],
            cast: vec![Person { name: "Actor C".into(), character: "Hero".into() }],
            collection: "Some Collection".into(),
            collection_part: Some(2),
            trailer: "https://www.youtube.com/watch?v=x".into(),
            backdrop: Some(b"\xFF\xD8\xFFwide".to_vec()),
            ..meta()
        }
    }

    #[test]
    fn matroska_cleaned_and_tracks_named() {
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("store");
        let p = d.path().join("m.mkv");
        let old = mkv_dirty();
        fs::write(&p, &old).unwrap();
        let opt = Options { clean: true, tracks: true };
        let edit = write(&p, &full_meta(), &opt, &store).unwrap().unwrap();
        let (ids, names) = walk(&p);
        assert_eq!(
            names,
            ["", "Português – Dublagem Clássica", "Português – Redublagem", "Português", "Português (Brasil) – Forçada"]
        );
        assert!(ids.contains(&TAGS) && ids.contains(&ATTACHMENTS) && ids.contains(&CLUSTER));
        let live = {
            // What a reader sees: the elements, not what Voids hide.
            let mut f = File::open(&p).unwrap();
            let mut out = Vec::new();
            let (_, size, h) = header_at(&mut f, 0).unwrap();
            let seg_at = h + size.unwrap();
            let (_, _, head) = header_at(&mut f, seg_at).unwrap();
            let mut at = seg_at + head;
            let len = f.metadata().unwrap().len();
            while at < len {
                let (id, size, head) = header_at(&mut f, at).unwrap();
                if id != VOID {
                    out.extend(read_at(&mut f, at, (head + size.unwrap()) as usize).unwrap());
                }
                at += head + size.unwrap();
            }
            String::from_utf8_lossy(&out).into_owned()
        };
        assert!(live.contains("font.ttf") && live.contains("BPS"));
        assert!(!live.contains("poster.jpg") && !live.contains("Downloaded") && !live.contains("site.com"));
        assert!(live.contains("cover.jpg") && live.contains("cover_land.jpg"));
        assert!(live.contains("Actor C") && live.contains("CHARACTER") && live.contains("Hero"));
        assert!(live.contains("Some Collection") && live.contains("LAW_RATING"));
        // The forced flag the old name told.
        let mut f = File::open(&p).unwrap();
        let bytes = fs::read(&p).unwrap();
        assert!(mkv_plan(&mut f, bytes.len() as u64, &full_meta(), &opt).unwrap().is_some());
        // Again: the same names.
        let edit2 = write(&p, &full_meta(), &opt, &store).unwrap().unwrap();
        assert_eq!(walk(&p).1, names);
        restore(&p, &edit2, &store).unwrap();
        restore(&p, &edit, &store).unwrap();
        assert_eq!(fs::read(&p).unwrap(), old);
    }

    #[test]
    fn matroska_tracks_without_room_left_alone() {
        // Tracks with no names right before the cluster: the names do not
        // fit, so the tracks stay as they were and the rest is written.
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("s");
        let p = d.path().join("m.mkv");
        let info = el(INFO, &uint_el(0x2ad7b1, 1_000_000));
        let tracks = el(TRACKS, &[entry(1, 1, "", "", "V_MPEG4/ISO/AVC", 0), entry(2, 2, "por", "", "A_AAC", 2)].concat());
        let cluster = el(CLUSTER, &[uint_el(0xe7, 0), el(0xa3, &[0x81, 0, 0, 0x80, 1, 2, 3])].concat());
        let old = mkv_from(vec![(INFO, info), (TRACKS, tracks), (CLUSTER, cluster)]);
        fs::write(&p, &old).unwrap();
        let opt = Options { clean: true, tracks: true };
        let edit = write(&p, &meta(), &opt, &store).unwrap().unwrap();
        let (ids, names) = walk(&p);
        assert_eq!(names, ["", ""]);
        assert!(ids.contains(&TAGS));
        restore(&p, &edit, &store).unwrap();
        assert_eq!(fs::read(&p).unwrap(), old);
        // With room after them (a Void), they get their names.
        let info = el(INFO, &uint_el(0x2ad7b1, 1_000_000));
        let tracks = el(TRACKS, &[entry(1, 1, "", "", "V_MPEG4/ISO/AVC", 0), entry(2, 2, "por", "", "A_AAC", 2)].concat());
        let cluster = el(CLUSTER, &[uint_el(0xe7, 0), el(0xa3, &[0x81, 0, 0, 0x80, 1, 2, 3])].concat());
        let old = mkv_from(vec![(INFO, info), (TRACKS, tracks), (VOID, void(40).unwrap()), (CLUSTER, cluster)]);
        fs::write(&p, &old).unwrap();
        let edit = write(&p, &meta(), &opt, &store).unwrap().unwrap();
        assert_eq!(walk(&p).1, ["", "Português"]);
        restore(&p, &edit, &store).unwrap();
        assert_eq!(fs::read(&p).unwrap(), old);
    }

    /// An MP4 track: handler, language, codec and channels.
    fn trak(handler: &[u8; 4], name: &str, lang: &str, codec: &[u8; 4], channels: u16) -> Vec<u8> {
        let mut mdhd = vec![0u8; 20];
        let l = lang.as_bytes();
        let v = (u16::from(l[0] - 0x60) << 10) | (u16::from(l[1] - 0x60) << 5) | u16::from(l[2] - 0x60);
        mdhd.extend(v.to_be_bytes());
        mdhd.extend([0, 0]);
        let mut hdlr = vec![0u8; 8];
        hdlr.extend_from_slice(handler);
        hdlr.extend([0u8; 12]);
        hdlr.extend_from_slice(name.as_bytes());
        hdlr.push(0);
        let mut entry = vec![0u8; 6];
        entry.extend([0, 1]);
        entry.extend([0u8; 8]);
        entry.extend(channels.to_be_bytes());
        entry.extend([0u8; 10]);
        let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
        stsd.extend(mp4_box(codec, &entry));
        let minf = mp4_box(b"minf", &mp4_box(b"stbl", &mp4_box(b"stsd", &stsd)));
        let mdia = mp4_box(b"mdia", &[mp4_box(b"mdhd", &mdhd), mp4_box(b"hdlr", &hdlr), minf].concat());
        mp4_box(b"trak", &[mp4_box(b"tkhd", &[0u8; 84]), mdia].concat())
    }

    #[test]
    fn mp4_cleaned_and_tracks_named() {
        let d = tempfile::tempdir().unwrap();
        let store = d.path().join("store");
        let p = d.path().join("v.mp4");
        let ftyp = mp4_box(b"ftyp", b"isom\0\0\0\0isom");
        let ilst = mp4_box(b"ilst", &[item(b"\xa9too", 1, b"encoder"), item(b"\xa9cmt", 1, b"www.site.com")].concat());
        let mut m = vec![0, 0, 0, 0];
        m.extend(handler());
        m.extend(ilst);
        let udta = mp4_box(b"udta", &[mp4_box(b"meta", &m), item(b"\xa9cmt", 1, b"loose")].concat());
        let moov = mp4_box(
            b"moov",
            &[
                mp4_box(b"mvhd", &[0u8; 100]),
                trak(b"vide", "VideoHandler", "und", b"avc1", 0),
                trak(b"soun", "Dublagem Clássica - site.com", "por", b"ac-3", 2),
                trak(b"soun", "Redublagem", "por", b"ac-3", 2),
                trak(b"sbtl", "English SDH", "eng", b"tx3g", 0),
                udta,
            ]
            .concat(),
        );
        let old = [ftyp, mp4_box(b"mdat", &[7u8; 1000]), moov].concat();
        fs::write(&p, &old).unwrap();
        let opt = Options { clean: true, tracks: true };
        let edit = write(&p, &full_meta(), &opt, &store).unwrap().unwrap();
        let bytes = fs::read(&p).unwrap();
        let mut f = File::open(&p).unwrap();
        let tops = mp4_tops(&mut f, bytes.len() as u64).unwrap();
        let mv = tops.iter().find(|b| &b.kind == b"moov").unwrap();
        let moov = &bytes[(mv.at + mv.head) as usize..(mv.at + mv.len) as usize];
        let names: Vec<String> = kids(moov).into_iter().filter(|k| &k.0 == b"trak").map(|k| mp4_track(k.2).name).collect();
        assert_eq!(names, ["", "Português – Dublagem Clássica", "Português – Redublagem", "English – SDH"]);
        let text = String::from_utf8_lossy(moov);
        assert!(!text.contains("encoder") && !text.contains("site.com") && !text.contains("loose"));
        assert!(text.contains("iTunMOVI") && text.contains("Actor C") && text.contains("Some Collection"));
        assert!(text.contains("VideoHandler"));
        let (got, cover) = read(&p).unwrap();
        assert_eq!(got.title, "Some Movie");
        assert!(cover);
        restore(&p, &edit, &store).unwrap();
        assert_eq!(fs::read(&p).unwrap(), old);
    }

    #[test]
    fn apple_ratings() {
        let mut m = meta();
        m.certification = "PG-13".into();
        assert_eq!(apple_rating(&m), "mpaa|PG-13|300|");
        m.certification = "BR-14".into();
        assert_eq!(apple_rating(&m), "");
        m.show = "Show".into();
        m.certification = "TV-MA".into();
        assert_eq!(apple_rating(&m), "us-tv|TV-MA|600|");
    }

    /// Writes (or with `RENOME_PROBE_UNDO`, puts back) the videos in
    /// `RENOME_PROBE_DIR`, with its `cover.jpg` and `wide.jpg`.
    #[test]
    #[ignore]
    fn probe_dir() {
        let Ok(dir) = std::env::var("RENOME_PROBE_DIR") else { return };
        let dir = Path::new(&dir);
        let store = dir.join("store");
        let undo = std::env::var("RENOME_PROBE_UNDO").is_ok();
        let mut m = full_meta();
        m.cover = fs::read(dir.join("cover.jpg")).ok();
        m.backdrop = fs::read(dir.join("wide.jpg")).ok();
        let opt = Options { clean: true, tracks: true };
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            let ext = p.extension().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            if !["mkv", "mp4"].contains(&ext.as_str()) {
                continue;
            }
            let saved = p.with_extension(format!("{ext}.edit.json"));
            if undo {
                let edit: VideoEdit = serde_json::from_slice(&fs::read(&saved).unwrap()).unwrap();
                restore(&p, &edit, &store).unwrap();
                println!("restored {}", p.display());
            } else {
                let edit = write(&p, &m, &opt, &store).unwrap().unwrap();
                fs::write(&saved, serde_json::to_vec(&edit).unwrap()).unwrap();
                println!("written {} ({} patches, {} -> {})", p.display(), edit.patches.len(), edit.old_len, edit.new_len);
            }
        }
    }

    /// Prints the tracks' names and the new ones, for the videos given in
    /// `RENOME_PROBE_NAMES` (one path per line); reads only.
    #[test]
    #[ignore]
    fn probe_names() {
        let Ok(list) = std::env::var("RENOME_PROBE_NAMES") else { return };
        for line in fs::read_to_string(list).unwrap().lines().filter(|l| !l.is_empty()) {
            let p = Path::new(line);
            println!("== {}", p.file_name().unwrap().to_string_lossy());
            let infos: Vec<crate::tracknames::TrackInfo> = match kind(p) {
                Some(Kind::Matroska) => {
                    let mut f = File::open(p).unwrap();
                    let (_, size, h) = header_at(&mut f, 0).unwrap();
                    let seg_at = h + size.unwrap();
                    let (_, _, head) = header_at(&mut f, seg_at).unwrap();
                    let mut at = seg_at + head;
                    let mut out = Vec::new();
                    while let Some((id, Some(size), head)) = header_at(&mut f, at) {
                        if id == TRACKS {
                            let b = read_at(&mut f, at + head, size as usize).unwrap();
                            out = mkv_tracks(&b).1;
                            break;
                        }
                        if id == CLUSTER {
                            break;
                        }
                        at += head + size;
                    }
                    out
                }
                Some(Kind::Mp4) => {
                    let mut f = File::open(p).unwrap();
                    let len = f.metadata().unwrap().len();
                    let tops = mp4_tops(&mut f, len).unwrap();
                    let mv = tops.iter().find(|b| &b.kind == b"moov").unwrap();
                    let moov = read_at(&mut f, mv.at + mv.head, (mv.len - mv.head) as usize).unwrap();
                    kids(&moov).into_iter().filter(|k| &k.0 == b"trak").map(|k| mp4_track(k.2)).collect()
                }
                None => continue,
            };
            let new = crate::tracknames::names(&infos);
            for (t, n) in infos.iter().zip(new) {
                println!("  {:?} {} {} {}ch {:?} -> {:?}{}", t.kind, t.lang, t.codec, t.channels, t.name, n.name, if n.forced { " [forced]" } else { "" });
            }
        }
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
