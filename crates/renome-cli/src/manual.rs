//! `manual [TOPIC]`: the window's manual as text, in the language chosen.

use std::io::IsTerminal;

use clap::ArgMatches;

use crate::Exit;
use crate::out;
use crate::text::t;

mod pages {
    include!(concat!(env!("OUT_DIR"), "/manual.rs"));
}

type Page = (&'static str, &'static str, bool, &'static str);

fn pages() -> &'static [Page] {
    match crate::text::lang() {
        "en" => pages::MANUAL_EN,
        "es" => pages::MANUAL_ES,
        _ => pages::MANUAL_PT,
    }
}

/// The pages in the window's order: the rules' (`r-<type>`) in the
/// order of its "+ Rule" menu (those by kind of file, then those on the
/// name's text, each by the rule's name in the language), each group's
/// first with the group's title; the others where they are.
fn ordered() -> Vec<(&'static Page, Option<String>)> {
    let all = pages();
    let rule_of = |p: &Page| {
        p.0.strip_prefix("r-")
            .filter(|k| renome_core::catalog::rule(k).is_some())
    };
    let (media, text) = renome_core::catalog::groups();
    let sorted = |group: &[&str], title: &str| {
        let mut list: Vec<&'static Page> = all
            .iter()
            .filter(|p| rule_of(p).is_some_and(|k| group.contains(&k)))
            .collect();
        list.sort_by_cached_key(|p| plain(&t(&format!("rule.{}", rule_of(p).unwrap()), &[])));
        list.into_iter()
            .enumerate()
            .map(|(i, p)| (p, (i == 0).then(|| t(title, &[]))))
            .collect::<Vec<_>>()
    };
    let mut rules = sorted(&media, "ruleGroupMedia");
    rules.extend(sorted(&text, "ruleGroupText"));
    let mut rules = rules.into_iter();
    all.iter()
        .map(|p| {
            if rule_of(p).is_some() {
                rules.next().unwrap()
            } else {
                (p, None)
            }
        })
        .collect()
}

use renome_core::text::fold as plain;

/// The page a topic names: its id (`r-movie` or `movie`), or the start of
/// its title, accents and case aside.
fn find(topic: &str) -> Option<&'static Page> {
    let q = plain(topic.trim());
    let all = pages();
    all.iter()
        .find(|p| p.0 == q || p.0.strip_prefix("r-") == Some(q.as_str()))
        .or_else(|| all.iter().find(|p| plain(p.1) == q))
        .or_else(|| all.iter().find(|p| plain(p.1).starts_with(&q)))
        .or_else(|| all.iter().find(|p| plain(p.1).contains(&q)))
}

fn width() -> usize {
    if !std::io::stdout().is_terminal() {
        return 80;
    }
    terminal_size::terminal_size().map_or(80, |(w, _)| (w.0 as usize).clamp(40, 100))
}

/// A line broken at the width, continued under its text (after a bullet,
/// a number or the indent).
fn wrap(line: &str, width: usize) -> Vec<String> {
    let lead = line.len() - line.trim_start().len();
    let marker = line.trim_start().split_once(' ').map_or(0, |(m, _)| {
        let bullet = m == "•"
            || m == "#"
            || (m.ends_with('.')
                && m[..m.len() - 1].chars().all(|c| c.is_ascii_digit())
                && m.len() > 1);
        if bullet { m.chars().count() + 1 } else { 0 }
    });
    let hang = " ".repeat(lead + marker);
    let mut out = Vec::new();
    let mut now = " ".repeat(lead);
    let mut fresh = true;
    for word in line.trim_start().split(' ') {
        if !fresh && now.chars().count() + 1 + word.chars().count() > width {
            out.push(std::mem::take(&mut now));
            now.push_str(&hang);
        } else if !fresh {
            now.push(' ');
        }
        now.push_str(word);
        fresh = false;
    }
    out.push(now);
    out
}

fn show(page: &Page) {
    let w = width();
    println!("{}", out::bold(page.1));
    println!();
    for line in page.3.lines() {
        if line.is_empty() {
            println!();
            continue;
        }
        let heading = line.starts_with("# ");
        for part in wrap(line.strip_prefix("# ").unwrap_or(line), w) {
            if heading {
                println!("{}", out::bold(&part));
            } else {
                println!("{part}");
            }
        }
    }
}

pub fn command(m: &ArgMatches) -> Result<Exit, String> {
    if m.get_flag("all") {
        for (n, (page, _)) in ordered().into_iter().enumerate() {
            if n > 0 {
                println!("\n");
            }
            show(page);
        }
        return Ok(Exit::Done);
    }
    let Some(topic) = m.get_one::<String>("topic") else {
        if out::json() {
            let list: Vec<_> = ordered()
                .into_iter()
                .map(|(p, group)| serde_json::json!({ "id": p.0, "title": p.1, "group": group }))
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&list).unwrap_or_default()
            );
            return Ok(Exit::Done);
        }
        for (p, group) in ordered() {
            if let Some(group) = group {
                println!("  {}", out::dim(&group));
            }
            let indent = if p.2 { "    " } else { "  " };
            println!("{indent}{:<14} {}", out::bold(p.0), p.1);
        }
        out::note(&format!("\n{}", t("manualHow", &[])));
        return Ok(Exit::Done);
    };
    let page = find(topic).ok_or_else(|| t("noManualTopic", &[("topic", topic)]))?;
    if out::json() {
        println!(
            "{}",
            serde_json::json!({ "id": page.0, "title": page.1, "text": page.3 })
        );
    } else {
        show(page);
    }
    Ok(Exit::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_the_same_pages() {
        let ids = |p: &[Page]| p.iter().map(|p| p.0).collect::<Vec<_>>();
        assert_eq!(ids(pages::MANUAL_PT), ids(pages::MANUAL_EN));
        assert_eq!(ids(pages::MANUAL_PT), ids(pages::MANUAL_ES));
        assert!(
            pages::MANUAL_EN
                .iter()
                .all(|p| !p.3.contains('<') || p.3.contains("&lt;") || !p.3.contains("</"))
        );
    }

    #[test]
    fn long_lines_wrap_under_their_text() {
        let lines = wrap("• one two three four five six", 12);
        assert_eq!(lines, ["• one two", "  three four", "  five six"]);
    }
}
