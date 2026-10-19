//! Command line tools: `deploy`, `debug-profile`, `update-data`, `price-refresh` (ports of src/deploy-commands.js,
//! scripts/debug-profile.mjs, scripts/update-data.mjs; the last one only keeps the price refresh running).

use std::sync::Arc;

use dps_core::jsfmt::js_string;
use dps_core::model::{Blessings, DpsJob};
use dps_skyblock::hypixel::{active_members, read_member, Hypixel};
use dps_skyblock::member::Json;
use dps_skyblock::mojang::{is_valid_ign, Mojang};
use dps_skyblock::prices::PriceService;

/// `JSON.stringify` of a parsed profile (keys in the order the parser kept them).
fn write_json(v: &Json, out: &mut String) {
    match v {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Num(x) => out.push_str(&if x.is_finite() { js_string(*x) } else { "null".to_string() }),
        Json::Str(s) => out.push_str(&serde_json::to_string(&**s).unwrap_or_default()),
        Json::Arr(a) => {
            out.push('[');
            for (i, e) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(e, out);
            }
            out.push(']');
        }
        Json::Obj(o) => {
            out.push('{');
            for (i, (k, e)) in o.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(&**k).unwrap_or_default());
                out.push(':');
                write_json(e, out);
            }
            out.push('}');
        }
    }
}

fn shown(v: Option<&Json>, none: &str) -> String {
    match v {
        None | Some(Json::Null) => none.to_string(),
        Some(v) => v.to_js_string(),
    }
}

/// Hidden `pgo-train <train.json>`: replays the packed members, jobs and prices of pgo/train.json.gz (gunzipped)
/// through `run_dps`, like /dps does. The Docker build runs it in a `-Cprofile-generate` binary to collect the
/// profile for the final `-Cprofile-use` build; it needs no network, token or other secret.
pub fn pgo_train(args: &[String]) -> i32 {
    use dps_core::model::{Member, PriceTables, Resource};
    let Some(path) = args.first() else {
        eprintln!("Usage: dps-bot pgo-train <train.json>");
        return 2;
    };
    let v: serde_json::Value = match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string())) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Cannot read {path}: {e}");
            return 1;
        }
    };
    let (mut resource, mut prices) = (Resource::default(), PriceTables::default());
    for p in v["items"].as_array().into_iter().flatten() {
        if let (Some(k), Ok(item)) = (p[0].as_str(), serde_json::from_value(p[1].clone())) {
            resource.insert(k.to_string(), item);
        }
    }
    for p in v["bazaar"].as_array().into_iter().flatten() {
        prices.bazaar.insert(p[0].as_str().unwrap_or_default().into(), p[1].as_f64().unwrap_or_default());
    }
    for p in v["auctions"].as_array().into_iter().flatten() {
        prices.auctions.insert(p[0].as_str().unwrap_or_default().into(), p[1].as_f64().unwrap_or_default());
    }
    let members: Vec<Member> = serde_json::from_value(v["members"].clone()).unwrap_or_default();
    let (resource, prices) = (Arc::new(resource), Arc::new(prices));
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut runs = 0;
    for c in v["cases"].as_array().into_iter().flatten() {
        let (Ok(job), Some(member)) = (serde_json::from_value::<DpsJob>(c["job"].clone()), c["member"].as_u64().and_then(|i| members.get(i as usize))) else {
            continue;
        };
        let _ = dps_calc::run_dps(member, &job, &resource, &prices, &cancel);
        runs += 1;
    }
    eprintln!("pgo-train: {runs} runs");
    0
}

pub async fn debug_profile(args: &[String]) -> i32 {
    let _ = dotenvy::from_filename(".env");
    let ign = args.first().map(String::as_str).unwrap_or("");
    let cls = args.get(1).map(String::as_str).unwrap_or("mage");
    let mode = args.get(2).map(String::as_str).unwrap_or("lcm");
    if !is_valid_ign(ign) {
        println!("Usage: npm run debug-profile -- <ign> [mage|arch|bers] [lcm|rcm]");
        return 1;
    }
    let mut stage = "Mojang lookup".to_string();
    if let Err(e) = debug_profile_inner(ign, cls, mode, &mut stage).await {
        println!("\nFAILED during {stage}:");
        println!("{e}");
    }
    0
}

async fn debug_profile_inner(ign: &str, cls: &str, mode: &str, stage: &mut String) -> Result<(), String> {
    let client = dps_skyblock::http::new_client();
    let mojang = Mojang::new(client.clone());
    let hypixel = Hypixel::new(client);
    let account = mojang.get_minecraft_profile(ign).await?.ok_or("No Minecraft account with that name.")?;
    *stage = "Hypixel profile and item list".into();
    let (resource, profile, collections) = tokio::join!(hypixel.items_resource(), hypixel.get_selected_profile(&account.uuid), hypixel.collections_resource());
    let (resource, profile) = (resource?, profile?);
    let collections = collections.ok();
    let profile = profile.ok_or("No SkyBlock profiles.")?;
    let file = format!("debug-{}.json", account.name);
    let mut text = format!("{{\"uuid\":{},\"profile\":", serde_json::to_string(&account.uuid).unwrap());
    write_json(&profile, &mut text);
    text.push('}');
    std::fs::write(&file, text).map_err(|e| e.to_string())?;
    println!("Saved the raw profile to {file}");
    *stage = "reading the profile (readMember)".into();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0);
    let member = read_member(&profile, &account.uuid, &resource, collections.as_deref(), now)?;
    let bank = member.pet_profile.bank.map(js_string).unwrap_or_else(|| "not shared (bank API off)".into());
    println!("Read {} items, Catacombs {}, bank {}.", member.items.len(), member.cata_level, bank);
    let me = profile.path(&["members", &account.uuid]);
    let members = profile.get("members").map(|m| m.entries().count()).unwrap_or(0);
    println!(
        "Bank: shared {}, personal {}, purse {}, members {} ({} still on the profile). Golden Dragon uses {}.",
        shown(profile.path(&["banking", "balance"]), "hidden"),
        shown(me.and_then(|m| m.path(&["profile", "bank_account"])), "hidden"),
        shown(me.and_then(|m| m.path(&["currencies", "coin_purse"])), "hidden"),
        members,
        active_members(&profile),
        member.pet_profile.bank.map(js_string).unwrap_or_else(|| "none (bank API off)".into()),
    );
    println!("Profile: {} (selected: {})", shown(profile.get("cute_name"), "?"), profile.get("selected").is_some_and(Json::truthy));
    let pets: Vec<String> = member.pets.iter().map(|p| format!("{} {}{}", p.kind, p.tier, p.held_item.as_deref().filter(|h| !h.is_empty()).map(|h| format!(" + {h}")).unwrap_or_default())).collect();
    println!("Pets read ({}): {}", member.pets.len(), if pets.is_empty() { "none".to_string() } else { pets.join(", ") });
    *stage = "loading prices".into();
    let prices = PriceService::new();
    let _handles = prices.start();
    prices.wait_ready().await;
    *stage = format!("planning ({cls} {mode})");
    let job = DpsJob {
        cls: cls.into(),
        target: None,
        mode: mode.into(),
        floor_mode: "master".into(),
        upgrade_mode: "all".into(),
        menu: "coins".into(),
        limit: Some(40),
        blessings: Blessings { power: 29, wisdom: 14, time: 5 },
        any_chest: false,
        no_clear_boots: false,
    };
    let snap = prices.snapshot();
    let member = Arc::new(member);
    let r = tokio::task::spawn_blocking(move || dps_calc::run_dps(&member, &job, &resource, &snap, &std::sync::atomic::AtomicBool::new(false)))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let steps = r.plan.as_ref().map(|p| p.steps.len()).unwrap_or(0);
    let first = r.plan.as_ref().and_then(|p| p.steps.first()).map(|s| s.label.clone()).unwrap_or_else(|| "-".into());
    println!("OK: {steps} steps. First: {first}");
    Ok(())
}

/// Registers /dps (src/deploy-commands.js).
pub async fn deploy() -> i32 {
    let _ = dotenvy::from_filename(".env");
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
    let (Some(token), Some(client_id)) = (env("DISCORD_TOKEN"), env("CLIENT_ID")) else {
        eprintln!("DISCORD_TOKEN and CLIENT_ID are needed in .env to register the command.");
        return 1;
    };
    let guild = env("GUILD_ID");
    let url = match &guild {
        Some(g) => format!("https://discord.com/api/v10/applications/{client_id}/guilds/{g}/commands"),
        None => format!("https://discord.com/api/v10/applications/{client_id}/commands"),
    };
    let body = serde_json::Value::Array(vec![crate::command::registration()]);
    let client = dps_skyblock::http::new_client();
    match client.put(&url).header("Authorization", format!("Bot {}", token.trim())).json(&body).send().await {
        Ok(r) if r.status().is_success() => {
            println!("Registered /dps {}.", guild.map(|g| format!("in guild {g}")).unwrap_or_else(|| "globally".into()));
            0
        }
        Ok(r) => {
            let status = r.status();
            eprintln!("Discord answered {status}: {}", r.text().await.unwrap_or_default());
            1
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// `update-data [dir]`: rewrites the NEU-derived JSON files (default `src/data`).
pub async fn update_data(args: &[String]) -> i32 {
    let dir = args.first().cloned().unwrap_or_else(|| "src/data".into());
    match dps_skyblock::update_data::update_data(&dir).await {
        Ok(s) => {
            println!("{s}");
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// Runs only the price refresh loops (no Discord), printing a line per source update. For idle-CPU measurements.
pub async fn price_refresh() -> i32 {
    let prices = PriceService::new();
    let _handles = prices.start();
    prices.wait_ready().await;
    println!("Prices loaded (version {}).", prices.version());
    let _ = tokio::signal::ctrl_c().await;
    0
}
