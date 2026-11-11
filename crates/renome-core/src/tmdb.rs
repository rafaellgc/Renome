//! Movies from TMDB (themoviedb.org), without the network part: guessing
//! the title and year from a file name, ranking search results, building
//! the new name and the `.nfo` file that media servers read.

use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use chrono::Datelike;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::text;

/// Title and year guessed from a file name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guess {
    pub title: String,
    pub year: Option<u16>,
    /// A TMDB id already in the name (`[tmdbid-603]`): no search needed.
    #[serde(default)]
    pub id: Option<u64>,
}

static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b((?:19|20)[0-9]{2})\b").unwrap());
static TMDB_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)tmdbid[-=](\d+)").unwrap());
pub(crate) static BRACKETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]*\]|\([^)]*\)|\{[^}]*\}").unwrap());
/// Release tags: whatever follows them is not part of the title.
pub(crate) static TAGS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(2160p|1080p|720p|576p|480p|4k|uhd|blu ?ray|brrip|bdrip|bdremux|remux|webrip|web-?dl|hdtv|hdrip|dvdrip|dvdscr|x26[45]|h ?26[45]|hevc|xvid|divx|hdr|hdr10\+?|dovi|10bit|aac|e?ac3|dts|ddp?5|atmos|truehd|amzn|dsnp|hmax|atvp|dual|dublado|legendado|nacional|extended|unrated|uncut|remastered|proper|repack|imax|multi|subbed|dubbed)\b",
    )
    .unwrap()
});

/// Guesses title and year from a file name without extension:
/// `O.Reino.das.Mares.2001.1080p.BluRay` → ("O Reino das Mares", 2001).
///
/// The year is the last plausible one with something before it, so
/// `2001 A Sea Voyage (1968)` and `1932.2020` work. Without a year,
/// the title stops at the first release tag (1080p, BluRay…).
pub fn guess(name: &str) -> Guess {
    let id = TMDB_ID.captures(name).and_then(|c| c[1].parse().ok());
    let spaced = name.replace(['.', '_'], " ");
    let max_year = chrono::Local::now().year() as u16 + 1;
    let year = YEAR
        .captures_iter(&spaced)
        .filter_map(|c| {
            let m = c.get(1)?;
            let y: u16 = m.as_str().parse().ok()?;
            let before = clean(&spaced[..m.start()]);
            (y <= max_year && !before.is_empty()).then_some((m.start(), y, before))
        })
        .last();
    match year {
        // Tags before the year (`Movie EXTENDED 2003`) are not the title either.
        Some((_, y, title)) => Guess {
            title: clean(&before_tags(&title)),
            year: Some(y),
            id,
        },
        None => Guess {
            title: clean(&before_tags(&spaced)),
            year: None,
            id,
        },
    }
}

/// The text before the first release tag that has something before it.
pub(crate) fn before_tags(s: &str) -> String {
    let cut = TAGS
        .find_iter(s)
        .map(|m| m.start())
        .find(|&i| !clean(&s[..i]).is_empty())
        .unwrap_or(s.len());
    s[..cut].to_string()
}

/// Drops bracketed parts and stray separators.
pub(crate) fn clean(s: &str) -> String {
    let s = BRACKETS.replace_all(s, " ");
    text::collapse_spaces(&s)
        .trim_matches(|c: char| c.is_whitespace() || "-–:,;([{".contains(c))
        .to_string()
}

/// A search result as the TMDB API returns it.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchResult {
    pub id: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub original_title: String,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub popularity: f64,
}

/// A movie offered to the user, best first.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: u64,
    pub title: String,
    pub original_title: String,
    pub year: Option<u16>,
    pub overview: String,
    pub poster_path: Option<String>,
    pub score: f64,
}

pub fn year_of(date: &Option<String>) -> Option<u16> {
    date.as_deref()?.get(..4)?.parse().ok()
}

fn words(s: &str) -> Vec<String> {
    text::strip_accents(&s.to_lowercase())
        .replace('&', " and ")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| roman(w).map_or_else(|| w.to_string(), |n| n.to_string()))
        .collect()
}

/// A Roman numeral up to 39 (`ii`, `xiv`), so "Part II" reads as "Part 2".
fn roman(w: &str) -> Option<u32> {
    const UNITS: [&str; 10] = ["", "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix"];
    let tens = w.len() - w.trim_start_matches('x').len();
    let unit = UNITS.iter().position(|u| *u == &w[tens..])?;
    let n = tens * 10 + unit;
    (tens <= 3 && n > 0).then_some(n as u32)
}

fn bigrams(s: &str) -> Vec<(char, char)> {
    let c: Vec<char> = s.chars().collect();
    c.windows(2).map(|w| (w[0], w[1])).collect()
}

/// Title similarity from 0 to 1, ignoring case, accents and punctuation.
pub fn similarity(a: &str, b: &str) -> f64 {
    let (wa, wb) = (words(a), words(b));
    if wa.is_empty() || wb.is_empty() {
        return 0.0;
    }
    if wa == wb {
        return 1.0;
    }
    let (ba, bb) = (bigrams(&wa.concat()), bigrams(&wb.concat()));
    if ba.is_empty() || bb.is_empty() {
        return 0.0;
    }
    let mut pool = bb.clone();
    let mut shared = 0;
    for g in &ba {
        if let Some(i) = pool.iter().position(|x| x == g) {
            pool.swap_remove(i);
            shared += 1;
        }
    }
    // Slightly below an exact match even when every bigram is shared.
    (2.0 * shared as f64 / (ba.len() + bb.len()) as f64).min(0.99)
}

/// Whether a folder is named after a movie: its TMDB id, or (almost) its
/// title with a year that is the movie's (or none).
pub fn names_movie(folder: &Path, m: &MovieMatch) -> bool {
    let name = folder.file_name().unwrap_or_default().to_string_lossy();
    let g = guess(&name);
    if g.id.is_some() {
        return g.id == Some(m.id);
    }
    let year_ok = match (g.year, m.year) {
        (Some(a), Some(b)) => a.abs_diff(b) <= 1,
        _ => true,
    };
    year_ok
        && [&m.title, &m.original_title]
            .iter()
            .any(|t| similarity(&g.title, t) >= 0.8)
}

fn year_penalty(guess: Option<u16>, year: Option<u16>) -> f64 {
    match (guess, year) {
        (None, _) => 0.0,
        (Some(_), None) => 0.25,
        (Some(g), Some(y)) => match g.abs_diff(y) {
            0 => 0.0,
            1 => 0.1, // years in file names are often off by one
            2 => 0.3,
            _ => 0.6,
        },
    }
}

/// Ranked candidates and whether the first one is a safe automatic pick:
/// (almost) the same title, the same year and no close second.
pub fn rank(guess: &Guess, results: Vec<SearchResult>) -> (Vec<Candidate>, bool) {
    let mut seen = HashSet::new();
    let mut out: Vec<(Candidate, f64)> = results
        .into_iter()
        .filter(|r| seen.insert(r.id))
        .map(|r| {
            let year = year_of(&r.release_date);
            let sim =
                similarity(&guess.title, &r.title).max(similarity(&guess.title, &r.original_title));
            // Popularity only breaks ties.
            let score = sim - year_penalty(guess.year, year)
                + (r.popularity.max(0.0).ln_1p() / 200.0).min(0.03);
            let c = Candidate {
                id: r.id,
                title: r.title,
                original_title: r.original_title,
                year,
                overview: r.overview.unwrap_or_default(),
                poster_path: r.poster_path,
                score,
            };
            (c, sim)
        })
        .collect();
    out.sort_by(|a, b| b.0.score.total_cmp(&a.0.score));
    let auto = match out.as_slice() {
        [] => false,
        [(best, sim), rest @ ..] => {
            let exact_year = guess.year.is_some() && best.year == guess.year;
            let clear = rest
                .first()
                .is_none_or(|(second, _)| second.score < best.score - 0.1);
            *sim >= 0.95 && clear && (exact_year || (guess.year.is_none() && *sim == 1.0))
        }
    };
    (out.into_iter().map(|(c, _)| c).collect(), auto)
}

/// The movie chosen for a file, as the rule needs it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieMatch {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub original_title: String,
    #[serde(default)]
    pub year: Option<u16>,
    /// What else TMDB tells (genre, country, director…), see
    /// [`crate::fields`]; with what the file's name tells (edition,
    /// source, release group).
    #[serde(default)]
    pub extra: crate::fields::Extra,
}

/// Makes a title safe for a file name: `:` becomes `colon` (" - " by
/// default), slashes become dashes, and other characters Windows refuses
/// are dropped.
pub fn safe_title(s: &str, colon: &str) -> String {
    safe_text(s, colon).trim_end_matches(['.', ' ']).to_string()
}

/// Like [`safe_title`], keeping final dots: in a file name the extension
/// follows them (`Na T.V..flac`).
pub fn safe_text(s: &str, colon: &str) -> String {
    let s = s.replace(" : ", ":").replace(": ", ":").replace(':', colon);
    let s: String = s
        .replace(['/', '\\'], "-")
        .replace('"', "'")
        .chars()
        .filter(|c| !"?*<>|".contains(*c))
        .collect();
    text::collapse_spaces(&s).trim_end().to_string()
}

/// The new name from the rule's template: `{title}`, `{original}`,
/// `{year}`, `{id}`, `{resolution}`, `{video codec}` (also in Portuguese
/// and Spanish). Empty brackets left by a missing year are removed, and so
/// is a dash left at an end.
pub fn movie_name(
    template: &str,
    m: &MovieMatch,
    colon: &str,
    video: Option<&crate::video::VideoInfo>,
) -> String {
    let title = safe_title(&m.title, colon);
    let original = safe_title(&m.original_title, colon);
    let year = m.year.map(|y| y.to_string()).unwrap_or_default();
    let id = m.id.to_string();
    let video = crate::video::Fields::of(video);
    let extra = crate::fields::safe(&m.extra, colon);
    let out = crate::rules::expand(template, |key| match key {
        "title" | "titulo" | "título" => Some(title.as_str()),
        "original" | "original title" | "título original" | "titulo original" => {
            Some(original.as_str())
        }
        "year" | "ano" | "año" => Some(year.as_str()),
        "id" | "tmdbid" => Some(id.as_str()),
        key => video.get(key).or_else(|| crate::fields::get(&extra, key)),
    });
    let out = out.replace("()", "").replace("[]", "");
    text::collapse_spaces(&out)
        .trim()
        .trim_end_matches([' ', '-', '–'])
        .trim_start_matches([' ', '-', '–'])
        .to_string()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Named {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct Cast {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub character: String,
    #[serde(default)]
    pub profile_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Crew {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub job: String,
    #[serde(default)]
    pub department: String,
    #[serde(default)]
    pub profile_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Collection {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct Keywords {
    #[serde(default)]
    pub keywords: Vec<Named>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Video {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub site: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub official: bool,
    #[serde(default)]
    pub iso_639_1: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Videos {
    #[serde(default)]
    pub results: Vec<Video>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Image {
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub iso_639_1: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Images {
    #[serde(default)]
    pub logos: Vec<Image>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Credits {
    #[serde(default)]
    pub cast: Vec<Cast>,
    #[serde(default)]
    pub crew: Vec<Crew>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Certification {
    #[serde(default)]
    pub certification: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct CountryRelease {
    #[serde(default)]
    pub iso_3166_1: String,
    #[serde(default)]
    pub release_dates: Vec<Certification>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ReleaseDates {
    #[serde(default)]
    pub results: Vec<CountryRelease>,
}

/// Movie details (`/movie/{id}` with credits and release dates).
#[derive(Debug, Default, Deserialize)]
pub struct Details {
    pub id: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub original_title: String,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub tagline: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub runtime: Option<u32>,
    #[serde(default)]
    pub vote_average: Option<f64>,
    #[serde(default)]
    pub vote_count: Option<u64>,
    #[serde(default)]
    pub genres: Vec<Named>,
    #[serde(default)]
    pub production_companies: Vec<Named>,
    #[serde(default)]
    pub production_countries: Vec<Named>,
    #[serde(default)]
    pub belongs_to_collection: Option<Collection>,
    #[serde(default)]
    pub imdb_id: Option<String>,
    #[serde(default)]
    pub original_language: String,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub credits: Credits,
    #[serde(default)]
    pub release_dates: ReleaseDates,
    #[serde(default)]
    pub keywords: Keywords,
    #[serde(default)]
    pub videos: Videos,
    #[serde(default)]
    pub images: Images,
}

impl Videos {
    /// The YouTube trailer, preferring the given language and official ones.
    pub fn trailer(&self, lang: &str) -> Option<&str> {
        self.results
            .iter()
            .filter(|v| v.site == "YouTube" && v.kind == "Trailer" && !v.key.is_empty())
            .max_by_key(|v| (v.iso_639_1.as_deref() == Some(lang), v.official))
            .map(|v| v.key.as_str())
    }
}

impl Images {
    /// The title logo (PNG), in the given language, else English, else any.
    pub fn logo(&self, lang: &str) -> Option<&str> {
        let png = || self.logos.iter().filter(|i| i.file_path.ends_with(".png"));
        png()
            .find(|i| i.iso_639_1.as_deref() == Some(lang))
            .or_else(|| png().find(|i| i.iso_639_1.as_deref() == Some("en")))
            .or_else(|| png().next())
            .map(|i| i.file_path.as_str())
    }
}

impl Details {
    pub fn trailer(&self, lang: &str) -> Option<&str> {
        self.videos.trailer(lang)
    }

    pub fn logo(&self, lang: &str) -> Option<&str> {
        self.images.logo(lang)
    }
}

/// A trailer in the form Kodi and Jellyfin play.
pub(crate) fn trailer_url(key: &str) -> String {
    format!("plugin://plugin.video.youtube/play/?video_id={key}")
}

pub(crate) fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Writes `<name>value</name>`, skipping empty values.
pub(crate) fn tag(o: &mut String, name: &str, value: &str) {
    if !value.is_empty() {
        o.push_str(&format!("  <{name}>{}</{name}>\n", xml(value)));
    }
}

/// A person (`<actor>`): Jellyfin reads each one's kind from `<type>`.
pub(crate) fn person(
    o: &mut String,
    name: &str,
    role: &str,
    kind: &str,
    order: usize,
    photo: &Option<String>,
) {
    o.push_str(&format!("  <actor>\n    <name>{}</name>\n", xml(name)));
    if !role.is_empty() {
        o.push_str(&format!("    <role>{}</role>\n", xml(role)));
    }
    o.push_str(&format!(
        "    <type>{kind}</type>\n    <order>{order}</order>\n"
    ));
    if let Some(p) = photo {
        o.push_str(&format!("    <thumb>{IMAGE_BASE}{}</thumb>\n", xml(p)));
    }
    o.push_str("  </actor>\n");
}

/// Crew members with a job, once each.
pub(crate) fn crew(all: &[Crew], pred: impl Fn(&Crew) -> bool) -> Vec<&Crew> {
    let mut seen = HashSet::new();
    all.iter()
        .filter(|c| pred(c) && seen.insert(c.name.as_str()))
        .collect()
}

/// The age rating in Jellyfin's TMDB form: `PG-13` in the US, `BR-12`
/// elsewhere.
/// The ids an `.nfo` gives: Kodi's, Jellyfin's and Emby's (`uniqueid`,
/// `tmdbid`, `imdbid`), or links to TMDB and IMDb in any text (a release's
/// notes).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct NfoIds {
    pub tmdb: Option<u64>,
    pub imdb: Option<String>,
}

impl NfoIds {
    pub fn any(&self) -> bool {
        self.tmdb.is_some() || self.imdb.is_some()
    }
}

/// Reads the ids of a movie (`root` = `movie`) or a show (`tvshow`) from
/// an `.nfo`'s text; a document about something else gives none of its
/// fields, only links of the right kind.
pub fn nfo_ids(text: &str, root: &str) -> NfoIds {
    static TMDB_LINK: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"themoviedb\.org/(movie|tv)/(\d+)").unwrap());
    static IMDB_LINK: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"imdb\.com/title/(tt\d{6,10})").unwrap());
    let mut ids = NfoIds::default();
    let text = text.trim_start_matches('\u{feff}');
    // Kodi allows a link after the document: the document is what comes
    // before it.
    let xml = text.rfind('>').map_or(text, |i| &text[..=i]);
    if let Ok(doc) = roxmltree::Document::parse(xml)
        && doc
            .root_element()
            .tag_name()
            .name()
            .eq_ignore_ascii_case(root)
    {
        for n in doc.root_element().children().filter(|n| n.is_element()) {
            let value = n.text().unwrap_or_default().trim();
            let name = n.tag_name().name().to_lowercase();
            let kind = n.attribute("type").unwrap_or_default().to_lowercase();
            match (name.as_str(), kind.as_str()) {
                ("uniqueid", "tmdb") | ("tmdbid", _) => {
                    ids.tmdb = ids.tmdb.or(value.parse().ok().filter(|&n| n > 0));
                }
                ("uniqueid", "imdb") | ("imdbid", _) | ("imdb_id", _) | ("id", _)
                    if value.starts_with("tt") =>
                {
                    ids.imdb.get_or_insert_with(|| value.to_string());
                }
                _ => {}
            }
        }
    }
    let kind = if root == "movie" { "movie" } else { "tv" };
    if ids.tmdb.is_none() {
        ids.tmdb = TMDB_LINK
            .captures_iter(text)
            .find(|c| &c[1] == kind)
            .and_then(|c| c[2].parse().ok());
    }
    if ids.imdb.is_none() {
        ids.imdb = IMDB_LINK.captures(text).map(|c| c[1].to_string());
    }
    ids
}

fn read_nfo(path: &Path, root: &str) -> NfoIds {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() && m.len() < 1 << 20 => {}
        _ => return NfoIds::default(),
    }
    std::fs::read(path)
        .map(|b| nfo_ids(&String::from_utf8_lossy(&b), root))
        .unwrap_or_default()
}

/// The ids of the movie a video (or a movie's folder) is, from the `.nfo`
/// beside it (`<name>.nfo`) or its folder's `movie.nfo`.
pub fn movie_nfo(path: &Path) -> NfoIds {
    if path.is_dir() {
        return read_nfo(&path.join("movie.nfo"), "movie");
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let dir = path.parent().unwrap_or(Path::new(""));
    let own = read_nfo(&dir.join(format!("{stem}.nfo")), "movie");
    if own.any() {
        return own;
    }
    read_nfo(&dir.join("movie.nfo"), "movie")
}

/// The ids of the show a folder holds, from its `tvshow.nfo` (or the one
/// of the folder above, for a season folder).
pub fn show_nfo(folder: &Path) -> NfoIds {
    let own = read_nfo(&folder.join("tvshow.nfo"), "tvshow");
    if own.any() {
        return own;
    }
    folder
        .parent()
        .map(|up| read_nfo(&up.join("tvshow.nfo"), "tvshow"))
        .unwrap_or_default()
}

/// The template fields a movie's details give (see [`crate::fields`]):
/// genre(s), country, language, runtime, the age rating of `lang`'s
/// country (`pt-BR`: Brazil's), director, studio, collection and release
/// date.
pub fn movie_extra(d: &Details, lang: &str) -> crate::fields::Extra {
    use crate::fields::add;
    let region = lang.split_once('-').map_or("US", |(_, r)| r);
    let mut e = crate::fields::Extra::new();
    let names = |v: &[Named], n: usize| {
        v.iter()
            .take(n)
            .map(|x| x.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    add(&mut e, "genre", &names(&d.genres, 1));
    add(&mut e, "genres", &names(&d.genres, 3));
    add(&mut e, "country", &names(&d.production_countries, 1));
    add(&mut e, "language", &d.original_language);
    add(&mut e, "runtime", &number(d.runtime));
    let cert = d
        .release_dates
        .results
        .iter()
        .find(|c| c.iso_3166_1.eq_ignore_ascii_case(region))
        .and_then(|c| c.release_dates.iter().find(|r| !r.certification.is_empty()))
        .map(|r| r.certification.as_str())
        .unwrap_or_default();
    add(&mut e, "certification", cert);
    let director = crew(&d.credits.crew, |c| c.job == "Director")
        .first()
        .map(|c| c.name.as_str())
        .unwrap_or_default();
    add(&mut e, "director", director);
    add(&mut e, "studio", &names(&d.production_companies, 1));
    if let Some(c) = &d.belongs_to_collection {
        add(&mut e, "collection", &c.name);
    }
    if let Some(date) = &d.release_date {
        crate::fields::add_date(&mut e, date);
    }
    e
}

pub(crate) fn rating_for(region: &str, cert: &str) -> String {
    if cert.is_empty() {
        String::new()
    } else if region.eq_ignore_ascii_case("US") {
        cert.to_string()
    } else {
        format!("{}-{cert}", region.to_uppercase())
    }
}

pub(crate) fn number<T: PartialOrd + Default + ToString>(v: Option<T>) -> String {
    v.filter(|x| *x > T::default())
        .map(|x| x.to_string())
        .unwrap_or_default()
}

/// A YouTube trailer's address, as players open it.
pub(crate) fn youtube(key: &str) -> String {
    format!("https://www.youtube.com/watch?v={key}")
}

/// TMDB's average as written (`7.8`), none when no one voted.
pub(crate) fn average(v: Option<f64>) -> String {
    v.filter(|&v| v > 0.0).map(|v| format!("{v:.1}")).unwrap_or_default()
}

/// What goes inside a movie's video (see [`crate::videotags`]): all its
/// `.nfo` has but pictures and the cast's photos. `lang` as in [`nfo`].
pub fn video_meta(d: &Details, lang: &str) -> crate::videotags::VideoMeta {
    use crate::videotags::{Person, VideoMeta};
    let (lang2, region) = lang.split_once('-').unwrap_or((lang, "US"));
    let names = |v: Vec<&Crew>| v.into_iter().map(|c| c.name.clone()).collect::<Vec<_>>();
    let cert = d
        .release_dates
        .results
        .iter()
        .find(|c| c.iso_3166_1.eq_ignore_ascii_case(region))
        .and_then(|c| c.release_dates.iter().find(|r| !r.certification.is_empty()))
        .map(|r| rating_for(region, &r.certification))
        .unwrap_or_default();
    VideoMeta {
        title: d.title.clone(),
        date: d.release_date.clone().unwrap_or_default(),
        description: d.overview.clone().unwrap_or_default(),
        genres: d.genres.iter().map(|g| g.name.clone()).collect(),
        imdb: d.imdb_id.clone().unwrap_or_default(),
        tmdb: format!("movie/{}", d.id),
        original_title: d.original_title.clone(),
        tagline: d.tagline.clone().unwrap_or_default(),
        certification: cert,
        rating: average(d.vote_average),
        keywords: d.keywords.keywords.iter().map(|k| k.name.clone()).collect(),
        countries: d.production_countries.iter().map(|c| c.name.clone()).collect(),
        studios: d.production_companies.iter().map(|c| c.name.clone()).collect(),
        directors: names(crew(&d.credits.crew, |c| c.job == "Director")),
        writers: names(crew(&d.credits.crew, |c| c.department == "Writing")),
        producers: names(crew(&d.credits.crew, |c| c.job == "Producer").into_iter().take(5).collect()),
        composers: names(crew(&d.credits.crew, |c| c.job == "Original Music Composer")),
        cast: d
            .credits
            .cast
            .iter()
            .take(20)
            .map(|a| Person { name: a.name.clone(), character: a.character.clone() })
            .collect(),
        collection: d.belongs_to_collection.as_ref().map(|c| c.name.clone()).unwrap_or_default(),
        trailer: d.trailer(lang2).map(youtube).unwrap_or_default(),
        ..Default::default()
    }
}

/// A movie's place in its collection, by release (those without a date
/// last): `parts` are the collection's movies, id and date.
pub fn collection_part(id: u64, parts: &[(u64, String)]) -> Option<u32> {
    let mut sorted: Vec<&(u64, String)> = parts.iter().collect();
    sorted.sort_by_key(|(i, d)| (d.is_empty(), d.clone(), *i));
    sorted.iter().position(|(i, _)| *i == id).map(|n| n as u32 + 1)
}

/// The `.nfo` file (Kodi format, read by Jellyfin, Kodi and Emby), with
/// every field Jellyfin's reader takes that TMDB has. `lang` is the
/// language of titles (`pt-BR`): its country picks the age rating, its
/// language the trailer.
pub fn nfo(d: &Details, lang: &str) -> String {
    let (lang2, region) = lang.split_once('-').unwrap_or((lang, "US"));
    let mut o =
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<movie>\n");
    let release = d.release_date.as_deref().unwrap_or_default();
    tag(&mut o, "title", &d.title);
    tag(&mut o, "originaltitle", &d.original_title);
    tag(
        &mut o,
        "year",
        &year_of(&d.release_date)
            .map(|y| y.to_string())
            .unwrap_or_default(),
    );
    tag(&mut o, "premiered", release);
    tag(&mut o, "releasedate", release);
    tag(&mut o, "plot", d.overview.as_deref().unwrap_or_default());
    tag(&mut o, "tagline", d.tagline.as_deref().unwrap_or_default());
    tag(&mut o, "runtime", &number(d.runtime));
    tag(
        &mut o,
        "rating",
        &d.vote_average
            .filter(|&v| v > 0.0)
            .map(|v| format!("{v:.1}"))
            .unwrap_or_default(),
    );
    tag(&mut o, "votes", &number(d.vote_count));
    let cert = d
        .release_dates
        .results
        .iter()
        .find(|c| c.iso_3166_1.eq_ignore_ascii_case(region))
        .and_then(|c| c.release_dates.iter().find(|r| !r.certification.is_empty()))
        .map(|r| rating_for(region, &r.certification))
        .unwrap_or_default();
    tag(&mut o, "mpaa", &cert);
    for g in &d.genres {
        tag(&mut o, "genre", &g.name);
    }
    for k in &d.keywords.keywords {
        tag(&mut o, "tag", &k.name);
    }
    for c in &d.production_countries {
        tag(&mut o, "country", &c.name);
    }
    for s in &d.production_companies {
        tag(&mut o, "studio", &s.name);
    }
    if let Some(key) = d.trailer(lang2) {
        tag(&mut o, "trailer", &trailer_url(key));
    }
    if let Some(set) = &d.belongs_to_collection {
        o.push_str(&format!(
            "  <set tmdbcolid=\"{}\">\n    <name>{}</name>\n  </set>\n",
            set.id,
            xml(&set.name)
        ));
    }
    for c in crew(&d.credits.crew, |c| c.job == "Director") {
        tag(&mut o, "director", &c.name);
    }
    for c in crew(&d.credits.crew, |c| c.department == "Writing") {
        tag(&mut o, "writer", &c.name);
    }
    tag(&mut o, "tmdbid", &d.id.to_string());
    tag(&mut o, "imdbid", d.imdb_id.as_deref().unwrap_or_default());
    o.push_str(&format!(
        "  <uniqueid type=\"tmdb\" default=\"true\">{}</uniqueid>\n",
        d.id
    ));
    if let Some(imdb) = d.imdb_id.as_deref().filter(|s| !s.is_empty()) {
        o.push_str(&format!(
            "  <uniqueid type=\"imdb\">{}</uniqueid>\n",
            xml(imdb)
        ));
    }
    for (i, a) in d.credits.cast.iter().take(20).enumerate() {
        person(&mut o, &a.name, &a.character, "Actor", i, &a.profile_path);
    }
    for (i, c) in crew(&d.credits.crew, |c| c.job == "Producer")
        .iter()
        .take(5)
        .enumerate()
    {
        person(&mut o, &c.name, "", "Producer", i, &c.profile_path);
    }
    for (i, c) in crew(&d.credits.crew, |c| c.job == "Original Music Composer")
        .iter()
        .enumerate()
    {
        person(&mut o, &c.name, "", "Composer", i, &c.profile_path);
    }
    o.push_str("</movie>\n");
    o
}

/// The language and flags at the end of a subtitle's name (`.pt-BR`,
/// `.eng.forced`, also `-forced` or ` SDH` as some name them), as written:
/// what the movie or episode name keeps (see [`subtitle_tags`]). Codes
/// must be lowercase, so a title such as `No` is not taken for one; a flag
/// may follow a dash, an underscore or a space, a language only a dot.
pub fn subtitle_suffix<'a>(stem: &'a str, ext: &str) -> &'a str {
    if !is_subtitle(ext) {
        return "";
    }
    let mut start = stem.len();
    for _ in 0..3 {
        let head = &stem[..start];
        // A language or flag after a dot (`.pt-BR`), else a flag after a
        // dash, an underscore or a space (`-forced`).
        let dotted = head
            .rfind('.')
            .filter(|&at| at > 0 && is_subtitle_tag(&head[at + 1..]));
        let other = head
            .rfind(['-', '_', ' '])
            .filter(|&at| at > 0 && is_subtitle_flag(&head[at + 1..]));
        match dotted.into_iter().chain(other).max() {
            Some(at) => start = at,
            None => break,
        }
    }
    &stem[start..]
}

/// The suffix as players read it (`Movie.pt-BR.forced.srt`): each part
/// after a dot, flags in lower case.
pub fn subtitle_tags(stem: &str, ext: &str) -> String {
    let raw = subtitle_suffix(stem, ext);
    let mut out = String::new();
    let mut rest = raw;
    while !rest.is_empty() {
        let sep = rest.starts_with('.');
        let body = &rest[1..];
        let end = body.find(['.', '-', '_', ' ']).unwrap_or(body.len());
        // A language's region (`pt-BR`) keeps its dash.
        let (part, next) = if sep {
            let (lang, more) = (&body[..end], &body[end..]);
            match more.strip_prefix(['-', '_']) {
                Some(r)
                    if !is_subtitle_flag(r.split(['.', '-', '_', ' ']).next().unwrap_or(""))
                        && is_subtitle_tag(
                            &body[..end + 1 + r.find(['.', '-', '_', ' ']).unwrap_or(r.len())],
                        ) =>
                {
                    let len = end + 1 + r.find(['.', '-', '_', ' ']).unwrap_or(r.len());
                    (body[..len].to_string(), &body[len..])
                }
                _ => (lang.to_string(), more),
            }
        } else {
            (body[..end].to_string(), &body[end..])
        };
        out.push('.');
        if is_subtitle_flag(&part) {
            out.push_str(&part.to_lowercase());
        } else {
            out.push_str(&part);
        }
        rest = next;
    }
    out
}

/// Flags that may follow a dash, an underscore or a space too: words a
/// title rarely ends with (`default`, `full`, `hi` only after a dot).
fn is_subtitle_flag(s: &str) -> bool {
    const FLAGS: &[&str] = &["forced", "sdh", "cc", "foreign"];
    FLAGS.contains(&s.to_lowercase().as_str())
}

fn is_subtitle_tag(s: &str) -> bool {
    const FLAGS: &[&str] = &["forced", "sdh", "cc", "default", "full"];
    const LANGS: &[&str] = &[
        "pt", "en", "es", "fr", "de", "it", "ja", "ko", "zh", "ru", "nl", "sv", "no", "nb", "da",
        "fi", "pl", "tr", "ar", "he", "hi", "el", "cs", "hu", "ro", "uk", "th", "vi", "id", "ms",
        "ca", "eu", "gl", "hr", "sr", "bg", "sk", "sl", "lt", "lv", "et", "fa", "por", "pob",
        "eng", "spa", "fre", "fra", "ger", "deu", "ita", "jpn", "kor", "chi", "zho", "rus", "dut",
        "nld", "swe", "nor", "dan", "fin", "pol", "tur", "ara", "heb", "gre", "ell", "cze", "ces",
        "hun", "rum", "ron", "ukr", "tha", "vie", "ind", "cat",
    ];
    let (lang, region) = s.split_once(['-', '_']).unwrap_or((s, ""));
    FLAGS.contains(&s)
        || is_subtitle_flag(s)
        || (LANGS.contains(&lang)
            && (region.is_empty()
                || ((2..=4).contains(&region.len())
                    && region.chars().all(|c| c.is_ascii_alphanumeric()))))
}

pub fn is_subtitle(ext: &str) -> bool {
    const SUBTITLES: &[&str] = &["srt", "ass", "ssa", "sub", "idx", "vtt", "sup", "smi"];
    SUBTITLES.contains(&ext.to_lowercase().as_str())
}

/// The files the movie and TV show rules name: videos and their subtitles
/// (posters, `.nfo` and other files are left alone).
pub fn is_screen(ext: &str) -> bool {
    is_video(ext) || is_subtitle(ext)
}

pub const IMAGE_BASE: &str = "https://image.tmdb.org/t/p/original";

/// Video extensions that get `.nfo` and images (subtitles and other files
/// that share the movie's name do not).
pub fn is_video(ext: &str) -> bool {
    const VIDEO: &[&str] = &[
        "mkv", "mp4", "m4v", "avi", "mov", "wmv", "ts", "m2ts", "mts", "webm", "mpg", "mpeg",
        "flv", "ogv", "iso", "vob", "divx", "rmvb",
    ];
    VIDEO.contains(&ext.to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(name: &str) -> (String, Option<u16>) {
        let x = guess(name);
        (x.title, x.year)
    }

    #[test]
    fn guesses() {
        let s = |t: &str, y: Option<u16>| (t.to_string(), y);
        assert_eq!(
            g("O.Senhor.dos.Aneis.A.Sociedade.do.Anel.2001.1080p.BluRay.x264-GRP"),
            s("O Senhor dos Aneis A Sociedade do Anel", Some(2001))
        );
        assert_eq!(g("Matrix (1999) [1080p]"), s("Matrix", Some(1999)));
        assert_eq!(
            g("2001 A Space Odyssey (1968)"),
            s("2001 A Space Odyssey", Some(1968))
        );
        assert_eq!(g("1917.2019.2160p.UHD"), s("1917", Some(2019)));
        assert_eq!(
            g("Blade.Runner.2049.2017"),
            s("Blade Runner 2049", Some(2017))
        );
        assert_eq!(
            g("[Grupo] Cidade de Deus - 2002"),
            s("Cidade de Deus", Some(2002))
        );
        assert_eq!(g("Charlottes.Web.2006"), s("Charlottes Web", Some(2006)));
        assert_eq!(
            g("Up.Altas.Aventuras.1080p.WEB-DL.DUAL"),
            s("Up Altas Aventuras", None)
        );
        assert_eq!(g("1917"), s("1917", None));
        assert_eq!(g("Amelie"), s("Amelie", None));
        assert_eq!(
            g("The.Lord.of.the.Rings.The.Return.of.the.King.EXTENDED.2003"),
            s("The Lord of the Rings The Return of the King", Some(2003))
        );
        let x = guess("Matrix (1999) [tmdbid-603]");
        assert_eq!(
            (x.title.as_str(), x.year, x.id),
            ("Matrix", Some(1999), Some(603))
        );
    }

    fn result(id: u64, title: &str, date: &str, pop: f64) -> SearchResult {
        SearchResult {
            id,
            title: title.into(),
            original_title: title.into(),
            release_date: Some(date.into()),
            overview: None,
            poster_path: None,
            popularity: pop,
        }
    }

    #[test]
    fn ranking() {
        let guess = Guess {
            title: "Halloween".into(),
            year: Some(2018),
            id: None,
        };
        let (c, auto) = rank(
            &guess,
            vec![
                result(1, "Halloween", "1978-10-25", 90.0),
                result(2, "Halloween", "2018-10-18", 40.0),
                result(3, "Halloween", "2007-08-31", 30.0),
                result(2, "Halloween", "2018-10-18", 40.0),
            ],
        );
        assert_eq!(c[0].id, 2);
        assert!(auto);

        // Year off by one: first, but the user decides.
        let guess = Guess {
            title: "O Senhor dos Aneis A Sociedade do Anel".into(),
            year: Some(2002),
            id: None,
        };
        let mut lotr = result(
            120,
            "O Senhor dos Anéis: A Sociedade do Anel",
            "2001-12-18",
            50.0,
        );
        lotr.original_title = "The Lord of the Rings: The Fellowship of the Ring".into();
        let (c, auto) = rank(&guess, vec![result(9, "Outro", "2002-01-01", 99.0), lotr]);
        assert_eq!(c[0].id, 120);
        assert!(!auto);
    }

    #[test]
    fn names() {
        let m = MovieMatch {
            extra: Default::default(),
            id: 120,
            title: "O Senhor dos Anéis: A Sociedade do Anel".into(),
            original_title: "The Lord of the Rings".into(),
            year: Some(2001),
        };
        assert_eq!(
            movie_name("{título} ({ano}) [tmdbid-{id}]", &m, " - ", None),
            "O Senhor dos Anéis - A Sociedade do Anel (2001) [tmdbid-120]"
        );
        let m = MovieMatch {
            extra: Default::default(),
            id: 7,
            title: "What About Bob? / AC/DC".into(),
            original_title: String::new(),
            year: None,
        };
        assert_eq!(
            movie_name("{title} ({year})", &m, " - ", None),
            "What About Bob - AC-DC"
        );
        // Versions by picture size, as Jellyfin names them.
        let uhd = crate::video::VideoInfo {
            width: 3840,
            height: 1608,
            codec: "HEVC".into(),
        };
        assert_eq!(
            movie_name("{título} - {resolução}", &m, " - ", Some(&uhd)),
            "What About Bob - AC-DC - 2160p"
        );
        assert_eq!(
            movie_name("{título} - {resolução}", &m, " - ", None),
            "What About Bob - AC-DC"
        );
    }

    #[test]
    fn subtitle_suffixes() {
        assert_eq!(roman("ii"), Some(2));
        assert_eq!(roman("xiv"), Some(14));
        assert_eq!(roman("xxxix"), Some(39));
        assert_eq!(roman("iiii"), None);
        assert_eq!(roman("mix"), None);
        assert_eq!(similarity("Part II", "Part 2"), 1.0);
        assert_eq!(subtitle_suffix("Matrix.1999.pt-BR", "srt"), ".pt-BR");
        assert_eq!(
            subtitle_suffix("Matrix.1999.eng.forced", "srt"),
            ".eng.forced"
        );
        // Flags after a dash or a space, as some name them.
        let name = "Alice (2010) [tmdbid-12155]";
        let t = |stem: &str| subtitle_tags(stem, "srt");
        assert_eq!(t(&format!("{name}-forced")), ".forced");
        assert_eq!(t(&format!("{name}.pt-BR-forced")), ".pt-BR.forced");
        assert_eq!(t(&format!("{name}.pt-BR.FORCED")), ".pt-BR.forced");
        assert_eq!(t(&format!("{name} SDH")), ".sdh");
        assert_eq!(t(&format!("{name}.eng.forced")), ".eng.forced");
        assert_eq!(t(&format!("{name}.pt-BR")), ".pt-BR");
        assert_eq!(t(name), "");
        assert_eq!(t("Say Hi"), "");
        assert_eq!(t("Uma Coisa Default"), "");
        assert_eq!(t("Movie.default"), ".default");
        assert_eq!(subtitle_suffix("Matrix.1999.pt", "mkv"), "");
        assert_eq!(subtitle_suffix("Movie.It", "srt"), "");
        assert_eq!(subtitle_suffix("pt", "srt"), "");
    }

    #[test]
    fn ids_from_nfo() {
        let kodi = r#"<?xml version="1.0"?><movie><title>X</title>
            <uniqueid type="imdb">tt0000001</uniqueid><uniqueid type="tmdb" default="true">42</uniqueid></movie>"#;
        assert_eq!(
            nfo_ids(kodi, "movie"),
            NfoIds {
                tmdb: Some(42),
                imdb: Some("tt0000001".into())
            }
        );
        // A show's document gives nothing to a movie.
        let show = r#"<tvshow><uniqueid type="tmdb">7</uniqueid></tvshow>"#;
        assert_eq!(nfo_ids(show, "movie"), NfoIds::default());
        assert_eq!(nfo_ids(show, "tvshow").tmdb, Some(7));
        // A document and a link after it; plain notes with links.
        let linked = "<movie><title>X</title></movie>\nhttps://www.themoviedb.org/movie/99-x";
        assert_eq!(nfo_ids(linked, "movie").tmdb, Some(99));
        let notes = "RELEASE NOTES\n  imdb: https://www.imdb.com/title/tt1234567/\n";
        assert_eq!(nfo_ids(notes, "movie").imdb.as_deref(), Some("tt1234567"));
        assert_eq!(
            nfo_ids("https://www.themoviedb.org/tv/5", "movie").tmdb,
            None
        );
    }

    #[test]
    fn nfo_file() {
        let d: Details = serde_json::from_str(
            r#"{"id":120,"title":"O Senhor & os Anéis","original_title":"LOTR","release_date":"2001-12-18",
                "runtime":178,"vote_average":8.42,"genres":[{"name":"Aventura"}],"imdb_id":"tt0120737",
                "credits":{"cast":[{"name":"Elijah Wood","character":"Frodo","profile_path":"/x.jpg"}],
                           "crew":[{"name":"Peter Jackson","job":"Director","department":"Directing"},
                                   {"name":"Fran Walsh","job":"Screenplay","department":"Writing"},
                                   {"name":"Fran Walsh","job":"Novel","department":"Writing"},
                                   {"name":"Howard Shore","job":"Original Music Composer","department":"Sound"}]},
                "belongs_to_collection":{"id":119,"name":"Senhor dos Anéis"},
                "keywords":{"keywords":[{"name":"elves"}]},
                "videos":{"results":[{"key":"en1","site":"YouTube","type":"Trailer","iso_639_1":"en","official":true},
                                     {"key":"pt1","site":"YouTube","type":"Trailer","iso_639_1":"pt"}]},
                "images":{"logos":[{"file_path":"/en.png","iso_639_1":"en"},{"file_path":"/pt.svg","iso_639_1":"pt"}]},
                "release_dates":{"results":[{"iso_3166_1":"BR","release_dates":[{"certification":""},{"certification":"12"}]}]}}"#,
        )
        .unwrap();
        // The same details, for inside the video.
        let m = video_meta(&d, "pt-BR");
        assert_eq!((m.title.as_str(), m.original_title.as_str(), m.tmdb.as_str()), ("O Senhor & os Anéis", "LOTR", "movie/120"));
        assert_eq!((m.certification.as_str(), m.rating.as_str()), ("BR-12", "8.4"));
        assert_eq!(m.directors, ["Peter Jackson"]);
        assert_eq!(m.writers, ["Fran Walsh"]);
        assert_eq!(m.composers, ["Howard Shore"]);
        assert_eq!((m.cast[0].name.as_str(), m.cast[0].character.as_str()), ("Elijah Wood", "Frodo"));
        assert_eq!(m.collection, "Senhor dos Anéis");
        assert_eq!(m.keywords, ["elves"]);
        assert_eq!(m.trailer, "https://www.youtube.com/watch?v=pt1");
        assert_eq!(
            collection_part(120, &[(122, "2003-12-17".into()), (121, "2002-12-18".into()), (120, "2001-12-18".into()), (9, String::new())]),
            Some(1)
        );
        assert_eq!(collection_part(9, &[(9, String::new()), (121, "2002-12-18".into())]), Some(2));
        let x = nfo(&d, "pt-BR");
        for want in [
            "<title>O Senhor &amp; os Anéis</title>",
            "<year>2001</year>",
            "<runtime>178</runtime>",
            "<rating>8.4</rating>",
            "<genre>Aventura</genre>",
            "<director>Peter Jackson</director>",
            "<mpaa>BR-12</mpaa>",
            "<uniqueid type=\"tmdb\" default=\"true\">120</uniqueid>",
            "<uniqueid type=\"imdb\">tt0120737</uniqueid>",
            "<role>Frodo</role>",
            "<type>Actor</type>",
            "<writer>Fran Walsh</writer>",
            "<type>Composer</type>",
            "<set tmdbcolid=\"119\">",
            "<tag>elves</tag>",
            "<trailer>plugin://plugin.video.youtube/play/?video_id=pt1</trailer>",
            "<releasedate>2001-12-18</releasedate>",
        ] {
            assert!(x.contains(want), "missing {want} in\n{x}");
        }
        assert_eq!(x.matches("<writer>").count(), 1);
        // The Portuguese logo is an SVG: the English PNG wins.
        assert_eq!(d.logo("pt"), Some("/en.png"));
    }
}
