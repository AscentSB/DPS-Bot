//! Interaction abstraction shared by the gateway and the HTTP-interactions server (port of `adaptInteraction` in
//! src/http/interactions.js). Handlers get an [`Interaction`] and call `reply` / `defer_reply` / `update` /
//! `defer_update` / `follow_up` / `edit_reply` on it, whichever transport it arrived on.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Map, Value};
use tokio::sync::oneshot;

pub const API: &str = "https://discord.com/api/v10";
const EPHEMERAL: u64 = 1 << 6;

pub const TYPE_PING: u64 = 1;
pub const TYPE_COMMAND: u64 = 2;
pub const TYPE_COMPONENT: u64 = 3;
pub const CALLBACK_PONG: u64 = 1;
pub const CALLBACK_MESSAGE: u64 = 4;
pub const CALLBACK_DEFERRED_MESSAGE: u64 = 5;
pub const CALLBACK_DEFERRED_UPDATE: u64 = 6;
pub const CALLBACK_UPDATE: u64 = 7;

#[derive(Debug, Clone)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
fn err<T>(s: impl Into<String>) -> Result<T> {
    Err(Error(s.into()))
}

pub type BoxFuture<T = ()> = Pin<Box<dyn Future<Output = T> + Send>>;
/// Called once per command or button interaction.
pub type Handler = Arc<dyn Fn(Interaction) -> BoxFuture + Send + Sync>;

/// A message payload. `embeds` and `components` are plain Discord JSON, so the view code decides their shape.
#[derive(Debug, Clone, Default)]
pub struct Message {
    pub content: Option<String>,
    pub embeds: Vec<Value>,
    pub components: Vec<Value>,
    pub ephemeral: bool,
    /// Send `components: []` (removes the buttons of a message being edited); `components` must be empty.
    pub clear_components: bool,
}

impl Message {
    pub fn text(content: impl Into<String>) -> Self {
        Message { content: Some(content.into()), ..Default::default() }
    }
    pub fn ephemeral(mut self) -> Self {
        self.ephemeral = true;
        self
    }
    /// API body: the ephemeral flag as `flags: 64`, and nothing can ping anyone.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(c) = &self.content {
            m.insert("content".into(), Value::String(c.clone()));
        }
        if !self.embeds.is_empty() {
            m.insert("embeds".into(), Value::Array(self.embeds.clone()));
        }
        if !self.components.is_empty() {
            m.insert("components".into(), Value::Array(self.components.clone()));
        } else if self.clear_components {
            m.insert("components".into(), Value::Array(Vec::new()));
        }
        if self.ephemeral {
            m.insert("flags".into(), json!(EPHEMERAL));
        }
        m.insert("allowed_mentions".into(), json!({ "parse": [] }));
        Value::Object(m)
    }
}
impl From<&str> for Message {
    fn from(s: &str) -> Self {
        Message::text(s)
    }
}
impl From<String> for Message {
    fn from(s: String) -> Self {
        Message::text(s)
    }
}

/// Webhook / bot-token REST calls shared by both transports. `base` is overridable for tests.
#[derive(Clone)]
pub struct Rest {
    pub client: reqwest::Client,
    pub base: String,
}

impl Rest {
    pub fn new(client: reqwest::Client) -> Self {
        Rest { client, base: API.to_string() }
    }
    pub fn with_base(client: reqwest::Client, base: impl Into<String>) -> Self {
        Rest { client, base: base.into() }
    }
    pub fn default_client() -> reqwest::Client {
        reqwest::Client::builder().timeout(Duration::from_secs(30)).build().expect("reqwest client")
    }
}

enum Backend {
    /// The first response is the HTTP body; taken once.
    Http(Mutex<Option<oneshot::Sender<Value>>>),
    Gateway(Arc<twilight_http::Client>),
}

/// One command or button interaction.
#[derive(Clone)]
pub struct Interaction {
    pub id: String,
    pub application_id: String,
    pub token: String,
    pub user_id: Option<String>,
    pub user_name: Option<String>,
    pub command_name: Option<String>,
    pub subcommand: Option<String>,
    pub custom_id: Option<String>,
    pub channel_id: Option<String>,
    /// The message a button sits on.
    pub message_id: Option<String>,
    pub is_button: bool,
    pub is_chat_input: bool,
    options: Arc<Vec<(String, Value)>>,
    backend: Arc<Backend>,
    rest: Rest,
}

fn str_of(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

impl Interaction {
    /// Build from a raw interaction object (HTTP transport; the gateway path converts to the same fields).
    /// `respond` receives the first response.
    pub fn from_raw_http(raw: &Value, respond: oneshot::Sender<Value>, rest: Rest) -> Interaction {
        Self::from_raw(raw, Backend::Http(Mutex::new(Some(respond))), rest)
    }

    pub fn from_raw_gateway(raw: &Value, http: Arc<twilight_http::Client>, rest: Rest) -> Interaction {
        Self::from_raw(raw, Backend::Gateway(http), rest)
    }

    fn from_raw(raw: &Value, backend: Backend, rest: Rest) -> Interaction {
        let data = &raw["data"];
        let top_options = data["options"].as_array();
        let sub = top_options.and_then(|o| o.iter().find(|o| o["type"].as_u64() == Some(1)));
        let opts: Vec<(String, Value)> = sub
            .and_then(|s| s["options"].as_array())
            .or(top_options)
            .map(|a| {
                a.iter()
                    .filter_map(|o| Some((o["name"].as_str()?.to_string(), o["value"].clone())))
                    .collect()
            })
            .unwrap_or_default();
        let user = if raw["member"]["user"].is_object() { &raw["member"]["user"] } else { &raw["user"] };
        let kind = raw["type"].as_u64().unwrap_or(0);
        Interaction {
            id: str_of(&raw["id"]).unwrap_or_default(),
            application_id: str_of(&raw["application_id"]).unwrap_or_default(),
            token: str_of(&raw["token"]).unwrap_or_default(),
            user_id: str_of(&user["id"]),
            user_name: str_of(&user["username"]),
            command_name: str_of(&data["name"]),
            subcommand: sub.and_then(|s| str_of(&s["name"])),
            custom_id: str_of(&data["custom_id"]),
            channel_id: str_of(&raw["channel_id"]).or_else(|| str_of(&raw["channel"]["id"])),
            message_id: str_of(&raw["message"]["id"]),
            is_button: kind == TYPE_COMPONENT && data["component_type"].as_u64() == Some(2),
            is_chat_input: kind == TYPE_COMMAND && data["type"].as_u64().unwrap_or(1) == 1,
            options: Arc::new(opts),
            backend: Arc::new(backend),
            rest,
        }
    }

    pub fn get_subcommand(&self) -> Option<&str> {
        self.subcommand.as_deref()
    }

    /// `options.getString(name)`: a string option, or None.
    pub fn get_string(&self, name: &str) -> Option<&str> {
        self.options.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.as_str())
    }

    /// `options.getString(name, true)`.
    pub fn require_string(&self, name: &str) -> Result<&str> {
        match self.get_string(name) {
            Some(s) => Ok(s),
            None => err(format!("Missing option {name}")),
        }
    }

    pub fn get_integer(&self, name: &str) -> Option<i64> {
        self.options.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.as_i64())
    }

    async fn respond(&self, body: Value) -> Result<()> {
        match &*self.backend {
            // A second response on the HTTP transport is ignored, as in the JS (headers already sent).
            Backend::Http(slot) => {
                if let Some(tx) = slot.lock().take() {
                    let _ = tx.send(body);
                }
                Ok(())
            }
            Backend::Gateway(http) => {
                let id: u64 = self.id.parse().map_err(|_| Error("bad interaction id".into()))?;
                use twilight_http::request::Request;
                use twilight_http::routing::Route;
                let req = Request::builder(&Route::InteractionCallback { interaction_id: id, interaction_token: &self.token })
                    .json(&body)
                    .use_authorization_token(false)
                    .build()
                    .map_err(|e| Error(format!("interaction callback: {e}")))?;
                http.request::<twilight_http::response::marker::EmptyBody>(req)
                    .await
                    .map_err(|e| Error(format!("interaction callback failed: {e}")))?;
                Ok(())
            }
        }
    }

    /// True while the HTTP transport has not sent its response yet.
    pub fn http_pending(&self) -> bool {
        matches!(&*self.backend, Backend::Http(slot) if slot.lock().is_some())
    }

    /// Drop the pending HTTP responder; the server then answers 500 "no response". No-op on the gateway.
    pub fn finish(&self) {
        if let Backend::Http(slot) = &*self.backend {
            slot.lock().take();
        }
    }

    pub async fn reply(&self, msg: impl Into<Message>) -> Result<()> {
        self.respond(json!({ "type": CALLBACK_MESSAGE, "data": msg.into().to_json() })).await
    }
    pub async fn defer_reply(&self) -> Result<()> {
        self.respond(json!({ "type": CALLBACK_DEFERRED_MESSAGE })).await
    }
    pub async fn update(&self, msg: impl Into<Message>) -> Result<()> {
        self.respond(json!({ "type": CALLBACK_UPDATE, "data": msg.into().to_json() })).await
    }
    pub async fn defer_update(&self) -> Result<()> {
        self.respond(json!({ "type": CALLBACK_DEFERRED_UPDATE })).await
    }

    fn webhook(&self) -> String {
        format!("{}/webhooks/{}/{}", self.rest.base, self.application_id, self.token)
    }

    pub async fn follow_up(&self, msg: impl Into<Message>) -> Result<()> {
        let res = self.rest.client.post(self.webhook()).json(&msg.into().to_json()).send().await.map_err(|e| Error(format!("Followup failed: {e}")))?;
        if !res.status().is_success() {
            return err(format!("Followup failed: HTTP {}", res.status().as_u16()));
        }
        Ok(())
    }

    pub async fn edit_reply(&self, msg: impl Into<Message>) -> Result<()> {
        let url = format!("{}/messages/@original", self.webhook());
        let res = self.rest.client.patch(url).json(&msg.into().to_json()).send().await.map_err(|e| Error(format!("Edit original response failed: {e}")))?;
        if !res.status().is_success() {
            return err(format!("Edit original response failed: HTTP {}", res.status().as_u16()));
        }
        Ok(())
    }

    /// Edit the original response with the bot token. The interaction webhook dies after 15 minutes; a message edit
    /// with the bot token keeps working, which is how buttons are removed when a session expires.
    pub async fn edit_original_as_bot(&self, bot_token: &str, msg: impl Into<Message>) -> Result<()> {
        let (channel, message) = match (&self.channel_id, &self.message_id) {
            (Some(c), Some(m)) => (c.clone(), m.clone()),
            _ => {
                // A command's original response: look its ids up while the webhook still answers.
                let url = format!("{}/messages/@original", self.webhook());
                let res = self.rest.client.get(url).send().await.map_err(|e| Error(format!("Fetch original response failed: {e}")))?;
                if !res.status().is_success() {
                    return err(format!("Fetch original response failed: HTTP {}", res.status().as_u16()));
                }
                let v: Value = res.json().await.map_err(|e| Error(format!("Fetch original response failed: {e}")))?;
                match (str_of(&v["channel_id"]), str_of(&v["id"])) {
                    (Some(c), Some(m)) => (c, m),
                    _ => return err("Original response has no ids"),
                }
            }
        };
        edit_message_as_bot(&self.rest, bot_token, &channel, &message, &msg.into()).await
    }
}

/// `PATCH /channels/{channel}/messages/{message}` as the bot.
pub async fn edit_message_as_bot(rest: &Rest, bot_token: &str, channel_id: &str, message_id: &str, msg: &Message) -> Result<()> {
    let url = format!("{}/channels/{channel_id}/messages/{message_id}", rest.base);
    let res = rest
        .client
        .patch(url)
        .header("Authorization", format!("Bot {bot_token}"))
        .json(&msg.to_json())
        .send()
        .await
        .map_err(|e| Error(format!("Edit message failed: {e}")))?;
    if !res.status().is_success() {
        return err(format!("Edit message failed: HTTP {}", res.status().as_u16()));
    }
    Ok(())
}
