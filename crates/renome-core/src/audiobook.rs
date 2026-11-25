//! Audiobooks: one book in one audio file (an `.m4b`) or in several (a
//! folder of parts or chapters). The book rule names them like books,
//! from what the catalogue says; here, which files make each book, in
//! which order, and what their tags and folder name say about it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::music::{self, AudioTags};

/// An audiobook: its files in order, and what they say it is.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioBook {
    pub paths: Vec<PathBuf>,
    pub title: String,
    pub author: String,
    pub year: Option<u16>,
    /// Who reads it: the narrator in the folder name (`{Name}`, `narrated
    /// by Name`), else the tags' composer, where audiobook programs keep
    /// it (unless it is the author).
    pub narrator: String,
    /// The series, from the tags' movement name (`MVNM`, `©mvn`), where
    /// audiobook taggers put it.
    pub series: String,
    pub series_index: String,
}

impl AudioBook {
    /// The book's name as a file name would say it, for the catalogue
    /// search: `Author - Title (Year)`.
    pub fn name(&self) -> String {
        let mut s = match (self.author.is_empty(), self.title.is_empty()) {
            (false, false) => format!("{} - {}", self.author, self.title),
            (true, _) => self.title.clone(),
            (false, true) => self.author.clone(),
        };
        if let Some(y) = self.year {
            s.push_str(&format!(" ({y})"));
        }
        s
    }
}

/// Words in audiobook folder names that are not part of the book.
static JUNK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)[(\[]\s*(unabridged|abridged|integral|completo|íntegra|audio ?book|audiolivro|audiolibro|\d+\s*k(bps)?|mp3|m4b|aac)\s*[)\]]|\b(unabridged|audiobook|audiolivro|audiolibro)\b",
    )
    .expect("valid regex")
});
/// `{Narrator}` (as Audiobookshelf writes it), `narrated by Name`,
/// `narrado por Name`, `read by Name`.
static NARRATOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\{([^}]+)\}|[\s(\[-]+(?:narrated by|read by|narrado por|lido por|narrado por|leído por)\s+([^)\]]+)[)\]]?")
        .expect("valid regex")
});

/// Title, author, year and narrator from a folder or file name.
fn from_name(name: &str) -> (crate::books::Guess, String) {
    let mut narrator = String::new();
    let mut s = name.to_string();
    if let Some(c) = NARRATOR.captures(&s) {
        narrator = c
            .get(1)
            .or_else(|| c.get(2))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        s = NARRATOR.replace(&s, " ").into_owned();
    }
    let s = JUNK.replace_all(&s, " ");
    (
        crate::books::guess_name(&crate::text::collapse_spaces(&s)),
        narrator,
    )
}

/// A file and its tags.
type Read = (PathBuf, Option<AudioTags>);

/// The files' order in their book: disc, track, then name as people sort
/// it (`2` before `10`).
fn order(a: &Read, b: &Read) -> std::cmp::Ordering {
    let key = |x: &Read| {
        let t = x.1.as_ref();
        let folder =
            x.0.parent()
                .and_then(|d| music::disc_folder(&d.file_name()?.to_string_lossy()));
        (
            t.and_then(|t| t.disc).or(folder).unwrap_or(0),
            t.and_then(|t| t.track).unwrap_or(0),
        )
    };
    key(a).cmp(&key(b)).then_with(|| {
        crate::files::natural_cmp(
            &a.0.file_name().unwrap_or_default().to_string_lossy(),
            &b.0.file_name().unwrap_or_default().to_string_lossy(),
        )
    })
}

/// The folder an audiobook's file belongs to: its own, or the one above
/// a disc folder (`CD1`, `Disc 2`).
pub fn book_dir(dir: &Path) -> PathBuf {
    match dir.parent() {
        Some(up)
            if music::disc_folder(&dir.file_name().unwrap_or_default().to_string_lossy())
                .is_some() =>
        {
            up.to_path_buf()
        }
        _ => dir.to_path_buf(),
    }
}

/// The audiobooks these audio files make: the files of a folder with the
/// same album (or none) are one book; `.m4b` files without an album are
/// a book each, as they usually are. Disc folders (`CD1`, `Disc 2`) count
/// as their parent folder.
pub fn group(paths: &[PathBuf]) -> Vec<AudioBook> {
    let read: Vec<Read> = paths
        .iter()
        .filter(|p| {
            p.extension()
                .is_some_and(|e| music::is_audio(&e.to_string_lossy()))
        })
        .map(|p| (p.clone(), music::read(p)))
        .collect();
    let mut groups: BTreeMap<(PathBuf, String), Vec<Read>> = BTreeMap::new();
    for (path, tags) in read {
        let dir = book_dir(path.parent().unwrap_or(Path::new("")));
        let album = tags
            .as_ref()
            .map(|t| t.album.to_lowercase())
            .unwrap_or_default();
        let single = album.is_empty()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("m4b"));
        let key = if single {
            path.to_string_lossy().into_owned()
        } else {
            album
        };
        groups.entry((dir, key)).or_default().push((path, tags));
    }
    groups
        .into_iter()
        .map(|((dir, _), mut files)| {
            files.sort_by(order);
            let first = files.iter().find_map(|(_, t)| t.as_ref());
            let tag = |get: fn(&AudioTags) -> &str| {
                files
                    .iter()
                    .filter_map(|(_, t)| t.as_ref())
                    .map(get)
                    .find(|s| !s.trim().is_empty())
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            };
            // The folder's name, or the file's for a book in one file.
            let name = if files.len() == 1 {
                files[0]
                    .0
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            } else {
                dir.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            };
            let (guess, named_narrator) = from_name(&name);
            let author = match tag(|t| &t.album_artist) {
                a if !a.is_empty() => a,
                _ => match tag(|t| &t.artist) {
                    a if !a.is_empty() => a,
                    _ => guess.author.clone(),
                },
            };
            let title = match tag(|t| &t.album) {
                t if !t.is_empty() => JUNK.replace_all(&t, " ").trim().to_string(),
                _ if files.len() == 1 && guess.title.is_empty() => tag(|t| &t.title),
                _ => guess.title.clone(),
            };
            let composer = tag(|t| &t.composer);
            let narrator = if !named_narrator.is_empty() {
                named_narrator
            } else if !composer.eq_ignore_ascii_case(&author) {
                composer
            } else {
                String::new()
            };
            AudioBook {
                title: crate::text::collapse_spaces(&title),
                author,
                year: first.and_then(|t| t.year).or(guess.year),
                narrator,
                series: tag(|t| &t.series),
                series_index: tag(|t| &t.series_index),
                paths: files.into_iter().map(|(p, _)| p).collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let (g, n) = from_name("Lia Marta Quaresma - Ecos (2019) {Tomé Rocha} [64kbps]");
        assert_eq!(
            (g.author.as_str(), g.title.as_str(), g.year),
            ("Lia Marta Quaresma", "Ecos", Some(2019))
        );
        assert_eq!(n, "Tomé Rocha");
        let (g, n) = from_name("Vento Leste (Unabridged) - narrado por Ana Lima");
        assert_eq!(g.title, "Vento Leste");
        assert_eq!(n, "Ana Lima");
        let b = AudioBook {
            title: "Ecos".into(),
            author: "Lia".into(),
            year: Some(2019),
            ..Default::default()
        };
        assert_eq!(b.name(), "Lia - Ecos (2019)");
    }

    #[test]
    fn groups_by_folder_and_album() {
        let d = tempfile::tempdir().unwrap();
        let book = d.path().join("Lia Marta Quaresma - Ecos {Tomé Rocha}");
        std::fs::create_dir_all(book.join("CD2")).unwrap();
        std::fs::create_dir_all(book.join("CD1")).unwrap();
        for p in ["CD1/Parte 2.mp3", "CD1/Parte 10.mp3", "CD2/Parte 1.mp3"] {
            std::fs::write(book.join(p), b"").unwrap();
        }
        let single = d.path().join("Avulsos");
        std::fs::create_dir_all(&single).unwrap();
        for p in ["Vento Leste.m4b", "Outro Livro (2001).m4b"] {
            std::fs::write(single.join(p), b"").unwrap();
        }
        let mut paths: Vec<PathBuf> = ["CD2/Parte 1.mp3", "CD1/Parte 10.mp3", "CD1/Parte 2.mp3"]
            .iter()
            .map(|p| book.join(p))
            .collect();
        paths.push(single.join("Vento Leste.m4b"));
        paths.push(single.join("Outro Livro (2001).m4b"));
        paths.push(d.path().join("notas.txt"));
        let books = group(&paths);
        assert_eq!(books.len(), 3);
        let ecos = books.iter().find(|b| b.title == "Ecos").unwrap();
        assert_eq!(ecos.author, "Lia Marta Quaresma");
        assert_eq!(ecos.narrator, "Tomé Rocha");
        // Disc folders count as the book's; without tags, by name.
        assert_eq!(
            ecos.paths,
            vec![
                book.join("CD1/Parte 2.mp3"),
                book.join("CD1/Parte 10.mp3"),
                book.join("CD2/Parte 1.mp3")
            ]
        );
        let other = books.iter().find(|b| b.title == "Outro Livro").unwrap();
        assert_eq!((other.year, other.paths.len()), (Some(2001), 1));
        assert!(books.iter().any(|b| b.title == "Vento Leste"));
    }
}
