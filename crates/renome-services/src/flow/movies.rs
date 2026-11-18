//! The movie rule: videos, their subtitles and folders are looked up on
//! TMDB; a sure first result is taken, the rest are doubts.

use super::Item;
use crate::{Host, tmdb};
use renome_core::tmdb::{Candidate, Guess};
use serde::Serialize;
use std::path::PathBuf;

/// What was found for a file.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub path: PathBuf,
    pub guess: Guess,
    pub candidates: Vec<Candidate>,
    /// Picked with confidence; `None`: a doubt.
    pub choice: Option<Candidate>,
    pub error: Option<String>,
    /// Doubts of one group share one answer (a movie and its subtitles).
    pub group: String,
}

/// The files the movie rule looks up: videos, subtitles and folders, not
/// a movie's extras or a release's adverts.
pub fn targets(items: &[Item]) -> Vec<&Item> {
    items
        .iter()
        .filter(|i| {
            (i.is_dir || renome_core::tmdb::is_screen(&i.ext()))
                && i.extra.is_none()
                && !i.release_junk
        })
        .collect()
}

/// The doubts sharing an answer: same guessed title and year.
fn group(guess: &Guess) -> String {
    format!(
        "{}\u{1f}{}",
        guess.title,
        guess.year.map(|y| y.to_string()).unwrap_or_default()
    )
}

/// Looks up the movie files of `items` (see [`targets`]). An error when
/// nothing could really be searched (a bad key, no connection); files not
/// reached before Stop are left out.
pub async fn search(
    host: &dyn Host,
    client: &tmdb::Client,
    items: &[Item],
) -> Result<Vec<Found>, String> {
    let wanted: Vec<tmdb::LookupItem> = targets(items)
        .into_iter()
        .map(|i| tmdb::LookupItem {
            path: i.path.clone(),
            name: i.read_stem(),
        })
        .collect();
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let found = tmdb::lookup(host, client, wanted).await;
    if let Some(failed) = found.iter().find_map(|f| f.error.clone())
        && found
            .iter()
            .all(|f| f.error.is_some() || f.guess.title.is_empty())
    {
        return Err(failed);
    }
    Ok(found
        .into_iter()
        .map(|f| Found {
            group: group(&f.guess),
            choice: if f.auto {
                f.candidates.first().cloned()
            } else {
                None
            },
            path: f.path,
            guess: f.guess,
            candidates: f.candidates,
            error: f.error,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> Item {
        Item {
            path: PathBuf::from(format!("/x/{name}")),
            original: name.into(),
            lookup_name: None,
            is_dir: false,
            extra: None,
            release_junk: false,
            untagged: false,
            convert: false,
        }
    }

    #[test]
    fn only_videos_subtitles_and_folders() {
        let mut extra = item("Making of.mkv");
        extra.extra = Some("Featurettes".into());
        let mut ad = item("Visit us.mp4");
        ad.release_junk = true;
        let mut dir = item("A Film (2001)");
        dir.is_dir = true;
        let items = vec![
            item("A Film.mkv"),
            item("A Film.srt"),
            item("cover.jpg"),
            extra,
            ad,
            dir,
        ];
        let names: Vec<&str> = targets(&items)
            .iter()
            .map(|i| i.original.as_str())
            .collect();
        assert_eq!(names, ["A Film.mkv", "A Film.srt", "A Film (2001)"]);
    }

    #[test]
    fn a_film_and_its_subtitles_share_a_group() {
        let a = renome_core::tmdb::guess("A Film 2001 1080p");
        let b = renome_core::tmdb::guess("A.Film.2001.pt-BR");
        assert_eq!(group(&a), group(&b));
    }
}
