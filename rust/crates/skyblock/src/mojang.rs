//! Port of src/api/mojang.js: username validation and the cached Minecraft account lookup.

use crate::http::{get_json, Client};
use dps_core::model::Account;
use parking_lot::Mutex;
use rustc_hash::FxHashMap;
use std::time::{Duration, Instant};

const FOUND: Duration = Duration::from_secs(24 * 60 * 60);
const MISSING: Duration = Duration::from_secs(10 * 60);
const PRUNE_EVERY: Duration = Duration::from_secs(60);

/// `/^[a-zA-Z0-9_]{2,16}$/`.
pub fn is_valid_ign(name: &str) -> bool {
    (2..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// `/^[0-9a-f]{32}$/i`.
pub fn is_valid_uuid(uuid: &str) -> bool {
    uuid.len() == 32 && uuid.bytes().all(|b| b.is_ascii_hexdigit())
}

struct Cache {
    map: FxHashMap<String, (Option<Account>, Instant)>,
    last_prune: Instant,
}

/// Username lookups, cached: found accounts for a day, unknown names for 10 minutes.
pub struct Mojang {
    client: Client,
    base: String,
    cache: Mutex<Cache>,
}

impl Mojang {
    pub fn new(client: Client) -> Self {
        Self::with_base(client, "https://api.mojang.com")
    }

    pub fn with_base(client: Client, base: &str) -> Self {
        Mojang {
            client,
            base: base.trim_end_matches('/').to_string(),
            cache: Mutex::new(Cache { map: FxHashMap::default(), last_prune: Instant::now() }),
        }
    }

    /// Look up a Minecraft account: `Ok(None)` if it does not exist (or the name is invalid, without any request).
    /// Errors when Mojang cannot be reached.
    pub async fn get_minecraft_profile(&self, username: &str) -> Result<Option<Account>, String> {
        if !is_valid_ign(username) {
            return Ok(None);
        }
        let key = username.to_ascii_lowercase();
        let now = Instant::now();
        {
            let mut c = self.cache.lock();
            if now.duration_since(c.last_prune) >= PRUNE_EVERY {
                c.map.retain(|_, (_, expires)| *expires > now);
                c.last_prune = now;
            }
            if let Some((v, expires)) = c.map.get(&key) {
                if *expires > now {
                    return Ok(v.clone());
                }
            }
        }
        let url = format!("{}/users/profiles/minecraft/{}", self.base, username);
        let data: Option<serde_json::Value> = get_json(&self.client, &url, &[]).await?;
        let value = data.and_then(|d| {
            let id = d.get("id")?.as_str()?;
            let name = d.get("name")?.as_str()?;
            (is_valid_uuid(id) && is_valid_ign(name)).then(|| Account { uuid: id.to_string(), name: name.to_string() })
        });
        let ttl = if value.is_some() { FOUND } else { MISSING };
        self.cache.lock().map.insert(key, (value.clone(), now + ttl));
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{new_client, testserver};
    use std::sync::atomic::Ordering;

    #[test]
    fn is_valid_ign_accepts_minecraft_usernames_only() {
        for ok in ["p3wr", "BOBSTEVE21", "a_b", "ab", "abcdefghijklmnop"] {
            assert!(is_valid_ign(ok), "{ok}");
        }
        for bad in ["a", "abcdefghijklmnopq", "@everyone", "a b", "x/../y", "https://x.y", "", "p3wr\n"] {
            assert!(!is_valid_ign(bad), "{bad}");
        }
    }

    #[tokio::test]
    async fn never_requests_an_invalid_username() {
        let s = testserver::serve(200, "{}").await;
        let m = Mojang::with_base(new_client(), &s.base);
        assert_eq!(m.get_minecraft_profile("@everyone").await.unwrap(), None);
        assert_eq!(s.hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn rejects_a_malformed_response() {
        let s = testserver::serve(200, r#"{"id":"../x","name":"p3wr"}"#).await;
        let m = Mojang::with_base(new_client(), &s.base);
        assert_eq!(m.get_minecraft_profile("p3wr").await.unwrap(), None);
        // The miss is cached.
        assert_eq!(m.get_minecraft_profile("P3WR").await.unwrap(), None);
        assert_eq!(s.hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn found_accounts_are_cached() {
        let s = testserver::serve(200, r#"{"id":"0123456789abcdef0123456789abcdef","name":"p3wr"}"#).await;
        let m = Mojang::with_base(new_client(), &s.base);
        let a = m.get_minecraft_profile("p3wr").await.unwrap().unwrap();
        assert_eq!(a.name, "p3wr");
        m.get_minecraft_profile("p3wr").await.unwrap();
        assert_eq!(s.hits.load(Ordering::SeqCst), 1);
        let nf = testserver::serve(404, "").await;
        let m = Mojang::with_base(new_client(), &nf.base);
        assert_eq!(m.get_minecraft_profile("nobody").await.unwrap(), None);
    }
}
