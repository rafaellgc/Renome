//! Books from Open Library, without the network part: guessing title and
//! author from a book's details and file name, the searches to make,
//! ranking the results, building the new name and the `metadata.opf` file
//! that Audiobookshelf and e-book managers read.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::ebook::{BookMeta, find_isbns, isbn13};
use crate::text;
use crate::tmdb::{BRACKETS, safe_title, similarity, tag, xml};

/// What a book is thought to be, from its details or its name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Guess {
    pub title: String,
    pub author: String,
    pub year: Option<u16>,
    /// ISBN-13s, best first.
    pub isbns: Vec<String>,
    /// Title and author came from the file's details, not its name.
    pub from_file: bool,
    pub series: String,
    pub series_index: String,
    /// From the file's details.
    pub publisher: String,
    pub language: String,
    /// Other ways to read the name (`Title - Author` besides `Author -
    /// Title`), as (title, author).
    pub alts: Vec<(String, String)>,
}

static YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(1[5-9][0-9]{2}|20[0-9]{2})\b").unwrap());
/// Words in book file names that are not part of title or author.
static JUNK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(z-?lib(rary)?(\.org)?|libgen(\.\w+)?|anna'?s archive|1lib|b-ok|epub|pdf|mobi|azw3?|fb2|djvu|e-?book|retail|ocr|scan(ned)?|digital|v\d+(\.\d+)*)\b",
    )
    .unwrap()
});
static SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+[-–—]+\s+|\s*--\s*").unwrap());
static BY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s+(?:by|por)\s+").unwrap());

/// Title, author, year and ISBN from a file name without extension:
/// `Tomé de Alvarenga - Vento Leste (1899)`, `Vento Leste - Tomé de
/// Alvarenga`, `Ecos by Lia Marta Quaresma`, `9780000000019.pdf`.
///
/// `Author - Title` is assumed; the other order goes in `alts`, and the
/// ranking finds out which one fits.
pub fn guess_name(stem: &str) -> Guess {
    let mut g = Guess {
        isbns: find_isbns(stem),
        ..Default::default()
    };
    // `_ ` is where a `:` was (`Ecos_ Uma breve história`).
    let mut s = stem.replace("_ ", ": ").replace('_', " ");
    if !s.contains(' ') {
        s = s.replace('.', " ");
    }
    // Bare ISBNs (no word before them) count too.
    for word in s.split(|c: char| c.is_whitespace() || "()[]{}".contains(c)) {
        if let Some(i) = isbn13(word)
            && word.chars().filter(|c| c.is_ascii_digit()).count() >= 9
            && !g.isbns.contains(&i)
        {
            g.isbns.push(i);
        }
    }
    for i in &g.isbns {
        // Remove the ISBN however it was written: its digits in a row.
        let pattern: String = i[3..12]
            .chars()
            .map(|c| format!("{c}[^0-9]{{0,2}}"))
            .collect();
        if let Ok(re) = Regex::new(&format!(r"(?:97[89][^0-9]{{0,2}})?{pattern}[0-9Xx]")) {
            s = re.replace_all(&s, " ").into_owned();
        }
    }
    static ISBN_WORD: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\bisbn(-?1[03])?\b:?").unwrap());
    s = ISBN_WORD.replace_all(&s, " ").into_owned();
    // A year in brackets is the book's (`(1899)`, `(2019, Vela)`).
    g.year = BRACKETS
        .find_iter(&s)
        .find_map(|m| YEAR.captures(m.as_str())?[1].parse().ok());
    let s = BRACKETS.replace_all(&s, " ");
    let s = JUNK.replace_all(&s, " ");
    let mut parts: Vec<String> = SPLIT
        .split(&s)
        .map(clean)
        .filter(|p| !p.is_empty())
        .collect();
    if let [one] = parts.as_slice()
        && let Some(by) = BY.find(one)
    {
        let (title, author) = (clean(&one[..by.start()]), clean(&one[by.end()..]));
        if !title.is_empty() && !author.is_empty() && author.split_whitespace().count() <= 4 {
            // `Title by Author`; also tried as one title.
            g.alts.push((one.clone(), String::new()));
            g.title = title;
            g.author = author;
            return g;
        }
    }
    // With three parts or more, a year or a small number on its own is the
    // year or the book's place in a series (`Author - 01 - Title`); with
    // two, it may be the title (`Rui Valadares - 2077`).
    if parts.len() >= 3
        && let Some(i) = parts.iter().position(|p| YEAR.is_match(p) && p.len() == 4)
    {
        g.year = g.year.or(parts[i].parse().ok());
        parts.remove(i);
    }
    if parts.len() >= 3
        && let Some(i) = parts
            .iter()
            .position(|p| p.len() <= 5 && p.chars().all(|c| c.is_ascii_digit() || c == '.'))
    {
        g.series_index = parts.remove(i);
    }
    match parts.as_slice() {
        [] => {}
        [title] => g.title = title.clone(),
        [author, title] => {
            g.title = title.clone();
            g.author = author.clone();
            g.alts.push((author.clone(), title.clone()));
        }
        [first, .., last] => {
            g.title = parts[1..].join(" - ");
            g.author = first.clone();
            g.alts.push((last.clone(), first.clone()));
            g.alts.push((first.clone(), last.clone()));
            g.alts.push((last.clone(), String::new()));
        }
    }
    g
}

/// Drops stray separators and spaces.
fn clean(s: &str) -> String {
    text::collapse_spaces(s)
        .trim_matches(|c: char| c.is_whitespace() || "-–—:,;.([{".contains(c))
        .to_string()
}

/// The guess for a book: its file's details, with the name for what they
/// lack.
pub fn guess(stem: &str, meta: &BookMeta) -> Guess {
    let named = guess_name(stem);
    if meta.title.is_empty() {
        let mut g = named;
        for i in &meta.isbns {
            if !g.isbns.contains(i) {
                g.isbns.push(i.clone());
            }
        }
        if g.author.is_empty()
            && let Some(a) = meta.authors.first()
        {
            g.author = a.clone();
        }
        g.series = meta.series.clone();
        g.publisher = meta.publisher.clone();
        g.language = meta.language.clone();
        if !meta.series_index.is_empty() {
            g.series_index = meta.series_index.clone();
        }
        return g;
    }
    let mut isbns = meta.isbns.clone();
    for i in named.isbns {
        if !isbns.contains(&i) {
            isbns.push(i);
        }
    }
    let author = text::nfc(meta.authors.first().map_or("", |a| a.trim()));
    let mut alts = Vec::new();
    if author.is_empty() {
        // The name may tell who wrote it.
        alts.push((meta.title.clone(), named.author.clone()));
        alts.extend(
            named
                .alts
                .iter()
                .map(|(_, a)| (meta.title.clone(), a.clone())),
        );
    }
    // The name may also be better than the details.
    alts.push((named.title.clone(), named.author.clone()));
    alts.extend(named.alts);
    alts.retain(|(t, _)| !t.is_empty());
    Guess {
        title: text::nfc(&meta.title),
        author,
        year: meta.year.or(named.year),
        isbns,
        from_file: true,
        series: meta.series.clone(),
        series_index: if meta.series_index.is_empty() {
            named.series_index
        } else {
            meta.series_index.clone()
        },
        publisher: meta.publisher.clone(),
        language: meta.language.clone(),
        alts,
    }
}

/// A search on Open Library.
#[derive(Debug, Clone, PartialEq)]
pub enum Query {
    Isbn(String),
    /// By title and author (`title:(…) author:(…)`): every book found has
    /// an edition with that title.
    Titled(String),
    /// Plain words, found in titles, authors or anywhere.
    Text(String),
    /// The author's best-known books, when no title finds the book: Open
    /// Library may have only the original, under another title.
    Author(String),
}

/// Only letters, digits and spaces: Open Library's search treats much of
/// the rest as syntax.
fn words_only(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '\'' {
                c
            } else {
                ' '
            }
        })
        .collect();
    text::collapse_spaces(&s).trim().to_string()
}

/// The title before its subtitle (`Ecos: Uma breve história` → `Ecos`).
fn main_title(t: &str) -> &str {
    t.split([':', '(']).next().unwrap_or(t).trim()
}

/// The author's surname, as the search finds it: initials (`J R R
/// Valença`) and first names an edition may not spell (`Lúcio Aneu
/// Tércio`) make it miss the book.
pub fn surname(author: &str) -> String {
    let words = name_words(author);
    let last = match author.contains(',') {
        // `Lindqvist, Otto`
        true => words.first(),
        false => words.last(),
    };
    last.cloned().unwrap_or_default()
}

/// The searches to make for a guess, in order, until one finds something:
/// each ISBN, then title and author, then the author's books.
pub fn queries(g: &Guess) -> Vec<Query> {
    let mut out: Vec<Query> = g.isbns.iter().take(3).cloned().map(Query::Isbn).collect();
    let mut add = |q: Query| {
        if !out.contains(&q) {
            out.push(q);
        }
    };
    let mut pairs = vec![(g.title.clone(), g.author.clone())];
    pairs.extend(g.alts.iter().cloned());
    let titled: Vec<(String, String)> = pairs
        .iter()
        .map(|(t, a)| (words_only(t), words_only(a)))
        .filter(|(t, _)| !t.is_empty())
        .collect();
    for (t, a) in pairs
        .iter()
        .filter(|(t, a)| !t.is_empty() && !surname(a).is_empty())
        .take(2)
    {
        add(Query::Titled(format!(
            "title:({}) author:({})",
            words_only(t),
            surname(a)
        )));
    }
    if let Some((title, author)) = titled.first() {
        // Plain words find the book whichever part is the title.
        add(Query::Text(format!("{title} {author}").trim().to_string()));
        let short = words_only(main_title(&g.title));
        if !short.is_empty() && short != *title {
            add(Query::Text(format!("{short} {author}").trim().to_string()));
        }
    }
    for (t, a) in titled.iter().skip(1).filter(|(_, a)| !a.is_empty()) {
        add(Query::Text(format!("{t} {a}")));
    }
    // A title alone finds books by anyone; with an author known, their own
    // books are a better last try.
    let mut names: Vec<&String> = Vec::new();
    for (_, a) in &pairs {
        if !name_words(a).is_empty() && !names.iter().any(|n| name_words(n) == name_words(a)) {
            names.push(a);
        }
    }
    for a in &names[..names.len().min(2)] {
        add(Query::Author(a.to_string()));
    }
    if names.is_empty()
        && let Some((title, _)) = titled.first()
    {
        add(Query::Text(title.clone()));
    }
    out
}

/// An author as the author search returns them.
#[derive(Debug, Default, Deserialize)]
pub struct AuthorDoc {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub work_count: u32,
}

#[derive(Debug, Default, Deserialize)]
pub struct AuthorPage {
    #[serde(default)]
    pub docs: Vec<AuthorDoc>,
}

/// The author meant by a name: one with all its words, the same name
/// before a longer one (`Otto Lindqvist`, not `Otto Lindqvist Foundation`),
/// then the one with most works (not a namesake with one book).
pub fn pick_author<'a>(name: &str, docs: &'a [AuthorDoc]) -> Option<&'a AuthorDoc> {
    let want = name_words(name);
    docs.iter()
        .filter(|d| author_fit(name, std::slice::from_ref(&d.name)).is_some_and(|f| f >= 0.99))
        .max_by_key(|d| (name_words(&d.name) == want, d.work_count))
}

/// The fields asked of the search, edition ones included.
pub const FIELDS: &str = "key,title,subtitle,author_name,first_publish_year,cover_i,edition_count,language,\
editions,editions.key,editions.title,editions.subtitle,editions.publish_date,editions.publisher,\
editions.language,editions.cover_i,editions.isbn";

#[derive(Debug, Default, Deserialize)]
pub struct SearchPage {
    #[serde(default)]
    pub docs: Vec<Doc>,
}

/// A work as the search returns it, with the edition that fits best.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Doc {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub author_name: Vec<String>,
    #[serde(default)]
    pub first_publish_year: Option<u16>,
    #[serde(default)]
    pub cover_i: Option<i64>,
    #[serde(default)]
    pub edition_count: u32,
    #[serde(default)]
    pub editions: Option<EditionPage>,
    /// The first result of a search by title ([`Query::Titled`]): some
    /// edition of it has that title, maybe one the search did not return.
    #[serde(skip)]
    pub top_by_title: bool,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct EditionPage {
    #[serde(default)]
    pub docs: Vec<Edition>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Edition {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub publish_date: Vec<String>,
    #[serde(default)]
    pub publisher: Vec<String>,
    #[serde(default)]
    pub language: Vec<String>,
    #[serde(default)]
    pub cover_i: Option<i64>,
    #[serde(default)]
    pub isbn: Vec<String>,
}

/// A book offered to the user, best first.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Candidate {
    /// The work (`OL1234567W`).
    pub work: String,
    /// The edition (`OL7654321M`), when known.
    pub edition: String,
    /// The title as it goes in the name.
    pub title: String,
    /// The work's title, often the original one.
    pub original_title: String,
    pub authors: Vec<String>,
    /// Year of first publication.
    pub year: Option<u16>,
    pub edition_year: Option<u16>,
    pub publisher: String,
    pub isbn: String,
    pub language: String,
    /// Open Library cover id.
    pub cover: Option<i64>,
    /// How many editions the work has: how well known it is.
    pub editions: u32,
    /// Found by the book's ISBN: edition, publisher and ISBN are this
    /// very book's (otherwise they are another edition's, for show).
    pub exact: bool,
    pub score: f64,
}

/// `/works/OL1W` → `OL1W`.
fn id_of(key: &str) -> String {
    key.rsplit('/').next().unwrap_or_default().to_string()
}

/// Words of a name worth comparing: no initials or particles.
fn name_words(s: &str) -> Vec<String> {
    const PARTICLES: &[&str] = &[
        "de", "da", "do", "das", "dos", "e", "y", "and", "von", "van", "der", "den", "del", "la",
        "le", "di", "du", "jr", "junior", "filho", "neto", "the",
    ];
    text::strip_accents(&s.to_lowercase())
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1 && !PARTICLES.contains(w))
        .map(str::to_string)
        .collect()
}

/// The initials of a name (`C. S. Moraes` → `c`, `s`).
fn initials(s: &str) -> Vec<char> {
    text::strip_accents(&s.to_lowercase())
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() == 1)
        .filter_map(|w| w.chars().next())
        .collect()
}

/// How much of the guessed author is one of a book's authors (0 to 1);
/// `None` without one. `Brandão, J. K.` fits `J.K. Brandão`; `C. S. Moraes`
/// does not fit `Moraes Pacheco`, whose names do not start with C and S.
pub fn author_fit(guess: &str, authors: &[String]) -> Option<f64> {
    let want = name_words(guess);
    if want.is_empty() {
        return None;
    }
    let want_initials = initials(guess);
    let fit = |author: &String| {
        let have = name_words(author);
        let hit = want
            .iter()
            .filter(|w| have.iter().any(|h| h == *w || similarity(h, w) >= 0.85))
            .count();
        let fit = hit as f64 / want.len() as f64;
        // The surname and the first name must be there: `Otto Lindqvist` is
        // not `Berg Lindqvist`.
        let given = match guess.contains(',') {
            true => want.get(1),
            false => want.first(),
        };
        let needed = [
            Some(surname(guess)),
            given.filter(|_| want.len() > 1).cloned(),
        ];
        let present = |w: &String| have.iter().any(|h| h == w || similarity(h, w) >= 0.85);
        let fit = match needed.iter().flatten().all(present) {
            true => fit,
            false => fit.min(0.4),
        };
        let starts: Vec<char> = text::strip_accents(&author.to_lowercase())
            .split(|c: char| !c.is_alphanumeric())
            .filter_map(|w| w.chars().next())
            .collect();
        match want_initials.iter().all(|i| starts.contains(i)) {
            true => fit,
            false => fit.min(0.4),
        }
    };
    Some(authors.iter().map(fit).fold(0.0, f64::max))
}

/// How well a title fits the guessed one, also without subtitles.
fn title_fit(guess: &str, title: &str) -> f64 {
    if guess.is_empty() || title.is_empty() {
        return 0.0;
    }
    similarity(guess, title)
        .max(similarity(main_title(guess), main_title(title)) - 0.02)
        .max(similarity(main_title(guess), title) - 0.02)
}

/// Whether all the words of `short` are in `long`.
fn short_form(short: &str, long: &str) -> bool {
    let words = |s: &str| -> Vec<String> {
        text::strip_accents(&s.to_lowercase())
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect()
    };
    let long = words(long);
    let short = words(short);
    !short.is_empty() && short.iter().all(|w| long.contains(w))
}

fn year_of(dates: &[String]) -> Option<u16> {
    dates
        .iter()
        .find_map(|d| YEAR.find(d)?.as_str().parse().ok())
}

/// Whether a text is written in capitals only.
fn shouting(s: &str) -> bool {
    s.chars().any(char::is_alphabetic) && !s.chars().any(char::is_lowercase)
}

/// A title with its article moved to the end, as some catalogues sort it
/// (`Crônicas de Arvoredo, As`), put back in front.
fn article_first(title: &str) -> String {
    static INVERTED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(.+),\s*(o|a|os|as|um|uma|the|an?|el|la|los|las|le|les|il|lo|gli)$")
            .unwrap()
    });
    match INVERTED.captures(title.trim()) {
        Some(c) => format!("{} {}", &c[2], &c[1]),
        None => title.trim().to_string(),
    }
}

/// The title in the file's details, without what is not the title: the
/// author, the series and the number in it (`Iara Mendonça - 03 - O Céu
/// Partido` → `O Céu Partido`). It is the publisher's own title of
/// this very book, better than a catalogue's when both name the same book
/// (`Antologia poética Alda Mourão`, `Cómo Rezar` for `Como rezar`).
fn own_title(g: &Guess) -> Option<String> {
    static NUMBERED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^((livro|book|libro|parte|part|vol(ume|umen)?\.?|tomo|n[ºo°]\.?)\s*)?([0-9]{1,3}|[ivxlc]+)$").unwrap()
    });
    // Several books in one (`Título A | Título B`) have no title of their own.
    if !g.from_file || g.title.trim().is_empty() || shouting(&g.title) || g.title.contains('|') {
        return None;
    }
    let fold = |s: &str| words_only(&text::strip_accents(&s.to_lowercase()));
    let series = fold(&g.series);
    // The file's author; without one, the readings of its name (which
    // may take a title for the author, so only then).
    let names: Vec<Vec<String>> = match name_words(&g.author).is_empty() {
        true => g.alts.iter().map(|(_, a)| name_words(a)).collect(),
        false => vec![name_words(&g.author)],
    };
    let names: Vec<Vec<String>> = names.into_iter().filter(|w| !w.is_empty()).collect();
    let is_author = |p: &str| names.contains(&name_words(p));
    let parts: Vec<&str> = g
        .title
        .split(" - ")
        .flat_map(|p| p.split(" – "))
        .map(str::trim)
        .filter(|p| {
            !p.is_empty()
                && !NUMBERED.is_match(p)
                && (series.is_empty() || fold(p) != series)
                && !is_author(p)
        })
        .collect();
    let title = text::nfc(&parts.join(" - "));
    // `Author: Title`
    let title = match title.split_once(':') {
        Some((head, rest)) if is_author(head) && !rest.trim().is_empty() => rest.trim().to_string(),
        _ => title,
    };
    (!title.is_empty()).then_some(title)
}

/// A catalogue's title without the author's name in front
/// (`Author Name - Title`, `Author Name: Title`).
fn without_author(title: &str, authors: &[String]) -> String {
    for sep in [" - ", " – ", ": ", ":"] {
        if let Some((head, rest)) = title.split_once(sep)
            && !rest.trim().is_empty()
            && authors
                .iter()
                .any(|a| !name_words(a).is_empty() && name_words(a) == name_words(head))
        {
            return rest.trim().to_string();
        }
    }
    title.to_string()
}

/// A title without the note of the edition's language (`Title: European
/// Portuguese Edition`, `Title (Portuguese Edition)`), which is not part
/// of it.
fn without_language(title: &str) -> String {
    static NOTE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)\s*[:(\[–-]\s*(?:(?:european|brazilian|american|british|international)\s+)?(?:portuguese|english|spanish|french|german|italian)\s+edition\s*[)\]]?\s*$|\s*[:(\[–-]\s*edi[cç][aã]o\s+(?:em\s+portugu[eê]s|portuguesa|brasileira)\s*[)\]]?\s*$|\s*[:(\[–-]\s*edici[oó]n\s+(?:en\s+espa[nñ]ol|española)\s*[)\]]?\s*$",
        )
        .unwrap()
    });
    let t = NOTE.replace(title, "");
    match t.trim().is_empty() {
        true => title.to_string(),
        false => t.trim().to_string(),
    }
}

/// The title a book gets: the file's own when it names the same book (or
/// is that very edition, found by its ISBN), as the publisher wrote it;
/// else the catalogue's. A subtitle only the file has goes when the
/// catalogue's title is the main one (`Title: A long subtitle` for
/// `Title`).
fn choose_title(g: &Guess, catalog: &str, authors: &[String], edition: bool) -> String {
    let catalog = without_language(&without_author(
        &article_first(&text::nfc(catalog)),
        authors,
    ));
    let Some(own) = own_title(g).map(|t| without_language(&t)) else {
        return catalog;
    };
    let sub = catalog.split_once(':').map_or("", |(_, s)| s.trim());
    let same = edition
        || title_fit(&own, &catalog) >= 0.85
        || (!sub.is_empty() && title_fit(&own, sub) >= 0.9);
    if !same {
        return catalog;
    }
    let fold = |s: &str| words_only(&text::strip_accents(&s.to_lowercase()));
    if own.contains(':')
        && !catalog.contains(':')
        && fold(main_title(&own)) == fold(main_title(&catalog))
    {
        return main_title(&own).to_string();
    }
    own
}

/// The authors with the file's own spelling of its author (`C. S. Moraes`
/// for `C.S. Moraes`, `Graciano Rocha` for `Graciano Rocha de
/// Oliveira`), so one author's books share one name and one folder.
fn own_authors(g: &Guess, list: Vec<String>) -> Vec<String> {
    let mine = text::nfc(g.author.trim());
    let named = name_words(&mine).len() + initials(&mine).len() >= 2;
    if !g.from_file || !named || shouting(&mine) || mine.contains(',') {
        return list;
    }
    let mut out: Vec<String> = Vec::new();
    for a in list {
        let a = match author_fit(&mine, std::slice::from_ref(&a)).is_some_and(|f| f >= 0.99) {
            true => mine.clone(),
            false => a,
        };
        if !out.iter().any(|o| name_words(o) == name_words(&a)) {
            out.push(a);
        }
    }
    out
}

/// Ranked candidates and whether the first one is a safe automatic pick.
/// `isbn` is the ISBN the results were found by: then the first one is
/// that very edition.
///
/// `lang` (`pt`) is the reader's language: a book with no edition in it is
/// shown by its own title, not by one in a third language.
pub fn rank(g: &Guess, docs: Vec<Doc>, isbn: Option<&str>, lang: &str) -> (Vec<Candidate>, bool) {
    let lang3 = match lang.split('-').next().unwrap_or("") {
        "pt" => "por",
        "en" => "eng",
        "es" => "spa",
        _ => "",
    };
    let mut pairs = vec![(g.title.clone(), g.author.clone())];
    pairs.extend(g.alts.iter().cloned());
    let any_author = pairs.iter().any(|(_, a)| !name_words(a).is_empty());
    let mut out: Vec<(Candidate, f64, Option<f64>)> = Vec::new();
    for (i, doc) in docs.into_iter().enumerate() {
        let ed = doc
            .editions
            .as_ref()
            .and_then(|e| e.docs.first())
            .cloned()
            .unwrap_or_default();
        // The reading of the name that fits this book best.
        let (fit, tfit, afit, guessed) = pairs
            .iter()
            .map(|(t, a)| {
                let tfit = title_fit(t, &ed.title).max(title_fit(t, &doc.title));
                let afit = author_fit(a, &doc.author_name);
                let term = match afit {
                    None => 0.0,
                    Some(x) if x >= 0.5 => 0.1 * x,
                    Some(_) => -0.4,
                };
                (tfit + term, tfit, afit, t.clone())
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap_or((0.0, 0.0, None, String::new()));
        let popular = ((doc.edition_count as f64).ln_1p() / 45.0).min(0.15);
        // A first publication after the edition in hand is the catalogue's
        // mistake (a work made of one new edition, or the wrong work).
        let year = doc
            .first_publish_year
            .filter(|y| g.year.is_none_or(|edition| *y <= edition));
        let edition_year = year_of(&ed.publish_date);
        let year_bonus = if g.year.is_some() && (g.year == year || g.year == edition_year) {
            0.03
        } else {
            0.0
        };
        let by_isbn = isbn.is_some() && i == 0;
        // Books found by a word or two of the title, by someone else, are
        // left out (`A regra do êxito` finds `Um sol no fim da serra`).
        let title_hit = pairs.iter().any(|(t, _)| {
            title_fit(t, &ed.title).max(title_fit(t, &doc.title))
                >= if any_author { 0.85 } else { 0.6 }
        });
        let author_hit = pairs
            .iter()
            .any(|(_, a)| author_fit(a, &doc.author_name).is_some_and(|x| x >= 0.5));
        if !(by_isbn || title_hit || author_hit) {
            continue;
        }
        // The book the search by title and author put first, when none of
        // the titles here fits what the book is called: another translation
        // of the same work (`2077` for "Dois Mil e Setenta e Sete"),
        // unless the guess is a short form of one (`Nina Torres`).
        let translated = doc.top_by_title
            && afit.is_some_and(|a| a >= 0.5)
            && !guessed.is_empty()
            && tfit < 0.8
            && !short_form(&guessed, &ed.title)
            && !short_form(&guessed, &doc.title);
        // It ranks as if its title fitted, but is never picked by itself.
        let ranked = if translated { fit.max(0.95 + 0.1) } else { fit };
        // Open Library's own order counts a little.
        let score = if by_isbn {
            2.0
        } else {
            ranked + popular + year_bonus - i as f64 * 0.01
        };
        // The edition's title is the one in the reader's language; for
        // another translation, the guessed title stays.
        let title = if by_isbn || title_fit(&guessed, &ed.title) >= 0.8 {
            ed.title.clone()
        } else if title_fit(&guessed, &doc.title) >= 0.8 {
            doc.title.clone()
        } else if translated {
            guessed.clone()
        } else if !ed.title.is_empty()
            && (lang3.is_empty() || ed.language.iter().any(|l| l == lang3))
        {
            ed.title.clone()
        } else {
            doc.title.clone()
        };
        let names = own_authors(g, authors(&doc.author_name));
        let title = choose_title(g, &title, &names, by_isbn && plausible(g, &title, &names));
        let isbn_here = match isbn {
            Some(i) if by_isbn => i.to_string(),
            _ => ed.isbn.iter().find_map(|i| isbn13(i)).unwrap_or_default(),
        };
        let c = Candidate {
            work: id_of(&doc.key),
            edition: id_of(&ed.key),
            title: text::collapse_spaces(title.trim()),
            original_title: article_first(&text::nfc(&doc.title)),
            authors: names,
            year,
            edition_year,
            publisher: ed.publisher.first().cloned().unwrap_or_default(),
            isbn: isbn_here,
            language: ed.language.first().cloned().unwrap_or_default(),
            cover: ed.cover_i.or(doc.cover_i),
            editions: doc.edition_count,
            exact: by_isbn,
            score,
        };
        out.push((
            c,
            if by_isbn { 1.0 } else { tfit },
            if by_isbn { Some(1.0) } else { afit },
        ));
    }
    // One work per result (Open Library repeats some).
    let mut seen = std::collections::HashSet::new();
    out.retain(|(c, ..)| seen.insert(c.work.clone()));
    out.sort_by(|a, b| b.0.score.total_cmp(&a.0.score));
    let auto = match out.as_slice() {
        [] => false,
        [(best, tfit, afit), rest @ ..] => {
            if isbn.is_some() && best.score >= 2.0 {
                // Without an author the name would lack one: the review asks.
                !best.authors.is_empty()
                    && (plausible(g, &best.title, &best.authors)
                        || plausible(g, &best.original_title, &best.authors))
            } else {
                let author_ok = match afit {
                    Some(a) => *a >= 0.5,
                    None => *tfit >= 0.99,
                };
                // A close second that is the same book (Open Library has
                // duplicates) does not make the choice doubtful.
                let clear = rest.first().is_none_or(|(second, ..)| {
                    second.score < best.score - 0.1 || same_book(best, second)
                });
                *tfit >= 0.95 && author_ok && clear
            }
        }
    };
    (out.into_iter().map(|(c, ..)| c).collect(), auto)
}

/// Each author once (`Ítalo Vieira Junior, Ítalo Vieira Júnior`), without
/// placeholders (`[author not identified]`) or a stray comma after the
/// name (`J.R.R. Valença,`).
fn authors(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in names {
        let n = text::nfc(n.trim().trim_end_matches([',', ';']).trim());
        let n = n.as_str();
        if n.is_empty() || n.starts_with('[') {
            continue;
        }
        if !out.iter().any(|o| name_words(o) == name_words(n)) {
            out.push(n.to_string());
        }
    }
    out
}

/// The reading of the guess (title and author) that fits a book best, so
/// a name read the wrong way round shows the right way in the review.
pub fn reading(g: &Guess, c: &Candidate) -> (String, String) {
    let mut pairs = vec![(g.title.clone(), g.author.clone())];
    pairs.extend(g.alts.iter().cloned());
    pairs
        .into_iter()
        .max_by(|a, b| {
            let fit = |(t, au): &(String, String)| {
                title_fit(t, &c.title).max(title_fit(t, &c.original_title))
                    + author_fit(au, &c.authors).unwrap_or(0.0)
            };
            fit(a).total_cmp(&fit(b))
        })
        .unwrap_or_default()
}

/// A book as BrasilAPI returns it by ISBN, from the records Brazilian
/// publishers keep at the CBL and other catalogues.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct BrasilBook {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub authors: Option<Vec<String>>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub year: Option<serde_json::Value>,
}

/// Whether a book found by ISBN is the one the file says it is: an ISBN
/// read from the text may be another book's (an advert at the end), and
/// catalogues have mistakes. Without a title or author to compare with,
/// the ISBN is trusted.
pub fn plausible(g: &Guess, title: &str, authors: &[String]) -> bool {
    let mut pairs = vec![(g.title.clone(), g.author.clone())];
    pairs.extend(g.alts.iter().cloned());
    pairs.iter().any(|(t, a)| {
        let title_ok = t.is_empty()
            || title_fit(t, title) >= 0.6
            || short_form(title, t)
            || short_form(t, title);
        let author_ok = authors.is_empty()
            || name_words(a).is_empty()
            || author_fit(a, authors).is_some_and(|f| f >= 0.5);
        title_ok && author_ok
    })
}

static FORMAT_NOTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\s*(?:[-–(\[]\s*)(?:e-?pub|e-?book|ebook|digital|kindle|pdf|livro eletr[oô]nico|formato menor|edi[cç][aã]o digital)\s*[)\]]?\s*$",
    )
    .unwrap()
});

/// A title as some records keep it: `((ED) A regra do êxito`,
/// `Terra seca - EPUB`, in capitals, or cut short (`A porta` for `A porta para a
/// fartura`): then the file's own title is better.
fn brasil_title(title: &str, g: &Guess) -> String {
    let mut t = title.trim();
    while let Some(rest) = t.strip_prefix('(') {
        match rest.trim_start_matches('(').split_once(')') {
            Some((tag, after)) if tag.len() <= 4 => t = after.trim_start(),
            _ => break,
        }
    }
    let t = text::collapse_spaces(FORMAT_NOTE.replace(t, "").trim());
    let fold = |s: &str| words_only(&text::strip_accents(&s.to_lowercase()));
    // The file's title without subtitle or what follows a dash
    // (`As 12 Regras do Êxito – Otto Lindqvist - Jonas Prado`).
    let first = main_title(&g.title)
        .split(" - ")
        .next()
        .and_then(|t| t.split(" – ").next())
        .unwrap_or_default()
        .trim()
        .to_string();
    let (mine, theirs) = (fold(&first), fold(&t));
    if mine.is_empty() {
        return t;
    }
    let caps = t.chars().any(char::is_alphabetic) && !t.chars().any(char::is_lowercase);
    // `A porta` for `A porta para a fartura`; the same title with the
    // file's accents and capitals; `2077: edição especial` for `2077`.
    let cut_short = mine.starts_with(&format!("{theirs} "));
    let same = mine == theirs;
    let with_note = fold(main_title(&t)) == mine;
    let longer_caps = caps && theirs.starts_with(&format!("{mine} "));
    if cut_short || same || with_note || longer_caps || (caps && title_fit(&first, &t) >= 0.9) {
        return first;
    }
    t
}

/// The edition of an ISBN BrasilAPI knows, and whether it is plausibly the
/// file's book. Its authors include translators and illustrators (`C. S.
/// Moraes, Rita Nunes`), and sometimes the publisher or nobody: when
/// the file names the author, the authors that fit it are kept, or the
/// file's author.
pub fn brasil_candidate(isbn: &str, b: &BrasilBook, g: &Guess) -> Option<(Candidate, bool)> {
    let raw = b.title.as_deref().unwrap_or_default();
    let title = brasil_title(raw, g);
    if title.is_empty() {
        return None;
    }
    let all = authors(b.authors.as_deref().unwrap_or_default());
    let publisher = b.publisher.clone().unwrap_or_default().trim().to_string();
    let named: Vec<String> = all
        .iter()
        .filter(|a| name_words(a) != name_words(&publisher))
        .cloned()
        .collect();
    let ok = plausible(g, raw, &named) || plausible(g, &title, &named);
    let fitting: Vec<String> = named
        .iter()
        .filter(|a| {
            std::iter::once(&g.author)
                .chain(g.alts.iter().map(|(_, a)| a))
                .any(|w| author_fit(w, std::slice::from_ref(a)).is_some_and(|f| f >= 0.5))
        })
        .cloned()
        .collect();
    let authors = match (fitting.is_empty(), g.author.trim().is_empty()) {
        (false, _) => fitting,
        (true, false) if ok => vec![g.author.trim().to_string()],
        _ => named,
    };
    let year = match &b.year {
        Some(serde_json::Value::Number(n)) => n.as_u64().and_then(|y| u16::try_from(y).ok()),
        Some(serde_json::Value::String(y)) => y.trim().get(..4).and_then(|y| y.parse().ok()),
        _ => None,
    };
    let title = choose_title(g, &title, &authors, ok);
    let c = Candidate {
        original_title: title.clone(),
        title,
        authors: own_authors(g, authors),
        edition_year: year.filter(|y| (1400..=2200).contains(y)),
        publisher,
        isbn: isbn.to_string(),
        language: "por".into(),
        editions: 1,
        exact: true,
        score: 2.0,
        ..Default::default()
    };
    Some((c, ok))
}

/// The edition found on BrasilAPI with what Open Library knows of its work,
/// when the search found the same book: its first year, original title and
/// cover. A plausible edition comes first, a safe choice; another one goes
/// after the books found, for the review.
pub fn with_edition(
    edition: Candidate,
    ok: bool,
    (found, auto): (Vec<Candidate>, bool),
) -> (Vec<Candidate>, bool) {
    let mut out = found;
    if !ok {
        out.push(edition);
        return (out, auto);
    }
    let author = |c: &Candidate| author_fit(c.authors.first().map_or("", |a| a), &edition.authors);
    let work = out.iter().position(|c| {
        title_fit(&edition.title, &c.title).max(title_fit(&edition.title, &c.original_title)) >= 0.9
            && author(c).is_none_or(|f| f >= 0.5)
    });
    let first = match work {
        Some(i) => {
            let w = out.remove(i);
            Candidate {
                work: w.work,
                original_title: w.original_title,
                year: w.year,
                cover: w.cover,
                editions: w.editions,
                authors: if edition.authors.is_empty() {
                    w.authors
                } else {
                    edition.authors.clone()
                },
                ..edition
            }
        }
        None => edition,
    };
    // Without an author the name would lack one: the review asks.
    let auto = !first.authors.is_empty();
    out.insert(0, first);
    (out, auto)
}

fn same_book(a: &Candidate, b: &Candidate) -> bool {
    let author = |c: &Candidate| c.authors.first().map(|a| name_words(a)).unwrap_or_default();
    similarity(&a.title, &b.title) == 1.0 && author(a) == author(b)
}

/// The book chosen for a file, as the rule needs it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BookMatch {
    pub work: String,
    pub title: String,
    pub original_title: String,
    pub authors: Vec<String>,
    pub year: Option<u16>,
    pub edition_year: Option<u16>,
    pub publisher: String,
    pub isbn: String,
    /// From the file's details.
    pub series: String,
    pub series_index: String,
}

/// A name as libraries sort it, by surname: `Moraes, C. S.`, `Alvarenga,
/// Tomé de`, `Rocha Filho, Graciano`. Names already in that form, or
/// of one word, stay as they are.
pub fn author_sort(name: &str) -> String {
    const SUFFIXES: &[&str] = &[
        "jr", "jr.", "junior", "júnior", "filho", "neto", "sobrinho", "ii", "iii", "iv",
    ];
    let name = text::collapse_spaces(name.trim());
    let words: Vec<&str> = name.split(' ').collect();
    if name.contains(',') || words.len() < 2 {
        return name;
    }
    let last = words[words.len() - 1].to_lowercase();
    let cut = match SUFFIXES.contains(&last.as_str()) && words.len() > 2 {
        true => words.len() - 2,
        false => words.len() - 1,
    };
    format!("{}, {}", words[cut..].join(" "), words[..cut].join(" "))
}

/// The first letter of the surname, for `M/Moraes, C. S.` folders:
/// without accents, in capitals; `#` for one that is not a letter.
pub fn initial(name: &str) -> String {
    match text::strip_accents(&author_sort(name)).chars().next() {
        Some(c) if c.is_alphabetic() => c.to_uppercase().collect(),
        Some(_) => "#".into(),
        None => String::new(),
    }
}

/// The number in a series with two digits, as tracks have, so that the
/// books sort in order: `1` → `01`, `2.0` → `02`, `1.5` → `01.5`.
fn volume(index: &str) -> String {
    let index = index.trim();
    let (whole, part) = index.split_once('.').unwrap_or((index, ""));
    match whole.parse::<u32>() {
        Ok(n) if part.trim_matches('0').is_empty() => format!("{n:02}"),
        Ok(n) => format!("{n:02}.{part}"),
        Err(_) => index.to_string(),
    }
}

/// The new name from the rule's template: `{author}`, `{authors}`,
/// `{author sort}`, `{initial}`, `{title}`, `{original}`, `{year}`,
/// `{edition year}`, `{publisher}`, `{isbn}`, `{series}`, `{volume}`,
/// `{id}` (also in Portuguese and Spanish). Brackets and dashes left empty
/// by missing values go.
pub fn book_name(template: &str, m: &BookMatch, colon: &str) -> String {
    let safe = |s: &str| safe_title(s, colon);
    let first = m.authors.first().map(String::as_str).unwrap_or_default();
    let author = safe(first);
    let sorted = safe(&author_sort(first));
    let letter = initial(first);
    let authors = safe(&m.authors.join(" & "));
    let title = safe(&m.title);
    let original = safe(&m.original_title);
    let year = m.year.map(|y| y.to_string()).unwrap_or_default();
    let edition_year = m.edition_year.map(|y| y.to_string()).unwrap_or_default();
    let publisher = safe(&m.publisher);
    let series = safe(&m.series);
    let volume = safe(&volume(&m.series_index));
    let out = crate::rules::expand(template, |key| match key {
        "author" | "autor" => Some(author.as_str()),
        "author sort" | "autor invertido" => Some(sorted.as_str()),
        "initial" | "inicial" => Some(letter.as_str()),
        "authors" | "autores" => Some(authors.as_str()),
        "title" | "título" | "titulo" => Some(title.as_str()),
        "original" | "original title" | "título original" | "titulo original" => {
            Some(original.as_str())
        }
        "year" | "ano" | "año" => Some(year.as_str()),
        "edition year" | "ano da edição" | "ano da edicao" | "año de edición"
        | "año de edicion" => Some(edition_year.as_str()),
        "publisher" | "editora" | "editorial" => Some(publisher.as_str()),
        "isbn" => Some(m.isbn.as_str()),
        "series" | "série" | "serie" => Some(series.as_str()),
        "volume" | "volumen" | "número" | "numero" => Some(volume.as_str()),
        "id" | "olid" => Some(m.work.as_str()),
        _ => None,
    });
    tidy(&out)
}

/// Removes what missing values leave behind: `()`, `[]`, `#`, and dashes
/// at the ends or doubled.
pub fn tidy(s: &str) -> String {
    static EMPTY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\(\s*\)|\[\s*\]|\{\s*\}|#(\s|$)").unwrap());
    static DOUBLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\s[-–—]\s*){2,}").unwrap());
    let s = EMPTY.replace_all(s, " ");
    let s = DOUBLE.replace_all(&s, " - ");
    text::collapse_spaces(&s)
        .trim_matches(|c: char| c.is_whitespace() || "-–—,".contains(c))
        .to_string()
}

/// Folders made from the rule's folder template (levels split at `/`);
/// levels that come out empty are skipped.
pub fn folders(template: &str, m: &BookMatch, colon: &str) -> Vec<String> {
    template
        .split('/')
        .map(|level| book_name(level, m, colon))
        .filter(|l| !l.is_empty())
        .collect()
}

/// Where the book's folders go: above folders already named after its
/// title, its author or both (`Author/Title/book.epub` stays in `Author`'s
/// parent, not inside), else the file's own folder.
pub fn library_root(dir: &Path, m: &BookMatch, levels: &[String]) -> PathBuf {
    let author = m.authors.first().cloned().unwrap_or_default();
    let both = [
        format!("{author} - {}", m.title),
        format!("{} - {author}", m.title),
    ];
    let mut names: Vec<&str> = levels.iter().map(String::as_str).collect();
    names.extend([
        m.title.as_str(),
        m.original_title.as_str(),
        m.series.as_str(),
    ]);
    let sorted: Vec<String> = m.authors.iter().map(|a| author_sort(a)).collect();
    let letter = m.authors.first().map(|a| initial(a)).unwrap_or_default();
    names.extend(m.authors.iter().map(String::as_str));
    names.extend(sorted.iter().map(String::as_str));
    names.push(&letter);
    names.extend(both.iter().map(String::as_str));
    let mut cur = dir;
    for _ in 0..levels.len() {
        let name = cur.file_name().unwrap_or_default().to_string_lossy();
        if !crate::music::names_any(&name, &names) {
            break;
        }
        match cur.parent() {
            Some(up) if !up.as_os_str().is_empty() => cur = up,
            _ => break,
        }
    }
    cur.to_path_buf()
}

/// A work's details (`/works/OL1W.json`).
#[derive(Debug, Default, Deserialize)]
pub struct Work {
    #[serde(default)]
    pub description: Option<Text>,
    #[serde(default)]
    pub subjects: Vec<String>,
}

/// Open Library gives some texts as a string, others as `{"value": …}`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Text {
    Plain(String),
    Typed { value: String },
}

impl Text {
    pub fn as_str(&self) -> &str {
        match self {
            Text::Plain(s) | Text::Typed { value: s } => s,
        }
    }
}

/// Open Library language codes (`por`) as the two-letter ones readers use.
fn language(code: &str) -> String {
    let two = match code {
        "por" => "pt",
        "eng" => "en",
        "spa" => "es",
        "fre" | "fra" => "fr",
        "ger" | "deu" => "de",
        "ita" => "it",
        "rus" => "ru",
        "jpn" => "ja",
        "chi" | "zho" => "zh",
        "dut" | "nld" => "nl",
        other => other,
    };
    two.to_string()
}

/// Open Library's descriptions are Markdown: `**bold**` and `[text](link)`
/// become plain text.
fn plain(s: &str) -> String {
    static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\([^)]*\)").unwrap());
    LINK.replace_all(s, "$1")
        .replace("**", "")
        .replace("__", "")
}

/// The `metadata.opf` file, which Audiobookshelf and e-book managers read
/// next to a book.
pub fn opf(m: &BookMatch, c: &Candidate, work: &Work, cover: Option<&str>) -> String {
    let id = if m.isbn.is_empty() { "olid" } else { "isbn" };
    let mut o = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
<package xmlns=\"http://www.idpf.org/2007/opf\" unique-identifier=\"{id}\" version=\"2.0\">\n\
 <metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:opf=\"http://www.idpf.org/2007/opf\">\n",
    );
    tag(&mut o, "dc:title", &m.title);
    for a in &m.authors {
        o.push_str(&format!(
            "  <dc:creator opf:role=\"aut\">{}</dc:creator>\n",
            xml(a)
        ));
    }
    tag(&mut o, "dc:publisher", &m.publisher);
    let year = m
        .edition_year
        .or(m.year)
        .map(|y| y.to_string())
        .unwrap_or_default();
    tag(&mut o, "dc:date", &year);
    tag(&mut o, "dc:language", &language(&c.language));
    if let Some(d) = &work.description {
        // Links and notes Open Library appends after a line of dashes go.
        let text = d
            .as_str()
            .split("\n----------")
            .next()
            .unwrap_or_default()
            .trim();
        tag(&mut o, "dc:description", &plain(text));
    }
    for s in work.subjects.iter().take(10) {
        tag(&mut o, "dc:subject", s);
    }
    if !m.isbn.is_empty() {
        o.push_str(&format!(
            "  <dc:identifier id=\"isbn\" opf:scheme=\"ISBN\">{}</dc:identifier>\n",
            xml(&m.isbn)
        ));
    }
    if !m.work.is_empty() {
        o.push_str(&format!(
            "  <dc:identifier id=\"olid\" opf:scheme=\"OPENLIBRARY\">{}</dc:identifier>\n",
            xml(&m.work)
        ));
    }
    if !m.series.is_empty() {
        o.push_str(&format!(
            "  <meta name=\"calibre:series\" content=\"{}\"/>\n",
            xml(&m.series)
        ));
        if !m.series_index.is_empty() {
            o.push_str(&format!(
                "  <meta name=\"calibre:series_index\" content=\"{}\"/>\n",
                xml(&m.series_index)
            ));
        }
    }
    o.push_str(" </metadata>\n");
    if let Some(cover) = cover {
        o.push_str(&format!(
            " <guide>\n  <reference type=\"cover\" title=\"Cover\" href=\"{}\"/>\n </guide>\n",
            xml(cover)
        ));
    }
    o.push_str("</package>\n");
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(work: &str, title: &str, author: &str, editions: u32, ed_title: &str) -> Doc {
        Doc {
            key: format!("/works/{work}"),
            title: title.into(),
            author_name: vec![author.into()],
            first_publish_year: Some(1900),
            edition_count: editions,
            editions: Some(EditionPage {
                docs: vec![Edition {
                    key: "/books/OL1M".into(),
                    title: ed_title.into(),
                    publish_date: vec!["2002".into()],
                    publisher: vec!["Editora Vela".into()],
                    ..Default::default()
                }],
            }),
            ..Default::default()
        }
    }

    #[test]
    fn names() {
        let g = guess_name("Tomé de Alvarenga - Vento Leste (1899)");
        assert_eq!(
            (g.title.as_str(), g.author.as_str(), g.year),
            ("Vento Leste", "Tomé de Alvarenga", Some(1899))
        );
        assert_eq!(
            g.alts,
            vec![("Tomé de Alvarenga".to_string(), "Vento Leste".to_string())]
        );

        let g = guess_name("Ecos by Lia Marta Quaresma");
        assert_eq!(
            (g.title.as_str(), g.author.as_str()),
            ("Ecos", "Lia Marta Quaresma")
        );

        let g = guess_name("Rui Valadares - 2077 (z-lib.org)");
        assert_eq!(
            (g.title.as_str(), g.author.as_str()),
            ("2077", "Rui Valadares")
        );

        let g = guess_name("2077");
        assert_eq!(g.title, "2077");

        let g = guess_name(
            "Quaresma, Lia Marta - Ecos_ Uma breve historia do silencio (2015, Vela) - libgen.li",
        );
        assert_eq!(g.author, "Quaresma, Lia Marta");
        assert_eq!(g.title, "Ecos: Uma breve historia do silencio");
        assert_eq!(g.year, Some(2015));

        let g = guess_name("ISBN 978-0-00-000001-9");
        assert_eq!(g.isbns, vec!["9780000000019"]);
        assert_eq!(g.title, "");
        let g = guess_name("9780000000019");
        assert_eq!(g.isbns, vec!["9780000000019"]);

        let g = guess_name("Brandão - 01 - Nina Torres e a Chave de Vidro");
        assert_eq!(g.series_index, "01");
        assert_eq!(
            (g.title.as_str(), g.author.as_str()),
            ("Nina Torres e a Chave de Vidro", "Brandão")
        );
    }

    #[test]
    fn from_details() {
        let meta = BookMeta {
            title: "Vento Leste".into(),
            authors: vec!["Tomé de Alvarenga".into()],
            isbns: vec!["9780000000026".into()],
            ..Default::default()
        };
        let g = guess("vento_leste_final", &meta);
        assert!(g.from_file);
        assert_eq!(g.author, "Tomé de Alvarenga");
        assert_eq!(queries(&g)[0], Query::Isbn("9780000000026".into()));
        assert_eq!(
            queries(&g)[1],
            Query::Titled("title:(Vento Leste) author:(alvarenga)".into())
        );
    }

    #[test]
    fn authors() {
        assert_eq!(
            author_fit("Brandão, J. K.", &["J.K. Brandão".into()]),
            Some(1.0)
        );
        assert_eq!(
            author_fit(
                "Tomé de Alvarenga",
                &["Joaquim Pedro Tomé de Alvarenga".into()]
            ),
            Some(1.0)
        );
        assert!(author_fit("Caio Ventura", &["Ivo Saraiva".into()]).unwrap() < 0.5);
        assert!(author_fit("C. S. Moraes", &["Moraes Pacheco".into()]).unwrap() < 0.5);
        assert_eq!(
            author_fit(
                "J. R. R. Valença",
                &["Heitor Valença".into(), "J.R.R. Valença".into()]
            ),
            Some(1.0)
        );
        assert_eq!(author_fit("", &["X".into()]), None);
    }

    #[test]
    fn ranking() {
        let g = guess_name("Tomé de Alvarenga - Vento Leste");
        let docs = vec![
            doc("OL2W", "Vento Leste", "Tomé de Alvarênga", 6, "Vento Leste"),
            doc(
                "OL1W",
                "Vento Leste",
                "Tomé de Alvarenga",
                90,
                "Vento Leste",
            ),
            doc(
                "OL3W",
                "Memórias de um Farol",
                "Tomé de Alvarenga",
                50,
                "Memórias de um farol",
            ),
        ];
        let (c, auto) = rank(&g, docs, None, "");
        assert_eq!(c[0].work, "OL1W");
        assert!(auto, "a duplicate second does not make it doubtful");

        // The other order (`Title - Author`) is found too.
        let g = guess_name("Vento Leste - Tomé de Alvarenga");
        let (c, auto) = rank(
            &g,
            vec![doc(
                "OL1W",
                "Vento Leste",
                "Tomé de Alvarenga",
                90,
                "Vento Leste",
            )],
            None,
            "",
        );
        assert_eq!(c[0].title, "Vento Leste");
        assert!(auto && !c[0].exact);
        assert_eq!(
            reading(&g, &c[0]),
            ("Vento Leste".into(), "Tomé de Alvarenga".into())
        );
        assert_eq!(
            super::authors(&[
                "A Júnior".into(),
                "A Junior".into(),
                "[author not identified]".into()
            ]),
            vec!["A Júnior"]
        );

        // Another translation, found by its title: the title the book has
        // stays.
        let g = guess_name("Rui Valadares - 2077");
        let translated = Doc {
            top_by_title: true,
            ..doc(
                "OL9W",
                "Twenty Seventy-Seven",
                "Rui Valadares",
                700,
                "Dois Mil e Setenta e Sete",
            )
        };
        let (c, auto) = rank(&g, vec![translated], None, "");
        assert_eq!(c[0].title, "2077");
        assert_eq!(c[0].original_title, "Twenty Seventy-Seven");
        assert!(!auto);

        // Another book by the author, found by plain words: its own title.
        let g = guess_name("Caio Ventura - Vento Leste");
        let (c, auto) = rank(
            &g,
            vec![doc(
                "OL5W",
                "O Relojoeiro",
                "Caio Ventura",
                141,
                "O Relojoeiro",
            )],
            None,
            "",
        );
        assert_eq!(c[0].title, "O Relojoeiro");
        assert!(!auto);

        // A short form of the title is not another translation.
        let g = guess_name("Brandão - Nina Torres");
        let hp = Doc {
            top_by_title: true,
            ..doc(
                "OL8W",
                "Nina Torres and the Glass Key",
                "J. K. Brandão",
                400,
                "Nina Torres e a Chave de Vidro",
            )
        };
        let (c, _) = rank(&g, vec![hp], None, "");
        assert_eq!(c[0].title, "Nina Torres e a Chave de Vidro");

        // Wrong author: never automatic.
        let g = guess_name("Caio Ventura - Vento Leste");
        let (_, auto) = rank(
            &g,
            vec![doc(
                "OL1W",
                "Vento Leste",
                "Tomé de Alvarenga",
                90,
                "Vento Leste",
            )],
            None,
            "",
        );
        assert!(!auto);

        // By ISBN: that edition.
        let g = Guess {
            isbns: vec!["9780000000026".into()],
            ..Default::default()
        };
        let (c, auto) = rank(
            &g,
            vec![doc(
                "OL1W",
                "Vento Leste",
                "Tomé de Alvarenga",
                90,
                "Vento Leste",
            )],
            Some("9780000000026"),
            "",
        );
        assert!(auto);
        assert_eq!(c[0].isbn, "9780000000026");
        assert_eq!(c[0].edition_year, Some(2002));
    }

    #[test]
    fn brasilapi() {
        let g = guess_name("A regra do êxito - Otto Lindqvist");
        let b: BrasilBook = serde_json::from_str(
            r#"{"isbn":"9780000000033","title":"((ED) A regra do êxito","authors":["Otto Lindqvist","Fulano Tradutor"],
               "publisher":"Editora Pedra Azul","year":null}"#,
        )
        .unwrap();
        let (e, ok) = brasil_candidate("9780000000033", &b, &g).unwrap();
        assert!(ok);
        assert_eq!(e.title, "A regra do êxito");
        assert_eq!(e.authors, vec!["Otto Lindqvist"]);
        assert!(e.exact && e.edition_year.is_none());

        // With the work Open Library has: its year and cover.
        let work = Candidate {
            work: "OL1W".into(),
            title: "A Regra do Êxito".into(),
            original_title: "The Rule of Success".into(),
            authors: vec!["Otto Lindqvist".into()],
            year: Some(1928),
            cover: Some(7),
            ..Default::default()
        };
        let other = Candidate {
            work: "OL2W".into(),
            title: "Quem Planta Colhe".into(),
            ..work.clone()
        };
        let (c, auto) = with_edition(e.clone(), true, (vec![other.clone(), work], false));
        assert!(auto);
        assert_eq!(
            (c[0].work.as_str(), c[0].year, c[0].cover),
            ("OL1W", Some(1928), Some(7))
        );
        assert_eq!(c[0].isbn, "9780000000033");
        assert_eq!(c[0].title, "A regra do êxito");
        assert_eq!(c.len(), 2);
        // Without it, the edition alone.
        let (c, _) = with_edition(e, true, (vec![other], false));
        assert_eq!(c[0].work, "");
        assert_eq!(c[1].work, "OL2W");

        // Cut short, with no author, or another book's ISBN.
        let g = guess(
            "x",
            &BookMeta {
                title: "A porta para a fartura".into(),
                authors: vec!["Otto Lindqvist".into()],
                ..Default::default()
            },
        );
        let b = BrasilBook {
            title: Some("A porta".into()),
            authors: Some(vec![]),
            ..Default::default()
        };
        let (e, ok) = brasil_candidate("1", &b, &g).unwrap();
        assert!(ok);
        assert_eq!(
            (e.title.as_str(), e.authors.clone()),
            ("A porta para a fartura", vec!["Otto Lindqvist".to_string()])
        );
        let b = BrasilBook {
            title: Some("Terra seca - EPUB".into()),
            ..Default::default()
        };
        assert_eq!(
            brasil_candidate("1", &b, &Guess::default())
                .unwrap()
                .0
                .title,
            "Terra seca"
        );
        let b = BrasilBook {
            title: Some("Rumo incerto".into()),
            authors: Some(vec!["Davi Pombal".into()]),
            ..Default::default()
        };
        assert!(!brasil_candidate("1", &b, &g).unwrap().1);
    }

    #[test]
    fn sorting() {
        assert_eq!(author_sort("C. S. Moraes"), "Moraes, C. S.");
        assert_eq!(author_sort("Tomé de Alvarenga"), "Alvarenga, Tomé de");
        assert_eq!(author_sort("Graciano Rocha Filho"), "Rocha Filho, Graciano");
        assert_eq!(author_sort("Lindqvist, Otto"), "Lindqvist, Otto");
        assert_eq!(author_sort("Teobaldo"), "Teobaldo");
        assert_eq!(initial("Iná de Quevedo"), "Q");
        assert_eq!(initial("Élio Zanetti"), "Z");
        assert_eq!(
            (volume("1"), volume("2.0"), volume("1.5"), volume("")),
            ("01".into(), "02".into(), "01.5".into(), String::new())
        );

        let m = BookMatch {
            title: "Prodígios".into(),
            authors: vec!["C. S. Moraes".into()],
            series: "Obras".into(),
            series_index: "3".into(),
            ..Default::default()
        };
        // Windows refuses folder names ending in a dot.
        assert_eq!(
            folders("{inicial}/{autor invertido}/{título}", &m, " - "),
            ["M", "Moraes, C. S", "Prodígios"]
        );
        assert_eq!(
            book_name("{série} {volume} - {título}", &m, " - "),
            "Obras 03 - Prodígios"
        );
        // Run again over its own folders: they are not nested again.
        let root = library_root(
            Path::new("/b/M/Moraes, C. S./Prodígios"),
            &m,
            &folders("{inicial}/{autor invertido}/{título}", &m, " - "),
        );
        assert_eq!(root, Path::new("/b"));
    }

    #[test]
    fn own_details() {
        let g = guess(
            "x",
            &BookMeta {
                title: "Iara Mendonça - 03 - O Céu Partido".into(),
                authors: vec!["Iara Mendonça".into()],
                ..Default::default()
            },
        );
        assert_eq!(own_title(&g).as_deref(), Some("O Céu Partido"));
        let g = guess(
            "x",
            &BookMeta {
                title: "O Reino das Marés - Parte 2 - As Duas Pontes".into(),
                authors: vec!["C.S. Moraes".into()],
                series: "O Reino das Marés".into(),
                ..Default::default()
            },
        );
        assert_eq!(own_title(&g).as_deref(), Some("As Duas Pontes"));
        assert_eq!(
            own_authors(&g, vec!["C. S. Moraes".into(), "Abel Nogueira".into()]),
            ["C.S. Moraes", "Abel Nogueira"]
        );
        // Composed accents: `Ma\u{301}rio` and `Mário` are one name.
        let g = guess(
            "x",
            &BookMeta {
                title: "Poesias".into(),
                authors: vec!["Ma\u{301}rio Esteves".into()],
                ..Default::default()
            },
        );
        assert_eq!(g.author, "Mário Esteves");
        assert_eq!(
            article_first("Crônicas de Arvoredo - Volume Único, As"),
            "As Crônicas de Arvoredo - Volume Único"
        );
        assert_eq!(article_first("Recife, Oregon"), "Recife, Oregon");
        assert!(author_fit("Otto Lindqvist", &["Berg Lindqvist".into()]).unwrap() < 0.5);
    }

    #[test]
    fn chosen_titles() {
        let g = |title: &str, author: &str| {
            guess(
                "x",
                &BookMeta {
                    title: title.into(),
                    authors: vec![author.into()],
                    ..Default::default()
                },
            )
        };
        let a = ["Rui Prado".to_string()];
        // The author in front of a title goes, from the file or the catalogue.
        assert_eq!(
            choose_title(
                &g("Rui Prado: Contos reunidos", "Rui Prado"),
                "Contos reunidos",
                &a,
                true
            ),
            "Contos reunidos"
        );
        assert_eq!(
            choose_title(
                &g("Contos reunidos", "Rui Prado"),
                "Rui Prado - Contos reunidos",
                &a,
                false
            ),
            "Contos reunidos"
        );
        // A subtitle only the file has goes.
        assert_eq!(
            choose_title(
                &g("A casa: uma história longa", "Rui Prado"),
                "A casa",
                &a,
                true
            ),
            "A casa"
        );
        // Several books in one: the catalogue's title.
        assert_eq!(
            choose_title(
                &g("Livro A | Livro B", "Rui Prado"),
                "Livros A e B",
                &a,
                true
            ),
            "Livros A e B"
        );
        // The file's title as the subtitle of the catalogue's.
        assert_eq!(
            choose_title(
                &g("As Duas Pontes", "Rui Prado"),
                "O Reino: As duas pontes",
                &a,
                false
            ),
            "As Duas Pontes"
        );
        // The edition's language is not part of the title.
        assert_eq!(
            choose_title(
                &g("A rua: European Portuguese Edition", "Rui Prado"),
                "A rua",
                &a,
                true
            ),
            "A rua"
        );
        assert_eq!(
            choose_title(&g("x", ""), "Atitude (Portuguese Edition)", &a, false),
            "Atitude"
        );
        // Another book: the catalogue's.
        assert_eq!(
            choose_title(&g("Contos reunidos", "Rui Prado"), "Poemas", &a, false),
            "Poemas"
        );
    }

    #[test]
    fn year_after_edition() {
        let mut g = guess_name("Rui Prado - Contos reunidos");
        g.year = Some(2015);
        let d = |year| Doc {
            first_publish_year: Some(year),
            ..doc("OL1W", "Contos reunidos", "Rui Prado", 3, "Contos reunidos")
        };
        assert_eq!(rank(&g, vec![d(2022)], None, "").0[0].year, None);
        assert_eq!(rank(&g, vec![d(1950)], None, "").0[0].year, Some(1950));
    }

    #[test]
    fn naming() {
        let m = BookMatch {
            work: "OL1W".into(),
            title: "Nina Torres e a Chave de Vidro".into(),
            authors: vec!["J. K. Brandão".into()],
            year: Some(1998),
            series: "Nina Torres".into(),
            series_index: "1".into(),
            ..Default::default()
        };
        assert_eq!(
            book_name("{autor} - {título} ({ano})", &m, " - "),
            "J. K. Brandão - Nina Torres e a Chave de Vidro (1998)"
        );
        assert_eq!(
            book_name("{série} {volume} - {título}", &m, " - "),
            "Nina Torres 01 - Nina Torres e a Chave de Vidro"
        );
        let none = BookMatch {
            series: String::new(),
            series_index: String::new(),
            year: None,
            ..m.clone()
        };
        assert_eq!(
            book_name("{series} #{volume} - {title} ({year})", &none, " - "),
            "Nina Torres e a Chave de Vidro"
        );
        assert_eq!(
            folders("{author}/{series}/{title}", &none, " - "),
            vec!["J. K. Brandão", "Nina Torres e a Chave de Vidro"]
        );
    }

    #[test]
    fn library() {
        let m = BookMatch {
            title: "Vento Leste".into(),
            authors: vec!["Tomé de Alvarenga".into()],
            ..Default::default()
        };
        let levels = vec!["Tomé de Alvarenga".to_string(), "Vento Leste".to_string()];
        let root = |d: &str| library_root(Path::new(d), &m, &levels);
        assert_eq!(root("/b/Tomé de Alvarenga/Vento Leste"), Path::new("/b"));
        assert_eq!(root("/b/Tomé de Alvarenga"), Path::new("/b"));
        assert_eq!(root("/b/Vento Leste - Tomé de Alvarenga"), Path::new("/b"));
        assert_eq!(root("/b/Livros"), Path::new("/b/Livros"));
    }

    #[test]
    fn opf_file() {
        let m = BookMatch {
            work: "OL1W".into(),
            title: "Vento Leste".into(),
            authors: vec!["Tomé de Alvarenga".into()],
            isbn: "9780000000026".into(),
            edition_year: Some(2002),
            ..Default::default()
        };
        let c = Candidate {
            language: "por".into(),
            ..Default::default()
        };
        let w: Work = serde_json::from_str(r#"{"description":{"type":"/type/text","value":"**Lúcio** e [Dora](https://x.org).\n----------\nVeja também"},"subjects":["Ficção"]}"#).unwrap();
        let o = opf(&m, &c, &w, Some("cover.jpg"));
        assert!(o.contains("<dc:language>pt</dc:language>"));
        assert!(o.contains("<dc:description>Lúcio e Dora.</dc:description>"));
        assert!(o.contains("opf:scheme=\"ISBN\">9780000000026<"));
        assert!(o.contains("href=\"cover.jpg\""));
    }
}
