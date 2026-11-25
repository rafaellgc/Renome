# Security policy

## Reporting a vulnerability

Please report security problems privately through GitHub's
**Security → Report a vulnerability** form on this repository, not in a public
issue. Say which version you use, what you did and what happened. You will
get an answer within a week; a confirmed problem is fixed in the next release
and credited if you wish.

## Supported versions

Only the latest release is supported. Updates are signed; the program checks
the signature before installing one.

## What Renome protects, and from what

- **Your files.** Renaming is journaled (a crash in the middle is recovered at
  the next start), history writes are atomic, and every change can be undone.
  Files are never overwritten by a rename.
- **Files that come from elsewhere.** Every reader treats a file as hostile:
  damaged or crafted files must give an error, never a crash
  (`crates/renome-core/tests/robustness.rs` damages every supported format and
  checks it). Archives downloaded from the internet are unpacked with a size
  limit.
- **Your keys** (catalogue API keys, server credentials) are kept in the
  system's keyring, not in the settings file (the program writes no log files).
- **The window** has a Content-Security-Policy and only the permissions listed
  in `src-tauri/capabilities/default.json`.
- **Dependencies** are checked on every push (`cargo deny`: advisories,
  licences, sources).

## What it does not protect against

Renome runs with your own permissions and renames what you ask it to rename.
Books with DRM are refused, not unlocked.
