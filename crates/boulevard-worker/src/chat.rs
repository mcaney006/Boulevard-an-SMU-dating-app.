//! Worker-side chat: verify membership in D1, then hand off to the match's
//! `ChatRoom` Durable Object. The DO owns messages, presence, and typing.

use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;
use worker::*;

use boulevard_core::validate::clean_message;

use crate::app::{body, query, ApiError, ApiResult, App};
use crate::auth::require;
use crate::db::{load_match, MatchRow};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedMessage {
    pub sender_id: String,
    pub body: String,
    pub created_at: i64,
}

fn stub(app: &App, match_id: &str) -> ApiResult<Stub> {
    Ok(app.env.durable_object("CHAT_ROOM")?.get_by_name(match_id)?)
}

async fn call(app: &App, match_id: &str, method: Method, path: &str, user: Option<&str>, json: Option<String>) -> ApiResult<Response> {
    let headers = Headers::new();
    headers.set("content-type", "application/json")?;
    if let Some(u) = user {
        headers.set("x-user", u)?;
    }
    let init = RequestInit {
        method,
        headers,
        body: json.map(|j| JsValue::from_str(&j)),
        ..Default::default()
    };
    let req = Request::new_with_init(&format!("https://chat.internal{path}"), &init)?;
    Ok(stub(app, match_id)?.fetch_with_request(req).await?)
}

async fn membership(app: &App, cur_id: &str, match_id: &str) -> ApiResult<MatchRow> {
    let db = app.db()?;
    let m = load_match(&db, match_id).await?.ok_or(ApiError::NotFound)?;
    if !m.has(cur_id) {
        return Err(ApiError::NotFound);
    }
    if m.closed_at.is_some() {
        return Err(ApiError::Custom { status: 410, code: "match_closed", message: "This match is closed.".into() });
    }
    Ok(m)
}

pub async fn history(req: &Request, app: &App, match_id: &str) -> ApiResult {
    let cur = require(req, app).await?;
    membership(app, cur.id(), match_id).await?;
    let q = query(req);
    let after = q.get("after").cloned().unwrap_or_else(|| "0".into());
    let limit = q.get("limit").cloned().unwrap_or_else(|| "50".into());
    call(app, match_id, Method::Get, &format!("/history?after={after}&limit={limit}"), Some(cur.id()), None).await
}

#[derive(Deserialize)]
struct SendBody {
    body: String,
}

pub async fn send(req: &mut Request, app: &App, match_id: &str) -> ApiResult {
    let cur = require(req, app).await?;
    membership(app, cur.id(), match_id).await?;
    let input: SendBody = body(req).await?;
    let text = clean_message(&input.body).map_err(|e| ApiError::Fields(vec![e]))?;
    let payload = serde_json::json!({ "sender_id": cur.id(), "body": text }).to_string();
    call(app, match_id, Method::Post, "/send", Some(cur.id()), Some(payload)).await
}

#[derive(Deserialize)]
struct ReadBody {
    last_read_id: i64,
}

pub async fn mark_read(req: &mut Request, app: &App, match_id: &str) -> ApiResult {
    let cur = require(req, app).await?;
    membership(app, cur.id(), match_id).await?;
    let input: ReadBody = body(req).await?;
    let payload = serde_json::json!({ "user_id": cur.id(), "last_read_id": input.last_read_id }).to_string();
    call(app, match_id, Method::Post, "/read", Some(cur.id()), Some(payload)).await
}

/// Upgrade to a WebSocket on the match's room. Native clients send the
/// bearer header; browsers may use `?token=`.
pub async fn websocket(req: &Request, app: &App, match_id: &str) -> ApiResult {
    let upgrade = req.headers().get("upgrade")?.unwrap_or_default();
    if !upgrade.eq_ignore_ascii_case("websocket") {
        return Err(ApiError::bad("Expected a WebSocket upgrade."));
    }
    let cur = require(req, app).await?;
    membership(app, cur.id(), match_id).await?;
    let headers = Headers::new();
    headers.set("upgrade", "websocket")?;
    headers.set("x-user", cur.id())?;
    let init = RequestInit { method: Method::Get, headers, ..Default::default() };
    let forwarded = Request::new_with_init("https://chat.internal/ws", &init)?;
    Ok(stub(app, match_id)?.fetch_with_request(forwarded).await?)
}

pub async fn seed_room(app: &App, match_id: &str, members: &[String], messages: &[SeedMessage]) -> ApiResult<()> {
    let payload = serde_json::json!({ "match_id": match_id, "members": members, "messages": messages }).to_string();
    let mut res = call(app, match_id, Method::Post, "/seed", None, Some(payload)).await?;
    if res.status_code() >= 300 {
        return Err(ApiError::Internal(format!("seed failed: {}", res.text().await.unwrap_or_default())));
    }
    Ok(())
}

pub async fn close_room(app: &App, match_id: &str) -> ApiResult<()> {
    call(app, match_id, Method::Post, "/close", None, None).await.map(|_| ())
}

pub async fn purge_room(app: &App, match_id: &str) -> ApiResult<()> {
    call(app, match_id, Method::Post, "/purge", None, None).await.map(|_| ())
}

pub async fn transcript(app: &App, match_id: &str) -> ApiResult<Response> {
    call(app, match_id, Method::Get, "/transcript", None, None).await
}
