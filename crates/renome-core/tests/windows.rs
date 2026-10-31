//! Renaming on Windows: what its file system does differently (names that
//! ignore case, reserved names, long paths, files held open, hidden and
//! system files, junctions, `\` in lists).
#![cfg(windows)]

use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use renome_core::{FileEntry, History, Preview, Problem, ScanOptions, Status, View, preview, scan};

fn files_in(dir: &Path, opts: &ScanOptions) -> Vec<FileEntry> {
    scan(&[dir.to_path_buf()], opts, &HashSet::new()).files
}

fn name(f: &FileEntry) -> String {
    f.path.file_name().unwrap().to_string_lossy().into_owned()
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// The preview with names typed by hand.
fn plan(files: &[FileEntry], names: &[(&Path, &str)]) -> Preview {
    let typed: HashMap<PathBuf, String> = names
        .iter()
        .map(|(p, n)| (p.to_path_buf(), n.to_string()))
        .collect();
    preview(files, &[], &typed, &View::default())
}

fn attrib(args: &[&str], path: &Path) {
    let ok = Command::new("attrib")
        .args(args)
        .arg(path)
        .status()
        .unwrap();
    assert!(ok.success());
}

#[test]
fn hidden_and_system_files_are_left_out() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::write(p.join("photo.jpg"), "x").unwrap();
    fs::write(p.join("desktop.ini"), "[.ShellClassInfo]").unwrap();
    fs::write(p.join("Thumbs.db"), "x").unwrap();
    attrib(&["+h", "+s"], &p.join("desktop.ini"));
    attrib(&["+h"], &p.join("Thumbs.db"));
    let seen = |hidden: bool| -> Vec<String> {
        let opts = ScanOptions {
            include_hidden: hidden,
            ..Default::default()
        };
        let mut v: Vec<String> = files_in(p, &opts).iter().map(name).collect();
        v.sort();
        v
    };
    assert_eq!(seen(false), ["photo.jpg"]);
    assert_eq!(seen(true), ["Thumbs.db", "desktop.ini", "photo.jpg"]);
}

#[test]
fn folders_marked_system_still_count() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    // A folder with a custom icon: Windows marks it system (or read-only).
    fs::create_dir(p.join("Custom")).unwrap();
    fs::write(p.join("Custom").join("a.txt"), "A").unwrap();
    attrib(&["+s"], &p.join("Custom"));
    // A hidden folder is left out, unless it is the one added.
    fs::create_dir(p.join("Secret")).unwrap();
    fs::write(p.join("Secret").join("b.txt"), "B").unwrap();
    attrib(&["+h"], &p.join("Secret"));
    let all: Vec<String> = files_in(p, &ScanOptions::default())
        .iter()
        .map(name)
        .collect();
    assert_eq!(all, ["a.txt"]);
    let inside: Vec<String> = files_in(&p.join("Secret"), &ScanOptions::default())
        .iter()
        .map(name)
        .collect();
    assert_eq!(inside, ["b.txt"]);
}

#[test]
fn case_only_change() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::write(p.join("Photo.JPG"), "x").unwrap();
    let files = files_in(p, &ScanOptions::default());
    let pv = plan(&files, &[(&p.join("Photo.JPG"), "photo.jpg")]);
    assert_eq!(pv.items[0].status, Status::Ok, "{:?}", pv.items[0].problem);
    let hist = tempfile::tempdir().unwrap();
    let h = History::new(hist.path());
    let r = h.apply(&pv.operations());
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(names(p), ["photo.jpg"]);
    let u = h.undo(r.history_id.as_deref().unwrap()).unwrap();
    assert!(u.errors.is_empty(), "{:?}", u.errors);
    assert_eq!(names(p), ["Photo.JPG"]);
}

#[test]
fn a_name_taken_in_another_case_is_a_conflict() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::write(p.join("a.txt"), "keep").unwrap();
    fs::write(p.join("b.txt"), "b").unwrap();
    let files: Vec<FileEntry> = files_in(p, &ScanOptions::default())
        .into_iter()
        .filter(|f| name(f) == "b.txt")
        .collect();
    let pv = plan(&files, &[(&p.join("b.txt"), "A.TXT")]);
    assert_eq!(pv.items[0].problem, Some(Problem::Exists));
    // Two files of the batch heading for one name in different cases.
    fs::write(p.join("c.txt"), "c").unwrap();
    let files = files_in(p, &ScanOptions::default());
    let pv = plan(
        &files,
        &[(&p.join("b.txt"), "New.txt"), (&p.join("c.txt"), "NEW.TXT")],
    );
    let dup = pv
        .items
        .iter()
        .filter(|i| i.problem == Some(Problem::Duplicate))
        .count();
    assert_eq!(dup, 2);
}

#[test]
fn swapped_names() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::write(p.join("a.txt"), "A").unwrap();
    fs::write(p.join("b.txt"), "B").unwrap();
    let files = files_in(p, &ScanOptions::default());
    let pv = plan(
        &files,
        &[(&p.join("a.txt"), "b.txt"), (&p.join("b.txt"), "a.txt")],
    );
    let hist = tempfile::tempdir().unwrap();
    let h = History::new(hist.path());
    let r = h.apply(&pv.operations());
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(fs::read_to_string(p.join("a.txt")).unwrap(), "B");
    assert_eq!(fs::read_to_string(p.join("b.txt")).unwrap(), "A");
}

#[test]
fn names_windows_refuses() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::write(p.join("a.txt"), "A").unwrap();
    let files = files_in(p, &ScanOptions::default());
    for bad in [
        "CON.txt",
        "nul",
        "COM1.log",
        "name.",
        "name ",
        "a:b.txt",
        "what?.txt",
        "a|b",
    ] {
        let pv = plan(&files, &[(&p.join("a.txt"), bad)]);
        assert_eq!(pv.items[0].problem, Some(Problem::WindowsChars), "{bad}");
    }
    for good in ["CONSOLE.txt", "con-txt", "COM10.txt", "Nome é ação.txt"] {
        let pv = plan(&files, &[(&p.join("a.txt"), good)]);
        assert_eq!(
            pv.items[0].status,
            Status::Ok,
            "{good}: {:?}",
            pv.items[0].problem
        );
    }
}

#[test]
fn long_names_count_as_windows_does() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::write(p.join("a.txt"), "A").unwrap();
    let files = files_in(p, &ScanOptions::default());
    // 200 characters, 600 bytes: Windows takes it.
    let cjk = format!("{}.txt", "名".repeat(200));
    let pv = plan(&files, &[(&p.join("a.txt"), &cjk)]);
    assert_eq!(pv.items[0].status, Status::Ok, "{:?}", pv.items[0].problem);
    let hist = tempfile::tempdir().unwrap();
    let h = History::new(hist.path());
    let r = h.apply(&pv.operations());
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(p.join(&cjk).exists());
    // 256 characters do not fit.
    let files = files_in(p, &ScanOptions::default());
    let long = format!("{}.txt", "a".repeat(252));
    let pv = plan(&files, &[(&p.join(&cjk), &long)]);
    assert_eq!(pv.items[0].problem, Some(Problem::TooLong));
}

#[test]
fn unicode_names() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::write(p.join("a.txt"), "A").unwrap();
    let files = files_in(p, &ScanOptions::default());
    let name = "Ação 日本語 🎵 Ñandú.txt";
    let pv = plan(&files, &[(&p.join("a.txt"), name)]);
    let hist = tempfile::tempdir().unwrap();
    let h = History::new(hist.path());
    let r = h.apply(&pv.operations());
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(names(p), [name]);
}

#[test]
fn deep_paths_past_260_characters() {
    let d = tempfile::tempdir().unwrap();
    let mut deep = d.path().to_path_buf();
    while deep.as_os_str().len() < 300 {
        deep.push("a folder with a fairly long name");
    }
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("song.flac"), "x").unwrap();
    let files = files_in(&deep, &ScanOptions::default());
    assert_eq!(files.len(), 1);
    let pv = plan(&files, &[(&deep.join("song.flac"), "01 - Song.flac")]);
    assert_eq!(pv.items[0].status, Status::Ok, "{:?}", pv.items[0].problem);
    let hist = tempfile::tempdir().unwrap();
    let h = History::new(hist.path());
    let r = h.apply(&pv.operations());
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(deep.join("01 - Song.flac").exists());
    let u = h.undo(r.history_id.as_deref().unwrap()).unwrap();
    assert!(u.errors.is_empty(), "{:?}", u.errors);
    assert!(deep.join("song.flac").exists());
}

#[test]
fn a_file_held_open_fails_alone() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::write(p.join("a.txt"), "A").unwrap();
    fs::write(p.join("b.txt"), "B").unwrap();
    // Open by another program that shares nothing (an editor, a player).
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(p.join("a.txt"))
        .unwrap();
    let files = files_in(p, &ScanOptions::default());
    let pv = plan(
        &files,
        &[(&p.join("a.txt"), "a2.txt"), (&p.join("b.txt"), "b2.txt")],
    );
    let hist = tempfile::tempdir().unwrap();
    let h = History::new(hist.path());
    let r = h.apply(&pv.operations());
    assert_eq!(r.errors.len(), 1, "{:?}", r.errors);
    assert_eq!(r.errors[0].from, p.join("a.txt"));
    assert_eq!(r.done.len(), 1);
    drop(held);
    // Nothing left under a temporary name.
    assert_eq!(names(p), ["a.txt", "b2.txt"]);
}

#[test]
fn read_only_files_are_renamed() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::write(p.join("a.txt"), "A").unwrap();
    let mut perm = fs::metadata(p.join("a.txt")).unwrap().permissions();
    perm.set_readonly(true);
    fs::set_permissions(p.join("a.txt"), perm).unwrap();
    let files = files_in(p, &ScanOptions::default());
    let pv = plan(&files, &[(&p.join("a.txt"), "b.txt")]);
    let hist = tempfile::tempdir().unwrap();
    let h = History::new(hist.path());
    let r = h.apply(&pv.operations());
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(
        fs::metadata(p.join("b.txt"))
            .unwrap()
            .permissions()
            .readonly()
    );
}

#[test]
fn a_folder_with_a_file_held_open() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::create_dir(p.join("Album")).unwrap();
    fs::write(p.join("Album").join("a.flac"), "A").unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(1) // read: a player
        .open(p.join("Album").join("a.flac"))
        .unwrap();
    let opts = ScanOptions {
        folders_as_items: true,
        ..Default::default()
    };
    let files = files_in(&p.join("Album"), &opts);
    let pv = plan(&files, &[(&p.join("Album"), "Album (1999)")]);
    let hist = tempfile::tempdir().unwrap();
    let h = History::new(hist.path());
    let r = h.apply(&pv.operations());
    // Windows refuses; the folder stays as it was, reported.
    assert_eq!(r.errors.len(), 1, "{:?}", r.done);
    drop(held);
    assert_eq!(names(p), ["Album"]);
    assert_eq!(names(&p.join("Album")), ["a.flac"]);
}

#[test]
fn a_junction_loop_ends() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::create_dir(p.join("dir")).unwrap();
    fs::write(p.join("dir").join("a.txt"), "A").unwrap();
    let ok = Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(p.join("dir").join("loop"))
        .arg(p.join("dir"))
        .output()
        .unwrap();
    assert!(ok.status.success(), "{ok:?}");
    let files = files_in(&p.join("dir"), &ScanOptions::default());
    let mut v: Vec<String> = files.iter().map(name).collect();
    v.sort();
    assert_eq!(v, ["a.txt", "loop"]);
}

#[test]
fn lists_with_backslashes_and_drive_letters() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::create_dir(p.join("CD 1")).unwrap();
    fs::write(p.join("CD 1").join("a.flac"), "A").unwrap();
    fs::write(p.join("b.flac"), "B").unwrap();
    let absolute = p.join("b.flac");
    fs::write(
        p.join("list.m3u"),
        format!("#EXTM3U\r\nCD 1\\a.flac\r\n{}\r\n", absolute.display()),
    )
    .unwrap();
    let files = files_in(p, &ScanOptions::default());
    let pv = plan(
        &files,
        &[
            (&p.join("CD 1").join("a.flac"), "01 - A.flac"),
            (&p.join("b.flac"), "02 - B.flac"),
        ],
    );
    let hist = tempfile::tempdir().unwrap();
    let h = History::new(hist.path());
    let r = h.apply(&pv.operations());
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(r.lists, 1);
    assert_eq!(
        fs::read_to_string(p.join("list.m3u")).unwrap(),
        format!(
            "#EXTM3U\r\nCD 1\\01 - A.flac\r\n{}\r\n",
            p.join("02 - B.flac").display()
        )
    );
    let u = h.undo(r.history_id.as_deref().unwrap()).unwrap();
    assert!(u.errors.is_empty(), "{:?}", u.errors);
    assert!(
        fs::read_to_string(p.join("list.m3u"))
            .unwrap()
            .contains("CD 1\\a.flac")
    );
}

#[test]
fn tags_of_a_song_held_open() {
    use renome_core::musicbrainz::SongMatch;
    use renome_core::tagwrite;
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let song = p.join("a.mp3");
    fs::copy("tests/fixtures/song.mp3", &song).unwrap();
    let before = fs::read(&song).unwrap();
    let m = SongMatch {
        title: "New".into(),
        track: 1,
        ..Default::default()
    };
    // A player reading it, without letting it be replaced.
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&song)
        .unwrap();
    let store = p.join("store");
    let r = tagwrite::write(&song, &tagwrite::fields_of(&m), None, false, &store);
    drop(held);
    assert!(r.is_err(), "{r:?}");
    // The song as it was, and no copy left beside it.
    assert_eq!(fs::read(&song).unwrap(), before);
    assert!(!tagwrite::temp_of(&song).exists());
    // Once the player lets go, it works.
    let r = tagwrite::write(&song, &tagwrite::fields_of(&m), None, false, &store);
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(renome_core::music::read(&song).unwrap().title, "New");
}
