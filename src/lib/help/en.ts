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
<p>Hidden files (starting with a dot, like <code>.config</code>, and, on Windows, those marked hidden or system, like <code>desktop.ini</code> and <code>Thumbs.db</code>) are left out when adding folders, unless you turn on the matching option. A hidden folder you add yourself comes in with all it holds.</p>
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
  <li><b>Remove</b> (or <kbd>Delete</kbd>) takes the selected files out of the list; <b>Clear</b> empties it. Files on disk are not touched. What the lookups found for them goes too, and a file added again starts fresh (<kbd>Ctrl</kbd>+<kbd>Z</kbd> brings it all back).</li>
  <li><kbd>Ctrl</kbd>+<kbd>Z</kbd> undoes the last change to the list.</li>
</ul>

<h4>Information columns</h4>
<p>The <b>☰</b> button, at the left end of the list's header, shows up with the rules and turns on columns with what the files hold: with the music rule, <b>Artist</b>, <b>Album</b>, <b>Title</b>, <b>Track</b>, <b>Year</b>, <b>Genre</b>, <b>Length</b> and <b>Format</b> (like <code>MP3 · 320 kbps</code> or <code>FLAC · 24/96</code>); with the movie or TV show one, <b>Resolution</b> and <b>Codec</b>; with the Insert rule using the date the photo was taken, <b>Date taken</b>. With none of these, the button is not there. Your choice is saved and comes back with the rule. Click the title of one to sort the list by it: sorting by artist or year keeps each album in disc and track order. Files without the information go last. <b>Export list</b> includes the columns that are on.</p>
<p><b>Limitations:</b> the information is read from the file itself, only while the column is on; in big folders or on slow disks, sorting by one of these columns the first time takes a while, because each file has to be read. Resolution and codec come only from the video formats Renome reads (MKV, WebM, MP4, MOV, AVI, MPEG and FLV), and the date taken only from photos with EXIF data. With many columns on in a narrow window, the list scrolls sideways.</p>`,
  },
  {
    id: "rules",
    title: "How rules work",
    html: `
<p>Rules are applied <b>from top to bottom</b>, each on the result of the previous one. Order matters: numbering and then changing case is not the same as the other way round.</p>
<ul>
  <li>The <b>checkbox</b> in the title turns the rule on and off without deleting it. While off, it is locked: turn it on to change its options or search.</li>
  <li>The up and down arrows change the order; the <b>×</b> removes the rule; clicking the title collapses the card, which then shows a summary of what the rule does (the text replaced, the template, how many were found).</li>
  <li>Long rules (movie, TV show, music, book, comic, photo, subtitle) are split into titled blocks (<b>Name</b>, <b>Lookup</b>, <b>Files beside</b>, <b>Inside the file</b>…). A long explanation shows two lines; click it to read the rest.</li>
  <li>A rule with an error (an invalid expression, say) gets a red border and is skipped until it is fixed.</li>
  <li><b>One rule by kind of file at a time</b>: movie, TV show, music, book, comic and photo do not go together, since each makes the whole name (the photo one also names videos by date) and one would undo the other's names. The subtitle rule goes in once, and only with movie or TV show (or with none of them, to fix loose subtitles); with music, book, comic or photo it is off. With one of them in the list, the others are off in <b>+ Rule</b> until it is removed (turning it off is not enough). The other rules (replace, number, insert…) go with any of them. A rule set saved before with more than one opens with the first only, and a notice says which were left out. For a folder of movies and shows mixed, rename in two passes.</li>
  <li>An online search (movie, TV show, music, book, comic) can be stopped with <b>Stop</b>, in the progress box. What was found stays, and searching again goes on with the rest. The request under way finishes first, which can take a few seconds.</li>
  <li><b>Pasting a link</b>: when the search by name does not find the right one, paste the link to its page in the catalogue into the search field of the choice window, and that very item comes. Movie: TMDB or IMDb link, or the <code>tt…</code> code. TV show: TMDB id or link, IMDb link, or AniList id/link. Book: an ISBN or the Open Library link. Album (in the album field): the release's MusicBrainz or Discogs link. Comic: the series' link in the catalogue the rule uses (Comic Vine, also an issue's; GCD; MangaDex; AniList; MangaUpdates). Limitations: on Metron only the address with the series' number works (not the one with its name); a link from a catalogue other than the rule's is refused; a number alone is read as a title (only for TV shows is it an id).</li>
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
<p>The <b>Sets</b> button, beside <b>+ Rule</b>, keeps rule sets by name inside the program. <b>Save the current rules…</b> asks for a name and saves; the saved sets show in the list, in alphabetical order, with how many rules they have. Clicking one opens it in place of the current rules. The pencil renames and the bin deletes (deleting does not touch the rules open). Saving with the name of a set that exists asks before replacing it. To take a set to another computer, use <b>Export to a file…</b> and <b>Import from a file…</b> (<code>.json</code> files). By default the program also remembers the rules when it is closed.</p>
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
  <li><b>Accents</b>: <code>Café</code> → <code>Cafe</code>; also special letters, like <code>Ørsted</code> → <code>Orsted</code>, <code>Straße</code> → <code>Strasse</code>, <code>Łódź</code> → <code>Lodz</code></li>
  <li><b>Digits</b>: <code>photo123</code> → <code>photo</code></li>
  <li><b>Number at the start</b>: the track number at the start of the name and the separator after it: <code>01 - Song</code>, <code>01. Song</code> and <code>1-05 Song</code> become <code>Song</code>. Numbers of up to three digits: years (<code>1999 Trip</code>) and dates (<code>2024-07-15 Beach</code>) stay, and so does a name that is only the number.</li>
  <li><b>Characters</b>: each character typed is removed wherever it appears. <code>_#</code> removes every <code>_</code> and <code>#</code>.</li>
  <li><b>Limit to</b> N characters: cuts the name (without the extension) after N characters, removing spaces and dashes left at the end. Useful for car stereos, USB sticks and old programs that do not take long names. With 0 there is no limit. Careful: two long names that only differ at the end may become the same, and the conflict shows in the list.</li>
</ul>`,
  },
  {
    id: "r-convert",
    title: "Convert characters",
    sub: true,
    html: `
<p>Turns characters into others, without deleting anything.</p>
<ul>
  <li><b>Fix broken accents</b>: names made on one system and read with another encoding get their accents turned into pairs of signs: <code>CoraÃ§Ã£o</code>, <code>SÃ£o JoÃ£o</code>, <code>donâ€™t</code>. The option brings back the right text: <code>Coração</code>, <code>São João</code>, <code>don’t</code>. A name that is already right stays as it is.</li>
  <li><b>Other alphabets in Latin letters</b>: Cyrillic (Russian, Ukrainian, Serbian…) and Greek become Latin letters, as they are pronounced: <code>Кино</code> → <code>Kino</code>, <code>Щедрик</code> → <code>Shchedrik</code>, <code>Ελλάδα</code> → <code>Ellada</code>. The rest of the name stays the same. Japanese, Chinese, Arabic and other alphabets are not converted.</li>
  <li><b>Web-safe name</b>: keeps only letters without accents, digits and dots, with <code>-</code> or <code>_</code> between words: <code>Férias em São Paulo (2024)</code> → <code>Ferias-em-Sao-Paulo-2024</code>. For sending files to websites, FTP servers and old devices. It also converts Cyrillic and Greek; letters of other alphabets leave the name. If nothing is left, the name is empty and the list shows the error.</li>
</ul>
<p><b>Limitations</b>: it only fixes the most common fault (UTF-8 text read as Latin-1 or Windows-1252). Names whose accents became <code>?</code> or <code>�</code> lost the letter for good and cannot be fixed. A name that mixes right and broken letters in the same word may stay as it is.</p>
<p>To fix names taken from tags, put this rule below the Music (tags) rule.</p>`,
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
  <li><b>From the content</b>: looks at each file's first bytes and gives it the extension of what it really is. A <code>.jpg</code> that is a PNG becomes <code>.png</code>; a <code>.cbz</code> that is a RAR becomes <code>.cbr</code> (and a <code>.rar</code> holding only pictures becomes <code>.cbz</code> when it is a zip); a file with no extension gets one. Extensions that already fit the content stay: a camera RAW (<code>.nef</code>, <code>.cr2</code>) is TIFF inside and does not become <code>.tif</code>; an <code>.m4b</code> does not become <code>.m4a</code>; UPPERCASE stays UPPERCASE. In a name like <code>tool_1.0_x86[0]</code>, what follows the last dot is not an extension, so the new one is added at the end. When the name already carries, before the wrong extension, one that fits the content, the wrong one is only taken off: <code>movie.nfo.pdf</code> that is text becomes <code>movie.nfo</code>, and <code>photo.png.jpg</code> that is a PNG becomes <code>photo.png</code>. The inner extension stays only if the file really is of that kind: <code>data.json.pdf</code> becomes <code>data.json</code> only when the content is valid JSON, and the same goes for XML, HTML, subtitles and playlists; free text formats (<code>.txt</code>, <code>.nfo</code>, <code>.csv</code>, <code>.log</code>) take any text.</li>
</ul>
<p>Text is handled apart: a text file changes only when its name says it is of a format that is not text (<code>.pdf</code>, <code>.jpg</code>, <code>.mp3</code>…). It then becomes <code>.txt</code>, or <code>.html</code>, <code>.xml</code>, <code>.svg</code>, <code>.json</code>, <code>.srt</code>, <code>.vtt</code>, <code>.ass</code> or <code>.m3u8</code> when its start tells which. A <code>.csv</code>, <code>.md</code>, <code>.log</code> or a text file with no extension (<code>README</code>) stays as it is.</p>
<p><b>Limitations:</b> it knows pictures, audio, video, PDF, books (EPUB, MOBI, FB2, DjVu), archives, Office and LibreOffice documents, fonts and text. Old Office documents (<code>.doc</code>, <code>.xls</code>), programs and databases are not told and stay as they are. Among texts, only the kinds above are told apart: a CSV named <code>.pdf</code> becomes <code>.txt</code>. Nothing is converted: only the name is fixed.</p>
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
  <li>Also, read from inside the video: <code>{resolution}</code> (<code>2160p</code>, <code>1080p</code>, <code>720p</code>, <code>576p</code>, <code>480p</code>; a film cropped wider, like 1920×800, stays <code>1080p</code>) and <code>{video codec}</code> (<code>H.264</code>, <code>HEVC</code>, <code>AV1</code>, <code>VP9</code>, <code>MPEG-4</code>). They are read from MKV, WebM, MP4, MOV, M4V, AVI, MPG, VOB and FLV (by their content, even with the wrong extension), without reading the whole video, and also from files downloaded halfway; in other formats (TS, WMV, RealMedia, ISO) they are empty. Subtitles get their video's. Use them in an optional part: <code>{title} ({year})&lt; - {resolution}&gt;</code> gives <code>Movie (2010) - 1080p.mkv</code>, the name Jellyfin uses to keep several versions of the same movie (with <b>Create a folder for each movie</b>, the versions share one folder without the resolution in its name).</li>
  <li><b>More fields</b>, from TMDB: <code>{genre}</code>, <code>{genres}</code> (up to three), <code>{country}</code>, <code>{language}</code> (the original, like <code>en</code>), <code>{runtime}</code> (minutes), <code>{certification}</code> (the age rating of the titles' language's country: <code>en-GB</code> gives the UK's), <code>{director}</code>, <code>{studio}</code>, <code>{collection}</code> and the release date. Dates: <code>{date}</code> (<code>2010-05-03</code>), <code>{month}</code> (<code>05</code>), <code>{day}</code> and the month's name in the field's language: <code>{month name}</code> gives <code>May</code>, <code>{nome do mês}</code> gives <code>maio</code> and <code>{nombre del mes}</code> gives <code>mayo</code>. From the file's name: <code>{edition}</code> (Director's Cut, Extended, Unrated, IMAX…), <code>{source}</code> (BluRay, Remux, WEB-DL, WEBRip, HDTV, DVD…) and <code>{release group}</code> (<code>-GROUP</code> at the end of a name with a source or resolution). Limitation: TMDB's come right after the search (one request more per movie), and country and studio come as TMDB writes them, mostly in English.</li>
  <li><b>What is already beside it</b>: if the video has an <code>.nfo</code> with the same name (or the folder has a <code>movie.nfo</code>), like those Renome, Jellyfin, Kodi and Emby write, its TMDB id says which movie it is, with no search by name. With only an IMDb id, or a TMDB or IMDb link in the text (like a release's notes), TMDB is asked by it.</li>
  <li><b>Optional parts</b>: a part between <code>&lt; &gt;</code> only shows when every variable in it has a value (the <code>&lt; &gt;</code> signs do not go into the name). <code>&lt;{year} - &gt;{title}</code> gives <code>2010 - Title</code>, or just <code>Title</code> when the year is not known.</li>
  <li><b>Fallback</b>: <code>{a|b}</code> puts in the first of those fields that has a value. <code>{title|year}</code> gives the title, or the year when there is no title. It may have more than two (<code>{a|b|c}</code>) and works inside an optional part.</li>
  <li><b>Digits</b>: <code>{field:N}</code> writes the numbers with N digits. <code>{a:3}</code> gives <code>005</code> instead of <code>05</code>, and <code>{a:1}</code> gives <code>5</code>, with no zeros in front. Decimals (<code>12.1</code>) and the disc before the track (<code>1-05</code>) stay as they are; text without a number does not change.</li>
  <li><b>Replace ":" with</b>: Windows does not accept colons; by default they become <code> - </code>. Slashes become dashes and <code>? * &lt; &gt; |</code> are removed.</li>
  <li>Only videos, subtitles and folders are looked up and renamed; images, <code>.nfo</code> and other files in the list keep their names, unless they are named after the movie or go with it to its new folder (see below). Subtitles with the same name as the movie get the same choice. Subtitles keep their language and the flags <b>forced</b> (only the lines in another language), <b>sdh</b>/<b>cc</b> (for the deaf) and <b>foreign</b> at the end of the name, as Jellyfin, Plex and Kodi read them: <code>Movie Name (1999) [tmdbid-12345].pt-BR.forced.srt</code>. A flag written with a dash, an underscore or a space (<code>Movie-forced.srt</code>) becomes <code>.forced</code>.</li>
  <li><b>Create a folder for each movie</b>: the movie and its subtitles go into a folder with the new name, as Jellyfin recommends: <code>Movies/Movie Name (1999) [tmdbid-12345]/Movie Name (1999) [tmdbid-12345].mkv</code>. A movie already in the right folder is not moved again. If it is in a folder named after it in another form (<code>Movie.Name.1999.1080p</code>), or in a folder of any name holding only it and what goes with a movie (subtitles, images, <code>.nfo</code>, <code>.txt</code>), the new folder goes next to it, not inside (see <b>Old folders that keep files</b>, under Preview and conflicts). Undo puts the files back and deletes the folder if it is left empty.</li>
  <li>What a media server left in the movie's folder (<code>backdrop.jpg</code>, <code>poster.jpg</code>, <code>movie.nfo</code>, <code>extrafanart/</code>) goes with it to the new folder, also when sending to another folder. Files named after the movie (<code>Name.nfo</code>, <code>Name-poster.jpg</code>) get the new name. Limitation: only files in the list, and only from a folder that is the movie's own (named after it, or holding only it and what goes with a movie); in a folder with other things, only the files named after the movie go along.</li>
  <li>To remove accents, add the <b>Remove characters → Accents</b> rule after this one.</li>
</ul>
<h4>Metadata (.nfo and poster)</h4>
<p>With <b>Save .nfo and poster next to the movie</b>, renaming also creates, next to each video, <code>Name (year) [tmdbid-…].nfo</code> and the poster; optionally the background image (fanart) and the movie's logo too. Inside the movie's folder the images are named <code>poster.jpg</code>, <code>fanart.jpg</code> and <code>logo.png</code>; next to other movies they start with the movie's name (<code>…-poster.jpg</code>). Jellyfin, Kodi and Emby read these files.</p>
<p>The <code>.nfo</code> holds everything TMDB has that Jellyfin reads: title and original title, year and release date, overview and tagline, runtime, rating and votes, the age rating of the chosen language's country (e.g. PG-13), genres, keywords (as tags), countries, studios, collection (e.g. "Collection Name"), YouTube trailer, director, writers, cast with characters and photos, producers, composer and the TMDB and IMDb ids. The movie file is not changed, existing files are not replaced, and <b>undoing</b> the rename deletes what was created.</p>
<p>Movies whose <b>names are already right</b> get the metadata too: the Rename button counts them and the confirmation says so. If the name already has <code>[tmdbid-…]</code>, the movie is recognized by that id, without searching. That way an organized library can get just the .nfo files and images.</p>
<h4>Details inside the video</h4>
<p>With <b>Write the details and cover inside the video</b>, when renaming the program writes into <b>MKV</b> and <b>MP4</b> videos what the <code>.nfo</code> would hold: title, original title, tagline, release date, synopsis, genres, keywords, countries, studios, age rating, TMDB score, directors, writers, producers, composers, up to 20 actors with their characters, the collection and the movie's place in it, the trailer link, the IMDb and TMDB codes, the poster as cover and the background picture, the way players (VLC, Kodi, Infuse, the phone's) and media servers read them. Videos that already have the right name get the details too.</p>
<p>Two options come with it:</p>
<ul>
  <li><b>Remove the details that came in the video first</b>: takes out what the file brought (titles with a site's name, comments, ads, old covers and pictures, the names of whoever encoded it). The <b>fonts</b> styled subtitles (ASS, common in anime) use stay, as do each track's technical details (bit rate, duration) and the <b>chapters</b>.</li>
  <li><b>Name the audio and subtitle tracks</b>: each track gets the name of its language, written in that language, with the region when it matters and what the track is: <code>Português (Brasil)</code>, <code>English – SDH</code>, <code>Español (Latinoamérica) – Forzada</code>, <code>English – Commentary</code>. The language comes from the track; when it says none ("und"), from the old name, and the track then says it. Forced, hearing-impaired and commentary tracks told only by their old names also get the right mark in MKV. When two tracks would get the same name (two Portuguese dubs, two English subtitles), they are told apart by what their old names said besides the language (<code>Português – Dublagem clássica</code> and <code>Português – Redublagem</code>; who made the subtitle), then by channels and format (<code>English – 5.1 DTS</code>, <code>English – ASS</code>) and, if nothing tells them apart, by a number (<code>Español – 1</code>, <code>Español – 2</code>). Site addresses and @handles are always taken out. The video track is left without a name.</li>
</ul>
<p>The video is <b>not rewritten</b>: only the details change, in seconds, even in a 20 GB movie. In MKV, the new details (and the fonts and attachments that stay) go to the end of the file, and the old ones become empty space; the tracks' names are changed where they are. In MP4, the video's index is written again in the same place when it fits, or at the end of the file. Before changing anything, the program keeps every byte it changes, and undo brings the file back exactly as it was.</p>
<p><b>Limitations:</b> only complete MKV, WebM and MP4 (with M4V and MOV); AVI, MPEG, TS and others stay as they are, with a warning. <b>The actors' photos do not go</b> inside the video, only their names. <b>Jellyfin, Kodi and Emby prefer the <code>.nfo</code> and their own lookups</b>: the details inside the video serve players such as VLC and Infuse more, and the video copied on its own. MP4 has no place for the background picture or the characters; the cast, directors and writers go in the field Apple TV and Infuse read, and original title, tagline, collection, trailer and keywords go in fields of their own that few players show. In MKV the score goes from 0 to 5 (TMDB's divided by 2), as the format asks. In an MKV without room reserved at the start to point to the new details (rare; mkvmerge always reserves it), nothing is written. The title shown at the top of some players (MKV's "segment title") and the tracks' names are changed only if they fit where they are; if the title does not fit, the old one is removed and the player shows the tags' title; if the names do not fit, the tracks stay as they were and the rest is written. When there is only one track of a language, what its old name said besides the language does not stay (there is nothing to tell apart); a track of an unknown language keeps the words that are not a site's. Cleaning does not touch chapter names. With cleaning or a new cover, the attached fonts are copied to the end of the file, which grows by their size (the old ones become empty space). An MP4 whose index goes to the end no longer starts playing before the download finishes when streamed over the internet (on the computer, nothing changes). Undo only works while the video is not changed by another program. The details come from TMDB in the language chosen in the options; without a poster on TMDB, it goes without a cover.</p>
<h4>Subtitles</h4>
<p>With <b>Download subtitles</b>, the program looks up subtitles for the movies already identified on <b>OpenSubtitles.com</b>, <b>SubDL</b> and <b>SubSource</b> and downloads the chosen ones <b>when renaming</b>, already with the video's new name, as Jellyfin, Plex and Kodi read them: <code>Movie Name (1999).en.srt</code> (<code>.sdh.srt</code> when for the hard of hearing).</p>
<ol>
  <li>In <b>Options → Subtitles</b>, enter the OpenSubtitles API key (and, to download more, the account's user name and password) and/or the SubDL key and/or the SubSource one. All are free. SubSource searches by the IMDb number, which the program asks TMDB for: it is only used when the TMDB key is entered too.</li>
  <li>In <b>Languages</b>, write the languages in the order you like, separated by commas: <code>en, pt-BR</code>. Two-letter codes with or without a country (<code>pt-BR</code>, <code>pt-PT</code>, <code>es</code>, <code>es-MX</code>) or three-letter ones (<code>por</code>, <code>eng</code>) work.</li>
  <li>After <b>Search TMDB</b>, click <b>Search subtitles</b>. Searching does not use up the quota. The review window shows, for each video and language (with the language's name spelled out), the subtitle chosen and the others. Each subtitle's name is that of the video it was made for, so it is often in English even for a subtitle in another language; change it if you like, or choose <b>Don't download</b>.</li>
  <li>Click <b>Rename</b>. Videos whose names are already right get their subtitles too (the button counts them).</li>
</ol>
<ul>
  <li><b>Which comes first</b>: the one marked <b>★</b>, made for this very file (OpenSubtitles recognizes the video by a "fingerprint" taken from its start and end, without sending the file); then those of the release the file's name tells (group, WEB or Blu-ray, resolution, codec); an extended or unrated cut on one side only falls behind. Last, the most downloaded, compared only with those of the same service (OpenSubtitles' counts run far higher and SubDL gives none). On a tie, an OpenSubtitles one without ★ comes after, to spare its few downloads a day. Those for the hard of hearing come a little after the others.</li>
  <li><b>What it has</b>: a language with a subtitle beside the video, with its name (<code>Movie.en.srt</code>, <code>Movie.eng.srt</code>), is skipped. A language found only as a subtitle track inside the MKV (forced ones do not count) shows in the review as <b>has it inside the video</b>, with <b>Don't download</b> chosen; to have a subtitle file beside it too, pick one from the list. A subtitle with just <code>pt</code> counts for <code>pt-BR</code> and <code>pt-PT</code>.</li>
  <li><b>Length</b>: after downloading, the program compares the end of the last line with the video's length. A subtitle ending over a minute after the video, or before its half, is for another version (another cut or speed), and the next one in the list is tried; when none matches, the first is written anyway, with a warning. In a pack of several subtitles (as SubDL's), the episode's is chosen and, among those left, the one named most like the video.</li>
  <li><b>Quota</b>: when the OpenSubtitles quota runs out halfway, the rest are looked for among SubDL's in the same language. When the downloads left today are not enough for all, languages are downloaded in the order asked (the first is the most important) and, for the last ones, SubDL's comes first when there is one, keeping the quota for those only OpenSubtitles has; those that cannot be downloaded are listed in a notice. The review window shows how many downloads are left today (with an account).</li>
  <li>Subtitles are saved as UTF-8 (those in Windows-1252, common in Western languages, are converted). A subtitle of the same name that exists is never replaced. <b>Undoing</b> the rename deletes the subtitles downloaded.</li>
</ul>
<p><b>Limitations:</b> OpenSubtitles limits downloads per day: 5 without an account, from 10 with a free account (more with VIP); the limits of SubDL and SubSource are their own. Without the ★, the subtitle may be early or late against your file, above all if it is from another release; the translation's quality depends on who uploaded it. Subtitles in more than one file (CD1, CD2) and machine-translated ones are left out. Only the subtitle tracks of MKV files are seen, not those of MP4. Only the movie's TMDB number, the languages and the video's fingerprint are sent (to SubSource, the IMDb number; to OpenSubtitles, the user name and password), never the file name.</p>
<h4>Missing movies of the collection</h4>
<p>With <b>Show the collection's missing movies</b> (on by default), when a chosen movie is part of a collection on TMDB (a trilogy, a franchise), the list gets faded rows for the collection's other released movies that are not in it, such as <code>1999 · Title</code>, with the release date, in the order they came out. They are only a notice, like the missing episodes: they are not counted or renamed. <b>Click the total</b> in the bottom bar to save the list to a CSV file (collection, title and release date). To know the collection, the program asks TMDB once more per movie and once per collection.</p>
<p>TMDB decides what is in a collection: sometimes spin-offs are left out, and sometimes movies you would not count as part of it are in.</p>
<h4>Limitations</h4>
<ul>
  <li>The movie is looked up by the title and year in the file name. Heavily abbreviated names, names with initials only or with the title in another language may not be found; in that case, fix the title in the review.</li>
  <li>Movies with the same name (remakes, say) can only be told apart by the year; without a year in the name, the choice usually goes to the review.</li>
  <li>Rare, regional or very recent movies may be missing from TMDB, or lack a translation, an overview or a poster.</li>
</ul>
<p class="tip">Only the searched title and year (and, for collections, the movie's TMDB id) are sent to TMDB, never the full file name or the folders.</p>`,
  },
  {
    id: "r-series",
    title: "TV show (TMDB, AniList)",
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
  <li>Also, read from inside the video: <code>{resolution}</code> and <code>{video codec}</code>, as in the Movie (TMDB) rule. They do not go into the show's folder names.</li>
  <li><b>More fields</b>, from TMDB: <code>{network}</code>, <code>{genre}</code>, <code>{genres}</code>, <code>{language}</code>, <code>{country}</code>, the episode's air date and <code>{absolute}</code>, the episode's number counted from the first, specials left out (handy for anime: <code>{show} - {absolute:3}</code>). Dates: <code>{date}</code> (<code>2010-05-03</code>), <code>{month}</code> (<code>05</code>), <code>{day}</code> and the month's name in the field's language: <code>{month name}</code> gives <code>May</code>, <code>{nome do mês}</code> gives <code>maio</code> and <code>{nombre del mes}</code> gives <code>mayo</code>. From the file's name: <code>{edition}</code>, <code>{source}</code> and <code>{release group}</code> (also <code>[Group]</code> at the start, common in anime).</li>
  <li><b>What is already beside it</b>: a <code>tvshow.nfo</code> in the episodes' folder (or the one above, when they are in a season folder) says which show it is by its TMDB or IMDb id, with no search by name. A show typed in the rule wins.</li>
  <li><b>Optional parts</b>: a part between <code>&lt; &gt;</code> only shows when every variable in it has a value (the <code>&lt; &gt;</code> signs do not go into the name). <code>{show} - S{season}E{episode}&lt; - {title}&gt;</code> only adds the dash when the episode has a title. It also works in the show folder template.</li>
  <li><b>Fallback</b>: <code>{a|b}</code> puts in the first of those fields that has a value. <code>{title|year}</code> gives the title, or the year when there is no title. It may have more than two (<code>{a|b|c}</code>) and works inside an optional part.</li>
  <li><b>Digits</b>: <code>{field:N}</code> writes the numbers with N digits. <code>{a:3}</code> gives <code>005</code> instead of <code>05</code>, and <code>{a:1}</code> gives <code>5</code>, with no zeros in front. Decimals (<code>12.1</code>) and the disc before the track (<code>1-05</code>) stay as they are; text without a number does not change.</li>
  <li>Untranslated episodes get the English (or original) title instead of "Episode 4".</li>
  <li>Subtitles keep their language at the end of the name: <code>….pt-BR.srt</code>.</li>
  <li>Files not in the chosen show are left as they are. Each file keeps its own show; searching again leaves the settled ones alone.</li>
</ul>
<h4>Show and season folders</h4>
<p>With <b>Organize into show and season folders</b>, each episode goes into the structure Jellyfin recommends: <code>Shows/Show Name (2008) [tmdbid-12345]/Season 01/…</code>. The show's folder is created next to the folder the show is already in (if the files are in <code>Show Name/Season 1</code>, the new folder goes next to <code>Show Name</code>, not inside it); with loose files, inside their folder. A folder counts as the show's when it has its name or when every video in it is an episode of that show, whatever it is called (<code>Show Name Complete 1080p</code>); the home folder, Downloads, Videos and the top of a disk never count. The show folder's name has its own template, with the same keys (default <code>{show} ({year}) [tmdbid-{id}]</code>; use <code>{original}</code> for the original name). Old folders left empty are deleted. If the show's old folder still holds other files (extras, images, a file in conflict), it stays next to the new one, renamed <code>Show Name (2008) [tmdbid-12345] (additional files)</code>, so both show up together; before renaming, the bottom bar shows <b>folder(s) with leftovers</b>, and the confirmation says which folder and how many files stay. After renaming, the old folder may be gone (<code>Cartoons/Show Name</code>) and the files show up in the new one next to it (<code>Cartoons/Show Name (2008) [tmdbid-12345]/Season 01/…</code>): <b>nothing was deleted, only moved</b>. <b>Undo</b> puts everything back, making the old folders again.</p>
<h4>Metadata (.nfo and images)</h4>
<p>With <b>Save .nfo and images for Jellyfin</b>, renaming also creates:</p>
<ul>
  <li>in the <b>show's folder</b>: <code>tvshow.nfo</code> (title, original title, year, overview, status, runtime, rating, age rating, genres, keywords, networks and studios, creators, trailer, cast with characters, and the TMDB, IMDb and TVDB ids), <code>poster.jpg</code> and each season's poster (<code>season01-poster.jpg</code>); optionally <code>fanart.jpg</code> and <code>logo.png</code>;</li>
  <li>in the <b>season's folder</b>: <code>season.nfo</code>, with the season's name and overview;</li>
  <li>next to <b>each episode</b>: <code>….nfo</code> (title, season and number, air date, overview, runtime, rating, directors, writers and guest stars) and the thumbnail <code>…-thumb.jpg</code>.</li>
</ul>
<p>The show and season files are only created when the episodes are in a show folder (named after it, or with <code>[tmdbid-…]</code>), the one from before or the one the folders option makes. Episodes whose names are already right get the metadata too. Existing files are never replaced, and undo deletes what was created.</p>
<h4>Details inside the video</h4>
<p>With <b>Write the details and cover inside the video</b>, each MKV or MP4 episode gets the show (with original title, genres, keywords, channel, studios, age rating, cast with characters and trailer), the season, the episode's number and title, the date it aired, the synopsis, the score, the directors, the writers, the guest stars, the season's poster (or the show's) and the show's background picture. The options to remove the old details and name the tracks work as for movies, with the same limitations; not for shows looked up on AniList.</p>
<h4>Subtitles</h4>
<p><b>Download subtitles</b> works as in the movie rule (see <b>Movie (TMDB)</b>), for the episodes already identified: the search is by show, season and episode, and the subtitle gets the episode's new name (<code>Show (2008) - S01E03 - Title.en.srt</code>). Limitations: TMDB only (with AniList, the option goes away); a file with several episodes gets no subtitle; a SubDL season pack is opened and only the episode's subtitle is used, when its name says which it is (<code>S01E03</code>).</p>
<h4>Missing episodes</h4>
<p>With <b>Show the missing episodes</b> (on by default), after the lookup the list gets faded rows for the aired episodes that no video in the list holds, such as <code>S01E16 · Title</code>, with the air date. They sit where the episode belongs, between its neighbours, and the bottom bar says how many are missing. They are only a notice: they cannot be selected and are left out of counts, renaming and the list's export. <b>Click that count</b> to save the missing episodes to a CSV file (show, season, episode, code, title and air date), to open in a spreadsheet. The window opens in the show's folder, with its name in the file name; you can pick another place.</p>
<p>Only seasons the list has at least one video of are checked; whole seasons you do not have are not shown. Episodes not aired yet, or without a date, are not either. An episode with only its subtitle counts as missing.</p>
<h4>Anime on AniList</h4>
<p>In <b>Where to look</b>, <b>AniList</b> looks the anime up on AniList (anilist.co), with no account or key; TMDB and its key are not used. The <b>Show</b> field also takes an AniList id or a link to its page (<code>anilist.co/anime/…</code>). On AniList each season is an entry of its own, linked to the next: the program starts from the first season and joins those after it, in order (season 1, 2…), and the search does not offer later seasons as shows of their own. Movies and specials are left out. Straight-through numbering (<code>Show - 30</code>) is counted across seasons, as with TMDB.</p>
<ul>
  <li><b>Episode titles</b> come from the streaming services' list AniList keeps (<code>Episode 3 - Title</code>), in English; when there is none, the episode has no title, and matching by the title in the file name does not help.</li>
  <li>The <b>air date</b> comes from AniList's schedule (only the last 50 of each season); an episode missing from it takes the date of the one before (several premiere on the same day), and the first one the premiere date.</li>
  <li>The show takes the file's name when it is one of its names on AniList (a translation, say); otherwise the English name, or the main one, in romanized Japanese.</li>
  <li><code>{id}</code> is AniList's, and the template's <code>[tmdbid-{id}]</code> becomes <code>[anilistid-…]</code>, which the AniList plugin for Jellyfin understands. More keys: <code>{genre}</code>, <code>{genres}</code> and <code>{country}</code>.</li>
  <li><b>.nfo files and images</b> are TMDB's only: with AniList, the option goes away.</li>
</ul>
<h4>Limitations</h4>
<ul>
  <li><b>AniList</b>: seasons follow its entries linked as sequels; a second part listed as something else (a special, a separate ONA) is left out, and its files go to the review. Non-commercial use only and about 30 requests a minute (the program waits 2 seconds between them; a show with 6 seasons takes about 12 seconds the first time). Answers are kept 7 days at most.</li>
  <li>A show is only recognized in a file name by the names it has on TMDB (in any language or country) or by what you typed. A nickname or acronym TMDB does not know makes the file count as another show's: type the show, or use that name in the field.</li>
  <li>Seasons are split as TMDB splits them. Anime and shows with absolute numbering are sometimes split differently there, and the numbers in the files do not match; check in the review.</li>
  <li>Specials (season 0) are often numbered differently from their releases and nearly always need a review.</li>
  <li>A file with several episodes is only recognized when they are in the same season.</li>
  <li>Missing episodes follow TMDB's numbering: if your files use another (the DVD order, or anime's running numbers), the list of what is missing may be wrong. Episodes hidden by the filter also count as missing.</li>
</ul>
<p class="tip">Only the show's name is sent to TMDB (or AniList), never file names or folders.</p>`,
  },
  {
    id: "r-music",
    title: "Music (tags)",
    sub: true,
    html: `
<p>Names each song from the <b>tags</b> stored in the file (ID3 in MP3, Vorbis comments in FLAC and OGG, MP4 in M4A, and others), with no internet needed.</p>
${ex("01.mp3", "01 - Song Title.mp3")}
<ul>
  <li><b>Template</b>: <code>{track}</code>, <code>{title}</code>, <code>{artist}</code>, <code>{album artist}</code>, <code>{album}</code>, <code>{year}</code>, <code>{disc}</code>, <code>{genre}</code> and <code>{composer}</code>. The default is <code>{track} - {title}</code>. The track has two digits (three on albums with over 99 tracks). <code>{disc}</code> only has a value on albums of more than one disc: on an album of one disc, it is empty.</li>
  <li>Also: <code>{total tracks}</code>, <code>{total discs}</code>, <code>{original year}</code> (the album's first release; without it, the <code>{year}</code>), <code>{label}</code>, <code>{catalog}</code> (the catalog number), <code>{artist sort}</code> and <code>{album artist sort}</code> (how the artist is sorted: <i>Beatles, The</i>; without that tag, the name itself) and <code>{initial}</code> (the first letter of the album artist sort, for folders like <code>{initial}/{album artist sort}</code>). These come from the tags or, when the album is found on MusicBrainz, from the release. From the file itself: <code>{format}</code> (<code>FLAC</code>, <code>MP3</code>…), <code>{bitrate}</code> (in kbit/s), <code>{sample rate}</code> (in kHz: <code>44.1</code>, <code>96</code>) and <code>{bits}</code> (<code>16</code>, <code>24</code>; lossy formats such as MP3 have none). Example: <code>{album} &lt;[{format} {bits}-{sample rate}]&gt;</code> gives <code>Album [FLAC 24-96]</code>.</li>
  <li><b>Release date</b>, from the tags (or from MusicBrainz, when the album is identified). Dates: <code>{date}</code> (<code>2010-05-03</code>), <code>{month}</code> (<code>05</code>), <code>{day}</code> and the month's name in the field's language: <code>{month name}</code> gives <code>May</code>, <code>{nome do mês}</code> gives <code>maio</code> and <code>{nombre del mes}</code> gives <code>mayo</code>. Limitation: many files have only the year; month and day are then empty.</li>
  <li><b>Optional parts</b>: a part between <code>&lt; &gt;</code> only shows when every variable in it has a value (the <code>&lt; &gt;</code> signs do not go into the name). In the folder template, <code>{album artist}/{album}/&lt;CD{disc}&gt;</code> makes a <code>CD1</code>, <code>CD2</code>… folder only on albums of more than one disc.</li>
  <li><b>Fallback</b>: <code>{a|b}</code> puts in the first of those fields that has a value. <code>{title|year}</code> gives the title, or the year when there is no title. It may have more than two (<code>{a|b|c}</code>) and works inside an optional part.</li>
  <li><b>Digits</b>: <code>{field:N}</code> writes the numbers with N digits. <code>{a:3}</code> gives <code>005</code> instead of <code>05</code>, and <code>{a:1}</code> gives <code>5</code>, with no zeros in front. Decimals (<code>12.1</code>) and the disc before the track (<code>1-05</code>) stay as they are; text without a number does not change.</li>
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
<p>With <b>Organize into artist and album folders</b>, each song goes into <code>{album artist}/{album} ({year})</code> (the folder template can be changed; each <code>/</code> is a level). So <code>Music/Artist/Album/CD1/01.mp3</code> becomes <code>Music/Artist/Album (1975)/01 - Song Title.mp3</code>: folders already named after the artist, the album or a disc (or inside the artist's folder) are replaced, not filled with new folders. Loose files get the folders inside the folder they are in, unless it holds songs of one album only: then it counts as the album's folder, whatever its name (<code>Music/New folder/01.flac</code> becomes <code>Music/Artist/Album (1975)/01 - Song Title.flac</code>), and it is deleted if left empty. The home folder, Music, Downloads and the other system folders never count. Small spelling differences between the folder and the tag do not get in the way. A folder that already gathers albums named like it (<code>Show (1000 - ..)</code> for albums "Show 1037 - …") is taken as a collection: the albums go straight into it, with no artist folders in between.</p>
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
  <li><b>Include the cover in the files</b>: takes out the pictures already inside each song and puts in the album's cover (from the Cover Art Archive or, if there is none, the folder's <code>cover.jpg</code>). In <b>Cover size</b>, pick 500 or 250 px for car stereos and old players that freeze on large covers or do not show them; the songs also get smaller. The <code>cover.jpg</code> saved in the folder stays large. Limitation: a cover taken from the folder (when the Cover Art Archive has none) goes in at whatever size it has, without being reduced.</li>
  <li>The writing is done on a copy, which only takes the file's place at the end; a crash midway does not spoil the song. If the program is closed or the computer goes off while writing, the next time Renome opens the songs already written go into the <b>History</b>, to undo, and the copy being written is deleted. Undo puts the old tags and pictures back (in MP3 and M4A, rare fields cleaning took out that are not text, such as other programs' binary data, and the ID3v1 and APE tags do not come back). The modification date changes, so players read the tags again. Since the content changes, a torrent no longer recognises these files.</li>
  <li>Lookups are spaced out (one every 2 seconds), as MusicBrainz asks; each album takes about 4 seconds.</li>
</ul>
<h4>Discogs (optional)</h4>
<p>Many regional, independent and vinyl releases are on <b>Discogs</b> (discogs.com) and not on MusicBrainz. To use it, create a free Discogs account, generate a <b>personal token</b> under Settings → Developers and paste it in <b>Options → Discogs</b>.</p>
<ul>
  <li>With the token, when MusicBrainz finds nothing for an album, the program searches Discogs by artist and album name. The edition is checked the same way (number of tracks, titles and lengths): if it fits, it is chosen by itself; if not, it goes to the review. Discogs candidates carry a <b>Discogs</b> label.</li>
  <li>In the review, next to <b>Search</b>, pick <b>MusicBrainz</b> or <b>Discogs</b> to search the other source, even when the first one found something.</li>
  <li>Names, folders, the cover (<code>cover.jpg</code>), missing tracks and writing tags work the same, with Discogs' data. Discs come from the track positions (<code>1-05</code>, <code>CD2-3</code>); vinyl sides (<code>A1</code>, <code>B2</code>) are numbered as one disc. The parts of a work (<code>Sonata: I. Allegro</code>) count as tracks.</li>
  <li>Only the artist and album name go to Discogs, with your token. The token is stored only on this computer, in the system's password store.</li>
</ul>
<p><b>Discogs limitations:</b> it is asked only when MusicBrainz finds nothing (or when you choose it in the review); identifying by sound does not use Discogs. Tags written from Discogs have fewer fields: title, artists, album, dates, track, disc, label, catalog number, barcode and genres (Discogs' styles first); there are no ids, composers, producers or ISRCs, so a later search looks the album up again. The cover is Discogs', usually about 600 pixels: <b>Cover size</b> can shrink it (500 or 250), but Large stays at 600. The country and year shown in the review are the search's; the number of tracks shows only after the edition is opened. Discogs allows about 60 requests a minute; each album uses 2 or 3.</p>
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
<h4>Edit the tags by hand</h4>
<p>For what MusicBrainz and Discogs do not find or get wrong (home recordings, self-made compilations, rare releases), select the songs in the list and click <b>Edit the details</b> in the rule. In the window, type title, artist, album artist, genre, album, year, track, total tracks, disc and total discs. What you type counts instead of what the song and the catalogue say: the new name is made from it and, with <b>Write the tags into the files</b>, it goes into the tags (the cover only comes from the folder). With several selected, only the fields you change count for all (handy to set the same album, year or genre at once); a field with different values shows empty and stays as each one has it. Changing the album or the album artist, or editing a song that was not found, takes the MusicBrainz ids out of the tags, since they would name another release; changing only the title, track or genre of a found song keeps them. Limitations: typed details last while the program is open and the files are in the list (like the searches); <b>Clear</b> forgets them and Ctrl+Z does not undo them; other fields (composer, label, lyrics) are not edited here; without <b>Write the tags into the files</b>, only the name changes.</p>
<h4>Missing tracks</h4>
<p>With <b>Show the missing tracks</b> (on by default), in the albums identified on MusicBrainz the list gets faded rows for the chosen release's tracks that no song in the list has, such as <code>05 · Title</code> (<code>2-05</code> on albums with more than one disc), in their place, between their neighbours. They are only a notice: they are not counted or renamed. <b>Click the total</b> in the bottom bar to save the list to a CSV file (album, disc, track and title). No extra request is made: the track list is the one already used to identify the album.</p>
<p>The list is the <b>chosen release's</b>: a release with bonus tracks shows the bonus tracks you do not have; another release of the same album may not have them. In the review you can pick the release that matches yours. A song that matched no track of the release makes its track show up as missing.</p>
<h4>Limitations</h4>
<ul>
  <li>Without the MusicBrainz search, names depend on the tags alone: wrong tags make wrong names.</li>
  <li>Regional, independent, bootleg or very recent releases may be missing from MusicBrainz; then the tags are used, unless Discogs (with your token) has them.</li>
  <li>By sound, very similar recordings (a remaster, an artist's own re-recording) may be mixed up. When the album is in doubt, it goes to the review.</li>
  <li>Songs in Opus, WMA and other formats not listed above are not listened to.</li>
  <li>For MP3 and M4A, undoing written tags does not bring back rare fields that are not text, or ID3v1 and APE tags.</li>
  <li>In MP3, several values of a field (genres, composers…) go into one (<code>Rock; Pop</code>), because the format used so Windows reads the tags (ID3v2.3) keeps one value a field.</li>
</ul>
<p class="tip">Without <b>Write the tags into the files</b>, Renome only reads the tags; the songs themselves are not changed. Only the artist and album name are sent to MusicBrainz.</p>`,
  },
  {
    id: "r-book",
    title: "Book (Open Library, Google Books)",
    sub: true,
    html: `
<p>Names each book after <b>Open Library</b> (openlibrary.org), an open book catalog run by the Internet Archive. When Open Library does not know an ISBN, it is looked up on <b>BrasilAPI</b> (brasilapi.com.br), which gathers the records Brazilian publishers keep at the CBL. Neither needs a key. With a free <b>Google Books</b> key, put in <b>Options → Google Books</b>, books Open Library does not settle are looked up there too (see below).</p>
${ex("book_title_author.pdf", "Author Name - Book Title (1937).pdf")}
<ol>
  <li>Add the rule and click <b>Search Open Library</b>. For each book (EPUB, MOBI, AZW3, PDF, FB2 and DjVu), the program reads the details stored in the file itself: title, author, ISBN, publisher and series. Whatever is missing comes from the file name.</li>
  <li>With the <b>ISBN</b>, the book is found right away, in its exact edition (<b>✓</b>), on Open Library or BrasilAPI. Without one, the program searches by title and author: when they match with no doubt, the book is picked by itself; otherwise (<b>?</b>), the review opens, with cover, authors, original title and number of editions, for you to choose (click, or press 1–9).</li>
  <li>The review shows the <b>file's cover</b> in its corner (the EPUB's cover, or the PDF's first page) and compares it with each candidate's cover: those with the same art get the <b>same cover</b> badge. An edition with another cover (other art, another publisher) does not get it, even when it is the right book.</li>
  <li>If none fits, fix the title or the author in the window and search again. You can also search by author alone, with the title empty, to see their books. Or mark <b>None of these</b> (<b>✕</b>: the file is left as it is).</li>
</ol>
<h4>Google Books</h4>
<p>Google Books is optional and needs a free key: create a Google Cloud project, turn the <b>Books API</b> on and create an API key. The order matters: turn the Books API on first (in <b>APIs &amp; Services → Library</b>, search for "Books API" and click <b>Enable</b>), because the key's <b>API restrictions</b> field only lists the APIs already on in the project. Then, in <b>Credentials → Create credentials → API key</b>, pick the Books API in the restrictions and leave <b>Application restrictions</b> at <b>None</b> (the other choices are for websites, IP addresses and phone apps). It needs no service account and no card. With it:</p>
<ul>
  <li>an ISBN Open Library does not know is looked up on Google Books first, then on BrasilAPI; the edition found comes with publisher, year, pages, language, cover and description;</li>
  <li>a book without an ISBN that Open Library does not pick by itself is also searched by title and the author's surname, editions in the titles' language first. When Google finds the same book as Open Library (title and author matching), it only fills what is missing: publisher, pages, ISBN, cover and description; the rest comes from Open Library. The other books it finds join the review after Open Library's, marked <b>Google Books</b>. If Open Library finds nothing, a Google book that matches beyond doubt is picked by itself;</li>
  <li>the cover saved comes from Open Library; only when it has none, from Google Books, at the largest size it gives (about 800 pixels wide). Google's description goes into <code>metadata.opf</code> when Open Library has none.</li>
</ul>
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
  <li><b>More fields</b>: <code>{language}</code> (<code>pt</code>, <code>en</code>) and <code>{pages}</code> (from Open Library: the edition's when found by ISBN, else the usual count among the editions). Limitation: the translator is not available, since Open Library does not give it.</li>
  <li><b>What is already beside it</b>: an <code>.opf</code> with the book's name, or the <code>metadata.opf</code> of a folder holding only that book (like those Renome and library managers write), counts more than the file's own details: title, authors, ISBN, year, publisher, language and series come from it.</li>
  <li><b>Optional parts</b>: a part between <code>&lt; &gt;</code> only shows when every variable in it has a value (the <code>&lt; &gt;</code> signs do not go into the name). <code>{author} - &lt;{series} {volume} - &gt;{title}</code> gives <code>Author - Series 03 - Title</code> for books in a series and <code>Author - Title</code> for the others. It also works in the folder template.</li>
  <li><b>Fallback</b>: <code>{a|b}</code> puts in the first of those fields that has a value. <code>{title|year}</code> gives the title, or the year when there is no title. It may have more than two (<code>{a|b|c}</code>) and works inside an optional part.</li>
  <li><b>Digits</b>: <code>{field:N}</code> writes the numbers with N digits. <code>{a:3}</code> gives <code>005</code> instead of <code>05</code>, and <code>{a:1}</code> gives <code>5</code>, with no zeros in front. Decimals (<code>12.1</code>) and the disc before the track (<code>1-05</code>) stay as they are; text without a number does not change.</li>
  <li><b>Swap title and author</b>: the ⇄ button in the review swaps the two fields, for names read the wrong way round.</li>
  <li>The <b>title</b> is the edition's in the language chosen in Options (<b>Language of titles</b>). When the file has another title, from another translation, the file's stays. The original title shows in the review and in <code>{original title}</code>.</li>
  <li>The same book in several formats (<code>.epub</code>, <code>.pdf</code>, <code>.mobi</code>) with the same details makes a single search and gets the same choice.</li>
</ul>
<h4>Folders, cover and details</h4>
<ul>
  <li><b>Organize into author and book folders</b>: each book goes into <code>{author}/{title}</code> (the folder template can be changed; each <code>/</code> is a level), as Audiobookshelf expects. In large libraries, <code>{initial}/{author sort}/{title}</code> gives <code>S/Surname, Name/Book Title</code>. A dot at the end of a folder name (<i>Surname, C. S.</i>) goes, since Windows does not accept folders ending in a dot. Folders already named after the author or the book are reused, without new folders inside. A folder holding one book only (in one or more formats) counts as that book's, whatever its name: the new folders go next to it, not inside. Folders left empty are deleted; undo puts everything back.</li>
  <li><b>Save the cover</b>: saves Open Library's cover (or, when it has none, Google Books') as <code>cover.jpg</code> in the book's folder; without the folders option, with the book file's name (<code>….jpg</code>).</li>
  <li><b>Save the book's details</b>: saves <code>metadata.opf</code> (or <code>….opf</code>, without the folders option), in OPF format, with title, authors, description, subjects, publisher, year, language, ISBN and series. Audiobookshelf and e-book managers read this file.</li>
  <li>Books whose names are already right get the cover and details too (they count in the <b>Rename</b> button). Existing files are not replaced, and undo deletes what was created. Without the option below, the book file itself is never changed.</li>
  <li><b>Write the details inside the book</b>, in <b>EPUB</b>, <b>AZW3</b>, <b>MOBI</b> and <b>FB2</b>: in EPUB, title, authors, year, publisher, language, ISBN and series go into the EPUB's own details (the OPF file inside it), which e-book readers and managers show. Only the details that change are replaced; the rest stays as it was: description, subjects, illustrators, translators, the cover and the book's own identifier (which readers such as Kobo use to recognise it). A new ISBN goes beside that identifier, not in its place. The series is written in the two most read ways (the old one, and EPUB 3's in EPUB 3 books). The text, images and the rest of the book are copied as they are. Books whose names are already right get the details too, and those that already have the same details are skipped.</li>
  <li>In <b>AZW3</b> and <b>MOBI</b>, the same details go into the book's header (the EXTH records and the book's name), which Kindles and managers read; in a book holding both versions (MOBI and KF8), into both. Text and pictures do not change. Series have no place in these formats. In <b>FB2</b>, authors, title, date, language and series go into <code>title-info</code>, and publisher, year and ISBN into <code>publish-info</code>, in the order the format wants; the synopsis, genres, cover and the rest stay, and the file keeps the encoding it was in (like Windows-1251). In these three formats the book is written anew whole (it is small) and the original is kept in the History until undo. Books with DRM are never touched.</li>
  <li><b>Upgrade EPUB 2 to EPUB 3</b>: an old EPUB gets EPUB 3's navigation page, made from the table of contents (NCX) it has, the modification date and the cover marked as EPUB 3 wants; the old table of contents stays, for old readers. Text, styles, fonts and pictures do not change. An EPUB that is already 3 stays as it is.</li>
  <li><b>Shrink large EPUB pictures</b>: JPEG and PNG pictures over 1600 pixels on their longest side become 1600, and JPEGs over 300 KB are compressed again (quality 85); each picture is only replaced if it gets at least 10% smaller. Picture names do not change, so the book still points to them. Good to email to a Kindle (which limits the size) or for readers with little space.</li>
  <li><b>Limitations</b> of these three options: a shrunk picture loses detail (maps and fine illustrations may become harder to read); a GIF, an SVG or a picture in a rare format is not touched; an EPUB 3 made this way is not checked by an official validator; the original of a rewritten book is kept in the History (taking the same space until deleted with the entry); over 1 GB, the book stays as it is.</li>
  <li>The writing is done in a copy that takes the book's place only at the end; if the program is closed halfway, what was already written goes to the <b>History</b>. Undo puts the old details back exactly as they were. As the content changes, torrents no longer recognise these files.</li>
</ul>
<h4>Edit the details by hand</h4>
<p>For what no catalogue finds or gets wrong (independent editions, course notes, fan translations), select the books in the list and click <b>Edit the details</b> in the rule. In the window, type title, authors (separated by <code>;</code>), year, publisher, language (<code>pt</code>, <code>en</code>…), series, number in the series and ISBN. The window comes filled with what the catalogue found or, with no search, with what the book says about itself (details inside the file and name). What you type counts instead: the new name is made from it; with <b>Write the details inside the EPUB</b>, it goes inside the EPUB and, with <b>Save the book's details</b>, into the <code>metadata.opf</code>. With several selected, only the fields you change count for all; a field with different values shows empty and stays as each one has it. Changing the title or the authors stops using the catalogue's book: its cover and description are not saved. Limitations: typed details last while the program is open and the files are in the list (like the searches); <b>Clear</b> forgets them and Ctrl+Z does not undo them; there is one year only (the edition's and the original become the same); other fields (description, translator, subjects) are not edited here; in PDF, MOBI and other formats, only the name and the <code>metadata.opf</code> change.</p>
<h4>Missing volumes</h4>
<p>With <b>Show the series' missing volumes</b> (on by default), after the search, for books in a series the list gets faded rows for the volumes missing <b>between yours</b>, from 1 to the highest number you have, such as <code>3</code>, in their place. They are only a notice: they are not counted or renamed. <b>Click the total</b> in the bottom bar to save the list to a CSV file (series and volume).</p>
<p>No catalogue has a reliable list of each series' books, so the program only sees the gaps between your files: what comes after your last volume does not show up. The series comes from the book's details (EPUB, FB2); the number, from them or the file name (<code>Author - 03 - Title</code>). Books without a series in their details are left out. Volumes such as <code>2.5</code> (stories between two books) neither fill nor open a gap.</p>
<h4>Converting to another format</h4>
<p>In <b>Convert to</b>, choose <b>EPUB</b>, <b>AZW3</b> (for Kindle), <b>FB2</b> or <b>PDF</b>. After renaming, each book in another format is converted: EPUB, MOBI, AZW, AZW3, PRC, PDF, FB2 and FBZ, and also Word (DOCX) and LibreOffice (ODT) documents, texts (TXT) and pages (HTML, HTMLZ), which join the rule once a format is chosen. The list already shows the name with the new extension. The original stays beside with its own extension (<code>Name.docx</code>) or goes to the Trash, as set in <b>The original file</b>; undo puts the original back when it stayed beside. The details found in the catalogue or typed in go inside the new book. A book already in the chosen format stays as it is.</p>
<p>What goes along: headings become chapters and the table of contents (the book's own, when it has one); italics, bold, underline, small capitals and superscripts; lists, tables, quotes, poems and epigraphs; footnotes, linked from their mark and back; pictures and the cover; centred and right-aligned paragraphs. Fonts, letter sizes in points and fixed margins are left out, for the reader to adjust. From an EPUB to another format, the book's own style goes along.</p>
<p><b>From PDF</b>: a PDF only keeps where each letter is on the page, so the text is rebuilt. Letters become words and lines, lines become blocks, and columns are put in reading order. Repeated headers, footers and page numbers go. Paragraphs are made again from indents, short last lines and the space between them; a word broken with a hyphen at a line's end is joined; a paragraph cut by a page goes on. Headings come from the letter size and the PDF's bookmarks, which also cut the chapters. A scanned PDF without text becomes a book of fixed pages, each page a picture. A page whose text its fonts do not let be read (symbols instead of letters) goes in as a picture.</p>
<p><b>To PDF</b>: the book is laid out in A5 pages (justified text, each chapter on a new page) and printed by the system's own web view, in a window that does not show; the PDF gets the title, authors and language.</p>
<h4>DRM</h4>
<p><b>Renome never converts books locked by DRM, and never tries to open or get around that lock</b>: the law forbids it (in the United States, the DMCA; in Brazil, Law 9,610/98, art. 107). A book with DRM gets the <b>DRM</b> mark in the list and is not converted; its name can still change. Recognised: EPUBs with encrypted content (Adobe, Apple, Readium LCP and others), Kindle books with encryption (MOBI, AZW, AZW3) and KFX, and locked PDFs, even those that open without a password but limit what can be done with them. Scrambled fonts in an EPUB are not DRM: the book is converted, without them. Converting a DRM-free book to read on your own device is personal use; what you do with the files is your responsibility.</p>
<h4>Limitations</h4>
<ul>
  <li><b>PDF</b>: many PDFs have empty or wrong document details (the Word file's name, the name of the computer it was made on). A <b>scanned</b> PDF is only a series of images, with no text to find the ISBN in. Then the file name is all that is left: a clear name, like <code>Author - Title</code>, helps a lot; a confusing one leads to the review. It is worth fixing these files' names with the other rules, put above this one in the list: the search reads the name as they leave it.</li>
  <li><b>Without an ISBN</b>, the book is found by title and author, but which edition it is stays unknown: the name uses the publisher and ISBN of the file itself, when it has them, and <code>{edition year}</code> stays empty.</li>
  <li><b>Open Library</b> is kept by volunteers. Recent books, books from small publishers and books from some countries may be missing, or lack a cover or a year, or show up more than once with slightly different details. Many translations are missing: when the title is not found, the review shows the author's other books, where the work may be under its original title, or not there at all. Then mark <b>None of these</b> and rename it by hand.</li>
  <li><b>BrasilAPI</b> only helps books with an ISBN, almost always from Brazilian publishers. It usually has no cover and no year. The year of first publication and the cover come from Open Library when it has the work; otherwise <code>{year}</code> stays empty and there is no cover. Its authors may include the translator or the illustrator: when the file says who the author is, only they are kept.</li>
  <li><b>Google Books</b> needs your key and takes about 1,000 requests a day; each book uses one to four. Once the quota is used up, the program goes on with Open Library and BrasilAPI only until the next day. Its answers are kept only while the program is open: Google's terms do not allow keeping them on disk. It does not give the series or the year of first publication, and it sometimes merges different editions into one record, with mixed details; covers may be missing or small. A book found only there, by its title, gets no <code>{year}</code>, and its publisher and ISBN are not used, since the file may not be that edition.</li>
  <li>Sometimes the <b>ISBN stored in the file belongs to another book</b> (an advert at the end, or a box set), or the catalog has a mistake. When the title and author found do not match the file's, the book is not picked by itself: it goes to the review.</li>
  <li><code>{year}</code> is the year of the work's first publication: a book first published in 1937 gets 1937, even in a 2019 edition. When the catalog gives a year after the edition stored in the file, which is its mistake, <code>{year}</code> stays empty. The year of your edition (<code>{edition year}</code>) is only known when the book is found by its ISBN.</li>
  <li>The <b>series</b> and <b>volume</b> come only from the file's details (EPUB's or FB2's) or from the number in its name; Open Library hardly ever has them.</li>
  <li>Comics (CBZ, CBR) and magazines are not handled by this rule.</li>
  <li>Details are only <b>written inside EPUB</b> files. MOBI, AZW3, PDF and FB2 stay as they are (the <code>metadata.opf</code> beside them serves them). A damaged EPUB is not changed and shows up in the error list. An EPUB with DRM is skipped too, since the reader could refuse the changed book.</li>
  <li><b>Converting PDF</b>: the text is rebuilt from where the letters are, so columns, tables, formulas, captions and text around pictures can come out in the wrong order or mixed. Tables become lines of text. A footnote is only recognised when its mark is raised in the text and the note is at the foot of the same page; notes at the end of the book stay as plain text. A word with a real hyphen broken at a line's end (<code>well-</code> / <code>known</code>) loses the hyphen when the book does not have that whole word elsewhere. Vertical or right-to-left text is not handled. There is no text recognition (OCR): a scanned PDF becomes fixed pages, which do not fit the screen.</li>
  <li><b>To AZW3</b>: the file follows the KF8 format and is checked by reading it back; it has been tried on a Kindle Paperwhite (text books and comics), but not on every Kindle model. Newer Kindles also take EPUB through Amazon's sending.</li>
  <li><b>From Kindle books</b>: fonts embedded in AZW3 are left out. Books with the dictionary compression (HUFF/CDIC) are read, but that path has been little tested.</li>
  <li><b>To FB2</b>: the format keeps no styles, so colours, alignments and sizes are lost; italics, bold, headings, notes, poems, quotes, simple tables and pictures (but SVG) stay.</li>
  <li><b>From DOCX and ODT</b>: text boxes go after their paragraph; comments and text deleted with tracked changes are left out; equations and vector drawings (WMF, EMF) do not show.</li>
  <li><b>From TXT</b>: chapters are recognised by common names (<code>Chapter 1</code>, <code>Part I</code>, <code>Prologue</code>…), and italics only when marked as <code>_this_</code>.</li>
  <li><b>To PDF</b>: Linux only, for now. Hyphenation depends on the system's dictionaries; without them, justified text can have wide spaces. Pages are A5, without page numbers. It takes a few seconds per book.</li>
  <li>DjVu and KFX are not converted.</li>
</ul>
<p class="tip">Only the searched title, author and ISBN are sent to Open Library and Google Books, and only the ISBN to BrasilAPI, never the full file name, the folders or the book's content.</p>`,
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
  <li>In <b>CBZ, CBR, CB7 and CBT</b> files, the program reads the <code>ComicInfo.xml</code> inside, when there is one: series, number, volume, title, year, publisher, writer and penciller. It is the most reliable data, with one exception: if the <code>ComicInfo.xml</code>'s series is another than the file name's (the English original of a translated edition, for instance), the name's series and title count, since that is what you called the file.</li>
  <li>Without it (and always for <b>PDF, EPUB and Kindle books</b>), it uses the file name. It understands issue numbers (<code>Series 012</code>, <code>#12</code>, <code>No. 12</code>, <code>Issue 12</code>, <code>#12.1</code>), volumes or tomes (<code>v02</code>, <code>Vol. 3</code>, <code>Vol. III</code>, <code>(Vol.03)</code>, <code>Tome 5</code>, <code>T05</code>, <code>Book 3</code>), manga chapters (<code>Ch. 21</code>, <code>Chapter 21</code>, <code>c045</code>), the year in brackets and the count (<code>(of 6)</code>, <code>#2 of 6</code>). What comes between the volume and the number belongs to the series (<code>Series Vol. 2 Annual #01</code> is the annual, not issue 1). A number followed by a word is part of the name (<code>Name with 714 Words</code>, <code>10 Years</code>), and a leading number followed by a dash is the issue's (<code>19 - Story Name</code>). Marks like <code>(Digital)</code>, <code>(c2c)</code> and group names in brackets are ignored.</li>
  <li>A file with just a number (<code>012.cbz</code>) takes its series from its folder; a folder named <code>Series Name (2012)</code> also gives the series' year, and a folder <code>Vol. 3</code>, the volume.</li>
  <li>A name with no spaces, words run together (<code>CaptainAuroraSpecial012.cbz</code>), is split where a capital follows a small letter and where a number follows a word: <code>Captain Aurora Special 012</code>. Names like <code>McLane</code> and a small first letter (<code>iHero</code>) stay together. Limitation: run-together acronyms (<code>XMenAnnual</code>) may be split in the wrong place; fix it in the choice window or with <b>Edit the details</b>.</li>
  <li>In a name that starts with its number (<code>19 - Story Name</code>), the number is the leading one: numbers inside the title do not count.</li>
  <li><b>The series is the folder's name</b>: for a folder of one series, with files that carry the story's number and title (<code>Series Name (Complete)/01 - Story Name</code>). Each file gets the folder's series (without the year and what is in brackets), and what its name had becomes the title: <code>Series Name #001 - Story Name</code>. When the name only repeats the series, or part of it (<code>SERIES v01</code> in a folder with the original and the translated name), there is no title. It is off by default, because in a folder that is not a series' (<code>Downloads</code>) its name would become everyone's series.</li>
  <li>A one-shot or collection without a number gets the series and year (<code>Album Name (2024)</code>), without the marks of the original name.</li>
  <li>Some programs write accented letters as their code between underscores (<code>M_244_nica</code> for <i>Mônica</i>); the program puts them back.</li>
  <li>Files without even a series to be found are left as they are.</li>
</ul>
<h4>Looking it up in a catalog</h4>
<p>In <b>Where to look</b>, choose the catalog:</p>
<ul>
  <li><b>Grand Comics Database</b> (comics.org): made by volunteers, with editions from many countries. It gives the series' name as published, the year, the publisher and the issues' numbers. It works best with a <b>free comics.org account</b>, entered in <b>Options → Grand Comics Database</b>. Without an account, the GCD takes only about 30 requests an hour, and may stop taking them; the first search without one says so.</li>
  <li><b>Metron</b> (metron.cloud): made by volunteers, strong in American comics, with every issue and its barcode. It asks for the <b>token</b> of a free Metron account, pasted in <b>Options → Metron</b>. Only there can an issue be found by its <b>barcode</b> (see below).</li>
  <li><b>Comic Vine</b> (comicvine.gamespot.com): strong in American comics, with each issue's title. It asks for a free key, pasted in <b>Options</b>.</li>
  <li><b>MangaDex</b> (mangadex.org): manga, with no account or key. It gives the series' name and year (in your language, when there is one, or in English) and the author; the volume and chapter stay the file's.</li>
  <li><b>AniList</b> (anilist.co): manga, with no account or key. It gives the series' name, year, author and number of volumes; the volume and chapter stay the file's.</li>
  <li><b>MangaUpdates</b> (mangaupdates.com): manga, with no account or key. It gives the series' name, year, author, original publisher and number of volumes; the volume and chapter stay the file's. The data comes from MangaUpdates.</li>
</ul>
<p>On AniList and MangaUpdates, when the file's name is one of the series' names (a translation, say), the series keeps that name; otherwise it takes the English name (AniList) or the main one, often romanized Japanese.</p>
<ol>
  <li>Click <b>Look up the series</b>. Files are grouped by series (or use <b>Series to look for</b> for all of them at once). When only one series has that name and the files' year (or is the only one found) and enough issues for their numbers, it is picked by itself; without the year the name is not enough (a translation of a new series may carry an old one's name), and you choose; otherwise you choose among the publications, shown with country, language, starting year and number of issues. If the full name finds nothing (<i>Series: Story</i>), the search tries the series alone.</li>
  <li>In the picker you can search again with another name and narrow the list by the <b>Year</b> the series began and the <b>Publisher</b> (part of the name will do; the manga catalogues are not searched by publisher). Series with fewer issues than the files' numbers go last. The GCD compares names letter by letter: case and accents do not count, but punctuation does, so the program searches the name as typed and also without its punctuation (<i>Series: Name</i> also finds <i>Series Name</i>). With the year, the GCD is asked for the name's longest word, and punctuation no longer gets in the way: <i>Series Name</i> also finds <i>Series: Name</i>. For the publisher, the short name the catalogue uses will do (<i>Titan</i> is found with <i>Titan Comics</i>), and several can be given, separated by <code>;</code>. On the GCD, the publisher costs one more request for each different publisher found (up to 12, the most frequent first): add the year to keep them few, above all without an account.</li>
  <li><b>Publishers to leave out</b> (in the rule): series from these publishers never come up in searches, the automatic one or the choice window's; only by a pasted link. Useful when a publisher with series of the same name keeps coming up instead of the right one. Separate with <code>;</code>; part of the name is enough, as in the Publisher field. Limitation: MangaDex and AniList do not tell the publisher, so nothing is left out of them.</li>
  <li>Each file is matched to its issue by number (or by volume, for manga numbered by volumes; a one-shot, with the series' only issue). Those that do not match get the series' name with their own number, in the same folder as the others, and open the review, with the series' list of issues, to choose the exact issue. <b>None of these</b> brings back the name the file says.</li>
  <li>The review shows the <b>file's cover</b> (its first page; in a PDF, the first page drawn) and compares it with each issue's cover: those with the same cover move to the top with the <b>same cover</b> badge, which also counts the same art in another language. With up to 60 issues with a cover, they are compared at once; with more, press <b>Compare covers</b> (the first 300; each cover is downloaded once a session). A variant cover, or an opening page before the cover, does not match.</li>
  <li>In <b>Choose the series</b>, the file's cover shows in the corner and is compared with the same issue's (or volume's) cover in each of the first 9 series: those that match move up with the <b>same cover</b> badge, once all are compared. This tells the right publication from another of the same name. It works with Metron, Comic Vine and MangaDex (one or two more requests a series; series with over 200 issues are left out); with AniList and MangaUpdates, which only have the series' cover, only for volume 1. <b>The Grand Comics Database does not give its covers to programs</b> (neither in the series list nor in the issues): with it there is no cover comparison. Click a cover (the file's, a series' or an issue's) to see it large, beside the file's cover; Esc or a click closes it. Covers show up gradually: each series asks the catalogue for its issue list and then downloads the cover, so the time depends on how fast the site answers and on the gap it requires between requests (the program waits 3 seconds between Metron's, about 1 second on Comic Vine and half a second on MangaDex). With Metron, comparing the 9 series can take half a minute; you can choose meanwhile.</li>
</ol>
<ul>
  <li><b>Ready-made template</b>: <b>Renome default</b> (which also works for Komga, which takes each folder as a series) or <b>Kavita</b> (without the title after " - ", which Kavita does not read). Both fill in the template and folders and turn series folders on, since both servers want each series in its own folder. You can change them freely afterwards.</li>
  <li><b>Template</b>: <code>{series}</code>, <code>{series year}</code>, <code>{number}</code> (three digits: <code>012</code>), <code>{volume}</code> (two), <code>{chapter}</code> (three), <code>{title}</code> (the issue's), <code>{year}</code> (the issue's), <code>{count}</code>, <code>{publisher}</code>, <code>{writer}</code>, <code>{artist}</code> and <code>{id}</code> (the catalog's). The default is <code>{series} v{volume} #{number} c{chapter} - {title} ({year})</code>: a missing value goes along with the mark stuck to it (<code>v</code>, <code>#</code>, <code>c</code>), ending in a dot (<code>Vol. </code>) or the dash before it. So an American issue becomes <code>Series #012 (2019)</code>, a manga volume, <code>Series v03</code>, and a collection with a title, <code>Series v01 - Title (2018)</code>.</li>
  <li><b>More fields</b>, from the <code>ComicInfo.xml</code>: <code>{story arc}</code>, <code>{imprint}</code>, <code>{format}</code>, <code>{genre}</code>, <code>{inker}</code>, <code>{colorist}</code>, <code>{letterer}</code>, <code>{cover artist}</code>, <code>{editor}</code>, <code>{translator}</code>, <code>{pages}</code>, <code>{age rating}</code>, <code>{language}</code>, <code>{alternate series}</code>, <code>{alternate number}</code>, <code>{gtin}</code>, and the cover date (the catalogue's, when the issue is found there). Dates: <code>{date}</code> (<code>2010-05-03</code>), <code>{month}</code> (<code>05</code>), <code>{day}</code> and the month's name in the field's language: <code>{month name}</code> gives <code>May</code>, <code>{nome do mês}</code> gives <code>maio</code> and <code>{nombre del mes}</code> gives <code>mayo</code>. <code>{scan info}</code> is who scanned it: the <code>ComicInfo.xml</code>'s, else the last note in brackets in the name that is no year or known word (<code>(Group)</code>), or what follows <code>--</code>. What follows <code>--</code> or <code>__</code> never goes into the series, and <code>c2c</code> and <code>FCBD</code> are ignored.</li>
  <li><b>Other formats of details</b>: without a <code>ComicInfo.xml</code>, the <code>MetronInfo.xml</code> inside the file is read (series, number, volume, stories, cover date, publisher and imprint, creators by role, arc, pages, age rating, language, GTIN and the Comic Vine id); without both, the details some programs keep in the zip's comment (ComicBookInfo).</li>
  <li><b>Optional parts</b>: a part between <code>&lt; &gt;</code> only shows when every variable in it has a value (the <code>&lt; &gt;</code> signs do not go into the name). <code>{series} #{number}&lt; - {writer}&gt;</code> only adds the writer when it is known. It also works in the folder template.</li>
  <li><b>Fallback</b>: <code>{a|b}</code> puts in the first of those fields that has a value. <code>{title|year}</code> gives the title, or the year when there is no title. It may have more than two (<code>{a|b|c}</code>) and works inside an optional part.</li>
  <li><b>Digits</b>: <code>{field:N}</code> writes the numbers with N digits. <code>{a:3}</code> gives <code>005</code> instead of <code>05</code>, and <code>{a:1}</code> gives <code>5</code>, with no zeros in front. Decimals (<code>12.1</code>) and the disc before the track (<code>1-05</code>) stay as they are; text without a number does not change.</li>
  <li><b>Organize into series folders</b>: each comic goes into <code>{series} ({series year})</code> (the folder template can be changed; each <code>/</code> is a level). A comic already in a folder named after its series stays where it is, even if the folder is named another way (<code>Series Name Vol. 2 (1988-1998)</code>, <code>Series Name/Vol. 1</code>, <code>Series Name</code> for a special <code>Series Name - Special</code>, or a slightly different spelling). A folder where every comic is from one series counts as that series' folder, whatever its name (<code>My comics</code>): the new folder goes next to it, not inside, and whatever stays in it follows the leftovers rule (see Preview and conflicts). The comics in the list are arranged together, as a library:<ul><li>each series gets one spelling, the most common in the list (case, accents and one letter apart in a word, as in <code>Apocalipse</code> and <code>Apocalypse</code>, count as the same series), in the folder's name and the files';</li><li>a special or one-shot goes into the folder of the series its name starts with (<code>Series Name - Special</code> into <code>Series Name</code>; when there are several, the one with the longest name);</li><li>one-shots that share what comes before " - " (<code>Series Name - Album One</code>, <code>Series Name - Album Two</code>) go together into a folder named after it;</li><li>the others each go into their series' folder, even with a single file.</li></ul>Undo puts everything back.</li>
</ul>
<h4>Barcode (Metron)</h4>
<p>With <b>Metron</b>, a comic with its barcode recorded in the <code>ComicInfo.xml</code> (the GTIN field) or the <code>MetronInfo.xml</code> is found straight by it, with the exact issue, without picking the series. With <b>Read the barcode printed on the pages</b>, the program also looks for the code in the picture of the first and the last two pages (the cover and the back cover) of the comics that do not have it recorded: the UPC of American comics, with the 5 extra digits that tell the issue and cover, or the ISBN of a collected edition. A code read this way also goes into the <code>ComicInfo.xml</code> when the details are written. Those not found by their code go on to the search by name.</p>
<h4>Writing the details into the file</h4>
<p>With <b>Write the details into the file (ComicInfo.xml)</b>, after renaming, each CBZ matched to an issue in the catalogue gets a <code>ComicInfo.xml</code> inside it with series, number, title, issue count, volume, cover date, writer, artist, publisher, the issue's web page in the catalogue and whatever else the file already told (story arc, imprint, colorist…). Komga, Kavita and comic readers read this file. If the CBZ already had one, only the fields that change are replaced: the summary, the page list and the rest stay. The pages are copied as they are, losing no quality. Comics whose names are already right get the details too (they count in the <b>Rename</b> button), and those that already have the same details are skipped. Those found on MangaDex, AniList and MangaUpdates are marked as manga.</p>
<p>The writing is done in a copy that takes the file's place only at the end; if the program is closed halfway, what was already written goes to the <b>History</b>. Undo puts the old <code>ComicInfo.xml</code> back, or takes out the one added. As the content changes, torrents no longer recognise these files.</p>
<h4>Editing the details by hand</h4>
<p>For what no catalogue finds (local editions, fanzines, rare scans), select the comics in the list and click <b>Edit the details</b>, in the rule. In the window, type series, series year, volume, number, title, year, publisher, writer and artist. What you type counts instead of what the file and the catalogue say: the new name is made from it and, with <b>Write the details</b>, it goes into the <code>ComicInfo.xml</code> (with no catalogue address). With several selected, only the fields you change count for all (useful to fix the series or publisher at once); a field with different values shows empty and stays as each one has it. Limitations: typed details last while the program is open and the files are in the list (like searches); <b>Clear</b> forgets them and Ctrl+Z does not undo them; other fields (story arc, colorist, summary) are not edited here.</p>
<h4>Removing the ComicInfo.xml</h4>
<p>With <b>Remove the ComicInfo.xml from CBZ files</b> (and <b>Write the details</b> off), each CBZ of the rule that has a <code>ComicInfo.xml</code> is left without it after renaming. It serves to clear wrong details another program wrote. The new name is made first, still from what it said. With <b>Write the details</b> on, the option becomes <b>Write from scratch</b>: the <code>ComicInfo.xml</code> of comics found in the catalogue keeps only what the catalogue tells (the page list stays); those not found are left as they are, since what they say is what names them. Undo puts the old one back. Limitations: only the <code>ComicInfo.xml</code> goes; the <code>MetronInfo.xml</code> and details in the zip's comment (ComicBookLover) stay; a comic converted to CBZ does not go through this clearing.</p>
<h4>Converting to CBZ</h4>
<p>With <b>Convert CBR, CB7, CBT, PDF, EPUB, MOBI/AZW3 and picture folders to CBZ</b>, each comic in RAR, 7z or TAR becomes a CBZ, the format every reader opens. The extension changes in the preview already (<code>Name.cbr</code> → <code>Name.cbz</code>), and <code>.cbz</code> files that are really RAR or 7z are converted too, even without a new name (they count in the <b>Rename</b> button). The pages are copied as they are, in the same order and without losing quality. A <b>PDF</b> is converted page by page, always in the way that keeps the most. When the page is a single picture, as in a scanned comic, the picture comes out as it is: JPEG stays JPEG; others become PNG, losslessly. That also goes for pictures in JPEG 2000, JBIG2 and fax, which are decoded losslessly and written as PNG. CMYK pictures (print) become RGB, with approximate colors. What is drawn under the picture, such as the background and frame of a web page printed to PDF, is left out, as long as the picture covers nearly the whole page. A page the PDF shows turned or mirrored comes out upright: a JPEG is turned losslessly when its size allows (nearly always); otherwise it becomes a PNG, bigger but also lossless, and an empty page becomes a white page as big as the others. Only when none of that fits (real text and drawing, layered pages, several pictures on a page, stamps and comments over it) is the page <b>drawn</b> as a reader shows it, at the resolution of its sharpest picture (or, when it has none, at <b>Size of drawn pages</b>: 1600, 2400 or 3200 pixels on the long side), and written as a PNG or, when much smaller, a JPEG. Several pages are drawn at once, one per processor core. In the preview, a PDF that will have drawn pages gets the tag <b>N page(s) drawn</b>, and the notice at the end says how many were. In <b>PDF pages that are not just a picture</b>, <b>Do not draw</b> leaves such PDFs as they are: only those that can be converted without drawing anything are. </p>
<p>An <b>EPUB</b> or <b>Kindle</b> book (MOBI, AZW3) is converted when every page of it is a picture, as in comics sold in these formats: the pictures come out as they are, in reading order, cover first. A book with text (a page with more than a number, or with no picture) is not a comic and stays as it is, since the text would be lost; so does an EPUB or Kindle book with DRM. A <b>folder of pictures only</b>, added with <b>Folders (rename the folders themselves)</b>, gets the comic's name and becomes a CBZ with its pictures (and those of its subfolders, as chapters), in number order; a <code>ComicInfo.xml</code> in it goes along. A folder with any other kind of file (video, document) stays as it is.</p>
<p>With <b>Write the details into the file</b> on, the PDF's, EPUB's or Kindle book's own title and author go into the <code>ComicInfo.xml</code> when the catalogue does not give them. With <b>Write the details into the file</b> on too, the <code>ComicInfo.xml</code> goes into the new CBZ; without it, what the original had goes as it was.</p>
<p>The CBZ is written in a copy beside and takes the original's place only when complete and checked. In <b>The original file</b> you choose what happens to it. <b>Keep beside</b> leaves the original in the same folder, with the new name and its own extension (<code>Name.cbr</code>; a folder takes the new name, with no extension); undo puts it back and deletes the CBZ. <b>Delete</b> sends the original to the system's Trash, where it can be recovered; undo only gives the CBZ its old name back, as a CBZ.</p>
<h4>E-book for Kindle, Kobo and reMarkable</h4>
<p>With <b>Make into an e-book</b>, after renaming, each comic (CBZ, CBR, CB7, CBT or PDF of pictures) becomes a <b>fixed-page</b> book, one page per picture, in one of these formats:</p>
<ul>
  <li><b>AZW3</b>, for Kindle (copy it over the cable to the <code>documents</code> folder).</li>
  <li><b>EPUB</b>, for reading apps and tablets; <b>KEPUB</b> is the same book named as Kobo's own reader uses (<code>Name.kepub.epub</code>), faster there.</li>
  <li><b>Fitted CBZ</b>: a CBZ with the pages already cut, split and sized to the screen, for KOReader and older Kindles. A CBZ made into a CBZ is remade in place, and the old one is kept in the history.</li>
  <li><b>PDF</b>, for reMarkable and the Kindle Scribe (which shows PDF better than AZW3). A PDF of pictures made into a PDF is remade in place too.</li>
</ul>
<p>The book carries the details of the issue found or typed in the details editor (series, number, title, writer, publisher, year), or those of the <code>ComicInfo.xml</code>, and a manga (from a manga source, or with <code>Manga</code> set to <code>YesAndRightToLeft</code>) opens right to left. The original stays beside, with its own extension, or goes to the Trash, and undo brings it back.</p>
<ul>
  <li><b>Reader</b>: pages are shrunk to fit the chosen screen (44 Kindle, Kobo and reMarkable models, each by its own name, grouped by brand and line, newest first; a tablet; or <b>Another size</b>, typed in); on grey-screen readers they are saved in grey, which makes the file smaller without changing what is seen. <b>Original size</b> leaves the size alone.</li>
  <li><b>Cut the empty margins</b>: removes the even white or black borders scanning leaves, keeping a little room. With <b>Cut the page number too</b>, a number or small mark alone at the top or bottom, apart from the drawing by margin, goes with them.</li>
  <li><b>Double pages</b> (wider than tall): <b>split in two</b>, in reading order (in a manga, the right one first); <b>turn whole</b>, to read with the reader sideways; <b>turn, then split</b> (the whole one and both halves); or leave them. A wide cover, scanned together with its back, keeps only the front (the right half; in a manga, the left).</li>
  <li><b>Improve contrast</b> (on by default): stretches the tones so a scan's greyish black becomes black and a yellowed background white; on ink screens, it darkens the middle tones a little, which they show too light.</li>
  <li><b>Enlarge small pages</b>: a page smaller than the screen is enlarged to fill it.</li>
  <li><b>Use only the screen's 16 greys</b> (grey screens): the page is reduced to the 16 tones the screen shows and saved as PNG when that is smaller than JPEG, which nearly always happens with line drawings.</li>
  <li><b>Soften dot patterns</b> (colour ink screens such as Colorsoft and Kobo Libra Colour): a light blur reduces the "rainbow" that fine halftone dots make on those screens.</li>
  <li><b>Webtoon</b>: a very long vertical strip is cut into pages as tall as the screen, in the white (or black) gaps between panels.</li>
  <li><b>Join the issues into a volume</b>: issues of the same series (and volume) in the same folder become one book, <code>Series #1-5</code>, with one table of contents entry per issue, in number order. The comics stay in the folder, renamed in their own format, and undo deletes the volume and brings the names back. A lone issue becomes a book on its own.</li>
  <li><b>Largest book size</b>: above it, the book is made in parts (<code>Name.azw3</code>, <code>Name (2).azw3</code>…), each titled to say which it is (<code>(2/3)</code>); undo deletes the parts. Sending to a Kindle by e-mail takes up to 50 MB, and the app and site up to 200 MB.</li>
</ul>
<p>A page that needs no change (no cut, split, shrink or tone change) goes as it was, without losing quality. The others are saved as good-quality JPEG; when the original scan was heavily compressed already, a little more, so the book does not grow.</p>
<p><b>Limitations:</b> folders of pictures and comics that are EPUB or MOBI already are not made into e-books (convert them to CBZ first); the book is made of pictures (the text in the balloons cannot be searched or enlarged as letters); a double page that is not clearly wider than tall is not split, and a very wide single page (a poster) is; margin cutting does not recognise textured or graded borders, and the page number is only recognised when apart from the drawing by margin; the contrast change applies to the whole page (it does not lighten just one dark part); webtoons are cut where a row is even and white or black, and a strip without such gaps is cut through a panel; on the Kindle Scribe and colour Kindles PNG is not used (they show PNG pages blank), and there PDF tends to work better than AZW3; to join issues, all of them must have their series found or typed, and the volume takes the first one's details; a book made in parts is cut between pages, without looking at where each issue ends. fixed-page AZW3 has been tried on a Kindle Paperwhite, but not on every Kindle model; to send it to a Kindle, copy it over the cable to the <code>documents</code> folder (the Kindle's e-mail takes EPUB, but converts it again).</p>
<h4>Missing issues</h4>
<p>With <b>Show the missing issues</b> (on by default), after the lookup the list gets faded rows for the series' published issues that no file in the list holds, such as <code>#7 · Title</code>, with the cover date. They sit where the issue belongs, between its neighbours, and the bottom bar says how many are missing. They are only a notice: they cannot be selected and are left out of counts and renaming. <b>Click that count</b> to save the missing issues to a CSV file (series, number, title and cover date); the window opens in the series' folder. Only series with some file matched to an issue count.</p>
<h4>Limitations</h4>
<ul>
  <li><b>CBR</b> (RAR) files are read with libarchive, an open source library built into the program. It opens almost all of them, but a few old RAR files fail (the official RAR program opens them): those stay as they are and show up in the error list. RAR files with a password and RAR files split into parts (<code>.part1.rar</code>) are not converted. RAR files cannot be written: to write details into a CBR, it has to be converted.</li>
  <li>Very unusual file names may be read wrong (a number that is part of the series' name, for instance). Check the <b>New name</b> column before renaming.</li>
  <li>Catalogs have gaps: Brazilian and other countries' editions may be missing on Comic Vine, and the GCD, made by volunteers, may be incomplete. When the right series is not there, skip it: the files keep the name they say themselves. The search does not always find a series by a translated name; in the picker, you can search again with another name (the original, for instance).</li>
  <li>On the GCD, the review shows only the issues' numbers, without titles or covers: fetching them would take one request per issue. A series with the same name in another language (the French edition, when there is one in yours) is not picked by itself.</li>
  <li>The GCD and Comic Vine count <b>issues and volumes</b>, not chapters: loose manga chapters are not looked up and keep the name read from the file. So does Metron. MangaDex, AniList and MangaUpdates give the series only.</li>
  <li><b>Request limits</b>: the GCD takes about 25 a minute with an account (about 30 an hour without one); Comic Vine, about 200 an hour, for non-commercial use only; Metron, about 20 a minute and 5,000 a day (a series with many issues takes one more request for each 100 issues, and the program waits 3 seconds between them); AniList, non-commercial use only and about 30 a minute. The program uses few per series and keeps the answers while it is open; if you reach a limit, wait a little and search again.</li>
  <li>The <b>missing issues</b> follow the catalogue's list: annuals, specials and oddly numbered issues (<code>#1.5</code>, <code>#0</code>) show up as missing if you lack them, and a series with hundreds of issues, of which you have a few, floods the list. Turn the option off then. The GCD's list has no cover dates.</li>
  <li>The <b>barcode</b> only helps on Metron, and only when Metron has that issue's code (many old ones and Brazilian ones do not). Many files have no back cover, or have it with the code cropped, blurred or too small; PDFs are not read (their pages would have to be drawn). A code shared by many issues (the UPC without the 5 extra digits, used on newsstand copies) does not tell the issue and is ignored. Reading the pages takes from half a second to a few seconds per file, so a large collection takes long. Rarely, a mark in the picture may be read as a valid code of another issue: check the preview.</li>
  <li><b>MangaDex</b>, <b>AniList</b> and <b>MangaUpdates</b> have no list of a manga's volumes (only of the chapters readers uploaded, and those of licensed manga have usually been taken down). So there the missing ones are only those missing <b>between your files</b>, from 1 to the highest number you have (by volume when every file has one, else by number or chapter): what comes after the last does not show.</li>
  <li>The <b>GCD</b> lists each variant cover as an issue of its own with the same number; the program keeps one a number (the regular cover, when there is one), in the count, the review and the missing issues.</li>
  <li>Details are only <b>written into CBZ</b> files (that are ZIPs inside). CBR, CB7, CBT, PDF, EPUB and Kindle files get them only when converted; EPUBs that are not comics can get them through the book rule. A damaged file is not changed and shows up in the error list. With the options off, the file is never changed.</li>
  <li><b>Converting</b> reads and writes each whole file: a large collection takes time and needs free space for one copy at a time. The CBZ is about the size of the original (sometimes a little bigger, since the pages are not compressed again). With <b>Keep beside</b>, both stay in the folder, and Komga and Kavita show both until you delete the originals. With <b>Delete</b>, undo does not bring the original back: it stays in the Trash until you empty it. A file that could not be converted (damaged, with a password) goes back to the extension it really has (<code>Name.cbr</code>).</li>
  <li>A <b>drawn</b> page is a picture of the page: text stops being text, the resolution is fixed and the file is much bigger than the PDF (a 1 MB text book can become a 90 MB CBZ). Layered pages lose a little when merged. Drawing is done by <code>hayro</code>, an open source library built into the program, which does not yet reproduce everything in PDFs (some color blending and transparency modes), and takes a few seconds per book. Password-protected PDFs are not converted. Pictures kept inside the page's content are taken out as they are only when not compressed (the others are drawn).</li>
  <li>An <b>EPUB</b> or <b>Kindle</b> book with text is not converted, not even an illustrated book or a comic that carries the balloons' text over its pictures: a CBZ holds only pictures, and there is no way to draw the book's text on them. Neither are those with several pictures on a page (where each one goes would be lost), those with DRM (nearly all bought in stores) and Kindle books with the format's stronger compression (some made by Amazon's program). Internet Archive EPUBs, made by text recognition, have pages without pictures and are not converted. An AZW3 kept beside keeps that extension; a MOBI, or an AZW3 with both versions of the format, becomes <code>.mobi</code>.</li>
  <li>A <b>folder of pictures</b> is only converted if it holds nothing but pictures (JPEG, PNG, GIF, WebP, AVIF, JPEG XL), the <code>ComicInfo.xml</code> and the files the system makes by itself (<code>Thumbs.db</code>, hidden files), up to three levels of subfolders. A photo folder is pictures only too: if it is in the list with the rule on, it becomes a CBZ, so check the preview.</li>
  <li>Only what the catalogue (or the file itself) tells is written: on the GCD, issues come without titles, and the manga catalogues only give the series. A <code>MetronInfo.xml</code> the file has is not updated.</li>
</ul>
<p class="tip">Only the searched series name and the series' and issues' numbers are sent to the catalogs, never the full file name, the folders or the content. The GCD sign-in goes only to comics.org.</p>`,
  },
  {
    id: "r-photo",
    title: "Photo",
    sub: true,
    html: `
<p>Names phone and camera <b>photos and videos</b> by <b>when they were taken</b> and, if you want, organizes them in folders by date. Works offline: everything is read from the files themselves.</p>
${ex("IMG_4821.JPG", "2019-05-12 14.30.22.JPG")}
${ex("IMG-20190513-WA0001.jpg", "2019-05-13.jpg")}
<h4>Where the date comes from</h4>
<ol>
  <li><b>From inside the file</b>: in photos, the EXIF date (JPEG, HEIC, PNG, WebP, TIFF and TIFF-based RAW files such as DNG, CR2, NEF, ARW, PEF and SRW); in MP4, MOV, M4V and 3GP videos, the recording date.</li>
  <li><b>From the file name</b> (on by default): <code>IMG_20190512_143022</code>, <code>PXL_20190512_143022123</code>, <code>VID_20190512_143022</code>, <code>IMG-20190512-WA0001</code> (WhatsApp: the day the photo arrived, see the limitations), <code>Screenshot_2019-05-12-14-30-22</code>, <code>WhatsApp Image 2019-05-12 at 14.30.22</code>, <code>2019-05-12 14.30.22</code>. A name with the day alone leaves the time empty. For <b>videos</b>, a name with date and time (<code>20260913_185946</code>, <code>VID_…</code>) wins over the date inside: the phone puts the start of the recording in the name, while the video usually holds when the file was closed, seconds or minutes later.</li>
  <li><b>From the change date</b>, only if you turn the option on. On a copied, downloaded or received photo it is usually when it was copied.</li>
</ol>
<p>With none of them, the file <b>stays as it is</b> and gets a mark in the list; the card says how many did. A date taken from the name or the change date gets a mark too (hover to see where it came from).</p>
<h4>Set the date by hand</h4>
<p>When a photo's date is wrong or missing (a photo received through WhatsApp loses the date it was taken and only keeps, in its name, the day it arrived), select the photos and videos in the list and click <b>Set the date</b> in the rule. Type day, month and year and, if you like, hour and minute. The typed date counts over every other: the file is named by it, goes to that date's folder and, with <b>Set the file date</b>, gets it as its modified date. Without a time, each photo keeps the time it had (from inside it or its name), or none. With several selected, all of them get the fields you change. Clearing every field goes back to the file's date. The mark in the list says <b>Date typed by hand</b>. Limitations: the date is not written inside the photo (in the EXIF), only in its name, folder and modified date; it lasts while the program is open and the files are in the list; <b>Clear</b> forgets it.</p>
<h4>Template</h4>
<p>Use <code>{year}</code>, <code>{month}</code>, <code>{day}</code>, <code>{hour}</code>, <code>{minute}</code>, <code>{second}</code>, <code>{date}</code> (2019-05-12), <code>{month name}</code>, <code>{camera}</code> (make and model, like <code>Apple iPhone 12</code>), <code>{width}</code>, <code>{height}</code> and <code>{name}</code> (the current name). The ready templates give <code>2019-05-12 14.30.22</code>, <code>20190512_143022</code>, with the camera in brackets, or the date before the current name. What is left empty (a time not known, a camera the photo does not tell) goes, with the separators and brackets around it.</p>
<h4>Files that go together</h4>
<p>In one folder, files with the <b>same name before the extension</b> are one photo and get the same new name: the JPEG and the RAW (<code>IMG_0001.JPG</code> and <code>IMG_0001.CR2</code>), the edits (<code>IMG_0001.xmp</code>, <code>IMG_0001.CR2.xmp</code>, the iPhone's <code>.AAE</code>) and a Live Photo's photo and video (<code>IMG_0001.HEIC</code> and <code>IMG_0001.MOV</code>). The date is that of the file that has it inside (the photo before the RAW and the video).</p>
<h4>Photos of the same second</h4>
<p>Photos in a burst, and photos whose new name the folder already has, get <code>-1</code>, <code>-2</code>… instead of a conflict, in the order of their current names; files that go together get the same number. Named so, they stay so the next time. Identical copies get no number: the conflict says they are the same. Two versions of one photo that are not exact copies (a 15 MB original and a recompressed 500 KB JPG), though, <b>are both taken</b>, each with its number. To keep only the better one, open <b>Compare</b> on the similar pictures before renaming and use <b>Keep only the largest</b>.</p>
<h4>Options</h4>
<ul>
  <li><b>Fix the clock</b>: hours added to the camera's date and the name's (not the change date, nor a name with the day alone), for a camera with its clock wrong or travel photos in another time zone. It applies to every file in the list: to fix one camera's only, filter the list first.</li>
  <li><b>Organize in folders by date</b>: a folder template with <code>/</code> between levels (<code>{year}/{year}-{month}</code>, <code>{year}/{month} - {month name}</code>…). The folders are made in the folder you added; if it is a date folder already (<code>Photos/2019/2019-05</code>), in the folder above them. Folders left empty are deleted.</li>
  <li><b>Set the file date</b>: the change date of the files renamed or moved becomes the photo's, which helps galleries and phones that sort by it. The file's content is not changed, and undo puts the old date back.</li>
</ul>
<p><b>Identical copies</b> and <b>similar photos</b> keep working with the rule: they show in the footer, and double-clicking one's mark opens the side-by-side comparison, from where the extra ones go to the Trash (see <b>Identical copies</b>).</p>
<p><b>Limitations:</b></p>
<ul>
  <li><b>Photos without the date they were taken.</b> WhatsApp and many other messaging apps and social networks remove the EXIF data when a photo is sent or saved, and so do many photos downloaded from the web or edited. In them, all that is left is the date in the name, which for WhatsApp (<code>IMG-20190513-WA0001</code>) is the day the photo <b>arrived</b>, not the day it was taken, or the modified date, which is the day of the copy or download. Renome has no way of finding the real date: a photo taken in 2015 and received in 2019 is named and goes to the 2019 folder. Before renaming, filter the list by <code>WA</code> or hover over the marks of dates taken from the name, and fix those photos with <b>Set the date</b>; or turn off <b>From the file name</b> so they stay as they are. To receive a photo with its date, ask for it to be sent through WhatsApp <b>as a document</b>: the file then arrives unchanged, with its EXIF.</li>
  <li>The first time, the start of each photo and video is read to find its date: on an external hard disk, about a minute for 13,000 files, with the progress at the bottom (<b>Reading the photos' dates…</b>). The list appears when the reading ends; after that, the preview is instant. The window keeps responding, and the searches for copies and similar pictures wait for this reading. Copying a large collection to another folder takes the time of copying all the data.</li>
  <li>Some RAW files may have no date read (such as CR3, RAF, ORF, RW2, X3F, CRW and MRW): they take the date of the JPEG with the same name, or the name's. MTS, M2TS and AVI videos only have the name's date or the change date.</li>
  <li>A video's recording date is, by the format's rule, in universal time (UTC), and is shown in this computer's time zone; a video recorded in another zone gets the time here. iPhone videos tell their zone and come out right. Some cameras write local time as if it were UTC: fix it with <b>Fix the clock</b>.</li>
  <li>Two different files with one name and different extensions in one folder (<code>photo.jpg</code> and <code>photo.png</code>) are taken as one photo.</li>
  <li>With folders by date, a photo in a subfolder (<code>Photos/Trip</code>) leaves it for the date's folder.</li>
  <li><b>Set the file date</b> applies only to files renamed or moved; one already named right is not touched. The creation date is not changed.</li>
  <li>Nothing is written inside the photos, and where they were taken (GPS) is not used.</li>
</ul>`,
  },
  {
    id: "r-subtitle",
    title: "Subtitle",
    sub: true,
    html: `
<p>Fixes <b>text subtitles</b> and takes out those <b>inside MKV videos</b>. Works offline and does not change file names: only the extension, when a subtitle becomes SRT. It can be used along with the Movie or TV show rule, which give the names. Everything can be undone from the History: the subtitle comes back exactly as it was, and those extracted are deleted.</p>
${ex("Movie.en.ass", "Movie.en.srt")}
<p>Formats read: <b>SRT</b> (SubRip), <b>ASS</b> and <b>SSA</b> (SubStation), <b>VTT</b> (WebVTT), text <b>SUB</b> (MicroDVD and SubViewer) and <b>SMI</b> (SAMI). The format is told by the content, not the extension. A file that is not a known text subtitle (a VobSub picture <code>.sub</code>, a <code>.sub</code> that is something else) <b>is not touched</b>. In the list, each subtitle that will change gets a tag with what will be done (hover it for the details).</p>
<h4>Options</h4>
<ul>
  <li><b>Convert to SRT</b>: subtitles in other formats become SRT, which almost every device, TV and program reads. Italics, bold, underline and lines placed on top (<code>{\\an8}</code>) go along; in SubStation, an italic style too. Colours, fonts, sizes, exact positions, karaoke and drawings are lost. The original stays beside, with its own extension, or goes to the Trash, as you choose.</li>
  <li><b>Write as UTF-8</b> (on by default): old subtitles in other encodings become UTF-8, and accents stop turning into odd symbols. The encoding is told by the language in the file name (<code>.ru</code> is Windows-1251, <code>.pl</code> is Windows-1250…) or, without one, by the text itself, the way browsers tell it (Western, Central European, Cyrillic, Greek, Turkish, Hebrew, Arabic, Japanese, Chinese, Korean…). Without this option, an edited subtitle keeps the encoding it had.</li>
  <li><b>Remove ads and credits</b>: removes lines with subtitle sites' notices (anywhere in the film) and, only at the start and the end, web addresses and credits of who made, synced or translated the subtitle (<code>Subtitles by…</code>, <code>Translation:</code> with the names under it). A sentence that only mentions a site, in the middle of a line, stays.</li>
  <li><b>Remove hearing-impaired marks</b>: removes sounds and descriptions in brackets (<code>[music]</code>, <code>[laughs]</code>); in parentheses when in capitals, short and alone on the line, or before the speech (<code>(Sighs)</code>); speakers' names in capitals before a colon (<code>JOHN: Where were you?</code>); and lines of only music notes. When only one line of a dialogue is left, its dash goes too.</li>
  <li><b>Delay or advance</b>: seconds added to every time (positive delays, negative brings forward; tenths are fine, like <code>-1.5</code>). Lines that would fall before the start of the video are removed.</li>
  <li><b>Frame rate</b>: for a subtitle that starts right and slowly drifts later or earlier. It was made for another version of the video (cinema and Blu-ray at 23.976 frames per second, European TV at 25). Choose from which rate to which: times are stretched or shrunk in the same proportion.</li>
  <li><b>Extract subtitles from MKV videos</b>: the text subtitles (SRT, ASS, VTT) inside the video become files beside it, with the video's name, the track's language and the <code>.forced</code> and <code>.sdh</code> marks when the track says so (<code>Movie.en.forced.srt</code>). In <b>Languages</b>, choose which (<code>pt-BR, en</code>); empty, all of them. With <b>Convert to SRT</b> they come out as SRT; the cleanings apply to them too, but times do not change, since the track is already in time with the video. The video is not changed.</li>
</ul>
<p><b>Limitations:</b></p>
<ul>
  <li><b>Picture subtitles</b> (Blu-ray PGS, DVD VobSub, DVB) are not text: they cannot be extracted or converted, since the letters of each picture would have to be recognised. The list tells how many of them each video has.</li>
  <li>Extracting reads the <b>whole video</b> (the subtitle is spread through the file): on an external disk, a 20 GB film takes a few minutes. There is progress and the Stop button. MKV and WebM videos only: subtitles inside MP4 and AVI are not extracted.</li>
  <li>Converting to SRT loses what SRT does not have: colours, fonts, exact positions, effects and karaoke. An ASS subtitle full of placed signs gets all of them as ordinary lines, at the bottom.</li>
  <li>The <b>encoding</b> of an old subtitle without a language in its name is a guess from its text: a very short subtitle, or one in an encoding close to another (like Turkish and Western), may come out wrong. Undo brings the original file back.</li>
  <li>The cleanings follow patterns, they do not understand the text: a whole short line in parentheses, or a name in capitals before a colon, is removed too; a sound described in lower case in the middle of a line stays. Subtitles written all in capitals only lose what is in brackets or alone in parentheses. Credits are looked for in Portuguese, English and Spanish.</li>
  <li>A subtitle that has only ads or credits is not emptied: it stays as it is, with a warning.</li>
  <li>A <b>MicroDVD</b> subtitle counts times in frames: without the frame rate in it (<code>{1}{1}23.976</code>) or in a video beside whose name starts the same, it cannot be converted or retimed, and gets the <b>no frame rate</b> tag.</li>
  <li>SAMI (<code>.smi</code>) with several languages: only the first is converted. A SAMI is only cleaned or retimed when converted to SRT.</li>
  <li>An edited subtitle is written again in full, in the format's standard form: the line numbering, extra blank lines and mixed line ends may change, without changing what is shown on screen.</li>
</ul>`,
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
  <li>a file that moves to another folder gets a folder icon in its new name, and the <b>Folder</b> column shows, in blue, the whole folder it goes to (<code>→ ~/Music/Artist/Album (2019)</code>), written like the other files' current folder.</li>
  <li>with a lookup rule on (movie, TV show, music, book, comic), a file it does not handle (a <code>.zip</code> in a show's folder, say) gets the grey tag <b>outside the rule</b>; hover over it to see why. The row stays as it is: you can rename it by hand, but the lookup will not touch it.</li>
</ul>
<p>In the first column, ● means the file will be renamed, a red <span class="bad">!</span> is a conflict and a yellow <span class="warn">!</span> is a warning. Hover to see why.</p>
<h4>Editing by hand</h4>
<p>Double-click the new name (or select a file and press <kbd>F2</kbd>) to type any name you want. <kbd>Enter</kbd> confirms and <kbd>Esc</kbd> cancels. A name edited by hand ignores the rules, but the folder they move the file into (the show's, the movie's) still applies; ↺ goes back to the rules.</p>
<h4>Old folders that keep files</h4>
<p>When the rules give a new name to the folder of a show, a movie, an album, an author or a comic series, the new folder is created <b>next to</b> the old one and the files move into it. An old folder left empty is deleted. If files remain in it (extras, images, a file in conflict, a file not in the list), it is not deleted: it takes the new folder's name with <code>(additional files)</code> at the end, so the two show up together. For example, <code>Movie Name.1999.1080p.BluRay</code> with a <code>sample.mkv</code> becomes <code>Movie Name (1999) [tmdbid-12345] (additional files)</code>, next to <code>Movie Name (1999) [tmdbid-12345]</code>.</p>
<ul>
  <li>Before renaming, the bottom bar shows, in yellow, <b>folder(s) with leftovers</b> (hover to see which), and the confirmation names the folder and how many files stay in it, even with confirmation turned off.</li>
  <li><b>Undo</b> puts the files back and gives the folder its old name.</li>
  <li>The home folder, Downloads, Documents, Videos, Music, Pictures, the Desktop and a disk's top folder are never renamed.</li>
  <li>A folder that becomes two (<code>Book Title</code> → <code>Author/Book Title</code>) keeps its name: there is no single new folder next to it. The same goes for a folder whose files go to different places (a folder with songs by several artists).</li>
  <li>A movie only gets its new folder next to the old one when the folder it is in is named after it (with or without tags like 1080p) or holds only the movie and what goes with it: subtitles, images, <code>.nfo</code> and <code>.txt</code>. A folder with another video, a document or a spreadsheet does not count: the movie's folder is created inside it.</li>
</ul>
<h4>Conflicts</h4>
<p>Files with a conflict are not renamed. The reasons are:</p>
<ul>
  <li>an empty name, or one containing <code>/</code>;</li>
  <li>a name longer than 255 bytes (on Windows, 255 characters);</li>
  <li>two files would get the same name in the same folder;</li>
  <li>a file with that name already exists outside the list (nothing is overwritten);</li>
  <li>on Windows: the characters <code>\\ : * ? " &lt; &gt; |</code>, or a dot or space at the end of the name.</li>
</ul>
<p>On other systems, those Windows characters only cause a yellow warning, which can be turned off in Options. On Windows, macOS and disks that ignore case (exFAT, FAT, NTFS, as on USB sticks), names that differ only in case count as the same.</p>
<p><b>Export list</b>, in the footer, saves the list as shown (folder, current name, new name, folder it moves into, status and problem) to a CSV file, which opens in any spreadsheet. It helps check a large batch calmly before renaming.</p>
<p>When something is missing (episodes, issues, tracks, movies or volumes), the footer shows the total, with a box beside it: <b>unticked</b>, the faded rows leave the list, to see only your files; <b>ticked</b>, they come back. Nothing is searched again, and the total and the export stay the same. The choice is saved.</p>
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
<p>The search runs in stages to be fast: only files of the same size are compared; then only their start and end; a whole file is read only when all of that matches. With large files, the footer shows the progress. Empty files, folders and symbolic links are left out.</p>
<h4>Repeated songs</h4>
<p>Besides identical copies, the program finds the <b>same song in different files</b>: an MP3 and a FLAC of one track, or a song on its album and again on a compilation. They get an outlined tag, <b>♪</b> and the group number (hover to see the others), and the footer shows <b>N repeated song(s)</b>: click it to see only them, grouped. Nothing is deleted or changed.</p>
<ul>
  <li>They count as the same song when the tags have the same <b>title</b> and <b>artist</b>, regardless of case, accents and punctuation, and without what is in brackets (<code>(Remaster 2011)</code>); guests (<code>feat.</code>) do not count. The <b>length</b> must be the same, within 3 seconds.</li>
  <li>Identical copies of one file already show as copies; they come in here only when another file of the same song is there too.</li>
</ul>
<p><b>Limitations:</b> only the tags count, not the sound. Songs without a title, an artist or a length are left out; wrong tags give false alarms or let repeats through. A live or remixed version with the same title and nearly the same length counts as the same song, and the same recording with titles written differently (<code>Pt. 1</code> and <code>Part One</code>) is not recognized. The first time, the tags of every song in the list are read, which takes a while in big folders.</p>
<h4>Repeated movies, episodes, comics and books</h4>
<p>In the same search, the program finds the <b>same movie, episode, comic or book in different files</b>: a film in 4K and in 1080p, an issue as CBR and as CBZ, a book as EPUB and as PDF. They get a <b>≡</b> tag with the group number, and the one worth keeping gets <b>★</b>. Hover to see the other files, each video's resolution and codec, each one's size and why that one is the best. The footer adds them up as <b>N repeated file(s)</b> (with the songs): click it to see only them, grouped. Nothing is deleted or changed; to keep only the best, select the others and remove or delete them yourself.</p>
<ul>
  <li><b>Movies</b>: same title and year in the name (<code>Movie.2010.1080p…</code> and <code>Movie (2010)</code>), or the same <code>[tmdbid-…]</code>. Without a year in the name it is left out, since it could be another version. Trailers, samples and extras (<code>-trailer</code>, <code>-sample</code>, folders <code>Extras</code>, <code>Bonus</code>…) and what follows the year after a dash (<code>Movie (2010) - Making Of</code>) do not count as the film. The best is the highest resolution and, with the same, the largest file.</li>
  <li><b>Episodes</b>: same show (in the name, or the folder above the season's), same season and episode (or date, or absolute number); the parts of an episode (<code>Part 1</code>, <code>Part 2</code>) stay apart. The best, as for movies.</li>
  <li><b>Comics</b>: same series, volume and number, from <code>ComicInfo.xml</code> or the name. The best is the largest, usually with the sharpest pages; with a similar size, the CBZ.</li>
  <li><b>Books</b>: same ISBN and title in the book's details, or the same words in the name (in any order: <code>Author - Title</code> and <code>Title - Author</code>) in <b>different formats</b>. The best is the handiest format: EPUB, then AZW3, MOBI, FB2 and PDF.</li>
</ul>
<p><b>Limitations:</b> everything comes from names and the details inside the files, not from the content: misleading names give false alarms or let repeats through. Two files of the same format with the same name in different folders only count as the same book when ISBN and title match (documents like monthly bills often share a name). Resolution is only read from MKV, MP4, AVI and other common video formats; without it, size decides. "Best" is a simple rule: a larger file is not always better, and two different versions of one film (an extended edition, for example) show as repeated.</p>
<h4>Similar pictures</h4>
<p>After that, the program compares the <b>pictures</b> in the list and finds those that look alike: the same photo in another size or format, recompressed or lightly edited (brightness, text on top), and shots taken one after another. They get an outlined tag, a picture icon and the group number (hover to see which the others are), and the footer shows <b>N similar picture(s)</b>: click it to see only them, grouped. Nothing is deleted or changed.</p>
<ul>
  <li>Each picture becomes a "fingerprint" of its shapes, taken from a very small grey copy, already turned the way the photo is shown. Pictures with nearly the same fingerprint are put in one group; a picture that looks like two others joins all three.</li>
  <li>Formats: JPEG, PNG, WebP, GIF (the first frame), BMP and TIFF. HEIC and camera RAW files only take part when they carry a JPEG thumbnail inside (nearly every RAW does; HEIC hardly ever).</li>
  <li>Identical copies of one file are already shown as copies; they only come in here when there is also another picture that looks like them.</li>
  <li>The fingerprints are kept in the program's cache folder: next time, only new or changed pictures are read. A fingerprint not used for 90 days is forgotten.</li>
</ul>
<p><b>Limitations:</b> only the overall shape of the picture counts, not what it shows. A <b>cropped</b> photo, or one turned by hand or mirrored, is not recognized, and two different pictures laid out alike (screenshots of the same program, pages of text, skies) may be taken as similar. Pictures of a single colour are left out. The first time, every picture is read whole: on an external hard disk, a few thousand photos take minutes (about 2 minutes for 3,700 photos), with the progress in the footer.</p>
<h4>Compare and send to the Trash</h4>
<p>With the <b>similar pictures</b> or the <b>identical copies</b> filter on, the footer shows <b>Compare</b>; double-clicking a row's group tag also opens the comparison at that group. The window shows one group at a time, side by side:</p>
<ul>
  <li>Each file shows its picture, name, folder, size in pixels and in bytes, and the date the photo was taken (or, without it, when it was last modified). The one with most pixels (or, on a tie, the largest file) gets the <b>★ largest</b> badge, and the oldest one <b>oldest</b>.</li>
  <li>Click a picture to see it big; the arrow keys go through the group's pictures (while big) or through the groups. <b>Esc</b> goes back or closes.</li>
  <li>Tick <b>Send to the Trash</b> on the files you don't want, in as many groups as you like. <b>Keep only the largest</b> ticks every file of the group but the largest.</li>
  <li>Nothing moves until you click <b>Move to the Trash</b> and confirm. If every file of some group is ticked, the confirmation says so. The files go to the <b>system's Trash</b> (where they can be recovered until it is emptied) and leave the list; the groups are made again.</li>
</ul>
<p><b>Limitations:</b> sending to the Trash is not in the History: to bring a file back, use the system's Trash (on macOS, drag it from the Trash to a folder: the Finder's <b>Put Back</b> does not show for it). Files that are not pictures (identical copies of videos, songs) show without a picture, only their details. On a disk without a Trash (some USB sticks and network disks), the file stays where it is and the program says so.</p>`,
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
<h4>Sending to another folder</h4>
<p>Next to the Rename button, the picker says where the files go: <b>In the same folder</b> (the usual: only renames), <b>Move to another folder…</b> or <b>Copy to another folder…</b>. It is for sorting a downloads folder straight into your library, also on another disk. The choice is not saved and goes back to "In the same folder" after each send: the list then shows the files in their new place, and sending them again would send them once more.</p>
<ul>
  <li>The folders the rules make (artist and album, movie, show and season, author, comic series) go inside the chosen folder: <code>Target/Artist/Album (1999)/01 - Song.flac</code>.</li>
  <li>Other files keep their subfolders from the folder you added: with <code>Photos</code> added, <code>Photos/2024/Beach/x.jpg</code> goes to <code>Target/2024/Beach/x.jpg</code>. Files added one by one go straight into the target.</li>
  <li>Every visible file goes, even those keeping their names. The Folder column shows where each will be.</li>
  <li><b>Copy</b>: the originals stay exactly as they are, and the copies get the new names. Covers, <code>.nfo</code> files, playlists and written tags go to the copies. <code>.cue</code>/<code>.m3u</code> lists copied along name the copies; the originals do not change. Each file's modification date is kept on its copy.</li>
  <li>Folders left empty at the source are deleted, but only those inside the folder you added: it, and those outside it, stay.</li>
  <li><b>Move</b> to another disk: the file is copied and only deleted from the source disk once the copy is complete and checked.</li>
  <li>While copying, the progress and a <b>Stop</b> button show: what was copied stays (and can be undone), the rest is left as it was.</li>
  <li>Each copy is made under a passing name and only gets its final name when whole; if the program closes halfway, the half copy is deleted at the next start. Nothing is overwritten: a name that already exists in the target shows as a conflict.</li>
  <li><b>Undo</b> of a copy deletes the copies made, but only those still as Renome left them. A copy you edited afterwards stays, and the notice says which. Undoing a move brings the files back, even from another disk.</li>
</ul>
<p><b>Limitations</b>: free space in the target is not checked first; if it runs out, the files that do not fit fail (leaving nothing halfway) and show in the error list. Copying many large files takes the disk's time: Renome does not make copying faster. Folders added as items are copied or moved whole.</p>
<h4>History</h4>
<p>Each rename is recorded in the <b>History</b>, even after closing the program. There you can:</p>
<ul>
  <li><b>Undo</b>: brings back the previous names. While undoing, a bar shows how far it is (the songs whose tags are going back, then the files coming back from another disk), with a <b>Stop</b> button. Once stopped, what is back stays so and the rest remains in the History, to undo later. Limitation: a large file coming back from another disk finishes before it stops.</li>
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
<p>The gear opens the options, split into <b>General</b>, <b>Renaming</b>, <b>Online searches</b>, <b>Accounts and keys</b> (each service with a key shows <b>set up</b>, and the menu counts how many are ready) and <b>Shortcuts</b>:</p>
<ul>
  <li><b>Language</b> and <b>Theme</b> (light, dark or system).</li>
  <li><b>Text size</b>: <b>Normal</b>, <b>Large</b> (15% bigger) or <b>Larger</b> (30% bigger). The whole window grows in proportion, like a browser's zoom; on a small screen, make the window bigger too.</li>
  <li><b>Remember rules when closing the app</b>.</li>
  <li><b>Include hidden files when adding folders</b>.</li>
  <li><b>Ask for confirmation before renaming</b>.</li>
  <li><b>Turn rules off after renaming</b>.</li>
  <li><b>Warn about names Windows does not accept</b>: characters such as <code>:</code> and <code>?</code>, a dot or space at the end, and reserved names such as <code>CON</code> and <code>NUL</code>. Useful for files headed to Windows. On a USB stick or disk formatted for Windows (FAT, exFAT), such names are not accepted at all: there they are an error, with or without this option.</li>
  <li><b>Online searches: keep the answers for</b> 1 day, 7 days (the default), 30 days, 6 months, or do not keep. The answers of TMDB, MusicBrainz, AcoustID, Discogs, Open Library, the Grand Comics Database, Comic Vine, Metron, MangaDex, AniList and MangaUpdates are kept on disk; looking up the same files again is instant and does not use up the services' limits (the GCD without an account takes only about 30 requests an hour). TMDB allows keeping them up to 6 months only, AniList's are kept 7 days at most (its terms do not allow making a copy of its data) and Discogs' last only 6 hours, whatever the choice. Keys and passwords are not kept with them. <b>Clear</b> deletes them all and shows how much they take. Limitation: a show still airing, a comic series still running or a fix made in a catalogue shows up only after the kept answer expires; to see it now, clear.</li>
  <li><b>TMDB</b>, <b>Google Books</b>, <b>Comic Vine</b>, <b>Metron</b>, <b>Grand Comics Database</b> and <b>Discogs</b>: the keys, the tokens and the account the movie, series, book, comic and music rules use; each has a <b>Test</b> button.</li>
  <li>The keys, the token and the password stay only on this computer, <b>encrypted in the system's password store</b> (GNOME Keyring or KDE Wallet on Linux, Credential Manager on Windows, Keychain on macOS), not in the options file. Limitation: a system without a password store (some minimal Linux installs) has nowhere to encrypt them; then they stay in the options file, unencrypted, and the Options say so. The store protects them from whoever reads the disk's files, not from another program running in your own session.</li>
  <li>When the store is locked (for example, with automatic login on Linux), the system asks for its password; the program opens anyway, and the keys appear once it is unlocked. On macOS, as Renome is not signed by Apple, the system may ask whether it can use the Keychain, again with each new version; choose <b>Always Allow</b>. If access is denied (or the store is not unlocked), the keys kept there are neither deleted nor replaced: the Options say so and have a <b>Try again</b> button.</li>
</ul>
<p>Options are stored in <code>~/.config/renome</code> (Linux), <code>%APPDATA%\\renome</code> (Windows) or <code>~/Library/Application Support/renome</code> (macOS). The history is in <code>~/.local/share/renome/history</code> on Linux and in the same folder as the options on the other systems. The kept answers of the searches are in <code>~/.cache/renome/answers</code> (Linux), <code>%LOCALAPPDATA%\\renome\\answers</code> (Windows) or <code>~/Library/Caches/renome/answers</code> (macOS).</p>`,
  },
];
