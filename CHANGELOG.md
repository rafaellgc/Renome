# Changelog

Release notes are written for each GitHub release. This file lists what
changed in the code between releases and is kept for the unreleased work.

## Unreleased

### Documentation
- The command-line chapter of the manual (pt, en, es) is rewritten in four
  parts, for people who never used a terminal: a first walk-through, every
  rule and every `preview`/`apply` option with an example, every subcommand
  (history, sets, options, keys, servers, duplicates, update, manual, cache,
  completions), and scripts, exit codes and limitations. The examples are
  real runs, shown as command and output blocks in the window and in
  `renome-cli manual`.

## 0.39.0

### Fixes
- Moving a release's own folder into a library no longer leaves the empty
  folder behind: when nothing is left in it, it is removed (otherwise it is
  renamed beside with the "extras" suffix).
- Series: a file that carries the show's name but a title nothing like the
  episode's with that number (another edition of the cartoons) is no longer
  matched on its numbers alone; the options are offered to choose from.

### Security
- Fixed two vulnerable dependencies (`url` RUSTSEC-2026-0592, `cookie`
  RUSTSEC-2026-0594).
- A downloaded subtitle archive can no longer unpack to an unlimited size.
- The window now has a Content-Security-Policy.

### Robustness
- A damaged video (MKV/MP4) whose header claims a huge size no longer aborts
  the program.
- Release builds unwind on a panic, so the guards around third-party decoders
  work in the shipped program.
- A panic no longer leaves every later command failing on a poisoned lock.
- New test that damages every supported file format and checks no reader
  panics.

### Tests and tooling
- `cargo bench -p renome-core` (scan, preview, sort) and `docs/PERFORMANCE.md`.
- Coverage is measured (72% of lines) and CI keeps it from falling below 70%.

### Fixes found by the new golden tests
- A damaged Matroska file whose numbers overflowed the time of a subtitle
  block (a panic in debug builds, garbage in release) now saturates.

### Code
- All 92 functions of over 100 lines (up to 572) were split into steps, each
  with a golden test of its output written first (or the scenario tests
  that already pinned it): book writing and reading (AZW3, MOBI, PDF, EPUB,
  FB2, Word, OpenDocument), tags, names, comics, subtitles, the rename plan,
  undo, the lookups of every catalogue and the command line. The only long
  function left is the list of the window's commands (one macro, which
  cannot be cut), and 15 test scenarios that read from top to bottom.
- The lookups' network flows are pinned by tests that start a small fake
  catalogue in the test itself.
- The dependencies are optimised in the dev profile: the tests that decode
  pictures went from minutes to seconds.
- `unwrap()` is forbidden outside tests; every remaining invariant says why.
- The window's `lib.rs` (2,490 lines) is split into modules by subject.
- Documentation builds without warnings; CI checks it and a coverage floor.
