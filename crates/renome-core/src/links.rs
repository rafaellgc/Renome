//! What a link or id pasted into a search box names: a page of one of the
//! catalogues the rules use (`themoviedb.org/movie/603-…`,
//! `comicvine.gamespot.com/…/4050-796/`, `musicbrainz.org/release/<uuid>`),
//! an IMDb id (`tt0133093`) or an ISBN. A search for it fetches that one
//! item instead of searching by name.

use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// A TMDB movie (`tv` false) or show.
    Tmdb {
        tv: bool,
        id: u64,
    },
    /// An IMDb title (`tt0133093`), movie or show.
    Imdb(String),
    /// A Comic Vine volume (`4050-…`) or issue (`4000-…`), by number.
    ComicVine {
        issue: bool,
        id: String,
    },
    Metron(String),
    /// A Grand Comics Database series.
    Gcd(String),
    /// A MangaDex title (a uuid).
    MangaDex(String),
    /// An AniList manga (`anime` false) or anime.
    AniList {
        anime: bool,
        id: u64,
    },
    /// A MangaUpdates series, by its number in the API (the site's links
    /// write it in base 36).
    MangaUpdates(String),
    /// An Open Library work (`OL…W`) or edition (`OL…M`).
    OpenLibrary(String),
    /// An ISBN-13 (an ISBN-10 is turned into one).
    Isbn(String),
    /// A MusicBrainz release (a uuid).
    MusicBrainz(String),
    /// A Discogs release.
    Discogs(String),
}

impl Link {
    /// The catalogue's name as the comic rule's source setting writes it,
    /// for links to comic catalogues.
    pub fn comic_source(&self) -> Option<&'static str> {
        Some(match self {
            Link::ComicVine { .. } => "comicvine",
            Link::Metron(_) => "metron",
            Link::Gcd(_) => "gcd",
            Link::MangaDex(_) => "mangadex",
            Link::AniList { anime: false, .. } => "anilist",
            Link::MangaUpdates(_) => "mangaupdates",
            _ => return None,
        })
    }
}

/// A pattern and how its match makes the link.
type Maker = (Regex, fn(&regex::Captures) -> Option<Link>);

static LINKS: LazyLock<Vec<Maker>> = LazyLock::new(|| {
    let re = |s: &str| Regex::new(&format!("(?i){s}")).unwrap();
    vec![
        (re(r"themoviedb\.org/(movie|tv)/(\d+)"), |c| {
            Some(Link::Tmdb {
                tv: c[1].eq_ignore_ascii_case("tv"),
                id: c[2].parse().ok()?,
            })
        }),
        (re(r"\b(tt\d{6,9})\b"), |c| {
            Some(Link::Imdb(c[1].to_ascii_lowercase()))
        }),
        (
            re(r"comicvine\.gamespot\.com/(?:[^/]+/)?(4050|4000)-(\d+)"),
            |c| {
                Some(Link::ComicVine {
                    issue: &c[1] == "4000",
                    id: c[2].to_string(),
                })
            },
        ),
        (re(r"metron\.cloud/series/(\d+)\b"), |c| {
            Some(Link::Metron(c[1].to_string()))
        }),
        (re(r"comics\.org/series/(\d+)"), |c| {
            Some(Link::Gcd(c[1].to_string()))
        }),
        (
            re(
                r"mangadex\.org/title/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})",
            ),
            |c| Some(Link::MangaDex(c[1].to_ascii_lowercase())),
        ),
        (re(r"anilist\.co/(manga|anime)/(\d+)"), |c| {
            Some(Link::AniList {
                anime: c[1].eq_ignore_ascii_case("anime"),
                id: c[2].parse().ok()?,
            })
        }),
        (re(r"mangaupdates\.com/series/([0-9a-z]+)"), |c| {
            let id = u64::from_str_radix(&c[1].to_ascii_lowercase(), 36).ok()?;
            Some(Link::MangaUpdates(id.to_string()))
        }),
        (re(r"mangaupdates\.com/series\.html\?id=(\d+)"), |c| {
            Some(Link::MangaUpdates(c[1].to_string()))
        }),
        (re(r"openlibrary\.org/(?:works|books)/(OL\d+[WM])"), |c| {
            Some(Link::OpenLibrary(c[1].to_ascii_uppercase()))
        }),
        (
            re(
                r"musicbrainz\.org/release/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})",
            ),
            |c| Some(Link::MusicBrainz(c[1].to_ascii_lowercase())),
        ),
        (re(r"discogs\.com/(?:[a-z_-]+/)?release/(\d+)"), |c| {
            Some(Link::Discogs(c[1].to_string()))
        }),
    ]
});

/// `tmdbid-603`, `[tmdbid=603]`: a TMDB id written as Jellyfin does.
static TMDB_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\btmdbid[-=](\d+)").unwrap());

/// The item a pasted text names, if it is a link or an id that says which
/// catalogue it is from. A bare number is not one: titles may be numbers
/// (a TV search box still takes them as TMDB ids).
pub fn parse(text: &str) -> Option<Link> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    for (re, make) in LINKS.iter() {
        if let Some(c) = re.captures(text)
            && let Some(link) = make(&c)
        {
            return Some(link);
        }
    }
    if let Some(c) = TMDB_ID.captures(text) {
        // Movie or show: the search box it is typed in says.
        return Some(Link::Tmdb {
            tv: false,
            id: c[1].parse().ok()?,
        });
    }
    isbn(text).map(Link::Isbn)
}

/// An ISBN typed alone (`978-85-359-1484-9`, `ISBN 0-306-40615-2`), as
/// ISBN-13. Only digits, dashes, spaces and a final X may be there, so a
/// title with a number in it is not one.
fn isbn(text: &str) -> Option<String> {
    let rest = text
        .strip_prefix("ISBN")
        .or_else(|| text.strip_prefix("isbn"))
        .unwrap_or(text)
        .trim_start_matches([':', ' ']);
    let ok = rest
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '-' | ' ' | 'X' | 'x'));
    let digits = rest.chars().filter(char::is_ascii_digit).count();
    (ok && digits >= 9)
        .then(|| crate::ebook::isbn13(rest))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_of_each_catalogue() {
        assert_eq!(
            parse("https://www.themoviedb.org/movie/603-the-matrix?language=pt-BR"),
            Some(Link::Tmdb { tv: false, id: 603 })
        );
        assert_eq!(
            parse("https://www.themoviedb.org/tv/1396"),
            Some(Link::Tmdb { tv: true, id: 1396 })
        );
        assert_eq!(
            parse("Filme [tmdbid-77]"),
            Some(Link::Tmdb { tv: false, id: 77 })
        );
        assert_eq!(
            parse("https://www.imdb.com/title/tt0133093/?ref_=x"),
            Some(Link::Imdb("tt0133093".into()))
        );
        assert_eq!(parse("TT0133093"), Some(Link::Imdb("tt0133093".into())));
        assert_eq!(
            parse("https://comicvine.gamespot.com/batman/4050-796/"),
            Some(Link::ComicVine {
                issue: false,
                id: "796".into()
            })
        );
        assert_eq!(
            parse("comicvine.gamespot.com/batman-1/4000-6702/"),
            Some(Link::ComicVine {
                issue: true,
                id: "6702".into()
            })
        );
        assert_eq!(
            parse("https://metron.cloud/series/123/"),
            Some(Link::Metron("123".into()))
        );
        assert_eq!(parse("https://metron.cloud/series/batman-2016/"), None);
        assert_eq!(
            parse("https://www.comics.org/series/1234/"),
            Some(Link::Gcd("1234".into()))
        );
        assert_eq!(
            parse("https://mangadex.org/title/A1C7C817-4E59-43B7-9365-09675A149A6F/nome"),
            Some(Link::MangaDex(
                "a1c7c817-4e59-43b7-9365-09675a149a6f".into()
            ))
        );
        assert_eq!(
            parse("https://anilist.co/manga/30013/x"),
            Some(Link::AniList {
                anime: false,
                id: 30013
            })
        );
        assert_eq!(
            parse("https://www.mangaupdates.com/series/itbe2pb/nome"),
            Some(Link::MangaUpdates(
                u64::from_str_radix("itbe2pb", 36).unwrap().to_string()
            ))
        );
        assert_eq!(
            parse("https://www.mangaupdates.com/series.html?id=1234"),
            Some(Link::MangaUpdates("1234".into()))
        );
        assert_eq!(
            parse("https://openlibrary.org/works/OL45804W/Nome"),
            Some(Link::OpenLibrary("OL45804W".into()))
        );
        assert_eq!(
            parse("https://openlibrary.org/books/OL7353617M"),
            Some(Link::OpenLibrary("OL7353617M".into()))
        );
        assert_eq!(
            parse("https://musicbrainz.org/release/b84ee12a-09ef-421b-82de-0441a926375b"),
            Some(Link::MusicBrainz(
                "b84ee12a-09ef-421b-82de-0441a926375b".into()
            ))
        );
        assert_eq!(
            parse("https://www.discogs.com/release/249504-Nome"),
            Some(Link::Discogs("249504".into()))
        );
        assert_eq!(
            parse("https://www.discogs.com/pt_BR/release/249504"),
            Some(Link::Discogs("249504".into()))
        );
    }

    #[test]
    fn isbns_and_plain_text() {
        assert_eq!(
            parse("978-85-359-1484-9"),
            Some(Link::Isbn("9788535914849".into()))
        );
        assert_eq!(
            parse("ISBN 0-306-40615-2"),
            Some(Link::Isbn("9780306406157".into()))
        );
        assert_eq!(parse("1984"), None);
        assert_eq!(parse("O Livro 9788535914849"), None);
        assert_eq!(parse("Uma série qualquer"), None);
        assert_eq!(parse(""), None);
    }
}
