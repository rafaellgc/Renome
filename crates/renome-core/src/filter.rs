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
//! or day, in this computer's time zone), or how long ago it was
//! modified (`<7d`, `>3mes`, `>1a`: days, weeks, months, years, named in
//! Portuguese, English or Spanish). `-` before one turns it round.
//! Folders have no size: a size condition leaves them out.
//!
//! `pasta:` (`folder:`, `carpeta:`) before a term looks in the names of
//! the folders the file is in, from the one added down: `pasta:"Temporada
//! 2"`, `-pasta:extras`, `pasta:S0?`. One of them must match.

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
    /// How long ago it was modified, as the moment that long ago (Unix
    /// seconds): `<7d` is newer than it.
    Age(Op, i64),
}

/// Bytes in a unit of size; `None` for anything else.
fn size_unit(unit: &str) -> Option<u64> {
    Some(match unit {
        "B" => 1,
        "KB" | "KIB" => 1 << 10,
        "MB" | "MIB" => 1 << 20,
        "GB" | "GIB" => 1 << 30,
        "TB" | "TIB" => 1 << 40,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Span {
    Days,
    Weeks,
    Months,
    Years,
}

/// A unit of time, in Portuguese, English or Spanish.
fn time_unit(unit: &str) -> Option<Span> {
    Some(match unit {
        "D" | "DIA" | "DIAS" | "DAY" | "DAYS" | "DÍA" | "DÍAS" => Span::Days,
        "W" | "S" | "SEM" | "SEMANA" | "SEMANAS" | "WEEK" | "WEEKS" => Span::Weeks,
        "MES" | "MÊS" | "MESES" | "MO" | "MONTH" | "MONTHS" => Span::Months,
        "A" | "ANO" | "ANOS" | "AÑO" | "AÑOS" | "Y" | "YEAR" | "YEARS" => Span::Years,
        _ => return None,
    })
}

fn is_unit(word: &str) -> bool {
    let up = word.to_uppercase();
    size_unit(&up).is_some() || time_unit(&up).is_some()
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
        let upper = rest.to_uppercase();
        let digits = upper.trim_end_matches(char::is_alphabetic);
        let unit = &upper[digits.len()..];
        if let Some(span) = time_unit(unit) {
            let Ok(n) = digits.parse::<u32>() else {
                return Some(bad());
            };
            if op == Op::Eq {
                return Some(bad());
            }
            let now = chrono::Local::now();
            let then = match span {
                Span::Days => now.checked_sub_signed(chrono::TimeDelta::days(i64::from(n))),
                Span::Weeks => now.checked_sub_signed(chrono::TimeDelta::weeks(i64::from(n))),
                Span::Months => now.checked_sub_months(chrono::Months::new(n)),
                Span::Years => n
                    .checked_mul(12)
                    .and_then(|m| now.checked_sub_months(chrono::Months::new(m))),
            };
            return Some(then.map(|t| Cond::Age(op, t.timestamp())).ok_or_else(|| format!("invalidFilter: {term}")));
        }
        if !unit.is_empty() {
            let Some(times) = size_unit(unit) else {
                return Some(bad());
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
            // Younger than the time: modified after the moment.
            Cond::Age(op, then) => modified.is_some_and(|m| match op {
                Op::Less => m > then,
                Op::LessEq => m >= then,
                Op::MoreEq => m <= then,
                Op::More | Op::Eq => m < then,
            }),
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

/// `pasta:Temporada 2`: text in the folders' names, or a pattern one of
/// them matches (`pasta:S0?`).
struct FolderTerm {
    glob: Option<globset::GlobMatcher>,
    text: String,
}

impl FolderTerm {
    fn new(text: &str) -> Result<Self, String> {
        let text = fold(text.trim());
        if text.is_empty() {
            return Err("invalidFilter: pasta:".into());
        }
        let glob = if text.contains(['*', '?']) {
            Some(
                GlobBuilder::new(&literal_except_wildcards(&text))
                    .case_insensitive(true)
                    .literal_separator(true)
                    .build()
                    .map_err(|e| format!("invalidFilter: {e}"))?
                    .compile_matcher(),
            )
        } else {
            None
        };
        Ok(Self { glob, text })
    }

    /// `folders`: the folders' names, folded, from the one added down.
    fn matches(&self, folders: &[String]) -> bool {
        match &self.glob {
            Some(g) => folders.iter().any(|f| g.is_match(f)),
            None => folders.join("/").contains(&self.text),
        }
    }
}

/// `pasta:`, `folder:`, `carpeta:` before a term: what it says is about
/// the folders.
fn folder_term(term: &str) -> Option<&str> {
    let (key, rest) = term.split_once(':')?;
    matches!(key.to_lowercase().as_str(), "pasta" | "folder" | "carpeta").then_some(rest)
}

/// The names of the folders a file is in, from the one it was added with
/// (a folder added by itself: its own) down, folded.
fn folders_of(file: &crate::files::FileEntry) -> Vec<String> {
    let top = file.base.parent().unwrap_or(&file.base);
    let rel = file.dir.strip_prefix(top).ok().filter(|r| !r.as_os_str().is_empty());
    match rel {
        Some(rel) => rel
            .components()
            .map(|c| fold(&c.as_os_str().to_string_lossy()))
            .collect(),
        None => file
            .dir
            .file_name()
            .map(|n| vec![fold(&n.to_string_lossy())])
            .unwrap_or_default(),
    }
}

#[derive(Default)]
pub struct Filter {
    include: Terms,
    exclude: Terms,
    /// Conditions on the file, and whether each is turned round.
    conds: Vec<(Cond, bool)>,
    /// Folders: one of these, and none of those.
    folders: Vec<FolderTerm>,
    no_folders: Vec<FolderTerm>,
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
                && is_unit(&term);
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
        let (mut folders, mut no_folders) = (Vec::new(), Vec::new());
        for (term, quoted) in split(text) {
            let (term, exclude) = match term.strip_prefix('-') {
                Some(rest) if !rest.is_empty() => (rest.to_string(), true),
                _ => (term, false),
            };
            if let Some(folder) = folder_term(&term) {
                let t = FolderTerm::new(folder)?;
                if exclude { no_folders.push(t) } else { folders.push(t) }
                continue;
            }
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
            folders,
            no_folders,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.include.is_empty()
            && self.exclude.is_empty()
            && self.conds.is_empty()
            && self.folders.is_empty()
            && self.no_folders.is_empty()
    }

    /// Whether the file is taken: its name, folders, size and date.
    pub fn takes(&self, file: &crate::files::FileEntry) -> bool {
        let size = (!file.is_dir).then_some(file.size);
        if !self
            .conds
            .iter()
            .all(|&(c, turned)| c.holds(size, file.modified) != turned)
        {
            return false;
        }
        if !self.folders.is_empty() || !self.no_folders.is_empty() {
            let names = folders_of(file);
            if !self.no_folders.iter().all(|t| !t.matches(&names))
                || !(self.folders.is_empty() || self.folders.iter().any(|t| t.matches(&names)))
            {
                return false;
            }
        }
        self.matches(&file.file_name())
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

    #[test]
    fn folders() {
        let at = |dir: &str, name: &str| {
            let mut f = entry(name, 1, None);
            f.base = "/midia/Sonic".into();
            f.dir = std::path::PathBuf::from(dir);
            f
        };
        let s1 = at("/midia/Sonic/Temporada 1", "a.mkv");
        let s2 = at("/midia/Sonic/Temporada 2", "b.mkv");
        let extra = at("/midia/Sonic/Temporada 2/Extras", "c.mkv");
        let top = at("/midia/Sonic", "d.mkv");
        let take = |text: &str, f: &crate::files::FileEntry| Filter::new(text).unwrap().takes(f);
        assert!(take("pasta:\"temporada 2\"", &s2) && !take("pasta:\"Temporada 2\"", &s1));
        assert!(take("pasta:\"Temporada 2\"", &extra));
        assert!(take("\"pasta:Temporada 2\" -pasta:extras", &s2));
        assert!(!take("pasta:temporada -pasta:extras", &extra));
        assert!(take("pasta:sonic", &top) && take("folder:SONIC", &s1));
        // The folders above the one added are not looked at.
        assert!(!take("pasta:midia", &s1));
        assert!(take("carpeta:temporada*", &s1) && !take("pasta:temporada*", &top));
        assert!(take("pasta:temporada mkv", &s1) && !take("pasta:temporada srt", &s1));
        assert!(take("pasta:\"Temporada 1\" pasta:\"Temporada 2\"", &s1));
        assert!(Filter::new("pasta:").is_err());
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
        assert!(take("<1.5GB", &sample) && !take("<1.5GB", &big));
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
        // How long ago.
        let now = chrono::Local::now().timestamp();
        let new = entry("Novo.mkv", 1, Some(now - 3 * 86_400));
        let old = entry("Velho.mkv", 1, Some(now - 400 * 86_400));
        assert!(take("<7d", &new) && !take("<7d", &old));
        assert!(take(">1a", &old) && !take(">1 ano", &new));
        assert!(take("> 2 semanas", &old) && take("<1w", &new));
        assert!(take(">3mes", &old) && take("<1 month", &new) && take("<4días", &new) && !take("<2 días", &new));
        assert!(take("-<7d", &old));
        for bad in [">", ">10XB", ">abc", ">2024-13", ">20-1", "<-5MB", "=7d", ">3m", ">40M"] {
            assert!(Filter::new(bad).is_err(), "{bad}");
        }
    }
}
