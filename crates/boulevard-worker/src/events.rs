//! Append-only event log. Every state change in Boulevard leaves a row here so
//! any projection (matches, reports, Plus) can be rebuilt and every decision
//! is auditable.

use worker::D1PreparedStatement;

use crate::app::ApiResult;
use crate::db::Db;

pub fn record(
    db: &Db,
    now: i64,
    kind: &str,
    actor: Option<&str>,
    subject: Option<&str>,
    payload: serde_json::Value,
) -> ApiResult<D1PreparedStatement> {
    db.stmt(
        "INSERT INTO events (ts, type, actor_id, subject_id, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
        &[
            now.into(),
            kind.into(),
            actor.map(str::to_string).into(),
            subject.map(str::to_string).into(),
            payload.to_string().into(),
        ],
    )
}

/// Convenience for the one-statement case.
pub async fn append(
    db: &Db,
    now: i64,
    kind: &str,
    actor: Option<&str>,
    subject: Option<&str>,
    payload: serde_json::Value,
) -> ApiResult<()> {
    let stmt = record(db, now, kind, actor, subject, payload)?;
    db.batch(vec![stmt]).await
}
