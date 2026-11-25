# Changelog

Release notes are written for each GitHub release. This file lists what
changed in the code between releases and is kept for the unreleased work.

## Unreleased

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

### Code
- `unwrap()` is forbidden outside tests; every remaining invariant says why.
- The window's `lib.rs` (2,490 lines) is split into modules by subject.
- Documentation builds without warnings; CI checks it and a coverage floor.
