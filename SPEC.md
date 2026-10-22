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
  `[`, `{` and the like are literal. Only visible files are renamed, and
  numbering only counts them.
- Files can be passed on the command line (`renome file1 file2 folder/`).
- Columns: current name, new name, folder, size, modification date.
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
| Case | lowercase; UPPERCASE; Sentence case; Title Case; Title Case except connectives (of, the, de, da…) |
| Insert | text, current date, file modification date, date the photo was taken (EXIF; optionally the modification date when missing, otherwise the name is unchanged) or the name of the folder the file is in; configurable date format; at the start, at the end or at a position (from the start or the end) |
| Number | template with `{n}` (number), `{name}` (current name) and `{folder}` (folder name); start, step, minimum digits (zero padding); optionally restart in each folder |
| Remove | spaces; repeated/leading/trailing spaces; accents; digits; custom characters |
| Remove range | from: position N or first occurrence of a text; to: end, position N or occurrence of a text; include the delimiters or not |
| Character at position | position N; optionally only if the current character is X (case-insensitive unless asked); replace with Y or delete |
| Extension | set a new one; lowercase/uppercase; remove |

Positions count **visible characters** (Unicode graphemes), starting at 1.

## 3. Preview

- Table: original name → new name, with a status per row:
  unchanged, ok, conflict (with the reason).
- The new name can be edited by hand in the table (overrides the rules for that row).
- Checks: empty name, `/` or null character, longer than 255 bytes,
  two files with the same target, target already exists outside the batch.
  On Windows, `\ : * ? " < > |` and a trailing dot or space are errors;
  elsewhere they are a non-blocking warning. On Windows and macOS, names
  that differ only in case count as the same name.
- Counters: total, changed, conflicts.

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
| Esc | Close the open dialog or menu |

On macOS, ⌘ replaces Ctrl. The list is also shown in Options.

## 6. Options

Saved in the user's config folder (e.g. `~/.config/renome/settings.json`).

- Language: English, Portuguese, Spanish. On first run, the system's.
- Theme: light, dark or system (default).
- Remember the rules between sessions (on).
- Include hidden files when adding folders (off).
- Ask for confirmation before renaming (on).
- Turn the rules off after renaming (on).
- Warn about characters Windows does not accept (on).
