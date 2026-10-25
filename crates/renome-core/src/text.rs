//! Unicode-aware text helpers. Positions are counted in graphemes (what the
//! user sees as one character).

use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
use unicode_segmentation::UnicodeSegmentation;

pub fn graphemes(s: &str) -> Vec<&str> {
    s.graphemes(true).collect()
}

/// Grapheme index matching a byte offset.
pub fn grapheme_index(s: &str, byte: usize) -> usize {
    s.grapheme_indices(true)
        .take_while(|(i, _)| *i < byte)
        .count()
}

/// Accented letters as one character each (`á`, not `a` and an accent),
/// as names are usually written: book details sometimes keep them apart,
/// and the two forms look the same but are different names.
pub fn nfc(s: &str) -> String {
    s.nfc().collect()
}

pub fn strip_accents(s: &str) -> String {
    s.nfd().filter(|c| !is_combining_mark(*c)).nfc().collect()
}

/// Trims spaces at both ends and collapses runs of spaces into one.
pub fn collapse_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Short words kept in lowercase by the "Title Case except connectives"
/// mode (unless they start the name).
#[rustfmt::skip]
const CONNECTIVES: &[&str] = &[
    // Portuguese
    "a", "à", "ao", "aos", "as", "às", "com", "da", "das", "de", "do", "dos", "e", "em", "na",
    "nas", "no", "nos", "o", "os", "ou", "para", "pela", "pelas", "pelo", "pelos", "por", "sem",
    "sob", "um", "uma", "umas", "uns",
    // English
    "an", "and", "at", "but", "by", "for", "from", "in", "nor", "of", "on", "or", "the", "to",
    "vs", "with",
    // Spanish
    "con", "del", "el", "la", "las", "los", "y",
];

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || c == '’'
}

/// Splits into alternating word / separator runs, keeping everything.
fn tokens(s: &str) -> Vec<(bool, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut cur: Option<bool> = None;
    for (i, c) in s.char_indices() {
        let w = is_word_char(c);
        if cur.is_some_and(|k| k != w) {
            out.push((cur.unwrap(), &s[start..i]));
            start = i;
        }
        cur = Some(w);
    }
    if let Some(k) = cur {
        out.push((k, &s[start..]));
    }
    out
}

fn capitalize(word: &str) -> String {
    let mut g = word.graphemes(true);
    match g.next() {
        Some(first) => first.to_uppercase() + &g.as_str().to_lowercase(),
        None => String::new(),
    }
}

pub fn sentence_case(s: &str) -> String {
    capitalize(s)
}

/// A Roman numeral from II to XXXIX (`IV`, `xii`), but not the words
/// "vi" and "xi" unless written in capitals.
fn is_roman(word: &str) -> bool {
    static ROMAN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^X{0,3}(?:IX|IV|VI{0,3}|I{0,3})$").unwrap());
    let upper = word.to_uppercase();
    let word_like = matches!(word, "vi" | "xi" | "Vi" | "Xi");
    word.len() > 1 && ROMAN.is_match(&upper) && !word_like
}

/// Words with capitals inside on purpose: `iPhone`, `McDonald`, `YouTube`.
fn is_mixed_case(word: &str) -> bool {
    let mut chars = word.chars();
    let first_upper = chars.next().is_some_and(char::is_uppercase);
    let rest: Vec<char> = chars.collect();
    let upper_inside = rest.iter().any(|c| c.is_uppercase());
    let lower_any = word.chars().any(char::is_lowercase);
    upper_inside && lower_any && (first_upper || word.starts_with(char::is_lowercase))
}

pub fn title_case(s: &str, keep_connectives: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut first_word = true;
    for (is_word, tok) in tokens(s) {
        if !is_word {
            out.push_str(tok);
            // A new part starts after " - ", ":", "(" and the like.
            if tok.contains([':', '(', '[', '.', '!', '?', '–', '—'])
                || (tok.contains('-') && tok.contains(char::is_whitespace))
            {
                first_word = true;
            }
            continue;
        }
        let lower = tok.to_lowercase();
        if is_roman(tok) {
            out.push_str(&tok.to_uppercase());
        } else if is_mixed_case(tok) {
            out.push_str(tok);
        } else if keep_connectives && !first_word && CONNECTIVES.contains(&lower.as_str()) {
            out.push_str(&lower);
        } else {
            out.push_str(&capitalize(tok));
        }
        first_word = false;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accents() {
        assert_eq!(strip_accents("Ação Pão Über Ñandú"), "Acao Pao Uber Nandu");
    }

    #[test]
    fn titles() {
        assert_eq!(
            title_case("o SENHOR dos anéis", false),
            "O Senhor Dos Anéis"
        );
        assert_eq!(title_case("o SENHOR dos anéis", true), "O Senhor dos Anéis");
        assert_eq!(
            title_case("rock_and-roll (live)", true),
            "Rock_and-Roll (Live)"
        );
        assert_eq!(title_case("don't stop", false), "Don't Stop");
        assert_eq!(sentence_case("éRA uma VEZ"), "Éra uma vez");
        assert_eq!(
            title_case("o senhor dos anéis - a sociedade do anel", true),
            "O Senhor dos Anéis - A Sociedade do Anel"
        );
        assert_eq!(
            title_case("STAR WARS IV: a new hope", true),
            "Star Wars IV: A New Hope"
        );
        assert_eq!(
            title_case("meu iPhone no McDonald's", true),
            "Meu iPhone no McDonald's"
        );
        assert_eq!(title_case("LIVRO VIVO", false), "Livro Vivo");
        assert_eq!(title_case("parte ii, eu vi", true), "Parte II, Eu Vi");
    }

    #[test]
    fn spaces() {
        assert_eq!(collapse_spaces("  a   b \t c "), "a b c");
    }
}
