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
}
