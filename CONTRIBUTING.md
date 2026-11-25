# Contributing

## Layout

| Crate | What lives there |
|---|---|
| `crates/renome-core` | Everything that decides something about files: rules, preview, apply and undo, readers and writers of each format. No network, no UI. |
| `crates/renome-services` | The catalogue clients (TMDB, MusicBrainz, …) and the lookup *flows* shared by the window and the command line. |
| `crates/renome-cli` | The command line (`renome-cli`). |
| `crates/renome-archive` | RAR reading (libarchive, vendored). |
| `src-tauri` | The window's commands: they call the crates above and nothing else. |
| `src` | The Svelte page: shows what Rust computed. |

**One rule above all: logic lives in the Rust crates.** If the window and the
command line both need a decision, it goes in `renome-core` or
`renome-services`; never copy it into TypeScript or into the CLI.

## Before a commit

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p renome-core -p renome-services -p renome-cli -p renome-archive
npm run check
```

CI runs the same, plus `cargo deny`, the documentation build and a coverage
floor, on Linux, Windows and macOS.

## Style

- No `unwrap()` outside tests (a lint forbids it). Handle the case, or write
  `expect("why it cannot fail")`.
- A new reader of a file format gets a seed in
  `crates/renome-core/tests/robustness.rs`.
- A bug fix comes with a test that fails without it.
- Text shown to people goes through `src/lib/i18n.ts` (pt, en, es).
- Keep `SPEC.md`, the README and the in-app manual in step with the behaviour.
- Commits: a short imperative summary; one logical change each.
