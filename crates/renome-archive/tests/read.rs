//! The test archives are from libarchive's own tests (BSD license).

use std::io::Read;
use std::path::Path;

/// Name, folder, other, encrypted, content.
type Seen = (String, bool, bool, bool, Vec<u8>);

fn list(name: &str) -> Result<Vec<Seen>, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    let mut out = Vec::new();
    renome_archive::read(&path, |e, d| {
        let mut v = Vec::new();
        if !e.dir && !e.other && !e.encrypted {
            d.read_to_end(&mut v).map_err(|e| e.to_string())?;
        }
        out.push((e.name.clone(), e.dir, e.other, e.encrypted, v));
        Ok(true)
    })?;
    Ok(out)
}

#[test]
fn reads_rar4() {
    let v = list("rar4.rar").unwrap();
    let names: Vec<&str> = v.iter().map(|e| e.0.as_str()).collect();
    assert_eq!(
        names,
        [
            "test.txt",
            "testlink",
            "testdir/test.txt",
            "testdir",
            "testemptydir"
        ]
    );
    assert_eq!(v[0].4, b"test text document\r\n");
    assert_eq!(v[2].4, b"test text document\r\n");
    // A link and folders, told apart.
    assert!(v[1].2 && !v[1].1);
    assert!(v[3].1 && v[4].1);
}

#[test]
fn reads_rar5() {
    let v = list("rar5.rar").unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].0, "test.bin");
    assert_eq!(v[0].4.len(), 1200);
}

#[test]
fn tells_locked_entries() {
    let v = list("rar4-locked.rar").unwrap();
    assert!(v.iter().all(|e| e.3));
}

#[test]
fn stops_when_asked_and_refuses_what_is_not_rar() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/rar4.rar");
    let mut seen = 0;
    renome_archive::read(&path, |_, _| {
        seen += 1;
        Ok(false)
    })
    .unwrap();
    assert_eq!(seen, 1);
    let d = std::env::temp_dir().join(format!("renome-archive-{}", std::process::id()));
    std::fs::write(&d, b"Rar!\x1a\x07\x00 not really").unwrap();
    assert!(renome_archive::read(&d, |_, _| Ok(true)).is_err());
    let _ = std::fs::remove_file(&d);
    assert!(renome_archive::is_rar(b"Rar!\x1a\x07\x01\x00"));
    assert!(!renome_archive::is_rar(b"PK\x03\x04"));
}
