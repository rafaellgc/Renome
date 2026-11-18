//! The book rule: e-books are looked up by their details and names, and
//! audiobooks (their audio files grouped into books) by what their tags
//! and folders say, on Open Library and Google Books; a sure first result
//! is taken, the rest are doubts. Then each file's book as the rule names
//! it: the catalogue's details (the edition's only when found by the
//! file's ISBN), one spelling per author across the list, an audiobook's
//! files with their part.

use super::Item;
use crate::Host;
use crate::openlibrary as ol;
use renome_core::books::{BookMatch, Candidate, Guess};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// An audiobook's file: the file its book is looked up by (its first),
/// its place among the book's files and what its tags say.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub leader: PathBuf,
    pub part: u32,
    /// How many files the book has (0: just one).
    pub parts: u32,
    pub narrator: String,
    pub series: String,
    pub series_index: String,
}

/// What was found for a book (an audiobook's under its first file).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub path: PathBuf,
    pub guess: Guess,
    pub candidates: Vec<Candidate>,
    /// Picked with confidence; `None`: a doubt.
    pub choice: Option<Candidate>,
    pub error: Option<String>,
    /// Doubts of one group share one answer (one book in several formats).
    pub group: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// Nothing to look up.
    pub empty: bool,
    /// The audiobooks' files, by path.
    pub parts: HashMap<PathBuf, Part>,
    pub found: Vec<Found>,
}

/// Looks up the books of `items` not `settled` (an audiobook is settled
/// with all its files); with `audiobooks`, the audio files too. An error
/// when nothing could really be searched.
pub async fn search(
    host: &dyn Host,
    client: &ol::Client,
    lang: &str,
    items: &[Item],
    settled: &HashSet<PathBuf>,
    audiobooks: bool,
) -> Result<Outcome, String> {
    let open = |i: &&Item| !i.is_dir && !settled.contains(&i.path);
    let mut wanted: Vec<ol::LookupItem> = items
        .iter()
        .filter(open)
        .filter(|i| renome_core::ebook::is_book(&i.ext()))
        .map(|i| ol::LookupItem {
            path: i.path.clone(),
            name: i.read_stem(),
        })
        .collect();
    let mut out = Outcome::default();
    if audiobooks {
        let audio: Vec<PathBuf> = items
            .iter()
            .filter(open)
            .filter(|i| renome_core::music::is_audio(&i.ext()))
            .map(|i| i.path.clone())
            .collect();
        if !audio.is_empty() {
            let groups = tokio::task::spawn_blocking(move || renome_core::audiobook::group(&audio))
                .await
                .unwrap_or_default();
            for b in groups {
                let leader = b.paths[0].clone();
                let count = b.paths.len() as u32;
                for (n, p) in b.paths.iter().enumerate() {
                    out.parts.insert(
                        p.clone(),
                        Part {
                            leader: leader.clone(),
                            part: n as u32 + 1,
                            parts: if count > 1 { count } else { 0 },
                            narrator: b.narrator.clone(),
                            series: b.series.clone(),
                            series_index: b.series_index.clone(),
                        },
                    );
                }
                if !b.title.is_empty() || !b.author.is_empty() {
                    let author = if b.author.is_empty() {
                        String::new()
                    } else {
                        format!("{} - ", b.author)
                    };
                    let year = b.year.map(|y| format!(" ({y})")).unwrap_or_default();
                    wanted.push(ol::LookupItem {
                        path: leader,
                        name: format!("{author}{}{year}", b.title),
                    });
                }
            }
        }
    }
    if wanted.is_empty() {
        out.empty = true;
        return Ok(out);
    }
    let found = ol::lookup(host, client, lang, wanted).await;
    if let Some(failed) = found.iter().find_map(|f| f.error.clone())
        && found
            .iter()
            .all(|f| f.error.is_some() || (f.guess.title.is_empty() && f.guess.isbns.is_empty()))
    {
        return Err(failed);
    }
    out.found = found
        .into_iter()
        .map(|f| Found {
            group: format!("{}\u{1f}{}", f.guess.title, f.guess.author),
            choice: if f.auto {
                f.candidates.first().cloned()
            } else {
                None
            },
            path: f.path,
            guess: f.guess,
            candidates: f.candidates,
            error: f.error,
        })
        .collect();
    Ok(out)
}

/// What the rule uses of a chosen book: the edition's publisher and ISBN
/// only when it was found by the file's ISBN (otherwise they would be
/// another edition's), else the file's own.
pub fn book_match(g: &Guess, c: &Candidate) -> BookMatch {
    let mut extra = renome_core::fields::Extra::new();
    let language = if c.exact || g.language.is_empty() {
        c.language.clone()
    } else {
        g.language.clone()
    };
    if !language.is_empty() {
        extra.insert("language".into(), language);
    }
    if let Some(p) = c.pages.filter(|p| *p > 0) {
        extra.insert("pages".into(), p.to_string());
    }
    BookMatch {
        work: c.work.clone(),
        title: c.title.clone(),
        original_title: c.original_title.clone(),
        authors: c.authors.clone(),
        year: c.year,
        edition_year: if c.exact { c.edition_year } else { None },
        publisher: if c.exact {
            c.publisher.clone()
        } else {
            g.publisher.clone()
        },
        isbn: if c.exact {
            c.isbn.clone()
        } else {
            g.isbns.first().cloned().unwrap_or_default()
        },
        series: g.series.clone(),
        series_index: g.series_index.clone(),
        extra,
        ..Default::default()
    }
}

/// A book chosen for a file (an audiobook's first file).
#[derive(Clone, Debug, Deserialize)]
pub struct Chosen {
    pub path: PathBuf,
    pub guess: Guess,
    pub choice: Candidate,
}

/// Each file's book as the rule names it: one spelling per author across
/// the list; an audiobook's files take its first file's book, with their
/// part (with `audiobooks` only).
pub fn matches(
    chosen: &[Chosen],
    parts: &HashMap<PathBuf, Part>,
    audiobooks: bool,
) -> HashMap<PathBuf, BookMatch> {
    let is_audio = |p: &PathBuf| {
        p.extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .is_some_and(|e| renome_core::music::is_audio(&e))
    };
    let raw: Vec<(PathBuf, BookMatch)> = chosen
        .iter()
        .filter(|c| audiobooks || !is_audio(&c.path))
        .map(|c| (c.path.clone(), book_match(&c.guess, &c.choice)))
        .collect();
    let names: Vec<String> = raw.iter().flat_map(|(_, m)| m.authors.clone()).collect();
    let spelling = renome_core::books::author_spellings(&names);
    let mut out = HashMap::new();
    for (p, mut m) in raw {
        m.title = renome_core::text::nfc(&m.title);
        let mut authors: Vec<String> = Vec::new();
        for a in &m.authors {
            let a = renome_core::text::nfc(a);
            let a = spelling.get(&a).cloned().unwrap_or(a);
            if !authors.contains(&a) {
                authors.push(a);
            }
        }
        m.authors = authors;
        out.insert(p, m);
    }
    if audiobooks {
        for (p, g) in parts {
            if let Some(m) = out.get(&g.leader).cloned() {
                let mut m = m;
                m.narrator = g.narrator.clone();
                m.part = g.part;
                m.parts = g.parts;
                if m.series.is_empty() {
                    m.series = g.series.clone();
                }
                if m.series_index.is_empty() {
                    m.series_index = g.series_index.clone();
                }
                out.insert(p.clone(), m);
            }
        }
    }
    out
}
