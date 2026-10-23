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
  <li><b>Title Case, except of/the/and…</b>: → <code>The Lord of the Rings</code>. Short linking words (of, the, and, to, in, de, da, del, la, y…) stay lowercase, except at the start of the name.</li>
</ul>
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
    id: "apply",
    title: "Renaming and undoing",
    html: `
<p><b>Rename</b> (<kbd>Ctrl</kbd>+<kbd>Enter</kbd>) applies the new names of the visible files that change and have no conflict. The program asks for confirmation first (can be turned off in Options).</p>
<ul>
  <li>Renaming happens in two steps, through a temporary name. That is why names can be swapped (a ↔ b), and no file is ever overwritten.</li>
  <li>Folders and the files inside them can be renamed together: the contents first, then the folder.</li>
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
