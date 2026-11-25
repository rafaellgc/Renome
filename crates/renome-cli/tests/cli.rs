#![allow(clippy::unwrap_used)] // a test that cannot set itself up should fail

//! renome-cli run as people and scripts run it, each test in a profile
//! of its own (`RENOME_DIR`), offline.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

struct Place {
    root: PathBuf,
}

impl Place {
    fn new(name: &str) -> Place {
        let root =
            std::env::temp_dir().join(format!("renome-cli-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("files")).unwrap();
        Place { root }
    }

    fn files(&self) -> PathBuf {
        self.root.join("files")
    }

    fn touch(&self, name: &str, content: &[u8]) -> PathBuf {
        let p = self.files().join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, content).unwrap();
        p
    }

    fn names(&self) -> Vec<String> {
        let mut n: Vec<String> = std::fs::read_dir(self.files())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        n.sort();
        n
    }

    /// Exit code, stdout, stderr.
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_renome-cli"))
            .arg("--lang")
            .arg("en")
            .args(args)
            .env("RENOME_DIR", self.root.join("profile"))
            .env("RENOME_SECRETS_SERVICE", "renome.clitest")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    fn json(&self, args: &[&str]) -> Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        let (code, out, err) = self.run(&all);
        assert!(code == 0 || code == 2, "exit {code}: {err}");
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"))
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn preview_changes_nothing_and_apply_renames_and_undo_puts_back() {
    let w = Place::new("apply");
    w.touch("a_b.txt", b"1");
    w.touch("c_d.txt", b"2");
    let rule = "replace:find=_,replace= ";
    let p = w.json(&["preview", s(&w.files()), "-r", rule]);
    assert_eq!(p["totals"]["changed"], 2);
    assert!(
        p["items"][0]["newPath"]
            .as_str()
            .unwrap()
            .ends_with("a b.txt")
    );
    assert_eq!(w.names(), ["a_b.txt", "c_d.txt"]);

    // No terminal: it must be told to go on.
    let (code, _, _) = w.run(&["apply", s(&w.files()), "-r", rule]);
    assert_eq!(code, 3);
    assert_eq!(w.names(), ["a_b.txt", "c_d.txt"]);

    let a = w.json(&["apply", s(&w.files()), "-r", rule, "--yes"]);
    assert_eq!(a["renamed"], 2);
    assert_eq!(w.names(), ["a b.txt", "c d.txt"]);
    let h = w.json(&["history"]);
    assert_eq!(h[0]["id"], a["historyId"]);

    let u = w.json(&["undo"]);
    assert_eq!(u["restored"], 2);
    assert_eq!(w.names(), ["a_b.txt", "c_d.txt"]);
}

#[test]
fn rules_run_in_order_and_take_names_by_hand() {
    let w = Place::new("order");
    let a = w.touch("one.txt", b"");
    w.touch("two.txt", b"");
    let p = w.json(&[
        "preview",
        s(&w.files()),
        "-r",
        "case:mode=upper",
        "-r",
        "number:template={n}-{name},start=7",
        "--name",
        &format!("{}=kept.txt", s(&a)),
    ]);
    let new: Vec<String> = p["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            Path::new(i["newPath"].as_str().unwrap())
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(new.contains(&"kept.txt".to_string()), "{new:?}");
    // Upper case first, then numbered (the hand-named file keeps its place).
    assert!(new.contains(&"08-TWO.txt".to_string()), "{new:?}");
}

#[test]
fn wrong_rules_and_options_are_usage_errors() {
    let w = Place::new("wrong");
    w.touch("x.txt", b"");
    assert_eq!(w.run(&["preview", s(&w.files()), "-r", "nosuchrule"]).0, 3);
    assert_eq!(
        w.run(&["preview", s(&w.files()), "-r", "replace:nosuch=1"])
            .0,
        3
    );
    assert_eq!(
        w.run(&[
            "preview",
            s(&w.files()),
            "-r",
            "case",
            "--edit",
            "x.txt:title=A"
        ])
        .0,
        3
    );
    assert_eq!(w.run(&["manual", "nosuchchapter"]).0, 3);
}

#[test]
fn sets_are_saved_shown_used_and_deleted() {
    let w = Place::new("sets");
    w.touch("a_b.txt", b"");
    assert_eq!(
        w.run(&["sets", "save", "Mine", "-r", "replace:find=_,replace=-"])
            .0,
        0
    );
    let list = w.json(&["sets", "list"]);
    assert!(list.to_string().contains("Mine"), "{list}");
    let p = w.json(&["preview", s(&w.files()), "--set", "Mine"]);
    assert!(
        p["items"][0]["newPath"]
            .as_str()
            .unwrap()
            .ends_with("a-b.txt")
    );
    assert_eq!(w.run(&["sets", "delete", "Mine"]).0, 0);
    assert_eq!(w.run(&["preview", s(&w.files()), "--set", "Mine"]).0, 3);
}

#[test]
fn options_are_read_and_changed() {
    let w = Place::new("options");
    assert_eq!(w.run(&["options", "set", "numberClashes", "true"]).0, 0);
    let (_, out, _) = w.run(&["options", "get", "numberClashes"]);
    assert!(out.contains("true"), "{out}");
    assert_eq!(w.run(&["options", "set", "numberClashes", "maybe"]).0, 3);
    assert_eq!(w.run(&["options", "reset", "numberClashes"]).0, 0);
    let (_, out, _) = w.run(&["options", "get", "numberClashes"]);
    assert!(out.contains("false"), "{out}");
}

#[test]
fn photo_dates_typed_by_hand_name_the_file() {
    let w = Place::new("photo");
    let a = w.touch("IMG_1.jpg", b"not a real picture");
    let p = w.json(&[
        "preview",
        s(&w.files()),
        "-r",
        "photo",
        "--edit",
        &format!("{}:date=2018-03-27 14:30", s(&a)),
    ]);
    let new = p["items"][0]["newPath"].as_str().unwrap();
    assert!(new.contains("2018-03-27"), "{new}");
    assert_eq!(
        w.run(&[
            "preview",
            s(&w.files()),
            "-r",
            "photo",
            "--edit",
            &format!("{}:date=2018-02-30", s(&a))
        ])
        .0,
        3
    );
}

#[test]
fn identical_copies_are_found() {
    let w = Place::new("dups");
    w.touch("a.bin", b"same content here");
    w.touch("b.bin", b"same content here");
    w.touch("c.bin", b"other");
    let d = w.json(&["duplicates", s(&w.files())]);
    let groups = d["identical"].as_array().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].as_array().unwrap().len(), 2);
}

#[test]
fn duplicates_are_listed_as_text() {
    let w = Place::new("dups-text");
    w.touch("a.bin", b"same content here");
    w.touch("b.bin", b"same content here");
    w.touch("c.bin", b"other");
    let (code, out, _) = w.run(&["duplicates", s(&w.files())]);
    assert_eq!(code, 0);
    let out = out.replace(w.files().to_str().unwrap(), "<dir>");
    assert_eq!(
        out,
        "2 files with an identical copy\n  1\n    <dir>/a.bin (17 B)\n    <dir>/b.bin (17 B)\n"
    );
}

#[test]
fn the_manual_and_help_are_there_in_each_language() {
    let w = Place::new("manual");
    let (code, out, _) = w.run(&["manual", "cli"]);
    assert_eq!(code, 0);
    assert!(out.contains("renome-cli"), "{out}");
    for lang in ["pt", "en", "es"] {
        let out = Command::new(env!("CARGO_BIN_EXE_renome-cli"))
            .args(["--lang", lang, "preview", "--help"])
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("--edit"));
    }
}
