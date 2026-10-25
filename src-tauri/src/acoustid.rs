//! AcoustID client: which recordings a song's fingerprint was heard as. At
//! most three requests a second, as the service asks.

use std::time::{Duration, Instant};

use renome_core::acoustid::{self as ac, Heard, Response};
use renome_core::fingerprint::Print;

const API: &str = "https://api.acoustid.org/v2/lookup";
/// Renome's application key (free for non-commercial programs).
const KEY: &str = "0VgOiCZa1a";

/// Tests point this at a local server with `RENOME_ACOUSTID_API`.
fn api() -> String {
    std::env::var("RENOME_ACOUSTID_API").unwrap_or_else(|_| API.into())
}

pub struct Client {
    http: reqwest::Client,
    last: tokio::sync::Mutex<Option<Instant>>,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!(
                    "Renome/",
                    env!("CARGO_PKG_VERSION"),
                    " ( https://github.com/rafaellgc/Renome )"
                ))
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            last: tokio::sync::Mutex::new(None),
        }
    }
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_connect() {
        "acoustidOffline".into()
    } else if e.is_timeout() {
        "acoustidTimeout".into()
    } else {
        format!("acoustidNetwork: {}", e.without_url())
    }
}

impl Client {
    /// The recordings and releases the song was heard as.
    pub async fn lookup(&self, print: &Print) -> Result<Heard, String> {
        let duration = print.duration.to_string();
        let form = [
            ("client", KEY),
            ("format", "json"),
            ("meta", "recordings sources releasegroups releases"),
            ("duration", &duration),
            ("fingerprint", &print.fingerprint),
        ];
        for attempt in 0..4 {
            // One request every 0.4 seconds, under the service's limit.
            let mut last = self.last.lock().await;
            if let Some(t) = *last {
                let wait = Duration::from_millis(400).saturating_sub(t.elapsed());
                tokio::time::sleep(wait).await;
            }
            let res = self.http.post(api()).form(&form).send().await;
            *last = Some(Instant::now());
            drop(last);
            let res = res.map_err(net_error)?;
            let code = res.status().as_u16();
            if matches!(code, 429 | 503) && attempt < 3 {
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
            // Errors come as JSON too (an invalid key is a 400).
            let body: Response = match res.json().await {
                Ok(b) => b,
                Err(_) if code == 200 => return Err("acoustidNetwork: invalid answer".into()),
                Err(_) => return Err(format!("acoustidNetwork: HTTP {code}")),
            };
            if let Some(e) = body.error {
                // 4: invalid key; 14: the key was blocked.
                return Err(match e.code {
                    4 | 14 => "acoustidKey".into(),
                    _ => format!("acoustidNetwork: {}", e.message),
                });
            }
            return Ok(ac::heard(&body));
        }
        Err("acoustidBusy".into())
    }
}
