//! Bazaar and auction house prices: port of src/api/priceFetch.js, src/worker/priceWorker.js and
//! src/api/prices.js.
//!
//! * Bazaar: `products.*.quick_status.buyPrice`, kept when >= 1.
//! * Auctions: page 0 first (unchanged `lastUpdated` -> nothing to do), the other pages 6 at a time with 3 attempts
//!   each (1 s / 2 s back-off); one failed page fails the refresh. Lowest BIN per key (see [`crate::price_nbt`]).
//!   Every auction is decoded once: the derived keys are cached by auction uuid across refreshes and evicted when
//!   the uuid is absent from the newest download. Keys are interned to `u32` so merging pages is array work.
//! * Smoothing: median of the lowest-BIN map over the last `AUCTION_SMOOTH_DOWNLOADS` changed downloads, for the
//!   ids in the newest one.
//! * [`PriceService`]: one background loop per source (5 minutes), `ready()`, `snapshot()` with a version counter.

use crate::price_nbt::{derive_keys, Scratch};
use dps_core::config::constants::{AUCTION_REFRESH_MS, AUCTION_SMOOTH_DOWNLOADS, BAZAAR_REFRESH_MS};
use dps_core::model::{PriceTables, SharedPrices};
use parking_lot::{Mutex, RwLock};
use rustc_hash::FxHashMap;
use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const API: &str = "https://api.hypixel.net/v2/skyblock";
pub const PAGE_CONCURRENCY: usize = 6;
pub const PAGE_ATTEMPTS: u32 = 3;

pub type PriceError = String;

// ---------------------------------------------------------------------------------------------------------------
// HTTP
// ---------------------------------------------------------------------------------------------------------------

/// Downloads bytes from the SkyBlock API (shared connection pool, gzip).
#[derive(Clone)]
pub struct Fetcher {
    client: reqwest::Client,
    api: String,
    /// Back-off unit between page attempts (JS: 1000 ms * attempt).
    pub backoff_ms: u64,
}

impl Fetcher {
    pub fn new() -> Self {
        Self::with_api(API)
    }

    pub fn with_api(api: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .expect("http client");
        Fetcher { client, api: api.trim_end_matches('/').to_owned(), backoff_ms: 1000 }
    }

    /// JS `getJson` without the parse: `None` for 204 / 404, an error for other non-2xx statuses.
    async fn get(&self, url: &str) -> Result<Option<Vec<u8>>, PriceError> {
        let res = self.client.get(url).send().await.map_err(|e| e.to_string())?;
        let status = res.status();
        if status.as_u16() == 204 || status.as_u16() == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            let host = res.url().host_str().unwrap_or("").to_owned();
            let body = res.text().await.unwrap_or_default();
            let body: String = body.chars().take(200).collect();
            return Err(format!("HTTP {} from {}: {}", status.as_u16(), host, body));
        }
        Ok(Some(res.bytes().await.map_err(|e| e.to_string())?.to_vec()))
    }

    pub async fn bazaar_bytes(&self) -> Result<Vec<u8>, PriceError> {
        self.get(&format!("{}/bazaar", self.api)).await?.ok_or_else(|| "Bazaar request failed.".to_owned())
    }

    /// One auction page with retries; the bytes are validated to be a successful page.
    async fn page_bytes(&self, page: usize) -> Result<(Vec<u8>, PageHead), PriceError> {
        let mut attempt = 1u32;
        loop {
            let tried: Result<(Vec<u8>, PageHead), PriceError> = async {
                let bytes = self.get(&format!("{}/auctions?page={page}", self.api)).await?;
                let bytes = bytes.ok_or_else(|| format!("Auction page {page} request failed."))?;
                let head: PageHead = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                if !head.success {
                    return Err(format!("Auction page {page} request failed."));
                }
                Ok((bytes, head))
            }
            .await;
            match tried {
                Ok(v) => return Ok(v),
                Err(e) if attempt >= PAGE_ATTEMPTS => return Err(e),
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(self.backoff_ms * attempt as u64)).await;
                    attempt += 1;
                }
            }
        }
    }
}

impl Default for Fetcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Bazaar
// ---------------------------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct QuickStatus {
    #[serde(rename = "buyPrice", default)]
    buy_price: Option<f64>,
}

#[derive(Deserialize)]
struct Product {
    #[serde(default)]
    quick_status: Option<QuickStatus>,
}

struct Products(Vec<(String, f64)>);

impl<'de> Deserialize<'de> for Products {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Products;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of products")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Products, A::Error> {
                let mut out = Vec::with_capacity(m.size_hint().unwrap_or(2048));
                while let Some(key) = m.next_key::<Cow<'de, str>>()? {
                    let p: Product = m.next_value()?;
                    if let Some(price) = p.quick_status.and_then(|q| q.buy_price) {
                        // Under 1 coin is a troll sell offer, not a real price.
                        if price >= 1.0 {
                            out.push((key.into_owned(), price));
                        }
                    }
                }
                Ok(Products(out))
            }
        }
        d.deserialize_map(V)
    }
}

#[derive(Deserialize)]
struct BazaarResponse {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    products: Option<Products>,
}

/// Parse a bazaar response into `[id, price]` entries (JS `fetchBazaar` after the download).
pub fn parse_bazaar(bytes: &[u8]) -> Result<Vec<(String, f64)>, PriceError> {
    let r: BazaarResponse = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if !r.success {
        return Err("Bazaar request failed.".into());
    }
    Ok(r.products.ok_or_else(|| "Bazaar request failed.".to_owned())?.0)
}

pub async fn fetch_bazaar(f: &Fetcher) -> Result<Vec<(String, f64)>, PriceError> {
    let bytes = f.bazaar_bytes().await?;
    tokio::task::spawn_blocking(move || parse_bazaar(&bytes)).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------------------------------------------
// Auctions
// ---------------------------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct PageHead {
    #[serde(default)]
    success: bool,
    #[serde(rename = "totalPages", default)]
    total_pages: Option<f64>,
    #[serde(rename = "lastUpdated", default)]
    last_updated: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    auctions: Option<IgnoredAny>,
}

#[derive(Deserialize)]
struct Page<'a> {
    #[serde(borrow, default)]
    auctions: Option<Vec<Auction<'a>>>,
}

#[derive(Deserialize)]
struct Auction<'a> {
    #[serde(default)]
    bin: Option<bool>,
    #[serde(borrow, default)]
    uuid: Option<Cow<'a, str>>,
    #[serde(default)]
    starting_bid: Option<f64>,
    #[serde(borrow, default)]
    item_bytes: Option<Cow<'a, str>>,
}

/// Interned price key names.
#[derive(Default)]
pub struct Interner {
    map: FxHashMap<Arc<str>, u32>,
    names: Vec<Arc<str>>,
}

impl Interner {
    fn intern(&mut self, s: String) -> u32 {
        if let Some(&i) = self.map.get(s.as_str()) {
            return i;
        }
        let a: Arc<str> = Arc::from(s);
        let i = self.names.len() as u32;
        self.names.push(a.clone());
        self.map.insert(a, i);
        i
    }
}

/// Derived keys of one auction (`None`: undecodable or no item id).
type Keys = Option<Arc<[u32]>>;
type Cache = FxHashMap<u128, Keys>;

/// What one page contributed.
pub struct PageOut {
    lowest: FxHashMap<u32, f64>,
    seen: Vec<(u128, Keys)>,
    pub decoded: usize,
    pub cached: usize,
}

fn parse_uuid(s: &str) -> Option<u128> {
    if s.len() != 32 {
        return None;
    }
    u128::from_str_radix(s, 16).ok()
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

fn process_page(bytes: &[u8], cache: &Cache, interner: &Mutex<Interner>) -> Result<PageOut, PriceError> {
    let page: Page = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let auctions = page.auctions.unwrap_or_default();
    let mut out = PageOut { lowest: FxHashMap::default(), seen: Vec::with_capacity(auctions.len()), decoded: 0, cached: 0 };
    SCRATCH.with(|s| {
        let mut scratch = s.borrow_mut();
        for a in &auctions {
            if a.bin != Some(true) {
                continue;
            }
            let Some(bid) = a.starting_bid else { continue };
            let uuid = a.uuid.as_deref().and_then(parse_uuid);
            let keys: Keys = match uuid.and_then(|u| cache.get(&u)) {
                Some(k) => {
                    out.cached += 1;
                    k.clone()
                }
                None => {
                    out.decoded += 1;
                    let derived = derive_keys(a.item_bytes.as_deref().unwrap_or(""), &mut scratch);
                    derived.map(|v| {
                        let mut g = interner.lock();
                        v.into_iter().map(|k| g.intern(k)).collect::<Vec<u32>>().into()
                    })
                }
            };
            if let Some(ks) = &keys {
                for &k in ks.iter() {
                    out.lowest.entry(k).and_modify(|cur| if !(*cur <= bid) { *cur = bid }).or_insert(bid);
                }
            }
            if let Some(u) = uuid {
                out.seen.push((u, keys));
            }
        }
    });
    Ok(out)
}

/// Auction state that lives across refreshes: uuid cache, key interner, recent downloads.
pub struct AuctionTracker {
    cache: Arc<Cache>,
    interner: Arc<Mutex<Interner>>,
    /// Lowest BIN per key id for the last changed downloads, oldest first (NaN = not listed).
    history: Vec<Vec<f64>>,
    last_updated: Option<f64>,
}

impl Default for AuctionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl AuctionTracker {
    pub fn new() -> Self {
        AuctionTracker { cache: Arc::new(Cache::default()), interner: Arc::new(Mutex::new(Interner::default())), history: Vec::new(), last_updated: None }
    }

    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// `lastUpdated` of the last accepted download.
    pub fn last_updated(&self) -> Option<f64> {
        self.last_updated
    }

    /// The newest download's lowest BIN per key (unsmoothed).
    pub fn latest_lowest(&self) -> FxHashMap<String, f64> {
        let names = &self.interner.lock().names;
        match self.history.last() {
            Some(m) => m.iter().enumerate().filter(|(_, v)| !v.is_nan()).map(|(i, v)| (names[i].to_string(), *v)).collect(),
            None => FxHashMap::default(),
        }
    }

    /// Process one downloaded page body (decodes only auctions not in the cache). Safe to call from many threads.
    pub fn process(&self, bytes: &[u8]) -> Result<PageOut, PriceError> {
        process_page(bytes, &self.cache, &self.interner)
    }

    /// Accept a complete download: replace the cache with what the newest download listed, push the lowest map
    /// onto the history and return the smoothed prices.
    pub fn accept(&mut self, last_updated: f64, outs: Vec<PageOut>) -> FxHashMap<String, f64> {
        let total: usize = outs.iter().map(|o| o.seen.len()).sum();
        let mut cache = Cache::with_capacity_and_hasher(total, Default::default());
        let mut lowest: Vec<f64> = Vec::new();
        for o in outs {
            for (k, bid) in o.lowest {
                let k = k as usize;
                if lowest.len() <= k {
                    lowest.resize(k + 1, f64::NAN);
                }
                let cur = lowest[k];
                if !(cur <= bid) {
                    lowest[k] = bid;
                }
            }
            for (u, keys) in o.seen {
                cache.insert(u, keys);
            }
        }
        self.cache = Arc::new(cache);
        self.history.push(lowest);
        if self.history.len() > AUCTION_SMOOTH_DOWNLOADS {
            self.history.remove(0);
        }
        self.last_updated = Some(last_updated);
        self.smooth()
    }

    /// JS `smoothLowest(history)`.
    fn smooth(&self) -> FxHashMap<String, f64> {
        let names = &self.interner.lock().names;
        let Some(latest) = self.history.last() else { return FxHashMap::default() };
        let mut out = FxHashMap::with_capacity_and_hasher(latest.len(), Default::default());
        let mut seen: Vec<f64> = Vec::with_capacity(self.history.len());
        for (id, v) in latest.iter().enumerate() {
            if v.is_nan() {
                continue;
            }
            seen.clear();
            for m in &self.history {
                if let Some(x) = m.get(id) {
                    if !x.is_nan() {
                        seen.push(*x);
                    }
                }
            }
            seen.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mid = seen.len() >> 1;
            let price = if seen.len() % 2 == 1 { seen[mid] } else { (seen[mid - 1] + seen[mid]) / 2.0 };
            out.insert(names[id].to_string(), price);
        }
        out
    }

    /// Download and process the auction house. `Ok(None)`: Hypixel has not updated it since the last download.
    /// A page that keeps failing fails the whole refresh and leaves the tracker untouched.
    pub async fn refresh(&mut self, f: &Fetcher) -> Result<Option<FxHashMap<String, f64>>, PriceError> {
        let (first_bytes, head) = f.page_bytes(0).await?;
        let last_updated = head.last_updated.unwrap_or(f64::NAN);
        if let (Some(since), Some(lu)) = (self.last_updated, head.last_updated) {
            if since == lu {
                return Ok(None);
            }
        }
        let total_pages = head.total_pages.filter(|t| t.is_finite()).map_or(1, |t| t.max(1.0).ceil() as usize);
        let next = Arc::new(AtomicUsize::new(1));
        let cache = self.cache.clone();
        let interner = self.interner.clone();
        let mut tasks = Vec::new();
        for _ in 0..PAGE_CONCURRENCY.min(total_pages.saturating_sub(1)) {
            let (f, next, cache, interner) = (f.clone(), next.clone(), cache.clone(), interner.clone());
            tasks.push(tokio::spawn(async move {
                let mut outs = Vec::new();
                loop {
                    let page = next.fetch_add(1, Ordering::Relaxed);
                    if page >= total_pages {
                        return Ok::<_, PriceError>(outs);
                    }
                    let (bytes, _) = f.page_bytes(page).await?;
                    let (c, i) = (cache.clone(), interner.clone());
                    let out = tokio::task::spawn_blocking(move || process_page(&bytes, &c, &i)).await.map_err(|e| e.to_string())??;
                    outs.push(out);
                }
            }));
        }
        let (c, i) = (cache.clone(), interner.clone());
        let first = tokio::task::spawn_blocking(move || process_page(&first_bytes, &c, &i)).await.map_err(|e| e.to_string())??;
        let mut outs = vec![first];
        let mut failure = None;
        for t in tasks {
            match t.await {
                Ok(Ok(mut v)) => outs.append(&mut v),
                Ok(Err(e)) => failure = failure.or(Some(e)),
                Err(e) => failure = failure.or(Some(e.to_string())),
            }
        }
        if let Some(e) = failure {
            return Err(e);
        }
        Ok(Some(self.accept(last_updated, outs)))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------------------------------------------

/// Retry delay while a source has never loaded.
const FIRST_LOAD_RETRY: Duration = Duration::from_secs(30);

struct Inner {
    tables: RwLock<SharedPrices>,
    version: AtomicU64,
    bazaar_ready: AtomicBool,
    auctions_ready: AtomicBool,
    ready_tx: tokio::sync::watch::Sender<bool>,
    fetcher: Fetcher,
    tracker: tokio::sync::Mutex<AuctionTracker>,
    bazaar_lock: tokio::sync::Mutex<()>,
}

/// Shared price cache with background refresh. Cheap to clone.
#[derive(Clone)]
pub struct PriceService {
    inner: Arc<Inner>,
}

impl Default for PriceService {
    fn default() -> Self {
        Self::new()
    }
}

impl PriceService {
    pub fn new() -> Self {
        Self::with_fetcher(Fetcher::new())
    }

    pub fn with_fetcher(fetcher: Fetcher) -> Self {
        let (ready_tx, _) = tokio::sync::watch::channel(false);
        PriceService {
            inner: Arc::new(Inner {
                tables: RwLock::new(Arc::new(PriceTables::default())),
                version: AtomicU64::new(0),
                bazaar_ready: AtomicBool::new(false),
                auctions_ready: AtomicBool::new(false),
                ready_tx,
                fetcher,
                tracker: tokio::sync::Mutex::new(AuctionTracker::new()),
                bazaar_lock: tokio::sync::Mutex::new(()),
            }),
        }
    }

    /// Start the two background loops on the current tokio runtime (first load immediately, then every 5 minutes;
    /// a source that has never loaded retries after 30 s). At most one refresh per source is in flight.
    pub fn start(&self) -> Vec<tokio::task::JoinHandle<()>> {
        let b = self.clone();
        let a = self.clone();
        vec![
            tokio::spawn(async move {
                loop {
                    let t = tokio::time::Instant::now();
                    match b.refresh_bazaar().await {
                        Ok(_) => tokio::time::sleep_until(t + Duration::from_millis(BAZAAR_REFRESH_MS)).await,
                        Err(e) => {
                            eprintln!("Price refresh (bazaar) failed: {e}");
                            let wait = if b.inner.bazaar_ready.load(Ordering::Relaxed) { Duration::from_millis(BAZAAR_REFRESH_MS) } else { FIRST_LOAD_RETRY };
                            tokio::time::sleep_until(t + wait).await
                        }
                    }
                }
            }),
            tokio::spawn(async move {
                loop {
                    let t = tokio::time::Instant::now();
                    match a.refresh_auctions().await {
                        Ok(_) => tokio::time::sleep_until(t + Duration::from_millis(AUCTION_REFRESH_MS)).await,
                        Err(e) => {
                            eprintln!("Price refresh (auctions) failed: {e}");
                            let wait = if a.inner.auctions_ready.load(Ordering::Relaxed) { Duration::from_millis(AUCTION_REFRESH_MS) } else { FIRST_LOAD_RETRY };
                            tokio::time::sleep_until(t + wait).await
                        }
                    }
                }
            }),
        ]
    }

    /// True once both sources have loaded at least once.
    pub fn ready(&self) -> bool {
        self.inner.bazaar_ready.load(Ordering::Acquire) && self.inner.auctions_ready.load(Ordering::Acquire)
    }

    /// Resolves once [`ready`](Self::ready) is true.
    pub async fn wait_ready(&self) {
        let mut rx = self.inner.ready_tx.subscribe();
        while !*rx.borrow_and_update() {
            if rx.changed().await.is_err() {
                return;
            }
        }
    }

    /// The current tables, frozen: a refresh swaps in new ones and never edits these (JS `snapshotPrices`).
    pub fn snapshot(&self) -> SharedPrices {
        self.inner.tables.read().clone()
    }

    /// Counter bumped whenever a refresh changed the tables.
    pub fn version(&self) -> u64 {
        self.inner.version.load(Ordering::Acquire)
    }

    fn mark_ready(&self, bazaar: bool) {
        let flag = if bazaar { &self.inner.bazaar_ready } else { &self.inner.auctions_ready };
        flag.store(true, Ordering::Release);
        if self.ready() {
            self.inner.ready_tx.send_replace(true);
        }
    }

    fn swap(&self, bazaar: Option<FxHashMap<String, f64>>, auctions: Option<FxHashMap<String, f64>>) {
        let mut g = self.inner.tables.write();
        let version = self.inner.version.fetch_add(1, Ordering::AcqRel) + 1;
        let mut t = PriceTables::clone(&g);
        if let Some(b) = bazaar {
            t.bazaar = b;
        }
        if let Some(a) = auctions {
            t.auctions = a;
        }
        t.version = version;
        *g = Arc::new(t);
    }

    /// Install tables directly (tests, tools). Marks both sources loaded.
    pub fn set_tables(&self, bazaar: FxHashMap<String, f64>, auctions: FxHashMap<String, f64>) {
        self.swap(Some(bazaar), Some(auctions));
        self.mark_ready(true);
        self.mark_ready(false);
    }

    /// Download the bazaar once (callers share the in-flight refresh by waiting on the lock).
    pub async fn refresh_bazaar(&self) -> Result<(), PriceError> {
        let _guard = self.inner.bazaar_lock.lock().await;
        let entries = fetch_bazaar(&self.inner.fetcher).await?;
        self.swap(Some(entries.into_iter().collect()), None);
        self.mark_ready(true);
        Ok(())
    }

    /// Download the auction house once. `Ok(false)`: unchanged since the last download.
    pub async fn refresh_auctions(&self) -> Result<bool, PriceError> {
        let mut tracker = self.inner.tracker.lock().await;
        match tracker.refresh(&self.inner.fetcher).await? {
            Some(map) => {
                self.swap(None, Some(map));
                self.mark_ready(false);
                Ok(true)
            }
            None => {
                self.mark_ready(false);
                Ok(false)
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests;
