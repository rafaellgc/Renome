//! Searches typed by the person in a review or picker (the window's, the
//! command line's chooser): a link or id pasted finds that one; else the
//! words are searched, a year at the end of a movie's title read as its
//! year, an author after ` / ` as the book's.

use crate::tmdb::{self, ShowSearch, TvItem};
use crate::{anilist, discogs, musicbrainz, openlibrary};
use renome_core::links::{self, Link};

/// A movie: a TMDB or IMDb link or id, else a title (with its year
/// typed apart, or at the end: `Title 1999`).
pub async fn movie(
    client: &tmdb::Client,
    text: &str,
    year: Option<u16>,
) -> Result<Vec<renome_core::tmdb::Candidate>, String> {
    let text = text.trim().trim_start_matches("tmdb:").trim();
    let (id, title, year) = match links::parse(text) {
        Some(Link::Tmdb { tv: false, id }) => (Some(id), String::new(), None),
        Some(Link::Imdb(tt)) => (
            Some(client.find_imdb(&tt, false).await.ok_or("linkNotFound")?),
            String::new(),
            None,
        ),
        Some(_) => return Err("linkOther".into()),
        None => match text.parse::<u64>() {
            Ok(id) => (Some(id), String::new(), None),
            Err(_) => match (year, text.rsplit_once(' ')) {
                (None, Some((t, y))) if y.len() == 4 && y.parse::<u16>().is_ok() => {
                    (None, t.trim().to_string(), y.parse().ok())
                }
                _ => (None, text.to_string(), year),
            },
        },
    };
    let guess = renome_core::tmdb::Guess { title, year, id };
    Ok(client.search(&guess).await?.0)
}

/// A show: on TMDB a TMDB link or an IMDb one, else its name; on AniList
/// its name. `items`: the files, to rank the shows by them.
pub async fn show(
    client: &tmdb::Client,
    anime: Option<&anilist::Client>,
    text: &str,
    items: &[TvItem],
) -> Result<ShowSearch, String> {
    let text = text.trim();
    if let Some(al) = anime {
        return al.anime_search(text, items).await;
    }
    let query = match links::parse(text) {
        Some(Link::Imdb(tt)) => client
            .find_imdb(&tt, true)
            .await
            .ok_or("linkNotFound")?
            .to_string(),
        Some(Link::Tmdb { tv: true, .. }) | None => text.to_string(),
        Some(_) if text.contains("tmdbid") => text.to_string(),
        Some(_) => return Err("linkOther".into()),
    };
    tmdb::search_show(client, &query, items).await
}

/// An album: a MusicBrainz or Discogs link, else the album (and artist)
/// typed, apart or as `artist / album`, on Discogs when asked (with the
/// person's token).
pub async fn album(
    mb: &musicbrainz::Client,
    dg: &discogs::Client,
    query: &renome_core::musicbrainz::AlbumQuery,
    on_discogs: bool,
) -> Result<Vec<renome_core::musicbrainz::Candidate>, String> {
    let split;
    let query = match query.album.split_once(" / ") {
        Some((artist, album)) if !artist.trim().is_empty() && !album.trim().is_empty() => {
            split = renome_core::musicbrainz::AlbumQuery {
                artist: artist.trim().to_string(),
                album: album.trim().to_string(),
                ..query.clone()
            };
            &split
        }
        _ => query,
    };
    match links::parse(query.album.trim()) {
        Some(Link::MusicBrainz(id)) => {
            let r = mb.release(&id).await?;
            return Ok(vec![renome_core::musicbrainz::summary(&r, 1.0)]);
        }
        Some(Link::Discogs(id)) => {
            let id = format!("{}{id}", renome_core::discogs::PREFIX);
            let r = dg.release(&id).await?;
            return Ok(vec![renome_core::musicbrainz::summary(&r, 1.0)]);
        }
        Some(_) => return Err("linkOther".into()),
        None => {}
    }
    if on_discogs && dg.ready() {
        dg.search(query).await
    } else {
        mb.search(query).await
    }
}

/// Where a new album search looks first: Discogs when what was found for
/// the album came from there (and the token is set), else MusicBrainz.
pub fn album_on_discogs(first_found: Option<&str>, token: bool) -> bool {
    token && first_found.is_some_and(|id| id.starts_with(renome_core::discogs::PREFIX))
}

/// A book: an ISBN or a link, else its title and author (typed apart, or
/// as `title / author`).
pub async fn book(
    ol: &openlibrary::Client,
    lang: &str,
    title: &str,
    author: &str,
) -> Result<Vec<renome_core::books::Candidate>, String> {
    if let Some(link) = links::parse(title.trim()) {
        return ol.by_link(&link, lang).await;
    }
    let (title, author) = match (author.trim(), title.split_once(" / ")) {
        ("", Some((t, a))) => (t.trim(), a.trim()),
        (a, _) => (title.trim(), a),
    };
    let guess = renome_core::books::Guess {
        title: title.into(),
        author: author.into(),
        from_file: true,
        ..Default::default()
    };
    Ok(ol.find(&guess, lang).await?.0)
}

/// A comic series: a link of the catalogue, else its name, from a year
/// and by a publisher when given (typed apart, or as `name 1999 /
/// publisher`; the year only when it can be one, see
/// [`renome_core::comics::is_year`]). Those that could hold the files'
/// highest issue first ([`super::comics::by_fit`]).
#[allow(clippy::too_many_arguments)]
pub async fn comic(
    sources: &crate::comics::Sources<'_>,
    source: &str,
    key: &str,
    text: &str,
    year: &str,
    publisher: &str,
    lang: &str,
    skip: &str,
    highest: f64,
) -> Result<Vec<renome_core::comics::Volume>, String> {
    let (name, year, publisher) = comic_query(text, year, publisher);
    let found = sources
        .search(source, key, &name, year, &publisher, lang, skip)
        .await?;
    Ok(super::comics::by_fit(found, highest))
}

/// What a comic search asks: the name, the year (typed apart, or at the
/// end of the name, when it can be one) and the publisher (typed apart,
/// or after ` / `); a link as it is.
fn comic_query(text: &str, year: &str, publisher: &str) -> (String, Option<u16>, String) {
    let year_of = |y: &str| {
        y.trim()
            .trim_matches(|c| c == '(' || c == ')')
            .parse::<u32>()
            .ok()
            .filter(|y| renome_core::comics::is_year(*y))
            .and_then(|y| u16::try_from(y).ok())
    };
    let mut name = text.trim();
    let mut publisher = publisher.trim();
    if links::parse(name).is_none()
        && publisher.is_empty()
        && let Some((n, p)) = name.split_once(" / ")
    {
        (name, publisher) = (n.trim(), p.trim());
    }
    let mut year = year_of(year);
    if year.is_none()
        && let Some((n, y)) = name.rsplit_once(' ')
        && let Some(y) = year_of(y)
    {
        (name, year) = (n.trim(), Some(y));
    }
    (name.to_string(), year, publisher.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discogs_first_when_found_there() {
        assert!(album_on_discogs(Some("discogs:123"), true));
        assert!(!album_on_discogs(Some("discogs:123"), false));
        assert!(!album_on_discogs(Some("abc-def"), true));
    }

    #[test]
    fn comic_searches_read_year_and_publisher() {
        let q = |t, y, p| super::comic_query(t, y, p);
        assert_eq!(
            q("Lanterna 1999", "", ""),
            ("Lanterna".into(), Some(1999), String::new())
        );
        assert_eq!(
            q("Lanterna 1999 / Editora Sol", "", ""),
            ("Lanterna".into(), Some(1999), "Editora Sol".into())
        );
        // An issue number is not a year; one typed apart wins.
        assert_eq!(
            q("Lanterna 52", "", ""),
            ("Lanterna 52".into(), None, String::new())
        );
        assert_eq!(
            q("Lanterna 1999", "(2004)", "X"),
            ("Lanterna 1999".into(), Some(2004), "X".into())
        );
        assert_eq!(
            q("Lanterna", "12", ""),
            ("Lanterna".into(), None, String::new())
        );
    }
}
