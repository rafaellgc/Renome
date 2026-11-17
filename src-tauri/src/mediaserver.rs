//! The media servers the libraries are on (Jellyfin, Emby or Plex), told
//! to look again at the folders where files were renamed or moved, so
//! they show up at once. Each server's library folders are read from it
//! and paired with the same folders on this computer (the same path, or
//! one the user picks when the server sees them elsewhere, as in a
//! container); only folders inside a paired one, turned on, are told,
//! by the server's own path. Nothing else about the files is sent.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Jellyfin and Emby, which answer alike.
    Jellyfin,
    Plex,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    pub kind: Kind,
    pub url: String,
    pub key: String,
    /// The library folders, paired with this computer's.
    #[serde(default)]
    pub folders: Vec<Folder>,
}

/// A library folder: the server's path and the same folder here.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub library_id: String,
    pub server: String,
    /// Empty: not paired yet.
    pub local: String,
    pub on: bool,
}

/// A library on the server and its folders.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub id: String,
    pub name: String,
    pub paths: Vec<LibraryPath>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPath {
    pub path: String,
    /// Whether the same path is a folder on this computer.
    pub here: bool,
}

#[derive(Deserialize)]
struct JfFolder {
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "ItemId", default)]
    item_id: String,
    #[serde(rename = "Locations", default)]
    locations: Vec<String>,
    #[serde(rename = "CollectionType", default)]
    kind: Option<String>,
}

/// Libraries the server makes itself, from the others' items: collections
/// (`boxsets`, kept in its own data folder) and playlists. No file is
/// renamed into them.
fn made_by_server(kind: Option<&str>) -> bool {
    kind.is_some_and(|k| k.eq_ignore_ascii_case("boxsets") || k.eq_ignore_ascii_case("playlists"))
}

#[derive(Deserialize)]
struct PlexAnswer {
    #[serde(rename = "MediaContainer")]
    container: PlexContainer,
}

#[derive(Deserialize)]
struct PlexContainer {
    #[serde(rename = "Directory", default)]
    directory: Vec<PlexSection>,
}

#[derive(Deserialize)]
struct PlexSection {
    key: String,
    #[serde(default)]
    title: String,
    #[serde(rename = "Location", default)]
    location: Vec<PlexLocation>,
}

#[derive(Deserialize)]
struct PlexLocation {
    path: String,
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("Renome/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_default()
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_connect() {
        "srvOffline".into()
    } else if e.is_timeout() {
        "srvTimeout".into()
    } else if e.is_builder() {
        "srvAddress".into()
    } else {
        format!("srvNetwork: {}", e.without_url())
    }
}

impl Server {
    /// The address as typed, with `http://` when it has no scheme and
    /// without a slash at the end.
    fn base(&self) -> Result<String, String> {
        let url = self.url.trim().trim_end_matches('/');
        if url.is_empty() {
            return Err("srvAddress".into());
        }
        Ok(if url.contains("://") {
            url.to_string()
        } else {
            format!("http://{url}")
        })
    }

    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
    ) -> Result<reqwest::RequestBuilder, String> {
        let key = self.key.trim();
        if key.is_empty() {
            return Err("srvRefused".into());
        }
        let url = format!("{}{path}", self.base()?);
        let req = http().request(method, &url);
        Ok(match self.kind {
            Kind::Jellyfin => req
                .header("Authorization", format!("MediaBrowser Token=\"{key}\""))
                .header("X-Emby-Token", key),
            Kind::Plex => req
                .header("X-Plex-Token", key)
                .header("Accept", "application/json"),
        })
    }

    async fn answer(req: reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
        let res = req.send().await.map_err(net_error)?;
        match res.status().as_u16() {
            200..=299 => Ok(res),
            401 | 403 => Err("srvRefused".into()),
            code => Err(format!("srvAnswer: {code}")),
        }
    }

    async fn send(&self, method: reqwest::Method, path: &str) -> Result<(), String> {
        Self::answer(self.request(method, path)?).await.map(|_| ())
    }

    /// The libraries and their folders.
    pub async fn libraries(&self) -> Result<Vec<Library>, String> {
        let here = |p: String| LibraryPath {
            here: Path::new(&p).is_dir(),
            path: p,
        };
        match self.kind {
            Kind::Jellyfin => {
                let res =
                    Self::answer(self.request(reqwest::Method::GET, "/Library/VirtualFolders")?)
                        .await?;
                let list: Vec<JfFolder> = res
                    .json()
                    .await
                    .map_err(|_| "srvAnswer: JSON".to_string())?;
                Ok(list
                    .into_iter()
                    .filter(|f| !made_by_server(f.kind.as_deref()))
                    .map(|f| Library {
                        id: f.item_id,
                        name: f.name,
                        paths: f.locations.into_iter().map(here).collect(),
                    })
                    .collect())
            }
            Kind::Plex => {
                let res =
                    Self::answer(self.request(reqwest::Method::GET, "/library/sections")?).await?;
                let a: PlexAnswer = res
                    .json()
                    .await
                    .map_err(|_| "srvAnswer: JSON".to_string())?;
                Ok(a.container
                    .directory
                    .into_iter()
                    .map(|d| Library {
                        id: d.key,
                        name: d.title,
                        paths: d.location.into_iter().map(|l| here(l.path)).collect(),
                    })
                    .collect())
            }
        }
    }

    /// Asks the server to look again at these folders (its paths) of one
    /// library; when it does not take that, at the whole library.
    async fn look_at(&self, library: &str, paths: &[String]) -> Result<(), String> {
        let exact = match self.kind {
            Kind::Jellyfin => {
                let updates: Vec<_> = paths
                    .iter()
                    .map(|p| serde_json::json!({ "Path": p, "UpdateType": "Modified" }))
                    .collect();
                Self::answer(
                    self.request(reqwest::Method::POST, "/Library/Media/Updated")?
                        .json(&serde_json::json!({ "Updates": updates })),
                )
                .await
                .map(|_| ())
            }
            Kind::Plex => {
                let mut result = Ok(());
                for p in paths {
                    let path = format!("/library/sections/{}/refresh", enc(library));
                    let r = Self::answer(
                        self.request(reqwest::Method::GET, &path)?
                            .query(&[("path", p)]),
                    )
                    .await;
                    if r.is_err() {
                        result = r.map(|_| ());
                        break;
                    }
                }
                result
            }
        };
        match exact {
            Err(e) if e.starts_with("srvAnswer") && !library.is_empty() => match self.kind {
                Kind::Jellyfin => {
                    let path = format!("/Items/{}/Refresh", enc(library));
                    Self::answer(
                        self.request(reqwest::Method::POST, &path)?
                            .query(&[("Recursive", "true")]),
                    )
                    .await
                    .map(|_| ())
                }
                Kind::Plex => {
                    self.send(
                        reqwest::Method::GET,
                        &format!("/library/sections/{}/refresh", enc(library)),
                    )
                    .await
                }
            },
            other => other,
        }
    }

    /// Tells the server about the folders where `changed` files were or
    /// went; how many folders it was told about (none: nothing to tell).
    pub async fn tell(&self, changed: &[PathBuf]) -> Result<usize, String> {
        let plan = plan(&self.folders, changed);
        let mut told = 0;
        for (library, paths) in &plan {
            self.look_at(library, paths).await?;
            told += paths.len();
        }
        Ok(told)
    }

    /// Whether the address and the key work.
    pub async fn check(&self) -> Result<(), String> {
        match self.kind {
            Kind::Jellyfin => self.send(reqwest::Method::GET, "/System/Info").await,
            Kind::Plex => self.send(reqwest::Method::GET, "/library/sections").await,
        }
    }
}

/// A path piece for an address.
fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// The folders to tell about, by library, as the server writes them: the
/// folders the changed files were in and went to (the nearest one still
/// there, for a folder that went away), inside a paired folder that is
/// on; a folder inside another one told about is left out.
pub fn plan(folders: &[Folder], changed: &[PathBuf]) -> BTreeMap<String, Vec<String>> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for path in changed {
        let mut dir = path.parent().map(Path::to_path_buf);
        while let Some(d) = &dir {
            if d.is_dir() {
                break;
            }
            dir = d.parent().map(Path::to_path_buf);
        }
        if let Some(d) = dir
            && !dirs.contains(&d)
        {
            dirs.push(d);
        }
    }
    let mut out: BTreeMap<String, Vec<(PathBuf, String)>> = BTreeMap::new();
    for dir in &dirs {
        // The deepest paired folder holding it.
        let Some((folder, rel)) = folders
            .iter()
            .filter(|f| f.on && !f.local.trim().is_empty())
            .filter_map(|f| Some((f, dir.strip_prefix(f.local.trim()).ok()?)))
            .max_by_key(|(f, _)| f.local.trim().len())
        else {
            continue;
        };
        let windows = folder.server.contains('\\') && !folder.server.contains('/');
        let sep = if windows { '\\' } else { '/' };
        let mut server = folder.server.trim_end_matches(['/', '\\']).to_string();
        for part in rel.components() {
            server.push(sep);
            server.push_str(&part.as_os_str().to_string_lossy());
        }
        if server.is_empty() {
            server.push(sep);
        }
        out.entry(folder.library_id.clone())
            .or_default()
            .push((dir.clone(), server));
    }
    out.into_iter()
        .map(|(library, list)| {
            let keep = list
                .iter()
                .filter(|(d, _)| !list.iter().any(|(o, _)| o != d && d.starts_with(o)))
                .map(|(_, s)| s.clone())
                .collect();
            (library, keep)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        let s = |url: &str| Server {
            kind: Kind::Plex,
            url: url.into(),
            key: "k".into(),
            folders: Vec::new(),
        };
        assert_eq!(
            s("192.168.0.10:32400/").base().unwrap(),
            "http://192.168.0.10:32400"
        );
        assert_eq!(
            s("https://media.casa:8920").base().unwrap(),
            "https://media.casa:8920"
        );
        assert!(s("  ").base().is_err());
    }

    /// A server on this computer answering each request with `status`
    /// and `body`; gives back what it was asked (head and body).
    fn fake(status: u16, body: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut got = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let n = stream.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    got.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&got).to_string();
                    if let Some(end) = text.find("\r\n\r\n") {
                        let len = text
                            .lines()
                            .find_map(|l| {
                                l.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if got.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                let _ = tx.send(String::from_utf8_lossy(&got).into_owned());
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (url, rx)
    }

    #[test]
    fn folders_to_tell() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Filmes");
        std::fs::create_dir_all(root.join("Novo (2020)")).unwrap();
        std::fs::create_dir_all(root.join("Outro")).unwrap();
        let shows = d.path().join("Séries");
        std::fs::create_dir_all(shows.join("Show/Season 01")).unwrap();
        let folder = |id: &str, server: &str, local: &Path, on: bool| Folder {
            library_id: id.into(),
            server: server.into(),
            local: local.to_string_lossy().into(),
            on,
        };
        let folders = [
            folder("1", "/data/movies", &root, true),
            folder("2", "D:\\Series", &shows, true),
        ];
        let changed = [
            // Moved out of a folder that went away, into a new one.
            root.join("Velho/Filme.mkv"),
            root.join("Novo (2020)/Novo (2020).mkv"),
            // Outside every library.
            d.path().join("Fotos/a.jpg"),
            shows.join("Show/Season 01/e1.mkv"),
            shows.join("Show/e1.mkv"),
        ];
        let p = plan(&folders, &changed);
        // The folder that went away: its nearest one still there, the
        // library's own, which holds the new one too.
        assert_eq!(p["1"], vec!["/data/movies"]);
        // Windows paths on the server; the season inside the show is
        // left out.
        assert_eq!(p["2"], vec!["D:\\Series\\Show"]);
        assert_eq!(p.len(), 2);

        let only_new = [root.join("Novo (2020)/a.mkv"), root.join("Outro/b.mkv")];
        let p = plan(&folders, &only_new);
        assert_eq!(
            p["1"],
            vec!["/data/movies/Novo (2020)", "/data/movies/Outro"]
        );
        // Turned off, or not paired: nothing.
        let off = [folder("1", "/data/movies", &root, false)];
        assert!(plan(&off, &only_new).is_empty());
        let unpaired = [folder("1", "/data/movies", Path::new(""), true)];
        assert!(plan(&unpaired, &only_new).is_empty());
    }

    #[test]
    fn asks_the_server() {
        tauri::async_runtime::block_on(asks());
    }

    async fn asks() {
        let server = |kind, url: &str, key: &str, folders: Vec<Folder>| Server {
            kind,
            url: url.into(),
            key: key.into(),
            folders,
        };
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("Filme (2020)")).unwrap();
        let folders = || {
            vec![Folder {
                library_id: "lib 1".into(),
                server: "/data/movies".into(),
                local: d.path().to_string_lossy().into(),
                on: true,
            }]
        };
        let changed = [d.path().join("Filme (2020)/Filme (2020).mkv")];

        // Jellyfin: its libraries, then the folder told by its own path.
        let (url, asked) = fake(
            200,
            r#"[{"Name":"Filmes","ItemId":"f1","CollectionType":"movies","Locations":["/data/movies","/nowhere/at/all"]},{"Name":"Coleções","ItemId":"c1","CollectionType":"boxsets","Locations":["/config/data/collections"]}]"#,
        );
        let jf = server(Kind::Jellyfin, &url, "abc", folders());
        let libs = jf.libraries().await.unwrap();
        // Its collections are its own, not a folder of files.
        assert_eq!(libs.len(), 1);
        assert_eq!(
            (libs[0].id.as_str(), libs[0].name.as_str()),
            ("f1", "Filmes")
        );
        assert_eq!(libs[0].paths[1].path, "/nowhere/at/all");
        assert!(!libs[0].paths[1].here);
        let req = asked.recv().unwrap();
        assert!(req.starts_with("GET /Library/VirtualFolders "), "{req}");
        assert!(req.to_lowercase().contains("x-emby-token: abc"));
        assert_eq!(jf.tell(&changed).await.unwrap(), 1);
        let req = asked.recv().unwrap();
        assert!(req.starts_with("POST /Library/Media/Updated "), "{req}");
        assert!(
            req.contains(r#""Path":"/data/movies/Filme (2020)""#),
            "{req}"
        );
        // Nothing in a library: nothing asked.
        assert_eq!(
            jf.tell(&[PathBuf::from("/elsewhere/a.jpg")]).await.unwrap(),
            0
        );
        assert!(asked.try_recv().is_err());

        // Plex: the section's folder scan with the server's path.
        let (url, asked) = fake(
            200,
            r#"{"MediaContainer":{"Directory":[{"key":"3","title":"Movies","Location":[{"id":1,"path":"/data/movies"}]}]}}"#,
        );
        let plex = server(Kind::Plex, &url, "tok", folders());
        let libs = plex.libraries().await.unwrap();
        assert_eq!(
            (libs[0].id.as_str(), libs[0].paths[0].path.as_str()),
            ("3", "/data/movies")
        );
        asked.recv().unwrap();
        assert_eq!(plex.tell(&changed).await.unwrap(), 1);
        let req = asked.recv().unwrap();
        assert!(
            req.starts_with(
                "GET /library/sections/lib%201/refresh?path=%2Fdata%2Fmovies%2FFilme+%282020%29 "
            ),
            "{req}"
        );
        assert!(req.to_lowercase().contains("x-plex-token: tok"));

        // Not taken by folder: the whole library, then the error.
        let (url, asked) = fake(404, "");
        let wrong = server(Kind::Jellyfin, &url, "x", folders());
        assert_eq!(wrong.tell(&changed).await.unwrap_err(), "srvAnswer: 404");
        assert!(
            asked
                .recv()
                .unwrap()
                .starts_with("POST /Library/Media/Updated ")
        );
        let req = asked.recv().unwrap();
        assert!(
            req.starts_with("POST /Items/lib%201/Refresh?Recursive=true "),
            "{req}"
        );

        let (url, _asked) = fake(401, "");
        assert_eq!(
            server(Kind::Plex, &url, "x", vec![])
                .check()
                .await
                .unwrap_err(),
            "srvRefused"
        );
        let none = server(Kind::Jellyfin, "127.0.0.1:1", "x", vec![]);
        assert_eq!(none.check().await.unwrap_err(), "srvOffline");
    }
}
