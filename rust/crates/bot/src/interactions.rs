//! HTTP interactions server (port of src/http/interactions.js): Discord POSTs each interaction here when the app has
//! an Interactions Endpoint URL. Every request is checked against the app's public key (Ed25519 over timestamp +
//! body, 401 if invalid), PINGs get a PONG, commands and buttons go to the handler. The first reply / defer / update
//! is the HTTP response (due within 3 seconds); everything after goes through the interaction webhook.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::header::CONTENT_TYPE;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::transport::{Handler, Interaction, Rest, CALLBACK_PONG, TYPE_PING};

pub const MAX_BODY: usize = 1_000_000;
/// Requests older than this are refused, so a captured request cannot be replayed later.
pub const MAX_AGE_S: f64 = 5.0 * 60.0;

pub fn public_key_from_hex(hex_key: &str) -> Result<VerifyingKey, String> {
    let bytes = hex::decode(hex_key.trim()).map_err(|e| format!("bad public key: {e}"))?;
    let arr: [u8; 32] = bytes.try_into().map_err(|_| "bad public key: need 32 bytes".to_string())?;
    VerifyingKey::from_bytes(&arr).map_err(|e| format!("bad public key: {e}"))
}

pub fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

/// `verifySignature`: 128 hex chars of signature, 1-12 digit timestamp within 5 minutes of `now_ms`, Ed25519 over
/// timestamp + body.
pub fn verify_signature(key: &VerifyingKey, signature: Option<&str>, timestamp: Option<&str>, body: &[u8], now_ms: f64) -> bool {
    let (Some(sig), Some(ts)) = (signature, timestamp) else { return false };
    if sig.len() != 128 || !sig.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    if ts.is_empty() || ts.len() > 12 || !ts.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let ts_n: f64 = ts.parse().unwrap_or(f64::NAN);
    if (now_ms / 1000.0 - ts_n).abs() > MAX_AGE_S || ts_n.is_nan() {
        return false;
    }
    let Ok(sig_bytes) = hex::decode(sig) else { return false };
    let Ok(sig_arr) = <[u8; 64]>::try_from(sig_bytes) else { return false };
    let signature = Signature::from_bytes(&sig_arr);
    let mut msg = Vec::with_capacity(ts.len() + body.len());
    msg.extend_from_slice(ts.as_bytes());
    msg.extend_from_slice(body);
    key.verify(&msg, &signature).is_ok()
}

fn json_response(status: u16, body: Option<Value>) -> Response<Full<Bytes>> {
    let bytes = match body {
        Some(v) => Bytes::from(serde_json::to_vec(&v).unwrap_or_default()),
        None => Bytes::new(),
    };
    Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header(CONTENT_TYPE, "application/json")
        .body(Full::new(bytes))
        .expect("response")
}

fn error_response(status: u16, msg: &str) -> Response<Full<Bytes>> {
    json_response(status, Some(json!({ "error": msg })))
}

pub struct InteractionServer {
    key: VerifyingKey,
    handler: Handler,
    rest: Rest,
}

impl InteractionServer {
    pub fn new(public_key_hex: &str, handler: Handler, rest: Rest) -> Result<Arc<Self>, String> {
        Ok(Arc::new(InteractionServer { key: public_key_from_hex(public_key_hex)?, handler, rest }))
    }

    /// Handle one request (also used directly by tests).
    pub async fn handle(self: &Arc<Self>, req: Request<Incoming>) -> Response<Full<Bytes>> {
        if req.method() != Method::POST {
            return error_response(405, "method not allowed");
        }
        let sig = req.headers().get("x-signature-ed25519").and_then(|v| v.to_str().ok()).map(str::to_string);
        let ts = req.headers().get("x-signature-timestamp").and_then(|v| v.to_str().ok()).map(str::to_string);
        let too_big = req.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<usize>().ok()).is_some_and(|n| n > MAX_BODY);
        if too_big {
            return error_response(413, "too large");
        }
        let body = match Limited::new(req.into_body(), MAX_BODY).collect().await {
            Ok(b) => b.to_bytes(),
            Err(_) => return error_response(413, "too large"),
        };
        if !verify_signature(&self.key, sig.as_deref(), ts.as_deref(), &body, now_ms()) {
            return error_response(401, "invalid request signature");
        }
        let Ok(raw) = serde_json::from_slice::<Value>(&body) else { return error_response(400, "bad json") };
        if raw["type"].as_u64() == Some(TYPE_PING) {
            return json_response(200, Some(json!({ "type": CALLBACK_PONG })));
        }
        let (tx, rx) = oneshot::channel();
        let interaction = Interaction::from_raw_http(&raw, tx, self.rest.clone());
        let handler = self.handler.clone();
        let i2 = interaction.clone();
        tokio::spawn(async move {
            if let Err(e) = tokio::spawn(handler(i2.clone())).await {
                eprintln!("Interaction handler failed: {e}");
            }
            // A handler that never answered still has to close the request.
            i2.finish();
        });
        drop(interaction);
        match rx.await {
            Ok(v) => json_response(200, Some(v)),
            Err(_) => error_response(500, "no response"),
        }
    }

    /// Accept connections until `shutdown` resolves.
    pub async fn serve(self: Arc<Self>, listener: TcpListener, shutdown: impl std::future::Future<Output = ()>) {
        tokio::pin!(shutdown);
        loop {
            let (stream, _) = tokio::select! {
                r = listener.accept() => match r {
                    Ok(x) => x,
                    Err(e) => { eprintln!("accept failed: {e}"); tokio::time::sleep(std::time::Duration::from_millis(100)).await; continue; }
                },
                _ = &mut shutdown => return,
            };
            let this = self.clone();
            tokio::spawn(async move {
                let svc = hyper::service::service_fn(move |req| {
                    let this = this.clone();
                    async move { Ok::<_, Infallible>(this.handle(req).await) }
                });
                let conn = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(stream), svc);
                let _ = conn.await;
            });
        }
    }
}

pub async fn listen(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Message;
    use ed25519_dalek::{Signer, SigningKey};
    use parking_lot::Mutex;

    type Seen = Arc<Mutex<Vec<(String, String, Option<String>, String)>>>;

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }
    fn public_hex() -> String {
        hex::encode(signing_key().verifying_key().to_bytes())
    }
    fn now_s() -> u64 {
        (now_ms() / 1000.0) as u64
    }
    fn sign(ts: &str, body: &str) -> String {
        hex::encode(signing_key().sign(format!("{ts}{body}").as_bytes()).to_bytes())
    }

    /// A fake Discord API: records (method, path, authorization, body) and answers 200 with a message-ish object.
    async fn fake_api() -> (String, Seen) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen: Seen = Arc::default();
        let s2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let seen = s2.clone();
                tokio::spawn(async move {
                    let svc = hyper::service::service_fn(move |req: Request<Incoming>| {
                        let seen = seen.clone();
                        async move {
                            let method = req.method().to_string();
                            let path = req.uri().path().to_string();
                            let auth = req.headers().get("authorization").and_then(|v| v.to_str().ok()).map(str::to_string);
                            let body = String::from_utf8(req.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
                            seen.lock().push((method, path, auth, body));
                            Ok::<_, Infallible>(json_response(200, Some(json!({ "id": "m1", "channel_id": "c1" }))))
                        }
                    });
                    let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(stream), svc).await;
                });
            }
        });
        (base, seen)
    }

    async fn start(handler: Handler, api_base: &str) -> String {
        let rest = Rest::with_base(Rest::default_client(), api_base);
        let server = InteractionServer::new(&public_hex(), handler, rest).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(server.serve(listener, std::future::pending()));
        url
    }

    async fn post(url: &str, payload: &Value, signed: bool, ts: Option<String>) -> (u16, Value) {
        let body = payload.to_string();
        let ts = ts.unwrap_or_else(|| now_s().to_string());
        let sig = if signed { sign(&ts, &body) } else { "00".repeat(64) };
        let res = reqwest::Client::new().post(url).header("X-Signature-Ed25519", sig).header("X-Signature-Timestamp", ts).body(body).send().await.unwrap();
        let status = res.status().as_u16();
        (status, res.json().await.unwrap_or(Value::Null))
    }

    #[test]
    fn signature_checks() {
        let key = signing_key().verifying_key();
        let body = b"{\"type\":1}";
        let ts = "1700000000";
        let sig = sign(ts, "{\"type\":1}");
        let now = 1_700_000_000_000.0;
        assert!(verify_signature(&key, Some(&sig), Some(ts), body, now));
        assert!(verify_signature(&key, Some(&sig.to_uppercase()), Some(ts), body, now));
        assert!(!verify_signature(&key, Some(&sig), Some(ts), b"{\"type\":2}", now));
        assert!(!verify_signature(&key, Some(&sig), Some(ts), body, now + 301_000.0));
        assert!(verify_signature(&key, Some(&sig), Some(ts), body, now + 299_000.0));
        assert!(!verify_signature(&key, Some("abcd"), Some(ts), body, now));
        assert!(!verify_signature(&key, None, Some(ts), body, now));
        assert!(!verify_signature(&key, Some(&sig), Some("17000000x0"), body, now));
        assert!(!verify_signature(&key, Some(&sig), Some("1700000000000000"), body, now));
        // A different key does not verify.
        let other = SigningKey::from_bytes(&[8u8; 32]).verifying_key();
        assert!(!verify_signature(&other, Some(&sig), Some(ts), body, now));
        assert!(public_key_from_hex("zz").is_err());
        assert!(public_key_from_hex(&public_hex()).is_ok());
    }

    #[tokio::test]
    async fn ping_gets_pong_and_bad_requests_are_refused() {
        let url = start(Arc::new(|_| Box::pin(async {})), "http://127.0.0.1:1").await;
        let (s, v) = post(&url, &json!({ "type": 1 }), true, None).await;
        assert_eq!((s, v), (200, json!({ "type": 1 })));
        assert_eq!(post(&url, &json!({ "type": 1 }), false, None).await.0, 401);
        assert_eq!(post(&url, &json!({ "type": 1 }), true, Some((now_s() - 3600).to_string())).await.0, 401);
        let get = reqwest::Client::new().get(&url).send().await.unwrap();
        assert_eq!(get.status().as_u16(), 405);
        // Signed but not JSON.
        let ts = now_s().to_string();
        let res = reqwest::Client::new().post(&url).header("X-Signature-Ed25519", sign(&ts, "nope")).header("X-Signature-Timestamp", &ts).body("nope").send().await.unwrap();
        assert_eq!(res.status().as_u16(), 400);
        // Over 1 MB.
        let big = "x".repeat(MAX_BODY + 1);
        let res = reqwest::Client::new().post(&url).header("X-Signature-Ed25519", sign(&ts, &big)).header("X-Signature-Timestamp", &ts).body(big).send().await.unwrap();
        assert_eq!(res.status().as_u16(), 413);
    }

    #[tokio::test]
    async fn a_handler_that_never_answers_gets_500() {
        let url = start(Arc::new(|_| Box::pin(async {})), "http://127.0.0.1:1").await;
        let (s, v) = post(&url, &json!({ "type": 2, "id": "i", "application_id": "a", "token": "t", "data": { "name": "dps" } }), true, None).await;
        assert_eq!((s, v), (500, json!({ "error": "no response" })));
    }

    #[tokio::test]
    async fn commands_read_options_defer_then_edit_the_original_without_pings() {
        let (api, seen) = fake_api().await;
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
        let done_tx = Arc::new(Mutex::new(Some(done_tx)));
        let handler: Handler = Arc::new(move |i| {
            let done = done_tx.clone();
            Box::pin(async move {
                assert!(i.is_chat_input && !i.is_button);
                assert_eq!(i.get_subcommand(), Some("mage"));
                assert_eq!(i.require_string("ign").unwrap(), "p3wr");
                assert!(i.require_string("nope").is_err());
                assert_eq!(i.get_integer("limit"), Some(5));
                assert_eq!(i.user_id.as_deref(), Some("u1"));
                assert_eq!(i.command_name.as_deref(), Some("dps"));
                i.defer_reply().await.unwrap();
                i.edit_reply("done @everyone").await.unwrap();
                i.follow_up(Message::text("more").ephemeral()).await.unwrap();
                i.edit_original_as_bot("TOK", Message::text("bye")).await.unwrap();
                done.lock().take().unwrap().send(()).unwrap();
            })
        });
        let url = start(handler, &api).await;
        let payload = json!({ "type": 2, "id": "i1", "application_id": "app", "token": "tok", "member": { "user": { "id": "u1" } },
            "data": { "name": "dps", "type": 1, "options": [{ "type": 1, "name": "mage", "options": [{ "name": "ign", "value": "p3wr" }, { "name": "limit", "value": 5 }] }] } });
        let (s, v) = post(&url, &payload, true, None).await;
        assert_eq!((s, v), (200, json!({ "type": 5 })));
        done_rx.await.unwrap();
        let seen = seen.lock();
        assert_eq!(seen[0].0, "PATCH");
        assert_eq!(seen[0].1, "/webhooks/app/tok/messages/@original");
        assert_eq!(serde_json::from_str::<Value>(&seen[0].3).unwrap(), json!({ "content": "done @everyone", "allowed_mentions": { "parse": [] } }));
        assert_eq!((seen[1].0.as_str(), seen[1].1.as_str()), ("POST", "/webhooks/app/tok"));
        assert_eq!(serde_json::from_str::<Value>(&seen[1].3).unwrap(), json!({ "content": "more", "flags": 64, "allowed_mentions": { "parse": [] } }));
        assert_eq!((seen[2].0.as_str(), seen[2].1.as_str()), ("GET", "/webhooks/app/tok/messages/@original"));
        assert_eq!((seen[3].0.as_str(), seen[3].1.as_str()), ("PATCH", "/channels/c1/messages/m1"));
        assert_eq!(seen[3].2.as_deref(), Some("Bot TOK"));
    }

    #[tokio::test]
    async fn buttons_are_recognized_and_ephemeral_replies_set_the_flag() {
        let handler: Handler = Arc::new(|i| {
            Box::pin(async move {
                assert!(i.is_button && !i.is_chat_input);
                assert_eq!(i.custom_id.as_deref(), Some("dps:1:next"));
                assert_eq!(i.user_id.as_deref(), Some("u2"));
                i.reply(Message::text("only you").ephemeral()).await.unwrap();
                // A second response is ignored, as in the JS.
                i.reply("again").await.unwrap();
            })
        });
        let url = start(handler, "http://127.0.0.1:1").await;
        let payload = json!({ "type": 3, "id": "i2", "application_id": "app", "token": "t", "user": { "id": "u2" }, "data": { "component_type": 2, "custom_id": "dps:1:next" } });
        let (s, v) = post(&url, &payload, true, None).await;
        assert_eq!(s, 200);
        assert_eq!(v, json!({ "type": 4, "data": { "content": "only you", "flags": 64, "allowed_mentions": { "parse": [] } } }));
    }

    #[tokio::test]
    async fn update_and_defer_update_shapes() {
        let handler: Handler = Arc::new(|i| {
            Box::pin(async move {
                if i.custom_id.as_deref() == Some("a") {
                    i.defer_update().await.unwrap();
                } else {
                    let mut m = Message::text("x");
                    m.embeds.push(json!({ "title": "t" }));
                    m.components.push(json!({ "type": 1, "components": [] }));
                    i.update(m).await.unwrap();
                }
            })
        });
        let url = start(handler, "http://127.0.0.1:1").await;
        let mk = |c: &str| json!({ "type": 3, "id": "i", "application_id": "app", "token": "t", "user": { "id": "u" }, "data": { "component_type": 2, "custom_id": c } });
        assert_eq!(post(&url, &mk("a"), true, None).await.1, json!({ "type": 6 }));
        assert_eq!(
            post(&url, &mk("b"), true, None).await.1,
            json!({ "type": 7, "data": { "content": "x", "embeds": [{ "title": "t" }], "components": [{ "type": 1, "components": [] }], "allowed_mentions": { "parse": [] } } })
        );
    }
}
