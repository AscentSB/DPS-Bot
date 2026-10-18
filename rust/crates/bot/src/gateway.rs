//! Gateway transport (default): one twilight shard with the Guilds intent. Interactions arrive over the websocket and
//! are answered through twilight-http interaction callbacks. Reconnecting and resuming is handled by the shard.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde_json::Value;
use twilight_gateway::{CloseFrame, Intents, Message, Shard, ShardId};

use crate::transport::{Handler, Interaction, Rest, TYPE_COMMAND, TYPE_COMPONENT};

/// "Logged in as name#1234" (or just the name for new-style usernames), from a READY `d.user`.
pub fn user_tag(user: &Value) -> String {
    let name = user["username"].as_str().unwrap_or("?");
    match user["discriminator"].as_str() {
        Some(d) if d != "0" && !d.is_empty() => format!("{name}#{d}"),
        _ => name.to_string(),
    }
}

/// Run until `shutdown` resolves (graceful close) or the gateway closes fatally (returns an error).
/// `on_ready` runs on every READY.
pub async fn run(
    token: String,
    handler: Handler,
    rest: Rest,
    on_ready: impl Fn(&str) + Send + 'static,
    shutdown: impl std::future::Future<Output = ()>,
) -> Result<(), String> {
    let http = Arc::new(twilight_http::Client::new(token.clone()));
    let mut shard = Shard::new(ShardId::ONE, token, Intents::GUILDS);
    tokio::pin!(shutdown);
    let mut closing = false;
    loop {
        let item = tokio::select! {
            item = shard.next() => item,
            _ = &mut shutdown, if !closing => {
                closing = true;
                shard.close(CloseFrame::NORMAL);
                continue;
            }
            _ = tokio::time::sleep(Duration::from_secs(5)), if closing => return Ok(()),
        };
        let Some(item) = item else {
            return if closing { Ok(()) } else { Err("The gateway connection closed and cannot be resumed (invalid token or intents?).".into()) };
        };
        let text = match item {
            Ok(Message::Text(t)) => t,
            Ok(Message::Close(_)) => continue,
            Err(e) => {
                eprintln!("Gateway error: {e}");
                // The shard reconnects on the next poll; never spin if the error repeats.
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue;
            }
        };
        if closing {
            continue;
        }
        let ready = text.contains("\"READY\"");
        let create = text.contains("\"INTERACTION_CREATE\"");
        if !ready && !create {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        match v["t"].as_str() {
            Some("READY") => on_ready(&user_tag(&v["d"]["user"])),
            Some("INTERACTION_CREATE") => {
                let kind = v["d"]["type"].as_u64().unwrap_or(0);
                if kind != TYPE_COMMAND && kind != TYPE_COMPONENT {
                    continue;
                }
                let interaction = Interaction::from_raw_gateway(&v["d"], http.clone(), rest.clone());
                let h = handler.clone();
                tokio::spawn(async move {
                    let task = tokio::spawn(h(interaction));
                    if let Err(e) = task.await {
                        eprintln!("Interaction handler failed: {e}");
                    }
                });
            }
            _ => {}
        }
    }
}
