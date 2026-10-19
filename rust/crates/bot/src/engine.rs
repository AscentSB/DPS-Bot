//! The seam between the /dps flow and the other crates (Mojang, Hypixel, prices, planner). The flow in `dps.rs` only
//! talks to the [`Engine`] trait; the integration step provides the real implementation, [`StubEngine`] serves the
//! golden test data.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dps_core::model::{Account, DpsJob, DpsResult, Member};
use parking_lot::Mutex;

use crate::transport::BoxFuture;

/// A plan that takes longer than this is stopped (JS `PLAN_TIMEOUT_MS`, env `PLAN_TIMEOUT_MS`, default 3 minutes).
pub fn plan_timeout() -> Duration {
    let ms = std::env::var("PLAN_TIMEOUT_MS").ok().and_then(|v| v.trim().parse::<f64>().ok()).filter(|v| v.is_finite() && *v > 0.0).unwrap_or(3.0 * 60.0 * 1000.0);
    Duration::from_millis(ms as u64)
}

/// Why looking up an account or loading a member failed. The variants decide the reply text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineError {
    /// Mojang could not be reached (any lookup failure).
    MojangDown,
    /// HYPIXEL_API_KEY missing or invalid, or HTTP 403.
    HypixelKey,
    /// HTTP 429.
    HypixelRateLimit,
    /// Any other Hypixel failure; carries the error message (not shown).
    Hypixel(String),
    /// readMember explained a profile with its Inventory API turned off; the message is ours and shown as is.
    InventoryDisabled(String),
    /// Anything else (generic failure reply).
    Other(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::MojangDown => f.write_str("Mojang is not reachable"),
            EngineError::HypixelKey => f.write_str("HYPIXEL_API_KEY is missing or invalid (HTTP 403)"),
            EngineError::HypixelRateLimit => f.write_str("HTTP 429"),
            EngineError::Hypixel(m) | EngineError::InventoryDisabled(m) | EngineError::Other(m) => f.write_str(m),
        }
    }
}
impl std::error::Error for EngineError {}

/// Why a plan produced no result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    Timeout,
    Cancelled,
    Failed(String),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::Timeout => f.write_str("Plan took too long."),
            PlanError::Cancelled => f.write_str("Plan cancelled."),
            PlanError::Failed(m) => f.write_str(m),
        }
    }
}
impl std::error::Error for PlanError {}

/// What the /dps flow needs from the rest of the bot.
pub trait Engine: Send + Sync {
    /// Prices have loaded at least once (JS `pricesReady()`).
    fn prices_ready(&self) -> bool;
    /// Mojang lookup. `Ok(None)` = no such account; `Err` = Mojang unreachable.
    fn lookup_account(&self, ign: &str) -> BoxFuture<Result<Option<Account>, EngineError>>;
    /// The selected profile's member (items resource, collections and profile fetched, then `readMember`).
    /// `Ok(None)` = the account has no SkyBlock profiles.
    fn load_member(&self, uuid: &str) -> BoxFuture<Result<Option<Arc<Member>>, EngineError>>;
    /// Run one plan. Must stop early with `PlanError::Cancelled` when `cancel` becomes true, and fail with
    /// `PlanError::Timeout` after [`plan_timeout`]. Never runs on the tokio worker threads.
    fn run(&self, member: Arc<Member>, job: DpsJob, cancel: Arc<AtomicBool>) -> BoxFuture<Result<DpsResult, PlanError>>;
}

// ---------------------------------------------------------------------------------------------------------------
// StubEngine: serves rust/testdata (profiles, members, golden plans).
// ---------------------------------------------------------------------------------------------------------------

/// Serves `testdata/profiles`, `members` and `golden`. The account name is the profile file name; a plan is the
/// golden file `<profile>__<cls>-<mode>-<floorMode>-<upgradeMode>[-anychest][-<target>][-b<p>.<w>.<t>]-<menu>.json`.
pub struct StubEngine {
    dir: PathBuf,
    ready: AtomicBool,
    /// Every `run` waits this long (polling `cancel` every 5 ms), to test timers and cancellation.
    delay: Mutex<Duration>,
    account_error: Mutex<Option<EngineError>>,
    member_error: Mutex<Option<EngineError>>,
    plan_error: Mutex<Option<PlanError>>,
    /// Member allocation address -> profile name, so `run` knows which golden file to serve.
    names: Mutex<HashMap<usize, String>>,
    members: Mutex<HashMap<String, Arc<Member>>>,
    runs: AtomicUsize,
    cancelled: Arc<AtomicUsize>,
    jobs: Mutex<Vec<DpsJob>>,
}

impl StubEngine {
    /// `DPS_TESTDATA` or the crate-relative `rust/testdata`.
    pub fn new() -> Self {
        let dir = std::env::var("DPS_TESTDATA").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")));
        Self::with_dir(dir)
    }

    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        StubEngine {
            dir: dir.into(),
            ready: AtomicBool::new(true),
            delay: Mutex::new(Duration::ZERO),
            account_error: Mutex::new(None),
            member_error: Mutex::new(None),
            plan_error: Mutex::new(None),
            names: Mutex::new(HashMap::new()),
            members: Mutex::new(HashMap::new()),
            runs: AtomicUsize::new(0),
            cancelled: Arc::new(AtomicUsize::new(0)),
            jobs: Mutex::new(Vec::new()),
        }
    }

    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::SeqCst);
    }
    pub fn set_delay(&self, d: Duration) {
        *self.delay.lock() = d;
    }
    /// The next `lookup_account` / `load_member` / `run` fails with this (taken once).
    pub fn fail_account(&self, e: EngineError) {
        *self.account_error.lock() = Some(e);
    }
    pub fn fail_member(&self, e: EngineError) {
        *self.member_error.lock() = Some(e);
    }
    pub fn fail_plan(&self, e: PlanError) {
        *self.plan_error.lock() = Some(e);
    }
    /// Plans started so far.
    pub fn runs(&self) -> usize {
        self.runs.load(Ordering::SeqCst)
    }
    /// Plans that ended because their cancel flag was set.
    pub fn cancelled(&self) -> usize {
        self.cancelled.load(Ordering::SeqCst)
    }
    /// The jobs `run` received, in order.
    pub fn jobs(&self) -> Vec<DpsJob> {
        self.jobs.lock().clone()
    }

    fn profile_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(self.dir.join("profiles")) {
            for f in rd.flatten() {
                if let Some(n) = f.file_name().to_str().and_then(|n| n.strip_suffix(".json")) {
                    out.push(n.to_string());
                }
            }
        }
        out.sort();
        out
    }

    fn read_json(&self, rel: &str) -> Result<serde_json::Value, String> {
        let text = std::fs::read_to_string(self.dir.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("{rel}: {e}"))
    }

    /// The golden case id for a job.
    pub fn case_id(profile: &str, job: &DpsJob) -> String {
        let mut id = format!("{profile}__{}-{}-{}-{}", job.cls, job.mode, job.floor_mode, job.upgrade_mode);
        if job.any_chest {
            id.push_str("-anychest");
        }
        if job.no_clear_boots {
            id.push_str("-noclearboots");
        }
        if let Some(t) = &job.target {
            id.push('-');
            id.push_str(t);
        }
        let b = job.blessings;
        if (b.power, b.wisdom, b.time) != (29, 14, 5) {
            id.push_str(&format!("-b{}.{}.{}", b.power, b.wisdom, b.time));
        }
        id.push('-');
        id.push_str(&job.menu);
        id
    }
}

impl Default for StubEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine for StubEngine {
    fn prices_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    fn lookup_account(&self, ign: &str) -> BoxFuture<Result<Option<Account>, EngineError>> {
        let res = (|| {
            if let Some(e) = self.account_error.lock().take() {
                return Err(e);
            }
            let Some(name) = self.profile_names().into_iter().find(|n| n.eq_ignore_ascii_case(ign)) else { return Ok(None) };
            let v = self.read_json(&format!("profiles/{name}.json")).map_err(EngineError::Other)?;
            let uuid = v["uuid"].as_str().unwrap_or_default().to_string();
            Ok(Some(Account { uuid, name }))
        })();
        Box::pin(async move { res })
    }

    fn load_member(&self, uuid: &str) -> BoxFuture<Result<Option<Arc<Member>>, EngineError>> {
        let res = (|| {
            if let Some(e) = self.member_error.lock().take() {
                return Err(e);
            }
            let Some(name) = self.profile_names().into_iter().find(|n| {
                self.read_json(&format!("profiles/{n}.json")).ok().is_some_and(|v| v["uuid"].as_str() == Some(uuid))
            }) else {
                return Ok(None);
            };
            if let Some(m) = self.members.lock().get(&name) {
                return Ok(Some(m.clone()));
            }
            let v = self.read_json(&format!("members/{name}.json")).map_err(EngineError::Other)?;
            if let Some(err) = v["error"].as_str() {
                return Err(EngineError::InventoryDisabled(err.to_string()));
            }
            let member: Member = serde_json::from_value(v).map_err(|e| EngineError::Other(e.to_string()))?;
            let member = Arc::new(member);
            self.names.lock().insert(Arc::as_ptr(&member) as usize, name.clone());
            self.members.lock().insert(name, member.clone());
            Ok(Some(member))
        })();
        Box::pin(async move { res })
    }

    fn run(&self, member: Arc<Member>, job: DpsJob, cancel: Arc<AtomicBool>) -> BoxFuture<Result<DpsResult, PlanError>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        self.jobs.lock().push(job.clone());
        let delay = *self.delay.lock();
        let forced = self.plan_error.lock().take();
        let name = self.names.lock().get(&(Arc::as_ptr(&member) as usize)).cloned();
        let result = match (&forced, name) {
            (Some(_), _) => Err(PlanError::Failed(String::new())),
            (None, None) => Err(PlanError::Failed("unknown member".into())),
            (None, Some(name)) => {
                let id = Self::case_id(&name, &job);
                self.read_json(&format!("golden/{id}.json"))
                    .map_err(PlanError::Failed)
                    .and_then(|v| serde_json::from_value::<DpsResult>(v).map_err(|e| PlanError::Failed(format!("{id}: {e}"))))
            }
        };
        let result = match forced {
            Some(e) => Err(e),
            None => result,
        };
        let cancelled = self.cancelled.clone();
        Box::pin(async move {
            let start = tokio::time::Instant::now();
            while start.elapsed() < delay {
                if cancel.load(Ordering::SeqCst) {
                    cancelled.fetch_add(1, Ordering::SeqCst);
                    return Err(PlanError::Cancelled);
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            if cancel.load(Ordering::SeqCst) {
                cancelled.fetch_add(1, Ordering::SeqCst);
                return Err(PlanError::Cancelled);
            }
            result
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// RealEngine: Mojang, Hypixel, prices and the planner.
// ---------------------------------------------------------------------------------------------------------------

use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use dps_skyblock::hypixel::Hypixel;
use dps_skyblock::mojang::Mojang;
use dps_skyblock::prices::PriceService;
use tokio::sync::Semaphore;

/// Plan results kept for repeated requests (RAM is cheap, a plan is seconds of CPU).
const PLAN_CACHE_MAX: usize = 512;

type CacheKey = (u64, u64);

#[derive(Default)]
struct PlanCache {
    map: HashMap<CacheKey, DpsResult>,
    order: VecDeque<CacheKey>,
}

pub struct RealEngine {
    hypixel: Hypixel,
    mojang: Arc<Mojang>,
    prices: PriceService,
    permits: Arc<Semaphore>,
    cache: Arc<Mutex<PlanCache>>,
}

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

/// 128-bit content key: two differently seeded hashes of the bytes.
fn content_key(parts: &[&[u8]]) -> CacheKey {
    let mut a = rustc_hash::FxHasher::default();
    let mut b = std::collections::hash_map::DefaultHasher::new();
    for p in parts {
        p.hash(&mut a);
        p.hash(&mut b);
    }
    (a.finish(), b.finish())
}

impl RealEngine {
    /// Builds the services; call inside the tokio runtime. `start_background` starts the price refresh.
    pub fn new() -> Arc<Self> {
        let client = dps_skyblock::http::new_client();
        let workers = std::env::var("PLAN_WORKERS").ok().and_then(|v| v.trim().parse::<f64>().ok()).filter(|v| v.is_finite()).map(|v| v.max(1.0) as usize).unwrap_or(2);
        Arc::new(RealEngine {
            hypixel: Hypixel::new(client.clone()),
            mojang: Arc::new(Mojang::new(client)),
            prices: PriceService::new(),
            permits: Arc::new(Semaphore::new(workers)),
            cache: Arc::new(Mutex::new(PlanCache::default())),
        })
    }

    /// Price refresh loops plus a warm-up of the item and collection lists (hourly refresh is inside `Hypixel`).
    pub fn start_background(&self) {
        let _ = self.prices.start();
        let h = self.hypixel.clone();
        tokio::spawn(async move {
            if let Err(e) = h.items_resource().await {
                eprintln!("Item list load failed: {e}");
            }
            let _ = h.collections_resource().await;
        });
    }

    pub fn prices(&self) -> &PriceService {
        &self.prices
    }
    pub fn hypixel(&self) -> &Hypixel {
        &self.hypixel
    }
}

fn classify_hypixel(msg: String) -> EngineError {
    if msg.contains("HYPIXEL_API_KEY") || msg.contains("HTTP 403") {
        EngineError::HypixelKey
    } else if msg.contains("HTTP 429") {
        EngineError::HypixelRateLimit
    } else {
        EngineError::Hypixel(msg)
    }
}

impl Engine for RealEngine {
    fn prices_ready(&self) -> bool {
        self.prices.ready()
    }

    fn lookup_account(&self, ign: &str) -> BoxFuture<Result<Option<Account>, EngineError>> {
        let m = self.mojang.clone();
        let ign = ign.to_string();
        Box::pin(async move { m.get_minecraft_profile(&ign).await.map_err(|e| {
            eprintln!("{e}");
            EngineError::MojangDown
        }) })
    }

    fn load_member(&self, uuid: &str) -> BoxFuture<Result<Option<Arc<Member>>, EngineError>> {
        let h = self.hypixel.clone();
        let uuid = uuid.to_string();
        Box::pin(async move {
            let (resource, profile, collections) = tokio::join!(h.items_resource(), h.get_selected_profile(&uuid), h.collections_resource());
            let (resource, profile) = match (resource, profile) {
                (Ok(r), Ok(p)) => (r, p),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("{e}");
                    return Err(classify_hypixel(e));
                }
            };
            let Some(profile) = profile else { return Ok(None) };
            let collections = collections.ok();
            // About 3 ms of work: cheaper inline than a thread hop.
            match dps_skyblock::hypixel::read_member(&profile, &uuid, &resource, collections.as_deref(), now_ms()) {
                Ok(m) => Ok(Some(Arc::new(m))),
                Err(e) if e.contains("Inventory API is disabled") => Err(EngineError::InventoryDisabled(e)),
                Err(e) => {
                    eprintln!("{e}");
                    Err(EngineError::Other(e))
                }
            }
        })
    }

    fn run(&self, member: Arc<Member>, job: DpsJob, cancel: Arc<AtomicBool>) -> BoxFuture<Result<DpsResult, PlanError>> {
        let h = self.hypixel.clone();
        let prices = self.prices.clone();
        let permits = self.permits.clone();
        let cache = self.cache.clone();
        Box::pin(async move {
            let snap = prices.snapshot();
            let key = {
                let m = serde_json::to_vec(&*member).map_err(|e| PlanError::Failed(e.to_string()))?;
                let j = serde_json::to_vec(&job).map_err(|e| PlanError::Failed(e.to_string()))?;
                content_key(&[&m, &j, &snap.version.to_le_bytes()])
            };
            if let Some(hit) = cache.lock().map.get(&key) {
                return Ok(hit.clone());
            }
            let resource = h.items_resource().await.map_err(PlanError::Failed)?;
            let permit = permits.acquire_owned().await.map_err(|e| PlanError::Failed(e.to_string()))?;
            if cancel.load(Ordering::SeqCst) {
                return Err(PlanError::Cancelled);
            }
            let flag = cancel.clone();
            let task = tokio::task::spawn_blocking(move || {
                // The permit is held until the thread has really finished, also after a timeout.
                let _permit = permit;
                dps_calc::run_dps(&member, &job, &resource, &snap, &flag)
            });
            let out = match tokio::time::timeout(plan_timeout(), task).await {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => return Err(PlanError::Failed(format!("Plan thread failed: {e}"))),
                Err(_) => {
                    // Stops the plan at its next step.
                    cancel.store(true, Ordering::SeqCst);
                    return Err(PlanError::Timeout);
                }
            };
            match out {
                Ok(r) => {
                    let mut c = cache.lock();
                    if c.map.insert(key, r.clone()).is_none() {
                        c.order.push_back(key);
                        while c.order.len() > PLAN_CACHE_MAX {
                            if let Some(old) = c.order.pop_front() {
                                c.map.remove(&old);
                            }
                        }
                    }
                    Ok(r)
                }
                Err(dps_calc::PlanError::Cancelled) => Err(PlanError::Cancelled),
                Err(e) => Err(PlanError::Failed(e.to_string())),
            }
        })
    }
}
