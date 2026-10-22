//! List filter: decides which files are shown (and therefore which ones
//! will be renamed).
//!
//! Terms are separated by commas, semicolons or spaces; any one may match.
//! Terms with `*` or `?` are patterns over the whole name (`*.jpg`); the rest
//! look for the text anywhere in the name, case-insensitively (`jpg`,
//! `holiday`). Other characters (`[`, `{`…) are always literal.

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

#[derive(Default)]
pub struct Filter {
    globs: Option<GlobSet>,
    terms: Vec<String>,
}

impl Filter {
    /// Error: `invalidFilter` followed by the detail.
    pub fn new(text: &str) -> Result<Self, String> {
        let mut globs = GlobSetBuilder::new();
        let mut has_glob = false;
        let mut terms = Vec::new();
        for part in text
            .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .filter(|p| !p.is_empty())
        {
            if part.contains(['*', '?']) {
                let g = GlobBuilder::new(&literal_except_wildcards(part))
                    .case_insensitive(true)
                    .literal_separator(true)
                    .build()
                    .map_err(|e| format!("invalidFilter: {e}"))?;
                globs.add(g);
                has_glob = true;
            } else {
                terms.push(part.to_lowercase());
            }
        }
        let globs = if has_glob {
            Some(globs.build().map_err(|e| format!("invalidFilter: {e}"))?)
        } else {
            None
        };
        Ok(Self { globs, terms })
    }

    pub fn is_empty(&self) -> bool {
        self.globs.is_none() && self.terms.is_empty()
    }

    pub fn matches(&self, file_name: &str) -> bool {
        if self.is_empty() {
            return true;
        }
        if self.globs.as_ref().is_some_and(|g| g.is_match(file_name)) {
            return true;
        }
        let lower = file_name.to_lowercase();
        self.terms.iter().any(|t| lower.contains(t))
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
}
