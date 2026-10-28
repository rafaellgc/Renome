import { ex, type Section } from "./types";

export const en: Section[] = [
  {
    id: "start",
    title: "Getting started",
    html: `
<p>Renome renames many files at once, in three steps:</p>
<ol>
  <li><b>Add the files</b>: drag files or folders into the window, or use <b>+ Add</b>.</li>
  <li><b>Build the rules</b>: under <b>+ Rule</b>, choose what to do with the names (replace text, number, insert a date…). The <b>New name</b> column shows the result right away, before anything changes on disk.</li>
  <li><b>Rename</b>: click <b>Rename</b> (or <kbd>Ctrl</kbd>+<kbd>Enter</kbd>). If something goes wrong, undo it in the <b>History</b>.</li>
</ol>
<p>Nothing changes on disk until you click Rename, so feel free to experiment.</p>`,
  },
  {
    id: "files",
    title: "File list",
    html: `
<h4>Adding</h4>
<p>The <b>+ Add</b> button has three options:</p>
<ul>
  <li><b>Files…</b>: pick one or more files.</li>
  <li><b>Folders (with subfolders)…</b>: adds every file inside the folders, subfolders included.</li>
  <li><b>Folders (rename the folders themselves)…</b>: adds the folders themselves, to change their names. They are shown with a folder icon.</li>
</ul>
<p>Dragging into the window works like "Files" and "Folders (with subfolders)". Files already in the list are skipped, and newly added ones are selected.</p>
<p>Hidden files (starting with a dot, like <code>.config</code>) are left out when adding folders, unless you turn on the matching option.</p>
<p>You can also open the program with files: <code>renome photo1.jpg photo2.jpg folder/</code>.</p>

<h4>Filtering</h4>
<p>The <b>Filter</b> field shows only matching files. <b>Only visible files are renamed</b>, and numbering counts only them.</p>
<ul>
  <li>Terms separated by commas or spaces: <code>jpg, holiday</code> shows names containing "jpg" <i>or</i> "holiday".</li>
  <li><code>*</code> and <code>?</code> are wildcards: <code>*.png</code> (ends in .png), <code>IMG_????.jpg</code> (any four characters).</li>
  <li>In quotes, looks for the whole phrase: <code>"breaking bad"</code>.</li>
  <li>With <code>-</code> in front, hides what matches: <code>-*.srt</code> hides subtitles; <code>*.mkv -sample</code> shows the .mkv files except samples.</li>
  <li>Case and accents do not matter: <code>cafe</code> finds "Café".</li>
</ul>

<h4>Sorting and arranging</h4>
<ul>
  <li>Click a column header to sort; click again to reverse.</li>
  <li>The default order is by <b>New name</b>: the list re-sorts as you change the rules.</li>
  <li>Select with click, <kbd>Ctrl</kbd>+click (several) and <kbd>Shift</kbd>+click (a range).</li>
  <li><b>⤒ ⤓</b> move the selected files up or down, handy to set the numbering. They do not work when sorting by New name.</li>
  <li><b>Remove</b> (or <kbd>Delete</kbd>) takes the selected files out of the list; <b>Clear</b> empties it. Files on disk are not touched.</li>
  <li><kbd>Ctrl</kbd>+<kbd>Z</kbd> undoes the last change to the list.</li>
</ul>`,
  },
  {
    id: "rules",
    title: "How rules work",
    html: `
<p>Rules are applied <b>from top to bottom</b>, each on the result of the previous one. Order matters: numbering and then changing case is not the same as the other way round.</p>
<ul>
  <li>The <b>checkbox</b> in the title turns the rule on and off without deleting it. While off, it is locked: turn it on to change its options or search.</li>
  <li><b>↑ ↓</b> change the order; <b>✕</b> removes the rule; clicking the title collapses the card.</li>
  <li>A rule with an error (an invalid expression, say) gets a red border and is skipped until it is fixed.</li>
</ul>
<h4>Apply to</h4>
<p>Several rules ask where to act:</p>
<ul>
  <li><b>Name (without extension)</b>: the default. In <code>photo.jpg</code>, acts only on <code>photo</code>.</li>
  <li><b>Extension</b>: only on <code>jpg</code>.</li>
  <li><b>Full name</b>: on the whole <code>photo.jpg</code>.</li>
</ul>
<p>Folders have no extension: in <code>Show.2020</code>, the <code>.2020</code> is part of the name.</p>
<h4>Saving and opening rules</h4>
<p>📂 opens and 💾 saves the rule set to a <code>.json</code> file, to reuse later. By default the program also remembers the rules when it is closed.</p>
<h4>Positions</h4>
<p>Where a rule asks for a position, it counts visible characters starting at 1. Accented letters and emoji count as one character.</p>`,
  },
  {
    id: "r-replace",
    title: "Replace text",
    sub: true,
    html: `
<p>Replaces one text with another.</p>
${ex("holiday_photo_01.jpg", "holiday photo 01.jpg")}
<ul>
  <li><b>Find</b> and <b>Replace with</b>: leave "Replace with" empty to delete the text.</li>
  <li><b>Match case</b>: unchecked, "Photo" also finds "photo" and "PHOTO".</li>
  <li><b>Only the first occurrence in each name</b>: replaces only the first time the text appears <i>in each name</i>. In <code>a_b_c</code>, replacing <code>_</code> with a space gives <code>a b_c</code>. If the text appears only once, every name changes the same way.</li>
</ul>`,
  },
  {
    id: "r-regex",
    title: "Regular expression",
    sub: true,
    html: `
<p>For patterns "Replace text" cannot reach. Groups in parentheses can be reused in the replacement as <code>$1</code>, <code>$2</code>…</p>
${ex("01 - Opening.mp3", "Opening (01).mp3")}
<p>Expression <code>(\\d+) - (.*)</code>, replace with <code>$2 ($1)</code>.</p>
<table>
  <tr><td><code>.</code></td><td>any character</td></tr>
  <tr><td><code>\\d</code> <code>\\s</code> <code>\\w</code></td><td>digit, space, letter/digit/_</td></tr>
  <tr><td><code>*</code> <code>+</code> <code>?</code></td><td>zero or more, one or more, optional</td></tr>
  <tr><td><code>{2}</code> <code>{1,3}</code></td><td>exactly 2, 1 to 3 times</td></tr>
  <tr><td><code>[abc]</code> <code>[^abc]</code></td><td>one of these characters, any but these</td></tr>
  <tr><td><code>^</code> <code>$</code></td><td>start and end of the text</td></tr>
  <tr><td><code>( )</code></td><td>group, reused as <code>$1</code>, <code>$2</code>…</td></tr>
  <tr><td><code>\\.</code> <code>\\(</code></td><td>a literal dot, a literal parenthesis</td></tr>
</table>
<p class="tip">If a group is followed by letters or digits, use braces: <code>\${1}a</code>. Written <code>$1a</code>, the program looks for a group named "1a", which does not exist.</p>
<p>Named groups work too: <code>(?P&lt;year&gt;\\d{4})</code> and then <code>\${year}</code>.</p>`,
  },
  {
    id: "r-rearrange",
    title: "Rearrange parts",
    sub: true,
    html: `
<p>Splits the name at a separator and puts it back together in another order. No regular expressions needed.</p>
${ex("Show - Episode - S01E02.mkv", "Show - S01E02 - Episode.mkv")}
<ul>
  <li><b>Separator</b>: the text between the parts (here, <code> - </code>). Spaces around each part are trimmed, and empty parts are dropped.</li>
  <li>The card shows the parts of the first file in the list, numbered. <b>Click a part</b> to add it to the new order.</li>
  <li><b>New order</b>: numbers are the parts; everything else is kept as text. For the example: <code>1 - 3 - 2</code>.</li>
  <li>Parts left out of the order are removed. Order <code>2 (1)</code> on <code>Smith - Ann</code> gives <code>Ann (Smith)</code>.</li>
  <li>If a name has fewer parts than the order asks for, it is left as it is.</li>
</ul>`,
  },
  {
    id: "r-case",
    title: "Upper / lower case",
    sub: true,
    html: `
<ul>
  <li><b>lowercase</b>: <code>A Day at the Beach</code> → <code>a day at the beach</code></li>
  <li><b>UPPERCASE</b>: → <code>A DAY AT THE BEACH</code></li>
  <li><b>Sentence case</b>: → <code>A day at the beach</code></li>
  <li><b>Title Case Every Word</b>: → <code>A Day At The Beach</code></li>
  <li><b>Title Case, except of/the/and…</b>: → <code>A Day at the Beach</code>. Short linking words (of, the, and, to, in, de, da, del, la, y…) stay lowercase, except at the start of the name or of a part (after <code> - </code>, <code>:</code> or <code>(</code>).</li>
</ul>
<p>In both Title Case modes, Roman numerals stay in capitals (<code>Fase III</code>, <code>Part II</code>) and words with capitals inside stay as they are (<code>iPhone</code>, <code>McDonald's</code>).</p>
<p>Use "Apply to: Extension" to change only the extension.</p>`,
  },
  {
    id: "r-insert",
    title: "Insert",
    sub: true,
    html: `
<p>Adds something to the name.</p>
<h4>What</h4>
<ul>
  <li><b>Text</b>: fixed text.</li>
  <li><b>Current date</b>: today's date.</li>
  <li><b>File modification date</b>.</li>
  <li><b>Date the photo was taken</b>: read from the photo's EXIF data (JPEG, HEIC, PNG, WebP, TIFF and RAW formats). Photos without that date are left as they are, unless <b>"Without that date, use the modification date"</b> is checked. Videos are not read yet.</li>
  <li><b>Folder name</b>: the name of the folder the file is in.</li>
</ul>
${ex("IMG_1234.jpg", "2024-07-15 IMG_1234.jpg")}
<h4>Date format</h4>
<table>
  <tr><td><code>%Y</code> <code>%y</code></td><td>year with 4 or 2 digits (2024, 24)</td></tr>
  <tr><td><code>%m</code> <code>%d</code></td><td>month and day (07, 15)</td></tr>
  <tr><td><code>%H</code> <code>%M</code> <code>%S</code></td><td>hour, minute, second</td></tr>
  <tr><td><code>%j</code></td><td>day of the year (001–366)</td></tr>
</table>
<p>Example: <code>%Y-%m-%d %H.%M.%S</code> gives <code>2024-07-15 14.32.10</code>. A <code>/</code> in the format becomes <code>-</code>, since file names cannot contain slashes.</p>
<h4>Where</h4>
<ul>
  <li><b>At the start</b> or <b>At the end</b> of the name (before the extension).</li>
  <li><b>At position</b> N: before the Nth character (1 = at the start).</li>
  <li><b>At position (counting from the end)</b> N: N characters before the end (0 = at the end, 1 = before the last one).</li>
</ul>`,
  },
  {
    id: "r-number",
    title: "Number",
    sub: true,
    html: `
<p>Numbers the files in list order.</p>
${ex("beach.jpg", "Trip 01.jpg")}
<ul>
  <li><b>Template</b>: the new name, with variables:
    <ul>
      <li><code>{n}</code>: the number;</li>
      <li><code>{name}</code>: the current name (without extension);</li>
      <li><code>{folder}</code>: the name of the folder the file is in.</li>
    </ul>
    Examples: <code>Trip {n}</code>, <code>{name} - {n}</code>, <code>{folder} {n}</code>.</li>
  <li><b>Start</b> and <b>Step</b>: 1 and 1 give 1, 2, 3…; 10 and 10 give 10, 20, 30…; a negative step counts down.</li>
  <li><b>Digits</b>: pads with zeros on the left (2 digits: 01, 02… 10).</li>
  <li><b>Restart numbering in each folder</b>: each folder starts over.</li>
</ul>
<p>Numbering follows the list order and counts only the files visible in the filter. When sorting by New name, it follows the current name order. To control the order, sort by another column or use ⤒ ⤓.</p>`,
  },
  {
    id: "r-remove",
    title: "Remove characters",
    sub: true,
    html: `
<ul>
  <li><b>Repeated and leading/trailing spaces</b>: <code>  my   holiday  photo </code> → <code>my holiday photo</code></li>
  <li><b>All spaces</b>: → <code>myholidayphoto</code></li>
  <li><b>Accents</b>: <code>Café</code> → <code>Cafe</code></li>
  <li><b>Digits</b>: <code>photo123</code> → <code>photo</code></li>
  <li><b>Characters</b>: each character typed is removed wherever it appears. <code>_#</code> removes every <code>_</code> and <code>#</code>.</li>
</ul>`,
  },
  {
    id: "r-range",
    title: "Remove range",
    sub: true,
    html: `
<p>Deletes a piece of the name, defined by where it starts and ends.</p>
<ul>
  <li><b>From</b>: the start of the name, a position, or the first time a text appears.</li>
  <li><b>To</b>: the end of the name, a position, or a text (looked for after the start).</li>
  <li><b>Also remove the delimiter texts</b>: whether the start and end texts themselves go too.</li>
</ul>
${ex("Movie [1080p] (2020).mkv", "Movie  (2020).mkv")}
<p>From text <code>[</code> to text <code>]</code>, removing the delimiters. The texts are case-sensitive. If one of them is missing from a name, that name is left as it is.</p>`,
  },
  {
    id: "r-char",
    title: "Character at position",
    sub: true,
    html: `
<p>Replaces or deletes the character at a position.</p>
<ul>
  <li><b>Position</b>: starting at 1. With <b>Count from end</b>, 1 is the last character.</li>
  <li><b>Only if it is</b>: only changes it if the character at that position is this one (empty = any). Case is ignored unless <b>Match case</b> is checked.</li>
  <li><b>Replace with</b>: the new character. Empty deletes it.</li>
</ul>
${ex("_photo.jpg", "photo.jpg")}
<p>Position 1, only if <code>_</code>, replace with nothing.</p>`,
  },
  {
    id: "r-ext",
    title: "Extension",
    sub: true,
    html: `
<ul>
  <li><b>lowercase</b> / <b>UPPERCASE</b>: <code>photo.JPG</code> → <code>photo.jpg</code></li>
  <li><b>Set</b>: replaces the extension with another (<code>jpeg</code> → <code>jpg</code>). It does not convert the file, it only changes the name.</li>
  <li><b>Remove</b>: drops the extension.</li>
</ul>
<p>Folders are not affected by this rule.</p>`,
  },
  {
    id: "r-movie",
    title: "Movie (TMDB)",
    sub: true,
    html: `
<p>Gives each movie its official name from <b>TMDB</b> (themoviedb.org), in the format Jellyfin recognizes. It needs a free <b>TMDB API key</b>, entered in Options.</p>
${ex("Movie.Name.2001.1080p.BluRay.x264.mkv", "Movie Name (2001) [tmdbid-12345].mkv")}
<div class="notice">
<p><b>Before using it: file names matter</b></p>
<p>The movie is looked up by the <b>title</b> and <b>year</b> in the file's name. Names full of release marks (<code>Movie.Name.2001.1080p.BluRay</code>) work well, since the marks are dropped. But a name without the title (<code>movie1.mkv</code>), with only initials or abbreviations, or with the title in a language TMDB does not have, is not found or goes to the review; without the year, movies with the same name get mixed up.</p>
<p>If your files are far from that, put the rules that fix the name above this one in the list (<b>Replace text</b>, <b>Remove range</b>, <b>Regular expression</b>): the search reads the name as they leave it, and the <b>New name</b> column shows the result before renaming.</p>
</div>
<ol>
  <li>Add the rule and click <b>Search TMDB</b>. The program takes the title and year from each file name (dots become spaces; brackets, parentheses and tags like 1080p, BluRay, x264 are dropped) and searches TMDB.</li>
  <li>When title and year clearly match, the movie is picked automatically (<b>✓</b> next to the new name). When in doubt (<b>?</b>), the review window opens: click the right movie (or press 1–9). The year in the file may be one year off from TMDB; that is taken into account.</li>
  <li>If none fits, fix the title or year in the window's search field, or choose <b>None of these</b> (<b>✕</b>: the file is left as it is).</li>
</ol>
<ul>
  <li><b>Template</b>: <code>{title}</code>, <code>{original}</code> (original title), <code>{year}</code> and <code>{id}</code>. The default is <code>{title} ({year}) [tmdbid-{id}]</code>.</li>
  <li><b>Replace ":" with</b>: Windows does not accept colons; by default they become <code> - </code>. Slashes become dashes and <code>? * &lt; &gt; |</code> are removed.</li>
  <li>Only videos, subtitles and folders are looked up and renamed; images, <code>.nfo</code> and other files in the list stay as they are. Subtitles with the same name as the movie get the same choice. Subtitles keep their language at the end of the name: <code>Movie Name (1999) [tmdbid-12345].pt-BR.srt</code>.</li>
  <li><b>Create a folder for each movie</b>: the movie and its subtitles go into a folder with the new name, as Jellyfin recommends: <code>Movies/Movie Name (1999) [tmdbid-12345]/Movie Name (1999) [tmdbid-12345].mkv</code>. A movie already in the right folder is not moved again. Undo puts the files back and deletes the folder if it is left empty.</li>
  <li>To remove accents, add the <b>Remove characters → Accents</b> rule after this one.</li>
</ul>
<h4>Metadata (.nfo and poster)</h4>
<p>With <b>Save .nfo and poster next to the movie</b>, renaming also creates, next to each video, <code>Name (year) [tmdbid-…].nfo</code> and the poster; optionally the background image (fanart) and the movie's logo too. Inside the movie's folder the images are named <code>poster.jpg</code>, <code>fanart.jpg</code> and <code>logo.png</code>; next to other movies they start with the movie's name (<code>…-poster.jpg</code>). Jellyfin, Kodi and Emby read these files.</p>
<p>The <code>.nfo</code> holds everything TMDB has that Jellyfin reads: title and original title, year and release date, overview and tagline, runtime, rating and votes, the age rating of the chosen language's country (e.g. PG-13), genres, keywords (as tags), countries, studios, collection (e.g. "Collection Name"), YouTube trailer, director, writers, cast with characters and photos, producers, composer and the TMDB and IMDb ids. The movie file is not changed, existing files are not replaced, and <b>undoing</b> the rename deletes what was created.</p>
<p>Movies whose <b>names are already right</b> get the metadata too: the Rename button counts them and the confirmation says so. If the name already has <code>[tmdbid-…]</code>, the movie is recognized by that id, without searching. That way an organized library can get just the .nfo files and images.</p>
<h4>Limitations</h4>
<ul>
  <li>The movie is looked up by the title and year in the file name. Heavily abbreviated names, names with initials only or with the title in another language may not be found; in that case, fix the title in the review.</li>
  <li>Movies with the same name (remakes, say) can only be told apart by the year; without a year in the name, the choice usually goes to the review.</li>
  <li>Rare, regional or very recent movies may be missing from TMDB, or lack a translation, an overview or a poster.</li>
</ul>
<p class="tip">Only the searched title and year are sent to TMDB, never the full file name or the folders.</p>`,
  },
  {
    id: "r-series",
    title: "TV show (TMDB)",
    sub: true,
    html: `
<p>Gives each episode its official <b>TMDB</b> name, with season, number and title, in the format Jellyfin recognizes. It uses the same API key as the movie rule.</p>
${ex("Show.Name.S01E03.720p.WEB-DL.mkv", "Show Name (2008) - S01E03 - Episode Title.mkv")}
<div class="notice">
<p><b>Before using it: file names matter</b></p>
<p>Each file is matched to its episode by what its name says. The series' name can come from the <b>Show</b> field, but the episode cannot: the name needs <b>the season and episode numbers</b> (<code>S01E03</code>, <code>1x03</code>; the season may also come from the folder) <b>or the episode's title</b>. A name with neither (<code>Series Name - part A.mkv</code>, <code>video_0042.mkv</code>) is not found and goes to the review, where it is chosen by hand.</p>
<p>If your files are far from that, put the rules that fix the name above this one in the list (<b>Replace text</b>, <b>Rearrange parts</b>, <b>Regular expression</b>, <b>Number</b>): the search reads the name as they leave it.</p>
</div>
<ol>
  <li>In the <b>Show</b> field, type the name or the TMDB id, or paste the link to the show's page (<code>themoviedb.org/tv/1396-…</code>). Then only that show's files are looked up; those whose name says another show (or carries another's <code>[tmdbid-…]</code>) are left as they are. <b>Left empty</b>, the program sorts the files by the show each name says (with the year, when the names have it; by the number, when they carry <code>[tmdbid-…]</code>) and looks up one show at a time, asking when in doubt. A file without the show in its name goes with the show of most files in the same folder; in a folder with no show names, the folder's name counts; in a folder of several shows and no majority, it is left out (the program says how many).</li>
  <li>Click <b>Find episodes</b>. If more than one show looks alike, choose the right one. The program downloads the whole episode list and looks for each visible file's episode.</li>
  <li>Episodes found with certainty get <b>✓</b>. Doubtful ones (<b>?</b>) and those not found (<b>✕</b>) go to the review window.</li>
</ol>
<h4>How each episode is found</h4>
<ul>
  <li><b>By number</b>, in many forms: <code>S01E02</code>, <code>s1e2</code>, <code>1x02</code>, <code>Season 1 Episode 2</code>, <code>Ep 02</code> (the season comes from the folder, such as <code>Season 2</code>), absolute numbering (<code>Show Name - 125</code>), <code>201</code> (season 2, episode 1) and air date (<code>2024.03.15</code>). Files with two episodes (<code>S01E01E02</code>, <code>S01E01-02</code>) become <code>S01E01-E02</code>. Names made only of bracketed pieces (<code>[Group][Show Name][S03E22][Title][1080p]</code>, also with <code>【】</code>) are read too: the piece with the number, the show before it and the title after it.</li>
  <li><b>By title</b>: the text after the number is compared with the titles of every episode, in your language, in English and in the original language, ignoring accents, case and small misspellings; Roman numerals count as numbers (<code>Part II</code> = <code>Part 2</code>). With every season in one folder, each starting from 1 (<code>02 - Title.mkv</code>), the title picks the season. With no number at all, the title alone is enough when it is clear.</li>
  <li><b>When the number and the title disagree</b> (the name says S02E08, but the title is S01E03's), the file gets <b>?</b>, and the review shows both, suggesting the title's.</li>
  <li><b>Files without the show's name</b> (<code>06 - Title.mkv</code>) outside the show's folder only get the number's episode when the title fits too (or there is no title); otherwise they go to the review.</li>
</ul>
<h4>Review</h4>
<p>Shows what was read from the name, the suggestions (click, press 1–9, or <b>Enter</b> for the first) and every episode of the show by season, with a filter by title or number (<code>s01e03</code>). <b>Ctrl+click</b> marks more than one episode of the same season, for a file with several episodes. Subtitles read the same way get the same choice. Only videos and subtitles are looked up; images, <code>.nfo</code> and other files in the list stay as they are.</p>
<ul>
  <li><b>Template</b>: <code>{show}</code>, <code>{original}</code> (original name), <code>{year}</code> (first aired), <code>{season}</code>, <code>{episode}</code> (two digits), <code>{title}</code> and <code>{id}</code>. The default is <code>{show} ({year}) - S{season}E{episode} - {title}</code>.</li>
  <li>Untranslated episodes get the English (or original) title instead of "Episode 4".</li>
  <li>Subtitles keep their language at the end of the name: <code>….pt-BR.srt</code>.</li>
  <li>Files not in the chosen show are left as they are. Each file keeps its own show; searching again leaves the settled ones alone.</li>
</ul>
<h4>Show and season folders</h4>
<p>With <b>Organize into show and season folders</b>, each episode goes into the structure Jellyfin recommends: <code>Shows/Show Name (2008) [tmdbid-12345]/Season 01/…</code>. The show's folder is created next to the folder the show is already in (if the files are in <code>Show Name/Season 1</code>, the new folder goes next to <code>Show Name</code>, not inside it); with loose files, inside their folder. The show folder's name has its own template, with the same keys (default <code>{show} ({year}) [tmdbid-{id}]</code>; use <code>{original}</code> for the original name). Old folders left empty are deleted; those that still hold other files stay. So after renaming, the old folder may be gone (<code>Cartoons/Show Name</code>) and the files show up in the new one next to it (<code>Cartoons/Show Name (2008) [tmdbid-12345]/Season 01/…</code>): <b>nothing was deleted, only moved</b>. <b>Undo</b> puts everything back, making the old folders again.</p>
<h4>Metadata (.nfo and images)</h4>
<p>With <b>Save .nfo and images for Jellyfin</b>, renaming also creates:</p>
<ul>
  <li>in the <b>show's folder</b>: <code>tvshow.nfo</code> (title, original title, year, overview, status, runtime, rating, age rating, genres, keywords, networks and studios, creators, trailer, cast with characters, and the TMDB, IMDb and TVDB ids), <code>poster.jpg</code> and each season's poster (<code>season01-poster.jpg</code>); optionally <code>fanart.jpg</code> and <code>logo.png</code>;</li>
  <li>in the <b>season's folder</b>: <code>season.nfo</code>, with the season's name and overview;</li>
  <li>next to <b>each episode</b>: <code>….nfo</code> (title, season and number, air date, overview, runtime, rating, directors, writers and guest stars) and the thumbnail <code>…-thumb.jpg</code>.</li>
</ul>
<p>The show and season files are only created when the episodes are in a show folder (named after it, or with <code>[tmdbid-…]</code>), the one from before or the one the folders option makes. Episodes whose names are already right get the metadata too. Existing files are never replaced, and undo deletes what was created.</p>
<h4>Missing episodes</h4>
<p>With <b>Show the missing episodes</b> (on by default), after the lookup the list gets faded rows for the aired episodes that no video in the list holds, such as <code>S01E16 · Title</code>, with the air date. They sit where the episode belongs, between its neighbours, and the bottom bar says how many are missing. They are only a notice: they cannot be selected and are left out of counts, renaming and export.</p>
<p>Only seasons the list has at least one video of are checked; whole seasons you do not have are not shown. Episodes not aired yet, or without a date, are not either. An episode with only its subtitle counts as missing.</p>
<h4>Limitations</h4>
<ul>
  <li>A show is only recognized in a file name by the names it has on TMDB (in any language or country) or by what you typed. A nickname or acronym TMDB does not know makes the file count as another show's: type the show, or use that name in the field.</li>
  <li>Seasons are split as TMDB splits them. Anime and shows with absolute numbering are sometimes split differently there, and the numbers in the files do not match; check in the review.</li>
  <li>Specials (season 0) are often numbered differently from their releases and nearly always need a review.</li>
  <li>A file with several episodes is only recognized when they are in the same season.</li>
  <li>Missing episodes follow TMDB's numbering: if your files use another (the DVD order, or anime's running numbers), the list of what is missing may be wrong. Episodes hidden by the filter also count as missing.</li>
</ul>
<p class="tip">Only the show's name is sent to TMDB, never file names or folders.</p>`,
  },
  {
    id: "r-music",
    title: "Music (tags)",
    sub: true,
    html: `
<p>Names each song from the <b>tags</b> stored in the file (ID3 in MP3, Vorbis comments in FLAC and OGG, MP4 in M4A, and others), with no internet needed.</p>
${ex("01.mp3", "01 - Song Title.mp3")}
<ul>
  <li><b>Template</b>: <code>{track}</code>, <code>{title}</code>, <code>{artist}</code>, <code>{album artist}</code>, <code>{album}</code>, <code>{year}</code>, <code>{disc}</code>, <code>{genre}</code> and <code>{composer}</code>. The default is <code>{track} - {title}</code>. The track has two digits (three on albums with over 99 tracks).</li>
  <li>Without a track number in the tags, the number the file name starts with stays (<code>07. Title.flac</code>), so the order is not lost. An album gets one year, the most common among its tracks: a bonus track from a reissue does not get a folder of its own.</li>
  <li>Empty tags leave nothing behind: with <code>{track} - {artist} - {title}</code>, a song without an artist becomes <code>01 - Title</code>.</li>
  <li><b>Disc number in the track</b>: on albums with more than one disc, the track becomes <code>1-01</code>, <code>2-01</code>…, so the discs do not mix.</li>
  <li><b>The album as a whole</b>: the tracks of one album (same folder, or <code>CD 01</code>/<code>CD 02</code> discs, and same album name) share the album's details. A track without a year gets the others'. Without an album artist in the tags, the artist of most tracks counts (guests such as <code>feat.</code> left out); with no majority, the album is a compilation: <code>Various Artists</code>, as Jellyfin knows it. So an album's tracks never scatter into different folders.</li>
  <li><b>Bonus tracks</b>: in a folder where most songs are one album's, tracks tagged with another album (a single, a soundtrack) and numbered after the album's (<code>15</code>, <code>16</code> after <code>01</code>–<code>14</code>) take that album.</li>
  <li><b>Singles</b>: a song tagged with the album <code>Single</code> gets its own title as album.</li>
  <li><b>Compilations</b>: on an album where most tracks are by others than the album's artist, the artist goes into the name before the title when the template lacks it: <code>03 - Track Artist - Title</code>.</li>
  <li>If the album name already has a year (<code>Show 12 (2025-05-04)</code>), the template's <code>({year})</code> is left out.</li>
  <li>Songs <b>without a title in their tags</b> are left as they are and get <b>✕</b> (unless the search identifies them by sound, see below); the card shows how many. With <b>Fill in from the file name</b>, the program reads <code>01 - Artist - Title</code>, <code>01. Title</code> or <code>1-05 Title</code> from the name, and the album and year from the folder (<code>Artist - Album (1975)</code>, or just <code>Album</code>; a <code>CD2</code> folder gives the disc). Existing tags come first.</li>
</ul>
<h4>Artist and album folders</h4>
<p>With <b>Organize into artist and album folders</b>, each song goes into <code>{album artist}/{album} ({year})</code> (the folder template can be changed; each <code>/</code> is a level). So <code>Music/Artist/Album/CD1/01.mp3</code> becomes <code>Music/Artist/Album (1975)/01 - Song Title.mp3</code>: folders already named after the artist, the album or a disc (or inside the artist's folder) are replaced, not filled with new folders. Loose files get the folders inside the folder they are in. Small spelling differences between the folder and the tag do not get in the way. A folder that already gathers albums named like it (<code>Show (1000 - ..)</code> for albums "Show 1037 - …") is taken as a collection: the albums go straight into it, with no artist folders in between.</p>
<div class="notice">
<p><b>Folders of loose songs stay together</b></p>
<p>A folder where nearly every song is from a different album, with one or two tracks from each (such as a <code>Loose</code>, <code>Misc</code> or <code>Singles</code> folder), is taken as a collection you put together. Its songs are renamed as usual but <b>stay in it</b>: they do not each go to their album's folder. Each song's album is still in its tags.</p>
<p>The exact rule: at least 3 songs, from at least 3 albums, with at least 4 in 5 of those albums having one or two tracks in the folder. A real album's folder, with several tracks from the same record, is organized as always. If you want the loose songs split up anyway, move them by hand to the folders you prefer.</p>
</div>
<p><b>Copies</b>: if two identical copies of an album (found by the program, as in <b>Identical copies</b>) would go to the same place, the first goes ahead and the other stays where it is, marked as a copy.</p>
<ul>
  <li>Lyrics (<code>.lrc</code>) with a song's name get its new name and go with it.</li>
  <li>Covers, <code>.cue</code>, <code>.log</code> and other files in the album's folder go along with most of the folder's songs (songs from other albums that are not bonus tracks follow their own). Subfolders without songs (<code>Artwork</code>, <code>Scans</code>) go along with the same name; so do files in the folder above <code>CD 01</code>/<code>CD 02</code>. If two such files would get the same name (a <code>log.txt</code> on each disc), they get their folder's name: <code>log (CD 01).txt</code>.</li>
  <li>Artist or album names that differ only in case (<code>artist name</code> and <code>Artist Name</code>, from different albums' tags) become one folder: the one already on disk, or the most used spelling.</li>
  <li>Old folders left empty are deleted; undo puts everything back.</li>
  <li><b>Create a playlist for each album</b>: after renaming, writes <code>&lt;folder name&gt;.m3u8</code> in each album's folder, with its songs (from every disc) in disc and track order. Albums whose names are already right get one too (they count in the <b>Rename</b> button). Not created when the album already has a playlist (<code>.m3u</code> or <code>.m3u8</code>): that one is updated to the new names, and the summary at the end says so. A <code>.cue</code> does not stop it, since it describes the disc and most players do not use it as a playlist. Not created for a single song either. Undo deletes the playlist created.</li>
  <li><code>.cue</code> files and <code>.m3u</code> playlists list the songs' new names (see <b>Renaming and undoing</b>).</li>
</ul>
<h4>Fixing with MusicBrainz</h4>
<p>For wrong or incomplete tags, <b>Search MusicBrainz</b> identifies each album on <b>MusicBrainz</b> (musicbrainz.org), an open music database. No key is needed.</p>
<ul>
  <li>The visible songs are grouped by album. If the tags already have the MusicBrainz id, the album is known at once (when every song is on it; otherwise it goes to the review); otherwise the program searches by artist and album name.</li>
  <li>One album often has several editions (original, remaster, deluxe, another country's). The program checks the <b>number of tracks</b>, <b>each one's title</b> and <b>length</b> (a track with the same number but another title and length does not count): if the best edition found fits the songs, it is picked automatically (<b>✓</b>). Otherwise the review opens, with cover, date, country, format, label and track count (green when it equals the folder's), for you to choose, or <b>None</b> to keep the tags.</li>
  <li>With the album chosen, names and folders use MusicBrainz's details: each track's title, artist, album artist, album name and year, in their official spelling. Each song is matched to its track by disc and number (or by title).</li>
  <li><b>Save the album's cover</b>: writes <code>cover.jpg</code> (from the Cover Art Archive) in the album's folder, if it has no cover yet. If the chosen edition has no image (a cassette, say), the album's cover is used. Undo deletes the cover created.</li>
  <li><b>Write the tags into the files</b>: after renaming, writes into the identified songs all MusicBrainz knows of them: title, artists (each one on its own too), album artist, album, date and original date, track and disc (with totals), label, catalog number, barcode, country, release type, medium, ISRC, genres, language, composer, lyricist, producer, arranger, mixer and the MusicBrainz ids, which Jellyfin, Navidrome and Plex use. For that, each album is looked up once more (about 2 seconds). Songs whose names are already right get them too (they count in the <b>Rename</b> button). A song that already has all of this is not written again. Works with MP3 (written as ID3v2.3), FLAC, OGG, Opus and M4A.</li>
  <li><b>Delete the other tags</b>: takes out what does not come from MusicBrainz (comments, site addresses, ripper notes, "encoded by"), and in MP3 also the secondary tags (ID3v1 and APE). Lyrics, ReplayGain, BPM, an embedded cue sheet (CUESHEET) stay, and genre and composer when MusicBrainz does not have them. Unchecked, the other tags stay as they are.</li>
  <li><b>Include the cover in the files</b>: takes out the pictures already inside each song and puts in the album's cover (from the Cover Art Archive or, if there is none, the folder's <code>cover.jpg</code>).</li>
  <li>The writing is done on a copy, which only takes the file's place at the end; a crash midway does not spoil the song. Undo puts the old tags and pictures back (in MP3 and M4A, rare fields cleaning took out and the ID3v1 and APE tags do not come back). The modification date changes, so players read the tags again. Since the content changes, a torrent no longer recognises these files.</li>
  <li>Lookups are spaced out (one every 2 seconds), as MusicBrainz asks; each album takes about 4 seconds.</li>
</ul>
<h4>Identify by sound (AcoustID)</h4>
<p>With <b>Identify by sound</b> checked (the default), the search also takes care of songs <b>without a title in their tags</b> (even with <b>Fill in from the file name</b>, since the name is only a guess) and of albums the search by name did not find or left in doubt. For an album in doubt, what was heard counts only if it identifies a release with certainty; otherwise the search's candidates stay. The program listens to the first 2 minutes of each song, computes the sound's <b>fingerprint</b> and asks <b>AcoustID</b> (acoustid.org), an open fingerprint database linked to MusicBrainz, which recording it is.</p>
<ul>
  <li>The songs of a folder are split among the albums their recordings are on: if most are on one album, the folder is that album; loose songs go to the artist's studio album rather than a compilation.</li>
  <li>Among the editions, the one with the track count nearest the folder's wins, then the earliest. If tracks and lengths fit, the album is picked automatically; otherwise it goes to the review, which shows <b>By sound:</b> with the album heard.</li>
  <li>A song without a title takes the name of its track on the chosen album. A song AcoustID does not know shows as <b>not recognised</b> and is left as it is.</li>
  <li>Reads MP3, FLAC, OGG, M4A (AAC and ALAC), WAV and AIFF. Other formats, such as Opus and WMA, are not listened to.</li>
  <li>Each song takes about half a second to be listened to and looked up.</li>
</ul>
<p class="tip">Only each song's sound fingerprint and length are sent to AcoustID, never the file.</p>
<h4>Limitations</h4>
<ul>
  <li>Without the MusicBrainz search, names depend on the tags alone: wrong tags make wrong names.</li>
  <li>Regional, independent, bootleg or very recent releases may be missing from MusicBrainz; then the tags are used.</li>
  <li>By sound, very similar recordings (a remaster, an artist's own re-recording) may be mixed up. When the album is in doubt, it goes to the review.</li>
  <li>Songs in Opus, WMA and other formats not listed above are not listened to.</li>
  <li>For MP3 and M4A, undoing written tags does not bring back rare fields or ID3v1 and APE tags.</li>
</ul>
<p class="tip">Without <b>Write the tags into the files</b>, Renome only reads the tags; the songs themselves are not changed. Only the artist and album name are sent to MusicBrainz.</p>`,
  },
  {
    id: "r-book",
    title: "Book (Open Library)",
    sub: true,
    html: `
<p>Names each book after <b>Open Library</b> (openlibrary.org), an open book catalog run by the Internet Archive. When Open Library does not know an ISBN, it is looked up on <b>BrasilAPI</b> (brasilapi.com.br), which gathers the records Brazilian publishers keep at the CBL. Neither needs a key.</p>
${ex("book_title_author.pdf", "Author Name - Book Title (1937).pdf")}
<ol>
  <li>Add the rule and click <b>Search Open Library</b>. For each book (EPUB, MOBI, AZW3, PDF, FB2 and DjVu), the program reads the details stored in the file itself: title, author, ISBN, publisher and series. Whatever is missing comes from the file name.</li>
  <li>With the <b>ISBN</b>, the book is found right away, in its exact edition (<b>✓</b>), on Open Library or BrasilAPI. Without one, the program searches by title and author: when they match with no doubt, the book is picked by itself; otherwise (<b>?</b>), the review opens, with cover, authors, original title and number of editions, for you to choose (click, or press 1–9).</li>
  <li>If none fits, fix the title or the author in the window and search again. You can also search by author alone, with the title empty, to see their books. Or mark <b>None of these</b> (<b>✕</b>: the file is left as it is).</li>
</ol>
<h4>Where the details come from</h4>
<ul>
  <li><b>EPUB</b>: title, authors, ISBN, publisher, language and series (from EPUB 2 or 3 details). With no ISBN in the details, the program looks for it in the text of the first and last pages, where the copyright page is.</li>
  <li><b>MOBI and AZW3</b> (Kindle): title, author, ISBN, publisher and date, from the file's header.</li>
  <li><b>PDF</b>: title and author from the document's details, and the ISBN looked for in the text of the first and last pages. Titles that programs put in by themselves (<code>Microsoft Word - final.docx</code>, <code>Untitled</code>) are ignored.</li>
  <li><b>FB2</b>: title, authors, series, publisher, year and ISBN.</li>
  <li><b>DjVu</b>: the file name only.</li>
  <li><b>File name</b>: understands <code>Author - Title</code> and <code>Title - Author</code> (the search finds out which is which), <code>Title by Author</code>, the year in brackets (<code>(1899)</code>, <code>(2015, Publisher)</code>), the number in a series (<code>Author - 03 - …</code>) and the ISBN. Marks like <code>z-lib.org</code>, <code>libgen</code>, <code>epub</code> and <code>retail</code> are ignored, and <code>_</code> becomes a space.</li>
</ul>
<ul>
  <li><b>Template</b>: <code>{author}</code> (the first one), <code>{authors}</code> (all of them, separated by &amp;), <code>{author sort}</code> (by surname: <i>Surname, Name</i>), <code>{initial}</code> (the surname's first letter), <code>{title}</code>, <code>{original title}</code>, <code>{year}</code> (of the work's first publication), <code>{edition year}</code>, <code>{publisher}</code>, <code>{isbn}</code>, <code>{series}</code>, <code>{volume}</code> (the number in the series, with two digits: <code>03</code>, so the books sort in order) and <code>{id}</code> (Open Library's). The default is <code>{author} - {title} ({year})</code>. Missing values leave nothing behind: without a series, <code>{series} {volume} - {title}</code> becomes just the title.</li>
  <li><b>Swap title and author</b>: the ⇄ button in the review swaps the two fields, for names read the wrong way round.</li>
  <li>The <b>title</b> is the edition's in the language chosen in Options (<b>Language of titles</b>). When the file has another title, from another translation, the file's stays. The original title shows in the review and in <code>{original title}</code>.</li>
  <li>The same book in several formats (<code>.epub</code>, <code>.pdf</code>, <code>.mobi</code>) with the same details makes a single search and gets the same choice.</li>
</ul>
<h4>Folders, cover and metadata.opf</h4>
<ul>
  <li><b>Organize into author and book folders</b>: each book goes into <code>{author}/{title}</code> (the folder template can be changed; each <code>/</code> is a level), as Audiobookshelf expects. In large libraries, <code>{initial}/{author sort}/{title}</code> gives <code>S/Surname, Name/Book Title</code>. A dot at the end of a folder name (<i>Surname, C. S.</i>) goes, since Windows does not accept folders ending in a dot. Folders already named after the author or the book are reused, without new folders inside. Folders left empty are deleted; undo puts everything back.</li>
  <li><b>Save the cover</b>: saves Open Library's cover as <code>cover.jpg</code> in the book's folder; without the folders option, with the book file's name (<code>….jpg</code>).</li>
  <li><b>Save the book's details</b>: saves <code>metadata.opf</code> (or <code>….opf</code>, without the folders option), in OPF format, with title, authors, description, subjects, publisher, year, language, ISBN and series. Audiobookshelf and e-book managers read this file.</li>
  <li>Books whose names are already right get the cover and details too (they count in the <b>Rename</b> button). Existing files are not replaced, and undo deletes what was created. The book file itself is never changed.</li>
</ul>
<h4>Limitations</h4>
<ul>
  <li><b>PDF</b>: many PDFs have empty or wrong document details (the Word file's name, the name of the computer it was made on). A <b>scanned</b> PDF is only a series of images, with no text to find the ISBN in. Then the file name is all that is left: a clear name, like <code>Author - Title</code>, helps a lot; a confusing one leads to the review. It is worth fixing these files' names with the other rules, put above this one in the list: the search reads the name as they leave it.</li>
  <li><b>Without an ISBN</b>, the book is found by title and author, but which edition it is stays unknown: the name uses the publisher and ISBN of the file itself, when it has them, and <code>{edition year}</code> stays empty.</li>
  <li><b>Open Library</b> is kept by volunteers. Recent books, books from small publishers and books from some countries may be missing, or lack a cover or a year, or show up more than once with slightly different details. Many translations are missing: when the title is not found, the review shows the author's other books, where the work may be under its original title, or not there at all. Then mark <b>None of these</b> and rename it by hand.</li>
  <li><b>BrasilAPI</b> only helps books with an ISBN, almost always from Brazilian publishers. It usually has no cover and no year. The year of first publication and the cover come from Open Library when it has the work; otherwise <code>{year}</code> stays empty and there is no cover. Its authors may include the translator or the illustrator: when the file says who the author is, only they are kept.</li>
  <li>Sometimes the <b>ISBN stored in the file belongs to another book</b> (an advert at the end, or a box set), or the catalog has a mistake. When the title and author found do not match the file's, the book is not picked by itself: it goes to the review.</li>
  <li><code>{year}</code> is the year of the work's first publication: a book first published in 1937 gets 1937, even in a 2019 edition. When the catalog gives a year after the edition stored in the file, which is its mistake, <code>{year}</code> stays empty. The year of your edition (<code>{edition year}</code>) is only known when the book is found by its ISBN.</li>
  <li>The <b>series</b> and <b>volume</b> come only from the file's details (EPUB's or FB2's) or from the number in its name; Open Library hardly ever has them.</li>
  <li>Comics (CBZ, CBR) and magazines are not handled by this rule.</li>
</ul>
<p class="tip">Only the searched title, author and ISBN are sent to Open Library, and only the ISBN to BrasilAPI, never the full file name, the folders or the book's content.</p>`,
  },
  {
    id: "r-comic",
    title: "Comic",
    sub: true,
    html: `
<p>Names each comic by series and number. It works two ways: <b>offline</b>, with what the file itself says, and, if you want, <b>looking the series up in a catalog</b> to fill in each issue's name, year, publisher and title.</p>
${ex("Series_Name_012_(2019)_(Digital)_(group).cbz", "Series Name #012 (2019).cbz")}
<div class="notice">
<p><b>Before using it: file names matter</b></p>
<p>Offline, this rule only knows what each file says about itself: its <code>ComicInfo.xml</code>, when there is one, and its name. It understands the usual ways of writing series, volume and number, but it cannot guess what the name does not say. An abbreviation (<code>SrNm 12</code>) or a numbered story without the series' name (<code>07 - Story Name</code>) cannot become <code>Series Name #007</code>.</p>
<p>For a good result, each file needs at least:</p>
<ul>
  <li>the <b>series' name</b> in full, always spelled the same way;</li>
  <li>the issue's <b>number</b> (<code>#12</code>, <code>012</code>) and, if the series had more than one run, the <b>volume</b> (<code>Vol. 2</code>);</li>
  <li>the title or year, if you want to keep them, after " - " or in brackets.</li>
</ul>
<p>Names like these work well: <code>Series Name #012</code>, <code>Series Name Vol. 2 #005</code>, <code>Series Name - Album Name (1990)</code>.</p>
<p>If your files are far from that, put the rules that fix the name above this one in the list: <b>Replace text</b> to turn abbreviations into the full name, <b>Rearrange parts</b> or <b>Regular expression</b> to change the order, <b>Remove range</b> to take out leftovers. The <b>Comic</b> rule and the search read the name as those rules leave it, and the <b>New name</b> column shows the result before renaming.</p>
<p>Two shortcuts: for a folder holding a single series, the <b>The series is the folder's name</b> option needs no other rules; and, when looking it up in a catalog, the <b>Series to look for</b> field finds the right series even from an abbreviated name.</p>
</div>
<h4>Offline</h4>
<ul>
  <li>In <b>CBZ, CB7 and CBT</b> files, the program reads the <code>ComicInfo.xml</code> inside, when there is one: series, number, volume, title, year, publisher, writer and penciller. It is the most reliable data, with one exception: if the <code>ComicInfo.xml</code>'s series is another than the file name's (the English original of a translated edition, for instance), the name's series and title count, since that is what you called the file.</li>
  <li>Without it (and always for <b>CBR, PDF and EPUB</b>), it uses the file name. It understands issue numbers (<code>Series 012</code>, <code>#12</code>, <code>No. 12</code>, <code>Issue 12</code>, <code>#12.1</code>), volumes or tomes (<code>v02</code>, <code>Vol. 3</code>, <code>Vol. III</code>, <code>(Vol.03)</code>, <code>Tome 5</code>, <code>T05</code>, <code>Book 3</code>), manga chapters (<code>Ch. 21</code>, <code>Chapter 21</code>, <code>c045</code>), the year in brackets and the count (<code>(of 6)</code>, <code>#2 of 6</code>). What comes between the volume and the number belongs to the series (<code>Series Vol. 2 Annual #01</code> is the annual, not issue 1). A number followed by a word is part of the name (<code>Name with 714 Words</code>, <code>10 Years</code>), and a leading number followed by a dash is the issue's (<code>19 - Story Name</code>). Marks like <code>(Digital)</code>, <code>(c2c)</code> and group names in brackets are ignored.</li>
  <li>A file with just a number (<code>012.cbz</code>) takes its series from its folder; a folder named <code>Series Name (2012)</code> also gives the series' year, and a folder <code>Vol. 3</code>, the volume.</li>
  <li>In a name that starts with its number (<code>19 - Story Name</code>), the number is the leading one: numbers inside the title do not count.</li>
  <li><b>The series is the folder's name</b>: for a folder of one series, with files that carry the story's number and title (<code>Series Name (Complete)/01 - Story Name</code>). Each file gets the folder's series (without the year and what is in brackets), and what its name had becomes the title: <code>Series Name #001 - Story Name</code>. It is off by default, because in a folder that is not a series' (<code>Downloads</code>) its name would become everyone's series.</li>
  <li>A one-shot or collection without a number gets the series and year (<code>Album Name (2024)</code>), without the marks of the original name.</li>
  <li>Some programs write accented letters as their code between underscores (<code>M_244_nica</code> for <i>Mônica</i>); the program puts them back.</li>
  <li>Files without even a series to be found are left as they are.</li>
</ul>
<h4>Looking it up in a catalog</h4>
<p>In <b>Where to look</b>, choose the catalog:</p>
<ul>
  <li><b>Grand Comics Database</b> (comics.org): made by volunteers, with editions from many countries. It gives the series' name as published, the year, the publisher and the issues' numbers. It asks for the <b>sign-in of a free account</b>: asked when you search and kept <b>only in memory</b> until the program closes; it is never saved. Without an account, the GCD takes only about 30 requests an hour, and may stop taking them.</li>
  <li><b>MangaDex</b> (mangadex.org): manga, with no account or key. It gives the series' name and year (in your language, when there is one, or in English); the volume and chapter stay the file's.</li>
  <li><b>Comic Vine</b> (comicvine.gamespot.com): strong in American comics, with each issue's title. It asks for a free key, pasted in <b>Options</b>.</li>
</ul>
<ol>
  <li>Click <b>Look up the series</b>. Files are grouped by series (or use <b>Series to look for</b> for all of them at once). When only one series has that name (and year) and enough issues for the files' numbers, it is picked by itself; otherwise you choose among the publications, shown with country, language, starting year and number of issues. If the full name finds nothing (<i>Series: Story</i>), the search tries the series alone.</li>
  <li>Each file is matched to its issue by number (or by volume, for manga numbered by volumes; a one-shot, with the series' only issue). Those that do not match get the series' name with their own number, in the same folder as the others, and open the review, with the series' list of issues, to choose the exact issue. <b>None of these</b> brings back the name the file says.</li>
</ol>
<ul>
  <li><b>Ready-made template</b>: <b>Renome default</b> (which also works for Komga, which takes each folder as a series) or <b>Kavita</b> (without the title after " - ", which Kavita does not read). Both fill in the template and folders and turn series folders on, since both servers want each series in its own folder. You can change them freely afterwards.</li>
  <li><b>Template</b>: <code>{series}</code>, <code>{series year}</code>, <code>{number}</code> (three digits: <code>012</code>), <code>{volume}</code> (two), <code>{chapter}</code> (three), <code>{title}</code> (the issue's), <code>{year}</code> (the issue's), <code>{count}</code>, <code>{publisher}</code>, <code>{writer}</code>, <code>{artist}</code> and <code>{id}</code> (the catalog's). The default is <code>{series} v{volume} #{number} c{chapter} - {title} ({year})</code>: a missing value goes along with the mark stuck to it (<code>v</code>, <code>#</code>, <code>c</code>), ending in a dot (<code>Vol. </code>) or the dash before it. So an American issue becomes <code>Series #012 (2019)</code>, a manga volume, <code>Series v03</code>, and a collection with a title, <code>Series v01 - Title (2018)</code>.</li>
  <li><b>Organize into series folders</b>: each comic goes into <code>{series} ({series year})</code> (the folder template can be changed; each <code>/</code> is a level). A comic already in a folder named after its series stays where it is, even if the folder is named another way (<code>Series Name Vol. 2 (1988-1998)</code>, <code>Series Name/Vol. 1</code>, <code>Series Name</code> for a special <code>Series Name - Special</code>, or a slightly different spelling). The comics in the list are arranged together, as a library:<ul><li>each series gets one spelling, the most common in the list (case, accents and one letter apart in a word, as in <code>Apocalipse</code> and <code>Apocalypse</code>, count as the same series), in the folder's name and the files';</li><li>a special or one-shot goes into the folder of the series its name starts with (<code>Series Name - Special</code> into <code>Series Name</code>; when there are several, the one with the longest name);</li><li>one-shots that share what comes before " - " (<code>Series Name - Album One</code>, <code>Series Name - Album Two</code>) go together into a folder named after it;</li><li>the others each go into their series' folder, even with a single file.</li></ul>Undo puts everything back.</li>
</ul>
<h4>Limitations</h4>
<ul>
  <li><b>CBR</b> (RAR) files are renamed from their file name only: there is no RAR reader with a free licence for the program to read what is inside. In practice, almost no CBR has a <code>ComicInfo.xml</code>.</li>
  <li>Very unusual file names may be read wrong (a number that is part of the series' name, for instance). Check the <b>New name</b> column before renaming.</li>
  <li>Catalogs have gaps: Brazilian and other countries' editions may be missing on Comic Vine, and the GCD, made by volunteers, may be incomplete. When the right series is not there, skip it: the files keep the name they say themselves. The search does not always find a series by a translated name; in the picker, you can search again with another name (the original, for instance).</li>
  <li>On the GCD, the review shows only the issues' numbers, without titles or covers: fetching them would take one request per issue. A series with the same name in another language (the French edition, when there is one in yours) is not picked by itself.</li>
  <li>The GCD and Comic Vine count <b>issues and volumes</b>, not chapters: loose manga chapters are not looked up and keep the name read from the file. MangaDex gives the series only.</li>
  <li><b>Request limits</b>: the GCD takes about 25 a minute with an account (about 30 an hour without one); Comic Vine, about 200 an hour, for non-commercial use only. The program uses few per series and keeps the answers while it is open; if you reach a limit, wait a little and search again.</li>
  <li>The comic file is never changed: its <code>ComicInfo.xml</code> is only read.</li>
</ul>
<p class="tip">Only the searched series name and the series' and issues' numbers are sent to the catalogs, never the full file name, the folders or the content. The GCD sign-in goes only to comics.org.</p>`,
  },
  {
    id: "preview",
    title: "Preview and conflicts",
    html: `
<p>The <b>New name</b> column shows the result of the rules right away. In rows that change:</p>
<ul>
  <li>in <b>Current name</b>, what goes away is shown <span class="del">in red, struck through</span>;</li>
  <li>in <b>New name</b>, what comes in is shown <span class="ins">in blue</span>;</li>
  <li>what only moved (the author now before the title, for instance) is shown <span class="mov">with a dotted underline</span>, on both sides.</li>
  <li>a file that moves to another folder gets a folder icon in its new name, and the <b>Folder</b> column shows, in blue, where it goes (<code>→ Artist/Album (2019)</code>).</li>
</ul>
<p>In the first column, ● means the file will be renamed, a red <span class="bad">!</span> is a conflict and a yellow <span class="warn">!</span> is a warning. Hover to see why.</p>
<h4>Editing by hand</h4>
<p>Double-click the new name (or select a file and press <kbd>F2</kbd>) to type any name you want. <kbd>Enter</kbd> confirms and <kbd>Esc</kbd> cancels. A name edited by hand ignores the rules; ↺ goes back to the rules.</p>
<h4>Conflicts</h4>
<p>Files with a conflict are not renamed. The reasons are:</p>
<ul>
  <li>an empty name, or one containing <code>/</code>;</li>
  <li>a name longer than 255 bytes;</li>
  <li>two files would get the same name in the same folder;</li>
  <li>a file with that name already exists outside the list (nothing is overwritten);</li>
  <li>on Windows: the characters <code>\\ : * ? " &lt; &gt; |</code>, or a dot or space at the end of the name.</li>
</ul>
<p>On other systems, those Windows characters only cause a yellow warning, which can be turned off in Options. On Windows, macOS and disks that ignore case (exFAT, FAT, NTFS, as on USB sticks), names that differ only in case count as the same.</p>
<p><b>Export list</b>, in the footer, saves the list as shown (folder, current name, new name, folder it moves into, status and problem) to a CSV file, which opens in any spreadsheet. It helps check a large batch calmly before renaming.</p>
`,
  },
  {
    id: "identical",
    title: "Identical copies",
    html: `
<p>After you add files, the program looks, in the background, for files in the list with <b>identical content</b>, even if their names differ. Nothing is deleted or changed: it is only a notice.</p>
<ul>
  <li>Files with a copy get a tag with a <b>group number</b> next to the name: files with the same number are identical to each other, even with different names.</li>
  <li><b>Select a file</b> and its copies are highlighted in the list. Hovering over the tag shows the names of the other copies.</li>
  <li>The footer shows how many files have a copy. <b>Click it</b> to see only those files, with the copies side by side; click again to see all. From there you can remove them from the list or rename them separately.</li>
  <li>When two copies would get the same name, the conflict says the files are identical.</li>
</ul>
<p>The search runs in stages to be fast: only files of the same size are compared; then only their start and end; a whole file is read only when all of that matches. With large files, the footer shows the progress. Empty files, folders and symbolic links are left out.</p>`,
  },
  {
    id: "apply",
    title: "Renaming and undoing",
    html: `
<p><b>Rename</b> (<kbd>Ctrl</kbd>+<kbd>Enter</kbd>) applies the new names of the visible files that change and have no conflict. The program asks for confirmation first (can be turned off in Options).</p>
<ul>
  <li>Renaming happens in two steps, through a temporary name. That is why names can be swapped (a ↔ b), and no file is ever overwritten.</li>
  <li>Folders and the files inside them can be renamed together: the contents first, then the folder.</li>
  <li>Cue sheets and playlists (<code>.cue</code>, <code>.m3u</code>, <code>.m3u8</code>) that list renamed or moved files are updated with the new names. A <code>.cue</code> listing only files that do not exist (a whole-disc file later split into tracks) is rebuilt with one track per file, matching each <code>TRACK</code> with the song in the folder tagged with that number; if that is not possible, it is deleted. <code>.m3u</code> playlists in which no song exists are deleted too. These are the only changes Renome makes to files; the original content is kept in the history, and undo puts it back.</li>
  <li>When it finishes, a note in the bottom right corner tells how many files were renamed, and goes away by itself.</li>
  <li>If a file fails (no permission, deleted by another program…), the others still go ahead, and at the end you get a list of what failed and why.</li>
  <li>After renaming, the rules are turned off (can be changed in Options), so they are not applied twice.</li>
</ul>
<h4>History</h4>
<p>Each rename is recorded in the <b>History</b>, even after closing the program. There you can:</p>
<ul>
  <li><b>Undo</b>: brings back the previous names.</li>
  <li><b>Try again</b>: shown when part of an undo failed (for example, a file was moved afterwards). Put the file back and try again.</li>
  <li><b>✕</b>: deletes the entry from the history. The files do not change, but that rename can no longer be undone.</li>
  <li><b>Clear history</b>: deletes all entries at once, with the same effect.</li>
</ul>
<p class="tip"><kbd>Ctrl</kbd>+<kbd>Z</kbd> undoes changes to the <i>list</i> (add, remove, move, edit name). To undo a rename that was already done, use the History.</p>
<p>If the program is closed or the computer goes off in the middle of a rename (or an undo), the next time Renome opens it finishes safely: files still under a temporary name get their name from before back, those already renamed go into the <b>History</b>, for you to undo if you want, and those an undo had already taken back leave it. No file is lost.</p>
`,
  },
  {
    id: "recipes",
    title: "Recipes",
    html: `
<h4>Photos with the date they were taken</h4>
<p>Insert → Date the photo was taken, format <code>%Y-%m-%d %H.%M.%S </code>, at the start.</p>
${ex("IMG_1234.jpg", "2024-07-15 14.32.10 IMG_1234.jpg")}
<h4>Number photos from several folders</h4>
<p>Add the folders (with subfolders). Number → template <code>{folder} {n}</code>, 3 digits, restart in each folder.</p>
${ex("Beach/IMG_0001.jpg", "Beach/Beach 001.jpg")}
<h4>Clean up episode names</h4>
<p>Replace text <code>.</code> with a space (Name without extension), then Remove range from text <code> 720p</code> to the end.</p>
${ex("Show.S01E02.720p.WEB.mkv", "Show S01E02.mkv")}
<h4>Consistent extensions</h4>
<p>Extension → lowercase. Add Extension → Set <code>jpg</code> with the filter <code>*.jpeg</code> to unify them.</p>`,
  },
  {
    id: "shortcuts",
    title: "Keyboard shortcuts",
    html: `
<table>
  <tr><td><kbd>Ctrl</kbd>+<kbd>Enter</kbd></td><td>Rename (works even while typing in a rule)</td></tr>
  <tr><td><kbd>Ctrl</kbd>+<kbd>Z</kbd></td><td>Undo the last change to the list</td></tr>
  <tr><td><kbd>Ctrl</kbd>+<kbd>O</kbd></td><td>Add files</td></tr>
  <tr><td><kbd>F2</kbd></td><td>Edit the new name of the selected file</td></tr>
  <tr><td><kbd>↑</kbd> <kbd>↓</kbd> <kbd>Home</kbd> <kbd>End</kbd></td><td>Move through the file list and select (reach it with <kbd>Tab</kbd> or a click); with <kbd>Shift</kbd>, extend the selection</td></tr>
  <tr><td><kbd>Delete</kbd></td><td>Remove the selected files from the list</td></tr>
  <tr><td><kbd>Ctrl</kbd>+<kbd>A</kbd></td><td>Select all</td></tr>
  <tr><td><kbd>F1</kbd></td><td>Open this help</td></tr>
  <tr><td><kbd>Esc</kbd></td><td>Close window or menu</td></tr>
</table>
<p>On macOS, use <kbd>⌘</kbd> instead of <kbd>Ctrl</kbd>.</p>`,
  },
  {
    id: "options",
    title: "Options",
    html: `
<p>The gear opens the options:</p>
<ul>
  <li><b>Language</b> and <b>Theme</b> (light, dark or system).</li>
  <li><b>Remember rules when closing the app</b>.</li>
  <li><b>Include hidden files when adding folders</b>.</li>
  <li><b>Ask for confirmation before renaming</b>.</li>
  <li><b>Turn rules off after renaming</b>.</li>
  <li><b>Warn about names Windows does not accept</b>: characters such as <code>:</code> and <code>?</code>, a dot or space at the end, and reserved names such as <code>CON</code> and <code>NUL</code>. Useful for files headed to a USB stick or to Windows.</li>
</ul>
<p>Options are stored in <code>~/.config/renome</code> (Linux), <code>%APPDATA%\\renome</code> (Windows) or <code>~/Library/Application Support/renome</code> (macOS). The history is in <code>~/.local/share/renome/history</code> on Linux and in the same folder as the options on the other systems.</p>`,
  },
];
