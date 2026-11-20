//! Details typed by hand (the window's details windows, the command
//! line's `--edit`): the fields typed replace a file's match (or what the
//! file says about itself, when nothing was chosen), the same way for
//! both: a book or comic retyped loses its catalogue id, a song moved to
//! another album loses its release's.

use renome_core::books::{BookMatch, Guess};
use renome_core::comics::ComicMatch;
use renome_core::musicbrainz::SongMatch;
use std::collections::BTreeMap;

/// The fields typed, by name (as the details windows name them).
pub type Changed = BTreeMap<String, String>;

/// A field of a details window (and of `--edit`).
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub key: &'static str,
    /// Typed as a whole number.
    pub numeric: bool,
    /// Shown across the window.
    pub wide: bool,
    /// The most characters it takes (numbers: digits), when limited.
    pub max_length: Option<usize>,
}

/// The digits a number field takes (a year, a track, a minute).
pub const DIGITS: usize = 4;

const fn f(key: &'static str, numeric: bool, wide: bool) -> Field {
    Field {
        key,
        numeric,
        wide,
        max_length: if numeric { Some(DIGITS) } else { None },
    }
}

/// Whether a value suits a field: a number field only digits, and no
/// more characters than it takes (empty clears it).
pub fn fits(field: &Field, value: &str) -> bool {
    (!field.numeric || value.chars().all(|c| c.is_ascii_digit()))
        && field.max_length.is_none_or(|n| value.chars().count() <= n)
}

/// A file's details as a details window shows them, by field: numbers as
/// digits (none or 0: empty), a book's authors joined by `; ` (as typed
/// back, see [`book`]), its language from its extra details.
pub fn as_text(kind: &str, value: &serde_json::Value) -> BTreeMap<String, String> {
    use serde_json::Value;
    fields_of(kind)
        .iter()
        .map(|f| {
            let v = if kind == "book" && f.key == "language" {
                &value["extra"]["language"]
            } else {
                &value[f.key]
            };
            let text = match v {
                Value::String(s) => s.clone(),
                Value::Number(n) if n.as_f64() != Some(0.0) => n.to_string(),
                Value::Array(list) => list
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; "),
                _ => String::new(),
            };
            (f.key.to_string(), text)
        })
        .collect()
}

pub const SONG: &[Field] = &[
    f("title", false, true),
    f("artist", false, false),
    f("albumArtist", false, false),
    f("genre", false, false),
    f("album", false, true),
    f("year", true, false),
    f("track", true, false),
    f("trackTotal", true, false),
    f("disc", true, false),
    f("discTotal", true, false),
];
pub const BOOK: &[Field] = &[
    f("title", false, true),
    f("authors", false, true),
    f("year", true, false),
    f("publisher", false, false),
    f("language", false, false),
    f("series", false, true),
    f("seriesIndex", false, false),
    f("isbn", false, false),
];
pub const COMIC: &[Field] = &[
    f("series", false, true),
    f("seriesYear", true, false),
    f("volume", true, false),
    f("number", false, false),
    f("title", false, true),
    f("year", true, false),
    f("publisher", false, false),
    f("writer", false, false),
    f("artist", false, false),
];
/// A photo's date in its parts (`--edit` also takes `date=` whole).
pub const PHOTO: &[Field] = &[
    f("day", true, false),
    f("month", true, false),
    f("year", true, false),
    f("hour", true, false),
    f("minute", true, false),
];

/// The editor of a lookup rule (`music`, `book`, `comic`) or `photo`.
pub fn fields_of(kind: &str) -> &'static [Field] {
    match kind {
        "music" | "song" => SONG,
        "book" => BOOK,
        "comic" => COMIC,
        "photo" => PHOTO,
        _ => &[],
    }
}

/// Each editor's fields, by rule.
pub fn fields_table() -> serde_json::Value {
    serde_json::json!({ "music": SONG, "book": BOOK, "comic": COMIC, "photo": PHOTO })
}

fn int<T: std::str::FromStr>(v: &str) -> Option<T> {
    v.trim().parse().ok()
}

/// A song with the fields typed.
pub fn song(mut m: SongMatch, changed: &Changed) -> SongMatch {
    for (k, v) in changed {
        match k.as_str() {
            "year" => m.year = int(v),
            "track" => m.track = int(v).unwrap_or(0),
            "trackTotal" => m.track_total = int(v).unwrap_or(0),
            "disc" => m.disc = int(v).unwrap_or(0),
            "discTotal" => m.disc_total = int(v).unwrap_or(0),
            "title" => m.title = v.clone(),
            "artist" => m.artist = v.clone(),
            "albumArtist" => m.album_artist = v.clone(),
            "album" => m.album = v.clone(),
            "genre" => m.genre = v.clone(),
            _ => {}
        }
    }
    // Another album: the release's ids no longer tell it.
    if m.typed
        || m.release.is_empty()
        || changed.contains_key("album")
        || changed.contains_key("albumArtist")
    {
        m.typed = true;
        m.release.clear();
        m.recording.clear();
        m.track_id.clear();
        m.release_group.clear();
        m.artist_ids.clear();
        m.album_artist_ids.clear();
    }
    m
}

/// What a book says about itself, as a start for typing (no catalogue).
pub fn book_from_guess(g: &Guess) -> BookMatch {
    let mut extra = renome_core::fields::Extra::new();
    if !g.language.is_empty() {
        extra.insert("language".into(), g.language.clone());
    }
    BookMatch {
        title: g.title.clone(),
        authors: g
            .author
            .split([';', '&'])
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .map(String::from)
            .collect(),
        year: g.year,
        publisher: g.publisher.clone(),
        isbn: g.isbns.first().cloned().unwrap_or_default(),
        series: g.series.clone(),
        series_index: g.series_index.clone(),
        extra,
        ..BookMatch::default()
    }
}

/// A book with the fields typed.
pub fn book(mut m: BookMatch, changed: &Changed) -> BookMatch {
    for (k, v) in changed {
        match k.as_str() {
            "authors" => {
                m.authors = v
                    .split(';')
                    .map(str::trim)
                    .filter(|a| !a.is_empty())
                    .map(String::from)
                    .collect()
            }
            "year" => {
                m.year = int(v);
                // One year typed: the edition's goes.
                m.edition_year = None;
            }
            "language" => {
                if v.is_empty() {
                    m.extra.remove("language");
                } else {
                    m.extra.insert("language".into(), v.clone());
                }
            }
            "title" => m.title = v.clone(),
            "publisher" => m.publisher = v.clone(),
            "series" => m.series = v.clone(),
            "seriesIndex" => m.series_index = v.clone(),
            "isbn" => m.isbn = v.clone(),
            _ => {}
        }
    }
    // Another book: the catalogue's work (and its cover) no longer tell it.
    if changed.contains_key("title") || changed.contains_key("authors") {
        m.work.clear();
    }
    m
}

/// A comic with the fields typed.
pub fn comic(mut m: ComicMatch, changed: &Changed) -> ComicMatch {
    for (k, v) in changed {
        match k.as_str() {
            "seriesYear" => m.series_year = int(v),
            "volume" => m.volume = int(v),
            "year" => m.year = int(v),
            "series" => m.series = v.clone(),
            "number" => m.number = v.clone(),
            "title" => m.title = v.clone(),
            "publisher" => m.publisher = v.clone(),
            "writer" => m.writer = v.clone(),
            "artist" => m.artist = v.clone(),
            _ => {}
        }
    }
    // The catalogue's issue no longer tells it.
    if changed.contains_key("series") || changed.contains_key("number") {
        m.id.clear();
    }
    m
}

/// A date's parts (year, month, day, hour, minute): `2018-03-27` or
/// `2018-03-27 14:05`; empty ones for no date.
pub fn date_parts(text: &str) -> [String; 5] {
    let (day, time) = text.split_once(' ').unwrap_or((text, ""));
    let d: Vec<&str> = day.split('-').collect();
    if d.len() != 3 {
        return Default::default();
    }
    let tm: Vec<&str> = time.split(':').collect();
    let s = |v: Option<&&str>| v.map(|x| x.trim().to_string()).unwrap_or_default();
    [
        d[0].into(),
        d[1].into(),
        d[2].into(),
        s(tm.first()),
        s(tm.get(1)),
    ]
}

/// The parts as one date; `""` when all are empty; `None` when they make
/// no date.
fn joined(y: &str, mo: &str, d: &str, h: &str, mi: &str) -> Option<String> {
    if [y, mo, d, h, mi].iter().all(|v| v.is_empty()) {
        return Some(String::new());
    }
    let (yy, mm, dd): (i32, u32, u32) = (y.parse().ok()?, mo.parse().ok()?, d.parse().ok()?);
    if y.len() != 4 || yy < 1900 {
        return None;
    }
    chrono::NaiveDate::from_ymd_opt(yy, mm, dd)?;
    let day = format!("{yy}-{mm:02}-{dd:02}");
    if h.is_empty() && mi.is_empty() {
        return Some(day);
    }
    let (hh, mn): (u32, u32) = (
        if h.is_empty() { 0 } else { h.parse().ok()? },
        if mi.is_empty() { 0 } else { mi.parse().ok()? },
    );
    (hh <= 23 && mn <= 59).then(|| format!("{day} {hh:02}:{mn:02}"))
}

/// A photo's date with the fields typed (`date` whole, or its parts:
/// `year`, `month`, `day`, `hour`, `minute`) over the one it had (`old`,
/// empty for what the file says); `""`: back to what the file says;
/// `None`: they make no date.
pub fn photo_date(old: &str, changed: &Changed) -> Option<String> {
    let text = match changed.get("date") {
        Some(d) => d.trim().to_string(),
        None => {
            let [y, mo, d, h, mi] = date_parts(old);
            let get =
                |k: &str, old: String| changed.get(k).map(|v| v.trim().to_string()).unwrap_or(old);
            joined(
                &get("year", y),
                &get("month", mo),
                &get("day", d),
                &get("hour", h),
                &get("minute", mi),
            )?
        }
    };
    if text.is_empty() {
        return Some(text);
    }
    let [y, mo, d, h, mi] = date_parts(&text);
    joined(&y, &mo, &d, &h, &mi).filter(|j| *j == text && !y.is_empty())
}

/// A name's stem, as the lookups read it (a folder's whole name).
fn stem_of(name: &str, is_dir: bool) -> String {
    if is_dir {
        return name.to_string();
    }
    std::path::Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string())
}

/// Whether a details window (and `--edit`) edits this row (see the
/// core's `plan::editable`).
pub fn editable(kind: &str, i: &renome_core::PreviewItem, audiobooks: bool) -> bool {
    renome_core::plan::editable(kind, i, audiobooks)
}

/// The rows a details window edits for those chosen, in the list's
/// order: those it edits ([`editable`]), an audiobook's files all
/// together.
pub fn rows(
    kind: &str,
    items: &[renome_core::PreviewItem],
    chosen: &std::collections::HashSet<std::path::PathBuf>,
    parts: &std::collections::HashMap<std::path::PathBuf, super::books::Part>,
    audiobooks: bool,
) -> Vec<std::path::PathBuf> {
    let leaders: std::collections::HashSet<&std::path::PathBuf> = items
        .iter()
        .filter(|i| chosen.contains(&i.path))
        .filter_map(|i| parts.get(&i.path).map(|p| &p.leader))
        .collect();
    items
        .iter()
        .filter(|i| editable(kind, i, audiobooks))
        .filter(|i| {
            chosen.contains(&i.path)
                || (kind == "book"
                    && parts
                        .get(&i.path)
                        .is_some_and(|p| leaders.contains(&p.leader)))
        })
        .map(|i| i.path.clone())
        .collect()
}

/// What a song's details start from: what was chosen or typed, else its
/// tags, else nothing (typed).
pub fn song_start(
    current: Option<SongMatch>,
    file: Option<&renome_core::FileEntry>,
    from_name: bool,
) -> SongMatch {
    current
        .or_else(|| {
            let f = file?;
            renome_core::rules::song_tags(f, from_name)
                .or_else(|| renome_core::rules::any_song_tags(f))
                .map(|t| SongMatch::from_tags(&t))
        })
        .unwrap_or_else(|| SongMatch {
            typed: true,
            ..SongMatch::default()
        })
}

/// What a book's details start from: what was chosen or typed, else what
/// the lookup guessed, else what the file and its name say.
pub fn book_start(
    current: Option<BookMatch>,
    guess: Option<&Guess>,
    path: &std::path::Path,
    name: &str,
) -> BookMatch {
    current.unwrap_or_else(|| match guess {
        Some(g) => book_from_guess(g),
        None => book_from_guess(&renome_core::books::guess(
            &stem_of(name, false),
            &renome_core::ebook::read(path),
        )),
    })
}

/// What a comic's details start from: what was chosen or typed, else
/// what the lookup read of it, else what the file and its name say.
pub fn comic_start(
    current: Option<ComicMatch>,
    own: Option<&ComicMatch>,
    path: &std::path::Path,
    name: &str,
    is_dir: bool,
    by_folder: bool,
) -> ComicMatch {
    current
        .or_else(|| own.cloned())
        .or_else(|| renome_core::comics::from_path(path, Some(&stem_of(name, is_dir)), by_folder))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(pairs: &[(&str, &str)]) -> Changed {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn dates_typed_in_parts_or_whole() {
        assert_eq!(
            photo_date("", &c(&[("year", "2018"), ("month", "3"), ("day", "27")])),
            Some("2018-03-27".into())
        );
        assert_eq!(
            photo_date("2018-03-27", &c(&[("hour", "14"), ("minute", "5")])),
            Some("2018-03-27 14:05".into())
        );
        assert_eq!(
            photo_date("2018-03-27", &c(&[("day", "31"), ("month", "2")])),
            None
        );
        assert_eq!(
            photo_date("2018-03-27", &c(&[("date", "")])),
            Some(String::new())
        );
        assert_eq!(photo_date("", &c(&[("date", "2018-02-30")])), None);
    }

    #[test]
    fn retyped_details_lose_their_catalogue() {
        let m = BookMatch {
            work: "OL1W".into(),
            ..Default::default()
        };
        assert_eq!(book(m.clone(), &c(&[("publisher", "X")])).work, "OL1W");
        assert_eq!(book(m, &c(&[("title", "Y")])).work, "");
        let s = SongMatch {
            release: "r".into(),
            ..Default::default()
        };
        assert_eq!(song(s.clone(), &c(&[("title", "t")])).release, "r");
        assert!(song(s, &c(&[("album", "a")])).typed);
    }

    #[test]
    fn fields_show_as_they_are_typed_back() {
        let book = serde_json::json!({
            "title": "Um Livro", "authors": ["Ana Lima", "Rui Sá"], "year": 0,
            "extra": { "language": "por" }, "seriesIndex": "2",
        });
        let text = as_text("book", &book);
        assert_eq!(text["authors"], "Ana Lima; Rui Sá");
        assert_eq!(text["year"], "");
        assert_eq!(text["language"], "por");
        assert_eq!(text["seriesIndex"], "2");
        // Read back, the same authors.
        let changed: Changed = [("authors".to_string(), text["authors"].clone())].into();
        assert_eq!(book_with(&changed).authors, vec!["Ana Lima", "Rui Sá"]);
        let year = BOOK.iter().find(|f| f.key == "year").unwrap();
        assert!(fits(year, "1999") && fits(year, ""));
        assert!(!fits(year, "19999") && !fits(year, "19a9"));
        let title = BOOK.iter().find(|f| f.key == "title").unwrap();
        assert!(fits(title, "Qualquer coisa, 123"));
    }

    fn book_with(changed: &Changed) -> BookMatch {
        book(BookMatch::default(), changed)
    }
}
