//! Pins what the comic rule's search answers, against a fake Metron
//! served from this process, so the function can be taken apart without
//! changing it. Set `GOLDEN_PRINT=1` to print the answers.
#![allow(clippy::unwrap_used)]

use renome_services::comics::Sources;
use renome_services::flow::comics::{Params, search};
use renome_services::flow::{Item, Question};
use renome_services::{Host, Sender, anilist, comicvine, gcd, mangadex, mangaupdates, metron};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn series(id: u32, year: u32, publisher: &str) -> Value {
    json!({"id": id, "series": format!("Capitão Aurora ({year})"), "name": "Capitão Aurora",
        "year_began": year, "issue_count": 3, "volume": 1,
        "publisher": {"id": 1, "name": publisher},
        "series_type": {"id": 1, "name": "Ongoing Series"}})
}

fn issues(base: u32, year: u32) -> Value {
    let list: Vec<Value> = (1..=3)
        .map(|n| {
            json!({"id": base + n, "number": n.to_string(),
                "issue": format!("Capitão Aurora ({year}) #{n}"),
                "cover_date": format!("{year}-0{n}-01")})
        })
        .collect();
    json!({"count": 3, "next": null, "results": list})
}

/// Answers the paths the flow asks, by the series asked for.
fn answer(path: &str, two: bool) -> Option<Value> {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    Some(match path {
        "/series/" => {
            let mut hits = vec![series(77, 2016, "Editora Estrela")];
            if two || query.contains("year_began=2019") {
                hits.push(series(78, 2019, "Editora Lua"));
            }
            json!({"count": hits.len(), "next": null, "results": hits})
        }
        "/series/77/" => series(77, 2016, "Editora Estrela"),
        "/series/78/" => series(78, 2019, "Editora Lua"),
        "/series/77/issue_list/" => issues(900, 2016),
        "/series/78/issue_list/" => issues(950, 2019),
        _ => return None,
    })
}

fn serve(two: bool) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]).to_string();
            let target = head.split_whitespace().nth(1).unwrap_or("/");
            let (code, body) = match answer(target, two) {
                Some(v) => (200, v.to_string()),
                None => (404, "{}".to_string()),
            };
            let _ = write!(
                s,
                "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    format!("http://{addr}")
}

/// A host that takes the first candidate offered and records what was said.
#[derive(Default)]
struct Scripted {
    said: Arc<Mutex<Vec<String>>>,
    asked: Arc<Mutex<Vec<Value>>>,
}

impl Host for Scripted {
    fn report(&self, event: &str, payload: Value) {
        self.said.lock().unwrap().push(format!("{event} {payload}"));
    }
    fn stop_signal(&self) -> Arc<dyn Fn() -> bool + Send + Sync> {
        Arc::new(|| false)
    }
    fn sender(&self) -> Sender {
        Arc::new(|_, _| {})
    }
    fn ask(
        &self,
        question: Question,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<Value>> + Send + '_>> {
        let v = serde_json::to_value(&question).unwrap();
        let first = v["candidates"][0].clone();
        self.asked.lock().unwrap().push(v);
        Box::pin(async move { (!first.is_null()).then_some(first) })
    }
}

fn params(series: &str) -> Params {
    serde_json::from_value(json!({"source": "metron", "key": "token", "series": series,
        "lang": "pt-br", "uiLang": "pt"}))
    .unwrap()
}

fn item(name: &str) -> Item {
    serde_json::from_value(json!({"path": format!("/nowhere/{name}"), "original": name,
        "lookupName": name})).unwrap()
}

async fn run(_two: bool, p: &Params, names: &[&str]) -> String {
    let host = Scripted::default();
    let (cv, mt, gcd, md, al, mu) = (
        comicvine::Client::default(),
        metron::Client::default(),
        gcd::Client::default(),
        mangadex::Client::default(),
        anilist::Client::default(),
        mangaupdates::Client::default(),
    );
    let sources = Sources { cv: &cv, mt: &mt, gcd: &gcd, md: &md, al: &al, mu: &mu };
    let items: Vec<Item> = names.iter().map(|n| item(n)).collect();
    let out = search(&host, &sources, p, &items, &HashSet::<PathBuf>::new(), HashMap::new())
        .await
        .unwrap();
    let mut v = serde_json::to_value(&out).unwrap();
    v["_asked"] = Value::Array(host.asked.lock().unwrap().clone());
    v["_said"] = json!(host.said.lock().unwrap().clone());
    serde_json::to_string(&v).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn comic_search_does_not_change() {
    let dir = tempfile::tempdir().unwrap();
    // SAFETY: set before any thread of this test reads them.
    unsafe {
        std::env::set_var("RENOME_DIR", dir.path());
        std::env::set_var("RENOME_METRON_API", serve(true));
    }
    let one = run(
        true,
        &params(""),
        &["Capitão Aurora 001 (2016).cbz", "Capitão Aurora 002 (2016).cbz", "Capitão Aurora 009 (2016).cbz"],
    )
    .await;
    let none = run(true, &params(""), &["notes.txt"]).await;
    let loose = run(true, &params("Capitão Aurora"), &["Capitão Aurora 003.cbz"]).await;
    let all = format!("{one}\n{none}\n{loose}");
    if std::env::var_os("GOLDEN_PRINT").is_some() {
        println!("{all}");
    }
    let sum = xxhash(&all);
    println!("hash {sum:016x}");
    assert_eq!(sum, GOLDEN, "the comic search's answer changed");
}

fn xxhash(s: &str) -> u64 {
    xxhash_rust::xxh3::xxh3_64(s.as_bytes())
}

const GOLDEN: u64 = 0x559adfecf12754d0;
