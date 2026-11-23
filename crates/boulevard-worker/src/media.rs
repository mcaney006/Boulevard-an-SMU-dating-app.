//! Profile photos in R2. If the `PHOTOS` binding is absent (R2 not enabled on
//! the account yet) every photo endpoint answers 503 `photos_disabled` and the
//! rest of the product keeps working.

use serde::Deserialize;
use worker::*;

use crate::app::{new_id, ApiError, ApiResult, App};
use crate::auth::require;
use crate::db::Db;
use crate::events;

const MAX_PHOTOS: i64 = 6;
const MAX_BYTES: usize = 5 * 1024 * 1024;

fn bucket(app: &App) -> ApiResult<Bucket> {
    app.env.bucket("PHOTOS").map_err(|_| ApiError::Custom {
        status: 503,
        code: "photos_disabled",
        message: "Photos are not enabled on this deployment yet.".into(),
    })
}

#[derive(Deserialize)]
struct PhotoRow {
    r2_key: String,
    content_type: String,
}

pub async fn photo_ids(db: &Db, user_id: &str) -> ApiResult<Vec<String>> {
    #[derive(Deserialize)]
    struct Row {
        id: String,
    }
    Ok(db
        .all::<Row>("SELECT id FROM photos WHERE user_id = ?1 ORDER BY position", &[user_id.into()])
        .await?
        .into_iter()
        .map(|r| r.id)
        .collect())
}

pub async fn list_mine(req: &Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    let db = app.db()?;
    let ids = photo_ids(&db, cur.id()).await?;
    let photos: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| serde_json::json!({ "id": id, "url": format!("/v1/photos/{id}") }))
        .collect();
    app.json(serde_json::json!({ "photos": photos, "max": MAX_PHOTOS, "enabled": app.env.bucket("PHOTOS").is_ok() }))
}

fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

pub async fn upload(req: &mut Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    let bucket = bucket(app)?;
    let db = app.db()?;
    let now = app.now();
    #[derive(Deserialize)]
    struct Count {
        n: i64,
    }
    let count: Option<Count> = db.first("SELECT COUNT(*) AS n FROM photos WHERE user_id = ?1", &[cur.id().into()]).await?;
    let position = count.map(|c| c.n).unwrap_or(0);
    if position >= MAX_PHOTOS {
        return Err(ApiError::bad("You already have six photos. Remove one first."));
    }
    let bytes = req.bytes().await?;
    if bytes.is_empty() {
        return Err(ApiError::bad("Send the image bytes as the request body."));
    }
    if bytes.len() > MAX_BYTES {
        return Err(ApiError::bad("Photos must be under 5 MB."));
    }
    let content_type = sniff(&bytes).ok_or_else(|| ApiError::bad("Use a JPEG, PNG, or WebP image."))?;
    let id = new_id("pho")?;
    let key = format!("photos/{}/{}", cur.id(), id);
    bucket
        .put(&key, bytes.clone())
        .http_metadata(HttpMetadata { content_type: Some(content_type.to_string()), ..Default::default() })
        .execute()
        .await?;
    db.batch(vec![
        db.stmt(
            "INSERT INTO photos (id, user_id, position, r2_key, content_type, bytes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            &[
                id.as_str().into(),
                cur.id().into(),
                position.into(),
                key.as_str().into(),
                content_type.into(),
                (bytes.len() as i64).into(),
                now.into(),
            ],
        )?,
        events::record(&db, now, "photo.added", Some(cur.id()), Some(cur.id()), serde_json::json!({ "photo_id": id, "bytes": bytes.len() }))?,
    ])
    .await?;
    app.json_status(serde_json::json!({ "id": id, "url": format!("/v1/photos/{id}"), "position": position }), 201)
}

pub async fn delete(req: &Request, app: &App, id: &str) -> ApiResult {
    let cur = require(req, app).await?;
    let db = app.db()?;
    let row: Option<PhotoRow> = db
        .first("SELECT id, user_id, r2_key, content_type FROM photos WHERE id = ?1 AND user_id = ?2", &[id.into(), cur.id().into()])
        .await?;
    let Some(row) = row else { return Err(ApiError::NotFound) };
    if let Ok(b) = app.env.bucket("PHOTOS") {
        let _ = b.delete(&row.r2_key).await;
    }
    let now = app.now();
    db.batch(vec![
        db.stmt("DELETE FROM photos WHERE id = ?1", &[id.into()])?,
        events::record(&db, now, "photo.removed", Some(cur.id()), Some(cur.id()), serde_json::json!({ "photo_id": id }))?,
    ])
    .await?;
    app.json(serde_json::json!({ "ok": true }))
}

pub async fn serve(req: &Request, app: &App, id: &str) -> ApiResult {
    require(req, app).await?;
    let bucket = bucket(app)?;
    let db = app.db()?;
    let row: Option<PhotoRow> = db
        .first("SELECT id, user_id, r2_key, content_type FROM photos WHERE id = ?1", &[id.into()])
        .await?;
    let Some(row) = row else { return Err(ApiError::NotFound) };
    let Some(obj) = bucket.get(&row.r2_key).execute().await? else { return Err(ApiError::NotFound) };
    let Some(body) = obj.body() else { return Err(ApiError::NotFound) };
    let bytes = body.bytes().await?;
    let headers = Headers::new();
    headers.set("content-type", &row.content_type)?;
    headers.set("cache-control", "private, max-age=3600")?;
    Ok(Response::from_bytes(bytes)?.with_headers(headers))
}

pub async fn delete_all_for_user(app: &App, db: &Db, user_id: &str) -> ApiResult<()> {
    let rows: Vec<PhotoRow> = db
        .all("SELECT id, user_id, r2_key, content_type FROM photos WHERE user_id = ?1", &[user_id.into()])
        .await?;
    if let Ok(b) = app.env.bucket("PHOTOS") {
        for r in rows {
            let _ = b.delete(&r.r2_key).await;
        }
    }
    Ok(())
}
