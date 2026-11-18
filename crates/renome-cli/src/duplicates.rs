//! `duplicates`: the groups the window's footer filters show, identical
//! copies, the same song or work in several files (best marked), pictures
//! that look alike; files sent to the Trash on request.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::PathBuf;

use clap::ArgMatches;
use renome_core::{FileEntry, ScanOptions};
use serde_json::{Value, json};

use crate::Exit;
use crate::out;
use crate::text::t;

pub fn command(m: &ArgMatches, settings: Option<&Value>) -> Result<Exit, String> {
    // Files to the Trash: no search.
    if let Some(paths) = m.get_many::<PathBuf>("trash") {
        let paths: Vec<PathBuf> = paths
            .map(|p| std::path::absolute(p).unwrap_or_else(|_| p.clone()))
            .collect();
        if !m.get_flag("yes") {
            if !std::io::stdin().is_terminal() {
                return Err(t("needYes", &[]));
            }
            for p in &paths {
                eprintln!("  {}", out::path(p));
            }
            if !crate::run::ask(&format!(
                "{} [s/N] ",
                t("compareTrashTitle", &[("n", &paths.len())])
            )) {
                return Ok(Exit::Done);
            }
        }
        let mut failed = 0;
        for p in &paths {
            if let Err(e) = renome_core::files::to_trash(p) {
                out::error(&format!("{}: {e}", out::path(p)));
                failed += 1;
            }
        }
        out::note(&t("compareTrashedN", &[("n", &(paths.len() - failed))]));
        return Ok(if failed == 0 {
            Exit::Done
        } else {
            Exit::Failed
        });
    }
    let paths: Vec<PathBuf> = m
        .get_many::<PathBuf>("paths")
        .unwrap()
        .map(|p| std::path::absolute(p).unwrap_or_else(|_| p.clone()))
        .collect();
    let opts = ScanOptions {
        recursive: !m.get_flag("no-recursive"),
        include_hidden: m.get_flag("hidden")
            || settings
                .and_then(|s| s.get("includeHidden")?.as_bool())
                .unwrap_or(false),
        folders_as_items: false,
    };
    let files: Vec<FileEntry> = renome_core::scan(&paths, &opts, &Default::default()).files;
    let (want_identical, want_repeated, want_similar) = {
        let (i, r, s) = (
            m.get_flag("identical"),
            m.get_flag("repeated"),
            m.get_flag("similar"),
        );
        if !i && !r && !s {
            (true, true, false)
        } else {
            (i, r, s)
        }
    };
    let progress = |what: &'static str| {
        move |done: u64, total: u64| {
            if out::live() && total > 0 {
                eprint!("\r{}   ", t(what, &[("p", &(done * 100 / total))]));
                if done == total {
                    eprintln!();
                }
            }
        }
    };
    let mut cache = renome_core::HashCache::default();
    let mut p = progress("identicalScanning");
    let identical =
        renome_core::find_identical(&files, &mut cache, &mut p, &crate::stop::requested)
            .unwrap_or_default();
    let by_group: HashMap<PathBuf, usize> = identical
        .iter()
        .enumerate()
        .flat_map(|(i, g)| g.iter().map(move |p| (p.clone(), i)))
        .collect();
    let songs = if want_repeated {
        renome_core::alike::same_songs(&files, &by_group, &crate::stop::requested)
            .unwrap_or_default()
    } else {
        vec![]
    };
    let works = if want_repeated {
        renome_core::repeated::same_works(&files, &by_group, &crate::stop::requested)
            .unwrap_or_default()
    } else {
        vec![]
    };
    let similar = if want_similar {
        let mut pictures = renome_core::similar::PictureCache::in_file(
            renome_services::cache_root().join("pictures"),
        );
        let mut p = progress("similarScanning");
        let groups = renome_core::similar::similar_pictures(
            &files,
            &by_group,
            &mut pictures,
            &mut p,
            &crate::stop::requested,
        )
        .unwrap_or_default();
        pictures.save();
        groups
    } else {
        vec![]
    };
    if out::json() {
        let works: Vec<Value> = works
            .iter()
            .map(|g| json!({ "kind": g.kind, "best": g.best, "files": g.items.iter().map(|i| &i.path).collect::<Vec<_>>() }))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "identical": if want_identical { identical.clone() } else { vec![] },
                "songs": songs,
                "works": works,
                "similar": similar,
            }))
            .unwrap_or_default()
        );
        return Ok(Exit::Done);
    }
    let size_of: HashMap<&PathBuf, u64> = files.iter().map(|f| (&f.path, f.size)).collect();
    let show = |title: String, groups: Vec<(Vec<PathBuf>, Option<PathBuf>)>| {
        if groups.is_empty() {
            return;
        }
        println!("{}", out::bold(&title));
        for (n, (g, best)) in groups.iter().enumerate() {
            println!("  {}", out::dim(&format!("{}", n + 1)));
            for p in g {
                let mark = if best.as_ref() == Some(p) {
                    out::green(" ★")
                } else {
                    String::new()
                };
                let size = size_of
                    .get(p)
                    .map(|s| out::dim(&format!(" ({})", out::size(*s))))
                    .unwrap_or_default();
                println!("    {}{size}{mark}", out::path(p));
            }
        }
    };
    if want_identical {
        show(
            t(
                "identicalCount",
                &[("n", &identical.iter().map(Vec::len).sum::<usize>())],
            ),
            identical.into_iter().map(|g| (g, None)).collect(),
        );
    }
    show(
        t(
            "alikeCount",
            &[("n", &songs.iter().map(Vec::len).sum::<usize>())],
        ),
        songs.into_iter().map(|g| (g, None)).collect(),
    );
    show(
        t(
            "repeatedCount",
            &[("n", &works.iter().map(|g| g.items.len()).sum::<usize>())],
        ),
        works
            .into_iter()
            .map(|g| (g.items.into_iter().map(|i| i.path).collect(), g.best))
            .collect(),
    );
    show(
        t(
            "similarCount",
            &[("n", &similar.iter().map(Vec::len).sum::<usize>())],
        ),
        similar.into_iter().map(|g| (g, None)).collect(),
    );
    Ok(Exit::Done)
}
