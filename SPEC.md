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
