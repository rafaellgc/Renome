# Renome — functional specification

Batch file renamer for Linux, Windows and macOS (Rust + Tauri + Svelte).
On macOS it needs version 11 or newer with Safari 16.4 or newer (the
interface runs in the system's WebKit; older ones show a blank window), and
the installer says so (`minimumSystemVersion`).

This document describes **what** the program does, not **how**.

## 1. File list

- "Add" button with three options: files (multiple selection), folders
  (their contents, with subfolders) or folders as items (to rename the
  folders themselves); drag and drop adds files and folder contents.
- A folder's name has no extension: extension rules leave it alone and
  dots in it are kept as text. Folders and files inside them can be renamed
  in the same batch.
- Each file keeps the folder it was added with (its own folder when
  added by itself), for sending to another folder (see 4).
- Hidden files (`.file`, `.git/`; on Windows also files marked hidden or
  system, like `desktop.ini`, and folders marked hidden: Windows marks
  folders with a custom look as system) are left out unless enabled in
  Options; a folder added itself is never left out.
- Files already in the list are ignored.
- Newly added files are selected and the list scrolls to them.
- **List filter**: terms separated by commas/spaces; terms with `*`/`?` are
  patterns (`*.jpg`), the rest look for the text in the name (`holiday`);
  `"quoted words"` look for the phrase; a term starting with `-` hides
  the names it matches (`-*.srt`); case and accents are ignored; `[`,
  `{` and the like are literal. Only visible files are renamed, and
  numbering only counts them.
- Files can be passed on the command line (`renome file1 file2 folder/`).
- Columns: current name, new name, folder, size, modification date (size
  and date are hidden in narrow windows).
  Folders are marked with an icon. In changed rows, what was removed from
  the current name and what was added to the new one are highlighted,
  comparing whole words (letters only inside a word that barely changed,
  `Mario` → `Mário`); words that are on both sides but moved are marked
  apart (dotted underline), and the common start and end (the
  extension) are never marked.
- **Optional columns** (☰ button at the left of the header, shown only
  when a column fits the rules in the stack: the music rule's song columns,
  the movie or TV show rule's resolution and codec, Insert's photo date
  when it inserts the date taken; sorted by a column no longer shown, the
  order stays as a manual one; the choice is saved in the settings): a song's artist, album, title, track (`2-05` on
  albums of several discs), year, genre, length and format (`MP3 · 320
  kbps`, `FLAC · 24/96`), a video's resolution and codec, a photo's date
  taken (EXIF; only image formats are opened for it). They sit between
  folder and size; past their minimum widths the list (with its header)
  scrolls sideways. Values are read only for the rows in view, once per
  file (cached with the file; read again when its size or date change),
  and "Export list" adds the columns that are on.
- Sort by clicking a header (click again to reverse): **new name**
  (default; the list re-sorts as the rules change), current name (natural
  order, `2 < 10`), folder, size, date, or an optional column. Sorting by
  an optional column reads every file (in the background, with a "reading
  the files" notice); files without the value go last either way, and
  ties go by album, disc and track (by disc and track when sorting by
  album), so each album stays in order.
- Move items up/down (manual order; unavailable when sorting by new name),
  remove selected, clear all. What the lookups found (movie, episode,
  book, comic, album) goes with the files taken out, and clearing also
  forgets the shows and series picked, so files added again start fresh;
  Ctrl+Z brings it all back with the files.
- Numbering follows the list order (in "new name" mode, the current name order).

## 2. Rules (pipeline)

Transformations are **rules** stacked and applied in sequence to the name. Only one
lookup rule (movie, TV show, music, book, comic) may be in the stack: with
one there, on or off, the others are disabled in the "+ Rule" menu until it
is removed; a preset or remembered set with more keeps the first and says
which were left out.
Each rule can be turned on/off, reordered and removed; a rule turned off
is locked (no option changes, no searches) until turned on. The preview updates
live. The rule set can be saved and loaded as a *preset* (JSON).

By default rules act on the name without the extension; text rules can
optionally act only on the extension or on the full name.

The lookup rules (movie, TV show, book, comic) read and look up the name
the rules above the first of them make, not the name on disk: a
"Replace text" above the comic rule turns an abbreviation into the series
before it is read. The preview sends that name to the app
(`lookupName`) when it differs from the file's.

| Rule | Parameters |
|---|---|
| Replace text | find, replace with, case-sensitive, first occurrence only |
| Regular expression | pattern, replacement (`$1`, `${name}`), case-sensitive, first occurrence only |
| Rearrange parts | separator; new order using part numbers and text (`1 - 3 - 2`); the parts of a sample file are shown and can be clicked to build the order. Parts are trimmed; parts left out are removed; names with fewer parts are unchanged |
| Case | lowercase; UPPERCASE; Sentence case; Title Case; Title Case except connectives (of, the, de, da…; a part after ` - `, `:`, `(`, `.` starts capitalized). Title Case keeps Roman numerals II–XXXIX in capitals (lowercase `vi`/`xi` are words) and leaves words with inner capitals (`iPhone`, `McDonald`) as they are |
| Insert | text, current date, file modification date, date the photo was taken (EXIF; optionally the modification date when missing, otherwise the name is unchanged) or the name of the folder the file is in; configurable date format; at the start, at the end or at a position (from the start or the end) |
| Number | template with `{n}` (number), `{name}` (current name) and `{folder}` (folder name); start, step, minimum digits (zero padding); optionally restart in each folder |
| Remove | spaces; repeated/leading/trailing spaces; accents (also letters written apart from their base: `ø` → `o`, `ß` → `ss`, `æ` → `ae`, `ł` → `l`, `đ`/`ð` → `d`, `þ` → `th`, `œ` → `oe`, `ı` → `i`, `ħ` → `h`); digits; custom characters; the number at the start (up to three digits, an optional `1-` disc before it, and the separator after it: `01 - `, `01. `, `1-05 `; years, dates and a name that is only the number stay); a maximum length in characters (cut at the end, without spaces, dashes, dots or commas left there; 0: no limit) |
| Convert characters | fix broken accents: each run of non-ASCII characters is turned back into the Windows-1252/Latin-1 bytes it was read as and, when those are UTF-8, decoded (up to 3 times, for text broken more than once; `SÃ£o` → `São`, `â€™` → `’`); text that does not decode stays; other alphabets in Latin letters: Cyrillic (Russian, Ukrainian, Belarusian, Serbian, Macedonian letters; `щ` → `shch`, `ъ`/`ь` dropped) and Greek (accents dropped, `ου` → `ou`), a capital as `Sh`, or `SH` inside a word in capitals; a web-safe name: the name in plain Latin letters (as above and with the accents rule's letters), then only ASCII letters, digits and dots, every other run of characters becoming one `-` or `_` (the user's choice), none at the ends |
| Remove range | from: position N or first occurrence of a text; to: end, position N or occurrence of a text; include the delimiters or not |
| Character at position | position N; optionally only if the current character is X (case-insensitive unless asked); replace with Y or delete |
| Extension | set a new one; lowercase/uppercase; remove; from the content: the extension the file's first bytes call for (pictures, camera RAW, audio, video, PDF, books, archives, Office/OpenDocument, fonts; zips told apart by what they hold: EPUB, Office, pictures only = comic), kept when it already fits (`.nef` is TIFF inside, `.m4b` is MP4), comic extensions turned into the comic one of the real archive (`.cbz` that is RAR → `.cbr`), UPPERCASE kept, split parts (`.r00`, `.001`) never touched; text (UTF-8, UTF-16 with its mark, Windows-1252 without controls) replaces only the extension of a format that is not text (`.pdf` → `.txt`, or `.html`, `.xml`, `.svg`, `.json`, `.srt`, `.vtt`, `.ass`, `.m3u8` when the start tells), never a text one or none; after a dot that starts no extension (`v1.0_x86[0]`) the new one is added; a fitting extension already in the name before the wrong one (one of the content's; for text, a free text format's — `txt`, `nfo`, `csv`, `log`… — or a structured one's only when the content was told as it: `json` valid JSON, `xml`/`opf`/`ncx` XML, `html`, `srt`/`vtt`/`ass`, `m3u`) is kept and the wrong one only taken off (`movie.nfo.pdf` → `movie.nfo`, `photo.png.jpg` → `photo.png`) |
| Movie (TMDB) | template with `{title}`, `{original}`, `{year}`, `{id}`, `{resolution}`, `{video codec}` (default `{title} ({year}) [tmdbid-{id}]`); what `:` becomes (default ` - `); a folder for each movie; save `.nfo` and poster (optionally fanart and logo) next to renamed videos; show the collection's missing movies. See 2.1 |
| TV show (TMDB, AniList) | the source (TMDB, or AniList for anime); the show (name, TMDB or AniList id or link; empty: from the files); template with `{show}`, `{original}`, `{year}`, `{season}`, `{episode}`, `{title}`, `{id}`, `{resolution}`, `{video codec}` (as for movies, see 2.1; not in the show folder) (default `{show} ({year}) - S{season}E{episode} - {title}`); what `:` becomes; show and season folders (show folder template, default `{show} ({year}) [tmdbid-{id}]`); save `.nfo` files and images (optionally fanart and logo); show the missing episodes. See 2.2 |
| Music (tags) | template with `{track}`, `{title}`, `{artist}`, `{album artist}`, `{album}`, `{year}`, `{disc}`, `{genre}`, `{composer}`, `{total tracks}`, `{total discs}`, `{original year}`, `{label}`, `{catalog}`, `{artist sort}`, `{album artist sort}`, `{initial}`, `{format}`, `{bitrate}`, `{sample rate}`, `{bits}` (default `{track} - {title}`); what `:` becomes; disc number in the track on multi-disc albums; fill in from the file name; artist and album folders (default `{album artist}/{album} ({year})`); show the missing tracks. See 2.3 |
| Book (Open Library) | template with `{author}`, `{authors}`, `{author sort}`, `{initial}`, `{title}`, `{original}`, `{year}`, `{edition year}`, `{publisher}`, `{isbn}`, `{series}`, `{volume}`, `{id}` (default `{author} - {title} ({year})`); what `:` becomes; author and book folders (default `{author}/{title}`); save the cover and `metadata.opf`; write the details inside EPUBs; show the series' missing volumes. See 2.4 |
| Comic (ComicInfo.xml, GCD, Metron, Comic Vine, MangaDex, AniList, MangaUpdates) | template with `{series}`, `{series year}`, `{number}`, `{volume}`, `{chapter}`, `{title}`, `{year}`, `{count}`, `{publisher}`, `{writer}`, `{artist}`, `{id}` (default `{series} v{volume} #{number} c{chapter} - {title} ({year})`); what `:` becomes; the series (empty: from the files); the catalogue; series folders (default `{series} ({series year})`); series from the folder; write `ComicInfo.xml` into CBZs; convert CBR, CB7, CBT and PDF to CBZ (the original kept beside or deleted); show the missing issues. See 2.5 |

Positions count **visible characters** (Unicode graphemes), starting at 1.

Templates (every rule above with one, file and folder templates alike)
can have **optional parts**: `<...>` is written without the `<>` when
every `{field}` in it has a value, and left out otherwise
(`{album artist}/{album}/<CD{disc}>`, `<{year} - >{title}`). Parts do
not nest and do not span a `/` folder level; a `<...>` with no known
field, or a `<` with no `>` after it, is kept as typed (Windows refuses
`<` and `>` in names anyway, so no name needed them).

A key may name **fallbacks**: `{a|b|c}` is the first of those keys with
a value (`{title|year}`: the title, else the year), empty when all are
empty; unknown choices are skipped, and one with no known key is kept as
typed. It works inside optional parts. `{key:N}` (N from 1 to 9) writes each
whole number in the value with N digits (`{episode:3}`: `001-E002`;
`{track:1}`: `5`), leaving decimals (`12.1`), a disc before a track
(`1-05`) and text without numbers as they are.

**What is already there** is read before searching:
- Movie: the `.nfo` beside the video (`<name>.nfo`, else the folder's
  `movie.nfo`; a movie folder's own `movie.nfo`): a `<movie>` document's
  `uniqueid type="tmdb"`/`tmdbid`, else its IMDb id (`uniqueid
  type="imdb"`, `imdbid`, `id` starting with `tt`), else TMDB movie or
  IMDb title links anywhere in the text (Kodi's link after the document,
  release notes). The TMDB id becomes the guess's id; an IMDb id is
  turned into one with TMDB's `/find` (kept like any answer).
- TV show, when no show is typed: the first `tvshow.nfo` (in a file's
  folder or the one above it) with a TMDB or IMDb id names the show.
- Book: `<name>.opf` beside the book, or `metadata.opf` when the folder
  holds no other book (under 1 MB, with a title or ISBN): its title,
  authors, year, publisher, language and series win over the file's own;
  its ISBNs come first.
- Comic: without a `ComicInfo.xml`, the `MetronInfo.xml` (series name,
  volume, issue count, format, number, stories joined by ` / ` or the
  collection title, cover date, publisher and imprint, credits by role:
  writer/script/story/plot, artist/penciller/illustrator/breakdowns/
  layouts, inker/embellisher/finishes, colorist, letterer, cover, editor,
  translator; arcs, first genre, pages, age rating, series language,
  alternative number, ISBN or UPC, the Comic Vine id); without either, a
  zip's comment with ComicBookInfo 1.x JSON (series, issue, volume,
  title, publisher, publication year and month, issue count, genre,
  language, credits by role).

**More fields** (`fields.rs`; each also in Portuguese and Spanish, an
empty one is empty, not unknown): dates give `{date}` (`2010-05-03`, or
the part known), `{month}` (two digits), `{day}` and the month's name in
the language of the key (`{month name}` May, `{nome do mês}` maio,
`{nombre del mes}` mayo).
- Movie (from the details the collections already fetch, one request per
  chosen movie, kept like any answer): `{genre}`, `{genres}` (three),
  `{country}`, `{language}` (original), `{runtime}` (minutes),
  `{certification}` (the titles' language's country's), `{director}`,
  `{studio}`, `{collection}`, release date.
- TV show (from the show and its episodes): `{network}`, `{genre}`,
  `{genres}`, `{language}`, `{country}`, the episode's air date,
  `{absolute}` (counted from the first episode, specials apart).
- Movie and TV show, from the file's name: `{edition}` (Director's Cut,
  Extended, Unrated, Uncut, Theatrical, Remastered, IMAX, Criterion,
  Special/Ultimate Edition, Final Cut, Anniversary), `{source}` (Remux,
  BluRay, WEB-DL, WEBRip, WEB, HDTV, DVDRip, DVD, HDRip, TVRip, VHSRip)
  and `{release group}` (`[Group]` at the start, or `-GROUP` at the end of
  a name with a source or resolution, not a codec).
- Music: the release date (tags' date, or the MusicBrainz release's).
- Book: `{language}` (two letters), `{pages}` (the edition's found by
  ISBN, else Open Library's median among editions).
- Comic (`ComicInfo.xml`): `{story arc}`, `{imprint}`, `{format}`,
  `{genre}`, `{inker}`, `{colorist}`, `{letterer}`, `{cover artist}`,
  `{editor}`, `{translator}`, `{pages}`, `{age rating}` (not "Unknown"),
  `{language}`, `{alternate series}`, `{alternate number}`, `{gtin}`,
  `{scan info}`, and the cover date (`Year`/`Month`/`Day`, replaced by the
  catalogue issue's cover date). From the name: the scanner is the last
  bracketed note that is not a year, a count, a volume or release words,
  or what follows `--`/`__` (never part of the series); `c2c` and `FCBD`
  are release words.

### 2.1 Movies (TMDB)

- Needs a TMDB API key (v3 key or v4 read token) in Options, plus the
  language of titles (default: the interface's). The key stays in the
  local settings; only the searched title and year (and, for collections,
  the chosen movies' ids) are sent to TMDB.
- Only videos, subtitles and folders take part (looked up and renamed);
  images, `.nfo` and other files stay as they are, even with a match
  kept from before.
- "Search TMDB" looks up the visible files without a movie yet. Title and
  year come from the name: dots and underscores become spaces; the year is
  the last plausible one (1900–next year) with text before it; without a
  year, the title stops at the first release tag (1080p, BluRay, x264…);
  bracketed parts are dropped. Files with the same guess (a movie and its
  subtitles) share one search and one choice.
- Each search asks TMDB by title, and also by title within the year.
  Candidates are ranked by title similarity (ignoring case, accents and
  punctuation, against the localized and the original title) minus a
  penalty for the year difference (±1 year is common and costs little).
  The first one is picked automatically only with (almost) the same title,
  the same year and no close second; otherwise the file waits for the
  user.
- Next to the new name, each file shows ✓ (picked), ? (choose) or ✕ (no
  movie); clicking it opens the review window: candidates with poster,
  title, original title, year and overview; click (or 1–9) to choose, a
  search field to try another title or year, "None of these".
- Files without a movie are left unchanged by the rule. Titles are made
  safe for file names: `:` becomes the chosen text, slashes become
  dashes, `? * < > |` are dropped; empty `()` or `[]` (no year) are
  removed.
- `{resolution}` and `{video codec}` come from the video's main track
  (the largest video track), read from its headers; the format is told
  by the first bytes, not the extension. Matroska/WebM: the segment's
  elements are walked by their headers to the track list (`PixelWidth`,
  `PixelHeight`, `CodecID`); MP4/QuickTime: the top-level boxes to
  `moov`, whose video tracks (`hdlr` `vide`) give the size of their
  first sample description (else the track header) and its format;
  AVI: the video stream's bitmap header (else the main header); MPEG
  program streams and video CDs (RIFF `CDXA`): the first sequence header
  (an extension marks MPEG-2); FLV: `onMetaData`'s `width`/`height` and
  the first video tag. The media data is skipped, not read, and only
  what is needed is parsed, so files cut short or with odd parts
  elsewhere are read; others (TS, WMV, RealMedia, ISO) give empty
  values. The
  resolution is named from the frame: `2160p` (width ≥ 3200 or height ≥
  1800), `1440p` (≥ 2200 or ≥ 1300), `1080p` (≥ 1700 or ≥ 1000), `720p`
  (≥ 1100 or ≥ 700), `576p` (height ≥ 540), `480p` (≥ 460), else the
  height (`360p`), so a film cropped wider keeps its width's name. The
  codec: `H.264`, `HEVC`, `AV1`, `VP9`, `VP8`, `VP6`, `H.263`, `MPEG-4`
  (also DivX/Xvid and Microsoft's), `MPEG-2`, `MPEG-1`, `MJPEG`,
  `ProRes`, else empty. A subtitle takes the video in its folder with the
  same movie (or episode), the one with its name (without the language)
  first, else the only one. A video is read only when the template asks
  for one of these. A dash left at either end of the name is removed.
- "Create a folder for each movie" moves each file with a movie (video
  and subtitles alike) into a folder named after its new name as the
  video's, without the subtitle's language and without the resolution
  and codec (so versions of a movie share it), inside its
  current folder; a file already in a folder with that name stays. When
  the current folder is the movie's own under another name, the new
  folder goes next to it instead. A folder is the movie's own when it is
  named after the movie (its name holds `tmdbid-<id>` of that movie, or its title, guessed like a
  file name, has similarity ≥ 0.8 with the title or original title and
  its year, if any, is within one of the movie's) or when it holds, three
  levels down, only videos of that movie (at least one), subtitles,
  images (jpg, jpeg, png, webp, gif, bmp), `.nfo` and `.txt` files; and
  in both cases it holds no other movie of the batch and is not one of
  the user's own folders (see 2.2: the home folder and its standard
  folders, a disk's top folder, the file system's). The preview shows the
  folder before the new name; the folder name is
  checked like a file name. Missing folders are created when renaming,
  recorded in the history entry, and removed by undo when left empty.
- Files with a movie, episode, book or comic (found in a catalogue) that
  no rule changed follow it: those named after it (its old stem, then
  `.`, `-` or a space: `Name.nfo`, `Name-poster.jpg`) get its new stem and
  go where it goes; and, in a movie's own folder (as above, with one
  movie), when that movie goes to a new folder (by its rule, or sent to
  another folder), the others go there too (`backdrop.jpg`, `movie.nfo`),
  with their subfolder (`extrafanart/`, two levels at most), when they
  were added with the movie (same folder added). Before "send to another
  folder" places what is left.
- With "save .nfo and poster", renaming also writes `<new name>.nfo` next
  to each renamed video, in Kodi format with every field Jellyfin's NFO
  reader takes that TMDB provides: title, originaltitle, year, premiered,
  releasedate, plot, tagline, runtime, rating, votes, mpaa (age rating of
  the language's country: `PG-13` in the US, `BR-12` elsewhere, like
  Jellyfin's own TMDB provider), genre, tag (keywords), country, studio,
  trailer (YouTube, Kodi plugin form), set with `tmdbcolid`, director,
  writer (writing department), tmdbid, imdbid, uniqueid (tmdb, imdb), and
  actor entries with `type` (20 actors with role and photo, 5 producers,
  composers). Images: poster, and optionally fanart (backdrop) and logo
  (PNG title logo in the language, else English); named `poster.jpg`,
  `fanart.jpg`, `logo.png` inside the movie's own folder, else
  `<new name>-poster.jpg` and so on. Existing files are never replaced
  (true of every file written beside others: `.nfo`, images, covers,
  `metadata.opf`, playlists; tags inside songs and details inside CBZ
  and EPUB files, unlike them, are rewritten when they differ, see 2.3,
  2.4 and 2.5).
  They are recorded in the rename's history entry and deleted by its undo.
- Movies whose names are already right (unchanged rows with a movie) get
  the metadata too when renaming: the Rename button counts them and the
  confirmation mentions them. Files that already exist are skipped
  without error (and not downloaded). Without any rename, the created
  files get a history entry of their own ("N metadata files"), undone by
  deleting them.
- A name with `tmdbid-N` (as in `[tmdbid-12345]`) is looked up by that id
  directly, and picked automatically.
- The About tab shows the TMDB logo and notice required by its terms.
- Subtitles (srt, ass, ssa, sub, idx, vtt, sup, smi) keep the language
  and flags at the end of their names (`.pt-BR`, `.eng.forced`: lowercase
  ISO codes, optionally with a region; flags forced, sdh, cc, default,
  full, foreign) after the movie or episode name, each after a dot, flags
  in lower case (`Movie.pt-BR.forced.srt`). The flags forced, sdh, cc and
  foreign are also read after a dash, an underscore or a space
  (`Movie-forced.srt` → `.forced`).

- "Show the collection's missing movies" (on by default; old saved rules
  get it on): for each chosen movie, TMDB is asked once for its
  collection (`/movie/{id}`) and once per collection for its movies
  (`/collection/{id}`); the released movies (release date known and not
  after today) of a collection that no listed video is matched to become
  faded rows like the missing episodes (`year · title`, release date),
  in release order (unknown dates last). The bottom bar counts them;
  clicking saves a CSV (collection, title, release date).

### 2.2 TV shows (TMDB)

- A name made only of bracketed pieces (`[Group][Show (1987)][S03E22]
  [Title][1080p]`, also `【】`) reads the piece with the episode, the
  one before it as the show and the one after as the title (not a release
  tag). The show's year and a `[tmdbid-N]` in the names are kept: an id
  most files carry looks the show up directly (by name if it finds
  nothing).
- Only videos and subtitles take part (looked up and renamed); folders,
  images, `.nfo` and other files stay as they are.
- Same key and language as movies. "Find episodes" first finds the show:
  by id (digits, a `themoviedb.org/tv/N` link or `tmdbid-N`), else by name
  (with an optional year) on TMDB's show search, ranked like movies. An
  exact, unambiguous match is picked automatically; otherwise a window
  lists the shows (poster, name, original name, year, overview) with a
  search field. The same text reuses the show picked for it before.
- With the field empty, the files are first grouped by show, and each
  group is searched in turn (by the text before the episode number most
  of its names share, with their year; by the `[tmdbid-N]` most carry;
  for a folder group, by the folder's name, the one above for a season
  folder). Names go together when their shows are the same (see below)
  or carry the same id. A name without a show joins the group with more
  than half of the named files in its folder (the one above a season
  folder); in a folder without named files, the folder's files make a
  group; otherwise it is left out, and a notice says how many. A group
  whose show cannot be told, or whose window is closed, is skipped; the
  others go on. The rule card lists the shows found.
- Files of another show are left alone (no mark): a `[tmdbid-N]` of
  another id, or a show name that is not the show's. Names are the same
  show when (after dropping the year and tags) their similarity is
  ≥ 0.8, the file's words hold the show's in a row (`Site - Show`), or
  the show's start with the file's (`Show` for `Show: The Series`). The
  show's names: its name and original name, its name in each language
  fetched, its TMDB alternative titles (`append_to_response=
  alternative_titles`) and the text it was searched by (unless an id).
- A name without a show and without an id, outside the show's folder
  (named as for the folders option, or the one above a season folder),
  keeps the numbers' episode only when it has no title or its title
  scores ≥ 0.5 against that episode; otherwise it waits for the user,
  that episode suggested first.
- The show is fetched with all its seasons (`append_to_response`, 20
  seasons per request) in the user's language, in English and in the
  original language. Each episode is named in the first of those with a
  real title (TMDB fills untranslated ones with "Episode N"); all titles
  are kept for matching.
- Numbers read from each name (dots and underscores as spaces), first
  match wins: `S01E02`/`T1 E2` (up to four digits each, so `S2015E05`;
  with `E02`/`-02` for more episodes), `1x02`, `Season 1 Episode 2`
  (also Portuguese, Spanish, French and German words, and `6ª Temporada
  Ep. 2`), an air date `2024.03.15`, `Episode 5`/`Ep 05`/`E05`/`Part 2`,
  else a bare number before any release tag that is not a year. Without a
  season, the folder gives it (`Season 2`, `Temporada 02`, `S02`,
  `Specials` = 0); a bare number in a season folder is the episode (or,
  from 100 up with the folder's season in front, `205` = episode 5). The
  text after the numbers, up to the first release tag and without
  bracketed parts, is the title; the text before, the show.
- Matching: the numbers give candidate episodes that exist in the show:
  season and episode; an episode without season is from the only season,
  else counted straight through the regular seasons; a bare number is
  counted straight through and also read as season and episode (`201` =
  S02E01); for shows with seasons over 99 episodes, only counted straight
  through when that episode exists; with a title, a bare number under 100
  is also that episode of each season (every season in one folder, each
  numbered from 1); an
  air date gives the episodes aired that day. The title is compared with
  every episode's titles (same similarity as movies; the show's name in
  front is ignored).
- Title similarity, here and in the other rules, is the Dice coefficient
  of the letter pairs of both names' words joined (case, accents and
  punctuation ignored): 1 only for the same words, at most 0.99
  otherwise.
- Title similarity (movies, shows) reads Roman numerals up to 39 as
  numbers: `Part II` = `Part 2`.
  - Numbers, no title: one candidate is picked; several wait for the user.
  - Numbers and title: when the title's best episode scores ≥ 0.7, is not
    among the number's candidates and beats them by over 0.2, the file
    waits for the user (a conflict), suggesting the title's episode first.
    Otherwise one candidate is picked, or the best of several when it
    scores ≥ 0.6 and leads by 0.1, or is the very title (1.0) and the
    others are not.
  - Title only: picked when it scores ≥ 0.85 and leads by 0.1; otherwise
    the episodes scoring ≥ 0.4 are suggested.
- Marks and review as for movies (✓, ?, ✕). The review shows what was
  read from the name, the conflict, up to 9 suggestions (still image,
  code, title, air date, overview; 1–9 or Enter for the first) and every
  episode by season (specials last) with a filter by title or code.
  Ctrl+click marks several episodes of one season for a multi-episode
  file. Undecided files read the same way get the same choice.
- Each file keeps its show. Searching again skips files already settled
  for the same show.
- Names: two-digit season and episode; several episodes give `01-E02`;
  titles are joined with ` & `; titles are made safe like movie titles;
  a missing title or year leaves no dangling ` - ` or `()`.
- "Show and season folders" moves each file with an episode (not
  folders added as items) into `<root>/<show folder>/Season NN` (`Season
  00` for specials). The root: the parent of the show's current folder,
  when the file is in it or in a season folder inside it (a folder names
  the show when it holds `tmdbid-<id>`, or its title, without year and
  tags, has similarity ≥ 0.8 with the show's name or original name, or
  starts with its words); else the parent of a loose season folder; else
  the file's own folder. A folder also counts as the show's, whatever its
  name, when every video in it (four levels down) is a listed episode of
  that show; never the user's own folders (home, Downloads, Desktop,
  Documents, Videos, Music, Pictures, Public), a disk's top folder (another
  device than its parent) or one right under `/`. A file already there
  stays. The preview shows `<show folder>/Season NN` before the new name;
  each folder name is checked like a file name.
- An old folder that files leave for a new one next to it keeps the files
  that do not leave (extras, a file in conflict or hidden by the filter):
  it is then renamed `<new folder> (<suffix>)` next to the new one (suffix
  in the UI language: "arquivos adicionais", "additional files",
  "archivos adicionales"; skipped if that name exists). This applies to
  every rule that moves files into folders (movie, series, music, book,
  comic): for each moving file, the destination root is the destination
  without the folder levels the rule made; the old and new paths below it
  are compared level by level, and the first level where they differ
  gives the old folder and its new one. Not renamed: a folder differing
  only in case (the same folder where case is ignored); a folder whose
  files go to different new folders; a folder some file goes into or that
  is itself in the list; a folder with fewer levels below the root than
  the new path (`Title` → `Author/Title`), except for episodes (the season
  folder is the extra level); the user's own folders, a disk's top folder
  or one right under `/`. It is an operation of the rename, run after the
  files leave it (shallower), undone with it. The bottom bar shows "N
  folder(s) with leftovers" (tooltip: each folder), and applying always
  asks first, naming the folder and the count, even with confirmation
  off. The result counts files and those folders apart.
- After any rename, the folders files were moved out of are deleted when
  left empty, up to the folder the move happened within (inside a folder
  renamed in the same batch, under its new name). Undo makes them again
  when moving the files back.
- With "save .nfo and images", renaming also writes, for renamed episode
  videos and for unchanged ones with an episode (counted by the Rename
  button and the confirmation, like movies):
  - next to the video: `<name>.nfo` with one `<episodedetails>` per
    episode in the file (title as in the name, showtitle, season,
    episode, aired, premiered, plot, runtime, rating, votes, uniqueid
    tmdb, director, writer, thumb, guest stars as `GuestStar`), and
    `<name>-thumb.jpg` (the first episode's still);
  - in the show's folder (the one above the episode's season folder when
    the folders option moved it there, else the current show folder as
    above, never just whatever folder holds the file; none for loose
    files): `tvshow.nfo`
    (title, originaltitle, showtitle, year, premiered, plot, tagline,
    runtime, rating, votes, mpaa as for movies, status Continuing/Ended/
    Unreleased, genre, tag, studio (networks, then companies), credits
    (creators), trailer, tmdbid, imdb_id, tvdbid, uniqueid tmdb/imdb/tvdb,
    episodeguide in Kodi 20's JSON form, 20 actors from the aggregate
    credits), `poster.jpg`, `seasonNN-poster.jpg`
    (`season-specials-poster.jpg`), and optionally `fanart.jpg` and
    `logo.png`;
  - in a season folder of the same season: `season.nfo` (title, plot,
    premiered, releasedate, seasonnumber).
  Show details and seasons are fetched once per batch. Existing files are
  skipped; created ones are recorded in the rename's history entry (or an
  entry of their own) and deleted by undo.
- The missing rows of every rule (episodes, issues, tracks, movies,
  volumes) have a checkbox beside their count in the bottom bar (on by
  default, saved as `showMissing`): off, the rows leave the list (the
  count and the CSV stay; nothing is searched again).
- "Show the missing episodes" (on by default): the list shows, between
  the files, a faded row for each aired episode (air date known and not
  after today) that no listed video holds, in the seasons at least one
  listed video (after the filter) is matched to; seasons without one are
  skipped. A multi-episode file covers all its episodes; a subtitle alone
  does not. Each row sits before the video with the nearest later
  episode of its season, else after the one with the nearest earlier
  (swapped when sorted backwards); it shows `SxxEyy · title` and the air
  date. These rows cannot be selected (clicks, arrows and Shift ranges
  skip them) and are not counted, renamed or in the list's export; the
  bottom bar says how many episodes are missing. Clicking that count
  saves them to a CSV file (same separator and UTF-8 mark as the list
  export): show (with year), season, episode, code (`S02E04`), title and
  air date (`YYYY-MM-DD`), sorted by show, season and episode. The save
  window starts in the folder the videos share, above a season folder,
  as `Show (Year) - missing episodes.csv` (one show). No extra
  request is made.
- Only the show's name (or id) is sent to TMDB.

- Source (`source`, `tmdb` by default; old rules get it): `anilist`
  looks anime up on AniList with no key (GraphQL; `type: ANIME`,
  formats TV, TV_SHORT, ONA and OVA in the search; entries with a
  PREQUEL relation to a season left out, unless they are all there is).
  The typed text may be an AniList id or `anilist.co/anime/<id>` link
  (then the show is picked by itself). The show is built from the
  entry picked and the SEQUEL chain of seasons (TV, TV_SHORT, ONA; 12 at
  most), one AniList request each, 2 s apart: season n is the nth
  entry; its episodes 1 to `episodes` (while airing, the next airing
  episode minus one); titles from `streamingEpisodes`
  (`Episode N - Title`, by N), else none; air dates from
  `airingSchedule` (50 per entry); an episode it lacks takes the date
  of the one before it, the first one the start date. Name: the typed or guessed name (without a trailing
  `(year)`) when it is one of the first entry's titles or synonyms,
  else English, else romaji; original name native (else romaji);
  aliases every title of every season; extra `genre`, `genres`,
  `country`. The show carries `source: "anilist"`, as do its matches: a
  template's `tmdbid-{` / `tmdbid={` becomes `anilistid-{`, a folder is
  the show's by `anilistid-<id>`, and a `[tmdbid-…]` in a file's name
  does not exclude it. The UI keeps AniList shows under negative keys
  (TMDB's ids may be the same numbers) and the picked shows under
  `source:query`; posters are AniList's whole URLs. No `.nfo` files or
  images (the option is hidden and ignored). Answers kept 7 days at most.

### 2.3 Music (tags)

- With artist and album folders, a folder of loose songs (at least 3
  songs from at least 3 albums, and at least 4 in 5 of those albums with
  only one or two songs there; proportions here and below are exact, with
  no rounding) is a collection: its songs are renamed but stay in it.
- With artist and album folders, a folder whose songs are the folders'
  root (its own songs, or those of its disc folders) and where every
  audio file three levels down is a song of the batch from one album
  (same album artist and album, ignoring case) is that album's folder
  whatever its name (the same goes for a folder of one comic series,
  every comic file in it of that series, and a folder of one book, every
  book file in it that book, same authors and title): the new folders go in its parent instead, and it is
  deleted if left empty (else it keeps its name: it becomes two levels,
  see the leftovers rule). Never the user's own folders, a disk's top
  folder or one right under `/`; never a root reached by names (`Music`
  above `Artist/Album`).
- A song whose tags have no track number keeps the number its name starts
  with. An album (folder + album name) gets one year, its tracks' most
  common, so a reissue's bonus track does not get a folder of its own.
- Tags are read with `lofty` (no audio decoded, no cover art): MP3/MP2
  (ID3v2, ID3v1, APE), FLAC, OGG Vorbis, Opus, Speex, M4A/M4B/M4P/M4R,
  AAC, AIFF, WAV, WavPack, APE, Musepack. Video containers (`mp4`, `m4v`,
  `3gp`) are left out. Each field comes from the first tag that has it,
  the file's main tag first. Read once per file and cached.
- Fields: title, artist, album artist (the artist's when missing),
  album, year (from the date), track and total, disc and total, genre,
  composer, original year (the year of `ORIGINALDATE`/`ORIGINALYEAR`/
  `TDOR`; in templates, the year when missing, and left empty like the
  year beside an album named with one), label, catalog number, artist
  and album artist sort names (`ARTISTSORT`, `ALBUMARTISTSORT`; in
  templates, the name itself when missing, and the album artist's falls
  back to the artist's sort name when the album has no artist of its
  own); `{initial}` is the album artist sort name's first letter,
  capital and without accents (`#` for a digit or sign). From the file,
  not its tags: format (the extension in capitals), bitrate (kbit/s, as
  the reader measures it), sample rate (kHz, `44.1`) and bit depth
  (lossless formats only). A song without a title (after the name fallback, if on) is
  left unchanged and flagged (`untagged`, ✕ in the table, a count in the
  card).
- Names: track with two digits (three when the album or track number
  goes over 99); `disc-track` (`1-01`) when the option is on and the
  album has more than one disc (disc total over 1, or disc over 1;
  `{disc}` is empty on other albums);
  values made safe like movie titles, keeping final dots in file names
  (the extension follows; folder names lose them); empty `()`/`[]` dropped, repeated
  ` - ` separators collapsed, separators at the ends trimmed.
- Album-wide agreement (before naming): songs tagged `Single`/`Singles`
  as album take their title as album. In a folder (disc folders count as
  their parent) of at least 3 songs where more than half share an album
  tag, songs with another album tag whose track number is above that
  album's highest on the same disc (bonus tracks: a single, a soundtrack
  song) take its album, album artist, year and release id. Then songs
  with the same album tag in one folder share: the highest disc number
  as the disc total of those without one (or with a lower one); the most common
  year for those without one; the most common album artist for those
  without one; when none has one, the main artist (`feat.`, `ft.`,
  `featuring`, `with` guests left out) of more than half the songs, else
  `Various Artists`.
- Compilations: an album where fewer than 4 in 5 songs have the album
  artist as main artist is a compilation; there a file template without
  `{artist}` gets `{artist} - ` before `{title}`.
- `{year}` is left empty in a template that also has `{album}` when the
  album already contains a year (19xx/20xx) (`Show 12 (2025-05-04)`).
- Fill in from the name: `01 - Artist - Title`, `01. Title`, `1-05
  Title`, `Artist - Title`, `Title`; album, artist and year from the
  folder (`Artist - Album (1975)`, `Album`), the one above a disc folder
  (`CD2`, `Disc 2`, which gives the disc). Only empty fields are filled.
- Folders: one level per `/` in the folder template (empty levels left
  out). The root: going up from the file's folder past folders that are
  a disc folder, are named (similarity ≥ 0.8, also without year and
  release tags, on both sides) after a level, the album, the album
  artist, the artist, "artist - album" or "album - artist" (or start
  with one of them, also comparing the words without bracketed parts,
  years and dates: `1012 - Concert 2024` for "Show 1012 - Concert 2024
  (2024-10-27)"; or
  are, with similarity ≥ 0.85, a run of at least two of its words, like
  `1037 - Title` for
  "Show 1037 - Title (2025)"), or
  sit inside a folder named after the artist;
  at most one level more than the template has.
- Folders some songs are moved out of (old artist or album folders, below
  their root) are climbed past by other songs whose root they would be,
  so a side project in `Artist [restored]/Side Project` ends up next to
  `Artist`, not inside the emptied folder.
- Collections: when every song going into a root has at least two folder
  levels and more than half have an album name starting with the
  root folder's name (without years and brackets: `Show (1000 - ..)` for
  "Show 1037 - …"), that level is dropped and the albums go straight into
  the root (a song already in its album folder there stays).
- Companions (when the music rule is on, for files no rule changed):
  a file with a song's stem in the same folder (lyrics) gets the song's
  new stem and destination; with folders, other files in a folder whose
  songs (in the list, without conflicts) all go to one place go there
  too, keeping their names (a folder's place is where more than half
  its songs go); files in song-less subfolders of such a
  folder (`Artwork/`, up to two levels) keep their subfolder; files in a
  folder whose song folders are all disc folders going to one place go
  there. Followers that would share a name get ` (<their folder>)`.
- Folder levels that differ only in case across the batch (`artist name`,
  `Artist Name`) are unified, for every rule that moves files: an existing
  folder's spelling, else the most common one (the first on a tie).
- "Create a playlist for each album": after renaming, each folder a
  song went to (the one above, for a disc folder) gets
  `<folder name>.m3u8` (UTF-8, `#EXTM3U`, one
  `#EXTINF:<seconds or -1>,<artist> - <title>` per song, paths relative
  to it, with `/` on every system) listing its songs and those of its
  disc folders, by disc (the disc folder's number, else the tag), track
  number and file name; not when it has fewer than two songs or already
  has a `.m3u` or `.m3u8` (which follows the renames; the summary counts
  those albums); a `.cue` does not stop it. Folders of visible songs that keep
  their names get one too, and count in the Rename button. Recorded in
  the history and deleted by undo.
- MusicBrainz (no key; `User-Agent: Renome/<version> ( <repo> )`; one
  request at a time, at least 2 s apart; a 503, which it also sends when
  shedding load whatever the pace, is retried up to 3 times after its
  `Retry-After`, at least 3 s and at most 60 s): "Search
  MusicBrainz" groups the visible tagged songs in albums (as above, after
  the album-wide agreement), skipping albums already settled. An album
  whose songs all carry the same release id (`MUSICBRAINZ_ALBUMID` and
  its ID3/MP4 forms) is looked up by it and taken when every song
  matches one of its tracks (else it waits for review). Otherwise it is
  searched (`release:"<album>" AND artist:"<album artist>"`, the artist
  left out for Various Artists, 25 results; when nothing is found, once
  more with the album name without bracketed parts, "Album (20th
  Anniversary Edition)" → "Album") and ranked: title similarity
  + 0.6 × artist similarity + 0.3 for the same track count as the songs
  (0.1 when more, −0.2 when fewer) + 0.1 for the same year + 0.05 if
  official + MusicBrainz's score / 2000. The best one is fetched with
  its tracks and picked when every song matches a track and at least
  90 % of the songs with a length are within 5 s of it; otherwise the
  album waits for the user (review: cover from the Cover Art Archive,
  title, artist, date, country, format, label, type, track count
  highlighted when equal to the songs'; a search with artist and album;
  "None" keeps the tags). Songs are matched to tracks by disc (1 when
  missing) and number when the track has the song's title (similarity ≥
  0.6, or ≥ 0.8 without bracketed parts) or length (within 5 s), else by
  title (≥ 0.8), each track once; a song that fits no track keeps its
  tags. "Picked without doubt" also needs every song matched this way.
  A song heard on AcoustID (below) is matched first by its recording.
- Discogs (optional; the user's personal token, set in Options → Discogs
  with a Test button that runs a one-result search, kept in the settings
  and handed to the backend at start and on every change; requests carry
  `Authorization: Discogs token=…` and `User-Agent: Renome/<version>
  +<repo>`, at least 1.1 s apart; 429 is retried twice after 20 s;
  401/403 is "invalid token"). With a token, an album MusicBrainz finds
  nothing for (after the bracket-less retry) is searched on Discogs
  (`/database/search`, `type=release`, `release_title` and `artist`,
  the artist left out for Various Artists, 25 results; when nothing is
  found, once more as free text `q="<artist> <album>"`), ranked as
  above (the search gives no track counts) and the best fetched
  (`/releases/<id>`, and its master's year, `/masters/<id>`, as the
  original year; both kept for the session) and picked under the same
  rule. Releases are read as MusicBrainz's, with ids `discogs:<n>` (tracks
  `discogs:<n>:<disc>-<position>`): artists as credited (`anv`, else the
  name without Discogs' `(2)`; "Various" → Various Artists) joined as
  Discogs joins them; `released` with its zero parts dropped
  (`1987-07-00` → `1987-07`), else the year; headings left out; an index
  track's parts are tracks of their own (`Work: Part`) unless their
  positions end in a letter (`3a`, `3b`: one track, its length the sum);
  tracks whose positions are parts of one (`12.1`, `12.2`, or `3a`, `3b`
  outside an index) are one track, titled `Part / Part`, its length the
  sum; the disc from the position (`1-05`, `CD2-3`, `CD2.1`), else disc 1
  (vinyl sides `A1`, `B2` number on as one disc), positions renumbered
  1, 2, 3… per disc; durations `m:ss`/`h:mm:ss`; labels (without
  `(n)`) and catalog numbers (not "none"); the barcode's digits; styles
  then genres as genres; no country in the tags (Discogs names
  countries, not codes). The review offers MusicBrainz or Discogs for a
  typed search when a token is set (Discogs by default when the album's
  release is Discogs'); Discogs candidates show the search's thumbnail
  and a "Discogs" label, and no track count. The cover (`cover.jpg` and
  the embedded one) is the release's primary image (else its first), as
  Discogs serves it (about 600 px); the embedded one is made smaller to
  the cover size chosen (500 or 250 px, JPEG quality 90, with the `image`
  crate), never larger.
  Writing tags from a Discogs release writes no MusicBrainz ids, and a
  match from Discogs does not call for writing because the tags lack its
  id. Credited in Help with Discogs' notice (not affiliated, sponsored or
  endorsed; trademark of Zink Media, LLC). Identifying by sound uses only
  MusicBrainz. Only the artist and album name are sent.
- "Identify by sound (AcoustID)", on by default: after the search, the
  songs of albums without a release picked (none found, or in doubt) and
  the visible songs without a title in their tags (even with one taken
  from the file name, which is only a guess) are identified by their
  sound; for an album the search had candidates for, what was heard
  counts only when it picks a release for some of its songs, else the
  search's candidates stay. Each one's first 120 s are
  decoded (`symphonia`: MP3, FLAC, Ogg Vorbis, MP4/M4A with AAC or ALAC,
  WAV, AIFF; others, such as Opus, are not heard) and fingerprinted
  (`rusty-chromaprint`, Chromaprint's default algorithm, compressed and
  base64url-encoded as `fpcalc` does); fingerprint and length go to
  AcoustID (`/v2/lookup`, POST, Renome's application key, one request
  every 0.4 s; 429/503, a failed connection and a timeout retried 3
  times after 3 s; an invalid key, or a network error that lasts, stops
  the listening and is reported). Answers with a
  score ≥ 0.5 count; of their recordings, those linked by at least half
  as many submissions (`sources`) as the most linked one (a fingerprint
  is sometimes linked to other songs too), with the releases they are on.
  Each folder's songs (its disc folders' too) are then split among
  releases: again and again, the release preferred for the songs still
  without one takes those of them it has, preferring one with at least
  half of the folder's songs (and more than one), then an album of the
  artist's own (type Album, no secondary type), then more of the songs,
  then a track count nearest the folder's song count, then the earliest
  date, then the order AcoustID gave (so ties go the same way every
  time). The releases with all of an album's songs are its candidates.
  When the first release takes at least half of a folder's songs (of
  two or more), the folder is that album: its other songs join it when
  their tags fit one of its tracks (as in the matching above; a
  remaster is often heard as the original recording).
  The first is fetched from MusicBrainz and picked as above (every song
  matched, lengths agreeing); otherwise the album waits for review
  ("By sound" shows the release heard; songs nothing was heard for are
  "not recognised"). The recordings heard are kept for the session, so
  a release chosen in the review is matched by them too. A song without
  a title is named from its release once matched.
- A matched song takes the release's title (track), artist (the track's
  artist credit, else the release's), album artist (release credit),
  album, year (release date), track and disc numbers and totals, and,
  when the release has them, the original year (release group's first
  release), the first label and catalog number (not `[none]`) and the
  artist and album artist sort names (each credited artist's sort name
  with the join phrases; none when one lacks it), in place of its tags,
  before the album-wide agreement. Typographic
  punctuation is made plain (U+2010/2011 hyphens, curly quotes, `…`);
  dashes stay.
- "Save the album's cover": after renaming, `cover.jpg` (front, 1200 px)
  in the folder each album with a release ends up in, unless it has a
  `cover.*`, `folder.*` or `front.*` file (an HTTP 5xx from the archive
  is retried once after 3 s); a release without artwork (a cassette
  edition, say) takes its release group's front; albums whose songs keep their
  names count in the Rename button too, when their folder has no cover yet
  (asked of the disk, `cover_dirs`). Recorded in the history and
  deleted by undo.
- "Write the tags into the files" (off by default), after renaming:
  each song matched to a release (moved or not; those keeping their
  names count in the Rename button when their tags differ in title,
  artist, album artist, album, year, track/disc numbers and totals or
  release id) gets title, artist, album artist, album, date (release
  date, else year), track and disc numbers and totals, and the
  MusicBrainz release, recording, release track, release group, artist
  and album artist ids; and, from the release looked up once more in
  full (labels, ISRCs, genres, recording, work and artist relationships):
  original date (release group's first release), labels, catalog
  numbers (not `[none]`), barcode, country, release type (primary and
  secondary, lower case), medium format, disc subtitle, ISRCs, genres
  (the recording's, else the release's, else the group's; the 3 most
  voted, first letter upper case), language (not `zxx`) and script,
  the works' composers, lyricists and writers, the recording's
  producers, arrangers, mixers and engineers, artist and album artist
  sort names, work ids, and each credited artist (`ARTISTS`,
  `ALBUMARTISTS`). The main tag (ID3v2, written as v2.3, for MP3, where
  several values of a field (genres, composers…) are joined into one,
  `Rock; Pop`, since v2.3 readers take one value a frame (the credits'
  pairs excepted),
  and the fields v2.3 has no frame of its own for (script, writer, artist
  sort, disc subtitle) go to `TXXX` frames `SCRIPT`, `WRITER`,
  `ARTISTSORT`, `DISCSUBTITLE`, read back as those fields; the label,
  written to `TPUB`, is read back from it;
  Vorbis comments for FLAC, Ogg Vorbis and Opus; ilst for M4A, with
  writer and arranger as freeform `----:com.apple.iTunes:WRITER` and
  `ARRANGER` atoms; other
  formats are reported as not writable) is split with `lofty` into the
  fields it knows and the rest, edited and merged back, so other fields
  and frames stay. A song whose fields (and cover) are already those is
  not written. "Delete the other tags" (on by default with it) rebuilds
  the tag from those fields and lyrics, ReplayGain, BPM, genre,
  composer, lyricist and the encoder (plus a Vorbis `CUESHEET`), and
  drops an MP3's ID3v1 and APE tags; the items removed are kept for
  undo (Vorbis comments whole; ID3v2 user texts `TXXX` and MP4 text
  atoms lofty does not map, by description or identifier; other such
  ID3v2 frames and MP4 atoms, and ID3v1/APE, are not). A matched song with anything else in its
  tags counts as pending in the Rename button. "Include the cover in the files" (on by default with it)
  removes every embedded picture and adds the release's front (as for
  `cover.jpg`, else the folder's `cover`/`folder`/`front` image), at
  the size chosen: 1200 px by default, or the archive's 500 or 250 px
  thumbnails (a folder image goes in as it is). The
  file is copied beside itself (`.<name>.renome-tags`), written, synced
  and renamed over the original; the modification date changes (media
  servers rescan by it). The fields' old values and the pictures removed
  (kept once each, by content hash, in the entry's `<id>.files/`) go to
  the rename's history entry (or one of their own); undo puts them back
  before moving files back, and keeps the entry pending if that fails.
  Each song is noted next to the entry before it is written and, with
  what it held, after; if the program stops in between, the next start
  deletes the copies left beside songs and puts the songs written into
  the entry, to undo (an undo of tags is noted the same way, for its
  copies).
- "Show the missing tracks" (on by default; old saved rules get it on):
  for each album with a release (picked or chosen), the release's tracks
  no listed song is matched to become faded rows like the missing
  episodes (`05 · title`, `2-05` with more than one disc), in the
  release's order. The track list comes with the lookup and the choice:
  no extra request. The bottom bar counts them; clicking saves a CSV
  (album as `album artist - album (year)`, disc, track, title).
- Only the album and artist names (and release ids) are sent to
  MusicBrainz; to AcoustID, only each song's fingerprint and length.
- The files' audio is never changed, nor their tags unless "Write the
  tags" is on (cue sheets and playlists that list them are updated,
  see 4).

### 2.4 Books (Open Library, Google Books, BrasilAPI)

- Applies to EPUB, KEPUB, MOBI, AZW, AZW3, PRC, PDF, FB2 and DjVu files.
  Each file's details are read: EPUB package document (EPUB 2/3, series
  from the `calibre:series` meta or `belongs-to-collection`), MOBI/AZW3
  header and EXTH records, PDF Info dictionary and XMP, FB2 description.
  Without an ISBN in the details, the text of the first and last pages
  (EPUB spine, PDF pages) is searched for one. Titles programs put in by
  themselves (`Microsoft Word - …`, `Untitled`) and placeholder authors
  are dropped; several authors in one field (`A;B;translation by C`) are
  split and roles left out; accents are composed (NFC).
- The file name is read too: `Author - Title`, `Title - Author` (both
  readings are tried), `Title by Author`, a year in brackets, a number in
  a series, ISBNs; site marks (`z-lib`, `libgen`…) are ignored and `_`
  stands for `:`.
- Searches, in order, until one is a safe pick: each ISBN on Open
  Library; `title:(…) author:(surname)` for the readings; plain words;
  then the author's best-known books (the author found with Open
  Library's author search: all name words, the same name before longer
  ones, then most works; the surname alone as a fallback). An ISBN Open
  Library does not know is looked up on Google Books (with the user's
  key; `q=isbn:…`), else on BrasilAPI; that edition is joined
  with the work Open Library has when the search finds it (first year,
  original title, cover).
- Google Books (optional; the user's API key, set with
  `google_books_key`, checked with `isbn:9780306406157`; `gbBadKey` for
  `keyInvalid`/400/401/403, `gbBusy` for a used-up daily quota or 429):
  `GET /books/v1/volumes` with `maxResults=10`, `printType=books`,
  requests 0.5 s apart, answers kept only for the session (Google's
  terms bind keeping to its cache headers, which allow none; nothing on
  disk). Without an ISBN, a book Open Library does not pick by itself
  (or one Open Library fails on) is searched as `intitle:<words>
  inauthor:<surname>` with `langRestrict` (the titles' language), then
  without it, for the first two readings, until one fits. A volume
  becomes a candidate (`source: google`): title (NFC), authors, publisher,
  edition year from `publishedDate`, ISBN (the one searched, else its
  ISBN-13, or its ISBN-10 made 13), language (`pt-BR` → `pt`), pages,
  cover (`imageLinks.thumbnail`, HTTPS, no `edge=curl`, `zoom=1`; saved
  at `zoom=0`, about 800 px wide), description. By ISBN it is the edition
  (`exact`, safe when plausible). By title it fits when the title fits
  (≥ 0.95) and the author does (≥ 0.5; with no author to compare, an
  exact title); one that is the same book as an Open Library candidate
  (title fit ≥ 0.9, author ≥ 0.5 or unknown) only fills that one's empty
  publisher, pages, ISBN, cover and description; the others, if they fit
  or are by the author with a title fit of 0.6 or more, follow Open
  Library's, best first, one per book. With nothing from Open Library,
  the best one is a safe pick when it fits, has an author and no other
  is within 0.1 (but the same book). The cover saved is Open Library's,
  else Google's; Google's description goes into `metadata.opf` when the
  work has none. A Google candidate found by title has no `year` (first
  publication), and, not being `exact`, its edition year, publisher and
  ISBN are not used in the name or `metadata.opf`. BrasilAPI candidates
  carry `source: brasilapi`.
- Ranking: title fit (also without subtitles), author fit (the surname
  and the first name must match, initials too: `C. S. X` is not
  `X Carroll`), popularity, Open Library's order. Results by someone else
  that match only a word or two of the title are left out. An edition
  found by ISBN is picked by itself only when its title and author are
  plausibly the file's (an ISBN read from the text may be another
  book's) and it has an author.
- The title is the file's own when it names the same book (its author,
  series and number in the series removed; a subtitle only it has goes),
  else the catalogue's (article moved to the end put back in front, the
  author's name in front removed); notes of the edition's language
  (`Portuguese Edition`) are dropped. The file's spelling of the author
  is kept, and across the batch each author gets one spelling (initial
  of the first name and surname; the most used one, not in capitals).
- A first-publication year after the edition stored in the file is
  dropped. The publisher, ISBN and edition year come from the edition
  only when it was found by ISBN.
- Template keys (also in Portuguese and Spanish): `{author}`,
  `{authors}`, `{author sort}` (`Surname, Name`), `{initial}`, `{title}`,
  `{original title}`, `{year}`, `{edition year}`, `{publisher}`,
  `{isbn}`, `{series}`, `{volume}` (two digits), `{id}`. Optional
  folders (default `{author}/{title}`, folder names without a final dot)
  reuse folders already named after the book or its author. Optionally
  `cover.jpg` and an OPF `metadata.opf` (or `<name>.jpg`/`.opf` without
  folders) are saved; existing files are kept; undo deletes them.
- "Show the series' missing volumes" (on by default; old saved rules get
  it on): no catalogue lists a series' books, so, as with MangaDex, for
  each series (name compared without case, accents or punctuation) with
  a looked-up book whose details give the series and whose details or
  name give a number, the whole
  numbers from 1 to the highest one held that no book has become faded
  rows (`3`); fractional numbers (`2.5`) neither fill nor open a gap.
  The bottom bar counts them; clicking saves a CSV (series, volume).
- "Write the details inside the EPUB" (off by default): for looked-up
  EPUB/KEPUB files (zips by content) the package document named by
  `META-INF/container.xml` is edited in place, by byte ranges, leaving
  everything else as written: the main `dc:title`, the authors
  (`dc:creator` with no role or `aut`; extra ones removed with their
  `refines` metas, a renamed one loses its `file-as`), the first
  non-modification `dc:date` (kept when it starts with the year), the
  first `dc:publisher` and `dc:language` (kept when the main subtag is the
  same), an ISBN (kept when any identifier is it; else a non-unique ISBN
  identifier is rewritten, else `urn:isbn:` is added; the package's
  unique identifier is never touched), and the series (`calibre:series`
  metas and/or EPUB 3 `belongs-to-collection` + `group-position`,
  updated where present, else added: the first form always, both in EPUB
  3). New elements use the document's prefix for Dublin Core (declaring
  it when absent), go before `</metadata>` with the children's
  indentation. Nothing changes → the file is not written. EPUBs with
  `META-INF/rights.xml` or content encrypted by anything but font
  obfuscation (DRM) are refused (`epubDrm`). Writing works like the
  comics' (see 2.5), with the same history, recovery and undo.
- Only the searched title, author and ISBN are sent to Open Library and
  Google Books, and only the ISBN to BrasilAPI. Without the option above the book files are
  never changed.

### 2.5 Comics (ComicInfo.xml, Comic Vine)

- Applies to CBZ, CBR, CB7, CBT, PDF, EPUB, MOBI, AZW3 and AZW files (and
  to folders of pictures listed as items, when "Convert" turns them into
  CBZ: named from the folder's whole name, its `ComicInfo.xml` read). The `ComicInfo.xml`
  (else `MetronInfo.xml`, else ComicBookInfo; see "What is already
  there")
  inside a CBZ/EPUB (zip), CBR (RAR 4 and 5, read with the copy of
  libarchive built into the program, `crates/renome-archive`; solid
  archives over 300 MB skipped, RAR 5 taken as solid), CB7 (7z; solid
  archives over 300 MB skipped) or CBT (tar) is read: series, number, count, volume (a number, or the
  series' first year when it is one), title, year, publisher, writer,
  penciller, web (a Comic Vine issue link gives its id). The archive type
  is told by the first bytes, not the extension (a `.cbz` that is a 7z is
  read).
- The file name is read for what that lacks: release notes in brackets
  are dropped (a year among them is the issue's, `(of N)` the count),
  dots and underscores stand for spaces (not in `12.1`), then a chapter
  (`Ch.`, `Cap.`, `Chapter`, `c045`), a volume (`v2`, `Vol.`, `Volume`,
  `Tome`, `Tomo`, `T05`, `Book`, `Livro`), an issue number (`#12`, `#12.1`,
  `nº`, `No.`, `Issue`, `Edição`), else the last bare number that is not
  a year; what is before is the series, what follows a dash the title.
  The folder gives the series for a name without one, the series' year
  (`Series (2012)`) and a volume (`Vol. 3`).
- Letters written as their code between underscores (`M_244_nica`,
  codes 192–255 of letters) are put back first.
- Offline, every comic with a series is named from that (one without a
  number is a one-shot: series and year); others are left alone.
- Volumes may be Roman (`Vol. III`) or in brackets (`(Vol.03)`); words
  between the volume and the number belong to the series (`Vol. 2 Annual
  #01`); a bare number followed by a word is part of the name; `#2 of 6`
  gives the count; folder years may be runs (`(1988-1998)`).
- A `ComicInfo.xml` whose series is not the file name's (a translation
  that kept the original's) gives only its writer and artist.
- Series folders: a comic already in a folder named after its series
  (another volume's form, a `Vol. 1` folder under it, a near spelling,
  or the series of a special included) stays. The batch's comics are
  arranged together: one spelling per series (the most common; case,
  accents and one letter apart in one word of five or more letters are
  the same), in folders and names; a series of one file goes into the
  folder of the longest series in the batch its name starts with, or,
  when two such files share what comes before ` - `, into one named so;
  every other series gets its folder, even for one file.
- A name that starts with its number (`19 - Story`) takes that number;
  bare numbers after it are no issue. An option (off by default) takes
  the series from the file's folder (without brackets and year), and what
  the name had as its series becomes the title (none when it is the
  folder's series, or words of it: a shorter name for the same series).
- Ready-made templates: Renome's default (Komga takes each folder as a
  series) and Kavita's (`{series} v{volume} #{number} c{chapter}
  ({year})`, no title); both turn series folders on.
- Catalogues, chosen in the rule: the Grand Comics Database (default;
  HTTP Basic sign-in of the user's free account, from the options (checked
  there with Test; the first search without one in a session offers the
  options or going on without); a refused account offers the options;
  without one, anonymous access of about 30 requests an hour; requests
  2.5 s apart), Metron (the user's token, `Authorization: Bearer`;
  about 20 requests a minute and 5,000 a day; requests 3 s
  apart; `/api/series/?name=` with `year_began`, a series' page and its
  `issue_list/` pages, `/api/issue/?upc=` or `?isbn=` for barcodes; the
  year Metron shows after a name, `Name (2016)`, is dropped when it is the
  first year), Comic Vine (the user's key; about 200
  requests an hour; requests 1.1 s apart; a `ComicInfo.xml` link gives the
  series directly), and three catalogues of manga series (series only,
  the file's volume and chapter stay): MangaDex (no account; titled in the
  reader's language when it has one, else in English; fan works left
  out), AniList (no account; GraphQL, `type: MANGA`, novels left out, the
  year as `startDate_greater`/`startDate_lesser`; name, start year,
  volumes, country, format, cover, writer from the staff roles Story,
  Story & Art or Original Creator; requests 2 s apart, as its terms allow
  90 a minute, 30 while degraded) and MangaUpdates (no account;
  `POST /v1/series/search`, doujinshi, novels and artbooks left out;
  the series picked is asked again, `/v1/series/<id>`, for its authors
  (type Author), original publisher and the volumes in its status,
  `15 Volumes (Hiatus)`; requests 1 s apart; credited, as its terms ask).
  On AniList and MangaUpdates the series takes the file's name when it is
  one of its titles (synonyms, associated names), else the English title
  (AniList) or the main one. Answers are kept for the session, and on disk
  as the options say (see 7).
- On Metron, before the series: each file's barcode (`gtin` from its
  `ComicInfo.xml` GTIN or the GTIN/ISBN/UPC of its `MetronInfo.xml`,
  digits only), and, with "Read the barcode printed on the pages" (off;
  only shown for Metron), the barcode read in the pictures of the first
  page and the last two (natural order of names; a PDF gives none) with
  `rxing` (UPC-A, EAN-13, UPC-E; with the 2- or 5-digit add-on; pictures
  brought down to 2000 px on the long side and read in gray, the last page
  first; codes with a wrong check digit and EAN-8 left out), a few files
  at a time (up to 6 threads). A read code goes into the match's `gtin`
  (written into `ComicInfo.xml` with the rest). A UPC read as an EAN-13
  loses its leading 0; 10 digits or 13 starting 978/979 are an ISBN, 12
  to 17 a UPC. A code with exactly one issue gives its series (and the
  series' issues) and that issue, `auto`; the files found so are left out
  of the search by name.
- Files are grouped by the series typed, else by their own; loose
  chapters are not looked up (but on the manga catalogues). A search that finds
  nothing for `Series: Story` tries `Series`. Found series are ranked by
  name (equal, then starting alike, then similarity), the reader's
  language first; one with the name and the files' year (or the only one
  found) that can hold the highest number among the files is picked by
  itself (without a year a name alone does not tell runs apart), else a
  picker
  lists them, those that can hold the numbers first. The picker searches
  again with another name, narrowed by the year the series began and
  its publisher (part of the name, case and accents ignored; not on
  the manga catalogues; on Metron only when its list tells publishers; one of the names typed, split at `;`, `/` or `,`, holds the
  catalogue's whole words or is part of it: `Titan Comics` finds
  `Titan`); a progress bar shows while it searches. The GCD asks the
  name as typed and without its punctuation (`:`, `,`, `;`, `.`, `!`,
  `?`, dashes; hyphens stay); with a year, it asks the name's longest word
  from that year instead (`/series/name/<word>/year/<year>/`), which
  finds the name however punctuated; for a publisher, it asks the
  publishers of what was found, the most frequent first (12 at most,
  cached). GCD variant covers (`1 [Variant Cover]`, listed as issues of
  their own) are left out: one issue a number, the plain one when there
  is one, in the count, the issues and the missing ones;
  Comic Vine takes 50 results instead of 20 when narrowed and filters
  them; MangaDex takes the year in the request. The series' issues
  (GCD: its issue descriptors, their first number; Comic Vine: 100 a
  page) are matched by number (`012` = `12`, `12.10` = `12.1`), or by
  volume for a book without a number, or, for a one-shot, the series' only
  issue (or its first, unnumbered, printing); two issues with one number,
  or none, open the review. The issue's title, when there is one, and its
  year replace the file's; otherwise the file's title stays, or what its
  series had after the catalogue's (`Series: Story`).
- **Covers in the reviews** (comic issues and books): the file's own
  cover is shown (`convert::cover`: an EPUB's cover item — manifest
  `cover-image`, else the `cover` meta's item, else an image item named
  cover —, a PDF's first page drawn at about 1000 px, else the first page
  of the archive, folder or picture-only Kindle book) and fingerprinted
  like similar pictures. Each catalogue cover is downloaded (four at a
  time, kept by address for the session) and fingerprinted; 12 bits or
  fewer apart is the "same cover" (measured: one volume's file vs its
  catalogue covers in several languages 0–14 bits, other volumes 22 and
  up). Book candidates (the first 12) are compared at once; comic issues
  at once when up to 60 have a cover, otherwise with "Compare covers"
  (the first 300). Issues with the same cover go first, nearest first,
  with a badge; books get the badge in place. Nothing is picked by the
  cover alone.
- **Covers in "Choose the series"**: the cover of the group's first file
  with a number or volume is shown and compared, one series after the
  other, with the first 9 candidates (`series_covers`): Metron and Comic
  Vine give the cover of the issue `find_issue` matches in the series'
  issue list (series of over 200 issues are skipped); MangaDex gives the
  covers of the file's volume, every language (`/cover?manga[]=`, once a
  session per manga), the nearest counting; AniList and MangaUpdates
  compare the series' own cover, only when the file is volume/number 1.
  Once all are compared, the matching series move first (keys 1–9 do not
  move while comparing), with the badge. The Grand Comics Database gives
  no covers: its series list has none and its picture server answers
  programs with a browser check (403), so it is not asked; the picker and
  the issue review say so. A catalogue picture the page cannot load by
  its address (MangaDex refuses other sites') is read by the program
  (`remote_image`, https only, images up to 4 MB) and shown as a data
  address. A series without a cover of its own (Metron's and Comic Vine's
  series lists have none) shows the issue cover it was compared with.
  Clicking a cover — the file's, a series', an issue's in the review —
  shows it large beside the file's (`CoverZoom`, MangaDex's 512 px copy);
  Esc or a click closes it without choosing.
- Template keys (also in Portuguese and Spanish): `{series}`,
  `{series year}`, `{number}` (three digits), `{volume}` (two),
  `{chapter}` (three), `{title}`, `{year}`, `{count}`, `{publisher}`,
  `{writer}`, `{artist}`, `{id}`. An empty key drops the label stuck to
  it (`v`, `#`, `c`) or ending in a dot (`Vol. `). Optional series
  folders (default `{series} ({series year})`) reuse folders already
  named after the series.
- "Show the missing issues" (on by default; old saved rules get it on):
  for each series (GCD or Comic Vine) with a listed file matched to an
  issue, the issues no listed file holds become faded rows, like the
  missing episodes (`#7 · title`, cover date), leaving out cover dates
  still to come. Issues are put in order by number (non-numbers last,
  then as the catalogue lists them); each row sits before the file with
  the nearest later issue, else after the nearest earlier one. The
  bottom bar counts them; clicking saves a CSV (series with start year,
  number, title, cover date) in the series' folder. With missing items
  of more than one kind (episodes, issues, tracks, movies, volumes), one
  count ("N missing") and a CSV with what they share: group (show,
  series, album, collection), code, title, date.
  The manga catalogues have no list of a manga's volumes (MangaDex only
  uploaded chapters, often taken down for licensed manga): for their series the rows are the
  numbers missing between the files, from 1 to the highest (by volume
  when every file has one, else by issue number, else by chapter;
  `v03`, `#3`, `c003`), without title or date.
- Only the series name and the series' and issues' ids go to the
  catalogues; the GCD sign-in only to comics.org.
  Files are never changed.

- "Write the details into the file (ComicInfo.xml)" (off by default):
  after renaming, each CBZ (a zip by content; other comic formats and
  broken zips are refused, `comicUnsupported` / `archiveBroken`) matched
  to a catalogue issue (or series, on a manga catalogue) gets a `ComicInfo.xml`:
  Title, Series, Number (else the chapter), Count, Volume, Year, Month,
  Day, Writer, Penciller, Publisher, Web (the issue's page: GCD
  `comics.org/issue/<id>/` or `/series/<id>/`, Comic Vine `4000-<id>` /
  `4050-<id>`, MangaDex `title/<id>`, AniList `anilist.co/manga/<id>`,
  MangaUpdates `series/<id in base 36>`; none for Metron, whose pages are
  named, not numbered), Manga `Yes` (the manga catalogues) and the
  match's extra fields (story arc, imprint, format, genre, inker,
  colorist, letterer, cover artist, editor, translator, age rating,
  language, alternate series/number, GTIN); empty values are not
  written. An existing `ComicInfo.xml` (the root one first, else one in a
  folder, any case) keeps every other element as it was (summary, page
  list); the changed and added ones join them, all in the schema's
  order. The same values already there → nothing is written. Unchanged
  rows count (in the Rename button) when their `ComicInfo.xml` (or other
  details read, see "What is already there") says something else
  (`retag`). A `MetronInfo.xml` is never updated.
- "Convert CBR, CB7, CBT, PDF, EPUB, MOBI/AZW3 and picture folders to CBZ"
  (off by default), with "The
  original file": keep beside (default) or delete; "PDF pages that are
  not just a picture": draw them (default) or leave such PDFs as they are
  (`drawPages`; then only PDFs with no page to draw are offered, and one
  found to need drawing when converting fails with `pdfDrawn`); "Size of
  drawn pages": 1600, 2400 (default) or 3200 (`pageSize`, the long side
  of a drawn page with no picture). A converted PDF's row has `drawn`
  (pages that would be drawn, estimated without the pictures' data;
  `pdf_check`), shown as a tag; the conversion's notice says how many
  were drawn (`drawn` in `write_inside`'s report). In the preview, a comic
  file (`.cbz`/`.cbr`/`.cb7`/`.cbt`) whose content is a RAR, 7z or tar, or
  a `.pdf`, or an EPUB, MOBI, AZW3 or AZW of pictures only, or a folder
  of pictures only (its name gets `.cbz`), gets the extension `cbz`
  (`convert` on the row; a
  `.cbz` that is a RAR keeps its name and counts in the Rename button as
  unchanged). A PDF (checked once per file: it opens
  without a password, with lopdf or else hayro; `pdf_of_pictures`) is
  converted page by page, each in the way that keeps the most
  (`pdf_files`). A page's content is followed through form XObjects
  (their `Matrix`, their own resources else the page's, up to 8 deep)
  with `q`/`Q`, `cm`, fill color and text rendering mode tracked; it draws
  pictures (image XObjects, and inline images, read by lopdf only when
  not compressed) and shapes (path painting, `sh`, text in a mode other
  than 3 or 7). A page drawing nothing becomes a white 1-bit PNG the size
  most other pages' pictures have (as shown). The page's picture is taken
  out when the biggest picture (by pixels) is the last thing drawn
  (itself again is fine), with nothing before it or, before it,
  decoration only (anything) while it covers 85% of the page's crop box
  (else media box); no visible annotation with an appearance (not Link
  or Popup, not hidden); its orientation (matrix and inherited `Rotate`)
  a quarter turn and/or mirror; an image mask only painted black on
  nothing; no `Mask` or `SMaskInData`; a soft mask only when it hides
  nothing (every value the highest); `Decode` only when it changes
  nothing, or `[1 0]` for one value (inverted). Then: a JPEG
  (`DCTDecode` after plain layers — Flate, LZW, ASCII85, ASCIIHex,
  RunLength, Brotli —, gray or RGB, upright) is copied as `NNN.jpg`; a
  CMYK JPEG is decoded and made RGB (approximate colors, `1 - c` times
  `1 - k`) and written as a JPEG (quality 95); JPEG 2000 (hayro-jpeg2000;
  transparency laid on white, CMYK made RGB), JBIG2 (hayro-jbig2, with
  `JBIG2Globals`) and fax Group 4 (`fax`; others are drawn) are decoded
  and written as PNG; raw pixels (plain layers only; gray at 1, 2, 4, 8
  or 16 bits, RGB at 8 or 16, CMYK at 8 made RGB, indexed with a gray,
  RGB, CMYK or one-ink palette at 1, 2, 4 or 8, one ink — Separation or
  a one-colorant DeviceN — as gray the other way round) go as they are
  into a lossless `NNN.png` (indexed ones with the palette). Not upright,
  a JPEG is turned without decoding by libjpeg-turbo (built in by
  `turbojpeg-sys`, no system library; "perfect" transforms only, i.e.
  when the edges that move are whole blocks), else the pixels are turned
  back into a PNG (a JPEG decoded first, values below 8 bits widened to
  8). Anything else, or a picture that fails to
  decode, and a PDF lopdf cannot read: the page is drawn with hayro, on
  white, at the scale of its sharpest picture (pixels over the points it
  is shown at) or `pageSize` pixels on the long side, at most 6000 (and
  16000 in any case), several pages at once (one hayro reader per worker,
  up to 8, sharing the file's bytes, which are read once; batches of 16
  pages kept in order), as a PNG (gray when every pixel is within 8 of gray)
  unless a JPEG (quality 92) is under two thirds of it; `Done` counts the
  drawn pages. Pages are numbered `001`… in page order (more digits past
  999); with "Write the details" on, the PDF's own title and author (its
  Info) go into the `ComicInfo.xml` as Title and Writer when the values
  have none (an EPUB's or Kindle book's title and authors too, read as
  for books).
- An EPUB (a zip whose first entry `mimetype` says
  `application/epub+zip`, or holding `META-INF/container.xml`;
  `pages::epub`) is a comic when every spine item is a picture item
  (not SVG) or a page whose markup, without its head, scripts, styles and
  comments, shows at most 20 letters (entities count one; spaces not) and
  one picture (`img src`, `image href`/`xlink:href`, links resolved and
  `%`-decoded; a page with none and no letters is blank and left out);
  at least one picture; pictures JPEG, PNG, GIF, WebP, AVIF or JPEG XL.
  A cover item (`properties="cover-image"`, else `<meta name="cover">`)
  no page shows goes first. A page or picture listed in
  `META-INF/encryption.xml` → `archiveLocked`. Pictures are copied as
  `NNN.<ext>` (`.jpeg` → `.jpg`) with their zip dates; a `ComicInfo.xml`
  in the EPUB goes along.
- A Kindle book (`BOOKMOBI` at byte 60; `pages::kindle`): its first
  record's header gives the text records, compression (none or PalmDOC;
  the dictionary one → `comicUnsupported`), encryption (any →
  `archiveLocked`), first picture record, trailing-entry flags, version
  (8: KF8) and, from EXTH, the KF8 part's record (121) and the cover
  (201). The KF8 part, when there is one, is tried first, then the old
  one. The text (trailing entries stripped, decompressed, at most 64 MB)
  must show at most 20 letters per picture, as for EPUB; pictures are
  taken in the text's order from `kindle:embed:XXXX` (base 32) in KF8,
  `hirecindex`, else `recindex`, else `lorecindex` in the old kind, as
  records counted from the first picture (in a KF8 part after an old one,
  from the file's start or the part's: the count that lands every one on
  a picture); an unused cover first. A record is a picture when JPEG, PNG
  or GIF (inside a `CRES` record past its 12-byte head); copied as
  `NNN.<ext>`. The original kept beside is `.azw3` when the file holds a
  KF8 part only, else `.mobi`.
- A folder of pictures (`convert::folder_pages`): files JPEG, PNG, GIF,
  WebP, AVIF or JPEG XL and `ComicInfo.xml`, in subfolders up to 3 deep,
  hidden files, `Thumbs.db`, `desktop.ini` and `ehthumbs.db` left out;
  anything else (another file, a link, a deeper folder) → not a comic;
  at least one picture. Entries keep their paths under the folder, in
  natural order (digit runs by value, case ignored), with their dates.
  The rename names the folder `Name.cbz`; converting it writes the zip,
  renames the folder to `Name` beside and the zip to `Name.cbz` (no
  permissions copied); undo removes the CBZ before putting the folder
  back; with the folder in the trash, the rename that made it gets `.cbz`
  added to its old name (`Converted::folder`). After converting, the
  list's row is read again as a file. After renaming,
  each is converted (`convert::convert`): every file entry, in the
  archive's order, is written into `.<name>.renome-tags` beside it
  (pictures stored, other files deflated, dates kept; folders and links
  left out; names with `\` turned into `/`, leading `/`, `.` and `..`
  parts dropped; a name repeated keeps the first), with the
  `ComicInfo.xml` updated by the values above when "Write the details"
  is on and the issue matched (added last when there is none), else
  copied as it is. Synced and read back (as many entries as written);
  then the original is renamed beside to the comic's name with its own
  extension (`Name.cbr`, `Name.cb7`, `Name.cbt`) and the zip renamed to
  the comic's name. Delete then sends the original to the system's trash
  (`trash` crate; in a Flatpak, `FLATPAK_ID`, `gio trash`, which goes
  through the desktop's trash portal); when the trash fails, it stays
  beside (`trashFailed: …` in the error list).
  Errors, the comic as it was: `archiveLocked` (a password),
  `archiveBroken: …` (damaged, or a RAR libarchive cannot decode),
  `convertTaken` (the original's name beside exists), `comicUnsupported`
  (not named `.cbz`, or a zip), `pdfLocked`. A RAR split in
  parts is read as a broken one. A comic the rename named `.cbz` that
  could not be converted takes its real extension back
  (`History::retarget`: the rename in the entry now ends there, or goes
  when that is its old name; an entry left empty goes). Recorded like written details (a backup with `converted`): undo
  renames a kept original back over the CBZ (`originalGone` when it is not
  there), before the renames take it back to its old name; with the
  original in the trash, the CBZ stays and the rename that made it gets
  `.cbz` as its old extension, so undo gives back the old name as a CBZ.
- Writing (comics and EPUBs): the archive is written again as
  `.<name>.renome-tags` beside it, every other entry raw-copied (no
  recompression; order, compression and the zip comment kept), the
  changed entry deflated in its place (new ones last), synced, given the
  original's permissions, then renamed over the file. The old entry's
  text is kept in the history entry's folder; undo writes it back the same
  way (or removes an added entry). Each file is noted in the entry's
  `.tagging` note like songs (see 2.3): a stop halfway removes the copy
  and keeps what was written for undo.

### 2.6 Order of operations

- Preview: each file's name goes through the rules from top to bottom
  (lookup rules use their choices, made on the name the rules above give);
  then the folders: shared roots and folder levels unified, a show's, a
  movie's, a comic series', a book's and (with music folders) an album's
  own folder found, collections kept, songs' companion files following
  them; then the checks: names, repeated targets, characters the target
  disk refuses, targets that exist; last, the old folders that keep
  files.
- Rename: the renames (written down first, two phases, deepest first),
  then the cue sheets and playlists that cite them, then the folders left
  empty, then the history entry; after it, each recorded in that entry,
  the movies' `.nfo` and images, the albums' covers, the playlists, the
  songs' tags, the books' cover and `metadata.opf`, the comics converted
  to CBZ and the details inside comics and books (one pass), the shows'
  `.nfo` and images.

## 3. Preview

- Table: original name → new name, with a status per row:
  unchanged, ok, conflict (with the reason). The Folder column shows the
  file's folder (home as `~`), or, in blue, `→ ` and the whole folder it
  moves to, written the same way.
- With a lookup rule on (movie, TV show, music, book, comic), a file none
  of them handles (movie: videos, subtitles, folders; TV: videos and
  subtitles; music: songs and their lyrics, cue sheets, playlists and
  covers; book: books; comic: comics) and that no other rule renames gets
  a grey "outside the rule" tag before its name, with why in a tooltip;
  the row is otherwise normal and can be named by hand.
- The new name can be edited by hand in the table (overrides the rules' name for that row; the folder the rules move it into still applies).
- Checks: empty name (including a name the rules emptied, leaving only
  the extension, which would make a hidden file), `/` or null character,
  longer than 255 bytes (on Windows, 255 UTF-16 units),
  two files with the same target, target already exists outside the batch.
  On Windows, `\ : * ? " < > |` and a trailing dot or space are errors;
  so they are on Linux when the target is on a disk that ignores case
  (FAT, exFAT, NTFS mounted so: formatted for Windows); elsewhere they
  are a non-blocking warning. Names that differ only in case count as
  the same name where the file system ignores case (see 4).
- Counters: total, changed, conflicts.
- While a long task runs (a search, reading added files, renaming and
  writing sidecar files, undoing), a box over the list says what it is
  doing, with a progress bar (moving when the total is not known yet);
  the list, the rules and the list's buttons and shortcuts wait. An
  online search (movie, TV show, music, book, comic) has a Stop button
  there: the request under way finishes, what was found is kept, the
  files not reached are left as they were (searching again goes on with
  them), and a note says the search was stopped.

### Identical copies

- After the list changes, files with identical content are looked for in
  the background (a newer search replaces an older one). Nothing is
  deleted or changed.
- Stages, each ruling out most candidates before the next: same size; same
  physical file (hard links, without reading); a hash of the first and
  last 4 KB; a hash of the whole file (xxHash3, 128 bits) only for what is
  left. Hashes are cached per path while the file's size and modification
  time stay the same. Empty files, folders and symlinks are left out.
- Rows with a copy show a tag with the group number (same number =
  identical; hover: which files), and selecting a file highlights its
  copies. The footer shows how many files have a copy and the progress
  while whole files are read;
  clicking it shows only those files, grouped.
- A "same name" conflict between identical files says so.
- **Repeated songs**: after the identical files, the same search finds
  songs that are the same music in different files (an MP3 and a FLAC,
  an album track and its compilation copy), by their tags only: same
  title and main artist (artist, else album artist, without `feat.`/`ft.`/
  `featuring`/`with` and what follows), compared without accents, case,
  punctuation, bracketed parts or a trailing `- … Remaster(ed)/Remix/
  Mono/Stereo …`; and lengths within 3 s of the group's shortest. Songs
  without a title, artist or length are left out; a group made only of
  identical copies of one file is left out (they are copies already).
  The tags are read on a copy of the list, without holding it, and kept
  for the list afterwards. Rows get an outlined tag (♪ and the group
  number; hover: which files), selecting one highlights the others, and
  the footer shows how many songs are repeated; clicking it shows only
  those, grouped. Nothing is changed.
- **Similar pictures**: then, pictures that look alike (the same photo
  resized, recompressed, in another format or lightly edited; shots taken
  a moment apart) are found by a 64-bit fingerprint of their shapes: the
  picture shrunk to 32×32 greys (cell averages; JPEGs decoded at 1/8,
  1/4 or 1/2 scale when that still gives 32 pixels a side), turned by its
  EXIF orientation, a discrete cosine transform of which the 8×8 lowest
  frequencies are kept, one bit per frequency but the average for being
  above their median. Pictures whose fingerprints differ in 8 bits or
  fewer are joined (transitively). JPEG, PNG, WebP, GIF (first frame),
  BMP, TIFF; HEIC and camera RAW (DNG, CR2, NEF, NRW, ARW, SRF, SR2, ORF,
  RW2, PEF) by the JPEG thumbnail in their EXIF data, when there is one.
  Files over 256 MB and pictures of one flat colour are left out; a group
  made only of identical copies of one file is left out. Read in parallel
  (half the processors, at most 6) after the copies and songs, stopped by
  a newer search; the footer shows the progress while new pictures are
  read. Fingerprints are kept by path, size and modification time in the
  cache folder (`pictures`, one line per picture; dropped when unused for
  90 days), apart from the catalogues' answers and their cache setting.
  Rows get an outlined tag (picture icon and group number; hover: which
  files), selecting one highlights the others, and the footer shows how
  many pictures look alike; clicking it shows only those, grouped.
  Nothing is changed.
- **Compare window**: with the similar-pictures or identical-copies filter
  on, the footer shows "Compare"; double-clicking a row's group tag opens
  it at that group. One group at a time, cards side by side: a smaller
  copy (JPEG, longest side 480 px, turned by EXIF orientation; JPEGs
  decoded at the smallest 1/8–1/2 scale that still gives that size; raw
  and HEIC by their EXIF thumbnail), name, folder, width × height as
  shown, size, date taken (EXIF) or else modified. Badges: "largest"
  (most pixels, then largest file; only when the group differs) and
  "oldest" (date taken, else modified; only when strictly oldest). A
  card opens big (1800 px copy); arrows go through the group's cards
  (big) or the groups; Esc goes back or closes (not while a question is
  open). Files are ticked for the trash across groups; "Keep only the
  largest" ticks all but the largest of the group. "Move to the Trash"
  asks first (warning when every file of a group is ticked), sends them
  to the system's trash (as converted originals are, Flatpak included),
  takes them out of the list and of the Ctrl+Z states, and searches for
  copies again; failures are listed (the file stays). Not recorded in
  the History.

- "Export list" (footer) saves the list as shown to a CSV file: folder,
  current name, new name, folder it moves into, status, problem; UTF-8
  with a byte order mark, `;` between fields in Portuguese and Spanish
  (`,` in English).

## 4. Apply

- Confirmation before renaming. Rows with conflicts are not applied.
- Safe two-phase renaming (source → temporary → target), which allows
  swaps (a↔b), cycles and case-only changes on case-insensitive file systems.
- Renames run by depth: deepest first, so a folder is renamed after the
  files inside it (undo runs the other way).
- Failure midway: whatever was done for that file is reverted.
- The target is taken only if free, checked and renamed in one step:
  `renameat2` with `RENAME_NOREPLACE` on Linux, `renamex_np` with
  `RENAME_EXCL` on macOS, `MoveFileExW` without replacing on Windows;
  where the file system cannot (or a Windows path is 260 characters or
  longer), a check right before.
- What is about to happen (each rename and its temporary name) is written
  to the history folder before starting and removed once the history
  entry is written. If the program stops in between, the next start puts
  files still under a temporary name back under their old one, records
  those already renamed as an entry that can be undone (with the folders
  they went into, removed by undo when empty), and lists the temporaries
  whose old name is taken. An undo is written down the same way; one
  stopped halfway takes what it had already undone out of its entry.
  History files are written through a copy that takes their place and
  synced to the disk, so a power cut leaves neither an empty entry nor a
  lost note.
- A move to another disk (the rename says so) is a copy: the file (a
  folder with everything in it, links as links on Linux and macOS) is
  copied to a passing name beside its target (`.renome-<pid>-<n>.part`,
  also noted before starting), with its modification date, synced to the
  disk and checked for size, renamed to its target only if that is free
  (as above), and only then is the original deleted; a file whose
  original cannot be deleted loses its copy again, a folder keeps it
  (the move counts) and what is left of the original gets its old name
  back and is reported ("partly left").
- **Send to another folder** (toolbar, next to Rename; "In the same
  folder", "Move to another folder…", "Copy to another folder…"; not
  saved, and back to "In the same folder" after each send, since the list
  then holds the files sent): the folders the rules make go inside the chosen
  folder instead of their root (no root is looked for, old folders are
  not taken for new ones nor renamed for leftovers); every other visible
  file (also one keeping its name) keeps the subfolders below the folder
  it was added with (`Photos/2024/x.jpg` → `<target>/2024/x.jpg`; a file
  or folder added by itself goes straight in), after the songs' companion
  files have followed their songs. Copies: made as for another disk,
  without the first phase (the originals stay, so a name another file of
  the batch has stays taken); metadata files, covers, playlists and tags
  go to the copies, and only lists copied along are rewritten (the
  folders the originals are in are not looked at). Each copy is
  recorded as it was left (size and date; for a folder its files' count,
  total size and latest date), updated when its tags are written; undo
  deletes the copies still so and keeps the others (out of the entry,
  listed), with their lists and tags untouched. Copies and moves to
  other disks report their progress (bytes, every 200 ms at most) and
  can be stopped between files: the files not reached keep their names.
  The next start after a stop halfway deletes passing copies, records
  whole copies in an entry, and, for a move to another disk whose
  original was still under its temporary name, deletes its copy when
  identical (size and date) and gives the original its name back.
- Renaming, undoing, adding folders and the preview run away from the
  window's thread; a preview that comes back after a newer one is dropped.
- Names are compared without case where the file system ignores it:
  always on Windows and macOS, and on Linux where looking an existing name
  up in the other case of its ASCII letters finds it (exFAT, FAT, NTFS
  mounted so; FAT mounted with `iocharset=ascii` tells `ç` from `Ç`, so
  only ASCII letters are swapped).
- When the history entry cannot be written, nothing more (metadata files,
  covers, playlists, tags) is written for that rename.
- Report at the end: successes and errors.
- Several files heading for one name are all blocked, except identical
  copies (by content, as found in the background): the first in the list
  goes ahead and the others stay, marked "copy".
- Folders files were moved out of are deleted when left empty (up to
  the folder the move happened within; for files sent to another folder,
  only below the folder they were added with, which stays); undo makes
  them again.
- Cue sheets and playlists (`.cue`, `.m3u`, `.m3u8`) in the folders
  renamed files left, or renamed/moved themselves, are rewritten so their
  references (`FILE "…"` lines in cue sheets, non-comment lines in
  playlists; relative or absolute, `/` or `\`, URLs left alone) point to
  the new names; references to files that stayed are adjusted when the
  list moved. A reference written with `\` keeps it; others get `/`,
  also on Windows (which takes it), so lists work on other systems.
  UTF-8 or Latin-1 is kept (a list whose new names Latin-1
  cannot hold is left alone), as are line endings. A cue sheet none of
  whose `FILE`s exist (a whole-disc image later split) is rebuilt: each
  `TRACK n` gets `FILE "<song>"` (`MP3` for .mp3, else `WAVE`) with the
  song in its folder tagged track `n` (and the cue's `REM DISCNUMBER`, if
  any) and `INDEX 01 00:00:00`; other `INDEX`, `PREGAP` and `POSTGAP`
  lines are dropped, the rest kept. When a track has no song or several,
  the cue is deleted. A playlist none of whose relative entries exist is
  deleted (with absolute entries, a missing drive is likelier, so it is
  only updated). The original content is saved next to the history entry
  (`<id>.files/`) before writing or deleting, and undo restores it
  (making its folder again if needed) before moving files back. Emptied
  folders are removed after the lists are handled.
- Each applied operation is recorded in a history (in the user's data
  folder, e.g. `~/.local/share/renome/history/`), allowing **undo** even
  after closing the program. If part of an undo fails (file moved or
  deleted), the entry shows the failure and can be retried once the files
  are back in place. Entries can be deleted from the history, one by one
  or all at once ("Clear history", confirmed); the files stay as they
  are. Entry ids come from the time (milliseconds) and the process,
  with `-2`, `-3`… if one is taken.
- When a rename (or undo) ends without errors, a short note in the
  bottom right corner says what was done and goes away after 4 s (or
  on a click); with errors, the error list is shown instead.
- Confirmations and error reports use the app's own dialogs; the confirm
  button is focused by default (Enter confirms), except in destructive
  dialogs (e.g. deleting a history entry), where Cancel is. Errors name the
  file and the reason (e.g. "file not found").

## 5. Keyboard shortcuts

| Keys | Action |
|---|---|
| Ctrl+Enter | Rename (also while typing in a rule) |
| Ctrl+Z | Undo the last change to the list: add, remove, clear, move or hand-edit a name (outside text fields; up to 100 steps; renames are undone in the History) |
| Ctrl+O | Add files |
| F2 | Edit the new name of the selected file |
| Delete | Remove the selected files from the list |
| Ctrl+A | Select all |
| F1 | Open the help |
| Esc | Close the open dialog or menu |

On macOS, ⌘ replaces Ctrl. The list is also shown in Options.

## 6. Help

A "?" button (or F1) opens a window with three tabs:

- **Manual**: a detailed guide in the interface language, with an index:
  getting started, file list, how rules work, each rule with examples,
  preview and conflicts, renaming and undoing, recipes, shortcuts, options.
- **About**: icon, version, description, author, credits (developed with
  the help of Claude, by Anthropic), license (a link to the Licenses tab)
  a link to the repository and an optional tip link (Ko-fi), which the
  status bar also shows at its right end, small. Nothing in the program
  depends on tipping.
- **Licenses**: Renome's MIT license in full, and the open source
  components built into the app, with a filter by name or license; each
  one opens to its link and license texts. The list is made when building
  (`scripts/licenses.js`, a Vite plugin): the Rust crates linked into the
  program (normal dependencies in `cargo metadata` for the target being
  built) and the npm packages whose code is in the interface bundle, with
  the license and notice files from each package. For "MIT OR …" only the
  MIT text is kept; packages without files get the standard text of their
  license from another package; identical texts are stored once
  (`licenses.json`, loaded when the tab opens).

## 7. Options

Saved in the user's config folder (e.g. `~/.config/renome/settings.json`).

The secrets (TMDB, Google Books, Comic Vine, Discogs and Metron keys, GCD email and password)
are not in that file: they go, as one JSON entry (service
`io.github.rafaellgc.Renome`, account `credentials`), to the system's
password store (Secret Service on Linux, Credential Manager on Windows,
Keychain on macOS; the `keyring` crate), encrypted there. Unchanged
secrets are not written again; when all are empty the entry is removed.
Secrets found in the file (from earlier versions) move to the store when
the settings load. When the store cannot be reached, the secrets stay in
the file, unencrypted, and the options say so. The store may wait to be
unlocked (the system asks for its password), so the settings load
without it (`load_settings`) and the secrets apart (`load_secrets`); until
they come, the options show them waiting and their fields are off.
`load_secrets` tells how the store answered: `stored`, `refused` (it
exists but did not answer: denied, as macOS asks after each new version,
locked, or failing) or `missing` (no store). A store not read in this
session is never written to nor its entry removed, so a refusal cannot
lose what it holds; the options then say so, with a "Try again" that
reads it once more, and keys typed meanwhile go to the file (newest:
they win over the store's when it next answers). Secrets are saved
apart too (`save_secrets`), and only when they changed from what was
loaded or last saved; `save_settings` keeps the file's secret fields as
they are. On macOS, an app not signed by Apple
may be asked about the Keychain again with each version.

- Language: English, Portuguese, Spanish. On first run, the system's.
- Theme: light, dark or system (default).
- Remember the rules between sessions (on).
- Include hidden files when adding folders (off).
- Ask for confirmation before renaming (on).
- Turn the rules off after renaming (on).
- Warn about characters Windows does not accept (on).
- Online searches: keep the answers for 1 day, 7 days (default), 30
  days, 6 months, or not at all (`cacheDays`), with the space they take
  and Clear. The answers (raw response bodies of TMDB, MusicBrainz,
  AcoustID, Discogs, Open Library, the GCD, Comic Vine, Metron, MangaDex,
  AniList and MangaUpdates; not
  images, not failures, never the key and account checks) are kept one
  file each in the user's cache folder (`~/.cache/renome/answers/<service>/`),
  named by a hash of the question (path, query and language, without keys
  or passwords), written whole (`.part`, then renamed), and their age is
  the file's date. TMDB's are kept at most 6 months (its terms), AniList's
  at most 7 days (its terms forbid using it as a store of its data),
  Discogs' at most 6 hours, whatever the choice. Expired ones are deleted when
  read and at start, in the background.
- Services: TMDB key and title language, Google Books key, Comic Vine key, Metron token
  (checked against `/api/series_type/`), GCD account (email and
  password), Discogs token; each with a Test button. A line
  says whether they are in the system's password store or in the file.
