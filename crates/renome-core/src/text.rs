//! Unicode-aware text helpers. Positions are counted in graphemes (what the
//! user sees as one character).

use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
use unicode_segmentation::UnicodeSegmentation;

pub fn graphemes(s: &str) -> Vec<&str> {
    s.graphemes(true).collect()
}

/// The first `max` visible characters, without spaces, dashes, dots or
/// commas left at the end.
pub fn cut(s: &str, max: usize) -> String {
    let g = graphemes(s);
    if g.len() <= max {
        return s.to_string();
    }
    g[..max]
        .concat()
        .trim_end_matches(|c: char| c.is_whitespace() || "-–—_.,;".contains(c))
        .to_string()
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

/// Latin letters that are not a plain letter with an accent, so taking
/// accents away leaves them as they are.
fn plain_letter(c: char) -> Option<&'static str> {
    Some(match c {
        'ß' => "ss",
        'ẞ' => "SS",
        'æ' => "ae",
        'Æ' => "Ae",
        'œ' => "oe",
        'Œ' => "Oe",
        'ø' => "o",
        'Ø' => "O",
        'ł' => "l",
        'Ł' => "L",
        'đ' => "d",
        'Đ' => "D",
        'ð' => "d",
        'Ð' => "D",
        'þ' => "th",
        'Þ' => "Th",
        'ı' => "i",
        'ħ' => "h",
        'Ħ' => "H",
        _ => return None,
    })
}

/// Letters without accents, also those written apart from their base
/// letter (`ø` → `o`, `ß` → `ss`, `æ` → `ae`).
pub fn plain_letters(s: &str) -> String {
    strip_accents(s).chars().fold(String::new(), |mut out, c| {
        match plain_letter(c) {
            Some(p) => out.push_str(p),
            None => out.push(c),
        }
        out
    })
}

/// A Cyrillic or Greek letter in Latin letters (lower case).
fn latin_of(c: char) -> Option<&'static str> {
    Some(match c {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'ґ' => "g",
        'д' => "d",
        'е' => "e",
        'ё' => "yo",
        'є' => "ye",
        'ж' => "zh",
        'з' => "z",
        'и' => "i",
        'і' => "i",
        'ї' => "yi",
        'й' => "y",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ў' => "u",
        'ф' => "f",
        'х' => "kh",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'ъ' | 'ь' => "",
        'ы' => "y",
        'э' => "e",
        'ю' => "yu",
        'я' => "ya",
        'ђ' => "dj",
        'ј' => "j",
        'љ' => "lj",
        'њ' => "nj",
        'ћ' => "c",
        'џ' => "dz",
        'ѓ' => "gj",
        'ќ' => "kj",
        'ѕ' => "dz",
        'α' => "a",
        'β' => "v",
        'γ' => "g",
        'δ' => "d",
        'ε' => "e",
        'ζ' => "z",
        'η' => "i",
        'θ' => "th",
        'ι' => "i",
        'κ' => "k",
        'λ' => "l",
        'μ' => "m",
        'ν' => "n",
        'ξ' => "x",
        'ο' => "o",
        'π' => "p",
        'ρ' => "r",
        'σ' | 'ς' => "s",
        'τ' => "t",
        'υ' => "y",
        'φ' => "f",
        'χ' => "ch",
        'ψ' => "ps",
        'ω' => "o",
        _ => return None,
    })
}

/// Cyrillic and Greek written in Latin letters (`Кино` → `Kino`,
/// `Ελλάδα` → `Ellada`); a capital stays one, and a whole word in
/// capitals stays so (`ЩИ` → `SHCHI`). Other text is kept.
pub fn to_latin(s: &str) -> String {
    // A Greek letter without its accent (`ά` → `α`); Cyrillic `й` and `ё`
    // are letters of their own.
    let base = |c: char| {
        let greek = ('\u{370}'..='\u{3ff}').contains(&c) || ('\u{1f00}'..='\u{1fff}').contains(&c);
        if greek {
            c.nfd().next().unwrap_or(c)
        } else {
            c
        }
    };
    let chars: Vec<char> = s.chars().map(base).collect();
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // `ου` is one sound.
        let (latin, used) = if lower(c) == 'ο' && chars.get(i + 1).is_some_and(|&n| lower(n) == 'υ')
        {
            (Some("ou"), 2)
        } else {
            (latin_of(lower(c)), 1)
        };
        let Some(latin) = latin else {
            out.push(c);
            i += 1;
            continue;
        };
        if !c.is_uppercase() {
            out.push_str(latin);
        } else {
            let after = chars.get(i + used).copied();
            let before = i.checked_sub(1).map(|b| chars[b]);
            let caps = after.is_some_and(char::is_uppercase)
                || (before.is_some_and(char::is_uppercase)
                    && !after.is_some_and(char::is_lowercase));
            if caps {
                out.push_str(&latin.to_uppercase());
            } else {
                let mut cs = latin.chars();
                if let Some(f) = cs.next() {
                    out.extend(f.to_uppercase());
                    out.push_str(cs.as_str());
                }
            }
        }
        i += used;
    }
    out
}

/// A name that any server or old program takes: plain Latin letters
/// (see [`to_latin`] and [`plain_letters`]), digits and dots; the spaces,
/// dashes and signs between them become one `sep`.
pub fn web_safe(s: &str, sep: &str) -> String {
    plain_letters(&to_latin(s))
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(sep)
}

/// The characters Windows-1252 has at bytes 0x80–0x9F (the five it leaves
/// out read as the control characters with those codes).
const CP1252_HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

/// The byte a character comes from when bytes are read as Windows-1252 or
/// Latin-1.
fn single_byte(c: char) -> Option<u8> {
    match c as u32 {
        n @ 0..=0xFF => Some(n as u8),
        _ => CP1252_HIGH
            .iter()
            .position(|&x| x == c)
            .map(|i| 0x80 + i as u8),
    }
}

/// Text written in UTF-8 but read as Windows-1252 or Latin-1 put back:
/// `SÃ£o JoÃ£o` → `São João`, `donâ€™t` → `don’t`. Each run of non-ASCII
/// characters is turned back into the bytes it was read from, and kept as
/// it is unless those bytes are UTF-8 (`São`, read right, stays). Text
/// broken twice is fixed twice.
pub fn fix_mojibake(s: &str) -> String {
    fn once(s: &str) -> String {
        let mut out = String::new();
        let mut run = String::new();
        let flush = |run: &mut String, out: &mut String| {
            let bytes: Option<Vec<u8>> = run.chars().map(single_byte).collect();
            match bytes.and_then(|b| String::from_utf8(b).ok()) {
                Some(fixed) => out.push_str(&fixed),
                None => out.push_str(run),
            }
            run.clear();
        };
        for c in s.chars() {
            if c.is_ascii() {
                flush(&mut run, &mut out);
                out.push(c);
            } else {
                run.push(c);
            }
        }
        flush(&mut run, &mut out);
        out
    }
    let mut s = s.to_string();
    for _ in 0..3 {
        let fixed = once(&s);
        if fixed == s {
            break;
        }
        s = fixed;
    }
    nfc(&s)
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
    fn other_letters() {
        assert_eq!(
            plain_letters("Ørsted Straße Æsir Łódź"),
            "Orsted Strasse Aesir Lodz"
        );
        assert_eq!(to_latin("Кино - Группа крови"), "Kino - Gruppa krovi");
        assert_eq!(
            to_latin("Щедрик ЩИ Йогурт Їжак"),
            "Shchedrik SHCHI Yogurt Yizhak"
        );
        assert_eq!(to_latin("Ελλάδα Μουσική"), "Ellada Mousiki");
        assert_eq!(to_latin("São 東京"), "São 東京");
        assert_eq!(
            web_safe("Canção & Coração (Ao Vivo) - Кино v1.2", "-"),
            "Cancao-Coracao-Ao-Vivo-Kino-v1.2"
        );
        assert_eq!(web_safe("  Minha foto_01 ", "_"), "Minha_foto_01");
    }

    #[test]
    fn mojibake() {
        assert_eq!(fix_mojibake("SÃ£o JoÃ£o - CanÃ§Ã£o"), "São João - Canção");
        assert_eq!(fix_mojibake("donâ€™t stop"), "don’t stop");
        // Read as Latin-1: the middle byte is a control character.
        assert_eq!(fix_mojibake("don\u{e2}\u{80}\u{99}t"), "don’t");
        // Broken twice.
        assert_eq!(fix_mojibake("SÃƒÂ£o"), "São");
        // Right text stays, as do signs that are not UTF-8 read wrongly.
        for ok in ["São João", "Ação é ótima", "Größe ½ ¿Qué?", "東京", "Кино"] {
            assert_eq!(fix_mojibake(ok), ok);
        }
    }

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
