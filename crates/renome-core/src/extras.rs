//! What sits next to a movie in a downloaded release besides the movie
//! itself: its extras (a making-of, trailers, deleted scenes), which media
//! servers show with the movie when they are in a folder of the right name,
//! and what the release's makers added to advertise themselves (link
//! shortcuts, a "read me" full of links, a short video named after a site),
//! which is no part of the movie.

use std::io::Read;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

/// The folders Jellyfin and Plex look for extras in, next to the movie.
pub const BEHIND_THE_SCENES: &str = "behind the scenes";
pub const TRAILERS: &str = "trailers";
pub const FEATURETTES: &str = "featurettes";
pub const DELETED_SCENES: &str = "deleted scenes";
pub const INTERVIEWS: &str = "interviews";
pub const SCENES: &str = "scenes";
pub const SHORTS: &str = "shorts";
pub const EXTRAS: &str = "extras";

/// Lower case, without accents, every run of separators one space.
fn plain(s: &str) -> String {
    let s = crate::text::strip_accents(s).to_lowercase();
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The extras folder a folder of this name stands for (`Making Of`,
/// `Bastidores`, `Trailers`, `Cenas deletadas`).
fn folder_kind(name: &str) -> Option<&'static str> {
    Some(match plain(name).as_str() {
        "making of"
        | "makingof"
        | "behind the scenes"
        | "bastidores"
        | "detras de camaras"
        | "detras de las camaras" => BEHIND_THE_SCENES,
        "trailers" | "trailer" | "trailers e teasers" | "teasers" => TRAILERS,
        "featurettes" | "featurette" => FEATURETTES,
        "deleted scenes" | "cenas deletadas" | "cenas excluidas" | "escenas eliminadas"
        | "escenas borradas" => DELETED_SCENES,
        "interviews" | "entrevistas" => INTERVIEWS,
        "extras" | "bonus" | "bonus features" | "special features" | "especiais"
        | "materiais extras" | "material extra" | "contenido extra" => EXTRAS,
        _ => return None,
    })
}

/// The extras folder a video with this name (without its extension) goes
/// to: Jellyfin's and Plex's own endings (`Movie-trailer`), or a name that
/// starts with "making of" followed by more (`Making.of.Movie.2017`).
fn name_kind(stem: &str) -> Option<&'static str> {
    if let Some((_, end)) = stem.rsplit_once('-') {
        let kind = match end.trim().to_lowercase().as_str() {
            "behindthescenes" => Some(BEHIND_THE_SCENES),
            "trailer" => Some(TRAILERS),
            "featurette" => Some(FEATURETTES),
            "deleted" | "deletedscene" => Some(DELETED_SCENES),
            "interview" => Some(INTERVIEWS),
            "scene" => Some(SCENES),
            "short" => Some(SHORTS),
            "extra" | "other" => Some(EXTRAS),
            _ => None,
        };
        if kind.is_some() {
            return kind;
        }
    }
    let p = plain(stem);
    let rest = p
        .strip_prefix("making of ")
        .or_else(|| p.strip_prefix("bastidores de "))?;
    rest.chars()
        .any(char::is_alphabetic)
        .then_some(BEHIND_THE_SCENES)
}

/// The extras folder a video goes to, when it is a movie's extra: by its
/// name, or by the folder it is in.
pub fn extra_kind(path: &Path) -> Option<&'static str> {
    if path.is_dir() {
        return None;
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy())
        .unwrap_or_default();
    if !crate::tmdb::is_video(&ext) {
        return None;
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    name_kind(&stem).or_else(|| {
        let dir = path.parent()?.file_name()?.to_string_lossy();
        folder_kind(&dir)
    })
}

/// A name that is a site's address (`COMANDOTORRENTS.COM`, `www.site.to`).
static SITE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[^a-z0-9])(?:www[.\s]|[a-z0-9][a-z0-9-]*\.(?:com|net|org|to|site|biz|info|tv|me|cc|io|xyz|top|club|online|vip|link|lol|app|in|ws|br)(?:$|[^a-z0-9]))",
    )
    .unwrap()
});

/// A name that is nothing but a site's address, at most with a word or two
/// around it (`COMANDOTORRENTS.COM`, `BLUDV.TO`, `TorrentDosFilmes.SITE`).
fn site_name(stem: &str) -> bool {
    SITE.is_match(stem) && plain(stem).split(' ').count() <= 4
}

/// The usual names of a release's "read me".
fn read_me(stem: &str) -> bool {
    matches!(
        plain(stem).as_str(),
        "leia"
            | "leia me"
            | "leiame"
            | "readme"
            | "read me"
            | "info"
            | "informacoes"
            | "leeme"
            | "lee me"
            | "lea me"
            | "lisez moi"
    )
}

/// Text with links in it, from its first 64 KiB.
fn has_links(path: &Path) -> bool {
    let Ok(f) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = Vec::new();
    if f.take(64 * 1024).read_to_end(&mut buf).is_err() {
        return false;
    }
    let text = String::from_utf8_lossy(&buf);
    text.contains("http://") || text.contains("https://") || SITE.is_match(&text)
}

/// A release's how-to about torrents (`Como Configurar o Utorrent.txt`,
/// `O Que é SEMEAR.txt`, `How to seed.txt`): named after them, or a text
/// about torrents and seeding.
fn torrent_howto(path: &Path, stem: &str) -> bool {
    let name = stem.to_lowercase();
    if ["torrent", "semear", "seed", "seeder", "semeie"]
        .iter()
        .any(|w| name.contains(w))
    {
        return true;
    }
    let Ok(f) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = Vec::new();
    if f.take(64 * 1024).read_to_end(&mut buf).is_err() {
        return false;
    }
    let text = String::from_utf8_lossy(&buf).to_lowercase();
    text.contains("torrent")
        && ["semear", "semeie", "seed", "seeder", "upload"]
            .iter()
            .any(|w| text.contains(w))
}

/// Largest video taken for an advertisement (a real movie is far larger).
const AD_VIDEO: u64 = 100 * 1024 * 1024;

/// What a release's makers add to advertise themselves: link shortcuts
/// (`.url`), a "read me" with links (`LEIA-ME.txt`), a how-to about
/// torrents, and a short video or an image named after a site
/// (`COMANDOTORRENTS.COM.mp4`). A movie's
/// extra is never taken for one.
pub fn release_junk(path: &Path, size: u64) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    match ext.as_str() {
        "url" | "webloc" | "website" => true,
        "txt" => {
            site_name(&stem) || (read_me(&stem) && has_links(path)) || torrent_howto(path, &stem)
        }
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" => site_name(&stem),
        e if crate::tmdb::is_video(e) => {
            size < AD_VIDEO && site_name(&stem) && extra_kind(path).is_none()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn extras_by_folder_and_name() {
        let d = tempfile::tempdir().unwrap();
        let p = |s: &str| d.path().join(s);
        assert_eq!(
            extra_kind(&p("Making Of/Making.of.Filme.2017.1080p.mkv")),
            Some(BEHIND_THE_SCENES)
        );
        assert_eq!(extra_kind(&p("Filme/qualquer.mkv")), None);
        assert_eq!(
            extra_kind(&p("Cenas Deletadas/a.mkv")),
            Some(DELETED_SCENES)
        );
        assert_eq!(extra_kind(&p("Filme-trailer.mp4")), Some(TRAILERS));
        assert_eq!(
            extra_kind(&p("Making.of.Filme.mkv")),
            Some(BEHIND_THE_SCENES)
        );
        // A movie called just that is no extra.
        assert_eq!(extra_kind(&p("Making Of (2006).mkv")), None);
        // Only videos.
        assert_eq!(extra_kind(&p("Trailers/notes.txt")), None);
        // A dash inside a title.
        assert_eq!(extra_kind(&p("Spider-Man.mkv")), None);
    }

    #[test]
    fn release_advertisements() {
        let d = tempfile::tempdir().unwrap();
        let w = |name: &str, body: &str| {
            let p = d.path().join(name);
            fs::write(&p, body).unwrap();
            p
        };
        assert!(release_junk(
            &w("GRUPO TELEGRAM.url", "[InternetShortcut]"),
            30
        ));
        assert!(release_junk(
            &w("LEIA-ME.txt", "Visite https://exemplo.site"),
            30
        ));
        assert!(release_junk(&w("LEIA.txt", "baixe em www.exemplo.com"), 30));
        // A note of one's own, even called that, has no links.
        assert!(!release_junk(&w("leia.txt", "comprar pipoca"), 30));
        assert!(!release_junk(&w("notas.txt", "https://exemplo.com"), 30));
        assert!(release_junk(&w("EXEMPLO.COM.txt", "x"), 30));
        let ad = w("COMANDOTORRENTS.COM.mp4", "x");
        assert!(release_junk(&ad, 20 * 1024 * 1024));
        // As large as a movie: never.
        assert!(!release_junk(&ad, 2 * 1024 * 1024 * 1024));
        // A movie's name with the site at the end is a movie, not an ad.
        let movie = w("Filme.2017.1080p.WEB-DL.x264.NAC.WWW.EXEMPLO.COM.mkv", "x");
        assert!(!release_junk(&movie, 20 * 1024 * 1024));
        assert!(release_junk(&w("EXEMPLO.COM.png", "x"), 30));
        assert!(!release_junk(&w("poster.jpg", "x"), 30));
        // How-tos about torrents.
        assert!(release_junk(
            &w("Como Configurar o Utorrent ou Bittorrent.txt", "x"),
            30
        ));
        assert!(release_junk(&w("O Que é SEMEAR.txt", "x"), 30));
        assert!(release_junk(
            &w(
                "Leia antes.txt",
                "Um torrent precisa de quem fique semeando: semear"
            ),
            30
        ));
        assert!(!release_junk(
            &w("sinopse.txt", "Um policial vai para o interior."),
            30
        ));
    }
}
