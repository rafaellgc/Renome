//! Movies from TMDB (themoviedb.org), without the network part: guessing
//! the title and year from a file name, ranking search results, building
//! the new name and the `.nfo` file that media servers read.

use std::collections::HashSet;
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
/// `{year}`, `{id}` (also in Portuguese and Spanish). Empty brackets left
/// by a missing year are removed.
pub fn movie_name(template: &str, m: &MovieMatch, colon: &str) -> String {
    let title = safe_title(&m.title, colon);
    let original = safe_title(&m.original_title, colon);
    let year = m.year.map(|y| y.to_string()).unwrap_or_default();
    let id = m.id.to_string();
    let out = crate::rules::expand(template, |key| match key {
        "title" | "titulo" | "título" => Some(title.as_str()),
        "original" | "original title" | "título original" | "titulo original" => {
            Some(original.as_str())
        }
        "year" | "ano" | "año" => Some(year.as_str()),
        "id" | "tmdbid" => Some(id.as_str()),
        _ => None,
    });
    let out = out.replace("()", "").replace("[]", "");
    text::collapse_spaces(&out).trim().to_string()
}

#[derive(Debug, Default, Deserialize)]
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
/// `.eng.forced`), kept when the movie or episode name replaces the rest.
/// Codes must be lowercase, so a title such as `No` is not taken for one.
pub fn subtitle_suffix<'a>(stem: &'a str, ext: &str) -> &'a str {
    if !is_subtitle(ext) {
        return "";
    }
    let mut start = stem.len();
    for _ in 0..3 {
        match stem[..start].rfind('.') {
            Some(dot) if dot > 0 && is_subtitle_tag(&stem[dot + 1..start]) => start = dot,
            _ => break,
        }
    }
    &stem[start..]
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
            id: 120,
            title: "O Senhor dos Anéis: A Sociedade do Anel".into(),
            original_title: "The Lord of the Rings".into(),
            year: Some(2001),
        };
        assert_eq!(
            movie_name("{título} ({ano}) [tmdbid-{id}]", &m, " - "),
            "O Senhor dos Anéis - A Sociedade do Anel (2001) [tmdbid-120]"
        );
        let m = MovieMatch {
            id: 7,
            title: "What About Bob? / AC/DC".into(),
            original_title: String::new(),
            year: None,
        };
        assert_eq!(
            movie_name("{title} ({year})", &m, " - "),
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
        assert_eq!(subtitle_suffix("Matrix.1999.pt", "mkv"), "");
        assert_eq!(subtitle_suffix("Movie.It", "srt"), "");
        assert_eq!(subtitle_suffix("pt", "srt"), "");
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
