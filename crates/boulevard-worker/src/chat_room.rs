//! One Durable Object per match. SQLite-backed, hibernating WebSockets.
//!
//! Persist first, then broadcast. The room also mirrors "last message" onto
//! the D1 `matches` row and appends a `message.sent` event so the audit trail
//! stays complete even for messages that never touch the Worker.

use serde::{Deserialize, Serialize};
use worker::*;

use boulevard_core::validate::clean_message;

const MAX_MESSAGES_PER_MINUTE: i64 = 30;

#[durable_object]
pub struct ChatRoom {
    state: State,
    env: Env,
    sql: SqlStorage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    id: i64,
    sender_id: String,
    body: String,
    kind: String,
    created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Member {
    user_id: String,
    last_read_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Attachment {
    user_id: String,
}

#[derive(Deserialize)]
struct SeedBody {
    match_id: String,
    members: Vec<String>,
    messages: Vec<crate::chat::SeedMessage>,
}

#[derive(Deserialize)]
struct SendBody {
    sender_id: String,
    body: String,
}

#[derive(Deserialize)]
struct ReadBody {
    user_id: String,
    last_read_id: i64,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientEvent {
    Message { body: String },
    Typing,
    Read { last_read_id: i64 },
    Ping,
}

fn now_ms() -> i64 {
    Date::now().as_millis() as i64
}

fn json_response<T: Serialize>(v: &T, status: u16) -> Result<Response> {
    Ok(Response::from_json(v)?.with_status(status))
}

fn err(msg: &str, status: u16) -> Result<Response> {
    json_response(&serde_json::json!({ "error": { "code": "chat", "message": msg } }), status)
}

impl ChatRoom {
    fn members(&self) -> Result<Vec<Member>> {
        self.sql.exec("SELECT user_id, last_read_id FROM members ORDER BY user_id", None)?.to_array()
    }

    fn is_member(&self, user: &str) -> Result<bool> {
        Ok(self.members()?.iter().any(|m| m.user_id == user))
    }

    fn match_id(&self) -> Option<String> {
        self.state.id().name()
    }

    fn insert_message(&self, sender: &str, body: &str, kind: &str, at: i64) -> Result<Message> {
        self.sql
            .exec(
                "INSERT INTO messages (sender_id, body, kind, created_at) VALUES (?, ?, ?, ?) RETURNING id, sender_id, body, kind, created_at",
                vec![sender.into(), body.into(), kind.into(), SqlStorageValue::Integer(at)],
            )?
            .one()
    }

    fn recent_count(&self, sender: &str, since: i64) -> Result<i64> {
        #[derive(Deserialize)]
        struct Row {
            n: i64,
        }
        let row: Row = self
            .sql
            .exec(
                "SELECT COUNT(*) AS n FROM messages WHERE sender_id = ? AND created_at >= ?",
                vec![sender.into(), SqlStorageValue::Integer(since)],
            )?
            .one()?;
        Ok(row.n)
    }

    fn broadcast(&self, payload: &serde_json::Value, except: Option<&str>) {
        let text = payload.to_string();
        for ws in self.state.get_websockets() {
            if let Some(ex) = except {
                let who = ws.deserialize_attachment::<Attachment>().ok().flatten().map(|a| a.user_id);
                if who.as_deref() == Some(ex) {
                    continue;
                }
            }
            let _ = ws.send_with_str(&text);
        }
    }

    /// Mirror the message onto D1 (last-message columns + event). Best effort.
    async fn mirror(&self, msg: &Message) {
        let Some(match_id) = self.match_id() else { return };
        let Ok(db) = self.env.d1("DB") else { return };
        let preview = crate::swipes::preview(&msg.body);
        let digest = crate::app::sha256_hex(&msg.body);
        let stmts = vec![
            db.prepare("UPDATE matches SET last_message_at = ?1, last_message_preview = ?2, last_message_from = ?3 WHERE id = ?4")
                .bind(&[
                    worker::wasm_bindgen::JsValue::from_f64(msg.created_at as f64),
                    worker::wasm_bindgen::JsValue::from_str(&preview),
                    worker::wasm_bindgen::JsValue::from_str(&msg.sender_id),
                    worker::wasm_bindgen::JsValue::from_str(&match_id),
                ]),
            db.prepare("INSERT INTO events (ts, type, actor_id, subject_id, payload) VALUES (?1, 'message.sent', ?2, ?3, ?4)")
                .bind(&[
                    worker::wasm_bindgen::JsValue::from_f64(msg.created_at as f64),
                    worker::wasm_bindgen::JsValue::from_str(&msg.sender_id),
                    worker::wasm_bindgen::JsValue::from_str(&match_id),
                    worker::wasm_bindgen::JsValue::from_str(
                        &serde_json::json!({ "message_id": msg.id, "len": msg.body.chars().count(), "sha256": digest, "kind": msg.kind }).to_string(),
                    ),
                ]),
        ];
        let stmts: Vec<D1PreparedStatement> = stmts.into_iter().filter_map(|s| s.ok()).collect();
        if let Err(e) = db.batch(stmts).await {
            console_error!("mirror message to D1 failed: {e}");
        }
    }

    async fn post_message(&self, sender: &str, body: &str) -> Result<std::result::Result<Message, String>> {
        if !self.is_member(sender)? {
            return Ok(Err("Not a member of this chat.".into()));
        }
        let text = match clean_message(body) {
            Ok(t) => t,
            Err(e) => return Ok(Err(e.message)),
        };
        let now = now_ms();
        if self.recent_count(sender, now - 60_000)? >= MAX_MESSAGES_PER_MINUTE {
            return Ok(Err("Slow down a little.".into()));
        }
        let msg = self.insert_message(sender, &text, "text", now)?;
        self.sql.exec(
            "UPDATE members SET last_read_id = ? WHERE user_id = ?",
            vec![SqlStorageValue::Integer(msg.id), sender.into()],
        )?;
        self.broadcast(&serde_json::json!({ "type": "message", "message": msg }), None);
        self.mirror(&msg).await;
        Ok(Ok(msg))
    }
}

impl DurableObject for ChatRoom {
    fn new(state: State, env: Env) -> Self {
        let sql = state.storage().sql();
        sql.exec(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sender_id TEXT NOT NULL,
                body TEXT NOT NULL,
                kind TEXT NOT NULL DEFAULT 'text',
                created_at INTEGER NOT NULL
            )",
            None,
        )
        .expect("create messages");
        sql.exec(
            "CREATE TABLE IF NOT EXISTS members (
                user_id TEXT PRIMARY KEY,
                last_read_id INTEGER NOT NULL DEFAULT 0,
                joined_at INTEGER NOT NULL
            )",
            None,
        )
        .expect("create members");
        sql.exec("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)", None)
            .expect("create meta");
        Self { state, env, sql }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let path = req.path();
        let user = req.headers().get("x-user")?.unwrap_or_default();
        match (req.method(), path.as_str()) {
            (Method::Post, "/seed") => {
                let body: SeedBody = req.json().await?;
                let now = now_ms();
                for m in &body.members {
                    self.sql.exec(
                        "INSERT OR IGNORE INTO members (user_id, last_read_id, joined_at) VALUES (?, 0, ?)",
                        vec![m.as_str().into(), SqlStorageValue::Integer(now)],
                    )?;
                }
                #[derive(Deserialize)]
                struct Seeded {
                    n: i64,
                }
                let already: Seeded = self
                    .sql
                    .exec("SELECT COUNT(*) AS n FROM meta WHERE key = 'seeded'", None)?
                    .one()?;
                let mut inserted = Vec::new();
                if already.n == 0 {
                    for m in &body.messages {
                        inserted.push(self.insert_message(&m.sender_id, &m.body, "note", m.created_at)?);
                    }
                    self.sql.exec(
                        "INSERT INTO meta (key, value) VALUES ('seeded', ?), ('match_id', ?)",
                        vec![now.to_string().into(), body.match_id.as_str().into()],
                    )?;
                }
                json_response(&serde_json::json!({ "ok": true, "seeded": inserted.len() }), 200)
            }
            (Method::Get, "/history") => {
                if !self.is_member(&user)? {
                    return err("Not a member.", 403);
                }
                let url = req.url()?;
                let mut after: i64 = 0;
                let mut limit: i64 = 50;
                for (k, v) in url.query_pairs() {
                    match k.as_ref() {
                        "after" => after = v.parse().unwrap_or(0),
                        "limit" => limit = v.parse().unwrap_or(50),
                        _ => {}
                    }
                }
                let limit = limit.clamp(1, 200);
                let messages: Vec<Message> = self
                    .sql
                    .exec(
                        "SELECT id, sender_id, body, kind, created_at FROM messages WHERE id > ? ORDER BY id ASC LIMIT ?",
                        vec![SqlStorageValue::Integer(after), SqlStorageValue::Integer(limit)],
                    )?
                    .to_array()?;
                let members = self.members()?;
                json_response(&serde_json::json!({ "messages": messages, "members": members }), 200)
            }
            (Method::Post, "/send") => {
                let body: SendBody = req.json().await?;
                if body.sender_id != user {
                    return err("Sender mismatch.", 403);
                }
                match self.post_message(&body.sender_id, &body.body).await? {
                    Ok(msg) => json_response(&serde_json::json!({ "ok": true, "message": msg }), 201),
                    Err(m) => err(&m, 400),
                }
            }
            (Method::Post, "/read") => {
                let body: ReadBody = req.json().await?;
                if body.user_id != user || !self.is_member(&user)? {
                    return err("Not a member.", 403);
                }
                self.sql.exec(
                    "UPDATE members SET last_read_id = MAX(last_read_id, ?) WHERE user_id = ?",
                    vec![SqlStorageValue::Integer(body.last_read_id), user.as_str().into()],
                )?;
                self.broadcast(&serde_json::json!({ "type": "read", "user_id": user, "last_read_id": body.last_read_id }), Some(&user));
                json_response(&serde_json::json!({ "ok": true }), 200)
            }
            (Method::Get, "/ws") => {
                if !self.is_member(&user)? {
                    return err("Not a member.", 403);
                }
                let pair = WebSocketPair::new()?;
                let server = pair.server;
                self.state.accept_websocket_with_tags(&server, &[user.as_str()]);
                server.serialize_attachment(Attachment { user_id: user.clone() })?;
                let members = self.members()?;
                let _ = server.send_with_str(
                    serde_json::json!({ "type": "hello", "user_id": user, "members": members }).to_string(),
                );
                self.broadcast(&serde_json::json!({ "type": "presence", "user_id": user, "online": true }), Some(&user));
                Response::from_websocket(pair.client)
            }
            (Method::Get, "/transcript") => {
                let messages: Vec<Message> = self
                    .sql
                    .exec("SELECT id, sender_id, body, kind, created_at FROM messages ORDER BY id ASC", None)?
                    .to_array()?;
                let members = self.members()?;
                json_response(&serde_json::json!({ "match_id": self.match_id(), "messages": messages, "members": members }), 200)
            }
            (Method::Post, "/close") => {
                for ws in self.state.get_websockets() {
                    let _ = ws.send_with_str(serde_json::json!({ "type": "closed" }).to_string());
                    let _ = ws.close(Some(1000), Some("match closed"));
                }
                json_response(&serde_json::json!({ "ok": true }), 200)
            }
            (Method::Post, "/purge") => {
                for ws in self.state.get_websockets() {
                    let _ = ws.close(Some(1000), Some("account deleted"));
                }
                self.sql.exec("DELETE FROM messages", None)?;
                self.sql.exec("DELETE FROM members", None)?;
                self.sql.exec("DELETE FROM meta", None)?;
                json_response(&serde_json::json!({ "ok": true }), 200)
            }
            _ => err("Not found.", 404),
        }
    }

    async fn websocket_message(&self, ws: WebSocket, message: WebSocketIncomingMessage) -> Result<()> {
        let Some(att) = ws.deserialize_attachment::<Attachment>()? else {
            let _ = ws.close(Some(4001), Some("no session"));
            return Ok(());
        };
        let text = match message {
            WebSocketIncomingMessage::String(s) => s,
            WebSocketIncomingMessage::Binary(_) => {
                let _ = ws.send_with_str(serde_json::json!({ "type": "error", "message": "Text frames only." }).to_string());
                return Ok(());
            }
        };
        let event: ClientEvent = match serde_json::from_str(&text) {
            Ok(e) => e,
            Err(_) => {
                let _ = ws.send_with_str(serde_json::json!({ "type": "error", "message": "Unknown event." }).to_string());
                return Ok(());
            }
        };
        match event {
            ClientEvent::Message { body } => {
                if let Err(m) = self.post_message(&att.user_id, &body).await? {
                    let _ = ws.send_with_str(serde_json::json!({ "type": "error", "message": m }).to_string());
                }
            }
            ClientEvent::Typing => {
                self.broadcast(&serde_json::json!({ "type": "typing", "user_id": att.user_id }), Some(&att.user_id));
            }
            ClientEvent::Read { last_read_id } => {
                self.sql.exec(
                    "UPDATE members SET last_read_id = MAX(last_read_id, ?) WHERE user_id = ?",
                    vec![SqlStorageValue::Integer(last_read_id), att.user_id.as_str().into()],
                )?;
                self.broadcast(&serde_json::json!({ "type": "read", "user_id": att.user_id, "last_read_id": last_read_id }), Some(&att.user_id));
            }
            ClientEvent::Ping => {
                let _ = ws.send_with_str(serde_json::json!({ "type": "pong", "at": now_ms() }).to_string());
            }
        }
        Ok(())
    }

    async fn websocket_close(&self, ws: WebSocket, _code: usize, _reason: String, _was_clean: bool) -> Result<()> {
        if let Some(att) = ws.deserialize_attachment::<Attachment>()? {
            self.broadcast(&serde_json::json!({ "type": "presence", "user_id": att.user_id, "online": false }), Some(&att.user_id));
        }
        Ok(())
    }

    async fn websocket_error(&self, ws: WebSocket, error: Error) -> Result<()> {
        console_warn!("websocket error: {error}");
        let _ = ws.close(Some(1011), Some("error"));
        Ok(())
    }
}
