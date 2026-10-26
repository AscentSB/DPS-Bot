//! Port of the network half of src/api/hypixel.js: the items and collections resources (hourly refresh, stale while
//! refreshing, one request in flight) and the selected-profile cache (5 minutes, shared in-flight request).
//! `readMember` and its helpers live in [`crate::member`] and are re-exported here.

use crate::http::{get_json, Client};
use crate::member::Json;
use crate::mojang::is_valid_uuid;
use dps_core::config::constants::{ITEMS_REFRESH_MS, PROFILE_CACHE_MS};
use dps_core::model::{CollectionInfo, CollectionsResource, ItemInfo, Resource};
use indexmap::IndexMap;
use parking_lot::Mutex;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::watch;

pub use crate::member::{
    active_members, coop_collection, read_collections, read_member, read_shards, read_tuning, wolf_slayer_speed,
};

const API: &str = "https://api.hypixel.net/v2";

type Shared<T> = Option<Result<T, String>>;

async fn wait_for<T: Clone>(mut rx: watch::Receiver<Shared<T>>) -> Result<T, String> {
    match rx.wait_for(|v| v.is_some()).await {
        Ok(v) => v.clone().unwrap(),
        Err(_) => Err("Request was dropped.".into()),
    }
}

type Fetch<T> = Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<Arc<T>, String>> + Send>> + Send + Sync>;

struct CacheState<T> {
    value: Option<(Arc<T>, Instant)>,
    loading: Option<watch::Receiver<Shared<Arc<T>>>>,
}

/// A value fetched on demand and refreshed after `refresh`: loaded once (callers share the request), then a stale
/// value is served while the new one loads in the background.
struct Cached<T> {
    label: &'static str,
    refresh: Duration,
    state: Mutex<CacheState<T>>,
    fetch: Fetch<T>,
}

impl<T: Send + Sync + 'static> Cached<T> {
    fn new(label: &'static str, refresh: Duration, fetch: Fetch<T>) -> Arc<Self> {
        Arc::new(Cached { label, refresh, state: Mutex::new(CacheState { value: None, loading: None }), fetch })
    }

    async fn get(self: &Arc<Self>) -> Result<Arc<T>, String> {
        let rx = {
            let mut st = self.state.lock();
            if let Some((v, at)) = &st.value {
                if at.elapsed() < self.refresh {
                    return Ok(v.clone());
                }
            }
            let stale = st.value.as_ref().map(|(v, _)| v.clone());
            if st.loading.is_none() {
                let (tx, rx) = watch::channel(None);
                st.loading = Some(rx);
                let this = self.clone();
                let fut = (self.fetch)();
                let had_stale = stale.is_some();
                tokio::spawn(async move {
                    // A panic in the fetch must not leave `loading` set forever (every later caller would wait).
                    struct Clear<T: Send + Sync + 'static>(Arc<Cached<T>>);
                    impl<T: Send + Sync + 'static> Drop for Clear<T> {
                        fn drop(&mut self) {
                            self.0.state.lock().loading = None;
                        }
                    }
                    let _clear = Clear(this.clone());
                    let r = fut.await;
                    {
                        let mut st = this.state.lock();
                        st.loading = None;
                        if let Ok(v) = &r {
                            st.value = Some((v.clone(), Instant::now()));
                        }
                    }
                    if let (true, Err(e)) = (had_stale, &r) {
                        eprintln!("{} refresh failed: {e}", this.label);
                    }
                    tx.send_replace(Some(r));
                });
            }
            if let Some(v) = stale {
                return Ok(v);
            }
            st.loading.clone().unwrap()
        };
        wait_for(rx).await
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MaybeItem {
    Ok(ItemInfo),
    Bad(serde::de::IgnoredAny),
}

#[derive(Deserialize, Default)]
struct ItemsResp {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    items: Vec<MaybeItem>,
}

#[derive(Deserialize, Default)]
struct Tier {
    #[serde(default, rename = "amountRequired")]
    amount_required: Option<f64>,
}

#[derive(Deserialize, Default)]
struct Coll {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tiers: Option<Vec<Tier>>,
}

#[derive(Deserialize, Default)]
struct Category {
    #[serde(default)]
    items: Option<IndexMap<String, Coll>>,
}

#[derive(Deserialize, Default)]
struct CollectionsResp {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    collections: Option<IndexMap<String, Category>>,
}

#[derive(Deserialize, Default)]
struct ProfilesResp {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    cause: Option<String>,
    #[serde(default)]
    profiles: Option<Vec<Json>>,
}

enum ProfileEntry {
    Loading(watch::Receiver<Shared<Option<Arc<Json>>>>),
    Ready(Option<Arc<Json>>, Instant),
}

struct Inner {
    client: Client,
    base: String,
    api_key: Option<String>,
    items: Arc<Cached<Resource>>,
    collections: Arc<Cached<CollectionsResource>>,
    profiles: Mutex<FxHashMap<String, ProfileEntry>>,
}

/// Hypixel API access with its caches. Cheap to clone; clones share the caches.
#[derive(Clone)]
pub struct Hypixel(Arc<Inner>);

impl Hypixel {
    /// The real API; the key is read from `HYPIXEL_API_KEY` when a profile is fetched.
    pub fn new(client: Client) -> Self {
        Self::with_base(client, API, None)
    }

    /// Custom base URL (tests) and an explicit key (`None` = read `HYPIXEL_API_KEY` at fetch time).
    pub fn with_base(client: Client, base: &str, api_key: Option<String>) -> Self {
        let base = base.trim_end_matches('/').to_string();
        let items = {
            let (c, b) = (client.clone(), base.clone());
            Cached::new(
                "Item list",
                Duration::from_millis(ITEMS_REFRESH_MS),
                Box::new(move || {
                    let (c, b) = (c.clone(), b.clone());
                    Box::pin(async move { fetch_items(&c, &b).await })
                }),
            )
        };
        let collections = {
            let (c, b) = (client.clone(), base.clone());
            Cached::new(
                "Collections",
                Duration::from_millis(ITEMS_REFRESH_MS),
                Box::new(move || {
                    let (c, b) = (c.clone(), b.clone());
                    Box::pin(async move { fetch_collections(&c, &b).await })
                }),
            )
        };
        Hypixel(Arc::new(Inner { client, base, api_key, items, collections, profiles: Mutex::new(FxHashMap::default()) }))
    }

    /// Item resource keyed by ITEM_ID, in API order.
    pub async fn items_resource(&self) -> Result<Arc<Resource>, String> {
        self.0.items.get().await
    }

    /// Collections resource: id -> { name, max }.
    pub async fn collections_resource(&self) -> Result<Arc<CollectionsResource>, String> {
        self.0.collections.get().await
    }

    /// The player's selected SkyBlock profile (`None` when the player has none), reused for 5 minutes so repeat runs
    /// do not spend the API key. Concurrent callers share one request.
    pub async fn get_selected_profile(&self, uuid: &str) -> Result<Option<Arc<Json>>, String> {
        if !is_valid_uuid(uuid) {
            return Err("Invalid UUID.".into());
        }
        let rx = {
            let mut map = self.0.profiles.lock();
            let ttl = Duration::from_millis(PROFILE_CACHE_MS);
            map.retain(|_, e| match e {
                ProfileEntry::Ready(_, at) => at.elapsed() <= ttl,
                ProfileEntry::Loading(_) => true,
            });
            match map.get(uuid) {
                Some(ProfileEntry::Ready(p, _)) => return Ok(p.clone()),
                Some(ProfileEntry::Loading(rx)) => rx.clone(),
                None => {
                    let (tx, rx) = watch::channel(None);
                    map.insert(uuid.to_string(), ProfileEntry::Loading(rx.clone()));
                    let inner = self.0.clone();
                    let uuid = uuid.to_string();
                    tokio::spawn(async move {
                        // A panic in the fetch must not leave the Loading entry in the map forever.
                        struct Clear(Arc<Inner>, String);
                        impl Drop for Clear {
                            fn drop(&mut self) {
                                let mut map = self.0.profiles.lock();
                                if matches!(map.get(&self.1), Some(ProfileEntry::Loading(_))) {
                                    map.remove(&self.1);
                                }
                            }
                        }
                        let _clear = Clear(inner.clone(), uuid.clone());
                        let r = fetch_selected_profile(&inner, &uuid).await;
                        {
                            let mut map = inner.profiles.lock();
                            match &r {
                                Ok(p) => {
                                    map.insert(uuid, ProfileEntry::Ready(p.clone(), Instant::now()));
                                }
                                Err(_) => {
                                    map.remove(&uuid);
                                }
                            }
                        }
                        tx.send_replace(Some(r));
                    });
                    rx
                }
            }
        };
        wait_for(rx).await
    }
}

async fn fetch_items(client: &Client, base: &str) -> Result<Arc<Resource>, String> {
    let data: Option<ItemsResp> = get_json(client, &format!("{base}/resources/skyblock/items"), &[]).await?;
    let data = data.filter(|d| d.success).ok_or("Could not load the SkyBlock items resource.")?;
    let mut map = Resource::with_capacity(data.items.len());
    for item in data.items {
        if let MaybeItem::Ok(i) = item {
            map.insert(i.id.clone(), i);
        }
    }
    dps_core::config::accessories::add_missing_items(&mut map);
    Ok(Arc::new(map))
}

async fn fetch_collections(client: &Client, base: &str) -> Result<Arc<CollectionsResource>, String> {
    let data: Option<CollectionsResp> = get_json(client, &format!("{base}/resources/skyblock/collections"), &[]).await?;
    let data = data.filter(|d| d.success).ok_or("Could not load the SkyBlock collections resource.")?;
    let mut map = CollectionsResource::new();
    for (_, category) in data.collections.unwrap_or_default() {
        for (id, c) in category.items.unwrap_or_default() {
            let max = c
                .tiers
                .unwrap_or_default()
                .iter()
                .fold(0.0, |m, t| dps_core::jsmath::max(m, t.amount_required.unwrap_or(0.0)));
            if max > 0.0 {
                let name = c.name.unwrap_or_else(|| id.clone());
                map.insert(id, CollectionInfo { name, max });
            }
        }
    }
    Ok(Arc::new(map))
}

async fn fetch_selected_profile(inner: &Inner, uuid: &str) -> Result<Option<Arc<Json>>, String> {
    let key = match &inner.api_key {
        Some(k) if !k.is_empty() => k.clone(),
        Some(_) => return Err("HYPIXEL_API_KEY is not set.".into()),
        None => std::env::var("HYPIXEL_API_KEY").ok().filter(|k| !k.is_empty()).ok_or("HYPIXEL_API_KEY is not set.")?,
    };
    let url = format!("{}/skyblock/profiles?uuid={uuid}", inner.base);
    let data: Option<ProfilesResp> = get_json(&inner.client, &url, &[("API-Key", key.as_str())]).await?;
    let data = match data {
        Some(d) if d.success => d,
        Some(d) => return Err(d.cause.unwrap_or_else(|| "Hypixel API request failed.".into())),
        None => return Err("Hypixel API request failed.".into()),
    };
    let profiles = data.profiles.unwrap_or_default();
    if profiles.is_empty() {
        return Ok(None);
    }
    let idx = profiles.iter().position(|p| p.get("selected").is_some_and(Json::truthy)).unwrap_or(0);
    Ok(profiles.into_iter().nth(idx).map(Arc::new))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{new_client, testserver};
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn a_profile_is_fetched_once_and_reused_within_the_cache_window() {
        let s = testserver::serve(200, r#"{"success":true,"profiles":[{"selected":false,"profile_id":"y"},{"selected":true,"profile_id":"x"}]}"#).await;
        let h = Hypixel::with_base(new_client(), &s.base, Some("test".into()));
        let uuid = "a".repeat(32);
        let (a, b) = tokio::join!(h.get_selected_profile(&uuid), h.get_selected_profile(&uuid));
        let (a, b) = (a.unwrap().unwrap(), b.unwrap().unwrap());
        let c = h.get_selected_profile(&uuid).await.unwrap().unwrap();
        assert_eq!(a.get("profile_id").and_then(Json::as_str), Some("x"));
        assert!(Arc::ptr_eq(&a, &b) && Arc::ptr_eq(&b, &c));
        assert_eq!(s.hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn errors_are_not_cached_and_uuids_are_checked() {
        let h = Hypixel::with_base(new_client(), "http://127.0.0.1:1", Some("k".into()));
        assert_eq!(h.get_selected_profile("nope").await.unwrap_err(), "Invalid UUID.");
        let s = testserver::serve(200, r#"{"success":false,"cause":"Invalid API key"}"#).await;
        let h = Hypixel::with_base(new_client(), &s.base, Some("k".into()));
        let uuid = "b".repeat(32);
        assert!(h.get_selected_profile(&uuid).await.is_err());
        assert!(h.get_selected_profile(&uuid).await.is_err());
        assert_eq!(s.hits.load(Ordering::SeqCst), 2);
        let none = Hypixel::with_base(new_client(), &s.base, Some(String::new()));
        assert_eq!(none.get_selected_profile(&uuid).await.unwrap_err(), "HYPIXEL_API_KEY is not set.");
    }

    #[tokio::test]
    async fn resources_load_once() {
        let s = testserver::serve(200, r#"{"success":true,"items":[{"id":"A","name":"a"},{"id":"B"},{"id":"A","name":"a2"}]}"#).await;
        let h = Hypixel::with_base(new_client(), &s.base, None);
        let (r1, r2) = tokio::join!(h.items_resource(), h.items_resource());
        let r1 = r1.unwrap();
        assert!(Arc::ptr_eq(&r1, &r2.unwrap()));
        assert_eq!(r1.keys().collect::<Vec<_>>(), ["A", "B", "BALLOON_HAT_2024", "BALLOON_HAT_2025"]);
        assert_eq!(r1["BALLOON_HAT_2024"].tier.as_deref(), Some("SPECIAL"));
        assert_eq!(r1["A"].name.as_deref(), Some("a2"));
        assert_eq!(s.hits.load(Ordering::SeqCst), 1);
    }
}
