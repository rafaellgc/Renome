//! Template fields beyond each rule's own: what the catalogues or the
//! files tell besides the name (a movie's genre, an episode's air date, a
//! comic's colorist), kept per match as a map of field → value, and the
//! names (English, Portuguese, Spanish) each field goes by in templates.
//!
//! A date (`YYYY-MM-DD`, or part of one) gives `{date}`, `{month}` (two
//! digits), `{day}` and the month's name, in the language of the key:
//! `{month name}` (May), `{nome do mês}` (maio), `{nombre del mes}` (mayo).

use std::collections::BTreeMap;

/// Field → value, by the field's English name.
pub type Extra = BTreeMap<String, String>;

/// Each field and the names it goes by (the first is its own).
const NAMES: &[&[&str]] = &[
    &["date", "data", "fecha"],
    &["month", "mês", "mes"],
    &["day", "dia", "día"],
    &["month name"],
    &["month name pt", "nome do mês", "nome do mes"],
    &["month name es", "nombre del mes"],
    &["genre", "gênero", "genero", "género"],
    &["genres", "gêneros", "generos", "géneros"],
    &["country", "país", "pais"],
    &["language", "idioma", "língua", "lingua"],
    &["runtime", "duração", "duracao", "duración", "duracion"],
    &[
        "certification",
        "classificação",
        "classificacao",
        "clasificación",
        "clasificacion",
    ],
    &["director", "diretor", "director"],
    &["studio", "estúdio", "estudio"],
    &["collection", "coleção", "colecao", "colección", "coleccion"],
    &["network", "emissora", "cadena"],
    &["absolute", "absoluto"],
    &["edition", "edição", "edicao", "edición", "edicion"],
    &["source", "origem", "fuente"],
    &["release group", "grupo", "grupo de lançamento"],
    &["pages", "páginas", "paginas"],
    &["translator", "tradutor", "traductor"],
    &["story arc", "arco"],
    &["imprint", "selo", "sello"],
    &["format", "formato"],
    &["inker", "arte-final", "entintador"],
    &["colorist", "colorista"],
    &["letterer", "letrista", "rotulista"],
    &["cover artist", "capista", "portadista"],
    &["editor"],
    &["age rating", "faixa etária", "faixa etaria", "edad"],
    &["alternate series", "série alternativa", "serie alternativa"],
    &[
        "alternate number",
        "número alternativo",
        "numero alternativo",
    ],
    &["scan info", "scan"],
    &["gtin"],
];

/// The field a template key names; `None` for keys not here.
pub fn field(key: &str) -> Option<&'static str> {
    let key = key.trim().to_lowercase();
    NAMES
        .iter()
        .find(|names| names.iter().any(|n| *n == key))
        .map(|names| names[0])
}

/// Whether a template names a field from here (so it needs the data).
pub fn wanted(template: &str) -> bool {
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            return false;
        };
        let inner = &rest[open + 1..open + close];
        let inner = inner.rsplit_once(':').map_or(inner, |(k, n)| {
            if n.trim().parse::<u8>().is_ok() {
                k
            } else {
                inner
            }
        });
        if inner.split('|').any(|k| field(k).is_some()) {
            return true;
        }
        rest = &rest[open + close + 1..];
    }
    false
}

const MONTHS: [[&str; 12]; 3] = [
    [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    [
        "janeiro",
        "fevereiro",
        "março",
        "abril",
        "maio",
        "junho",
        "julho",
        "agosto",
        "setembro",
        "outubro",
        "novembro",
        "dezembro",
    ],
    [
        "enero",
        "febrero",
        "marzo",
        "abril",
        "mayo",
        "junio",
        "julio",
        "agosto",
        "septiembre",
        "octubre",
        "noviembre",
        "diciembre",
    ],
];

/// Puts a date in: `YYYY-MM-DD`, `YYYY-MM` or a year and month apart.
pub fn add_date(extra: &mut Extra, date: &str) {
    let mut parts = date.trim().split(['-', '/']);
    let year = parts
        .next()
        .and_then(|y| y.parse::<u16>().ok())
        .filter(|&y| y > 0);
    let month = parts.next().and_then(|m| m.parse::<u8>().ok());
    let day = parts
        .next()
        .and_then(|d| d.get(..2.min(d.len()))?.parse::<u8>().ok());
    add_ymd(extra, year, month, day);
}

/// Puts a date in from its parts (any may be missing).
pub fn add_ymd(extra: &mut Extra, year: Option<u16>, month: Option<u8>, day: Option<u8>) {
    let month = month.filter(|m| (1..=12).contains(m));
    let day = day
        .filter(|d| (1..=31).contains(d))
        .filter(|_| month.is_some());
    let mut date = year.map(|y| format!("{y:04}")).unwrap_or_default();
    if let Some(m) = month {
        let i = usize::from(m - 1);
        extra.insert("month".into(), format!("{m:02}"));
        extra.insert("month name".into(), MONTHS[0][i].into());
        extra.insert("month name pt".into(), MONTHS[1][i].into());
        extra.insert("month name es".into(), MONTHS[2][i].into());
        if !date.is_empty() {
            date.push_str(&format!("-{m:02}"));
        }
    }
    if let Some(d) = day {
        extra.insert("day".into(), format!("{d:02}"));
        if year.is_some() {
            date.push_str(&format!("-{d:02}"));
        }
    }
    if !date.is_empty() {
        extra.insert("date".into(), date);
    }
}

/// Puts a value in when it is not empty.
pub fn add(extra: &mut Extra, field: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        extra.insert(field.into(), value.to_string());
    }
}

/// The values made safe for names, by field.
pub fn safe(extra: &Extra, colon: &str) -> Extra {
    // A date given alone (from the interface) brings its parts.
    let mut full;
    let extra = match extra.get("date") {
        Some(d) if !extra.contains_key("month") => {
            full = extra.clone();
            add_date(&mut full, &d.clone());
            &full
        }
        _ => extra,
    };
    extra
        .iter()
        .map(|(k, v)| (k.clone(), crate::tmdb::safe_title(v, colon)))
        .collect()
}

/// A template key's value: the field's, empty when the match lacks it,
/// `None` for keys not here (another rule's own).
pub fn get<'a>(safe: &'a Extra, key: &str) -> Option<&'a str> {
    let f = field(key)?;
    Some(safe.get(f).map_or("", String::as_str))
}

/// What a video's release name tells besides the title: the edition
/// (`Director's Cut`, `Extended`), the source (`BluRay`, `WEB-DL`) and,
/// in names with a source or resolution, the group (`-GROUP` at the end,
/// `[Group]` at the start).
pub fn add_release(extra: &mut Extra, name: &str) {
    use regex::Regex;
    use std::sync::LazyLock;
    static EDITION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)\b(director'?s[ ._-]?cut|extended(?:[ ._-](?:cut|edition))?|unrated|uncut|theatrical(?:[ ._-]cut)?|remastered|imax|criterion|special[ ._-]edition|ultimate[ ._-](?:cut|edition)|final[ ._-]cut|(?:\d+(?:th|st|nd|rd)[ ._-])?anniversary(?:[ ._-]edition)?)\b",
        )
        .unwrap()
    });
    static SOURCE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)\b(remux|blu-?ray|bdrip|brrip|bd25|bd50|web-?dl|webrip|web|hdtv|pdtv|dvdrip|dvd5|dvd9|dvd|hdrip|tvrip|vhsrip)\b",
        )
        .unwrap()
    });
    static RESOLUTION: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\b(?:2160|1080|720|576|480)[pi]\b|\b4k\b").unwrap());
    static GROUP_END: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"-([A-Za-z0-9]{2,20})(?:\[[^\]]*\])?$").unwrap());
    static GROUP_START: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\[([^\]]{2,30})\]").unwrap());
    if let Some(m) = EDITION.find(name) {
        let words: Vec<String> = m
            .as_str()
            .split([' ', '.', '_', '-'])
            .filter(|w| !w.is_empty())
            .map(|w| {
                let lower = w.to_lowercase();
                match lower.as_str() {
                    "imax" => "IMAX".to_string(),
                    "directors" | "director's" => "Director's".to_string(),
                    _ => {
                        let mut c = lower.chars();
                        c.next()
                            .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                            .unwrap_or_default()
                    }
                }
            })
            .collect();
        add(extra, "edition", &words.join(" "));
    }
    let sources: Vec<String> = SOURCE
        .find_iter(name)
        .map(|m| m.as_str().to_lowercase().replace('-', ""))
        .collect();
    let has = |s: &str| sources.iter().any(|x| x == s);
    let source = if has("remux") {
        "Remux"
    } else if has("bluray") || has("bdrip") || has("brrip") || has("bd25") || has("bd50") {
        "BluRay"
    } else if has("webdl") {
        "WEB-DL"
    } else if has("webrip") {
        "WEBRip"
    } else if has("web") {
        "WEB"
    } else if has("hdtv") || has("pdtv") {
        "HDTV"
    } else if has("dvdrip") {
        "DVDRip"
    } else if has("dvd") || has("dvd5") || has("dvd9") {
        "DVD"
    } else if has("hdrip") {
        "HDRip"
    } else if has("tvrip") {
        "TVRip"
    } else if has("vhsrip") {
        "VHSRip"
    } else {
        ""
    };
    add(extra, "source", source);
    if let Some(c) = GROUP_START.captures(name) {
        add(extra, "release group", &c[1]);
    } else if (!source.is_empty() || RESOLUTION.is_match(name))
        && let Some(c) = GROUP_END.captures(name.trim())
    {
        let g = &c[1];
        // A codec or resolution at the end is no group.
        let lower = g.to_lowercase();
        if !matches!(
            lower.as_str(),
            "dl" | "rip" | "x264" | "x265" | "h264" | "h265" | "hevc"
        ) && !RESOLUTION.is_match(g)
        {
            add(extra, "release group", g);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        let mut e = Extra::new();
        add_date(&mut e, "2010-05-03");
        let s = safe(&e, " - ");
        assert_eq!(get(&s, "month name"), Some("May"));
        assert_eq!(get(&s, "nome do mês"), Some("maio"));
        assert_eq!(get(&s, "Nombre del mes"), Some("mayo"));
        assert_eq!(get(&s, "dia"), Some("03"));
        assert_eq!(get(&s, "data"), Some("2010-05-03"));
        assert_eq!(get(&s, "genre"), Some(""));
        assert_eq!(get(&s, "title"), None);
        let mut e = Extra::new();
        add_date(&mut e, "1999");
        assert_eq!(e.get("date").map(String::as_str), Some("1999"));
        assert!(!e.contains_key("month"));
        let mut e = Extra::new();
        add_ymd(&mut e, Some(1987), Some(13), Some(2));
        assert_eq!(e.get("date").map(String::as_str), Some("1987"));
    }

    #[test]
    fn release_names() {
        let r = |name: &str| {
            let mut e = Extra::new();
            add_release(&mut e, name);
            let get = |k: &str| e.get(k).cloned().unwrap_or_default();
            (get("edition"), get("source"), get("release group"))
        };
        assert_eq!(
            r("Movie.Title.2010.Directors.Cut.1080p.BluRay.x264-GRP"),
            ("Director's Cut".into(), "BluRay".into(), "GRP".into())
        );
        assert_eq!(
            r("Movie 2010 EXTENDED 2160p WEB-DL"),
            ("Extended".into(), "WEB-DL".into(), "".into())
        );
        assert_eq!(
            r("[SubGroup] Show - 01 (1080p)"),
            ("".into(), "".into(), "SubGroup".into())
        );
        // A dash in a plain name is no group.
        assert_eq!(r("Spider-Man"), ("".into(), "".into(), "".into()));
        assert_eq!(
            r("Movie.2010.720p.HDTV.x264"),
            ("".into(), "HDTV".into(), "".into())
        );
    }

    #[test]
    fn templates() {
        assert!(wanted("{title} ({year}) {genre}"));
        assert!(wanted("{title|gênero:2}"));
        assert!(!wanted("{title} ({year})"));
    }
}
