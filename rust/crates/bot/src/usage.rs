//! Running total of /dps runs (port of src/lib/usage.js), kept in usage.json in the working directory (USAGE_FILE
//! to move it). Each time the total gains a digit (10, 100, 1,000, ...) the bot owner gets a DM: OWNER_ID if set,
//! else the app's owner (or team owner) from the Developer Portal. The DM needs DISCORD_TOKEN; without it the
//! milestone is only logged.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use dps_core::jsfmt::to_locale_string;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;

use crate::transport::{BoxFuture, API};

/// True when n is the first number with its digit count (10, 100, 1000, ...).
pub fn is_new_digit(n: u64) -> bool {
    n >= 10 && n.to_string().bytes().skip(1).all(|b| b == b'0')
}

pub trait Notifier: Send + Sync {
    fn notify(&self, text: String) -> BoxFuture<Result<(), String>>;
}

struct Inner {
    file: PathBuf,
    notifier: Arc<dyn Notifier>,
    total: AsyncMutex<Option<u64>>,
    /// Latest total, read by the writer when it runs.
    latest: Mutex<u64>,
    writer: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone)]
pub struct UsageCounter(Arc<Inner>);

fn parse_total(text: &str) -> Result<u64, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    // Math.max(0, Math.floor(Number(total) || 0))
    let n = match &v["total"] {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0),
        Value::Bool(true) => 1.0,
        _ => 0.0,
    };
    let n = if n.is_finite() { n.floor() } else { 0.0 };
    Ok(if n > 0.0 { n as u64 } else { 0 })
}

impl UsageCounter {
    pub fn new(file: PathBuf, notifier: Arc<dyn Notifier>) -> Self {
        UsageCounter(Arc::new(Inner { file, notifier, total: AsyncMutex::new(None), latest: Mutex::new(0), writer: Mutex::new(None) }))
    }

    /// The default counter: USAGE_FILE or usage.json in the working directory, DMing the owner.
    pub fn from_env() -> Self {
        let file = std::env::var("USAGE_FILE").ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("usage.json"));
        Self::new(file, Arc::new(DiscordNotifier::default()))
    }

    async fn load(&self) -> u64 {
        let mut guard = self.0.total.lock().await;
        if let Some(n) = *guard {
            return n;
        }
        let n = match tokio::fs::read_to_string(&self.0.file).await {
            Ok(text) => match parse_total(&text) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("usage.json unreadable, counting from 0: {e}");
                    0
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
            Err(e) => {
                eprintln!("usage.json unreadable, counting from 0: {e}");
                0
            }
        };
        *guard = Some(n);
        *self.0.latest.lock() = n;
        n
    }

    /// Writes run one at a time; each writes the latest total through a temp file, so a crash never leaves half a file.
    fn save(&self) {
        let inner = self.0.clone();
        let mut slot = self.0.writer.lock();
        let prev = slot.take();
        *slot = Some(tokio::spawn(async move {
            if let Some(p) = prev {
                let _ = p.await;
            }
            let total = *inner.latest.lock();
            let mut tmp = inner.file.clone().into_os_string();
            tmp.push(".tmp");
            let tmp = PathBuf::from(tmp);
            let res = async {
                tokio::fs::write(&tmp, format!("{}\n", json!({ "total": total }))).await?;
                tokio::fs::rename(&tmp, &inner.file).await
            }
            .await;
            if let Err(e) = res {
                eprintln!("usage.json write failed: {e}");
            }
        }));
    }

    pub async fn record(&self) -> u64 {
        self.load().await;
        let n = {
            let mut guard = self.0.total.lock().await;
            let n = guard.unwrap_or(0) + 1;
            *guard = Some(n);
            *self.0.latest.lock() = n;
            n
        };
        self.save();
        if is_new_digit(n) {
            let fut = self.0.notifier.notify(format!("/dps has been run {} times.", to_locale_string(n as f64)));
            tokio::spawn(async move {
                if let Err(e) = fut.await {
                    eprintln!("Usage milestone DM failed: {e}");
                }
            });
        }
        n
    }

    pub async fn total(&self) -> u64 {
        self.load().await
    }

    /// Wait for pending writes.
    pub async fn flush(&self) {
        let h = self.0.writer.lock().take();
        if let Some(h) = h {
            let _ = h.await;
        }
    }
}

static GLOBAL: OnceLock<UsageCounter> = OnceLock::new();
/// The process-wide counter (the JS `usage` export). Must first be called inside a tokio runtime context only when
/// recording; construction itself does no I/O.
pub fn global() -> &'static UsageCounter {
    GLOBAL.get_or_init(UsageCounter::from_env)
}

/// DMs the owner through the Discord REST API.
#[derive(Default)]
struct DiscordInner {
    owner_id: tokio::sync::OnceCell<String>,
    client: OnceLock<reqwest::Client>,
}

impl DiscordInner {
    fn client(&self) -> &reqwest::Client {
        self.client.get_or_init(|| reqwest::Client::builder().timeout(Duration::from_secs(15)).build().expect("client"))
    }

    async fn discord(&self, route: &str, token: &str, post: Option<Value>) -> Result<Value, String> {
        let url = format!("{API}{route}");
        let req = match post {
            Some(body) => self.client().post(url).json(&body),
            None => self.client().get(url),
        };
        let res = req.header("Authorization", format!("Bot {token}")).send().await.map_err(|e| format!("{route}: {e}"))?;
        if !res.status().is_success() {
            return Err(format!("{route}: HTTP {}", res.status().as_u16()));
        }
        res.json().await.map_err(|e| format!("{route}: {e}"))
    }

    async fn send(&self, text: String) -> Result<(), String> {
        println!("{text}");
        let token = match std::env::var("DISCORD_TOKEN") {
            Ok(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => return Ok(()),
        };
        let owner = self
            .owner_id
            .get_or_try_init(|| async {
                if let Ok(o) = std::env::var("OWNER_ID") {
                    if !o.trim().is_empty() {
                        return Ok(o.trim().to_string());
                    }
                }
                let app = self.discord("/oauth2/applications/@me", &token, None).await?;
                let id = app["team"]["owner_user_id"].as_str().or_else(|| app["owner"]["id"].as_str());
                id.map(str::to_string).ok_or_else(|| "No owner found. Set OWNER_ID in .env.".to_string())
            })
            .await?
            .clone();
        let channel = self.discord("/users/@me/channels", &token, Some(json!({ "recipient_id": owner }))).await?;
        let cid = channel["id"].as_str().ok_or("no DM channel id")?;
        self.discord(&format!("/channels/{cid}/messages"), &token, Some(json!({ "content": text, "allowed_mentions": { "parse": [] } }))).await?;
        Ok(())
    }
}

#[derive(Default, Clone)]
pub struct DiscordNotifier(Arc<DiscordInner>);

impl Notifier for DiscordNotifier {
    fn notify(&self, text: String) -> BoxFuture<Result<(), String>> {
        let this = self.0.clone();
        Box::pin(async move { this.send(text).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Collect(Arc<Mutex<Vec<String>>>);
    impl Notifier for Collect {
        fn notify(&self, text: String) -> BoxFuture<Result<(), String>> {
            let sink = self.0.clone();
            Box::pin(async move {
                sink.lock().push(text);
                Ok(())
            })
        }
    }

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dps-bot-usage-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_new_digit_is_10_100_1000_and_so_on() {
        let got: Vec<u64> = [1u64, 9, 10, 11, 99, 100, 101, 1000, 1010, 10000].into_iter().filter(|n| is_new_digit(*n)).collect();
        assert_eq!(got, vec![10, 100, 1000, 10000]);
    }

    #[tokio::test]
    async fn the_run_total_persists_and_notifies_once_per_new_digit() {
        let dir = tmp_dir("a");
        let file = dir.join("usage.json");
        std::fs::write(&file, r#"{"total":95}"#).unwrap();
        let sent = Arc::new(Mutex::new(vec![]));
        let counter = UsageCounter::new(file.clone(), Arc::new(Collect(sent.clone())));
        for _ in 0..10 {
            counter.record().await;
        }
        counter.flush().await;
        tokio::task::yield_now().await;
        assert_eq!(counter.total().await, 105);
        assert_eq!(*sent.lock(), vec!["/dps has been run 100 times.".to_string()]);
        assert_eq!(serde_json::from_str::<Value>(&std::fs::read_to_string(&file).unwrap()).unwrap(), json!({ "total": 105 }));
        assert!(!dir.join("usage.json.tmp").exists());
        let again = UsageCounter::new(file, Arc::new(Collect::default()));
        assert_eq!(again.total().await, 105);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn missing_or_broken_files_count_from_zero_and_totals_are_sanitised() {
        let dir = tmp_dir("b");
        let c = UsageCounter::new(dir.join("none.json"), Arc::new(Collect::default()));
        assert_eq!(c.record().await, 1);
        c.flush().await;
        assert_eq!(std::fs::read_to_string(dir.join("none.json")).unwrap(), "{\"total\":1}\n");
        std::fs::write(dir.join("bad.json"), "not json").unwrap();
        assert_eq!(UsageCounter::new(dir.join("bad.json"), Arc::new(Collect::default())).total().await, 0);
        for (text, want) in [(r#"{"total":-5}"#, 0), (r#"{"total":"7.9"}"#, 7), (r#"{"total":null}"#, 0), (r#"{}"#, 0)] {
            std::fs::write(dir.join("t.json"), text).unwrap();
            assert_eq!(UsageCounter::new(dir.join("t.json"), Arc::new(Collect::default())).total().await, want, "{text}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
