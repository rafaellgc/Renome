//! Renaming rules. Each [`Step`] is a rule that can be turned on or off;
//! the [`Pipeline`] compiles the list and applies the rules in order.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::LazyLock;

use chrono::format::{Item, StrftimeItems};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use regex::{NoExpand, Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::books::{self, BookMatch};
use crate::comics;
use crate::files::{FileEntry, join_name, split_name};
use crate::music::{self, AudioTags};
use crate::text;
use crate::tmdb::{self, MovieMatch};
use crate::tv::{self, EpisodeMatch};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    #[serde(default = "yes")]
    pub enabled: bool,
    pub rule: Rule,
}

fn yes() -> bool {
    true
}

fn page_size() -> u32 {
    2400
}

/// Part of the name a text rule acts on.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    #[default]
    Name,
    Ext,
    Full,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CaseMode {
    Lower,
    Upper,
    Sentence,
    Title,
    TitleSmart,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InsertSource {
    Text,
    Now,
    Modified,
    /// Date the photo was taken (EXIF).
    Taken,
    /// Name of the folder the file is in.
    Folder,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Anchor {
    Start,
    End,
    /// Position N counted from the start (1 = before the first character).
    Index,
    /// Position N counted from the end (0 = after the last character).
    FromEnd,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Bound {
    Start,
    End,
    Position,
    Text,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ExtMode {
    Set,
    Lower,
    Upper,
    Remove,
    /// The one the content calls for (a `.jpg` that is a PNG becomes
    /// `.png`); a fitting one, or content not told, stays.
    Content,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Rule {
    Replace {
        find: String,
        replace: String,
        #[serde(default)]
        case_sensitive: bool,
        #[serde(default)]
        first_only: bool,
        #[serde(default)]
        target: Target,
    },
    Regex {
        pattern: String,
        replace: String,
        #[serde(default)]
        case_sensitive: bool,
        #[serde(default)]
        first_only: bool,
        #[serde(default)]
        target: Target,
    },
    Case {
        mode: CaseMode,
        #[serde(default)]
        target: Target,
    },
    Insert {
        source: InsertSource,
        #[serde(default)]
        text: String,
        /// strftime format for dates (`%Y-%m-%d`).
        #[serde(default)]
        date_format: String,
        /// For `Taken`: files without that date use the modification date.
        #[serde(default)]
        fallback: bool,
        at: Anchor,
        #[serde(default)]
        position: usize,
    },
    Number {
        /// Template with `{n}`, `{name}` and `{folder}` (or the Portuguese
        /// and Spanish names: `{nome}`, `{nombre}`, `{pasta}`, `{carpeta}`).
        template: String,
        start: i64,
        step: i64,
        digits: usize,
        /// Restarts the numbering in each folder.
        #[serde(default)]
        per_folder: bool,
    },
    Remove {
        #[serde(default)]
        all_spaces: bool,
        #[serde(default)]
        extra_spaces: bool,
        #[serde(default)]
        accents: bool,
        #[serde(default)]
        digits: bool,
        #[serde(default)]
        chars: String,
        /// A track number at the start (`01 - `, `01. `, `1-05 `).
        #[serde(default)]
        leading_number: bool,
        /// Cut to this many characters (0: no limit).
        #[serde(default)]
        max_length: usize,
        #[serde(default)]
        target: Target,
    },
    /// Characters turned into others.
    Convert {
        /// Accents broken by a wrong encoding (`SÃ£o` → `São`).
        #[serde(default)]
        mojibake: bool,
        /// Cyrillic and Greek in Latin letters (`Кино` → `Kino`).
        #[serde(default)]
        latin: bool,
        /// A name any server takes, with this between words (`-`, `_`);
        /// empty: off.
        #[serde(default)]
        web: String,
        #[serde(default)]
        target: Target,
    },
    RemoveRange {
        from: Bound,
        #[serde(default)]
        from_position: usize,
        #[serde(default)]
        from_text: String,
        to: Bound,
        #[serde(default)]
        to_position: usize,
        #[serde(default)]
        to_text: String,
        /// When the bounds are text, also removes the delimiters.
        #[serde(default)]
        inclusive: bool,
    },
    CharAt {
        position: usize,
        #[serde(default)]
        from_end: bool,
        /// Only changes it if the current character is this one (empty = any).
        #[serde(default)]
        only_if: String,
        /// Whether `only_if` must match case.
        #[serde(default)]
        case_sensitive: bool,
        /// Replacement; empty deletes the character.
        #[serde(default)]
        replace: String,
    },
    Extension {
        mode: ExtMode,
        #[serde(default)]
        value: String,
    },
    /// Splits the name into parts at `separator` and rebuilds it from
    /// `order`, where numbers refer to parts (1-based) and everything else
    /// is kept as text: `1 - 3 - 2`.
    Rearrange {
        separator: String,
        order: String,
        #[serde(default)]
        target: Target,
    },
    /// Names the file after the movie chosen for it on TMDB (files without
    /// one are left alone). The search itself happens outside the rules.
    Movie {
        /// `{title}`, `{original}`, `{year}`, `{id}` (or `{título}`, `{ano}`…).
        template: String,
        /// What `:` in titles becomes.
        #[serde(default = "colon")]
        colon: String,
        /// Write `.nfo` and poster next to renamed videos.
        #[serde(default)]
        sidecar: bool,
        /// Also the backdrop image and the title logo.
        #[serde(default)]
        fanart: bool,
        /// Moves each file into a folder with the new name.
        #[serde(default)]
        folder: bool,
        /// Lists the released movies missing from the collections the
        /// files are in (used by the app, not here).
        #[serde(default = "yes")]
        missing: bool,
        /// Downloads subtitles in `sub_langs` along with the rename (used
        /// by the app, not here).
        #[serde(default)]
        subtitles: bool,
        /// Their languages, in order: `pt-BR, en`.
        #[serde(default)]
        sub_langs: String,
        /// Title, date and cover written inside MKV and MP4 videos (by
        /// the app).
        #[serde(default)]
        video_tags: bool,
    },
    /// Names each file after the episode chosen for it on TMDB (files
    /// without one are left alone). The search happens outside the rules.
    Series {
        /// `{show}`, `{year}`, `{season}`, `{episode}`, `{title}`…
        template: String,
        #[serde(default = "colon")]
        colon: String,
        /// The show to look for, as typed (used by the app, not here).
        #[serde(default)]
        show: String,
        /// Moves each file into `<show folder>/Season NN`.
        #[serde(default)]
        folders: bool,
        /// Name of the show's folder, with the same keys as `template`.
        #[serde(default = "show_folder")]
        folder_template: String,
        /// Write `.nfo` files and images (used by the app, not here).
        #[serde(default)]
        sidecar: bool,
        /// Also backdrop and logo (used by the app, not here).
        #[serde(default)]
        fanart: bool,
        /// Lists the aired episodes missing from the seasons the files
        /// cover (used by the app, not here).
        #[serde(default = "yes")]
        missing: bool,
        /// Downloads subtitles in `sub_langs` along with the rename (used
        /// by the app, not here).
        #[serde(default)]
        subtitles: bool,
        /// Their languages, in order: `pt-BR, en`.
        #[serde(default)]
        sub_langs: String,
        /// Title, date and cover written inside MKV and MP4 videos (by
        /// the app).
        #[serde(default)]
        video_tags: bool,
    },
    /// Names songs after their tags; files without them are left alone.
    Music {
        /// `{track}`, `{title}`, `{artist}`, `{album}`…
        template: String,
        #[serde(default = "colon")]
        colon: String,
        /// `1-01` on albums with more than one disc.
        #[serde(default = "yes")]
        disc_prefix: bool,
        /// Missing tags come from the file name and its folder.
        #[serde(default)]
        from_name: bool,
        /// Moves each song into folders made from `folder_template`.
        #[serde(default)]
        folders: bool,
        /// Folder levels separated by `/`.
        #[serde(default = "music_folder")]
        folder_template: String,
        /// Save the album's cover from MusicBrainz (used by the app).
        #[serde(default)]
        cover: bool,
        /// Write a playlist for each album (used by the app).
        #[serde(default)]
        playlist: bool,
        /// Identify songs by their sound on AcoustID (used by the app).
        #[serde(default = "yes")]
        listen: bool,
        /// Write the release's details into the songs' tags (used by the app).
        #[serde(default)]
        write_tags: bool,
        /// With `write_tags`, the front cover too, in place of the pictures
        /// the songs have.
        #[serde(default = "yes")]
        embed_cover: bool,
        /// The embedded cover's size in pixels: 1200, 500 or 250, the
        /// sizes the Cover Art Archive has (used by the app).
        #[serde(default = "cover_size")]
        cover_size: u32,
        /// With `write_tags`, the songs' other tags go (site names, ripper
        /// notes), but for lyrics, loudness and the like.
        #[serde(default = "yes")]
        clean_tags: bool,
        /// Lists the tracks missing from the albums with a release (used
        /// by the app).
        #[serde(default = "yes")]
        missing: bool,
    },
    /// Names each book after the one chosen for it on Open Library (files
    /// without one are left alone). The search happens outside the rules.
    Book {
        /// `{author}`, `{title}`, `{year}`, `{series}`, `{volume}`…
        template: String,
        #[serde(default = "colon")]
        colon: String,
        /// Moves each book into folders made from `folder_template`.
        #[serde(default)]
        folders: bool,
        /// Folder levels separated by `/`, with the same keys as `template`.
        #[serde(default = "book_folder")]
        folder_template: String,
        /// Save the cover next to the book (used by the app).
        #[serde(default)]
        cover: bool,
        /// Write a `metadata.opf` file (used by the app).
        #[serde(default)]
        opf: bool,
        /// Lists the volumes missing between a series' books (used by the
        /// app).
        #[serde(default = "yes")]
        missing: bool,
        /// Converts the books (and Word, OpenDocument, text and HTML
        /// documents) to this format: `epub`, `azw3`, `fb2` or `pdf`;
        /// empty: no conversion. They are named with its extension here;
        /// the app converts them after renaming (see [`crate::bookconv`]).
        #[serde(default)]
        convert_to: String,
    },
    /// Names comics after their series and number: the issue chosen on
    /// Comic Vine, else what the file's `ComicInfo.xml` and name say. Files
    /// with neither are left alone.
    Comic {
        /// `{series}`, `{number}`, `{volume}`, `{chapter}`, `{year}`…
        template: String,
        #[serde(default = "colon")]
        colon: String,
        /// The series to look for, as typed (used by the app, not here).
        #[serde(default)]
        series: String,
        /// Where to look it up: `gcd`, `comicvine` or `mangadex` (used by
        /// the app, not here).
        #[serde(default = "comic_source")]
        source: String,
        /// Moves each comic into folders made from `folder_template`.
        #[serde(default)]
        folders: bool,
        /// Folder levels separated by `/`, with the same keys as `template`.
        #[serde(default = "comic_folder")]
        folder_template: String,
        /// The series is the name of the folder each file is in.
        #[serde(default)]
        by_folder: bool,
        /// Lists the issues missing from the series the files are from
        /// (used by the app, not here).
        #[serde(default = "yes")]
        missing: bool,
        /// CBR, CB7 and CBT files (and `.cbz` files that are not zips) and
        /// PDFs are named `.cbz`: the app then converts them (see
        /// [`crate::convert`]).
        #[serde(default)]
        convert: bool,
        /// PDF pages that are not just a picture are drawn; else such PDFs
        /// are not converted.
        #[serde(default = "yes")]
        draw_pages: bool,
        /// The long side, in pixels, of a drawn page with no picture (used
        /// by the app).
        #[serde(default = "page_size")]
        page_size: u32,
        /// The `ComicInfo.xml` CBZ files already have is cleared: written
        /// again with only what the app writes (with `writeInfo`), or
        /// taken out (without it). Done by the app; marks such files.
        #[serde(default)]
        clean_info: bool,
    },
    /// Names photos and videos by when they were taken (see
    /// [`crate::photos`]); files without a date are left alone.
    Photo {
        /// `{year}`, `{month}`, `{day}`, `{hour}`, `{minute}`, `{second}`,
        /// `{camera}`, `{width}`, `{height}`, `{name}`…
        template: String,
        /// The date in the name, for files without one inside.
        #[serde(default = "yes")]
        from_name: bool,
        /// Then the file's change date.
        #[serde(default)]
        modified: bool,
        /// Hours added to the camera's time.
        #[serde(default)]
        shift_hours: i32,
        /// Moves each file into folders made from `folder_template`.
        #[serde(default)]
        folders: bool,
        /// Folder levels separated by `/`.
        #[serde(default = "photo_folder")]
        folder_template: String,
        /// Sets the renamed files' change date to when they were taken
        /// (used by the app).
        #[serde(default)]
        set_modified: bool,
    },
    /// Subtitle tools (see [`crate::subtext`], [`crate::mkvsubs`]): other
    /// text formats named `.srt` (the app then makes them SubRip); the
    /// rest (encoding, cleaning, times, tracks taken out of videos) is
    /// done by the app after the rename. Names are not changed otherwise.
    Subtitle {
        #[serde(default)]
        to_srt: bool,
        #[serde(default = "yes")]
        utf8: bool,
        #[serde(default)]
        ads: bool,
        #[serde(default)]
        hearing: bool,
        /// Milliseconds added to every time.
        #[serde(default)]
        shift_ms: i64,
        /// Frame rates to retime from and to (0: not).
        #[serde(default)]
        fps_from: f64,
        #[serde(default)]
        fps_to: f64,
        /// Text tracks of Matroska videos written beside them.
        #[serde(default)]
        extract: bool,
        /// The languages taken out (`pt-BR, en`; empty: all).
        #[serde(default)]
        extract_langs: String,
    },
}

fn photo_folder() -> String {
    "{year}/{year}-{month}".into()
}

fn cover_size() -> u32 {
    1200
}

fn colon() -> String {
    " - ".into()
}

fn comic_source() -> String {
    "gcd".into()
}

fn comic_folder() -> String {
    "{series} ({series year})".into()
}

fn book_folder() -> String {
    "{author}/{title}".into()
}

fn music_folder() -> String {
    "{album artist}/{album} ({year})".into()
}

/// A song's tags for the music rule: `None` for other files and songs
/// without a title (even from the name, with `from_name`).
pub fn song_tags(file: &FileEntry, from_name: bool) -> Option<AudioTags> {
    if file.is_dir || !music::is_audio(&file.ext) {
        return None;
    }
    let mut t = file.audio_tags().cloned().unwrap_or_default();
    if from_name {
        t.fill_from(music::from_name(&file.stem, &file.dir));
    } else if t.track.is_none() {
        // No track number in the tags: the one the name starts with stays,
        // or renaming would lose the order.
        t.track = music::from_name(&file.stem, &file.dir).track;
    }
    (!t.title.is_empty()).then_some(t)
}

/// A song's tags even without a title: for songs identified by their sound,
/// whose details come from the release.
pub fn any_song_tags(file: &FileEntry) -> Option<AudioTags> {
    (!file.is_dir && music::is_audio(&file.ext))
        .then(|| file.audio_tags().cloned().unwrap_or_default())
}

fn song_of(ctx: &Ctx, from_name: bool) -> Option<AudioTags> {
    match ctx.song {
        Some(t) => Some(t.clone()),
        None => song_tags(ctx.file, from_name),
    }
}

fn show_folder() -> String {
    "{show} ({year}) [tmdbid-{id}]".into()
}

/// Where a rule moves a renamed file.
#[derive(Debug, Clone, PartialEq)]
pub enum Move {
    /// Into a folder named after its new name, in its current folder.
    OwnFolder,
    /// Into `root/folders…` (a show's folder and season folder).
    Into { root: PathBuf, folders: Vec<String> },
}

/// A rule configuration error: index in the list, code
/// (`invalidRegex`, `invalidDateFormat`) and detail.
#[derive(Debug, Clone, Serialize)]
pub struct RuleError {
    pub index: usize,
    pub code: &'static str,
    pub detail: String,
}

/// Per-file context while applying the rules.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub index: usize,
    /// Position among the files of the same folder.
    pub folder_index: usize,
    pub file: &'a FileEntry,
    pub now: DateTime<Local>,
    /// The movie chosen for this file, for the movie rule.
    pub movie: Option<&'a MovieMatch>,
    /// The episode chosen for this file, for the series rule.
    pub episode: Option<&'a EpisodeMatch>,
    /// The song's tags with the album-wide fields agreed (by the preview),
    /// for the music rule; read from the file when missing.
    pub song: Option<&'a AudioTags>,
    /// The book chosen for this file, for the book rule.
    pub book: Option<&'a BookMatch>,
    /// The issue chosen for this file, for the comic rule; else the
    /// comic is named from the file.
    pub comic: Option<&'a crate::comics::ComicMatch>,
    /// The series its folder is named after, when not its own (see
    /// [`crate::comics::arrange`]).
    pub comic_folder: Option<&'a crate::comics::ComicMatch>,
    /// The video whose picture names it: itself, or a subtitle's video.
    pub video: Option<&'a FileEntry>,
    /// Names the movie's folder: as the video, without a subtitle's
    /// language or the picture's size.
    pub as_folder: bool,
    /// What the photo rule names this file by (its photo's date).
    pub photo: Option<&'a crate::photos::Named>,
}

enum Compiled {
    Replace {
        re: Regex,
        replace: String,
        literal: bool,
        first_only: bool,
        target: Target,
    },
    Plain(Rule),
}

impl Compiled {
    /// A rule that looks the file up by its name.
    fn is_lookup(&self) -> bool {
        matches!(
            self,
            Compiled::Plain(
                Rule::Movie { .. } | Rule::Series { .. } | Rule::Book { .. } | Rule::Comic { .. }
            )
        )
    }
}

pub struct Pipeline {
    steps: Vec<Compiled>,
    pub errors: Vec<RuleError>,
}

impl Pipeline {
    /// A comic rule converts comics to CBZ: whether it draws PDF pages.
    pub fn converts_comics(&self) -> Option<bool> {
        self.steps.iter().find_map(|s| match s {
            Compiled::Plain(Rule::Comic {
                convert: true,
                draw_pages,
                ..
            }) => Some(*draw_pages),
            _ => None,
        })
    }

    /// A book rule converts books: to what.
    pub fn converts_books(&self) -> Option<crate::bookconv::Target> {
        self.steps.iter().find_map(|s| match s {
            Compiled::Plain(Rule::Book { convert_to, .. }) => {
                crate::bookconv::Target::from_ext(convert_to)
            }
            _ => None,
        })
    }

    /// The subtitle rule's tools, and whether (and which languages) it
    /// takes text tracks out of videos, when one is on.
    pub fn subtitle_tools(&self) -> Option<(crate::subtext::Tools, Option<String>)> {
        self.steps.iter().find_map(|s| match s {
            Compiled::Plain(Rule::Subtitle {
                to_srt,
                utf8,
                ads,
                hearing,
                shift_ms,
                fps_from,
                fps_to,
                extract,
                extract_langs,
            }) => Some((
                crate::subtext::Tools {
                    to_srt: *to_srt,
                    utf8: *utf8,
                    ads: *ads,
                    hearing: *hearing,
                    shift_ms: *shift_ms,
                    fps_from: *fps_from,
                    fps_to: *fps_to,
                },
                extract.then(|| extract_langs.clone()),
            )),
            _ => None,
        })
    }

    /// A comic rule clears the `ComicInfo.xml` CBZ files have.
    pub fn cleans_comic_info(&self) -> bool {
        self.steps.iter().any(|s| {
            matches!(
                s,
                Compiled::Plain(Rule::Comic {
                    clean_info: true,
                    ..
                })
            )
        })
    }

    /// Compiles the enabled rules. Invalid rules are skipped and reported in
    /// `errors`, so the preview keeps working while the user is typing.
    pub fn new(steps: &[Step]) -> Self {
        let mut compiled = Vec::new();
        let mut errors = Vec::new();
        for (index, step) in steps.iter().enumerate().filter(|(_, s)| s.enabled) {
            match compile(&step.rule) {
                Ok(Some(c)) => compiled.push(c),
                Ok(None) => {}
                Err((code, detail)) => errors.push(RuleError {
                    index,
                    code,
                    detail,
                }),
            }
        }
        Self {
            steps: compiled,
            errors,
        }
    }

    /// Computes a file's new name (stem, extension).
    pub fn run(&self, ctx: &Ctx) -> (String, String) {
        let (stem, ext, _) = self.run_full(ctx);
        (stem, ext)
    }

    /// The photo rule's way of reading dates, and whether it sets the
    /// files' change dates, when one is on.
    pub fn photos(&self) -> Option<(crate::photos::Options, bool)> {
        self.steps.iter().find_map(|s| match s {
            Compiled::Plain(Rule::Photo {
                from_name,
                modified,
                shift_hours,
                set_modified,
                ..
            }) => Some((
                crate::photos::Options {
                    from_name: *from_name,
                    modified: *modified,
                    shift_hours: *shift_hours,
                },
                *set_modified,
            )),
            _ => None,
        })
    }

    /// The music rule's `(from_name, folders)`, when one is on.
    pub fn music(&self) -> Option<(bool, bool)> {
        self.steps.iter().find_map(|s| match s {
            Compiled::Plain(Rule::Music {
                from_name, folders, ..
            }) => Some((*from_name, *folders)),
            _ => None,
        })
    }

    /// Like [`run`](Self::run), plus where a rule moves the file (movie
    /// rule with "folder", series rule with "folders").
    pub fn run_full(&self, ctx: &Ctx) -> (String, String, Option<Move>) {
        self.run_steps(ctx, false)
    }

    /// The name (stem, extension) the rules above the first lookup rule
    /// (movie, series, book, comic) make: what that rule reads and looks
    /// up. `None` without a lookup rule.
    pub fn lookup_name(&self, ctx: &Ctx) -> Option<(String, String)> {
        self.steps.iter().any(Compiled::is_lookup).then(|| {
            let (stem, ext, _) = self.run_steps(ctx, true);
            (stem, ext)
        })
    }

    fn run_steps(&self, ctx: &Ctx, to_lookup: bool) -> (String, String, Option<Move>) {
        let mut moved = None;
        let mut stem = ctx.file.stem.clone();
        let mut ext = ctx.file.ext.clone();
        for step in &self.steps {
            if to_lookup && step.is_lookup() {
                break;
            }
            match step {
                Compiled::Replace {
                    re,
                    replace,
                    literal,
                    first_only,
                    target,
                } => {
                    let limit = if *first_only { 1 } else { 0 };
                    on_target(&mut stem, &mut ext, *target, |s| {
                        if *literal {
                            re.replacen(s, limit, NoExpand(replace)).into_owned()
                        } else {
                            re.replacen(s, limit, replace.as_str()).into_owned()
                        }
                    });
                }
                Compiled::Plain(rule) => {
                    // Folders themselves are renamed in place.
                    if !ctx.file.is_dir
                        && let Rule::Movie { folder: true, .. } = rule
                        && ctx.movie.is_some()
                    {
                        moved = Some(Move::OwnFolder);
                    } else if !ctx.file.is_dir
                        && let Rule::Series {
                            folders: true,
                            folder_template,
                            colon,
                            ..
                        } = rule
                        && let Some(m) = ctx.episode
                    {
                        moved = Some(Move::Into {
                            root: tv::library_root(&ctx.file.dir, m),
                            folders: vec![
                                tv::episode_name(folder_template, m, colon, None),
                                tv::season_folder(m.season),
                            ],
                        });
                    }
                    if !ctx.file.is_dir
                        && let Rule::Book {
                            folders: true,
                            folder_template,
                            colon,
                            ..
                        } = rule
                        && let Some(m) = ctx.book
                    {
                        let levels = books::folders(folder_template, m, colon);
                        if !levels.is_empty() {
                            moved = Some(Move::Into {
                                root: books::library_root(&ctx.file.dir, m, &levels),
                                folders: levels,
                            });
                        }
                    }
                    if !ctx.file.is_dir
                        && let Rule::Comic {
                            folders: true,
                            folder_template,
                            colon,
                            by_folder,
                            ..
                        } = rule
                        && let Some(m) = comic_of(ctx, *by_folder, &stem)
                    {
                        let m = ctx.comic_folder.cloned().unwrap_or(m);
                        let levels = comics::folders(folder_template, &m, colon);
                        // Already in its series' folder: it stays.
                        if !levels.is_empty() && !comics::in_place(&ctx.file.dir, &m) {
                            moved = Some(Move::Into {
                                root: comics::library_root(&ctx.file.dir, &m, &levels),
                                folders: levels,
                            });
                        }
                    }
                    if !ctx.file.is_dir
                        && let Rule::Photo {
                            folders: true,
                            folder_template,
                            ..
                        } = rule
                        && let Some(p) = ctx.photo
                    {
                        let levels = crate::photos::folders(folder_template, &p.shot);
                        if !levels.is_empty() {
                            moved = Some(Move::Into {
                                root: crate::photos::library_root(&ctx.file.base, levels.len()),
                                folders: levels,
                            });
                        }
                    }
                    if let Rule::Music {
                        folders: true,
                        folder_template,
                        colon,
                        from_name,
                        ..
                    } = rule
                        && let Some(t) = song_of(ctx, *from_name)
                    {
                        let levels = music::folders(folder_template, &t, colon);
                        if !levels.is_empty() {
                            moved = Some(Move::Into {
                                root: music::library_root(&ctx.file.dir, &t, &levels),
                                folders: levels,
                            });
                        }
                    }
                    apply_plain(rule, &mut stem, &mut ext, ctx)
                }
            }
            // Folder names have no extension: undo any split made by a
            // rule on the full name.
            if ctx.file.is_dir && !ext.is_empty() {
                stem = join_name(&stem, &ext);
                ext.clear();
            }
        }
        (stem, ext, moved)
    }
}

fn compile(rule: &Rule) -> Result<Option<Compiled>, (&'static str, String)> {
    let (pattern, replace, case_sensitive, first_only, target, literal) = match rule {
        Rule::Replace {
            find,
            replace,
            case_sensitive,
            first_only,
            target,
        } => {
            if find.is_empty() {
                return Ok(None);
            }
            (
                regex::escape(find),
                replace,
                *case_sensitive,
                *first_only,
                *target,
                true,
            )
        }
        Rule::Regex {
            pattern,
            replace,
            case_sensitive,
            first_only,
            target,
        } => {
            if pattern.is_empty() {
                return Ok(None);
            }
            (
                pattern.clone(),
                replace,
                *case_sensitive,
                *first_only,
                *target,
                false,
            )
        }
        Rule::Insert {
            source: InsertSource::Now | InsertSource::Modified | InsertSource::Taken,
            date_format,
            ..
        } => {
            validate_date_format(date_format)?;
            return Ok(Some(Compiled::Plain(rule.clone())));
        }
        _ => return Ok(Some(Compiled::Plain(rule.clone()))),
    };
    let re = RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|e| ("invalidRegex", e.to_string()))?;
    Ok(Some(Compiled::Replace {
        re,
        replace: replace.clone(),
        literal,
        first_only,
        target,
    }))
}

fn validate_date_format(fmt: &str) -> Result<(), (&'static str, String)> {
    if StrftimeItems::new(fmt).any(|i| matches!(i, Item::Error)) {
        Err(("invalidDateFormat", fmt.to_string()))
    } else {
        Ok(())
    }
}

fn on_target(stem: &mut String, ext: &mut String, target: Target, f: impl Fn(&str) -> String) {
    match target {
        Target::Name => *stem = f(stem),
        Target::Ext => *ext = f(ext),
        Target::Full => {
            let full = f(&join_name(stem, ext));
            let (s, e) = split_name(&full);
            (*stem, *ext) = (s.to_string(), e.to_string());
        }
    }
}

fn format_date(ts: NaiveDateTime, fmt: &str) -> String {
    let fmt = if fmt.is_empty() { "%Y-%m-%d" } else { fmt };
    // `/` cannot appear in file names.
    ts.format(fmt).to_string().replace('/', "-")
}

fn apply_plain(rule: &Rule, stem: &mut String, ext: &mut String, ctx: &Ctx) {
    match rule {
        Rule::Case { mode, target } => on_target(stem, ext, *target, |s| match mode {
            CaseMode::Lower => s.to_lowercase(),
            CaseMode::Upper => s.to_uppercase(),
            CaseMode::Sentence => text::sentence_case(s),
            CaseMode::Title => text::title_case(s, false),
            CaseMode::TitleSmart => text::title_case(s, true),
        }),

        Rule::Insert {
            source,
            text: literal,
            date_format,
            fallback,
            at,
            position,
        } => {
            let modified = || {
                let t = Local.timestamp_opt(ctx.file.modified?, 0).single()?;
                Some(t.naive_local())
            };
            let date = match source {
                InsertSource::Now => Some(ctx.now.naive_local()),
                InsertSource::Modified => modified(),
                InsertSource::Taken => ctx
                    .file
                    .taken()
                    .or_else(|| if *fallback { modified() } else { None }),
                InsertSource::Text | InsertSource::Folder => None,
            };
            let piece = match source {
                InsertSource::Text => literal.clone(),
                InsertSource::Folder => folder_name(ctx.file),
                _ => match date {
                    Some(t) => format_date(t, date_format),
                    // No date for this file: leave the name as it is.
                    None => return,
                },
            };
            let g = text::graphemes(stem);
            let idx = match at {
                Anchor::Start => 0,
                Anchor::End => g.len(),
                Anchor::Index => position.saturating_sub(1).min(g.len()),
                Anchor::FromEnd => g.len().saturating_sub(*position),
            };
            *stem = format!("{}{}{}", g[..idx].concat(), piece, g[idx..].concat());
        }

        Rule::Number {
            template,
            start,
            step,
            digits,
            per_folder,
        } => {
            let index = if *per_folder {
                ctx.folder_index
            } else {
                ctx.index
            };
            let n = start.saturating_add(step.saturating_mul(index as i64));
            let num = format!("{n:0digits$}");
            let folder = folder_name(ctx.file);
            *stem = expand(template, |key| match key {
                "n" => Some(num.as_str()),
                "name" | "nome" | "nombre" => Some(stem.as_str()),
                "folder" | "pasta" | "carpeta" => Some(folder.as_str()),
                _ => None,
            });
        }

        Rule::Remove {
            all_spaces,
            extra_spaces,
            accents,
            digits,
            chars,
            leading_number,
            max_length,
            target,
        } => {
            on_target(stem, ext, *target, |s| {
                let mut s = s.to_string();
                if *leading_number
                    && let Some(m) = LEADING_NUMBER.find(&s)
                    && m.end() < s.len()
                {
                    s = s[m.end()..].to_string();
                }
                if *accents {
                    s = text::plain_letters(&s);
                }
                if *digits {
                    s.retain(|c| !c.is_ascii_digit());
                }
                if !chars.is_empty() {
                    s.retain(|c| !chars.contains(c));
                }
                if *all_spaces {
                    s.retain(|c| !c.is_whitespace());
                } else if *extra_spaces {
                    s = text::collapse_spaces(&s);
                }
                if *max_length > 0 {
                    s = text::cut(&s, *max_length);
                }
                s
            });
        }

        Rule::Convert {
            mojibake,
            latin,
            web,
            target,
        } => {
            on_target(stem, ext, *target, |s| {
                let mut s = s.to_string();
                if *mojibake {
                    s = text::fix_mojibake(&s);
                }
                if *latin {
                    s = text::to_latin(&s);
                }
                if !web.is_empty() {
                    s = text::web_safe(&s, web);
                }
                s
            });
        }

        Rule::RemoveRange {
            from,
            from_position,
            from_text,
            to,
            to_position,
            to_text,
            inclusive,
        } => {
            if let Some(s) = remove_range(
                stem,
                *from,
                *from_position,
                from_text,
                *to,
                *to_position,
                to_text,
                *inclusive,
            ) {
                *stem = s;
            }
        }

        Rule::CharAt {
            position,
            from_end,
            only_if,
            case_sensitive,
            replace,
        } => {
            let mut g = text::graphemes(stem);
            if *position == 0 || *position > g.len() {
                return;
            }
            let i = if *from_end {
                g.len() - position
            } else {
                position - 1
            };
            let same = if *case_sensitive {
                g[i] == only_if
            } else {
                g[i].to_lowercase() == only_if.to_lowercase()
            };
            if !only_if.is_empty() && !same {
                return;
            }
            if replace.is_empty() {
                g.remove(i);
            } else {
                g[i] = replace;
            }
            *stem = g.concat();
        }

        Rule::Extension { .. } if ctx.file.is_dir => {}
        Rule::Extension { mode, value } => match mode {
            ExtMode::Set => *ext = value.trim_start_matches('.').to_string(),
            ExtMode::Lower => *ext = ext.to_lowercase(),
            ExtMode::Upper => *ext = ext.to_uppercase(),
            ExtMode::Remove => ext.clear(),
            ExtMode::Content => {
                if let Some((s, e)) = ctx.file.kind().and_then(|k| k.fix_name(stem, ext)) {
                    *stem = s;
                    *ext = e;
                }
            }
        },

        Rule::Rearrange {
            separator,
            order,
            target,
        } => on_target(stem, ext, *target, |s| {
            rearrange(s, separator, order).unwrap_or_else(|| s.to_string())
        }),

        Rule::Movie {
            template, colon, ..
        } => {
            if let Some(m) = ctx.movie {
                let m = &with_release_movie(m, ctx.file);
                let name = tmdb::movie_name(template, m, colon, video_of(ctx, template));
                if !name.is_empty() {
                    let suffix = if ctx.as_folder {
                        String::new()
                    } else {
                        tmdb::subtitle_tags(stem, ext)
                    };
                    *stem = name + &suffix;
                }
            }
        }

        Rule::Series {
            template, colon, ..
        } => {
            if let Some(m) = ctx.episode {
                let m = &with_release_episode(m, ctx.file);
                let name = tv::episode_name(template, m, colon, video_of(ctx, template));
                if !name.is_empty() {
                    *stem = name + &tmdb::subtitle_tags(stem, ext);
                }
            }
        }

        Rule::Music {
            template,
            colon,
            disc_prefix,
            from_name,
            ..
        } => {
            if let Some(t) = song_of(ctx, *from_name) {
                let name = music::expand(template, &t, colon, *disc_prefix);
                if !name.is_empty() {
                    *stem = name;
                }
            }
        }

        Rule::Book {
            template,
            colon,
            convert_to,
            ..
        } => {
            if let Some(m) = ctx.book {
                let name = books::book_name(template, m, colon);
                if !name.is_empty() {
                    *stem = name;
                }
            }
            if let Some(t) = crate::bookconv::Target::from_ext(convert_to)
                && converts_book(ctx.file, t)
            {
                *ext = t.ext().into();
            }
        }

        Rule::Comic {
            template,
            colon,
            by_folder,
            convert,
            draw_pages,
            ..
        } => {
            // A folder only when it becomes a CBZ.
            let folder_ok = !ctx.file.is_dir || (*convert && converts(ctx.file, *draw_pages));
            if folder_ok && let Some(m) = comic_of(ctx, *by_folder, stem) {
                let name = comics::comic_name(template, &m, colon);
                if !name.is_empty() {
                    *stem = name;
                }
            }
            if *convert && converts(ctx.file, *draw_pages) {
                *ext = "cbz".into();
            }
        }

        Rule::Photo { template, .. } => {
            if let Some(p) = ctx.photo {
                let name = crate::photos::expand(template, &p.shot, &ctx.file.stem, false);
                if !name.is_empty() {
                    *stem = name + &p.kept;
                }
            }
        }

        Rule::Subtitle { to_srt: true, .. } => {
            if !ctx.file.is_dir && crate::subtext::converts(ext, ctx.file.subtitle()) {
                *ext = "srt".into();
            }
        }
        Rule::Subtitle { .. } => {}

        Rule::Replace { .. } | Rule::Regex { .. } => unreachable!("compiled separately"),
    }
}

/// A comic the comic rule's "convert" turns into a CBZ: a RAR, 7z or tar
/// inside, named as a comic, or a PDF that opens (with `draw` off, only
/// one with no page to draw), or an EPUB, Kindle book or folder of
/// pictures only.
pub fn converts(file: &crate::files::FileEntry, draw: bool) -> bool {
    if file.is_dir {
        return file.convert_check().is_some();
    }
    let ext = file.ext.to_lowercase();
    let kind = file.kind();
    if ext == "pdf" {
        return kind.is_some_and(|k| k.names.contains(&"pdf"))
            && file.convert_check().is_some_and(|n| draw || n == 0);
    }
    if matches!(ext.as_str(), "epub" | "mobi" | "azw3" | "azw") {
        return file.convert_check().is_some();
    }
    matches!(ext.as_str(), "cbz" | "cbr" | "cb7" | "cbt")
        && kind.and_then(|k| k.comic).is_some_and(|c| c != "cbz")
}

/// A book (or document) the book rule's "convert" turns into `t`: one in
/// another format that can be read and is not locked by DRM.
pub fn converts_book(file: &crate::files::FileEntry, t: crate::bookconv::Target) -> bool {
    if file.is_dir || !(crate::ebook::is_book(&file.ext) || crate::bookconv::is_document(&file.ext))
    {
        return false;
    }
    match file.book_source() {
        Some((s, locked)) => {
            !locked
                && crate::bookconv::takes_file(t, s, &file.path)
                && crate::bookconv::readable(s, t)
        }
        None => false,
    }
}

/// The comic a file is: the issue chosen for it, else what it says about
/// itself, its name being `stem` (what the rules above made of it).
fn comic_of(ctx: &Ctx, by_folder: bool, stem: &str) -> Option<comics::ComicMatch> {
    if !ctx.file.is_dir && !comics::is_comic(&ctx.file.ext) {
        return None;
    }
    ctx.comic
        .cloned()
        .or_else(|| comics::from_name(ctx.file, stem, by_folder))
}

/// The video's picture, read only when the template asks for it.
fn video_of<'a>(ctx: &Ctx<'a>, template: &str) -> Option<&'a crate::video::VideoInfo> {
    if ctx.as_folder || !crate::video::wanted(template) {
        return None;
    }
    ctx.video?.video_info()
}

fn folder_name(file: &FileEntry) -> String {
    file.dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A number of up to three digits at the start, with a disc before it
/// (`1-05`) and what separates it from the rest; not the whole name.
static LEADING_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:\d{1,2}[-.])?\d{1,3}(?:\s*[-.)_]\s*|\s+)(?:[-–]\s*)?").unwrap()
});

/// The movie with what its file's name tells (edition, source, group).
fn with_release_movie(m: &MovieMatch, file: &FileEntry) -> MovieMatch {
    let mut m = m.clone();
    crate::fields::add_release(&mut m.extra, &file.stem);
    m
}

/// The episode with what its file's name tells.
fn with_release_episode(m: &EpisodeMatch, file: &FileEntry) -> EpisodeMatch {
    let mut m = m.clone();
    crate::fields::add_release(&mut m.extra, &file.stem);
    m
}

/// A template key's value. `{a|b|c}` is the first of those keys with a
/// value (`{title|year}`: the title, else the year); empty when all of
/// them are, unknown when none is known. `{key:N}` writes its numbers
/// with N digits (`{episode:3}`: `001`; `{track:1}`: `1`).
pub(crate) fn key_value<'a>(
    value: &impl Fn(&str) -> Option<&'a str>,
    key: &str,
) -> Option<Cow<'a, str>> {
    if let Some((base, width)) = split_width(key) {
        return key_value(value, base).map(|v| Cow::Owned(digits(&v, width)));
    }
    if !key.contains('|') {
        return value(key).map(Cow::Borrowed);
    }
    let mut known = None;
    for k in key.split('|').map(str::trim) {
        if let Some(v) = value(k) {
            if !v.trim().is_empty() {
                return Some(Cow::Borrowed(v));
            }
            known.get_or_insert(Cow::Borrowed(v));
        }
    }
    known
}

/// `key:N` → (`key`, N), for N from 1 to 9.
fn split_width(key: &str) -> Option<(&str, usize)> {
    let (base, width) = key.rsplit_once(':')?;
    let width = width.trim().parse::<usize>().ok()?;
    (1..=9).contains(&width).then_some((base, width))
}

/// Writes each whole number in `s` with `width` digits (more when it has
/// them), leaving decimals (`12.1`) and a disc before a track (`1-05`) as
/// they are: `01-E02` → `001-E002` with 3.
fn digits(s: &str, width: usize) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            let ch = s[i..].chars().next().unwrap_or_default();
            out.push(ch);
            i += ch.len_utf8().max(1);
            continue;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let run = &s[start..i];
        let decimal = start > 0 && b[start - 1] == b'.';
        let disc = b.get(i) == Some(&b'-') && b.get(i + 1).is_some_and(u8::is_ascii_digit);
        if decimal || disc {
            out.push_str(run);
        } else {
            let whole = run.trim_start_matches('0');
            let whole = if whole.is_empty() { "0" } else { whole };
            out.push_str(&format!("{whole:0>width$}"));
        }
    }
    out
}

/// Whether a template uses `key`, alone or among the choices of a
/// `{a|b}`.
pub(crate) fn uses_key(template: &str, key: &str) -> bool {
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            return false;
        };
        let inner = &rest[open + 1..open + close];
        let inner = split_width(inner).map_or(inner, |(base, _)| base);
        if inner.split('|').any(|k| k.trim() == key) {
            return true;
        }
        rest = &rest[open + close + 1..];
    }
    false
}

/// Resolves the optional sections of a template: `<...>` is kept (without
/// the `<>`) when every key in it has a value, else dropped
/// (`{album}< - CD{disc}>`). A section without known keys, or a `<` with
/// no `>` after it, stays as written.
pub(crate) fn optional_sections<'a>(
    template: &str,
    value: impl Fn(&str) -> Option<&'a str>,
) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('<') {
        let tail = &rest[open..];
        let Some(close) = tail.find('>') else {
            break;
        };
        out.push_str(&rest[..open]);
        let inner = &tail[1..close];
        let mut keys = 0;
        let mut filled = true;
        let mut scan = inner;
        while let Some(start) = scan.find('{') {
            let Some(end) = scan[start..].find('}') else {
                break;
            };
            if let Some(v) = key_value(&value, &scan[start + 1..start + end]) {
                keys += 1;
                filled &= !v.trim().is_empty();
            }
            scan = &scan[start + end + 1..];
        }
        if keys == 0 {
            out.push_str(&tail[..=close]);
        } else if filled {
            out.push_str(inner);
        }
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

/// Replaces `{key}` in a single pass, so values that contain braces are
/// not expanded again. Unknown keys are kept as they are. Optional
/// sections (`<...>`) are resolved first.
pub(crate) fn expand<'a>(template: &str, value: impl Fn(&str) -> Option<&'a str>) -> String {
    let template = optional_sections(template, &value);
    let mut out = String::new();
    let mut rest = template.as_str();
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let tail = &rest[open..];
        match tail
            .find('}')
            .and_then(|close| Some((close, key_value(&value, &tail[1..close])?)))
        {
            Some((close, v)) => {
                out.push_str(&v);
                rest = &tail[close + 1..];
            }
            None => {
                out.push('{');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Piece of a rearrange `order`: a part number or literal text.
enum Piece<'a> {
    Part(usize),
    Text(&'a str),
}

fn parse_order(order: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut rest = order;
    while !rest.is_empty() {
        let digits = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if digits > 0 {
            pieces.push(Piece::Part(rest[..digits].parse().unwrap_or(usize::MAX)));
            rest = &rest[digits..];
        } else {
            let text = rest
                .find(|c: char| c.is_ascii_digit())
                .unwrap_or(rest.len());
            pieces.push(Piece::Text(&rest[..text]));
            rest = &rest[text..];
        }
    }
    pieces
}

/// Splits at `separator` (parts are trimmed, empty ones dropped) and
/// rebuilds from `order`. `None` (name unchanged) when the order refers to
/// a part the name does not have, so names that do not fit are left alone.
pub fn rearrange(s: &str, separator: &str, order: &str) -> Option<String> {
    if separator.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s
        .split(separator)
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let pieces = parse_order(order);
    if !pieces.iter().any(|p| matches!(p, Piece::Part(_))) {
        return None;
    }
    let mut out = String::new();
    for piece in pieces {
        match piece {
            Piece::Text(t) => out.push_str(t),
            Piece::Part(n) => out.push_str(parts.get(n.checked_sub(1)?)?),
        }
    }
    Some(out)
}

#[allow(clippy::too_many_arguments)]
fn remove_range(
    s: &str,
    from: Bound,
    from_position: usize,
    from_text: &str,
    to: Bound,
    to_position: usize,
    to_text: &str,
    inclusive: bool,
) -> Option<String> {
    let g = text::graphemes(s);
    let start = match from {
        Bound::Start => 0,
        Bound::End => return None,
        Bound::Position => from_position.max(1) - 1,
        Bound::Text => {
            if from_text.is_empty() {
                return None;
            }
            let b = s.find(from_text)?;
            if inclusive {
                text::grapheme_index(s, b)
            } else {
                text::grapheme_index(s, b + from_text.len())
            }
        }
    };
    let end = match to {
        Bound::Start => return None,
        Bound::End => g.len(),
        Bound::Position => to_position.min(g.len()),
        Bound::Text => {
            if to_text.is_empty() {
                return None;
            }
            // Look for the end after the start (and after the opening delimiter).
            let from_byte: usize = g[..start.min(g.len())].iter().map(|x| x.len()).sum();
            let skip = if from == Bound::Text && inclusive {
                from_text.len()
            } else {
                0
            };
            let base = (from_byte + skip).min(s.len());
            let b = base + s[base..].find(to_text)?;
            if inclusive {
                text::grapheme_index(s, b + to_text.len())
            } else {
                text::grapheme_index(s, b)
            }
        }
    };
    if start >= end || start >= g.len() {
        return None;
    }
    Some(format!("{}{}", g[..start].concat(), g[end..].concat()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn file(name: &str) -> FileEntry {
        let (stem, ext) = split_name(name);
        FileEntry {
            path: PathBuf::from("/tmp").join(name),
            dir: PathBuf::from("/tmp"),
            stem: stem.into(),
            ext: ext.into(),
            size: 0,
            modified: Some(1_700_000_000),
            is_dir: false,
            base: PathBuf::from("/tmp"),
            taken: Default::default(),
            tags: Default::default(),
            comic: Default::default(),
            video: Default::default(),
            kind: Default::default(),
            convert_pages: Default::default(),
            book: Default::default(),
            photo: Default::default(),
            subtitle: Default::default(),
            sub_tracks: Default::default(),
            isbn_title: Default::default(),
        }
    }

    fn run(rules: Vec<Rule>, name: &str, index: usize) -> String {
        let steps: Vec<Step> = rules
            .into_iter()
            .map(|rule| Step {
                enabled: true,
                rule,
            })
            .collect();
        let p = Pipeline::new(&steps);
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        run_on(&p, &file(name), index)
    }

    fn run_on(p: &Pipeline, f: &FileEntry, index: usize) -> String {
        let (s, e) = p.run(&Ctx {
            index,
            folder_index: index,
            file: f,
            now: Local::now(),
            movie: None,
            episode: None,
            song: None,
            book: None,
            comic: None,
            comic_folder: None,
            video: None,
            as_folder: false,
            photo: None,
        });
        join_name(&s, &e)
    }

    fn replace(find: &str, rep: &str, cs: bool, first: bool) -> Rule {
        Rule::Replace {
            find: find.into(),
            replace: rep.into(),
            case_sensitive: cs,
            first_only: first,
            target: Target::Name,
        }
    }

    #[test]
    fn optional_sections() {
        let value = |key: &str| match key {
            "a" => Some("A"),
            "empty" => Some(""),
            _ => None,
        };
        assert_eq!(expand("{a}< - {a}>", value), "A - A");
        assert_eq!(expand("{a}< - {empty}>", value), "A");
        // Every key must have a value.
        assert_eq!(expand("<{a} {empty}>x", value), "x");
        // Without known keys, or unclosed, it stays as written.
        assert_eq!(expand("<b> <{zzz}> {a}", value), "<b> <{zzz}> A");
        assert_eq!(expand("{a} <{a}", value), "A <A");
        // Values are not read as sections.
        assert_eq!(expand("{a}<{a}>", |_| Some("<x>")), "<x><x>");
    }

    #[test]
    fn fallback_keys() {
        let value = |key: &str| match key {
            "a" => Some("A"),
            "empty" => Some(""),
            _ => None,
        };
        assert_eq!(expand("{empty|a}", value), "A");
        assert_eq!(expand("{a|empty}", value), "A");
        assert_eq!(expand("{ empty | a }", value), "A");
        assert_eq!(expand("x{empty|empty}y", value), "xy");
        // Unknown choices are skipped; none known stays as typed.
        assert_eq!(expand("{zzz|a}", value), "A");
        assert_eq!(expand("{zzz|yyy}", value), "{zzz|yyy}");
        assert_eq!(expand("<({empty|a})>", value), "(A)");
        assert_eq!(expand("<({empty|empty})>z", value), "z");
        assert!(uses_key("{title|year} - x", "year"));
        assert!(uses_key("{episode:3}", "episode"));
        let n = |key: &str| match key {
            "ep" => Some("01-E02"),
            "track" => Some("1-05"),
            "num" => Some("12.1"),
            "word" => Some("III"),
            "empty" => Some(""),
            _ => None,
        };
        assert_eq!(expand("{ep:3}", n), "001-E002");
        assert_eq!(expand("{ep:1}", n), "1-E2");
        assert_eq!(expand("{track:3}", n), "1-005");
        assert_eq!(expand("{num:3}", n), "012.1");
        assert_eq!(expand("{word:3}", n), "III");
        assert_eq!(expand("<#{empty:3}>x", n), "x");
        assert_eq!(expand("{empty|num:2}", n), "12.1");
        assert_eq!(expand("{x:3}", n), "{x:3}");
        assert!(!uses_key("{album artist}", "artist"));
    }

    #[test]
    fn replace_variants() {
        assert_eq!(
            run(vec![replace("a", "o", false, false)], "AaA.txt", 0),
            "ooo.txt"
        );
        assert_eq!(
            run(vec![replace("a", "o", true, false)], "AaA.txt", 0),
            "AoA.txt"
        );
        assert_eq!(
            run(vec![replace("a", "o", false, true)], "AaA.txt", 0),
            "oaA.txt"
        );
        // Literal replacement: `$1` is not expanded.
        assert_eq!(
            run(vec![replace(".", "$1", false, false)], "a.b.txt", 0),
            "a$1b.txt"
        );
    }

    #[test]
    fn rearrange_parts() {
        let r = |sep: &str, order: &str| Rule::Rearrange {
            separator: sep.into(),
            order: order.into(),
            target: Target::Name,
        };
        // Extra spaces around the separator are absorbed.
        assert_eq!(
            run(
                vec![r(" - ", "1 - 3 - 2")],
                "Heróis da Humanidade - Joana d’Arc  - S02E06.mkv",
                0
            ),
            "Heróis da Humanidade - S02E06 - Joana d’Arc.mkv"
        );
        assert_eq!(run(vec![r("-", "3 (1)")], "a -  b-c.txt", 0), "c (a).txt");
        assert_eq!(
            run(vec![r(", ", "2 1")], "Silva, Ana.txt", 0),
            "Ana Silva.txt"
        );
        // Missing part or no separator: unchanged.
        assert_eq!(run(vec![r(" - ", "1 - 3")], "a - b.txt", 0), "a - b.txt");
        assert_eq!(run(vec![r(" - ", "2 1")], "single.txt", 0), "single.txt");
        // Order without numbers or part 0: unchanged.
        assert_eq!(run(vec![r(" - ", "x")], "a - b.txt", 0), "a - b.txt");
        assert_eq!(run(vec![r(" - ", "0 1")], "a - b.txt", 0), "a - b.txt");
        // Two-digit part numbers.
        let many = (1..=12)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("_");
        assert_eq!(
            run(vec![r("_", "12_1")], &format!("{many}.txt"), 0),
            "12_1.txt"
        );
    }

    #[test]
    fn regex_groups() {
        let r = Rule::Regex {
            pattern: r"(\d+)-(\w+)".into(),
            replace: "$2 $1".into(),
            case_sensitive: false,
            first_only: false,
            target: Target::Name,
        };
        assert_eq!(run(vec![r], "12-abc.mp4", 0), "abc 12.mp4");
    }

    #[test]
    fn invalid_regex_reported() {
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Regex {
                pattern: "(".into(),
                replace: "".into(),
                case_sensitive: false,
                first_only: false,
                target: Target::Name,
            },
        }];
        assert_eq!(Pipeline::new(&steps).errors.len(), 1);
    }

    #[test]
    fn numbering() {
        let r = || Rule::Number {
            template: "{nome} - {n}".into(),
            start: 1,
            step: 2,
            digits: 3,
            per_folder: false,
        };
        assert_eq!(run(vec![r()], "foto.jpg", 0), "foto - 001.jpg");
        assert_eq!(run(vec![r()], "foto.jpg", 4), "foto - 009.jpg");
    }

    #[test]
    fn insert_positions() {
        let ins = |at, position| Rule::Insert {
            source: InsertSource::Text,
            text: "X".into(),
            date_format: String::new(),
            fallback: false,
            at,
            position,
        };
        assert_eq!(run(vec![ins(Anchor::Start, 0)], "ação.txt", 0), "Xação.txt");
        assert_eq!(run(vec![ins(Anchor::End, 0)], "ação.txt", 0), "açãoX.txt");
        assert_eq!(run(vec![ins(Anchor::Index, 3)], "ação.txt", 0), "açXão.txt");
        assert_eq!(
            run(vec![ins(Anchor::FromEnd, 1)], "ação.txt", 0),
            "açãXo.txt"
        );
        assert_eq!(run(vec![ins(Anchor::Index, 99)], "ab", 0), "abX");
    }

    #[test]
    fn insert_modified_date() {
        let r = Rule::Insert {
            source: InsertSource::Modified,
            text: String::new(),
            date_format: "%Y_".into(),
            fallback: false,
            at: Anchor::Start,
            position: 0,
        };
        assert_eq!(run(vec![r], "a.txt", 0), "2023_a.txt");
    }

    /// Minimal JPEG whose EXIF holds only `DateTime`.
    fn jpeg_with_date(date: &str) -> Vec<u8> {
        let mut tiff = b"II*\0\x08\0\0\0\x01\0\x32\x01\x02\0\x14\0\0\0\x1a\0\0\0\0\0\0\0".to_vec();
        tiff.extend_from_slice(date.as_bytes());
        tiff.push(0);
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend(tiff);
        let mut jpg = vec![0xFF, 0xD8, 0xFF, 0xE1];
        jpg.extend(((app1.len() + 2) as u16).to_be_bytes());
        jpg.extend(app1);
        jpg.extend([0xFF, 0xD9]);
        jpg
    }

    #[test]
    fn extension_from_content() {
        let d = tempfile::tempdir().unwrap();
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
        let entry =
            |p: &std::path::Path| FileEntry::from_path(p, &std::fs::metadata(p).unwrap()).unwrap();
        let rule = Pipeline::new(&[Step {
            enabled: true,
            rule: Rule::Extension {
                mode: ExtMode::Content,
                value: String::new(),
            },
        }]);
        for (name, want) in [
            ("photo.JPG", "photo.PNG"),
            ("photo.png", "photo.png"),
            ("photo", "photo.png"),
            ("tool_1.0_x86[0]", "tool_1.0_x86[0].png"),
            ("photo.png.jpg", "photo.png"),
            ("photo.gif.jpg", "photo.gif.png"),
            ("v1.2.jpg", "v1.2.png"),
            ("notes.txt", "notes.txt"),
        ] {
            let p = d.path().join(name);
            let bytes: &[u8] = if name == "notes.txt" {
                b"plain text"
            } else {
                png
            };
            std::fs::write(&p, bytes).unwrap();
            assert_eq!(run_on(&rule, &entry(&p), 0), want);
        }
    }

    #[test]
    fn insert_date_taken() {
        let d = tempfile::tempdir().unwrap();
        let photo = d.path().join("IMG_1.jpg");
        let plain = d.path().join("note.txt");
        std::fs::write(&photo, jpeg_with_date("2021:03:04 05:06:07")).unwrap();
        std::fs::write(&plain, "x").unwrap();
        let entry =
            |p: &std::path::Path| FileEntry::from_path(p, &std::fs::metadata(p).unwrap()).unwrap();
        let taken = |fallback| {
            Pipeline::new(&[Step {
                enabled: true,
                rule: Rule::Insert {
                    source: InsertSource::Taken,
                    text: String::new(),
                    date_format: "%Y-%m-%d %H.%M.%S ".into(),
                    fallback,
                    at: Anchor::Start,
                    position: 0,
                },
            }])
        };
        assert_eq!(
            run_on(&taken(false), &entry(&photo), 0),
            "2021-03-04 05.06.07 IMG_1.jpg"
        );
        // No EXIF: unchanged, or the modification date as a fallback.
        assert_eq!(run_on(&taken(false), &entry(&plain), 0), "note.txt");
        let year = Local::now().format("%Y-").to_string();
        assert!(run_on(&taken(true), &entry(&plain), 0).starts_with(&year));
    }

    #[test]
    fn folder_name_and_template() {
        let r = Rule::Number {
            template: "{pasta} {n} {x} {name}".into(),
            start: 1,
            step: 1,
            digits: 2,
            per_folder: false,
        };
        // `{n}` inside the current name is not expanded again; unknown keys stay.
        assert_eq!(run(vec![r], "a{n}.txt", 0), "tmp 01 {x} a{n}.txt");
        let ins = Rule::Insert {
            source: InsertSource::Folder,
            text: String::new(),
            date_format: String::new(),
            fallback: false,
            at: Anchor::Start,
            position: 0,
        };
        assert_eq!(run(vec![ins], "a.txt", 0), "tmpa.txt");
    }

    #[test]
    fn folders_keep_dots() {
        let mut f = file("Show.2020");
        (f.stem, f.ext, f.is_dir) = ("Show.2020".into(), String::new(), true);
        let p = Pipeline::new(&[
            Step {
                enabled: true,
                rule: Rule::Extension {
                    mode: ExtMode::Set,
                    value: "jpg".into(),
                },
            },
            Step {
                enabled: true,
                rule: Rule::Replace {
                    find: "w.2".into(),
                    replace: "w (2".into(),
                    case_sensitive: false,
                    first_only: false,
                    target: Target::Full,
                },
            },
            Step {
                enabled: true,
                rule: Rule::Insert {
                    source: InsertSource::Text,
                    text: ")".into(),
                    date_format: String::new(),
                    fallback: false,
                    at: Anchor::End,
                    position: 0,
                },
            },
        ]);
        assert_eq!(run_on(&p, &f, 0), "Show (2020)");
    }

    #[test]
    fn bad_date_format_reported() {
        let steps = vec![Step {
            enabled: true,
            rule: Rule::Insert {
                source: InsertSource::Now,
                text: String::new(),
                date_format: "%Q".into(),
                fallback: false,
                at: Anchor::Start,
                position: 0,
            },
        }];
        assert_eq!(Pipeline::new(&steps).errors.len(), 1);
    }

    #[test]
    fn remove_stuff() {
        let r = Rule::Remove {
            all_spaces: false,
            extra_spaces: true,
            accents: true,
            digits: true,
            chars: "_".into(),
            leading_number: false,
            max_length: 0,
            target: Target::Name,
        };
        assert_eq!(
            run(vec![r], "  Canção_01   do  Mês .txt", 0),
            "Cancao do Mes.txt"
        );
        let r = |leading_number, max_length| Rule::Remove {
            all_spaces: false,
            extra_spaces: true,
            accents: false,
            digits: false,
            chars: String::new(),
            leading_number,
            max_length,
            target: Target::Name,
        };
        for (from, to) in [
            ("01 - Song.mp3", "Song.mp3"),
            ("01. Song.mp3", "Song.mp3"),
            ("1-05 Song.mp3", "Song.mp3"),
            ("05_Song.mp3", "Song.mp3"),
            ("3 - - Song.mp3", "Song.mp3"),
            // Not a date, a year or the whole name.
            ("2024-07-15 Trip.jpg", "2024-07-15 Trip.jpg"),
            ("1999 Song.mp3", "1999 Song.mp3"),
            ("01.mp3", "01.mp3"),
            ("Song 01.mp3", "Song 01.mp3"),
        ] {
            assert_eq!(run(vec![r(true, 0)], from, 0), to, "{from}");
        }
        // Cut at a length, without spaces or separators left at the end.
        assert_eq!(
            run(vec![r(false, 10)], "Música de Teste - Parte.flac", 0),
            "Música de.flac"
        );
        assert_eq!(
            run(vec![r(false, 12)], "Abc - Defghij.txt", 0),
            "Abc - Defghi.txt"
        );
        assert_eq!(run(vec![r(false, 6)], "Abc - Def.txt", 0), "Abc.txt");
    }

    #[test]
    fn convert_characters() {
        let r = |mojibake, latin, web: &str| Rule::Convert {
            mojibake,
            latin,
            web: web.into(),
            target: Target::Name,
        };
        assert_eq!(
            run(
                vec![r(true, false, "")],
                "02 - CoraÃ§Ã£o de Estudante.mp3",
                0
            ),
            "02 - Coração de Estudante.mp3"
        );
        assert_eq!(
            run(vec![r(false, true, "")], "Кино - Звезда.mp3", 0),
            "Kino - Zvezda.mp3"
        );
        assert_eq!(
            run(
                vec![r(true, false, "-")],
                "Férias em SÃ£o Paulo (2024).JPG",
                0
            ),
            "Ferias-em-Sao-Paulo-2024.JPG"
        );
    }

    #[test]
    fn remove_range_variants() {
        let rr = |from, fp, ft: &str, to, tp, tt: &str, inclusive| Rule::RemoveRange {
            from,
            from_position: fp,
            from_text: ft.into(),
            to,
            to_position: tp,
            to_text: tt.into(),
            inclusive,
        };
        assert_eq!(
            run(
                vec![rr(Bound::Position, 2, "", Bound::Position, 4, "", false)],
                "abcdef.x",
                0
            ),
            "aef.x"
        );
        assert_eq!(
            run(
                vec![rr(Bound::Text, 0, "[", Bound::Text, 0, "]", true)],
                "show [720p] s01.mkv",
                0
            ),
            "show  s01.mkv"
        );
        assert_eq!(
            run(
                vec![rr(Bound::Text, 0, "[", Bound::Text, 0, "]", false)],
                "show [720p].mkv",
                0
            ),
            "show [].mkv"
        );
        assert_eq!(
            run(
                vec![rr(Bound::Text, 0, " - ", Bound::End, 0, "", true)],
                "Autor - Titulo.txt",
                0
            ),
            "Autor.txt"
        );
        // Missing delimiter: nothing changes.
        assert_eq!(
            run(
                vec![rr(Bound::Text, 0, "{", Bound::End, 0, "", true)],
                "abc.x",
                0
            ),
            "abc.x"
        );
    }

    #[test]
    fn char_at() {
        let c = |position, from_end, only_if: &str, replace: &str| Rule::CharAt {
            position,
            from_end,
            only_if: only_if.into(),
            case_sensitive: false,
            replace: replace.into(),
        };
        assert_eq!(run(vec![c(1, false, "", "Z")], "abc", 0), "Zbc");
        assert_eq!(run(vec![c(1, true, "", "")], "abc", 0), "ab");
        assert_eq!(run(vec![c(2, false, "x", "")], "abc", 0), "abc");
        assert_eq!(run(vec![c(9, false, "", "")], "abc", 0), "abc");
        // "Only if" ignores case unless asked.
        assert_eq!(run(vec![c(1, false, "A", "x")], "abc", 0), "xbc");
        let strict = Rule::CharAt {
            position: 1,
            from_end: false,
            only_if: "A".into(),
            case_sensitive: true,
            replace: "x".into(),
        };
        assert_eq!(run(vec![strict], "abc", 0), "abc");
    }

    #[test]
    fn extension_and_targets() {
        assert_eq!(
            run(
                vec![Rule::Extension {
                    mode: ExtMode::Lower,
                    value: String::new()
                }],
                "A.JPG",
                0
            ),
            "A.jpg"
        );
        assert_eq!(
            run(
                vec![Rule::Extension {
                    mode: ExtMode::Set,
                    value: ".jpeg".into()
                }],
                "A.jpg",
                0
            ),
            "A.jpeg"
        );
        assert_eq!(
            run(
                vec![Rule::Extension {
                    mode: ExtMode::Remove,
                    value: String::new()
                }],
                "A.jpg",
                0
            ),
            "A"
        );
        let full = Rule::Replace {
            find: ".".into(),
            replace: "_".into(),
            case_sensitive: false,
            first_only: false,
            target: Target::Full,
        };
        assert_eq!(run(vec![full], "a.b.c", 0), "a_b_c");
    }

    #[test]
    fn disabled_steps_skipped() {
        let steps = vec![Step {
            enabled: false,
            rule: replace("a", "b", false, false),
        }];
        let f = file("a.txt");
        let (s, _) = Pipeline::new(&steps).run(&Ctx {
            index: 0,
            folder_index: 0,
            file: &f,
            now: Local::now(),
            movie: None,
            episode: None,
            song: None,
            book: None,
            comic: None,
            comic_folder: None,
            video: None,
            as_folder: false,
            photo: None,
        });
        assert_eq!(s, "a");
    }

    #[test]
    fn json_roundtrip() {
        let json = r#"[{"enabled":true,"rule":{"type":"replace","find":"a","replace":"b","caseSensitive":true}},
                       {"rule":{"type":"case","mode":"titleSmart"}}]"#;
        let steps: Vec<Step> = serde_json::from_str(json).unwrap();
        assert_eq!(steps.len(), 2);
        assert!(steps[1].enabled);
    }
}
