//! Port of src/api/http.js: one shared `reqwest::Client` and `get_json` (15 s timeout, 204/404 -> `None`, readable
//! `HTTP <status> from <host>: <body>` errors).

use serde::de::DeserializeOwned;
use std::time::Duration;

pub use reqwest::Client;

/// Default request timeout (`AbortSignal.timeout(15000)`).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// The shared HTTP client: connection reuse, gzip, rustls. Build it once and clone it (clones share the pool).
pub fn new_client() -> Client {
    Client::builder()
        .user_agent("dps-bot")
        .gzip(true)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(60))
        .build()
        .expect("reqwest client")
}

/// `new URL(url).host`: host name plus the port when it is not the scheme default.
fn host_of(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(u) => match (u.host_str(), u.port()) {
            (Some(h), Some(p)) => format!("{h}:{p}"),
            (Some(h), None) => h.to_string(),
            _ => String::new(),
        },
        Err(_) => String::new(),
    }
}

fn first_chars(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// GET `url` and return the body bytes; `None` on 204 / 404. Non-2xx statuses are errors.
pub async fn get_bytes(
    client: &Client,
    url: &str,
    headers: &[(&str, &str)],
    timeout: Duration,
) -> Result<Option<Vec<u8>>, String> {
    let mut req = client.get(url).timeout(timeout);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let fail = |e: reqwest::Error| {
        if e.is_timeout() {
            "The operation was aborted due to timeout".to_string()
        } else {
            "fetch failed".to_string()
        }
    };
    let res = req.send().await.map_err(fail)?;
    let status = res.status();
    if status.as_u16() == 204 || status.as_u16() == 404 {
        return Ok(None);
    }
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(format!("HTTP {} from {}: {}", status.as_u16(), host_of(url), first_chars(&body, 200)));
    }
    let bytes = res.bytes().await.map_err(fail)?;
    Ok(Some(bytes.to_vec()))
}

/// JS `getJson(url, { headers })`: parsed body, or `None` on 204 / 404.
pub async fn get_json<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    headers: &[(&str, &str)],
) -> Result<Option<T>, String> {
    get_json_timeout(client, url, headers, DEFAULT_TIMEOUT).await
}

pub async fn get_json_timeout<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    headers: &[(&str, &str)],
    timeout: Duration,
) -> Result<Option<T>, String> {
    match get_bytes(client, url, headers, timeout).await? {
        None => Ok(None),
        Some(b) => serde_json::from_slice(&b).map(Some).map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
pub(crate) mod testserver {
    //! A tiny one-shot-per-connection HTTP server for tests: answers every request with the given status and body
    //! and counts requests.
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    pub struct Server {
        pub base: String,
        pub hits: Arc<AtomicUsize>,
    }

    pub async fn serve(status: u16, body: &'static str) -> Server {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = listener.accept().await else { return };
                let h = h.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let mut got = Vec::new();
                    loop {
                        let Ok(n) = s.read(&mut buf).await else { return };
                        if n == 0 {
                            return;
                        }
                        got.extend_from_slice(&buf[..n]);
                        if got.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    h.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                    let resp = format!(
                        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = s.write_all(resp.as_bytes()).await;
                    let _ = s.shutdown().await;
                });
            }
        });
        Server { base, hits }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn statuses() {
        let c = new_client();
        let ok = testserver::serve(200, r#"{"a":1}"#).await;
        let v: Option<serde_json::Value> = get_json(&c, &format!("{}/x", ok.base), &[]).await.unwrap();
        assert_eq!(v.unwrap()["a"], 1);
        let nf = testserver::serve(404, "nope").await;
        assert!(get_json::<serde_json::Value>(&c, &nf.base, &[]).await.unwrap().is_none());
        let nc = testserver::serve(204, "").await;
        assert!(get_json::<serde_json::Value>(&c, &nc.base, &[]).await.unwrap().is_none());
        let bad = testserver::serve(500, "boom").await;
        let e = get_json::<serde_json::Value>(&c, &format!("{}/y", bad.base), &[]).await.unwrap_err();
        assert_eq!(e, format!("HTTP 500 from {}: boom", bad.base.trim_start_matches("http://")));
    }
}
