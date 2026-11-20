//! What is written once the files are renamed, decided and done the same
//! way for the window and the command line: the release's adverts to the
//! trash, `.nfo` files and images, album covers and playlists, the details
//! inside songs, audiobooks, videos, books and comics, conversions, the
//! subtitle rule's fixes and the subtitles downloaded.
//!
//! [`plan`] says which files get what, from the rules that are on, the
//! list as it was renamed (a file renamed, or already named right) and
//! what the lookups chose; [`run`] does it, in the window's order,
//! reporting each stage (`after-stage`) and recording what it creates or
//! changes in the rename's history entry, so undo takes it back.

use super::books::Part;
use super::comics::Named;
use crate::Host;
use crate::after::{self as aft, AudiobookTagJob, InsideJob, SubJob, TagJob, VideoTagOptions};
use crate::musicbrainz as mbs;
use crate::openlibrary as ol;
use crate::subtitles as subs;
use crate::tmdb;
use renome_core::books::{BookMatch, Candidate, Guess};
use renome_core::musicbrainz::SongMatch;
use renome_core::tv::EpisodeMatch;
use renome_core::{History, Operation, Preview, PreviewItem, Status};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/readers.rs"));

/// The page size (width, height in mm) of a book printed to PDF for a
/// reader model: its screen's; `None` (A5) for a model not in the list.
pub fn pdf_page(model: &str) -> Option<(f64, f64)> {
    let device = MODELS.iter().find(|(m, _)| *m == model)?.1;
    let (_, w, h, ppi) = SCREENS.iter().find(|(d, ..)| *d == device)?;
    let mm = |px: u32| ((f64::from(px) / f64::from(*ppi)) * 254.0).round() / 10.0;
    Some((mm(*w), mm(*h)))
}

/// A book found in a catalogue: what the file was taken for and the one
/// chosen.
#[derive(Clone, Debug, Deserialize)]
pub struct BookFound {
    pub guess: Guess,
    pub choice: Candidate,
}

/// The subtitles found for a video, with the one picked per language (an
/// index in `candidates`; -1: none).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SubsFound {
    pub have: Vec<String>,
    pub candidates: Vec<renome_core::subtitles::Candidate>,
    pub pick: BTreeMap<String, i64>,
}

/// What the lookups chose, by each file's path before renaming.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Chosen {
    /// Each movie's TMDB id.
    pub movies: HashMap<PathBuf, u64>,
    pub episodes: HashMap<PathBuf, EpisodeMatch>,
    /// Each song's details (chosen or typed in).
    pub songs: HashMap<PathBuf, SongMatch>,
    /// Each album chosen: its release and songs.
    pub albums: Vec<(String, Vec<PathBuf>)>,
    /// Each book's details as the rule names it (chosen or typed in).
    pub books: HashMap<PathBuf, BookMatch>,
    /// What was found for each book (an audiobook's under its first file).
    pub book_found: HashMap<PathBuf, BookFound>,
    /// The books whose details were typed in.
    pub book_typed: HashSet<PathBuf>,
    /// The audiobooks' files.
    pub book_parts: HashMap<PathBuf, Part>,
    /// Each comic's details with its catalogue page (typed in: no page).
    pub comics: HashMap<PathBuf, Named>,
    /// The subtitles found for each video, and the languages asked for.
    pub subtitles: HashMap<PathBuf, SubsFound>,
    pub sub_langs: Vec<String>,
}

/// The options of the rules that are on (each a rule as saved, with its
/// `type`); an option left out takes its default.
#[derive(Clone, Copy)]
pub(crate) struct Opts<'a> {
    kind: &'a str,
    rule: Option<&'a Value>,
}

impl<'a> Opts<'a> {
    pub(crate) fn of(rules: &'a [Value], kind: &'a str) -> Self {
        Opts {
            kind,
            rule: rules
                .iter()
                .find(|r| r.get("type").and_then(Value::as_str) == Some(kind)),
        }
    }
    fn on(&self) -> bool {
        self.rule.is_some()
    }
    fn value(&self, key: &str) -> Value {
        let Some(rule) = self.rule else {
            return Value::Null;
        };
        // An option that does nothing as the rule is set is off (see
        // `catalog::inert`: the window hides it).
        if renome_core::catalog::inert(rule).contains(&key) {
            return Value::Bool(false);
        }
        if let Some(v) = rule.get(key).filter(|v| !v.is_null()) {
            return v.clone();
        }
        renome_core::catalog::rule(self.kind)
            .and_then(|s| s.option(key))
            .map(|o| renome_core::catalog::default_value(o, "en"))
            .unwrap_or(Value::Null)
    }
    /// An option that is on (off when the rule is not).
    pub(crate) fn bool(&self, key: &str) -> bool {
        self.value(key).as_bool().unwrap_or(false)
    }
    fn text(&self, key: &str) -> String {
        self.value(key).as_str().unwrap_or_default().to_string()
    }
    fn number(&self, key: &str, default: u64) -> u64 {
        self.value(key).as_u64().unwrap_or(default)
    }
}

fn ext(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}
fn is_video(p: &Path) -> bool {
    renome_core::tmdb::is_video(&ext(p))
}
fn is_audio(p: &Path) -> bool {
    renome_core::music::is_audio(&ext(p))
}
fn is_book(p: &Path) -> bool {
    renome_core::ebook::is_book(&ext(p))
}

/// A file of the list that is (or will be) where it goes: renamed, or
/// named right already.
struct Pair<'a> {
    from: &'a PathBuf,
    to: PathBuf,
    item: &'a PreviewItem,
    renamed: bool,
}

fn pairs<'a>(preview: &'a Preview, moved: &HashMap<PathBuf, PathBuf>) -> Vec<Pair<'a>> {
    preview
        .items
        .iter()
        .filter_map(|i| match moved.get(&i.path) {
            Some(to) => Some(Pair {
                from: &i.path,
                to: to.clone(),
                item: i,
                renamed: true,
            }),
            None => (i.status == Status::Unchanged).then(|| Pair {
                from: &i.path,
                to: i.path.clone(),
                item: i,
                renamed: false,
            }),
        })
        .collect()
}

/// What is written, file by file (each where it is after renaming).
#[derive(Default)]
pub struct Jobs {
    /// The release's adverts, the folders they leave empty, and how many
    /// files of the list they are.
    pub trash: Option<(Vec<PathBuf>, Vec<PathBuf>, usize)>,
    pub movie_sidecars: Vec<tmdb::SidecarItem>,
    pub covers: Vec<mbs::CoverItem>,
    pub playlists: Vec<PathBuf>,
    pub song_tags: Vec<TagJob>,
    pub book_sidecars: Vec<ol::SidecarItem>,
    pub audiobook_tags: Vec<AudiobookTagJob>,
    pub inside: Vec<InsideJob>,
    pub tv_sidecars: Vec<tmdb::TvSidecarItem>,
    pub video_tags: Vec<tmdb::VideoTagItem>,
    pub sub_tools: Vec<SubJob>,
    pub subtitles: Vec<subs::Write>,
}

/// How much there is of each kind of work (for the question before
/// renaming and the Rename button).
#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    /// `.nfo` files and images of movies and episodes.
    pub sidecars: usize,
    /// Covers and `metadata.opf` of books.
    pub book_meta: usize,
    pub covers: usize,
    pub playlists: usize,
    /// Songs and audiobook files whose tags are written.
    pub tags: usize,
    /// Comics and books written, converted or polished.
    pub inside: usize,
    pub video_tags: usize,
    pub sub_tools: usize,
    /// Subtitle files to download.
    pub subtitles: usize,
    /// Files with work besides the renames (named right already): what
    /// the Rename button counts beside the renames.
    pub besides: usize,
    /// What the question before renaming says of this work (see
    /// [`Counts::lines`]).
    pub lines: Vec<Line>,
}

/// A line said of the work: a text's key and its count.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Line {
    pub key: &'static str,
    pub n: usize,
}

/// The lines with a count, in order.
fn lines(all: &[(&'static str, usize)]) -> Vec<Line> {
    all.iter()
        .filter(|(_, n)| *n > 0)
        .map(|&(key, n)| Line { key, n })
        .collect()
}

impl Counts {
    /// What the question before renaming says of the work besides the
    /// renames, in its order (the window's question and the command
    /// line's list are these).
    pub fn lines(&self) -> Vec<Line> {
        lines(&[
            ("confirmMeta", self.sidecars),
            ("confirmBookMeta", self.book_meta),
            ("confirmCovers", self.covers),
            ("confirmPlaylists", self.playlists),
            ("confirmTags", self.tags),
            ("confirmInside", self.inside + self.video_tags),
            ("confirmSubTools", self.sub_tools),
            ("confirmSubs", self.subtitles),
        ])
    }
}

impl Jobs {
    pub fn counts(&self) -> Counts {
        let mut c = Counts {
            sidecars: self.movie_sidecars.len() + self.tv_sidecars.len(),
            book_meta: self.book_sidecars.len(),
            covers: self.covers.len(),
            playlists: self.playlists.len(),
            tags: self.song_tags.len() + self.audiobook_tags.len(),
            inside: self.inside.len(),
            video_tags: self.video_tags.len(),
            sub_tools: self.sub_tools.len(),
            subtitles: self.subtitles.iter().map(|w| w.langs.len()).sum(),
            besides: self.movie_sidecars.len()
                + self.tv_sidecars.len()
                + self.book_sidecars.len()
                + self.covers.len()
                + self.playlists.len()
                + self.song_tags.len()
                + self.audiobook_tags.len()
                + self.inside.len()
                + self.video_tags.len()
                + self.sub_tools.len()
                + self.subtitles.len(),
            lines: Vec::new(),
        };
        c.lines = c.lines();
        c
    }
}

/// What a book's cover, `metadata.opf` and tags are written from: its
/// details and what the catalogue gave alongside (only while a book typed
/// in is still the catalogue's work).
fn book_sidecar(chosen: &Chosen, from: &PathBuf, to: &Path) -> Option<ol::SidecarItem> {
    let mut book = chosen.books.get(from)?.clone();
    // An audiobook's files: the book of its first, with their part.
    let part = chosen.book_parts.get(from);
    let leader = part.map_or(from, |g| &g.leader);
    if let Some(g) = part {
        book.part = g.part;
        book.parts = g.parts;
        if book.narrator.is_empty() {
            book.narrator = g.narrator.clone();
        }
    }
    let found = chosen.book_found.get(leader);
    if chosen.book_typed.contains(from) || chosen.book_typed.contains(leader) {
        let same = found.filter(|_| !book.work.is_empty()).map(|f| &f.choice);
        let language = book.extra.get("language").cloned().unwrap_or_default();
        return Some(ol::SidecarItem {
            path: to.to_path_buf(),
            cover: same.and_then(|c| c.cover),
            cover_url: same.map(|c| c.cover_url.clone()).unwrap_or_default(),
            description: same.map(|c| c.description.clone()).unwrap_or_default(),
            language,
            book,
        });
    }
    let f = found?;
    Some(ol::SidecarItem {
        path: to.to_path_buf(),
        cover: f.choice.cover,
        cover_url: f.choice.cover_url.clone(),
        description: f.choice.description.clone(),
        language: if f.choice.exact {
            f.choice.language.clone()
        } else {
            f.guess.language.clone()
        },
        book,
    })
}

/// Which files get what. `moved`: where each renamed file went (none
/// before renaming: then only the files already named right, the work
/// there is besides the renames).
pub fn plan(
    rules: &[Value],
    preview: &Preview,
    chosen: &Chosen,
    moved: &HashMap<PathBuf, PathBuf>,
) -> Jobs {
    let movie = Opts::of(rules, "movie");
    let series = Opts::of(rules, "series");
    let music = Opts::of(rules, "music");
    let book = Opts::of(rules, "book");
    let comic = Opts::of(rules, "comic");
    let subtitle = Opts::of(rules, "subtitle");
    let pairs = pairs(preview, moved);
    let mut jobs = Jobs::default();

    if (movie.bool("trashAds") || series.bool("trashAds"))
        && !moved.is_empty()
        && (!preview.trash.is_empty() || !preview.trash_folders.is_empty())
    {
        let ads = preview.items.iter().filter(|i| i.to_trash).count();
        jobs.trash = Some((preview.trash.clone(), preview.trash_folders.clone(), ads));
    }
    for p in &pairs {
        let video = !p.item.is_dir && is_video(&p.to);
        if let Some(id) = chosen.movies.get(p.from).filter(|_| video) {
            if movie.bool("nfo") {
                jobs.movie_sidecars.push(tmdb::SidecarItem {
                    path: p.to.clone(),
                    id: *id,
                });
            }
            if movie.bool("videoTags") && renome_core::videotags::writable(&p.to) {
                jobs.video_tags.push(tmdb::VideoTagItem {
                    path: p.to.clone(),
                    movie: Some(*id),
                    show: None,
                    season: 0,
                    episodes: vec![],
                });
            }
        }
        // AniList's shows get neither (`catalog::inert`: off).
        if let Some(m) = chosen.episodes.get(p.from).filter(|_| video) {
            if series.bool("nfo") {
                jobs.tv_sidecars.push(tmdb::TvSidecarItem {
                    path: p.to.clone(),
                    id: m.id,
                    season: m.season,
                    episodes: m.episodes.clone(),
                    titles: m.titles.clone(),
                });
            }
            if series.bool("videoTags") && m.id > 0 && renome_core::videotags::writable(&p.to) {
                jobs.video_tags.push(tmdb::VideoTagItem {
                    path: p.to.clone(),
                    movie: None,
                    show: Some(m.id),
                    season: m.season,
                    episodes: m.episodes.clone(),
                });
            }
        }
        if let Some(song) = chosen.songs.get(p.from)
            && music.bool("writeTags")
            && is_audio(&p.to)
            && (p.renamed || p.item.retag || (music.bool("cleanTags") && p.item.junk))
        {
            jobs.song_tags.push(TagJob {
                path: p.to.clone(),
                song: song.clone(),
            });
        }
        if book.on() && !p.item.is_dir && chosen.books.contains_key(p.from) {
            let leads = chosen
                .book_parts
                .get(p.from)
                .is_none_or(|g| &g.leader == p.from);
            if (book.bool("cover") || book.bool("metadataOpf"))
                && (is_book(&p.to) || (is_audio(&p.to) && leads))
                && let Some(s) = book_sidecar(chosen, p.from, &p.to)
            {
                jobs.book_sidecars.push(s);
            }
            if book.bool("audiobooks")
                && book.bool("audioTags")
                && is_audio(&p.to)
                && let Some(s) = book_sidecar(chosen, p.from, &p.to)
            {
                jobs.audiobook_tags.push(AudiobookTagJob {
                    path: s.path,
                    book: s.book,
                    cover: s.cover,
                    cover_url: s.cover_url,
                    description: s.description,
                    language: s.language,
                });
            }
        }
        if comic.on()
            && let Some(job) = comic_job(&comic, chosen, p)
        {
            jobs.inside.push(job);
        }
        if book.on()
            && !p.item.is_dir
            && let Some(job) = book_job(&book, chosen, p)
        {
            jobs.inside.push(job);
        }
        if subtitle.on() && p.item.status != Status::Conflict {
            let fix = p.item.sub_fix.as_ref().is_some_and(|f| !f.no_fps);
            let extract = p.item.sub_extract > 0;
            if fix || extract {
                jobs.sub_tools.push(SubJob {
                    path: p.to.clone(),
                    fix,
                    extract,
                });
            }
        }
    }

    // Album covers: in the folder most of each album's renamed songs went to.
    if music.bool("cover") {
        let here: HashSet<&PathBuf> = pairs.iter().map(|p| p.from).collect();
        let albums: Vec<(String, Vec<PathBuf>)> = chosen
            .albums
            .iter()
            .filter(|(_, songs)| songs.iter().any(|s| here.contains(s)))
            .cloned()
            .collect();
        jobs.covers = super::music::cover_items(&albums, moved)
            .into_iter()
            .filter(|c| !mbs::has_cover(&c.dir))
            .collect();
    }
    // Playlists: the folders the songs went to, and those whose songs all
    // keep their names (when they would get one).
    if music.bool("playlist") {
        let mut dirs: Vec<PathBuf> = Vec::new();
        let mut add = |d: PathBuf| {
            if !dirs.contains(&d) {
                dirs.push(d);
            }
        };
        for p in pairs.iter().filter(|p| p.renamed && is_audio(&p.to)) {
            if let Some(d) = p.to.parent() {
                add(d.to_path_buf());
            }
        }
        let changed: HashSet<&PathBuf> = preview
            .items
            .iter()
            .filter(|i| i.status != Status::Unchanged)
            .map(|i| &i.dir)
            .collect();
        for i in preview.items.iter().filter(|i| {
            !i.is_dir
                && i.status == Status::Unchanged
                && is_audio(&i.path)
                && !changed.contains(&i.dir)
        }) {
            if let Some(d) = renome_core::playlists::playlist_dir(&i.dir) {
                add(d);
            }
        }
        jobs.playlists = dirs;
    }
    if movie.bool("subtitles") || series.bool("subtitles") {
        jobs.subtitles = subtitle_writes(chosen, &pairs);
    }
    jobs
}

/// The subtitles picked for each video, downloaded beside it.
fn subtitle_writes(chosen: &Chosen, pairs: &[Pair]) -> Vec<subs::Write> {
    let picked: Vec<super::subtitles::Chosen> = pairs
        .iter()
        .filter(|p| !p.item.is_dir && is_video(&p.to))
        .filter_map(|p| {
            let s = chosen.subtitles.get(p.from)?;
            let e = chosen.episodes.get(p.from);
            Some(super::subtitles::Chosen {
                path: p.to.clone(),
                season: e.map(|e| u32::from(e.season)),
                episode: e.and_then(|e| e.episodes.first()).map(|n| u32::from(*n)),
                have: s.have.clone(),
                candidates: s.candidates.clone(),
                pick: s.pick.clone(),
            })
        })
        .collect();
    super::subtitles::writes(picked, &chosen.sub_langs)
}

/// What renaming the list would write besides the renames: for the files
/// already named right (the rest go with their renames), and the
/// subtitles of every file that is not in conflict.
pub fn counts(rules: &[Value], preview: &Preview, chosen: &Chosen) -> Counts {
    let mut c = plan(rules, preview, chosen, &HashMap::new()).counts();
    let subtitles =
        Opts::of(rules, "movie").bool("subtitles") || Opts::of(rules, "series").bool("subtitles");
    c.subtitles = if subtitles {
        let all: Vec<Pair> = preview
            .items
            .iter()
            .filter(|i| i.status != Status::Conflict)
            .map(|i| Pair {
                from: &i.path,
                to: i.path.clone(),
                item: i,
                renamed: i.status != Status::Unchanged,
            })
            .collect();
        subtitle_writes(chosen, &all)
            .iter()
            .map(|w| w.langs.len())
            .sum()
    } else {
        0
    };
    c
}

/// What the comic rule writes into a comic (CBZ): its ComicInfo.xml (or
/// takes it out), a conversion to CBZ, an e-book made of it.
fn comic_job(rule: &Opts, chosen: &Chosen, p: &Pair) -> Option<InsideJob> {
    let named = chosen.comics.get(p.from);
    let write_info = rule.bool("writeInfo");
    let clean_info = rule.bool("cleanInfo");
    let convert = rule.bool("convert") && p.item.convert;
    let clears = clean_info && p.item.junk && !write_info;
    let original = || -> Option<renome_core::convert::Original> {
        serde_json::from_value(Value::from(rule.text("original"))).ok()
    };
    let mut job = InsideJob {
        path: p.to.clone(),
        comic: None,
        web: String::new(),
        manga: false,
        book: None,
        language: String::new(),
        fresh: clean_info,
        convert: None,
        pdf: renome_core::convert::PdfOptions {
            draw: rule.bool("drawPdfPages"),
            size: rule.number("pageSize", 2400) as u32,
            info: write_info,
        },
        book_to: None,
        pdf_page: None,
        polish: None,
        comic_to: None,
        ebook: Default::default(),
    };
    let details = |job: &mut InsideJob| {
        if let Some(n) = named {
            job.comic = Some(n.comic.clone());
            job.web = n.web.clone();
            job.manga = n.manga;
        }
    };
    if let Some(format) = p.item.comic_to {
        if !p.renamed {
            return None;
        }
        details(&mut job);
        job.fresh = false;
        job.comic_to = Some(format);
        job.convert = original();
        let mut ebook = json!({
            "device": rule.value("device"),
            "width": rule.number("width", 0),
            "height": rule.number("height", 0),
            "crop": rule.value("crop").as_bool().unwrap_or(true),
            "numbers": rule.bool("cropPageNumbers"),
            "contrast": rule.value("contrast").as_bool().unwrap_or(true),
            "upscale": rule.bool("upscale"),
            "greys16": rule.bool("sixteenGreys"),
            "soften": rule.bool("reduceRainbow"),
            "webtoon": rule.bool("webtoon"),
            "join": rule.bool("join"),
            "maxMb": rule.number("maxMb", 0),
        });
        // Older rules said only whether double pages are split.
        ebook["spread"] = match rule.rule.and_then(|r| r.get("spread")) {
            Some(s) if !s.is_null() => s.clone(),
            _ if rule.rule.and_then(|r| r.get("split")) == Some(&Value::Bool(false)) => {
                Value::from("keep")
            }
            _ => Value::from("split"),
        };
        if ebook["device"].is_null() {
            ebook["device"] = Value::from("kindle-pw");
        }
        job.ebook = serde_json::from_value(ebook).unwrap_or_default();
        return Some(job);
    }
    let cbz = ext(&p.to) == "cbz";
    let writes =
        write_info && named.is_some() && (p.renamed || p.item.retag || (clean_info && p.item.junk));
    if !(convert || (cbz && (writes || clears))) {
        return None;
    }
    if write_info {
        details(&mut job);
    }
    if convert {
        job.convert = original();
    }
    Some(job)
}

/// What the book rule writes into a book: its details, an EPUB polished,
/// a conversion to another format.
fn book_job(rule: &Opts, chosen: &Chosen, p: &Pair) -> Option<InsideJob> {
    let has = chosen.books.contains_key(p.from);
    let mut job = InsideJob {
        path: p.to.clone(),
        comic: None,
        web: String::new(),
        manga: false,
        book: None,
        language: String::new(),
        fresh: false,
        convert: None,
        pdf: Default::default(),
        book_to: None,
        pdf_page: None,
        polish: None,
        comic_to: None,
        ebook: Default::default(),
    };
    let details = |job: &mut InsideJob| {
        if let Some(s) = book_sidecar(chosen, p.from, &p.to) {
            job.book = Some(s.book);
            job.language = s.language;
        }
    };
    if let Some(target) = p.item.book_to {
        if rule.text("convertTo").is_empty() {
            return None;
        }
        if has {
            details(&mut job);
        }
        job.book_to = Some(target);
        job.convert = serde_json::from_value(Value::from(rule.text("original"))).ok();
        if target == renome_core::bookconv::Target::Pdf {
            job.pdf_page = pdf_page(&rule.text("model"));
        }
        return Some(job);
    }
    let epub = matches!(ext(&p.to).as_str(), "epub" | "kepub");
    let polish =
        (rule.bool("epub3") || rule.bool("shrinkImages")).then(|| renome_core::bookmeta::Polish {
            epub3: rule.bool("epub3"),
            shrink: rule.bool("shrinkImages"),
        });
    let writable =
        renome_core::inside::book_writable(&p.to) || renome_core::bookmeta::writable(&p.to);
    let writes = rule.bool("writeEpub") && has && writable;
    if !writes && !(epub && polish.is_some()) {
        return None;
    }
    if writes {
        details(&mut job);
    }
    if epub {
        job.polish = polish;
    }
    Some(job)
}

/// What [`run`] needs to reach the catalogues.
pub struct Services<'a> {
    pub tmdb_key: String,
    pub tmdb_lang: String,
    pub mb: &'a mbs::Client,
    pub dg: &'a crate::discogs::Client,
    pub ol: &'a ol::Client,
    /// The subtitle services' keys (only for downloading subtitles).
    pub subs: subs::Auth,
}

/// What went wrong in one stage: how much it did, and each file's reason
/// (an empty file: the whole stage failed).
#[derive(Debug, Serialize)]
pub struct Trouble {
    pub stage: &'static str,
    pub done: usize,
    pub errors: Vec<(String, String)>,
}

/// What [`run`] did.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub ads_trashed: usize,
    pub ads_failed: usize,
    /// Files made beside the renamed ones (`.nfo`, images, covers,
    /// playlists, `metadata.opf`).
    pub sidecars: usize,
    /// Albums whose playlist was there already (and follows the renames).
    pub kept_lists: usize,
    pub tags: usize,
    pub audio_tags: usize,
    /// Comics, books and videos written inside.
    pub inside: usize,
    /// PDF pages drawn while converting comics.
    pub drawn: usize,
    pub sub_fixed: usize,
    pub sub_extracted: usize,
    pub subtitles: usize,
    /// Downloads left today on OpenSubtitles.
    pub remaining: Option<i64>,
    pub troubles: Vec<Trouble>,
    /// What went to the trash (the window takes it off its list).
    #[serde(skip)]
    pub trashed: Vec<PathBuf>,
    /// Files whose tags changed (the window reads them again).
    #[serde(skip)]
    pub retagged: Vec<PathBuf>,
    /// Files a conversion moved (the window follows them).
    #[serde(skip)]
    pub moved: Vec<Operation>,
    /// Where each file of the window's list went with them (filled by the
    /// window, see `apply::Follow`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub followed: Vec<(std::path::PathBuf, std::path::PathBuf)>,
    /// What was done, as said once it is (see [`Report::lines`]).
    pub lines: Vec<Line>,
}

impl Report {
    /// What was done besides the renames, as the notice after renaming
    /// says it, in its order (the window's and the command line's).
    pub fn lines(&self) -> Vec<Line> {
        lines(&[
            ("adsTrashedN", self.ads_trashed),
            ("adsFailedN", self.ads_failed),
            ("sidecarsN", self.sidecars),
            ("subsSavedN", self.subtitles),
            ("subToolsN", self.sub_fixed),
            ("subToolsM", self.sub_extracted),
            ("playlistsKeptN", self.kept_lists),
            ("tagsN", self.tags),
            ("audioTagsN", self.audio_tags),
            ("insideN", self.inside),
            ("insideDrawnN", self.drawn),
        ])
    }

    fn trouble(&mut self, stage: &'static str, done: usize, errors: Vec<(String, String)>) {
        if !errors.is_empty() {
            self.troubles.push(Trouble {
                stage,
                done,
                errors,
            });
        }
    }
}

fn stage(host: &dyn Host, name: &str, total: usize) {
    host.report("after-stage", json!({ "stage": name, "total": total }));
}

/// Does `jobs`, recording what is made or changed in the entry
/// `history_id` (when the rename made one; with renames but no entry,
/// only the adverts go: undo could not take the rest back).
pub async fn run(
    host: &dyn Host,
    history: &History,
    history_id: Option<String>,
    renamed: bool,
    rules: &[Value],
    jobs: Jobs,
    services: &Services<'_>,
) -> Report {
    let mut out = Report::default();
    let movie = Opts::of(rules, "movie");
    let series = Opts::of(rules, "series");
    let music = Opts::of(rules, "music");
    let book = Opts::of(rules, "book");
    let subtitle = Opts::of(rules, "subtitle");
    if let Some((files, folders, ads)) = &jobs.trash {
        let (trashed, failed) = renome_core::plan::trash_leftovers(files, folders);
        // Counted in files (a folder sent whole holds several), unless
        // some failed.
        out.ads_trashed = if failed.is_empty() {
            *ads
        } else {
            trashed.len()
        };
        out.ads_failed = failed.len();
        out.trashed = trashed;
    }
    if renamed && history_id.is_none() {
        out.lines = out.lines();
        return out;
    }
    let id = || history_id.clone();
    let sidecars = |out: &mut Report, mut r: tmdb::SidecarReport, own: bool| {
        if !own {
            aft::record_created(history, id(), &mut r);
        }
        out.sidecars += r.created.len();
        out.kept_lists += r.kept;
        let done = out.sidecars;
        out.trouble("sidecars", done, r.errors);
    };
    if !jobs.movie_sidecars.is_empty() {
        stage(host, "sidecars", jobs.movie_sidecars.len());
        let r = aft::movie_sidecars(
            host,
            history,
            &services.tmdb_key,
            &services.tmdb_lang,
            id(),
            jobs.movie_sidecars,
            movie.bool("backdrop"),
            movie.bool("folder"),
        )
        .await;
        sidecars(&mut out, r, true);
    }
    if !jobs.covers.is_empty() {
        stage(host, "covers", jobs.covers.len());
        let r = mbs::covers(host, services.mb, services.dg, jobs.covers).await;
        sidecars(&mut out, r, false);
    }
    if !jobs.playlists.is_empty() {
        stage(host, "playlists", jobs.playlists.len());
        let r = aft::playlists(history, id(), jobs.playlists);
        sidecars(&mut out, r, true);
    }
    if !jobs.song_tags.is_empty() {
        stage(host, "tags", jobs.song_tags.len());
        match aft::song_tags(
            host,
            history,
            services.mb,
            services.dg,
            id(),
            jobs.song_tags,
            music.bool("embedCover"),
            music.bool("cleanTags"),
            Some(music.number("coverSize", 1200) as u32),
        )
        .await
        {
            Ok((r, written)) => {
                out.tags = r.written;
                out.retagged.extend(written);
                out.trouble("tags", r.written, r.errors);
            }
            Err(e) => out.trouble("tags", 0, vec![(String::new(), e)]),
        }
    }
    if !jobs.book_sidecars.is_empty() {
        stage(host, "bookMeta", jobs.book_sidecars.len());
        let r = ol::sidecars(
            host,
            services.ol,
            jobs.book_sidecars,
            book.bool("cover"),
            book.bool("metadataOpf"),
            book.bool("folders"),
        )
        .await;
        sidecars(&mut out, r, false);
    }
    // After the covers: a book without one in the catalogue takes the one
    // just saved in its folder.
    if !jobs.audiobook_tags.is_empty() {
        stage(host, "audioTags", jobs.audiobook_tags.len());
        match aft::audiobook_tags(
            host,
            history,
            services.ol,
            id(),
            jobs.audiobook_tags,
            true,
            book.bool("audioClean"),
        )
        .await
        {
            Ok((r, written)) => {
                out.audio_tags = r.written;
                out.retagged.extend(written);
                out.trouble("audioTags", r.written, r.errors);
            }
            Err(e) => out.trouble("audioTags", 0, vec![(String::new(), e)]),
        }
    }
    if !jobs.inside.is_empty() {
        stage(host, "inside", jobs.inside.len());
        match aft::inside(host, history, id(), jobs.inside).await {
            Ok(o) => {
                out.inside += o.report.written;
                out.drawn = o.report.drawn;
                out.retagged.extend(o.written);
                out.moved = o.moved;
                let done = out.inside;
                out.trouble("inside", done, o.report.errors);
            }
            Err(e) => out.trouble("inside", 0, vec![(String::new(), e)]),
        }
    }
    if !jobs.tv_sidecars.is_empty() {
        stage(host, "sidecars", jobs.tv_sidecars.len());
        let client = tmdb::Client::new(&services.tmdb_key, &services.tmdb_lang);
        let r = tmdb::tv_sidecars(
            host,
            &client,
            jobs.tv_sidecars,
            series.bool("backdrop"),
            series.bool("folders"),
        )
        .await;
        sidecars(&mut out, r, false);
    }
    if !jobs.video_tags.is_empty() {
        stage(host, "inside", jobs.video_tags.len());
        let vr = if movie.on() { movie } else { series };
        let options = VideoTagOptions {
            cover: true,
            clean: vr.bool("videoClean"),
            tracks: vr.bool("videoTracks"),
        };
        match aft::video_tags(
            host,
            history,
            &services.tmdb_key,
            &services.tmdb_lang,
            id(),
            jobs.video_tags,
            options,
        )
        .await
        {
            Ok(r) => {
                out.inside += r.written;
                let done = out.inside;
                out.trouble("inside", done, r.errors);
            }
            Err(e) => out.trouble("inside", 0, vec![(String::new(), e)]),
        }
    }
    if !jobs.sub_tools.is_empty() {
        stage(host, "subTools", jobs.sub_tools.len());
        let tools: renome_core::subtext::Tools = subtitle
            .rule
            .and_then(|r| serde_json::from_value(r.clone()).ok())
            .unwrap_or_default();
        let original = match subtitle.text("original").as_str() {
            "delete" => renome_core::convert::Original::Delete,
            _ => renome_core::convert::Original::Keep,
        };
        let langs = if subtitle.bool("extract") {
            subtitle.text("extractLangs")
        } else {
            String::new()
        };
        match aft::subtitle_tools(host, history, id(), jobs.sub_tools, tools, original, langs).await
        {
            Ok((r, changed)) => {
                out.sub_fixed = r.written;
                out.sub_extracted = r.created;
                out.retagged.extend(changed);
                out.trouble("subTools", r.written, r.errors);
            }
            Err(e) => out.trouble("subTools", 0, vec![(String::new(), e)]),
        }
    }
    if !jobs.subtitles.is_empty() {
        stage(
            host,
            "subtitles",
            jobs.subtitles.iter().map(|w| w.langs.len()).sum(),
        );
        let stop = host.stop_signal();
        let mut written =
            subs::write(host, services.subs.clone(), jobs.subtitles, move || stop()).await;
        let mut r = tmdb::SidecarReport {
            created: std::mem::take(&mut written.created),
            errors: std::mem::take(&mut written.errors),
            kept: 0,
        };
        aft::record_created(history, id(), &mut r);
        out.subtitles = r.created.len();
        out.remaining = written.remaining;
        out.trouble("subtitles", r.created.len(), r.errors);
    }
    host.report("after-stage", json!({ "stage": Value::Null }));
    out.lines = out.lines();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reader_model_gives_its_screen() {
        // Every model's screen is in the list.
        for (model, device) in MODELS {
            assert!(
                SCREENS.iter().any(|(d, ..)| d == device),
                "{model}: {device}"
            );
        }
        let (w, h) = pdf_page(MODELS[0].0).unwrap();
        assert!(w > 50.0 && h > w);
        assert_eq!(pdf_page("no-such-reader"), None);
    }

    #[test]
    fn options_left_out_take_their_defaults() {
        let rules = vec![json!({ "type": "music", "writeTags": true })];
        let m = Opts::of(&rules, "music");
        assert!(m.bool("writeTags"));
        // Clean tags and embed the cover: on by default.
        assert!(m.bool("cleanTags"));
        assert!(m.bool("embedCover"));
        assert!(!m.bool("playlist"));
        assert!(!Opts::of(&rules, "movie").bool("nfo"));
    }
}
