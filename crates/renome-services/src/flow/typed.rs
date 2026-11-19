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
/// typed, on Discogs when asked (with the person's token).
pub async fn album(
    mb: &musicbrainz::Client,
    dg: &discogs::Client,
    query: &renome_core::musicbrainz::AlbumQuery,
    on_discogs: bool,
) -> Result<Vec<renome_core::musicbrainz::Candidate>, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discogs_first_when_found_there() {
        assert!(album_on_discogs(Some("discogs:123"), true));
        assert!(!album_on_discogs(Some("discogs:123"), false));
        assert!(!album_on_discogs(Some("abc-def"), true));
    }
}
