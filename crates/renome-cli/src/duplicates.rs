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

/// Files to the Trash: no search. Asks first unless told not to.
fn trash(paths: Vec<PathBuf>, yes: bool) -> Result<Exit, String> {
    if !yes {
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
    Ok(if failed == 0 {
        Exit::Done
    } else {
        Exit::Failed
    })
}

/// A progress line, when a person is watching.
fn progress(what: &'static str) -> impl FnMut(u64, u64) {
    move |done: u64, total: u64| {
        if out::live() && total > 0 {
            eprint!("\r{}   ", t(what, &[("p", &(done * 100 / total))]));
            if done == total {
                eprintln!();
            }
        }
    }
}

/// The groups found.
struct Groups {
    identical: Vec<Vec<PathBuf>>,
    songs: Vec<Vec<PathBuf>>,
    works: Vec<renome_core::repeated::Group>,
    similar: Vec<Vec<PathBuf>>,
}

/// Looks for the kinds of copies asked for among `files`.
fn find_groups(files: &[FileEntry], want_repeated: bool, want_similar: bool) -> Groups {
    let mut cache = renome_core::HashCache::default();
    let mut p = progress("identicalScanning");
    let identical = renome_core::find_identical(files, &mut cache, &mut p, &crate::stop::requested)
        .unwrap_or_default();
    let by_group = renome_core::repeated::by_group(&identical);
    // The same song or work in other files (as the window finds them).
    let (songs, works) = if want_repeated {
        renome_core::repeated::repeated(files, &by_group, &crate::stop::requested)
            .unwrap_or_default()
    } else {
        Default::default()
    };
    let similar = if want_similar {
        let mut pictures = renome_core::similar::PictureCache::in_file(
            renome_services::cache_root().join("pictures"),
        );
        let mut p = progress("similarScanning");
        let groups = renome_core::similar::similar_pictures(
            files,
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
    Groups {
        identical,
        songs,
        works,
        similar,
    }
}

fn print_json(g: Groups, want_identical: bool) {
    let works: Vec<Value> = g
        .works
        .iter()
        .map(|g| json!({ "kind": g.kind, "best": g.best, "files": g.items.iter().map(|i| &i.path).collect::<Vec<_>>() }))
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "identical": if want_identical { g.identical } else { vec![] },
            "songs": g.songs,
            "works": works,
            "similar": g.similar,
        }))
        .unwrap_or_default()
    );
}

/// Prints a kind of group: each with its files (and sizes), the best
/// marked.
fn print_groups(
    title: String,
    groups: Vec<(Vec<PathBuf>, Option<PathBuf>)>,
    size_of: &HashMap<&PathBuf, u64>,
) {
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
}

fn print_text(g: Groups, files: &[FileEntry], want_identical: bool) {
    let size_of: HashMap<&PathBuf, u64> = files.iter().map(|f| (&f.path, f.size)).collect();
    if want_identical {
        print_groups(
            t(
                "identicalCount",
                &[("n", &g.identical.iter().map(Vec::len).sum::<usize>())],
            ),
            g.identical.into_iter().map(|g| (g, None)).collect(),
            &size_of,
        );
    }
    print_groups(
        t(
            "alikeCount",
            &[("n", &g.songs.iter().map(Vec::len).sum::<usize>())],
        ),
        g.songs.into_iter().map(|g| (g, None)).collect(),
        &size_of,
    );
    print_groups(
        t(
            "repeatedCount",
            &[("n", &g.works.iter().map(|g| g.items.len()).sum::<usize>())],
        ),
        g.works
            .into_iter()
            .map(|g| (g.items.into_iter().map(|i| i.path).collect(), g.best))
            .collect(),
        &size_of,
    );
    print_groups(
        t(
            "similarCount",
            &[("n", &g.similar.iter().map(Vec::len).sum::<usize>())],
        ),
        g.similar
            .into_iter()
            .map(|g| {
                // The one worth keeping, when it stands out (as the window marks it).
                let worth: Vec<renome_core::similar::Worth> = g
                    .iter()
                    .filter_map(|p| files.iter().find(|f| &f.path == p))
                    .map(renome_core::similar::Worth::read)
                    .collect();
                let best = renome_core::similar::picks(&worth).largest;
                (g, best)
            })
            .collect(),
        &size_of,
    );
}

pub fn command(m: &ArgMatches, settings: Option<&Value>) -> Result<Exit, String> {
    if let Some(paths) = m.get_many::<PathBuf>("trash") {
        let paths: Vec<PathBuf> = paths
            .map(|p| std::path::absolute(p).unwrap_or_else(|_| p.clone()))
            .collect();
        return trash(paths, m.get_flag("yes"));
    }
    let paths: Vec<PathBuf> = m
        .get_many::<PathBuf>("paths")
        .expect("required by the argument parser")
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
    let (i, r, s) = (
        m.get_flag("identical"),
        m.get_flag("repeated"),
        m.get_flag("similar"),
    );
    let (want_identical, want_repeated, want_similar) = if !i && !r && !s {
        (true, true, false)
    } else {
        (i, r, s)
    };
    let groups = find_groups(&files, want_repeated, want_similar);
    if out::json() {
        print_json(groups, want_identical);
    } else {
        print_text(groups, &files, want_identical);
    }
    Ok(Exit::Done)
}
