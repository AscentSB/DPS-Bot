//! The Discord bot (port of src/index.js, src/commands, src/http, src/lib/usage.js). See rust/PORTING_NOTES.md.
//! Module owners: embed, command, session = agent "bot-view"; transport, gateway, interactions, usage, check_env and
//! this file = agent "bot-transport"; dps, engine = agent "bot-flow".

mod check_env;
mod cli;
mod command;
mod dps;
mod embed;
mod engine;
mod gateway;
mod interactions;
mod session;
mod transport;
mod usage;

use std::sync::Arc;

use transport::{Handler, Interaction, Rest};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Resolves on SIGINT or SIGTERM.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// The /dps command and its buttons. Replies "not wired yet" until `dps::install(engine)` has been called.
async fn dps_handle(i: Interaction) {
    dps::handle(i).await
}

/// `route` in src/index.js.
async fn route(i: Interaction) {
    if i.is_button && i.custom_id.as_deref().is_some_and(|c| c.starts_with("dps:")) {
        return dps_handle(i).await;
    }
    if !i.is_chat_input || i.command_name.as_deref() != Some("dps") {
        return;
    }
    dps_handle(i).await
}

fn handler() -> Handler {
    Arc::new(|i| Box::pin(route(i)))
}

/// Prices (bazaar and auction house, every 5 minutes) and the item list (hourly) load once at launch, in the
/// background; commands read the cache and say "try again" until the first load is done.
fn start_background() {
    let engine = engine::RealEngine::new();
    engine.start_background();
    dps::install(engine);
}

async fn run_bot() -> i32 {
    let _ = dotenvy::from_filename(".env");
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
    if env("HYPIXEL_API_KEY").is_none() {
        eprintln!("HYPIXEL_API_KEY is missing from .env: /dps cannot read profiles until it is set.");
    }
    let rest = Rest::new(Rest::default_client());
    let port = std::env::var("INTERACTIONS_PORT").ok().and_then(|p| p.trim().parse::<f64>().ok()).filter(|p| *p != 0.0 && p.is_finite());
    let code = if let Some(port) = port {
        let key = std::env::var("DISCORD_PUBLIC_KEY").unwrap_or_default();
        if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
            eprintln!("DISCORD_PUBLIC_KEY (64 hex characters) is needed for HTTP interactions.");
            return 1;
        }
        let server = match interactions::InteractionServer::new(&key, handler(), rest) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        };
        let listener = match interactions::listen(port as u16).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Cannot listen on port {port}: {e}");
                return 1;
            }
        };
        start_background();
        println!("HTTP interactions on port {port}");
        server.serve(listener, shutdown_signal()).await;
        0
    } else {
        let Some(token) = env("DISCORD_TOKEN") else {
            eprintln!("DISCORD_TOKEN is missing. Put DISCORD_TOKEN=your-bot-token in a file named .env (not .env.txt) in the bot folder, next to package.json.");
            return 1;
        };
        let token = token.trim().to_string();
        start_background();
        match gateway::run(token, handler(), rest, |tag| println!("Logged in as {tag}"), shutdown_signal()).await {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        }
    };
    usage::global().flush().await;
    code
}

/// Plan scoring stops getting faster past about 4 threads (candidate generation is serial), while
/// extra threads still burn CPU on scheduling and contention, so cap rayon's pool unless
/// RAYON_NUM_THREADS says otherwise.
fn init_rayon() {
    if std::env::var_os("RAYON_NUM_THREADS").is_some() {
        return;
    }
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let _ = rayon::ThreadPoolBuilder::new().num_threads(cores.min(4)).build_global();
}

fn main() {
    init_rayon();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        None => {
            let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("tokio runtime");
            rt.block_on(run_bot())
        }
        // check-env reports on the real environment, so it must not load .env first.
        Some("check-env") => check_env::run(),
        // Hidden: the training workload of the profile-guided Docker build (see ../../pgo).
        Some("pgo-train") => cli::pgo_train(&args[1..]),
        Some(cmd @ ("deploy" | "debug-profile" | "update-data" | "price-refresh")) => {
            let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("tokio runtime");
            let rest = &args[1..];
            rt.block_on(async {
                match cmd {
                    "deploy" => cli::deploy().await,
                    "debug-profile" => cli::debug_profile(rest).await,
                    "update-data" => cli::update_data(rest).await,
                    _ => cli::price_refresh().await,
                }
            })
        }
        Some(other) => {
            eprintln!("Unknown command {other:?}. Usage: dps-bot [deploy | check-env | debug-profile <ign> [cls] [mode] | update-data [dir] | price-refresh]");
            2
        }
    };
    std::process::exit(code);
}
