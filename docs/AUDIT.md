# Quality audit (2026-11-25, v0.38.1)

Measured against the professional-software checklist (architecture, code
quality, tests, robustness, security, performance, UX, observability, docs,
build and process). Numbers come from tools, not from reading samples.

## Measured

| Item | Result |
|---|---|
| `cargo fmt --check` | clean |
| `clippy -D warnings` (all crates, all targets) | clean |
| Tests | 418 core + 49 services/cli + 18 others, all pass (7 ignored) |
| `cargo machete` | no unused dependency |
| `cargo deny` licenses / bans / sources | ok |
| `cargo audit` | **2 vulnerabilities** (url, cookie) — fixed in 2ce4565; 7 "unmaintained" warnings from transitive crates (chrono, paste, zbus family, proc-macro-error, glib 0.18 via GTK3) that cannot be fixed here |
| Production `unwrap()` | 320 (regex constants and group-0 captures are most; the rest to be triaged) |
| `unsafe` blocks | 9, all FFI for no-replace rename and `syncfs` (apply.rs), env var in CLI/test |
| `clippy::indexing_slicing` | 933 sites (panic on out-of-range; `panic = "abort"` in release) |
| `clippy::too_many_lines` / `cognitive_complexity` | 75 / 15 functions |
| Casts that can truncate or lose sign | ~330 sites |
| Fuzzing | none |
| Benchmarks | none |
| Tauri CSP | `null` (no policy) |
| Docs | README, SPEC, in-app manual; no SECURITY.md, CONTRIBUTING.md, CHANGELOG.md |
| TS/Svelte | thin: flows live in Rust (renome-services::flow); 5 loose `any`/`console` hits |

## Verdict by area

- **Architecture**: good. Logic is in `renome-core` / `renome-services`; the
  window (`src-tauri`) and `renome-cli` call it. Remaining weak points:
  `src-tauri/src/lib.rs` (2,490 lines, 105 commands in one file), `plan.rs`
  (5,423), `convert.rs` (3,610).
- **Code quality**: fmt/clippy are clean, but pedantic-level problems remain
  (long functions, unchecked indexing, unwraps, lossy casts).
- **Tests**: broad (500+), but no coverage figure is enforced, no fuzzing of
  parsers (subtitles, e-books, archives, PDF, tags) and no property tests.
- **Robustness**: crash-safe renames (journal), atomic history; unwraps and
  slicing on external input are the open risk.
- **Security**: advisories fixed; secrets in keyring; updater is signed.
  Open: CSP, archive extraction limits to be verified, no SECURITY.md.
- **Performance**: nothing measured; no benches, no budgets.
- **Process**: CI runs fmt, clippy, tests (3 OSes), deny; lacks audit as a
  failing job, coverage, doc build, MSRV, and docs for contributors.

## After the refactor (same day)

| Item | Before | After |
|---|---|---|
| Production `unwrap()` | 320 | 0 (denied by lint) |
| Advisories | 2 vulnerabilities | 0 |
| Functions over 100 lines | 92 | 58 (the 10 over 150: 3 network flows, the window's `run()`, and 6 of 120-160 lines) |
| Readers that panic on damaged files | 2 found | 0 (test of every reader; one in a dependency handled by profile) |
| Window CSP | none | set |
| Coverage | 72.4% (unmeasured by CI) | floor of 70% enforced in CI |
