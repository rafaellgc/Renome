//! Renaming rules. Each [`Step`] is a rule that can be turned on or off;
//! the [`Pipeline`] compiles the list and applies the rules in order.

use std::path::PathBuf;

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
        /// With `write_tags`, the songs' other tags go (site names, ripper
        /// notes), but for lyrics, loudness and the like.
        #[serde(default = "yes")]
        clean_tags: bool,
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
    },
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
                                tv::episode_name(folder_template, m, colon),
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
            target,
        } => {
            on_target(stem, ext, *target, |s| {
                let mut s = s.to_string();
                if *accents {
                    s = text::strip_accents(&s);
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
                let name = tmdb::movie_name(template, m, colon);
                if !name.is_empty() {
                    *stem = name + tmdb::subtitle_suffix(stem, ext);
                }
            }
        }

        Rule::Series {
            template, colon, ..
        } => {
            if let Some(m) = ctx.episode {
                let name = tv::episode_name(template, m, colon);
                if !name.is_empty() {
                    *stem = name + tmdb::subtitle_suffix(stem, ext);
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
            template, colon, ..
        } => {
            if let Some(m) = ctx.book {
                let name = books::book_name(template, m, colon);
                if !name.is_empty() {
                    *stem = name;
                }
            }
        }

        Rule::Comic {
            template,
            colon,
            by_folder,
            ..
        } => {
            if let Some(m) = comic_of(ctx, *by_folder, stem) {
                let name = comics::comic_name(template, &m, colon);
                if !name.is_empty() {
                    *stem = name;
                }
            }
        }

        Rule::Replace { .. } | Rule::Regex { .. } => unreachable!("compiled separately"),
    }
}

/// The comic a file is: the issue chosen for it, else what it says about
/// itself, its name being `stem` (what the rules above made of it).
fn comic_of(ctx: &Ctx, by_folder: bool, stem: &str) -> Option<comics::ComicMatch> {
    if ctx.file.is_dir || !comics::is_comic(&ctx.file.ext) {
        return None;
    }
    ctx.comic
        .cloned()
        .or_else(|| comics::from_name(ctx.file, stem, by_folder))
}

fn folder_name(file: &FileEntry) -> String {
    file.dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Replaces `{key}` in a single pass, so values that contain braces are
/// not expanded again. Unknown keys are kept as they are.
pub(crate) fn expand<'a>(template: &str, value: impl Fn(&str) -> Option<&'a str>) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let tail = &rest[open..];
        match tail
            .find('}')
            .and_then(|close| Some((close, value(&tail[1..close])?)))
        {
            Some((close, v)) => {
                out.push_str(v);
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
            taken: Default::default(),
            tags: Default::default(),
            comic: Default::default(),
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
            target: Target::Name,
        };
        assert_eq!(
            run(vec![r], "  Canção_01   do  Mês .txt", 0),
            "Cancao do Mes.txt"
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
