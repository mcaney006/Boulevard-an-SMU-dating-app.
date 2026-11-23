//! Thin D1 helpers plus the row types shared by handlers.

use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;
use worker::*;

use crate::app::{ApiError, ApiResult};

/// A bound SQL value.
#[derive(Debug, Clone)]
pub enum V {
    S(String),
    I(i64),
    N,
}

impl From<&str> for V {
    fn from(s: &str) -> Self {
        V::S(s.to_string())
    }
}
impl From<String> for V {
    fn from(s: String) -> Self {
        V::S(s)
    }
}
impl From<i64> for V {
    fn from(i: i64) -> Self {
        V::I(i)
    }
}
impl From<i32> for V {
    fn from(i: i32) -> Self {
        V::I(i as i64)
    }
}
impl From<bool> for V {
    fn from(b: bool) -> Self {
        V::I(b as i64)
    }
}
impl<T: Into<V>> From<Option<T>> for V {
    fn from(o: Option<T>) -> Self {
        match o {
            Some(v) => v.into(),
            None => V::N,
        }
    }
}

fn js(v: &V) -> JsValue {
    match v {
        V::S(s) => JsValue::from_str(s),
        V::I(i) => JsValue::from_f64(*i as f64),
        V::N => JsValue::NULL,
    }
}

pub struct Db {
    inner: D1Database,
}

impl Db {
    pub fn new(inner: D1Database) -> Self {
        Self { inner }
    }

    pub fn stmt(&self, sql: &str, params: &[V]) -> ApiResult<D1PreparedStatement> {
        let values: Vec<JsValue> = params.iter().map(js).collect();
        Ok(self.inner.prepare(sql).bind(&values)?)
    }

    pub async fn first<T: for<'a> Deserialize<'a>>(&self, sql: &str, params: &[V]) -> ApiResult<Option<T>> {
        Ok(self.stmt(sql, params)?.first::<T>(None).await?)
    }

    pub async fn all<T: for<'a> Deserialize<'a>>(&self, sql: &str, params: &[V]) -> ApiResult<Vec<T>> {
        let res = self.stmt(sql, params)?.all().await?;
        if let Some(err) = res.error() {
            return Err(ApiError::Internal(err));
        }
        Ok(res.results::<T>()?)
    }

    pub async fn run(&self, sql: &str, params: &[V]) -> ApiResult<()> {
        let res = self.stmt(sql, params)?.run().await?;
        if let Some(err) = res.error() {
            return Err(ApiError::Internal(err));
        }
        Ok(())
    }

    /// Execute statements atomically (D1 batches are transactional).
    pub async fn batch(&self, statements: Vec<D1PreparedStatement>) -> ApiResult<()> {
        if statements.is_empty() {
            return Ok(());
        }
        let results = self.inner.batch(statements).await?;
        for r in results {
            if let Some(err) = r.error() {
                return Err(ApiError::Internal(err));
            }
        }
        Ok(())
    }

    /// Fixed-window counter. Returns `Ok(true)` while under `limit`.
    pub async fn rate_limit(&self, key: &str, window_ms: i64, limit: i64, now: i64) -> ApiResult<(bool, i64)> {
        let window_start = now - now.rem_euclid(window_ms);
        #[derive(Deserialize)]
        struct Row {
            count: i64,
        }
        let row: Option<Row> = self
            .first(
                "INSERT INTO rate_limits (key, window_start, count) VALUES (?1, ?2, 1)
                 ON CONFLICT(key) DO UPDATE SET
                   count = CASE WHEN rate_limits.window_start = excluded.window_start THEN rate_limits.count + 1 ELSE 1 END,
                   window_start = excluded.window_start
                 RETURNING count",
                &[key.into(), window_start.into()],
            )
            .await?;
        let count = row.map(|r| r.count).unwrap_or(1);
        let retry_after_s = ((window_start + window_ms - now).max(0) + 999) / 1000;
        Ok((count <= limit, retry_after_s))
    }
}

// ---- rows -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRow {
    pub id: String,
    pub email: String,
    pub status: String,
    pub status_reason: Option<String>,
    pub created_at: i64,
    pub verified_at: Option<i64>,
    pub last_active_at: i64,
    pub plus_active: i64,
    pub plus_source: Option<String>,
    pub is_admin: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileRow {
    pub user_id: String,
    pub display_name: String,
    pub gender: String,
    pub year: String,
    pub major: String,
    pub greek: String,
    pub intent: String,
    pub looking_for: String,
    pub bio: String,
    pub completed: i64,
    pub updated_at: i64,
}

impl ProfileRow {
    pub fn to_profile(&self) -> Option<boulevard_core::Profile> {
        Some(boulevard_core::Profile {
            display_name: self.display_name.clone(),
            gender: boulevard_core::Gender::parse(&self.gender)?,
            year: boulevard_core::Year::parse(&self.year)?,
            major: self.major.clone(),
            greek: self.greek.clone(),
            intent: boulevard_core::Intent::parse(&self.intent)?,
            looking_for: boulevard_core::Looking::parse(&self.looking_for)?,
            bio: self.bio.clone(),
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FiltersRow {
    pub intent: Option<String>,
    pub year: Option<String>,
    pub greek: Option<String>,
    pub major: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchRow {
    pub id: String,
    pub user_a: String,
    pub user_b: String,
    pub created_at: i64,
    pub closed_at: Option<i64>,
    pub closed_reason: Option<String>,
    pub last_message_at: Option<i64>,
    pub last_message_preview: Option<String>,
    pub last_message_from: Option<String>,
}

impl MatchRow {
    pub fn other(&self, me: &str) -> &str {
        if self.user_a == me {
            &self.user_b
        } else {
            &self.user_a
        }
    }
    pub fn has(&self, user: &str) -> bool {
        self.user_a == user || self.user_b == user
    }
}

/// Canonical pair ordering so (a, b) and (b, a) hit the same row.
pub fn pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

pub async fn load_user(db: &Db, id: &str) -> ApiResult<Option<UserRow>> {
    db.first("SELECT * FROM users WHERE id = ?1", &[id.into()]).await
}

pub async fn load_profile(db: &Db, user_id: &str) -> ApiResult<Option<ProfileRow>> {
    db.first("SELECT * FROM profiles WHERE user_id = ?1", &[user_id.into()]).await
}

pub async fn load_filters(db: &Db, user_id: &str) -> ApiResult<FiltersRow> {
    Ok(db
        .first::<FiltersRow>(
            "SELECT intent, year, greek, major FROM user_filters WHERE user_id = ?1",
            &[user_id.into()],
        )
        .await?
        .unwrap_or_default())
}

pub async fn load_match(db: &Db, id: &str) -> ApiResult<Option<MatchRow>> {
    db.first("SELECT * FROM matches WHERE id = ?1", &[id.into()]).await
}

pub async fn is_blocked_either_way(db: &Db, a: &str, b: &str) -> ApiResult<bool> {
    #[derive(Deserialize)]
    struct Row {
        n: i64,
    }
    let row: Option<Row> = db
        .first(
            "SELECT COUNT(*) AS n FROM blocks WHERE (blocker_id = ?1 AND blocked_id = ?2) OR (blocker_id = ?2 AND blocked_id = ?1)",
            &[a.into(), b.into()],
        )
        .await?;
    Ok(row.map(|r| r.n > 0).unwrap_or(false))
}
