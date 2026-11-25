#![allow(clippy::unwrap_used)] // a test that cannot set itself up should fail

//! Damaged files must never make a reader panic: every reader that opens a
//! file from the disk gets each seed file truncated, with bytes changed,
//! zeroed, duplicated and emptied, and must answer with an error or nothing.
//! (The release build aborts on a panic, so a panic here is a crash there.)

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use renome_core::bookconv::{self, Source};
use renome_core::{ScanOptions, comics, ebook, fingerprint, kind, mkvsubs, music, scan, similar};
use renome_core::{subtext, videotags};

/// The last panic's message and place (the hook is silenced during the run).
static LAST_PANIC: Mutex<String> = Mutex::new(String::new());
/// How many panics the hook has seen.
static PANICS: AtomicUsize = AtomicUsize::new(0);

/// A small deterministic generator, so a failure repeats.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let o = zip::write::SimpleFileOptions::default();
        for (n, b) in files {
            z.start_file(*n, o).unwrap();
            z.write_all(b).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

/// (file name, bytes) of every seed: the fixtures on disk and made-up ones.
fn seeds() -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = Vec::new();
    // Relative to the crate, as the other tests are (so the same test
    // runs from a copy of the fixtures on another machine); a folder that
    // is not there only means fewer seeds.
    for dir in ["tests/fixtures", "../renome-archive/tests/data"] {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            v.push((
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            ));
        }
    }
    let srt = "1\n00:00:01,000 --> 00:00:03,000\nHello there\n\n2\n00:00:04,000 --> 00:00:06,500\n<i>Second line</i>\nand a third\n\n";
    let vtt = "WEBVTT\n\n00:01.000 --> 00:03.000\nHello <c.red>there</c>\n\n00:04.000 --> 00:06.500\nSecond\n";
    let ass = "[Script Info]\nTitle: x\n\n[V4+ Styles]\nFormat: Name, Fontname\nStyle: Default,Arial\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Hello {\\b1}there{\\b0}\\Nagain\n";
    let sub = "{25}{75}Hello|there\n{100}{150}Second\n";
    let smi =
        "<SAMI><BODY><SYNC Start=1000><P Class=ENCC>Hello<SYNC Start=3000><P>&nbsp;</BODY></SAMI>";
    let fb2 = "<?xml version=\"1.0\" encoding=\"utf-8\"?><FictionBook xmlns=\"http://www.gribuser.ru/xml/fictionbook/2.0\"><description><title-info><genre>prose</genre><author><first-name>A</first-name><last-name>B</last-name></author><book-title>T</book-title><lang>en</lang></title-info></description><body><section><title><p>One</p></title><p>Hello <emphasis>there</emphasis>.</p></section></body></FictionBook>";
    let html = "<html><head><title>T</title><meta name=\"author\" content=\"A\"></head><body><h1>One</h1><p>Hello <b>there</b>.</p><ul><li>a</li><li>b</li></ul></body></html>";
    let xhtml = "<?xml version=\"1.0\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>c</title></head><body><h1>One</h1><p>Hello.</p></body></html>";
    let opf = "<?xml version=\"1.0\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"id\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"id\">x</dc:identifier><dc:title>T</dc:title><dc:creator>A</dc:creator><dc:language>en</dc:language></metadata><manifest><item id=\"c\" href=\"c.xhtml\" media-type=\"application/xhtml+xml\"/></manifest><spine><itemref idref=\"c\"/></spine></package>";
    let container = "<?xml version=\"1.0\"?><container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>";
    let info = "<?xml version=\"1.0\"?><ComicInfo><Series>S</Series><Number>3</Number><Year>2001</Year></ComicInfo>";
    let docx = "<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>One</w:t></w:r></w:p><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>";
    let pdf = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]/Contents 4 0 R>>endobj\n4 0 obj<</Length 35>>stream\nBT /F1 12 Tf 10 100 Td (Hello) Tj ET\nendstream endobj\ntrailer<</Root 1 0 R>>\n%%EOF\n";
    for (n, t) in [
        ("a.srt", srt),
        ("a.vtt", vtt),
        ("a.ass", ass),
        ("a.ssa", ass),
        ("a.sub", sub),
        ("a.smi", smi),
        ("a.fb2", fb2),
        ("a.html", html),
        ("a.txt", "One\n\nHello there.\nSecond paragraph.\n"),
        ("a.xml", info),
    ] {
        v.push((n.into(), t.as_bytes().to_vec()));
    }
    v.push((
        "a.epub".into(),
        zip_of(&[
            ("mimetype", b"application/epub+zip"),
            ("META-INF/container.xml", container.as_bytes()),
            ("content.opf", opf.as_bytes()),
            ("c.xhtml", xhtml.as_bytes()),
        ]),
    ));
    v.push((
        "a.cbz".into(),
        zip_of(&[("ComicInfo.xml", info.as_bytes()), ("01.jpg", &v[0].1)]),
    ));
    v.push((
        "a.docx".into(),
        zip_of(&[("word/document.xml", docx.as_bytes())]),
    ));
    v.push(("a.fbz".into(), zip_of(&[("a.fb2", fb2.as_bytes())])));
    v.push(("a.htmlz".into(), zip_of(&[("index.html", html.as_bytes())])));
    v.push(("a.pdf".into(), pdf.to_vec()));
    v
}

/// One damaged copy of `seed`: kind `k` of damage, picked by `rng`.
fn damage(seed: &[u8], k: usize, rng: &mut Rng) -> Vec<u8> {
    let mut b = seed.to_vec();
    let n = b.len();
    match k {
        0 => b.clear(),
        1 => b.truncate(1.min(n)),
        2 => b.truncate(n / 4),
        3 => b.truncate(n / 2),
        4 => b.truncate(n.saturating_sub(1)),
        5 => b.truncate(rng.below(n)),
        6 => {
            // A few bytes changed anywhere.
            for _ in 0..1 + rng.below(4) {
                if n > 0 {
                    let i = rng.below(n);
                    b[i] = rng.next() as u8;
                }
            }
        }
        7 => {
            // Many bytes changed.
            for _ in 0..n / 20 + 1 {
                if n > 0 {
                    let i = rng.below(n);
                    b[i] ^= 1 << rng.below(8);
                }
            }
        }
        8 => {
            // A run of zeros or 0xFF (lengths and counts gone wild).
            if n > 0 {
                let from = rng.below(n);
                let to = (from + 1 + rng.below(64)).min(n);
                let fill = if rng.below(2) == 0 { 0 } else { 0xFF };
                b[from..to].fill(fill);
            }
        }
        9 => {
            // The head intact, the rest garbage.
            let keep = rng.below(n.min(96) + 1);
            for x in b.iter_mut().skip(keep) {
                *x = rng.next() as u8;
            }
        }
        _ => {
            // A piece repeated.
            if n > 0 {
                let from = rng.below(n);
                let to = (from + 1 + rng.below(256)).min(n);
                let piece = b[from..to].to_vec();
                for _ in 0..8 {
                    b.splice(from..from, piece.iter().copied());
                }
            }
        }
    }
    b
}

const KINDS: usize = 11;
const ROUNDS: usize = 10;

/// Every reader that takes a file: its name and what it runs. A panic or a
/// stall is reported against the name.
fn read_all(path: &Path, ext: &str, bytes: &[u8], failures: &mut Vec<String>, label: &str) {
    let mut run = |name: &str, mut f: &mut dyn FnMut()| {
        if std::env::var_os("ROBUSTNESS_TRACE").is_some() {
            eprintln!("{label}: {name}");
        }
        let t = Instant::now();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut f));
        if r.is_err() {
            let at = LAST_PANIC.lock().unwrap().clone();
            failures.push(format!("{label}: {name} panicked: {at}"));
        } else if t.elapsed() > Duration::from_secs(20) {
            failures.push(format!("{label}: {name} took {:?}", t.elapsed()));
        }
    };
    run("kind::read", &mut || {
        let _ = kind::read(path);
    });
    run("music::read", &mut || {
        let _ = music::read(path);
    });
    run("music::probe", &mut || {
        let _ = music::probe(path);
    });
    run("videotags::read", &mut || {
        let _ = videotags::read(path);
    });
    run("mkvsubs::tracks", &mut || {
        let _ = mkvsubs::tracks(path);
    });
    run("comics::read", &mut || {
        let _ = comics::read(path);
    });
    run("ebook::read_as", &mut || {
        let _ = ebook::read_as(path, ext);
    });
    run("subtext::summary", &mut || {
        let _ = subtext::summary(path);
    });
    run("fingerprint::of", &mut || {
        let _ = fingerprint::of(path);
    });
    run("similar::fingerprint_file", &mut || {
        let _ = similar::fingerprint_file(path);
    });
    run("subtext::decode", &mut || {
        let (text, _) = subtext::decode(bytes, None);
        let _ = subtext::detect(&text);
        let _ = subtext::to_utf8(bytes, Some("en"));
    });
    run("bookconv::source_of", &mut || {
        let _ = bookconv::source_of(path);
    });
    run("bookconv::drm", &mut || {
        for s in [Source::Epub, Source::Kindle, Source::Pdf] {
            let _ = bookconv::drm::locked(path, s);
        }
    });
    run("scan+photos", &mut || {
        let dir = path.parent().unwrap().to_path_buf();
        let found = scan(&[dir], &ScanOptions::default(), &HashSet::new());
        let refs: Vec<_> = found.files.iter().collect();
        renome_core::photos::read_all(&refs, &|_, _| {});
    });
    for s in [
        Source::Epub,
        Source::Kindle,
        Source::Fb2,
        Source::Fbz,
        Source::Docx,
        Source::Odt,
        Source::Html,
        Source::Htmlz,
        Source::Txt,
        Source::Pdf,
    ] {
        // `bookconv::read` catches panics itself and answers "bookBroken".
        if std::env::var_os("ROBUSTNESS_TRACE").is_some() {
            eprintln!("{label}: bookconv::read as {s:?}");
        }
        let t = Instant::now();
        let before = PANICS.load(Ordering::SeqCst);
        let _ = bookconv::read(path, s);
        if PANICS.load(Ordering::SeqCst) != before {
            let at = LAST_PANIC.lock().unwrap().clone();
            failures.push(format!("{label}: bookconv::read as {s:?} panicked: {at}"));
        } else if t.elapsed() > Duration::from_secs(20) {
            failures.push(format!(
                "{label}: bookconv::read as {s:?} took {:?}",
                t.elapsed()
            ));
        }
    }
}

#[test]
fn damaged_files_never_panic_a_reader() {
    std::panic::set_hook(Box::new(|info| {
        let at = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        *LAST_PANIC.lock().unwrap() = format!("{msg} at {at}");
        PANICS.fetch_add(1, Ordering::SeqCst);
    }));
    // The detector works: a panic is seen and counted.
    let before = PANICS.load(Ordering::SeqCst);
    let _ = std::panic::catch_unwind(|| panic!("canary"));
    assert_eq!(PANICS.load(Ordering::SeqCst), before + 1);
    let dir = tempfile::tempdir().unwrap();
    let mut failures: Vec<String> = Vec::new();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for (name, seed) in seeds() {
        let ext = name.rsplit('.').next().unwrap_or("").to_string();
        for k in 0..KINDS {
            for round in 0..if k <= 4 { 1 } else { ROUNDS } {
                let bytes = damage(&seed, k, &mut rng);
                // Each case in a folder of its own: the scan reads only it.
                let case: PathBuf = dir.path().join(format!("{k}-{round}"));
                std::fs::create_dir_all(&case).unwrap();
                let path = case.join(format!("file.{ext}"));
                std::fs::write(&path, &bytes).unwrap();
                let label = format!("{name} damage {k} round {round}");
                read_all(&path, &ext, &bytes, &mut failures, &label);
                std::fs::remove_dir_all(&case).unwrap();
            }
        }
    }
    let _ = std::panic::take_hook();
    failures.sort();
    failures.dedup_by(|a, b| {
        a.split(": ").nth(1) == b.split(": ").nth(1)
            && a.split(" at ").nth(1) == b.split(" at ").nth(1)
    });
    assert!(
        failures.is_empty(),
        "{} readers failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
