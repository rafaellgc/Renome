//! List filter: decides which files are shown (and therefore which ones
//! will be renamed).
//!
//! Terms are separated by commas, semicolons or spaces; any one may match.
//! Terms with `*` or `?` are patterns over the whole name (`*.jpg`); the rest
//! look for the text anywhere in the name (`jpg`, `holiday`). `"two words"`
//! looks for the phrase, and a term starting with `-` leaves out the names
//! it matches (`-*.srt`, `-sample`). Case and accents are ignored. Other
//! characters (`[`, `{`…) are always literal.

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::text::strip_accents;

/// Terms of one kind: patterns and plain text.
#[derive(Default)]
struct Terms {
    globs: Option<GlobSet>,
    text: Vec<String>,
}

impl Terms {
    fn build(globs: Vec<String>, text: Vec<String>) -> Result<Self, String> {
        let globs = if globs.is_empty() {
            None
        } else {
            let mut set = GlobSetBuilder::new();
            for g in globs {
                set.add(
                    GlobBuilder::new(&literal_except_wildcards(&g))
                        .case_insensitive(true)
                        .literal_separator(true)
                        .build()
                        .map_err(|e| format!("invalidFilter: {e}"))?,
                );
            }
            Some(set.build().map_err(|e| format!("invalidFilter: {e}"))?)
        };
        Ok(Self { globs, text })
    }

    fn is_empty(&self) -> bool {
        self.globs.is_none() && self.text.is_empty()
    }

    /// `name` is already folded: lowercase, no accents; `words` also has
    /// dots and underscores as spaces.
    fn matches(&self, name: &str, words: &str) -> bool {
        self.globs.as_ref().is_some_and(|g| g.is_match(name))
            || self.text.iter().any(|t| words.contains(t))
    }
}

#[derive(Default)]
pub struct Filter {
    include: Terms,
    exclude: Terms,
}

fn fold(s: &str) -> String {
    strip_accents(&s.to_lowercase())
}

/// For plain text: `O.Senhor.dos.Aneis` has the words `senhor dos`.
fn as_words(folded: &str) -> String {
    folded.replace(['.', '_'], " ")
}

/// Splits at separators, keeping `"quoted phrases"` whole; returns
/// (term, quoted).
fn split(text: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut was_quoted = false;
    for c in text.chars() {
        if c == '"' {
            quoted = !quoted;
            was_quoted = true;
        } else if !quoted && (c == ',' || c == ';' || c.is_whitespace()) {
            if !cur.is_empty() {
                out.push((std::mem::take(&mut cur), was_quoted));
            }
            was_quoted = false;
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push((cur, was_quoted));
    }
    out
}

impl Filter {
    /// Error: `invalidFilter` followed by the detail.
    pub fn new(text: &str) -> Result<Self, String> {
        let (mut inc_globs, mut inc_text) = (Vec::new(), Vec::new());
        let (mut exc_globs, mut exc_text) = (Vec::new(), Vec::new());
        for (term, quoted) in split(text) {
            let (term, exclude) = match term.strip_prefix('-') {
                Some(rest) if !rest.is_empty() => (rest.to_string(), true),
                _ => (term, false),
            };
            let term = fold(&term);
            let (globs, plain) = if exclude {
                (&mut exc_globs, &mut exc_text)
            } else {
                (&mut inc_globs, &mut inc_text)
            };
            if !quoted && term.contains(['*', '?']) {
                globs.push(term);
            } else {
                plain.push(as_words(&term));
            }
        }
        Ok(Self {
            include: Terms::build(inc_globs, inc_text)?,
            exclude: Terms::build(exc_globs, exc_text)?,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.include.is_empty() && self.exclude.is_empty()
    }

    pub fn matches(&self, file_name: &str) -> bool {
        if self.is_empty() {
            return true;
        }
        let name = fold(file_name);
        let words = as_words(&name);
        !self.exclude.matches(&name, &words)
            && (self.include.is_empty() || self.include.matches(&name, &words))
    }
}

/// Escapes everything except `*` and `?`, so `[`, `{` etc. are literal.
fn literal_except_wildcards(term: &str) -> String {
    let mut out = String::with_capacity(term.len());
    let mut rest = term;
    while let Some(i) = rest.find(['*', '?']) {
        out.push_str(&globset::escape(&rest[..i]));
        out.push_str(&rest[i..=i]);
        rest = &rest[i + 1..];
    }
    out.push_str(&globset::escape(rest));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_and_globs() {
        let f = Filter::new("jpg, *.PNG").unwrap();
        assert!(f.matches("Foto.JPG"));
        assert!(f.matches("a.png"));
        assert!(!f.matches("a.png.txt"));
        assert!(!f.matches("doc.txt"));

        let f = Filter::new("férias").unwrap();
        assert!(f.matches("Fotos FÉRIAS 2024.jpg"));

        assert!(Filter::new("  ").unwrap().matches("qualquer"));

        // Brackets and braces are literal, with or without wildcards.
        let f = Filter::new("[").unwrap();
        assert!(f.matches("Filme [2020].mkv"));
        assert!(!f.matches("Filme 2020.mkv"));
        let f = Filter::new("*[2020]*").unwrap();
        assert!(f.matches("Filme [2020].mkv"));
        assert!(!f.matches("Filme 2.mkv"));
        assert!(Filter::new("{a,b}*").unwrap().matches("{a,b}.txt"));
    }

    #[test]
    fn accents_phrases_and_exclusions() {
        assert!(Filter::new("ferias").unwrap().matches("Fotos Férias.jpg"));
        assert!(Filter::new("*FÉRIAS*").unwrap().matches("fotos ferias.jpg"));

        let f = Filter::new("\"breaking bad\"").unwrap();
        assert!(f.matches("Breaking Bad S01E01.mkv"));
        assert!(!f.matches("Bad Boys.mkv"));
        assert!(f.matches("Breaking.Bad.S01E01.mkv"));
        assert!(Filter::new(".jpg").unwrap().matches("foto.JPG"));

        let f = Filter::new("-*.srt -sample").unwrap();
        assert!(f.matches("Filme.mkv"));
        assert!(!f.matches("Filme.pt.srt"));
        assert!(!f.matches("Filme-SAMPLE.mkv"));

        let f = Filter::new("*.mkv, -sample").unwrap();
        assert!(f.matches("a.mkv"));
        assert!(!f.matches("a.sample.mkv"));
        assert!(!f.matches("a.txt"));

        // A lone dash is a term, not an exclusion.
        assert!(Filter::new("-").unwrap().matches("a - b"));
    }
}
