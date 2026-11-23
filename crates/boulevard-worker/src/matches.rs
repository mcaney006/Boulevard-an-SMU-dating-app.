//! Open matches and unmatching.

use worker::*;

use crate::app::{ApiError, ApiResult, App};
use crate::auth::require;
use crate::db::{load_match, MatchRow};
use crate::deck::peer_card;
use crate::events;

pub async fn list(req: &Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    let db = app.db()?;
    let me = cur.id();
    let rows: Vec<MatchRow> = db
        .all(
            "SELECT * FROM matches WHERE (user_a = ?1 OR user_b = ?1) AND closed_at IS NULL
             ORDER BY COALESCE(last_message_at, created_at) DESC LIMIT 200",
            &[me.into()],
        )
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for m in &rows {
        let other = m.other(me);
        let Some(card) = peer_card(&db, other).await? else { continue };
        let last_message = m.last_message_at.map(|at| {
            serde_json::json!({
                "at": at,
                "preview": m.last_message_preview,
                "from_me": m.last_message_from.as_deref() == Some(me),
            })
        });
        out.push(serde_json::json!({
            "id": m.id,
            "created_at": m.created_at,
            "other": card,
            "last_message": last_message,
        }));
    }
    app.json(serde_json::json!({ "matches": out }))
}

pub async fn unmatch(req: &Request, app: &App, id: &str) -> ApiResult {
    let cur = require(req, app).await?;
    let db = app.db()?;
    let m = load_match(&db, id).await?.ok_or(ApiError::NotFound)?;
    if !m.has(cur.id()) {
        return Err(ApiError::NotFound);
    }
    if m.closed_at.is_some() {
        return app.json(serde_json::json!({ "ok": true, "already": true }));
    }
    let now = app.now();
    db.batch(vec![
        db.stmt(
            "UPDATE matches SET closed_at = ?1, closed_reason = 'unmatched', closed_by = ?2 WHERE id = ?3 AND closed_at IS NULL",
            &[now.into(), cur.id().into(), id.into()],
        )?,
        events::record(&db, now, "match.closed", Some(cur.id()), Some(id), serde_json::json!({ "reason": "unmatched" }))?,
    ])
    .await?;
    let _ = crate::chat::close_room(app, id).await;
    app.json(serde_json::json!({ "ok": true }))
}

