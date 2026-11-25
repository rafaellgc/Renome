//! Pins what `after::inside` does to comics and books and what it reports,
//! so it can be taken apart without changing it. `GOLDEN_PRINT=1` prints.
#![allow(clippy::unwrap_used)]

use renome_core::History;
use renome_services::Quiet;
use renome_services::after::{InsideJob, inside};
use serde_json::json;
use std::io::{Read, Write};
use std::path::Path;

fn zip_of(path: &Path, files: &[(&str, &str)]) {
    let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let o = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default());
    for (name, body) in files {
        z.start_file(*name, o).unwrap();
        z.write_all(body.as_bytes()).unwrap();
    }
    z.finish().unwrap();
}

fn entry(path: &Path, name: &str) -> String {
    let Ok(f) = std::fs::File::open(path) else {
        return "<missing>".into();
    };
    let mut z = zip::ZipArchive::new(f).unwrap();
    let mut s = String::new();
    match z.by_name(name) {
        Ok(mut e) => {
            e.read_to_string(&mut s).unwrap();
            s
        }
        Err(_) => "<none>".into(),
    }
}

fn names(path: &Path) -> Vec<String> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap_or_default().to_string()).collect()
}

const OPF: &str = r#"<?xml version="1.0" encoding="utf-8"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">x</dc:identifier><dc:title>Old</dc:title><dc:language>pt</dc:language></metadata><manifest/><spine/></package>"#;
const CONTAINER: &str = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;

fn job(path: &Path, extra: serde_json::Value) -> InsideJob {
    let mut v = json!({"path": path});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn inside_does_not_change() {
    let dir = tempfile::tempdir().unwrap();
    let t = dir.path();
    let hist = History::new(t.join("history"));
    let comic = json!({"series": "Capitão Aurora", "number": "002", "year": 2016, "title": "A Volta"});
    let a = t.join("a.cbz");
    zip_of(&a, &[("01.jpg", "x")]);
    let fresh = t.join("fresh.cbz");
    zip_of(&fresh, &[("01.jpg", "x"), ("ComicInfo.xml", "<ComicInfo><Series>Old</Series></ComicInfo>")]);
    let wipe = t.join("wipe.cbz");
    zip_of(&wipe, &[("01.jpg", "x"), ("ComicInfo.xml", "<ComicInfo><Series>Old</Series></ComicInfo>")]);
    let txt = t.join("notes.txt");
    std::fs::write(&txt, "x").unwrap();
    let gone = t.join("gone.cbz");
    let book = t.join("b.epub");
    zip_of(&book, &[("mimetype", "application/epub+zip"), ("META-INF/container.xml", CONTAINER), ("content.opf", OPF)]);
    let jobs = vec![
        job(&a, json!({"comic": comic, "web": "https://example.org/i/2"})),
        job(&fresh, json!({"comic": comic, "fresh": true})),
        job(&wipe, json!({"fresh": true})),
        job(&txt, json!({"comic": comic})),
        job(&gone, json!({"comic": comic})),
        job(&book, json!({"book": {"title": "Novo Título", "authors": ["Ana Mar"]}, "language": "pt"})),
        job(&t.join("none.pdf"), json!({"book": {"title": "T"}})),
    ];
    let out = inside(&Quiet, &hist, None, jobs).await.unwrap();
    let mut all = format!(
        "{}\n{:?}\n{:?}\n",
        serde_json::to_string(&out.report).unwrap(),
        out.written,
        out.moved
    );
    for (p, n) in [(&a, "ComicInfo.xml"), (&fresh, "ComicInfo.xml"), (&wipe, "ComicInfo.xml"), (&book, "content.opf")] {
        all.push_str(&format!("{:?} {}\n{}\n", names(p), n, entry(p, n)));
    }
    let all = all.replace(t.to_str().unwrap(), "T");
    if std::env::var_os("GOLDEN_PRINT").is_some() {
        println!("{all}");
    }
    let sum = xxhash_rust::xxh3::xxh3_64(all.as_bytes());
    println!("hash {sum:016x}");
    assert_eq!(sum, GOLDEN, "what `inside` does changed");
}

const GOLDEN: u64 = 0x1067ddb0d78383b1;
