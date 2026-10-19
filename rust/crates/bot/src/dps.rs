//! The /dps command and its buttons (port of `execute()` and `handleButton()` in src/commands/dps.js): cooldown,
//! option parsing, replies for every failure, sessions with owner-only buttons and an inactivity timer, and the
//! lists (profile, max levels, Skeleton Master chest) computed on first open and cached per plan key.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use dps_core::config::constants::blessings_with;
use dps_core::model::{DpsJob, Member};
use dps_core::{Class, FloorMode, Mode, Target};
use parking_lot::Mutex;
use tokio::task::JoinHandle;

use crate::embed::{build_embed, buttons, escape_markdown};
use crate::engine::{Engine, EngineError, PlanError};
use crate::session::*;
use crate::transport::{Interaction, Message};
use crate::usage::UsageCounter;

/// The bot-token / interaction state of one /dps message. All fields behind one lock, never held across an await.
struct Live {
    s: Session,
    member: Arc<Member>,
    /// The latest interaction on this message (for the expiry edit through its token).
    interaction: Interaction,
    timer: Option<JoinHandle<()>>,
    /// Cancel flag of the list being computed, if any.
    cancel: Option<Arc<AtomicBool>>,
}

type Entry = Arc<Mutex<Live>>;

pub struct Flow {
    engine: Arc<dyn Engine>,
    usage: UsageCounter,
    bot_token: Option<String>,
    inactivity: std::time::Duration,
    cooldowns: Mutex<HashMap<String, Instant>>,
    sessions: Mutex<HashMap<String, Entry>>,
}

/// `isValidIgn`: 2 to 16 ASCII letters, digits or underscores.
pub fn is_valid_ign(name: &str) -> bool {
    (2..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn class_of(sub: &str) -> Option<Class> {
    match sub {
        "archer" => Some(Class::Arch),
        "berserker" => Some(Class::Bers),
        "mage" => Some(Class::Mage),
        _ => None,
    }
}

fn view_message(id: &str, s: &Session) -> Message {
    let rows = match buttons(id, s) {
        serde_json::Value::Array(a) => a,
        other => vec![other],
    };
    Message { embeds: vec![build_embed(s)], components: rows, ..Default::default() }
}

fn job_of(s: &Session, upgrade_mode: UpgradeMode, menu: &str, limit: Option<usize>) -> DpsJob {
    DpsJob {
        cls: s.cls.js_name().into(),
        target: s.target.map(|t| t.js_name().into()),
        mode: s.mode.js_name().into(),
        floor_mode: s.floor_mode.js_name().into(),
        upgrade_mode: upgrade_mode.js_name().into(),
        menu: menu.into(),
        limit,
        blessings: s.blessings.unwrap_or_default(),
        any_chest: s.any_chest,
        no_clear_boots: s.no_clear_boots,
    }
}

impl Flow {
    pub fn new(engine: Arc<dyn Engine>, usage: UsageCounter, bot_token: Option<String>, inactivity: std::time::Duration) -> Arc<Flow> {
        Arc::new(Flow { engine, usage, bot_token, inactivity, cooldowns: Mutex::new(HashMap::new()), sessions: Mutex::new(HashMap::new()) })
    }

    /// Process-wide setup: the global usage counter, DISCORD_TOKEN for the expiry edit, the 30 s inactivity time.
    pub fn from_env(engine: Arc<dyn Engine>) -> Arc<Flow> {
        let token = std::env::var("DISCORD_TOKEN").ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        Flow::new(engine, crate::usage::global().clone(), token, INACTIVITY)
    }

    /// Open sessions (tests).
    pub fn session_count(&self) -> usize {
        self.sessions.lock().len()
    }

    pub async fn handle(self: &Arc<Self>, i: Interaction) {
        if i.is_button {
            self.handle_button(i).await
        } else {
            self.execute(i).await
        }
    }

    // -------------------------------------------------------------------------------------------------------------
    // /dps <class> ...
    // -------------------------------------------------------------------------------------------------------------

    pub async fn execute(self: &Arc<Self>, i: Interaction) {
        let now = Instant::now();
        let user = i.user_id.clone().unwrap_or_default();
        let wait = {
            let mut last_run = self.cooldowns.lock();
            last_run.retain(|_, at| now.duration_since(*at) < COOLDOWN);
            match last_run.get(&user) {
                Some(at) => Some((COOLDOWN - now.duration_since(*at)).as_secs_f64().ceil() as u64),
                None => {
                    last_run.insert(user.clone(), now);
                    None
                }
            }
        };
        if let Some(wait) = wait {
            return log(i.reply(Message::text(cooldown_reply(wait)).ephemeral()).await);
        }
        let Some(cls) = i.get_subcommand().and_then(class_of) else { return };
        let name = match i.require_string("ign") {
            Ok(n) => dps_core::text::js_trim(n).to_string(),
            Err(e) => return eprintln!("{e}"),
        };
        let (mode, floor_mode) = if cls == Class::Mage {
            (
                i.get_string("set").and_then(Mode::from_js).unwrap_or(Mode::Lcm),
                i.get_string("floor").and_then(FloorMode::from_js).unwrap_or(FloorMode::Master),
            )
        } else {
            (Mode::Lcm, FloorMode::Master)
        };
        let int = |n: &str| i.get_integer(n).map(|v| v as f64);
        let blessings = blessings_with(int("power"), int("wisdom"), int("time"));
        let target = i.get_string("target").and_then(Target::from_js);
        // Replies never repeat the typed IGN or raw error text, so user input cannot reach the message.
        if !is_valid_ign(&name) {
            return log(i.reply(Message::text(INVALID_IGN).ephemeral()).await);
        }
        if !self.engine.prices_ready() {
            return log(i.reply(Message::text(PRICES_LOADING).ephemeral()).await);
        }
        log(i.defer_reply().await);
        let usage = self.usage.clone();
        tokio::spawn(async move {
            usage.record().await;
        });

        let mut s = Session::new(cls, String::new(), user.parse().unwrap_or(0));
        s.target = target;
        s.mode = mode;
        s.floor_mode = floor_mode;
        s.blessings = Some(blessings);

        let account = match self.engine.lookup_account(&name).await {
            Ok(a) => a,
            Err(e) => {
                eprintln!("{e}");
                return edit(&i, MOJANG_DOWN).await;
            }
        };
        let Some(account) = account else { return edit(&i, NO_ACCOUNT).await };
        let member = match self.engine.load_member(&account.uuid).await {
            Ok(Some(m)) => m,
            Ok(None) => return edit(&i, no_profiles_reply(&escape_markdown(&account.name))).await,
            Err(e) => {
                eprintln!("{e}");
                return edit(&i, load_error_reply(&e)).await;
            }
        };
        s.name = account.name;
        s.cata_level = Some(member.cata_level as f64);

        // The planner runs off the tokio threads inside the engine, so the bot stays connected while it works.
        let job = job_of(&s, UpgradeMode::All, "coins", Some(PLAN_STEPS));
        let result = match self.engine.run(member.clone(), job, Arc::new(AtomicBool::new(false))).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{e}");
                return edit(&i, if e == PlanError::Timeout { PLAN_TOO_LONG } else { GENERIC_FAILURE }).await;
            }
        };
        s.coins.insert(plan_key(UpgradeMode::All, false, false), Arc::new(result));
        self.prune();
        let id = i.id.clone();
        let now = Instant::now();
        s.used_at = now;
        s.interaction_at = now;
        let msg = view_message(&id, &s);
        let entry: Entry = Arc::new(Mutex::new(Live { s, member, interaction: i.clone(), timer: None, cancel: None }));
        self.sessions.lock().insert(id.clone(), entry.clone());
        if let Err(e) = i.edit_reply(msg).await {
            eprintln!("{e}");
            return edit(&i, GENERIC_FAILURE).await;
        }
        // The timer starts once results are on screen (the first plan can take a while and is not inactivity).
        self.arm(&id, &mut entry.lock());
    }

    // -------------------------------------------------------------------------------------------------------------
    // Sessions
    // -------------------------------------------------------------------------------------------------------------

    fn prune(self: &Arc<Self>) {
        let old: Vec<String> = self.sessions.lock().iter().filter(|(_, e)| e.lock().s.used_at.elapsed() > SESSION).map(|(id, _)| id.clone()).collect();
        for id in old {
            let flow = self.clone();
            tokio::spawn(async move { flow.expire(&id).await });
        }
    }

    /// (Re)start a session's inactivity timer. Each session has its own, so one user's never touches another's.
    fn arm(self: &Arc<Self>, id: &str, live: &mut Live) {
        if let Some(t) = live.timer.take() {
            t.abort();
        }
        let flow = self.clone();
        let id = id.to_string();
        let wait = self.inactivity;
        live.timer = Some(tokio::spawn(async move {
            tokio::time::sleep(wait).await;
            flow.expire(&id).await;
        }));
    }

    /// Inactive: forget the session (freeing its plans and profile data), stop a calculation it started, and edit the
    /// message to drop its buttons and add a note. The edit goes to the message itself (bot permission, no 15-minute
    /// token limit); if that fails it falls back to the latest interaction's token while still valid. A deleted
    /// message or missing permission is logged, never thrown.
    pub async fn expire(self: &Arc<Self>, id: &str) {
        let Some(entry) = self.sessions.lock().remove(id) else { return };
        let (interaction, interaction_at) = {
            let mut live = entry.lock();
            if let Some(c) = live.cancel.take() {
                c.store(true, Ordering::SeqCst);
            }
            if let Some(t) = live.timer.take() {
                // Never abort ourselves mid-edit (the timer task is the one running this).
                if tokio::task::try_id() != Some(t.id()) {
                    t.abort();
                }
            }
            (live.interaction.clone(), live.s.interaction_at)
        };
        let note = Message { content: Some(INACTIVE_NOTE.into()), clear_components: true, ..Default::default() };
        let via_bot = match &self.bot_token {
            Some(token) => interaction.edit_original_as_bot(token, note.clone()).await.map_err(|e| e.0),
            None => Err("No message to edit.".to_string()),
        };
        if let Err(first) = via_bot {
            let second = if interaction_at.elapsed() > TOKEN_EDIT { Err(first) } else { interaction.edit_reply(note).await.map_err(|e| e.0) };
            if let Err(e) = second {
                eprintln!("Could not mark a /dps message inactive ({e}).");
            }
        }
    }

    // -------------------------------------------------------------------------------------------------------------
    // Buttons: Back / Next / Coin upgrades / Profile upgrades / levels / Skeleton Master chest
    // -------------------------------------------------------------------------------------------------------------

    pub async fn handle_button(self: &Arc<Self>, i: Interaction) {
        let custom = i.custom_id.clone().unwrap_or_default();
        let mut parts = custom.split(':');
        let (_, id, action) = (parts.next(), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let entry = self.sessions.lock().get(id).cloned();
        let Some(entry) = entry else { return log(i.reply(Message::text(EXPIRED_REPLY).ephemeral()).await) };
        let user: Option<u64> = i.user_id.as_deref().and_then(|u| u.parse().ok());

        // Everything that changes the session, under one short lock.
        let changed = {
            let mut live = entry.lock();
            if user != Some(live.s.owner) {
                None
            } else {
            live.s.used_at = Instant::now();
            self.arm(id, &mut live);
            live.interaction = i.clone();
            live.s.interaction_at = Instant::now();
            let s = &mut live.s;
            match action {
                "prev" => s.page = s.page.saturating_sub(1),
                "next" => s.page = (s.page + 1).min(page_count(s) - 1),
                "coins" => {
                    s.view = View::Coins;
                    s.page = 0;
                }
                "profile" => {
                    s.view = View::Profile;
                    s.page = 0;
                }
                "levels" => {
                    s.upgrade_mode = if s.upgrade_mode == UpgradeMode::All { UpgradeMode::Max } else { UpgradeMode::All };
                    s.page = 0;
                }
                "chest" if chest_toggle(s) => {
                    s.any_chest = !s.any_chest;
                    s.page = 0;
                }
                "clearboots" if clear_boots_toggle(s) => {
                    s.no_clear_boots = !s.no_clear_boots;
                    s.page = 0;
                }
                _ => {}
            }
            let missing = match s.view {
                View::Profile => s.profile.is_none(),
                View::Coins => s.current().is_none(),
            };
            Some((missing, if missing { None } else { Some(view_message(id, &live.s)) }))
            }
        };
        let Some((missing, immediate)) = changed else { return log(i.reply(Message::text(NOT_OWNER).ephemeral()).await) };
        if !missing {
            return log(i.update(immediate.expect("view")).await);
        }

        // Profile upgrades and the max-level plan are worked out the first time they are opened.
        log(i.defer_update().await);
        let (view, upgrade_mode, job, member, cancel) = {
            let mut live = entry.lock();
            // The user is waiting on this list, so the inactivity timer pauses until it shows (it restarts below).
            // The cancel flag still stops it if the session is dropped another way.
            if let Some(t) = live.timer.take() {
                t.abort();
            }
            let cancel = Arc::new(AtomicBool::new(false));
            live.cancel = Some(cancel.clone());
            let s = &live.s;
            // The JS profile job has no upgradeMode (the menu never reads it), so it is always the same job: cache hits.
            let job = if s.view == View::Profile { job_of(s, UpgradeMode::All, "profile", None) } else { job_of(s, s.upgrade_mode, "coins", Some(PLAN_STEPS)) };
            (s.view, s.upgrade_mode, job, live.member.clone(), cancel)
        };
        let (any_chest, no_clear_boots) = (job.any_chest, job.no_clear_boots);
        let outcome = self.engine.run(member, job, cancel).await;
        let same = |flow: &Flow| flow.sessions.lock().get(id).is_some_and(|e| Arc::ptr_eq(e, &entry));
        let stored = {
            let mut live = entry.lock();
            match outcome {
                Ok(r) => match view {
                    View::Profile => match r.profile {
                        Some(p) => {
                            live.s.profile = Some(p);
                            Ok(())
                        }
                        None => Err(PlanError::Failed("no profile menu".into())),
                    },
                    View::Coins => {
                        live.s.coins.insert(plan_key(upgrade_mode, any_chest, no_clear_boots), Arc::new(r));
                        Ok(())
                    }
                },
                Err(e) => Err(e),
            }
        };
        if let Err(err) = stored {
            // Stopped because the session went inactive: the message already says so.
            if err == PlanError::Cancelled || !same(self) {
                return;
            }
            eprintln!("{err}");
            {
                let mut live = entry.lock();
                live.s.view = View::Coins;
                live.s.upgrade_mode = UpgradeMode::All;
                live.s.any_chest = false;
                live.s.no_clear_boots = false;
            }
            let what = if action == "levels" { "the max-level list" } else { "that list" };
            let lead = if err == PlanError::Timeout { format!("Working out {what} took too long and was stopped.") } else { format!("Could not work out {what}.") };
            log(i.follow_up(Message::text(format!("{lead} Showing the every-level coin list instead. Try again in a minute.")).ephemeral()).await);
        }
        // The session may have gone inactive while this ran; then its buttons stay removed.
        if !same(self) {
            return;
        }
        let msg = {
            let mut live = entry.lock();
            live.cancel = None;
            view_message(id, &live.s)
        };
        log(i.edit_reply(msg).await);
        // New results on screen: the inactivity timer starts over.
        if same(self) {
            self.arm(id, &mut entry.lock());
        }
    }
}

fn log<T>(r: crate::transport::Result<T>) {
    if let Err(e) = r {
        eprintln!("{e}");
    }
}

async fn edit(i: &Interaction, text: impl Into<String>) {
    log(i.edit_reply(Message::text(text)).await);
}

/// The reply for a failed profile load (JS `hypixelError`, or the readMember text, or the generic failure).
fn load_error_reply(e: &EngineError) -> String {
    match e {
        EngineError::HypixelKey => HYPIXEL_KEY_BAD.into(),
        EngineError::HypixelRateLimit => HYPIXEL_RATE_LIMIT.into(),
        EngineError::Hypixel(m) => hypixel_error(m).into(),
        EngineError::InventoryDisabled(m) => m.clone(),
        EngineError::MojangDown | EngineError::Other(_) => GENERIC_FAILURE.into(),
    }
}

// -----------------------------------------------------------------------------------------------------------------
// Process-wide flow, installed by main once the real engine exists.
// -----------------------------------------------------------------------------------------------------------------

static FLOW: OnceLock<Arc<Flow>> = OnceLock::new();

/// Install the engine: `/dps` and its buttons work from then on. Returns false if one was installed already.
pub fn install(engine: Arc<dyn Engine>) -> bool {
    FLOW.set(Flow::from_env(engine)).is_ok()
}

/// Entry point used by `route` in main.rs.
pub async fn handle(i: Interaction) {
    match FLOW.get() {
        Some(flow) => flow.handle(i).await,
        None => log(i.reply(Message::text("/dps is not wired yet.").ephemeral()).await),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::StubEngine;
    use crate::interactions::{now_ms, InteractionServer};
    use crate::transport::{BoxFuture, Handler, Rest};
    use crate::usage::Notifier;
    use ed25519_dalek::{Signer, SigningKey};
    use http_body_util::{BodyExt, Full};
    use hyper::body::{Bytes, Incoming};
    use hyper::{Request, Response};
    use hyper_util::rt::TokioIo;
    use serde_json::{json, Value};
    use std::convert::Infallible;
    use std::time::Duration;
    use tokio::net::TcpListener;

    // ---- harness -------------------------------------------------------------------------------------------

    type Seen = Arc<Mutex<Vec<(String, String, Option<String>, Value)>>>;

    struct NoNotify;
    impl Notifier for NoNotify {
        fn notify(&self, _text: String) -> BoxFuture<Result<(), String>> {
            Box::pin(async { Ok(()) })
        }
    }

    fn td(name: &str) -> std::path::PathBuf {
        let base = std::env::var("DPS_TESTDATA").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata").into());
        std::path::Path::new(&base).join(name)
    }

    fn usage_counter(tag: &str) -> UsageCounter {
        let file = std::env::temp_dir().join(format!("dps-flow-usage-{}-{tag}.json", std::process::id()));
        let _ = std::fs::remove_file(&file);
        UsageCounter::new(file, Arc::new(NoNotify))
    }

    /// A fake Discord API. Paths starting with a prefix in `fail` answer 404.
    async fn fake_api(fail: Arc<Mutex<Vec<String>>>) -> (String, Seen) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen: Seen = Arc::default();
        let s2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let seen = s2.clone();
                let fail = fail.clone();
                tokio::spawn(async move {
                    let svc = hyper::service::service_fn(move |req: Request<Incoming>| {
                        let seen = seen.clone();
                        let fail = fail.clone();
                        async move {
                            let method = req.method().to_string();
                            let path = req.uri().path().to_string();
                            let auth = req.headers().get("authorization").and_then(|v| v.to_str().ok()).map(str::to_string);
                            let bytes = req.into_body().collect().await.unwrap().to_bytes();
                            let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                            seen.lock().push((method, path.clone(), auth, body));
                            let failing = fail.lock().iter().any(|p| path.starts_with(p.as_str()));
                            let (status, out) = if failing { (404, json!({ "message": "Unknown Message" })) } else { (200, json!({ "id": "m1", "channel_id": "c1" })) };
                            Ok::<_, Infallible>(Response::builder().status(status).header("content-type", "application/json").body(Full::new(Bytes::from(out.to_string()))).unwrap())
                        }
                    });
                    let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(stream), svc).await;
                });
            }
        });
        (base, seen)
    }

    struct Bench {
        flow: Arc<Flow>,
        engine: Arc<StubEngine>,
        rest: Rest,
        seen: Seen,
        fail: Arc<Mutex<Vec<String>>>,
    }

    async fn bench(tag: &str, inactivity: Duration, token: Option<&str>) -> Bench {
        let fail = Arc::new(Mutex::new(Vec::new()));
        let (base, seen) = fake_api(fail.clone()).await;
        let engine = Arc::new(StubEngine::new());
        let flow = Flow::new(engine.clone(), usage_counter(tag), token.map(str::to_string), inactivity);
        Bench { flow, engine, rest: Rest::with_base(Rest::default_client(), base), seen, fail }
    }

    fn command_raw(id: &str, token: &str, user: &str, sub: &str, opts: Value) -> Value {
        json!({ "type": 2, "id": id, "application_id": "app", "token": token, "channel_id": "c1", "member": { "user": { "id": user } },
            "data": { "name": "dps", "type": 1, "options": [{ "type": 1, "name": sub, "options": opts }] } })
    }
    fn ign_opts(ign: &str) -> Value {
        json!([{ "name": "ign", "value": ign }])
    }
    fn button_raw(session: &str, action: &str, token: &str, user: &str) -> Value {
        json!({ "type": 3, "id": format!("b-{token}"), "application_id": "app", "token": token, "channel_id": "c1", "message": { "id": "m1" },
            "member": { "user": { "id": user } }, "data": { "component_type": 2, "custom_id": format!("dps:{session}:{action}") } })
    }

    impl Bench {
        /// Run one interaction to completion; returns the first response (HTTP body) or None if there was none.
        async fn run(&self, raw: Value) -> Option<Value> {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let i = Interaction::from_raw_http(&raw, tx, self.rest.clone());
            self.flow.handle(i.clone()).await;
            i.finish();
            rx.await.ok()
        }
        fn edits(&self) -> Vec<(String, String, Option<String>, Value)> {
            self.seen.lock().clone()
        }
    }

    fn reply_text(r: &Value) -> String {
        r["data"]["content"].as_str().unwrap_or_default().to_string()
    }

    async fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
        for _ in 0..400 {
            if ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for {what}");
    }

    // ---- flow checks ported from test/dps.test.js ------------------------------------------------------------

    #[tokio::test]
    async fn per_user_cooldown_of_10_seconds() {
        let b = bench("cooldown", INACTIVITY, None).await;
        let go = |user: &'static str| command_raw("1", "t", user, "archer", ign_opts("@everyone"));
        let r1 = b.run(go("u1")).await.unwrap();
        let r2 = b.run(go("u1")).await.unwrap();
        let r3 = b.run(go("u2")).await.unwrap();
        assert!(reply_text(&r1).contains("not a valid Minecraft username"));
        assert_eq!(reply_text(&r2), "Please wait 10s before using /dps again.");
        assert!(reply_text(&r3).contains("not a valid Minecraft username"));
        assert_eq!(r2["data"]["flags"], 64);
    }

    #[tokio::test]
    async fn answers_right_away_while_prices_load() {
        let b = bench("prices", INACTIVITY, None).await;
        b.engine.set_ready(false);
        let r = b.run(command_raw("1", "t", "u3", "archer", ign_opts("p3wr"))).await.unwrap();
        assert_eq!(reply_text(&r), PRICES_LOADING);
        assert_eq!(r["data"]["flags"], 64);
        assert!(b.edits().is_empty());
    }

    #[test]
    fn ign_validation() {
        for ok in ["ab", "Ben_MC", "a_b_c_d_e_f_g_h_"] {
            assert!(is_valid_ign(ok), "{ok}");
        }
        for bad in ["", "a", "@everyone", "seventeen_chars_x", "sp ace", "é_é_é", "ab\n"] {
            assert!(!is_valid_ign(bad), "{bad:?}");
        }
    }

    /// Open a Ben_MC archer session with id `sid`; returns the first edit body.
    async fn open(b: &Bench, sid: &str, user: &str) -> Value {
        let r = b.run(command_raw(sid, &format!("tok-{sid}"), user, "archer", ign_opts("Ben_MC"))).await.unwrap();
        assert_eq!(r, json!({ "type": 5 }));
        let seen = b.edits();
        let e = seen.iter().rev().find(|e| e.1 == format!("/webhooks/app/tok-{sid}/messages/@original")).expect("edit");
        e.3.clone()
    }

    #[tokio::test]
    async fn sessions_go_inactive_independently_and_clicks_reset_the_timer() {
        let b = bench("inactive", Duration::from_millis(400), Some("BOTTOK")).await;
        open(&b, "A", "1").await;
        b.fail.lock().clear();
        open(&b, "B", "2").await;
        // (the cooldown is per user, so two users can open at once)
        assert_eq!(b.flow.session_count(), 2);
        tokio::time::sleep(Duration::from_millis(250)).await;
        let r = b.run(button_raw("A", "next", "ca1", "1")).await.unwrap();
        assert_eq!(r["type"], 7);
        tokio::time::sleep(Duration::from_millis(250)).await;
        // B timed out on its own; A's click restarted A's timer.
        wait_for("B expiry", || b.flow.session_count() == 1).await;
        let bot_edits = || b.edits().into_iter().filter(|e| e.1.starts_with("/channels/")).collect::<Vec<_>>();
        wait_for("B edit", || !bot_edits().is_empty()).await;
        let e = &bot_edits()[0];
        assert_eq!((e.0.as_str(), e.2.as_deref()), ("PATCH", Some("Bot BOTTOK")));
        assert_eq!(e.3, json!({ "content": INACTIVE_NOTE, "components": [], "allowed_mentions": { "parse": [] } }));
        // A's last interaction was the button (message m1), B's was its command (message looked up: m1 too).
        assert_eq!(bot_edits().len(), 1);
        wait_for("A expiry", || b.flow.session_count() == 0).await;
        wait_for("A edit", || bot_edits().len() == 2).await;
        let r = b.run(button_raw("A", "next", "ca2", "1")).await.unwrap();
        assert_eq!(reply_text(&r), EXPIRED_REPLY);
        assert_eq!(r["data"]["flags"], 64);
    }

    #[tokio::test]
    async fn expiry_falls_back_to_the_interaction_token_and_never_throws() {
        let b = bench("fallback", Duration::from_millis(200), Some("BOTTOK")).await;
        open(&b, "A", "1").await;
        // The message is gone for the bot: the edit through the bot token fails, the interaction token is used.
        b.fail.lock().push("/channels".into());
        wait_for("expiry", || b.flow.session_count() == 0).await;
        wait_for("fallback edit", || b.edits().iter().filter(|e| e.0 == "PATCH" && e.1 == "/webhooks/app/tok-A/messages/@original").count() == 2).await;
        let last = b.edits().into_iter().last().unwrap();
        assert_eq!(last.3, json!({ "content": INACTIVE_NOTE, "components": [], "allowed_mentions": { "parse": [] } }));
        // Everything fails and there is no bot token: logged only.
        let b2 = bench("fallback2", Duration::from_millis(100), None).await;
        open(&b2, "C", "3").await;
        b2.fail.lock().push("/webhooks".into());
        wait_for("expiry 2", || b2.flow.session_count() == 0).await;
    }

    #[tokio::test]
    async fn no_bot_token_edits_through_the_interaction_token() {
        let b = bench("notoken", Duration::from_millis(150), None).await;
        open(&b, "A", "1").await;
        wait_for("expiry", || b.flow.session_count() == 0).await;
        wait_for("edit", || b.edits().len() == 2).await;
        assert_eq!(b.edits()[1].1, "/webhooks/app/tok-A/messages/@original");
        assert!(b.edits().iter().all(|e| !e.1.starts_with("/channels")));
    }

    #[tokio::test]
    async fn skeleton_master_chest_button_and_plan_switch() {
        let b = bench("chest", INACTIVITY, None).await;
        let ids = |m: &Value| -> Vec<String> {
            m["data"]["components"].as_array().unwrap().iter().flat_map(|r| r["components"].as_array().unwrap().iter().map(|c| c["custom_id"].as_str().unwrap().split(':').nth(2).unwrap().to_string())).collect()
        };
        // Archer at Catacombs 50 (Ben_MC): the chest button is there; the Mage has none.
        let first = open(&b, "c1", "1").await;
        let all: Vec<String> = first["components"].as_array().unwrap().iter().flat_map(|r| r["components"].as_array().unwrap().iter().map(|c| c["custom_id"].as_str().unwrap().to_string())).collect();
        assert!(all.iter().any(|c| c == "dps:c1:chest"));
        b.run(json!({ "type": 2, "id": "c2", "application_id": "app", "token": "tok-c2", "channel_id": "c1", "member": { "user": { "id": "2" } },
            "data": { "name": "dps", "type": 1, "options": [{ "type": 1, "name": "mage", "options": ign_opts("Ben_MC") }] } })).await;
        let seen = b.edits();
        let mage = &seen.iter().rev().find(|e| e.1.contains("tok-c2")).unwrap().3;
        assert!(!mage["components"].to_string().contains(":chest"));

        let r = b.run(button_raw("c1", "chest", "cc1", "1")).await.unwrap();
        // chest is not cached: deferred update, then the edit with the any-chest plan.
        assert_eq!(r, json!({ "type": 6 }));
        let edit = b.edits().into_iter().rev().find(|e| e.1.contains("cc1")).unwrap().3;
        assert!(edit["embeds"][0]["footer"]["text"].as_str().unwrap().contains("Any chestplate allowed"));
        assert!(edit.to_string().contains("Skeleton Master chest: off"));
        assert!(b.engine.jobs().last().unwrap().any_chest);
        // Toggling back and forth reuses the cached plans: an immediate update, no new run.
        let runs = b.engine.runs();
        let r = b.run(button_raw("c1", "chest", "cc2", "1")).await.unwrap();
        assert_eq!(r["type"], 7);
        assert!(ids(&r).contains(&"chest".to_string()));
        assert!(r["data"]["components"].to_string().contains("Skeleton Master chest: forced"));
        let r = b.run(button_raw("c1", "chest", "cc3", "1")).await.unwrap();
        assert_eq!(r["type"], 7);
        assert_eq!(b.engine.runs(), runs);
    }

    #[tokio::test]
    async fn review_profile_job_ignores_level_mode_and_ign_is_trimmed_like_js() {
        let b = bench("review1", INACTIVITY, None).await;
        // JS String.trim removes U+FEFF (but not U+0085), so this name is valid in JS.
        let r = b.run(command_raw("r1", "tok-r1", "1", "archer", ign_opts("\u{FEFF} Ben_MC\u{A0}"))).await.unwrap();
        assert_eq!(r, json!({ "type": 5 }));
        assert_eq!(b.flow.session_count(), 1);
        let r = b.run(command_raw("r2", "tok-r2", "2", "archer", ign_opts("Ben_MC\u{85}"))).await.unwrap();
        assert!(reply_text(&r).contains("not a valid Minecraft username"));
        // Max levels first, then the profile list: the JS profile job has no upgradeMode, so it is the "all" job.
        let r = b.run(button_raw("r1", "levels", "rb1", "1")).await.unwrap();
        assert_eq!(r, json!({ "type": 6 }));
        let r = b.run(button_raw("r1", "profile", "rb2", "1")).await.unwrap();
        assert_eq!(r, json!({ "type": 6 }));
        let job = b.engine.jobs().last().unwrap().clone();
        assert_eq!((job.menu.as_str(), job.upgrade_mode.as_str(), job.limit), ("profile", "all", None));
    }

    // ---- error replies ------------------------------------------------------------------------------------------

    #[tokio::test]
    async fn error_replies() {
        let b = bench("errors", INACTIVITY, None).await;
        let last_edit = |b: &Bench, tok: &str| b.edits().into_iter().rev().find(|e| e.1.contains(tok)).unwrap().3["content"].as_str().unwrap().to_string();
        let mut n = 0;
        let mut go = |ign: &'static str| {
            n += 1;
            command_raw(&n.to_string(), &format!("e{n}"), &format!("{}", 100 + n), "archer", ign_opts(ign))
        };
        b.run(go("NoSuchPlayerHere")).await;
        assert_eq!(last_edit(&b, "e1"), NO_ACCOUNT);
        b.engine.fail_account(EngineError::MojangDown);
        b.run(go("Ben_MC")).await;
        assert_eq!(last_edit(&b, "e2"), MOJANG_DOWN);
        for (k, (err, want)) in [
            (EngineError::HypixelKey, HYPIXEL_KEY_BAD),
            (EngineError::HypixelRateLimit, HYPIXEL_RATE_LIMIT),
            (EngineError::Hypixel("HTTP 502".into()), HYPIXEL_DOWN),
            (EngineError::Hypixel("HTTP 403 Forbidden".into()), HYPIXEL_KEY_BAD),
            (EngineError::InventoryDisabled("Inventory API is disabled for this profile.".into()), "Inventory API is disabled for this profile."),
            (EngineError::Other("x".into()), GENERIC_FAILURE),
        ]
        .into_iter()
        .enumerate()
        {
            b.engine.fail_member(err);
            b.run(go("Ben_MC")).await;
            assert_eq!(last_edit(&b, &format!("e{}", k + 3)), want);
        }
        b.engine.fail_plan(PlanError::Timeout);
        b.run(go("Ben_MC")).await;
        assert_eq!(last_edit(&b, "e9"), PLAN_TOO_LONG);
        b.engine.fail_plan(PlanError::Failed("boom".into()));
        b.run(go("Ben_MC")).await;
        assert_eq!(last_edit(&b, "e10"), GENERIC_FAILURE);
        assert_eq!(b.flow.session_count(), 0);
        // Every failure was deferred first.
    }

    #[tokio::test]
    async fn blessings_and_options_reach_the_job() {
        let b = bench("opts", INACTIVITY, None).await;
        let opts = json!([{ "name": "ign", "value": "  Ben_MC " }, { "name": "power", "value": 19 }, { "name": "wisdom", "value": 12 }, { "name": "time", "value": 0 }, { "name": "target", "value": "dragon" }]);
        b.run(command_raw("1", "x1", "7", "archer", opts)).await;
        let job = b.engine.jobs()[0].clone();
        assert_eq!((job.cls.as_str(), job.target.as_deref(), job.mode.as_str(), job.floor_mode.as_str()), ("arch", Some("dragon"), "lcm", "master"));
        assert_eq!((job.blessings.power, job.blessings.wisdom, job.blessings.time), (19, 12, 0));
        assert_eq!((job.upgrade_mode.as_str(), job.menu.as_str(), job.limit, job.any_chest), ("all", "coins", Some(40), false));
        // Out-of-range blessing falls back to the default; Mage options.
        let opts = json!([{ "name": "ign", "value": "Ben_MC" }, { "name": "power", "value": 3 }, { "name": "set", "value": "rcm" }, { "name": "floor", "value": "normal" }]);
        b.run(command_raw("2", "x2", "8", "mage", opts)).await;
        let job = b.engine.jobs()[1].clone();
        assert_eq!((job.cls.as_str(), job.mode.as_str(), job.floor_mode.as_str(), job.blessings.power), ("mage", "rcm", "normal", 29));
        // Berserker ignores set / floor.
        let opts = json!([{ "name": "ign", "value": "Ben_MC" }, { "name": "set", "value": "rcm" }]);
        b.run(command_raw("3", "x3", "9", "berserker", opts)).await;
        let job = b.engine.jobs()[2].clone();
        assert_eq!((job.cls.as_str(), job.mode.as_str()), ("bers", "lcm"));
    }

    #[tokio::test]
    async fn list_failures_fall_back_to_the_coin_list_with_a_follow_up() {
        let b = bench("listfail", INACTIVITY, None).await;
        open(&b, "A", "1").await;
        b.engine.fail_plan(PlanError::Timeout);
        let r = b.run(button_raw("A", "levels", "l1", "1")).await.unwrap();
        assert_eq!(r, json!({ "type": 6 }));
        let seen = b.edits();
        let post = seen.iter().find(|e| e.0 == "POST" && e.1 == "/webhooks/app/l1").unwrap();
        assert_eq!(post.3["content"], "Working out the max-level list took too long and was stopped. Showing the every-level coin list instead. Try again in a minute.");
        assert_eq!(post.3["flags"], 64);
        let edit = seen.iter().find(|e| e.0 == "PATCH" && e.1 == "/webhooks/app/l1/messages/@original").unwrap();
        assert!(edit.3["embeds"][0]["footer"]["text"].as_str().unwrap().contains("Every level"));
        b.engine.fail_plan(PlanError::Failed("x".into()));
        b.run(button_raw("A", "profile", "l2", "1")).await;
        let seen = b.edits();
        let post = seen.iter().find(|e| e.0 == "POST" && e.1 == "/webhooks/app/l2").unwrap();
        assert_eq!(post.3["content"], "Could not work out that list. Showing the every-level coin list instead. Try again in a minute.");
        let edit = seen.iter().find(|e| e.0 == "PATCH" && e.1 == "/webhooks/app/l2/messages/@original").unwrap();
        assert!(edit.3["embeds"][0]["title"].as_str().unwrap().ends_with("Coin upgrades"));
    }

    #[tokio::test]
    async fn expiry_while_a_list_is_computed_cancels_it_and_keeps_buttons_removed() {
        let b = bench("cancel", Duration::from_millis(300), Some("BOTTOK")).await;
        open(&b, "A", "1").await;
        b.engine.set_delay(Duration::from_secs(5));
        let start = Instant::now();
        let flow = b.flow.clone();
        // The timer is paused while the list is computed: nothing expires for a while...
        let rest = b.rest.clone();
        let click = tokio::spawn(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let i = Interaction::from_raw_http(&button_raw("A", "profile", "p1", "1"), tx, rest);
            flow.handle(i.clone()).await;
            i.finish();
            rx.await.ok()
        });
        tokio::time::sleep(Duration::from_millis(700)).await;
        assert_eq!(b.flow.session_count(), 1, "timer must pause while the list is computed");
        assert_eq!(b.engine.cancelled(), 0);
        // ... until the session is dropped another way (pruning / expire): the plan is cancelled.
        b.flow.expire("A").await;
        let r = click.await.unwrap().unwrap();
        assert_eq!(r, json!({ "type": 6 }));
        assert_eq!(b.engine.cancelled(), 1);
        assert!(start.elapsed() < Duration::from_secs(4));
        // No list edit after the cancel (only the note).
        let edits = b.edits();
        assert!(!edits.iter().any(|e| e.1.contains("p1")));
        assert_eq!(b.flow.session_count(), 0);
    }

    #[tokio::test]
    async fn timer_restarts_after_a_computed_list_shows() {
        let b = bench("restart", Duration::from_millis(300), Some("BOTTOK")).await;
        open(&b, "A", "1").await;
        b.engine.set_delay(Duration::from_millis(500));
        let r = b.run(button_raw("A", "profile", "p1", "1")).await.unwrap();
        assert_eq!(r, json!({ "type": 6 }));
        // The list took longer than the inactivity time and the session is still there.
        assert_eq!(b.flow.session_count(), 1);
        let edit = b.edits().into_iter().find(|e| e.1.contains("p1")).unwrap().3;
        assert!(edit["embeds"][0]["title"].as_str().unwrap().ends_with("Profile upgrades"));
        wait_for("expiry after restart", || b.flow.session_count() == 0).await;
    }

    #[tokio::test]
    async fn non_owner_and_expired_buttons_get_ephemeral_replies() {
        let b = bench("owner", INACTIVITY, None).await;
        open(&b, "A", "1").await;
        let r = b.run(button_raw("A", "next", "n1", "999")).await.unwrap();
        assert_eq!(reply_text(&r), NOT_OWNER);
        assert_eq!(r["data"]["flags"], 64);
        assert_eq!(r["type"], 4);
        let r = b.run(button_raw("nope", "next", "n2", "1")).await.unwrap();
        assert_eq!(reply_text(&r), EXPIRED_REPLY);
        assert_eq!(r["data"]["flags"], 64);
    }

    // ---- end to end through the HTTP transport ----------------------------------------------------------------

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[11u8; 32])
    }
    async fn post(url: &str, payload: &Value) -> Value {
        let body = payload.to_string();
        let ts = ((now_ms() / 1000.0) as u64).to_string();
        let sig = hex::encode(signing_key().sign(format!("{ts}{body}").as_bytes()).to_bytes());
        let res = reqwest::Client::new().post(url).header("X-Signature-Ed25519", sig).header("X-Signature-Timestamp", ts).body(body).send().await.unwrap();
        assert_eq!(res.status().as_u16(), 200);
        res.json().await.unwrap()
    }

    #[tokio::test]
    async fn end_to_end_http_flow_matches_the_golden_embeds() {
        let b = bench("e2e", Duration::from_millis(1500), Some("BOTTOK")).await;
        let flow = b.flow.clone();
        let handler: Handler = Arc::new(move |i| {
            let flow = flow.clone();
            Box::pin(async move { flow.handle(i).await })
        });
        let server = InteractionServer::new(&hex::encode(signing_key().verifying_key().to_bytes()), handler, b.rest.clone()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(server.serve(listener, std::future::pending()));

        let golden: Value = serde_json::from_str(&std::fs::read_to_string(td("embeds/Ben_MC__arch-lcm-master-all-coins.json")).unwrap()).unwrap();
        let entries = golden["entries"].as_array().unwrap();
        let sid = entries[0]["id"].as_str().unwrap().to_string();
        let page = |n: usize| entries.iter().find(|e| e["session"]["view"] == "coins" && e["session"]["page"] == n).unwrap().clone();
        let patch = |tok: &str| {
            let want = format!("/webhooks/app/{tok}/messages/@original");
            b.edits().into_iter().find(|e| e.0 == "PATCH" && e.1 == want).map(|e| e.3)
        };
        let check = |body: &Value, entry: &Value| {
            assert_eq!(body["embeds"][0], entry["embed"]);
            assert_eq!(body["components"], entry["components"]);
        };

        // /dps archer Ben_MC -> deferred -> edit with the golden embed.
        let r = post(&url, &command_raw(&sid, "tok-first", "1000", "archer", ign_opts("Ben_MC"))).await;
        assert_eq!(r, json!({ "type": 5 }));
        wait_for("first edit", || patch("tok-first").is_some()).await;
        check(&patch("tok-first").unwrap(), &page(0));
        // Cooldown for the same user.
        let r = post(&url, &command_raw("77", "tok-cd", "1000", "archer", ign_opts("Ben_MC"))).await;
        assert_eq!(r["data"]["content"], "Please wait 10s before using /dps again.");

        // next -> page 1, next, prev (immediate updates).
        let go = |action: &'static str, tok: &'static str| {
            let url = url.clone();
            let sid = sid.clone();
            async move { post(&url, &button_raw(&sid, action, tok, "1000")).await }
        };
        let r = go("next", "n1").await;
        assert_eq!(r["type"], 7);
        check(&r["data"], &page(1));
        let r = go("next", "n2").await;
        check(&r["data"], &page(2));
        let r = go("prev", "n3").await;
        check(&r["data"], &page(1));

        // Profile upgrades: computed on first open (deferred update + edit), golden profile embed.
        let profile = entries.iter().find(|e| e["session"]["view"] == "profile").unwrap().clone();
        let r = go("profile", "pr1").await;
        assert_eq!(r, json!({ "type": 6 }));
        wait_for("profile edit", || patch("pr1").is_some()).await;
        check(&patch("pr1").unwrap(), &profile);
        // Coin upgrades again: cached, page 0.
        let runs = b.engine.runs();
        let r = go("coins", "co1").await;
        assert_eq!(r["type"], 7);
        check(&r["data"], &page(0));
        // Profile again: cached.
        let r = go("profile", "pr2").await;
        assert_eq!(r["type"], 7);
        check(&r["data"], &profile);
        assert_eq!(b.engine.runs(), runs);
        let _ = go("coins", "co2").await;

        // Max levels, then the chest toggle: each worked out once.
        let r = go("levels", "lv1").await;
        assert_eq!(r, json!({ "type": 6 }));
        wait_for("levels edit", || patch("lv1").is_some()).await;
        let body = patch("lv1").unwrap();
        assert!(body["embeds"][0]["footer"]["text"].as_str().unwrap().contains("Max levels"));
        assert!(body.to_string().contains("Show every level"));
        let last = b.engine.jobs().last().unwrap().clone();
        assert_eq!((last.upgrade_mode.as_str(), last.menu.as_str(), last.any_chest), ("max", "coins", false));
        let golden_max: Value = serde_json::from_str(&std::fs::read_to_string(td("golden/Ben_MC__arch-lcm-master-max-coins.json")).unwrap()).unwrap();
        let first_label = golden_max["plan"]["steps"][0]["label"].as_str().unwrap();
        assert!(body["embeds"][0]["description"].as_str().unwrap().contains(first_label));

        let r = go("chest", "ch1").await;
        assert_eq!(r, json!({ "type": 6 }));
        wait_for("chest edit", || patch("ch1").is_some()).await;
        let last = b.engine.jobs().last().unwrap().clone();
        assert_eq!((last.upgrade_mode.as_str(), last.any_chest), ("max", true));

        // A non-owner is refused; the owner's buttons keep working.
        let r = post(&url, &button_raw(&sid, "next", "x1", "5")).await;
        assert_eq!(r["data"]["content"], NOT_OWNER);

        // Inactivity: no clicks for 1.5 s -> the message is edited with the bot token (buttons removed).
        wait_for("expiry edit", || b.edits().iter().any(|e| e.1.starts_with("/channels/c1/messages/m1"))).await;
        let e = b.edits().into_iter().find(|e| e.1.starts_with("/channels/")).unwrap();
        assert_eq!(e.2.as_deref(), Some("Bot BOTTOK"));
        assert_eq!(e.3["content"], INACTIVE_NOTE);
        assert_eq!(e.3["components"], json!([]));
        let r = go("next", "late").await;
        assert_eq!(r["data"]["content"], EXPIRED_REPLY);
    }
}
