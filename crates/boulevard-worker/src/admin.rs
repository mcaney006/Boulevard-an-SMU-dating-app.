//! Moderation. Gated by `ADMIN_EMAILS` or `users.is_admin`. Every action is an event.

use serde::Deserialize;
use worker::*;

use crate::app::{body, query, ApiError, ApiResult, App};
use crate::auth::require_admin;
use crate::db::{load_match, load_profile, load_user, MatchRow, UserRow};
use crate::deck::peer_card;
use crate::events;

#[derive(Deserialize, serde::Serialize)]
struct ReportRow {
    id: String,
    reporter_id: String,
    target_id: String,
    match_id: Option<String>,
    reason: String,
    details: Option<String>,
    status: String,
    created_at: i64,
    resolved_at: Option<i64>,
    resolved_by: Option<String>,
    resolution: Option<String>,
    resolution_note: Option<String>,
}

pub async fn list_reports(req: &Request, app: &App) -> ApiResult {
    require_admin(req, app).await?;
    let db = app.db()?;
    let q = query(req);
    let status = q.get("status").cloned().unwrap_or_else(|| "open".into());
    let rows: Vec<ReportRow> = db
        .all(
            "SELECT * FROM reports WHERE status = ?1 ORDER BY created_at DESC LIMIT 200",
            &[status.as_str().into()],
        )
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let target = peer_card(&db, &r.target_id).await?;
        let reporter = peer_card(&db, &r.reporter_id).await?;
        let mut val = serde_json::to_value(&r)?;
        val["target"] = target.unwrap_or(serde_json::Value::Null);
        val["reporter"] = reporter.unwrap_or(serde_json::Value::Null);
        out.push(val);
    }
    app.json(serde_json::json!({ "reports": out }))
}

#[derive(Deserialize)]
struct ResolveBody {
    action: String,
    note: Option<String>,
}

pub async fn resolve_report(req: &mut Request, app: &App, id: &str) -> ApiResult {
    let admin = require_admin(req, app).await?;
    let input: ResolveBody = body(req).await?;
    let action = input.action.as_str();
    if !["dismiss", "warn", "suspend", "ban"].contains(&action) {
        return Err(ApiError::bad("action must be dismiss, warn, suspend, or ban"));
    }
    let db = app.db()?;
    let now = app.now();
    let report: Option<ReportRow> = db.first("SELECT * FROM reports WHERE id = ?1", &[id.into()]).await?;
    let Some(report) = report else { return Err(ApiError::NotFound) };
    let note = input.note.map(|n| n.trim().chars().take(2000).collect::<String>());
    let target = report.target_id.clone();

    let mut batch = vec![db.stmt(
        "UPDATE reports SET status = 'resolved', resolved_at = ?1, resolved_by = ?2, resolution = ?3, resolution_note = ?4 WHERE id = ?5",
        &[now.into(), admin.id().into(), action.into(), note.clone().into(), id.into()],
    )?];
    let mut closed: Vec<MatchRow> = vec![];
    match action {
        "suspend" | "ban" => {
            let status = if action == "ban" { "banned" } else { "suspended" };
            let reason = note.clone().unwrap_or_else(|| {
                if action == "ban" { "This account is closed.".into() } else { "This account is suspended.".into() }
            });
            batch.push(db.stmt(
                "UPDATE users SET status = ?1, status_reason = ?2 WHERE id = ?3",
                &[status.into(), reason.as_str().into(), target.as_str().into()],
            )?);
            batch.push(db.stmt(
                "UPDATE sessions SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL",
                &[now.into(), target.as_str().into()],
            )?);
            if action == "ban" {
                closed = db
                    .all(
                        "SELECT * FROM matches WHERE (user_a = ?1 OR user_b = ?1) AND closed_at IS NULL",
                        &[target.as_str().into()],
                    )
                    .await?;
                batch.push(db.stmt(
                    "UPDATE matches SET closed_at = ?1, closed_reason = 'moderation', closed_by = ?2 WHERE (user_a = ?3 OR user_b = ?3) AND closed_at IS NULL",
                    &[now.into(), admin.id().into(), target.as_str().into()],
                )?);
                for m in &closed {
                    batch.push(events::record(&db, now, "match.closed", Some(admin.id()), Some(&m.id), serde_json::json!({ "reason": "moderation" }))?);
                }
            }
        }
        _ => {}
    }
    batch.push(events::record(
        &db,
        now,
        &format!("moderation.{action}"),
        Some(admin.id()),
        Some(&target),
        serde_json::json!({ "report_id": id, "note": note }),
    )?);
    db.batch(batch).await?;
    for m in &closed {
        let _ = crate::chat::close_room(app, &m.id).await;
    }
    app.json(serde_json::json!({ "ok": true, "action": action, "matches_closed": closed.len() }))
}

pub async fn get_user(req: &Request, app: &App, id: &str) -> ApiResult {
    require_admin(req, app).await?;
    let db = app.db()?;
    let user: UserRow = load_user(&db, id).await?.ok_or(ApiError::NotFound)?;
    let profile = load_profile(&db, id).await?.and_then(|p| p.to_profile());
    #[derive(Deserialize)]
    struct Count {
        n: i64,
    }
    let reports_against: Option<Count> = db.first("SELECT COUNT(*) AS n FROM reports WHERE target_id = ?1", &[id.into()]).await?;
    let reports_filed: Option<Count> = db.first("SELECT COUNT(*) AS n FROM reports WHERE reporter_id = ?1", &[id.into()]).await?;
    let matches: Option<Count> = db.first("SELECT COUNT(*) AS n FROM matches WHERE user_a = ?1 OR user_b = ?1", &[id.into()]).await?;
    #[derive(Deserialize, serde::Serialize)]
    struct Ev {
        id: i64,
        ts: i64,
        #[serde(rename = "type")]
        kind: String,
        actor_id: Option<String>,
        subject_id: Option<String>,
        payload: String,
    }
    let events: Vec<Ev> = db
        .all(
            "SELECT id, ts, type, actor_id, subject_id, payload FROM events WHERE actor_id = ?1 OR subject_id = ?1 ORDER BY id DESC LIMIT 50",
            &[id.into()],
        )
        .await?;
    app.json(serde_json::json!({
        "user": user,
        "profile": profile,
        "counts": {
            "reports_against": reports_against.map(|c| c.n).unwrap_or(0),
            "reports_filed": reports_filed.map(|c| c.n).unwrap_or(0),
            "matches": matches.map(|c| c.n).unwrap_or(0),
        },
        "events": events,
    }))
}

pub async fn transcript(req: &Request, app: &App, match_id: &str) -> ApiResult {
    let admin = require_admin(req, app).await?;
    let db = app.db()?;
    let m = load_match(&db, match_id).await?.ok_or(ApiError::NotFound)?;
    events::append(&db, app.now(), "moderation.transcript_viewed", Some(admin.id()), Some(&m.id), serde_json::json!({})).await?;
    crate::chat::transcript(app, match_id).await
}
