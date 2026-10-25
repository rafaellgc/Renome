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
  <li>The <b>checkbox</b> in the title turns the rule on and off without deleting it.</li>
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
  <li><b>lowercase</b>: <code>The Lord Of The Rings</code> → <code>the lord of the rings</code></li>
  <li><b>UPPERCASE</b>: → <code>THE LORD OF THE RINGS</code></li>
  <li><b>Sentence case</b>: → <code>The lord of the rings</code></li>
  <li><b>Title Case Every Word</b>: → <code>The Lord Of The Rings</code></li>
  <li><b>Title Case, except of/the/and…</b>: → <code>The Lord of the Rings</code>. Short linking words (of, the, and, to, in, de, da, del, la, y…) stay lowercase, except at the start of the name or of a part (after <code> - </code>, <code>:</code> or <code>(</code>).</li>
</ul>
<p>In both Title Case modes, Roman numerals stay in capitals (<code>Rocky III</code>, <code>Part II</code>) and words with capitals inside stay as they are (<code>iPhone</code>, <code>McDonald's</code>).</p>
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
${ex("The.Lord.of.the.Rings.2001.1080p.BluRay.x264.mkv", "The Lord of the Rings - The Fellowship of the Ring (2001) [tmdbid-120].mkv")}
<ol>
  <li>Add the rule and click <b>Search TMDB</b>. The program takes the title and year from each file name (dots become spaces; brackets, parentheses and tags like 1080p, BluRay, x264 are dropped) and searches TMDB.</li>
  <li>When title and year clearly match, the movie is picked automatically (<b>✓</b> next to the new name). When in doubt (<b>?</b>), the review window opens: click the right movie (or press 1–9). The year in the file may be one year off from TMDB; that is taken into account.</li>
  <li>If none fits, fix the title or year in the window's search field, or choose <b>None of these</b> (<b>✕</b>: the file is left as it is).</li>
</ol>
<ul>
  <li><b>Template</b>: <code>{title}</code>, <code>{original}</code> (original title), <code>{year}</code> and <code>{id}</code>. The default is <code>{title} ({year}) [tmdbid-{id}]</code>.</li>
  <li><b>Replace ":" with</b>: Windows does not accept colons; by default they become <code> - </code>. Slashes become dashes and <code>? * &lt; &gt; |</code> are removed.</li>
  <li>Subtitles and other files with the same name as the movie get the same choice. Subtitles keep their language at the end of the name: <code>The Matrix (1999) [tmdbid-603].pt-BR.srt</code>.</li>
  <li><b>Create a folder for each movie</b>: the movie and its subtitles go into a folder with the new name, as Jellyfin recommends: <code>Movies/The Matrix (1999) [tmdbid-603]/The Matrix (1999) [tmdbid-603].mkv</code>. A movie already in the right folder is not moved again. Undo puts the files back and deletes the folder if it is left empty.</li>
  <li>To remove accents, add the <b>Remove characters → Accents</b> rule after this one.</li>
</ul>
<h4>Metadata (.nfo and poster)</h4>
<p>With <b>Save .nfo and poster next to the movie</b>, renaming also creates, next to each video, <code>Name (year) [tmdbid-…].nfo</code> and the poster; optionally the background image (fanart) and the movie's logo too. Inside the movie's folder the images are named <code>poster.jpg</code>, <code>fanart.jpg</code> and <code>logo.png</code>; next to other movies they start with the movie's name (<code>…-poster.jpg</code>). Jellyfin, Kodi and Emby read these files.</p>
<p>The <code>.nfo</code> holds everything TMDB has that Jellyfin reads: title and original title, year and release date, overview and tagline, runtime, rating and votes, the age rating of the chosen language's country (e.g. PG-13), genres, keywords (as tags), countries, studios, collection (e.g. "The Lord of the Rings Collection"), YouTube trailer, director, writers, cast with characters and photos, producers, composer and the TMDB and IMDb ids. The movie file is not changed, existing files are not replaced, and <b>undoing</b> the rename deletes what was created.</p>
<p>Movies whose <b>names are already right</b> get the metadata too: the Rename button counts them and the confirmation says so. If the name already has <code>[tmdbid-…]</code>, the movie is recognized by that id, without searching. That way an organized library can get just the .nfo files and images.</p>
<p class="tip">Only the searched title and year are sent to TMDB, never the full file name or the folders.</p>`,
  },
  {
    id: "r-series",
    title: "TV show (TMDB)",
    sub: true,
    html: `
<p>Gives each episode its official <b>TMDB</b> name, with season, number and title, in the format Jellyfin recognizes. It uses the same API key as the movie rule.</p>
${ex("Breaking.Bad.S01E03.720p.WEB-DL.mkv", "Breaking Bad (2008) - S01E03 - ...And the Bag's in the River.mkv")}
<ol>
  <li>In the <b>Show</b> field, type the name or the TMDB id, or paste the link to the show's page (<code>themoviedb.org/tv/1396-…</code>). Left empty, the program uses the name the files share, or the folder's name.</li>
  <li>Click <b>Find episodes</b>. If more than one show looks alike, choose the right one. The program downloads the whole episode list and looks for each visible file's episode.</li>
  <li>Episodes found with certainty get <b>✓</b>. Doubtful ones (<b>?</b>) and those not found (<b>✕</b>) go to the review window.</li>
</ol>
<h4>How each episode is found</h4>
<ul>
  <li><b>By number</b>, in many forms: <code>S01E02</code>, <code>s1e2</code>, <code>1x02</code>, <code>Season 1 Episode 2</code>, <code>Ep 02</code> (the season comes from the folder, such as <code>Season 2</code>), absolute numbering (<code>Naruto - 125</code>), <code>201</code> (season 2, episode 1) and air date (<code>2024.03.15</code>). Files with two episodes (<code>S01E01E02</code>, <code>S01E01-02</code>) become <code>S01E01-E02</code>.</li>
  <li><b>By title</b>: the text after the number is compared with the titles of every episode, in your language, in English and in the original language, ignoring accents, case and small misspellings. With no number at all, the title alone is enough when it is clear.</li>
  <li><b>When the number and the title disagree</b> (the name says S02E08, but the title is S01E03's), the file gets <b>?</b>, and the review shows both, suggesting the title's.</li>
</ul>
<h4>Review</h4>
<p>Shows what was read from the name, the suggestions (click, press 1–9, or <b>Enter</b> for the first) and every episode of the show by season, with a filter by title or number (<code>s01e03</code>). <b>Ctrl+click</b> marks more than one episode of the same season, for a file with several episodes. Subtitles read the same way get the same choice.</p>
<ul>
  <li><b>Template</b>: <code>{show}</code>, <code>{original}</code> (original name), <code>{year}</code> (first aired), <code>{season}</code>, <code>{episode}</code> (two digits), <code>{title}</code> and <code>{id}</code>. The default is <code>{show} ({year}) - S{season}E{episode} - {title}</code>.</li>
  <li>Untranslated episodes get the English (or original) title instead of "Episode 4".</li>
  <li>Subtitles keep their language at the end of the name: <code>….pt-BR.srt</code>.</li>
  <li>Files not in the chosen show are left as they are. For several shows, filter the list and search one at a time: each file keeps its own.</li>
</ul>
<h4>Show and season folders</h4>
<p>With <b>Organize into show and season folders</b>, each episode goes into the structure Jellyfin recommends: <code>Shows/Breaking Bad (2008) [tmdbid-1396]/Season 01/…</code>. The show's folder is created next to the folder the show is already in (if the files are in <code>Breaking Bad/Season 1</code>, the new folder goes next to <code>Breaking Bad</code>, not inside it); with loose files, inside their folder. The show folder's name has its own template, with the same keys (default <code>{show} ({year}) [tmdbid-{id}]</code>; use <code>{original}</code> for the original name). Old folders left empty are deleted; those that still hold other files stay. <b>Undo</b> puts everything back, making the old folders again.</p>
<h4>Metadata (.nfo and images)</h4>
<p>With <b>Save .nfo and images for Jellyfin</b>, renaming also creates:</p>
<ul>
  <li>in the <b>show's folder</b>: <code>tvshow.nfo</code> (title, original title, year, overview, status, runtime, rating, age rating, genres, keywords, networks and studios, creators, trailer, cast with characters, and the TMDB, IMDb and TVDB ids), <code>poster.jpg</code> and each season's poster (<code>season01-poster.jpg</code>); optionally <code>fanart.jpg</code> and <code>logo.png</code>;</li>
  <li>in the <b>season's folder</b>: <code>season.nfo</code>, with the season's name and overview;</li>
  <li>next to <b>each episode</b>: <code>….nfo</code> (title, season and number, air date, overview, runtime, rating, directors, writers and guest stars) and the thumbnail <code>…-thumb.jpg</code>.</li>
</ul>
<p>The show and season files are only created when the episodes are in a show folder (named after it, or with <code>[tmdbid-…]</code>), the one from before or the one the folders option makes. Episodes whose names are already right get the metadata too. Existing files are never replaced, and undo deletes what was created.</p>
<p class="tip">Only the show's name is sent to TMDB, never file names or folders.</p>`,
  },
  {
    id: "r-music",
    title: "Music (tags)",
    sub: true,
    html: `
<p>Names each song from the <b>tags</b> stored in the file (ID3 in MP3, Vorbis comments in FLAC and OGG, MP4 in M4A, and others), with no internet needed.</p>
${ex("01.mp3", "01 - Shine On You Crazy Diamond.mp3")}
<ul>
  <li><b>Template</b>: <code>{track}</code>, <code>{title}</code>, <code>{artist}</code>, <code>{album artist}</code>, <code>{album}</code>, <code>{year}</code>, <code>{disc}</code>, <code>{genre}</code> and <code>{composer}</code>. The default is <code>{track} - {title}</code>. The track has two digits (three on albums with over 99 tracks).</li>
  <li>Empty tags leave nothing behind: with <code>{track} - {artist} - {title}</code>, a song without an artist becomes <code>01 - Title</code>.</li>
  <li><b>Disc number in the track</b>: on albums with more than one disc, the track becomes <code>1-01</code>, <code>2-01</code>…, so the discs do not mix.</li>
  <li><b>The album as a whole</b>: the tracks of one album (same folder, or <code>CD 01</code>/<code>CD 02</code> discs, and same album name) share the album's details. A track without a year gets the others'. Without an album artist in the tags, the artist of most tracks counts (guests such as <code>feat.</code> left out); with no majority, the album is a compilation: <code>Various Artists</code>, as Jellyfin knows it. So an album's tracks never scatter into different folders.</li>
  <li><b>Bonus tracks</b>: in a folder where most songs are one album's, tracks tagged with another album (a single, a soundtrack) and numbered after the album's (<code>15</code>, <code>16</code> after <code>01</code>–<code>14</code>) take that album.</li>
  <li><b>Singles</b>: a song tagged with the album <code>Single</code> gets its own title as album.</li>
  <li><b>Compilations</b>: on an album where most tracks are by others than the album's artist, the artist goes into the name before the title when the template lacks it: <code>03 - Eric Burdon - Orange and Red Beams</code>.</li>
  <li>If the album name already has a year (<code>Show 12 (2025-05-04)</code>), the template's <code>({year})</code> is left out.</li>
  <li>Songs <b>without a title in their tags</b> are left as they are and get <b>✕</b>; the card shows how many. With <b>Fill in from the file name</b>, the program reads <code>01 - Artist - Title</code>, <code>01. Title</code> or <code>1-05 Title</code> from the name, and the album and year from the folder (<code>Artist - Album (1975)</code>, or just <code>Album</code>; a <code>CD2</code> folder gives the disc). Existing tags come first.</li>
</ul>
<h4>Artist and album folders</h4>
<p>With <b>Organize into artist and album folders</b>, each song goes into <code>{album artist}/{album} ({year})</code> (the folder template can be changed; each <code>/</code> is a level). So <code>Music/Pink Floyd/WYWH/CD1/01.mp3</code> becomes <code>Music/Pink Floyd/Wish You Were Here (1975)/01 - Shine On….mp3</code>: folders already named after the artist, the album or a disc (or inside the artist's folder) are replaced, not filled with new folders. Loose files get the folders inside the folder they are in. Small spelling differences between the folder and the tag do not get in the way. A folder that already gathers albums named like it (<code>Show (1000 - ..)</code> for albums "Show 1037 - …") is taken as a collection: the albums go straight into it, with no artist folders in between.</p>
<p><b>Copies</b>: if two identical copies of an album (found by the program, as in <b>Identical copies</b>) would go to the same place, the first goes ahead and the other stays where it is, marked as a copy.</p>
<ul>
  <li>Lyrics (<code>.lrc</code>) with a song's name get its new name and go with it.</li>
  <li>Covers, <code>.cue</code>, <code>.log</code> and other files in the album's folder go along with most of the folder's songs (songs from other albums that are not bonus tracks follow their own). Subfolders without songs (<code>Artwork</code>, <code>Scans</code>) go along with the same name; so do files in the folder above <code>CD 01</code>/<code>CD 02</code>. If two such files would get the same name (a <code>log.txt</code> on each disc), they get their folder's name: <code>log (CD 01).txt</code>.</li>
  <li>Artist or album names that differ only in case (<code>a-ha</code> and <code>A-Ha</code>, from different albums' tags) become one folder: the one already on disk, or the most used spelling.</li>
  <li>Old folders left empty are deleted; undo puts everything back.</li>
  <li><b>Create a playlist for each album</b>: after renaming, writes <code>&lt;folder name&gt;.m3u8</code> in each album's folder, with its songs (from every disc) in disc and track order. Albums whose names are already right get one too (they count in the <b>Rename</b> button). Not created when the album already has a <code>.cue</code> or a playlist, nor for a single song. Undo deletes the playlist created.</li>
  <li><code>.cue</code> files and <code>.m3u</code> playlists list the songs' new names (see <b>Renaming and undoing</b>).</li>
</ul>
<h4>Fixing with MusicBrainz</h4>
<p>For wrong or incomplete tags, <b>Search MusicBrainz</b> identifies each album on <b>MusicBrainz</b> (musicbrainz.org), an open music database. No key is needed.</p>
<ul>
  <li>The visible songs are grouped by album. If the tags already have the MusicBrainz id, the album is known at once (when every song is on it; otherwise it goes to the review); otherwise the program searches by artist and album name.</li>
  <li>One album often has several editions (original, remaster, deluxe, another country's). The program checks the <b>number of tracks</b>, <b>each one's title</b> and <b>length</b> (a track with the same number but another title and length does not count): if the best edition found fits the songs, it is picked automatically (<b>✓</b>). Otherwise the review opens, with cover, date, country, format, label and track count (green when it equals the folder's), for you to choose, or <b>None</b> to keep the tags.</li>
  <li>With the album chosen, names and folders use MusicBrainz's details: each track's title, artist, album artist, album name and year, in their official spelling. Each song is matched to its track by disc and number (or by title).</li>
  <li><b>Save the album's cover</b>: writes <code>cover.jpg</code> (from the Cover Art Archive) in the album's folder, if it has no cover yet. Undo deletes the cover created.</li>
  <li>Lookups are spaced out (one every 2 seconds), as MusicBrainz asks; each album takes about 4 seconds.</li>
</ul>
<p class="tip">Renome only reads the tags; the songs themselves are not changed. Only the artist and album name are sent to MusicBrainz.</p>`,
  },
  {
    id: "preview",
    title: "Preview and conflicts",
    html: `
<p>The <b>New name</b> column shows the result of the rules right away. In rows that change:</p>
<ul>
  <li>in <b>Current name</b>, what goes away is shown <span class="del">in red, struck through</span>;</li>
  <li>in <b>New name</b>, what comes in is shown <span class="ins">in blue</span>.</li>
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
<p>On other systems, those Windows characters only cause a yellow warning, which can be turned off in Options. On Windows and macOS, names that differ only in case count as the same.</p>`,
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
  <li>If a file fails (no permission, deleted by another program…), the others still go ahead, and at the end you get a list of what failed and why.</li>
  <li>After renaming, the rules are turned off (can be changed in Options), so they are not applied twice.</li>
</ul>
<h4>History</h4>
<p>Each rename is recorded in the <b>History</b>, even after closing the program. There you can:</p>
<ul>
  <li><b>Undo</b>: brings back the previous names.</li>
  <li><b>Try again</b>: shown when part of an undo failed (for example, a file was moved afterwards). Put the file back and try again.</li>
  <li><b>✕</b>: deletes the entry from the history. The files do not change, but that rename can no longer be undone.</li>
</ul>
<p class="tip"><kbd>Ctrl</kbd>+<kbd>Z</kbd> undoes changes to the <i>list</i> (add, remove, move, edit name). To undo a rename that was already done, use the History.</p>`,
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
  <li><b>Warn about characters Windows does not accept</b>: useful for files headed to a USB stick or to Windows.</li>
</ul>
<p>Options are stored in <code>~/.config/renome</code> (Linux), <code>%APPDATA%\\renome</code> (Windows) or <code>~/Library/Application Support/renome</code> (macOS). The history is in <code>~/.local/share/renome/history</code> on Linux and in the same folder as the options on the other systems.</p>`,
  },
];
