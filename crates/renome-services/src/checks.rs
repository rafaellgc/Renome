//! The Test buttons of the options' keys and accounts (the command line's
//! `keys test` too): each service asked whether what was typed works, and
//! what is said when it does.

use crate::flow::after::Line;
use crate::{comicvine, discogs, gcd, googlebooks, metron, subtitles, tmdb};

/// The error when a setting the test needs is empty.
pub const UNSET: &str = "keyUnsetTest";

/// Tests the service `name` (`tmdb`, `google-books`, `comicvine`,
/// `metron`, `discogs`, `subdl`, `subsource`, `gcd…`, `opensubtitles…`)
/// with the settings `get` reads (keys trimmed, passwords as typed): what
/// to say when it works, else the service's error ([`UNSET`] for a
/// setting left empty).
pub async fn test(name: &str, get: &(dyn Fn(&str) -> String + Sync)) -> Result<Line, String> {
    let read = |field: &str| {
        let v = get(field);
        if field.ends_with("Password") {
            v
        } else {
            v.trim().to_string()
        }
    };
    let need = |field: &str| {
        let v = read(field);
        if v.is_empty() {
            Err(UNSET.to_string())
        } else {
            Ok(v)
        }
    };
    let said = |key| Line {
        key,
        n: 1,
        ..Line::default()
    };
    let done = match name {
        "tmdb" => tmdb::Client::new(&need("tmdbKey")?, "en-US").check().await,
        "google-books" => {
            googlebooks::Client::default()
                .check(&need("googleBooksKey")?)
                .await
        }
        "comicvine" => {
            comicvine::Client::default()
                .check(&need("comicVineKey")?)
                .await
        }
        "metron" => metron::Client::default().check(&need("metronToken")?).await,
        "discogs" => {
            discogs::Client::default()
                .check(&need("discogsToken")?)
                .await
        }
        "subdl" => subtitles::check_subdl(need("subdlKey")?).await,
        "subsource" => subtitles::check_subsource(need("subsourceKey")?).await,
        n if n.starts_with("gcd") => {
            return gcd::Client::default()
                .check(&need("gcdEmail")?, &need("gcdPassword")?)
                .await
                .map(|()| said("gcdAccountOk"));
        }
        _ => {
            let auth = subtitles::Auth {
                os_key: need("osKey")?,
                os_user: read("osUser"),
                os_password: read("osPassword"),
                ..subtitles::Auth::default()
            };
            return subtitles::check_os(auth).await.map(|left| match left {
                Some(n) => Line {
                    key: "osAccountOk",
                    n: usize::try_from(n).unwrap_or(0),
                    ..Line::default()
                },
                None => said("tmdbKeyOk"),
            });
        }
    };
    done.map(|()| said("tmdbKeyOk"))
}
