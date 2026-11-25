#![allow(clippy::unwrap_used)] // a benchmark that cannot set itself up should stop

//! How long the work a user waits for takes on a big list: scanning a
//! folder, the preview after each change to the rules (the most repeated
//! step: it runs on every keystroke), and sorting by natural order.
//!
//! `cargo bench -p renome-core`. The figures of the last run are in
//! `docs/PERFORMANCE.md`.

use std::collections::{HashMap, HashSet};

use criterion::{Criterion, criterion_group, criterion_main};
use renome_core::files::{SortKey, natural_cmp, sort_files};
use renome_core::rules::{CaseMode, Rule, Step, Target};
use renome_core::{FileEntry, ScanOptions, View, preview, scan};

const FILES: usize = 10_000;

fn folder() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..FILES {
        let sub = dir.path().join(format!("folder {}", i % 50));
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join(format!("Some.Made.Up.Name.{i}.2020.1080p.mkv")),
            b"x",
        )
        .unwrap();
    }
    dir
}

fn steps() -> Vec<Step> {
    let step = |rule| Step {
        enabled: true,
        only: String::new(),
        rule,
    };
    vec![
        step(Rule::Replace {
            find: ".".into(),
            replace: " ".into(),
            case_sensitive: true,
            first_only: false,
            target: Target::Name,
        }),
        step(Rule::Case {
            mode: CaseMode::Title,
            target: Target::Name,
            keep: String::new(),
        }),
    ]
}

fn benches(c: &mut Criterion) {
    let dir = folder();
    let mut group = c.benchmark_group("big list");
    group.sample_size(10);
    group.bench_function("scan 10k files", |b| {
        b.iter(|| {
            scan(
                &[dir.path().to_path_buf()],
                &ScanOptions::default(),
                &HashSet::new(),
            )
        })
    });
    let files: Vec<FileEntry> = scan(
        &[dir.path().to_path_buf()],
        &ScanOptions::default(),
        &HashSet::new(),
    )
    .files;
    let steps = steps();
    let typed = HashMap::new();
    group.bench_function("preview 10k files, 2 rules", |b| {
        b.iter(|| preview(&files, &steps, &typed, &View::default()))
    });
    group.bench_function("sort 10k files by name", |b| {
        b.iter(|| {
            let mut f = files.clone();
            sort_files(&mut f, SortKey::Name, false);
            f
        })
    });
    group.finish();
    c.bench_function("natural_cmp", |b| {
        b.iter(|| {
            natural_cmp(
                "Episode 12 - The Long Name 2020",
                "Episode 102 - The Long Name 2020",
            )
        })
    });
}

criterion_group!(all, benches);
criterion_main!(all);
