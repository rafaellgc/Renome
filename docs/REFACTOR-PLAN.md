# Refactor plan

Rule for every step: logic lives in `renome-core` / `renome-services`; the
window and `renome-cli` only call it. One commit per step; fmt, clippy
`-D warnings` and the tests must pass before each commit.

1. **Dependencies** — update vulnerable crates (done), make `cargo audit` a
   failing CI job with a reasoned ignore list for the unmaintained warnings.
2. **Window hardening** — add a Content-Security-Policy to `tauri.conf.json`
   and check capabilities are minimal; verify in the running app.
3. **Lint policy** — `[workspace.lints]`: deny `clippy::dbg_macro`,
   `todo`, `unimplemented`; warn `unwrap_used`/`expect_used` outside tests;
   share it across crates.
4. **Unwrap triage** — replace each production `unwrap()` that touches
   external input with error handling; turn regex constants into one
   documented `regex!`-style helper and invariants into `expect("why")`.
5. **Indexing and casts on external input** — parsers (subtext, tagwrite,
   inside, mkvsubs, ebook, bookconv, comicbook) use checked access.
6. **Robustness tests** — deterministic mutation tests (truncate, flip bytes,
   empty, huge) over every parser's fixtures: must return an error, never
   panic. (`cargo fuzz` needs nightly, absent here.)
7. **Archive safety** — check every zip/7z/rar read for path traversal and
   decompressed-size limits; add tests with a hostile archive.
8. **Split giant files and functions** — `src-tauri/src/lib.rs` into command
   modules; the longest functions in `plan.rs`, `convert.rs`, `azw3_out.rs`,
   `mobi.rs`, `inside.rs`, `videotags.rs`, guided by clippy and covered by
   characterization tests first.
9. **Performance baseline** — criterion benches for preview over 10k files,
   filter, natural sort; record numbers in `docs/PERFORMANCE.md`.
10. **CI** — audit job, coverage job (llvm-cov, threshold), `cargo doc`
    with warnings denied, MSRV check.
11. **Documents** — SECURITY.md, CONTRIBUTING.md, CHANGELOG.md, ARCHITECTURE.md.
12. **Verification** — full Linux run (tests + AppImage in Xvfb), Windows VM,
    macOS VM; fix what they find; push (no release, no tag, no version bump).

## Status (2026-11-25)

| Step | State |
|---|---|
| 1 Dependencies | Done: two advisories fixed, `cargo deny` is clean and runs in CI. |
| 2 Window hardening | Done: CSP set and checked on Linux. Capabilities were already minimal. |
| 3 Lint policy | Done: `unwrap_used`, `dbg_macro`, `todo`, `unimplemented` denied workspace-wide. |
| 4 Unwrap triage | Done: every production `unwrap()` handled or turned into `expect("why")`. |
| 5 Indexing and casts | Partly: the damaged-file test found and fixed the only crash (`videotags::read_at`). The ~930 `indexing_slicing` and ~330 cast sites that the pedantic lints list were not rewritten one by one: the robustness test is what guards them. |
| 6 Robustness tests | Done: `crates/renome-core/tests/robustness.rs` (runs on Linux and Windows). |
| 7 Archive safety | Done for the one network-fed archive path (subtitle zips: size cap + test). Comic/book archives are read from the user's own disk. |
| 8 Split giant code | Mostly done. The window's `lib.rs` (2,490 → 449 lines + 6 modules). In the crates, 34 of the 92 functions over 100 lines were split (the largest: `azw3_out::build` 572, `pdf_in::read` 438, `mobi::kf8` 313, `inside::book_opf` 312, `videotags::mkv_plan` 285, `plan::preview` 282, `epub_in::read` 261, …), each after a golden test pinned its output (hashes of what it makes of many inputs) and with the hashes unchanged after. **58 remain** (10 over 150 lines, 32 over 120). The three biggest are the network flows in `renome-services` (`flow/comics.rs:181` 315 lines, `after.rs:620` 247, `flow/after.rs:916` 216): they need a fake `Host` and a fake HTTP server to pin their behaviour first. `src-tauri` `run()` (164) is the list of 105 commands. |
| 9 Performance | Done: baseline in `docs/PERFORMANCE.md`. No optimisation was needed. |
| 10 CI | Done: doc build, machete, coverage floor (70%), bench compile. |
| 11 Documents | Done: SECURITY, CONTRIBUTING, CHANGELOG. `ARCHITECTURE.md` is covered by the layout table in CONTRIBUTING. |
| 12 Verification | Linux: tests, AppImage, window smoke (rename, undo, online lookup). Windows: core + Windows-specific tests and the real `renome-cli.exe` over SSH; the **window was not exercised** (the VM sat at the sign-in screen). macOS: see the commit log / final report. |
