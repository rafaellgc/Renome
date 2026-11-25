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
