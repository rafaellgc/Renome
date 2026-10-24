# Renome — functional specification

Batch file renamer for Linux, Windows and macOS (Rust + Tauri + Svelte).

This document describes **what** the program does, not **how**.

## 1. File list

- "Add" button with three options: files (multiple selection), folders
  (their contents, with subfolders) or folders as items (to rename the
  folders themselves); drag and drop adds files and folder contents.
- A folder's name has no extension: extension rules leave it alone and
  dots in it are kept as text. Folders and files inside them can be renamed
  in the same batch.
- Hidden files (`.file`, `.git/`) are left out unless enabled in Options.
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
  the current name and what was added to the new one are highlighted.
- Sort by clicking a header (click again to reverse): **new name**
  (default; the list re-sorts as the rules change), current name (natural
  order, `2 < 10`), folder, size, date.
- Move items up/down (manual order; unavailable when sorting by new name),
  remove selected, clear all.
- Numbering follows the list order (in "new name" mode, the current name order).

## 2. Rules (pipeline)

Transformations are **rules** stacked and applied in sequence to the name.
Each rule can be turned on/off, reordered and removed. The preview updates
live. The rule set can be saved and loaded as a *preset* (JSON).

By default rules act on the name without the extension; text rules can
optionally act only on the extension or on the full name.

| Rule | Parameters |
|---|---|
| Replace text | find, replace with, case-sensitive, first occurrence only |
| Regular expression | pattern, replacement (`$1`, `${name}`), case-sensitive, first occurrence only |
| Rearrange parts | separator; new order using part numbers and text (`1 - 3 - 2`); the parts of a sample file are shown and can be clicked to build the order. Parts are trimmed; parts left out are removed; names with fewer parts are unchanged |
| Case | lowercase; UPPERCASE; Sentence case; Title Case; Title Case except connectives (of, the, de, da…; a part after ` - `, `:`, `(`, `.` starts capitalized). Title Case keeps Roman numerals II–XXXIX in capitals (lowercase `vi`/`xi` are words) and leaves words with inner capitals (`iPhone`, `McDonald`) as they are |
| Insert | text, current date, file modification date, date the photo was taken (EXIF; optionally the modification date when missing, otherwise the name is unchanged) or the name of the folder the file is in; configurable date format; at the start, at the end or at a position (from the start or the end) |
| Number | template with `{n}` (number), `{name}` (current name) and `{folder}` (folder name); start, step, minimum digits (zero padding); optionally restart in each folder |
| Remove | spaces; repeated/leading/trailing spaces; accents; digits; custom characters |
| Remove range | from: position N or first occurrence of a text; to: end, position N or occurrence of a text; include the delimiters or not |
| Character at position | position N; optionally only if the current character is X (case-insensitive unless asked); replace with Y or delete |
| Extension | set a new one; lowercase/uppercase; remove |
| Movie (TMDB) | template with `{title}`, `{original}`, `{year}`, `{id}` (default `{title} ({year}) [tmdbid-{id}]`); what `:` becomes (default ` - `); a folder for each movie; save `.nfo` and poster (optionally fanart and logo) next to renamed videos. See 2.1 |
| TV show (TMDB) | the show (name, TMDB id or link; empty: from the files); template with `{show}`, `{original}`, `{year}`, `{season}`, `{episode}`, `{title}`, `{id}` (default `{show} ({year}) - S{season}E{episode} - {title}`); what `:` becomes; show and season folders (show folder template, default `{show} ({year}) [tmdbid-{id}]`); save `.nfo` files and images (optionally fanart and logo). See 2.2 |
| Music (tags) | template with `{track}`, `{title}`, `{artist}`, `{album artist}`, `{album}`, `{year}`, `{disc}`, `{genre}`, `{composer}` (default `{track} - {title}`); what `:` becomes; disc number in the track on multi-disc albums; fill in from the file name; artist and album folders (default `{album artist}/{album} ({year})`). See 2.3 |

Positions count **visible characters** (Unicode graphemes), starting at 1.

### 2.1 Movies (TMDB)

- Needs a TMDB API key (v3 key or v4 read token) in Options, plus the
  language of titles (default: the interface's). The key stays in the
  local settings; only the searched title and year are sent to TMDB.
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
- "Create a folder for each movie" moves each file with a movie (video
  and subtitles alike) into a folder named after its new name, inside its
  current folder; a file already in a folder with that name stays. The
  preview shows the folder before the new name; the folder name is
  checked like a file name. Missing folders are created when renaming,
  recorded in the history entry, and removed by undo when left empty.
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
  `<new name>-poster.jpg` and so on. Existing files are never replaced.
  They are recorded in the rename's history entry and deleted by its undo.
- Movies whose names are already right (unchanged rows with a movie) get
  the metadata too when renaming: the Rename button counts them and the
  confirmation mentions them. Files that already exist are skipped
  without error (and not downloaded). Without any rename, the created
  files get a history entry of their own ("N metadata files"), undone by
  deleting them.
- A name with `tmdbid-N` (as in `[tmdbid-603]`) is looked up by that id
  directly, and picked automatically.
- The About tab shows the TMDB logo and notice required by its terms.
- Subtitles (srt, ass, ssa, sub, idx, vtt, sup, smi) keep the language
  and flags at the end of their names (`.pt-BR`, `.eng.forced`: lowercase
  ISO codes, optionally with a region) after the movie or episode name.

### 2.2 TV shows (TMDB)

- Same key and language as movies. "Find episodes" first finds the show:
  by id (digits, a `themoviedb.org/tv/N` link or `tmdbid-N`), else by name
  (with an optional year) on TMDB's show search, ranked like movies. With
  the field empty, the name is the text before the episode number most
  file names share, else the folder's name (the one above, for a season
  folder). An exact, unambiguous match is picked automatically; otherwise
  a window lists the shows (poster, name, original name, year, overview)
  with a search field. The same typed text reuses the show picked last.
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
  through when that episode exists; an
  air date gives the episodes aired that day. The title is compared with
  every episode's titles (same similarity as movies; the show's name in
  front is ignored).
  - Numbers, no title: one candidate is picked; several wait for the user.
  - Numbers and title: when the title's best episode scores ≥ 0.7, is not
    among the number's candidates and beats them by over 0.2, the file
    waits for the user (a conflict), suggesting the title's episode first.
    Otherwise one candidate is picked, or the best of several when it
    scores ≥ 0.6 and leads by 0.1.
  - Title only: picked when it scores ≥ 0.85 and leads by 0.1; otherwise
    the episodes scoring ≥ 0.4 are suggested.
- Marks and review as for movies (✓, ?, ✕). The review shows what was
  read from the name, the conflict, up to 9 suggestions (still image,
  code, title, air date, overview; 1–9 or Enter for the first) and every
  episode by season (specials last) with a filter by title or code.
  Ctrl+click marks several episodes of one season for a multi-episode
  file. Undecided files read the same way get the same choice.
- Each file keeps its show, so several shows can be done one at a time
  (filter, search). Searching again skips files already settled for the
  same show.
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
  the file's own folder. A file already there stays. The preview shows
  `<show folder>/Season NN` before the new name; each folder name is
  checked like a file name.
- After any rename, the folders files were moved out of are deleted when
  left empty, up to the folder the move happened within. Undo makes them
  again when moving the files back.
- With "save .nfo and images", renaming also writes, for renamed episode
  videos and for unchanged ones with an episode (counted by the Rename
  button and the confirmation, like movies):
  - next to the video: `<name>.nfo` with one `<episodedetails>` per
    episode in the file (title as in the name, showtitle, season,
    episode, aired, premiered, plot, runtime, rating, votes, uniqueid
    tmdb, director, writer, thumb, guest stars as `GuestStar`), and
    `<name>-thumb.jpg` (the first episode's still);
  - in the show's folder (the one the folders option made, or the
    current show folder as above; none for loose files): `tvshow.nfo`
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
- Only the show's name (or id) is sent to TMDB.

### 2.3 Music (tags)

- Tags are read with `lofty` (no audio decoded, no cover art): MP3/MP2
  (ID3v2, ID3v1, APE), FLAC, OGG Vorbis, Opus, Speex, M4A/M4B/M4P/M4R,
  AAC, AIFF, WAV, WavPack, APE, Musepack. Video containers (`mp4`, `m4v`,
  `3gp`) are left out. Each field comes from the first tag that has it,
  the file's main tag first. Read once per file and cached.
- Fields: title, artist, album artist (the artist's when missing),
  album, year (from the date), track and total, disc and total, genre,
  composer. A song without a title (after the name fallback, if on) is
  left unchanged and flagged (`untagged`, ✕ in the table, a count in the
  card).
- Names: track with two digits (three when the album or track number
  goes over 99); `disc-track` (`1-01`) when the option is on and the
  album has more than one disc (disc total over 1, or disc over 1);
  values made safe like movie titles, keeping final dots in file names
  (the extension follows; folder names lose them); empty `()`/`[]` dropped, repeated
  ` - ` separators collapsed, separators at the ends trimmed.
- Album-wide agreement (before naming): songs tagged `Single`/`Singles`
  as album take their title as album. In a folder (disc folders count as
  their parent) of at least 3 songs where more than half share an album
  tag, songs with another album tag whose track number is above that
  album's highest on the same disc (bonus tracks: a single, a soundtrack
  song) take its album, album artist, year and release id. Then songs
  with the same album tag in one folder share: the most common
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
  years and dates: `1012 - Samhain 2024` for "Show 1012 - Samhain 2024
  (2024-10-27)"; or
  are, with similarity ≥ 0.85, a run of at least two of its words, like
  `1037 - Title` for
  "Show 1037 - Title (2025)"), or
  sit inside a folder named after the artist;
  at most one level more than the template has.
- Folders some songs are moved out of (old artist or album folders, below
  their root) are climbed past by other songs whose root they would be,
  so a side project in `Korn [restored]/Jonathan Davis` ends up next to
  `Korn`, not inside the emptied folder.
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
- Folder levels that differ only in case across the batch (`a-ha`,
  `A-Ha`) are unified, for every rule that moves files: an existing
  folder's spelling, else the most common one (the first on a tie).
- "Create a playlist for each album": after renaming, each folder a
  song went to (the one above, for a disc folder) gets
  `<folder name>.m3u8` (UTF-8, `#EXTM3U`, one
  `#EXTINF:<seconds or -1>,<artist> - <title>` per song, paths relative
  to it) listing its songs and those of its
  disc folders, by disc (the disc folder's number, else the tag), track
  number and file name; not when it has fewer than two songs or already
  has a `.cue`, `.m3u` or `.m3u8`. Recorded in the history and deleted by
  undo.
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
  left out for Various Artists, 25 results) and ranked: title similarity
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
- A matched song takes the release's title (track), artist (the track's
  artist credit, else the release's), album artist (release credit),
  album, year (release date), track and disc numbers and totals, in
  place of its tags, before the album-wide agreement. Typographic
  punctuation is made plain (U+2010/2011 hyphens, curly quotes, `…`);
  dashes stay.
- "Save the album's cover": after renaming, `cover.jpg` (front, 1200 px)
  in the folder each album with a release ends up in, unless it has a
  `cover.*`, `folder.*` or `front.*` file; albums whose songs keep their
  names count in the Rename button too. Recorded in the history and
  deleted by undo.
- Only the album and artist names (and release ids) are sent.
- The files' audio and tags are never changed (cue sheets and
  playlists that list them are updated, see 4).

## 3. Preview

- Table: original name → new name, with a status per row:
  unchanged, ok, conflict (with the reason).
- The new name can be edited by hand in the table (overrides the rules for that row).
- Checks: empty name (including a name the rules emptied, leaving only
  the extension, which would make a hidden file), `/` or null character,
  longer than 255 bytes,
  two files with the same target, target already exists outside the batch.
  On Windows, `\ : * ? " < > |` and a trailing dot or space are errors;
  elsewhere they are a non-blocking warning. On Windows and macOS, names
  that differ only in case count as the same name.
- Counters: total, changed, conflicts.

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

## 4. Apply

- Confirmation before renaming. Rows with conflicts are not applied.
- Safe two-phase renaming (source → temporary → target), which allows
  swaps (a↔b), cycles and case-only changes on case-insensitive file systems.
- Renames run by depth: deepest first, so a folder is renamed after the
  files inside it (undo runs the other way).
- Failure midway: whatever was done for that file is reverted.
- Report at the end: successes and errors.
- Several files heading for one name are all blocked, except identical
  copies (by content, as found in the background): the first in the list
  goes ahead and the others stay, marked "copy".
- Folders files were moved out of are deleted when left empty (up to
  the folder the move happened within); undo makes them again.
- Cue sheets and playlists (`.cue`, `.m3u`, `.m3u8`) in the folders
  renamed files left, or renamed/moved themselves, are rewritten so their
  references (`FILE "…"` lines in cue sheets, non-comment lines in
  playlists; relative or absolute, `/` or `\`, URLs left alone) point to
  the new names; references to files that stayed are adjusted when the
  list moved. UTF-8 or Latin-1 is kept (a list whose new names Latin-1
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
  are back in place. Entries can be deleted from the history.
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
  and a link to the repository.
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

- Language: English, Portuguese, Spanish. On first run, the system's.
- Theme: light, dark or system (default).
- Remember the rules between sessions (on).
- Include hidden files when adding folders (off).
- Ask for confirmation before renaming (on).
- Turn the rules off after renaming (on).
- Warn about characters Windows does not accept (on).
