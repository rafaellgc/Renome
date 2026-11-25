//! Pins `flow::after::run` on the stages that need no network (what is written inside files, a stage that fails), so it can be taken
//! apart without changing it. `GOLDEN_PRINT=1` prints.
#![allow(clippy::unwrap_used)]

use renome_core::History;
use renome_services::flow::after::{Jobs, Services, run};
use renome_services::{Quiet, discogs, musicbrainz, openlibrary, subtitles};
use serde_json::json;
use std::io::Write;
use std::path::Path;

fn cbz(path: &Path) {
    let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let o = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default());
    z.start_file("01.jpg", o).unwrap();
    z.write_all(b"x").unwrap();
    z.finish().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn run_does_not_change() {
    let dir = tempfile::tempdir().unwrap();
    let t = dir.path();
    let (mb, dg, ol) = (
        musicbrainz::Client::default(),
        discogs::Client::default(),
        openlibrary::Client::default(),
    );
    let services = Services {
        tmdb_key: String::new(),
        tmdb_lang: "pt-BR".into(),
        mb: &mb,
        dg: &dg,
        ol: &ol,
        subs: subtitles::Auth::default(),
    };
    let a = t.join("a.cbz");
    cbz(&a);
    let list = t.join("missing.m3u");
    let job = |p: &Path| -> renome_services::after::InsideJob {
        serde_json::from_value(
            json!({"path": p, "comic": {"series": "Capitão Aurora", "number": "1"}}),
        )
        .unwrap()
    };
    let jobs = Jobs {
        playlists: vec![list],
        inside: vec![job(&a), job(&t.join("gone.cbz"))],
        ..Jobs::default()
    };
    let hist = History::new(t.join("history"));
    let rules = vec![json!({"kind": "comic", "on": true})];
    let r = run(&Quiet, &hist, None, false, &rules, jobs, &services).await;
    let all = format!(
        "{}\n{:?}\n{:?}\n{:?}",
        serde_json::to_string(&r).unwrap(),
        r.trashed,
        r.retagged,
        r.moved
    )
    .replace(t.to_str().unwrap(), "T");
    if std::env::var_os("GOLDEN_PRINT").is_some() {
        println!("{all}");
    }
    let sum = xxhash_rust::xxh3::xxh3_64(all.as_bytes());
    println!("hash {sum:016x}");
    assert_eq!(sum, GOLDEN, "what `run` does changed");
}

const GOLDEN: u64 = 0xee3db8d4a8d3e27c;
