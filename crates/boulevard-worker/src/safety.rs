//! Blocks and reports. Both are cheap to file and recorded as events; a block
//! also closes any open match in the same batch.

use serde::Deserialize;
use worker::*;

use crate::app::{body, new_id, ApiError, ApiResult, App};
use crate::auth::require;
use crate::db::{load_user, pair, V};
use crate::events;

#[derive(Deserialize)]
struct BlockBody {
    user_id: String,
}

#[derive(Deserialize)]
struct OpenMatch {
    id: String,
}

pub async fn block(req: &mut Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    let input: BlockBody = body(req).await?;
    let target = input.user_id.trim().to_string();
    if target.is_empty() || target == cur.id() {
        return Err(ApiError::bad("Pick someone else."));
    }
    let db = app.db()?;
    let now = app.now();
    if !boulevard_core::standins::is_standin_id(&target) && load_user(&db, &target).await?.is_none() {
        return Err(ApiError::NotFound);
    }
    let (a, b) = pair(cur.id(), &target);
    let open: Option<OpenMatch> = db
        .first(
            "SELECT id FROM matches WHERE user_a = ?1 AND user_b = ?2 AND closed_at IS NULL",
            &[a.as_str().into(), b.as_str().into()],
        )
        .await?;
    let mut batch = vec![
        db.stmt(
            "INSERT OR IGNORE INTO blocks (blocker_id, blocked_id, created_at) VALUES (?1, ?2, ?3)",
            &[cur.id().into(), target.as_str().into(), now.into()],
        )?,
        events::record(&db, now, "user.blocked", Some(cur.id()), Some(&target), serde_json::json!({}))?,
    ];
    if let Some(m) = &open {
        batch.push(db.stmt(
            "UPDATE matches SET closed_at = ?1, closed_reason = 'blocked', closed_by = ?2 WHERE id = ?3 AND closed_at IS NULL",
            &[now.into(), cur.id().into(), m.id.as_str().into()],
        )?);
        batch.push(events::record(&db, now, "match.closed", Some(cur.id()), Some(&m.id), serde_json::json!({ "reason": "blocked" }))?);
    }
    db.batch(batch).await?;
    if let Some(m) = &open {
        let _ = crate::chat::close_room(app, &m.id).await;
    }
    app.json(serde_json::json!({ "ok": true, "closed_match": open.map(|m| m.id) }))
}

pub async fn unblock(req: &Request, app: &App, target: &str) -> ApiResult {
    let cur = require(req, app).await?;
    let db = app.db()?;
    let now = app.now();
    db.batch(vec![
        db.stmt("DELETE FROM blocks WHERE blocker_id = ?1 AND blocked_id = ?2", &[cur.id().into(), target.into()])?,
        events::record(&db, now, "user.unblocked", Some(cur.id()), Some(target), serde_json::json!({}))?,
    ])
    .await?;
    app.json(serde_json::json!({ "ok": true }))
}

pub const REPORT_REASONS: &[&str] = &["fake_profile", "harassment", "inappropriate_content", "underage", "spam", "other"];

#[derive(Deserialize)]
struct ReportBody {
    target_id: String,
    reason: String,
    details: Option<String>,
    match_id: Option<String>,
    #[serde(default)]
    block: bool,
}

pub async fn report(req: &mut Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    let input: ReportBody = body(req).await?;
    let target = input.target_id.trim().to_string();
    if target.is_empty() || target == cur.id() {
        return Err(ApiError::bad("Pick someone else."));
    }
    if !REPORT_REASONS.contains(&input.reason.as_str()) {
        return Err(ApiError::bad(format!("reason must be one of: {}", REPORT_REASONS.join(", "))));
    }
    let details = input.details.map(|d| d.trim().chars().take(2000).collect::<String>()).filter(|d| !d.is_empty());
    let db = app.db()?;
    let now = app.now();
    let (ok, retry) = db.rate_limit(&format!("report:{}", cur.id()), 24 * 60 * 60 * 1000, 10, now).await?;
    if !ok {
        return Err(ApiError::RateLimited { message: "You have filed a lot of reports today. Email us if it is urgent.".into(), retry_after_s: retry });
    }
    let id = new_id("rpt")?;
    db.batch(vec![
        db.stmt(
            "INSERT INTO reports (id, reporter_id, target_id, match_id, reason, details, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7)",
            &[
                id.as_str().into(),
                cur.id().into(),
                target.as_str().into(),
                input.match_id.clone().into(),
                input.reason.as_str().into(),
                details.clone().into(),
                now.into(),
            ],
        )?,
        events::record(
            &db,
            now,
            "report.filed",
            Some(cur.id()),
            Some(&target),
            serde_json::json!({ "report_id": id, "reason": input.reason, "match_id": input.match_id }),
        )?,
    ])
    .await?;
    let mut response = serde_json::json!({ "ok": true, "report_id": id });
    if input.block {
        // Reuse the block path so the match closes the same way.
        let mut fake = Request::new_with_init(
            "https://boulevard.internal/v1/blocks",
            &RequestInit {
                method: Method::Post,
                headers: req.headers().clone(),
                body: Some(worker::wasm_bindgen::JsValue::from_str(&serde_json::json!({ "user_id": target }).to_string())),
                ..Default::default()
            },
        )?;
        let mut blocked = block(&mut fake, app).await?;
        response["blocked"] = blocked.json::<serde_json::Value>().await.unwrap_or(serde_json::json!({ "ok": true }));
    }
    let _ = V::N;
    app.json(response)
}
