//! List filter: decides which files are shown (and therefore which ones
//! will be renamed).
//!
//! Terms are separated by commas, semicolons or spaces; any one may match.
//! Terms with `*` or `?` are patterns over the whole name (`*.jpg`); the rest
//! look for the text anywhere in the name (`jpg`, `holiday`). `"two words"`
//! looks for the phrase, and a term starting with `-` leaves out the names
//! it matches (`-*.srt`, `-sample`). Case and accents are ignored. Other
//! characters (`[`, `{`…) are always literal.
//!
//! A term starting with `>`, `<`, `>=`, `<=` or `=` is a condition on the
//! file rather than on its name, and all of them must hold: a size with
//! its unit (`>700MB`, `<1.5GB`; B, KB, MB, GB, TB, of 1024) or a
//! modification date (`>=2024`, `<2024-06`, `=2024-05-12`, a year, month
//! or day, in this computer's time zone). `-` before one turns it round.
//! Folders have no size: a size condition leaves them out.

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

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Less,
    LessEq,
    Eq,
    MoreEq,
    More,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Cond {
    /// Bytes.
    Size(Op, u64),
    /// A period, in Unix seconds: from its start, up to its end.
    Date(Op, i64, i64),
}

impl Cond {
    /// `>700MB`, `<=2024-06`; `None` when the term is not a condition.
    fn parse(term: &str) -> Option<Result<Self, String>> {
        let (op, rest) = [
            (">=", Op::MoreEq),
            ("<=", Op::LessEq),
            (">", Op::More),
            ("<", Op::Less),
            ("=", Op::Eq),
        ]
        .into_iter()
        .find_map(|(sign, op)| term.strip_prefix(sign).map(|r| (op, r.trim())))?;
        let bad = || Err(format!("invalidFilter: {term}"));
        if rest.is_empty() {
            return Some(bad());
        }
        let upper = rest.to_ascii_uppercase();
        let digits = upper.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        let unit = &upper[digits.len()..];
        if !unit.is_empty() {
            let times: u64 = match unit {
                "B" => 1,
                "K" | "KB" | "KIB" => 1 << 10,
                "M" | "MB" | "MIB" => 1 << 20,
                "G" | "GB" | "GIB" => 1 << 30,
                "T" | "TB" | "TIB" => 1 << 40,
                _ => return Some(bad()),
            };
            let Ok(n) = digits.parse::<f64>() else {
                return Some(bad());
            };
            if !n.is_finite() || n < 0.0 {
                return Some(bad());
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
            let bytes = (n * times as f64).round() as u64;
            return Some(Ok(Cond::Size(op, bytes)));
        }
        Some(period(rest).map(|(a, b)| Cond::Date(op, a, b)).ok_or_else(|| format!("invalidFilter: {term}")))
    }

    fn holds(self, size: Option<u64>, modified: Option<i64>) -> bool {
        let cmp = |op: Op, v: i64, start: i64, end: i64| match op {
            Op::Less => v < start,
            Op::LessEq => v < end,
            Op::Eq => v >= start && v < end,
            Op::MoreEq => v >= start,
            Op::More => v >= end,
        };
        match self {
            Cond::Size(op, n) => size.is_some_and(|s| {
                let (s, n) = (i128::from(s), i128::from(n));
                match op {
                    Op::Less => s < n,
                    Op::LessEq => s <= n,
                    Op::Eq => s == n,
                    Op::MoreEq => s >= n,
                    Op::More => s > n,
                }
            }),
            Cond::Date(op, start, end) => modified.is_some_and(|m| cmp(op, m, start, end)),
        }
    }
}

/// `2024`, `2024-05`, `2024-05-12`: when that year, month or day starts
/// and ends here.
fn period(text: &str) -> Option<(i64, i64)> {
    use chrono::{Local, NaiveDate, TimeZone};
    let parts: Vec<&str> = text.split('-').collect();
    let num = |i: usize, len: std::ops::RangeInclusive<usize>| -> Option<u32> {
        let p = parts.get(i)?;
        (len.contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit())).then(|| p.parse().ok())?
    };
    let year = i32::try_from(num(0, 4..=4)?).ok()?;
    let (start, end) = match parts.len() {
        1 => (
            NaiveDate::from_ymd_opt(year, 1, 1)?,
            NaiveDate::from_ymd_opt(year + 1, 1, 1)?,
        ),
        2 => {
            let m = num(1, 1..=2)?;
            let start = NaiveDate::from_ymd_opt(year, m, 1)?;
            (start, start.checked_add_months(chrono::Months::new(1))?)
        }
        3 => {
            let start = NaiveDate::from_ymd_opt(year, num(1, 1..=2)?, num(2, 1..=2)?)?;
            (start, start.succ_opt()?)
        }
        _ => return None,
    };
    let at = |d: NaiveDate| {
        Local
            .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
            .earliest()
            .map(|t| t.timestamp())
    };
    Some((at(start)?, at(end)?))
}

#[derive(Default)]
pub struct Filter {
    include: Terms,
    exclude: Terms,
    /// Conditions on the file, and whether each is turned round.
    conds: Vec<(Cond, bool)>,
}

fn fold(s: &str) -> String {
    strip_accents(&s.to_lowercase())
}

/// For plain text: `O.Reino.das.Mares` has the words `reino das`.
fn as_words(folded: &str) -> String {
    folded.replace(['.', '_'], " ")
}

/// Splits at separators, keeping `"quoted phrases"` whole and a
/// condition written with spaces (`> 40 MB`, `>= 2024`) as one; returns
/// (term, quoted).
fn split(text: &str) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    for (term, quoted) in split_plain(text) {
        if let Some((last, false)) = out.last_mut()
            && !quoted
        {
            let bare = last.trim_start_matches('-');
            let sign = matches!(bare, ">" | "<" | ">=" | "<=" | "=");
            let unit = bare.starts_with(['>', '<', '='])
                && bare.ends_with(|c: char| c.is_ascii_digit())
                && matches!(
                    term.to_ascii_uppercase().as_str(),
                    "B" | "K" | "KB" | "KIB" | "M" | "MB" | "MIB" | "G" | "GB" | "GIB" | "T" | "TB" | "TIB"
                );
            if sign || unit {
                last.push_str(&term);
                continue;
            }
        }
        out.push((term, quoted));
    }
    out
}

fn split_plain(text: &str) -> Vec<(String, bool)> {
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
        let mut conds = Vec::new();
        for (term, quoted) in split(text) {
            let (term, exclude) = match term.strip_prefix('-') {
                Some(rest) if !rest.is_empty() => (rest.to_string(), true),
                _ => (term, false),
            };
            if !quoted && let Some(cond) = Cond::parse(&term) {
                conds.push((cond?, exclude));
                continue;
            }
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
            conds,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.include.is_empty() && self.exclude.is_empty() && self.conds.is_empty()
    }

    /// Whether the file is taken: its name, size and date.
    pub fn takes(&self, file: &crate::files::FileEntry) -> bool {
        let size = (!file.is_dir).then_some(file.size);
        self.conds
            .iter()
            .all(|&(c, turned)| c.holds(size, file.modified) != turned)
            && self.matches(&file.file_name())
    }

    /// Whether a name is taken, the conditions on the file aside.
    pub fn matches(&self, file_name: &str) -> bool {
        if self.include.is_empty() && self.exclude.is_empty() {
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

    fn entry(name: &str, size: u64, modified: Option<i64>) -> crate::files::FileEntry {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(name);
        std::fs::write(&p, b"").unwrap();
        let mut f = crate::files::FileEntry::from_path(&p, &std::fs::metadata(&p).unwrap()).unwrap();
        f.size = size;
        f.modified = modified;
        f
    }

    #[test]
    fn sizes_and_dates() {
        use chrono::TimeZone;
        let at = |y, m, d| chrono::Local.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap().timestamp();
        let big = entry("Filme.mkv", 2 << 30, Some(at(2024, 5, 12)));
        let sample = entry("Filme-sample.mkv", 30 << 20, Some(at(2023, 12, 31)));
        let take = |text: &str, f: &crate::files::FileEntry| Filter::new(text).unwrap().takes(f);
        assert!(take(">700MB", &big) && !take(">700MB", &sample));
        assert!(take("<100mb", &sample) && !take("<100MB", &big));
        assert!(take(">=2gb", &big) && !take(">2GB", &big));
        assert!(take("<1.5G", &sample) && !take("<1.5G", &big));
        // Every condition holds, and they hold together with the name.
        assert!(take("*.mkv >1GB", &big) && !take("*.mkv >1GB", &sample));
        assert!(!take("*.srt >1GB", &big));
        assert!(take("-<100MB", &big) && !take("-<100MB", &sample));
        // Dates: a year, month or day.
        assert!(take(">=2024", &big) && !take(">=2024", &sample));
        assert!(take("<2024", &sample) && !take("<2024", &big));
        assert!(take("=2024-05", &big) && !take("=2024-06", &big));
        assert!(take("=2024-05-12", &big) && !take(">2024-05-12", &big));
        assert!(take("<=2024-05-12", &big) && take(">2024-05-11", &big));
        assert!(take(">=2024-01-01 <2024-07", &big));
        // With spaces.
        assert!(take("> 1.5 GB", &big) && !take("> 1.5 GB", &sample));
        assert!(take(">= 2024", &big) && take("mkv, -< 100 MB", &big));
        assert!(take(">40mb", &big) && take("> 40mb", &big) && take(">40 MB", &big));
        assert!(!take(">2020", &entry("a.txt", 1, None)));
        // Folders have no size.
        let mut dir = entry("Pasta", 0, Some(at(2024, 1, 1)));
        dir.is_dir = true;
        assert!(!take("<1MB", &dir) && take("=2024", &dir));
        // Quoted, it is text in the name.
        assert!(Filter::new("\">2\"").unwrap().matches("a >2 b"));
        for bad in [">", ">10XB", ">abc", ">2024-13", ">20-1", "<-5MB"] {
            assert!(Filter::new(bad).is_err(), "{bad}");
        }
    }
}
