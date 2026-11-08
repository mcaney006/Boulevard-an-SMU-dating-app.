//! `@smu.edu` one-time codes and bearer sessions.
//!
//! Flow: `POST /v1/auth/start {email}` sends a six-digit code (10 minutes,
//! five attempts). `POST /v1/auth/verify {email, code}` consumes it, creates
//! the user on first sign-in, records `auth.verified`, and returns a session
//! token. Tokens are stored hashed; only the client ever holds the plaintext.

use base64::Engine;
use serde::{Deserialize, Serialize};
use worker::*;

use crate::app::{body, client_ip, new_id, otp_code, random_bytes, sha256_hex, ApiError, ApiResult, App};
use crate::db::{load_user, UserRow, V};
use crate::events;

pub const SESSION_TTL_MS: i64 = 30 * 24 * 60 * 60 * 1000;
const OTP_TTL_MS: i64 = 10 * 60 * 1000;
const OTP_MAX_ATTEMPTS: i64 = 5;
const HOUR_MS: i64 = 60 * 60 * 1000;

#[derive(Deserialize)]
struct StartBody {
    email: String,
}

#[derive(Serialize)]
struct StartResponse {
    ok: bool,
    email: String,
    expires_in_s: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_code: Option<String>,
}

pub async fn start(req: &mut Request, app: &App) -> ApiResult {
    let ip = client_ip(req);
    let input: StartBody = body(req).await?;
    let check = boulevard_core::normalize_email(&input.email);
    if !check.ok {
        return Err(ApiError::bad(check.reason));
    }
    let email = check.normalized;
    let db = app.db()?;
    let now = app.now();

    let per_email = app.var("OTP_PER_EMAIL_HOURLY").and_then(|v| v.parse().ok()).unwrap_or(5);
    let per_ip = app.var("OTP_PER_IP_HOURLY").and_then(|v| v.parse().ok()).unwrap_or(20);
    let (ok_email, retry) = db.rate_limit(&format!("otp:email:{email}"), HOUR_MS, per_email, now).await?;
    if !ok_email {
        return Err(ApiError::RateLimited {
            message: "Too many codes for that address. Try again later.".into(),
            retry_after_s: retry,
        });
    }
    let (ok_ip, retry) = db.rate_limit(&format!("otp:ip:{ip}"), HOUR_MS, per_ip, now).await?;
    if !ok_ip {
        return Err(ApiError::RateLimited {
            message: "Too many requests. Try again later.".into(),
            retry_after_s: retry,
        });
    }

    // A banned address never gets a code, and we do not say why.
    if let Some(user) = user_by_email(&db, &email).await? {
        if user.status == "banned" {
            return Ok(Response::from_json(&StartResponse {
                ok: true,
                email,
                expires_in_s: OTP_TTL_MS / 1000,
                dev_code: None,
            })?);
        }
    }

    let id = new_id("otp")?;
    let code = otp_code()?;
    let code_hash = hash_code(&id, &email, &code);
    db.batch(vec![
        db.stmt(
            "UPDATE otp_codes SET consumed_at = ?1 WHERE email = ?2 AND consumed_at IS NULL",
            &[now.into(), email.as_str().into()],
        )?,
        db.stmt(
            "INSERT INTO otp_codes (id, email, code_hash, created_at, expires_at, attempts, consumed_at, ip)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, NULL, ?6)",
            &[
                id.as_str().into(),
                email.as_str().into(),
                code_hash.into(),
                now.into(),
                (now + OTP_TTL_MS).into(),
                ip.as_str().into(),
            ],
        )?,
        events::record(&db, now, "auth.code_sent", None, Some(&email), serde_json::json!({ "ip": ip }))?,
    ])
    .await?;

    crate::email::send_code(app, &email, &code).await?;

    let dev_code = if app.var_bool("DEV_RETURN_OTP") { Some(code) } else { None };
    Ok(Response::from_json(&StartResponse {
        ok: true,
        email,
        expires_in_s: OTP_TTL_MS / 1000,
        dev_code,
    })?)
}

#[derive(Deserialize)]
struct VerifyBody {
    email: String,
    code: String,
}

#[derive(Deserialize)]
struct OtpRow {
    id: String,
    code_hash: String,
    expires_at: i64,
    attempts: i64,
}

#[derive(Serialize)]
pub struct SessionUser {
    pub id: String,
    pub email: String,
    pub status: String,
    pub plus: bool,
    pub admin: bool,
    pub profile_complete: bool,
}

pub async fn verify(req: &mut Request, app: &App) -> ApiResult {
    let input: VerifyBody = body(req).await?;
    let check = boulevard_core::normalize_email(&input.email);
    if !check.ok {
        return Err(ApiError::bad(check.reason));
    }
    let email = check.normalized;
    let code = input.code.trim().to_string();
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::bad("Enter the six-digit code."));
    }
    let db = app.db()?;
    let now = app.now();

    let row: Option<OtpRow> = db
        .first(
            "SELECT id, code_hash, expires_at, attempts FROM otp_codes
             WHERE email = ?1 AND consumed_at IS NULL ORDER BY created_at DESC LIMIT 1",
            &[email.as_str().into()],
        )
        .await?;
    let Some(otp) = row else {
        return Err(ApiError::bad("That code is not valid. Request a new one."));
    };
    if now > otp.expires_at {
        return Err(ApiError::bad("That code expired. Request a new one."));
    }
    if otp.attempts >= OTP_MAX_ATTEMPTS {
        db.run("UPDATE otp_codes SET consumed_at = ?1 WHERE id = ?2", &[now.into(), otp.id.as_str().into()])
            .await?;
        return Err(ApiError::bad("Too many wrong tries. Request a new code."));
    }
    if hash_code(&otp.id, &email, &code) != otp.code_hash {
        db.run("UPDATE otp_codes SET attempts = attempts + 1 WHERE id = ?1", &[otp.id.as_str().into()])
            .await?;
        return Err(ApiError::bad("That code is not right."));
    }

    // Code is good. Consume it, upsert the user, open a session: one batch.
    let existing = user_by_email(&db, &email).await?;
    let user_id = match &existing {
        Some(u) => u.id.clone(),
        None => new_id("usr")?,
    };
    if let Some(u) = &existing {
        match u.status.as_str() {
            "banned" => return Err(ApiError::Forbidden("This account is closed.".into())),
            "suspended" => {
                return Err(ApiError::Forbidden(
                    u.status_reason.clone().unwrap_or_else(|| "This account is suspended.".into()),
                ))
            }
            _ => {}
        }
    }

    let token = new_token()?;
    let token_hash = sha256_hex(&token);
    let ua = req.headers().get("user-agent").ok().flatten().unwrap_or_default();
    let mut batch = vec![db.stmt(
        "UPDATE otp_codes SET consumed_at = ?1 WHERE id = ?2",
        &[now.into(), otp.id.as_str().into()],
    )?];
    match &existing {
        None => batch.push(db.stmt(
            "INSERT INTO users (id, email, email_hash, status, created_at, verified_at, last_active_at)
             VALUES (?1, ?2, ?3, 'active', ?4, ?4, ?4)",
            &[
                user_id.as_str().into(),
                email.as_str().into(),
                sha256_hex(&email).into(),
                now.into(),
            ],
        )?),
        Some(u) if u.status == "deleted" => batch.push(db.stmt(
            "UPDATE users SET status = 'active', status_reason = NULL, verified_at = ?1, last_active_at = ?1 WHERE id = ?2",
            &[now.into(), user_id.as_str().into()],
        )?),
        Some(_) => batch.push(db.stmt(
            "UPDATE users SET verified_at = ?1, last_active_at = ?1 WHERE id = ?2",
            &[now.into(), user_id.as_str().into()],
        )?),
    }
    batch.push(db.stmt(
        "INSERT INTO sessions (id, user_id, created_at, expires_at, last_seen_at, user_agent)
         VALUES (?1, ?2, ?3, ?4, ?3, ?5)",
        &[
            token_hash.into(),
            user_id.as_str().into(),
            now.into(),
            (now + SESSION_TTL_MS).into(),
            ua.into(),
        ],
    )?);
    batch.push(events::record(
        &db,
        now,
        "auth.verified",
        Some(&user_id),
        Some(&user_id),
        serde_json::json!({ "email_domain": "smu.edu", "new_user": existing.is_none() }),
    )?);
    db.batch(batch).await?;

    let user = load_user(&db, &user_id).await?.ok_or(ApiError::Internal("user vanished".into()))?;
    let profile = crate::db::load_profile(&db, &user_id).await?;
    Ok(Response::from_json(&serde_json::json!({
        "token": token,
        "expires_at": now + SESSION_TTL_MS,
        "user": session_user(app, &user, profile.is_some()),
    }))?)
}

pub async fn logout(req: &Request, app: &App) -> ApiResult {
    let Some(token) = bearer(req) else { return Err(ApiError::Unauthorized) };
    let db = app.db()?;
    db.run(
        "UPDATE sessions SET revoked_at = ?1 WHERE id = ?2 AND revoked_at IS NULL",
        &[app.now().into(), sha256_hex(&token).into()],
    )
    .await?;
    Ok(Response::from_json(&serde_json::json!({ "ok": true }))?)
}

/// The signed-in member for a request. Rejects missing, expired, revoked
/// sessions and suspended/banned/deleted accounts.
pub struct Current {
    pub user: UserRow,
}

impl Current {
    pub fn id(&self) -> &str {
        &self.user.id
    }
    pub fn plus(&self) -> bool {
        self.user.plus_active != 0
    }
    pub fn is_admin(&self, app: &App) -> bool {
        self.user.is_admin != 0 || app.admin_emails().contains(&self.user.email)
    }
}

pub async fn require(req: &Request, app: &App) -> ApiResult<Current> {
    let token = bearer(req).or_else(|| ws_token(req)).ok_or(ApiError::Unauthorized)?;
    require_token(&token, app).await
}

pub async fn require_token(token: &str, app: &App) -> ApiResult<Current> {
    let db = app.db()?;
    let token_hash = sha256_hex(token);
    #[derive(Deserialize)]
    struct Row {
        user_id: String,
        expires_at: i64,
        last_seen_at: i64,
    }
    let row: Option<Row> = db
        .first(
            "SELECT user_id, expires_at, last_seen_at FROM sessions WHERE id = ?1 AND revoked_at IS NULL",
            &[token_hash.as_str().into()],
        )
        .await?;
    let Some(session) = row else { return Err(ApiError::Unauthorized) };
    let now = app.now();
    if now > session.expires_at {
        return Err(ApiError::Unauthorized);
    }
    let user = load_user(&db, &session.user_id).await?.ok_or(ApiError::Unauthorized)?;
    match user.status.as_str() {
        "active" => {}
        "suspended" => {
            return Err(ApiError::Forbidden(
                user.status_reason.clone().unwrap_or_else(|| "This account is suspended.".into()),
            ))
        }
        _ => return Err(ApiError::Unauthorized),
    }
    // Touch activity at most every five minutes to keep writes cheap.
    if now - session.last_seen_at > 5 * 60 * 1000 {
        db.batch(vec![
            db.stmt("UPDATE sessions SET last_seen_at = ?1 WHERE id = ?2", &[now.into(), token_hash.as_str().into()])?,
            db.stmt("UPDATE users SET last_active_at = ?1 WHERE id = ?2", &[now.into(), user.id.as_str().into()])?,
        ])
        .await?;
    }
    Ok(Current { user })
}

pub async fn require_admin(req: &Request, app: &App) -> ApiResult<Current> {
    let current = require(req, app).await?;
    if !current.is_admin(app) {
        return Err(ApiError::Forbidden("Moderators only.".into()));
    }
    Ok(current)
}

pub fn session_user(app: &App, user: &UserRow, profile_complete: bool) -> SessionUser {
    let admin = user.is_admin != 0 || app.admin_emails().contains(&user.email);
    SessionUser {
        id: user.id.clone(),
        email: user.email.clone(),
        status: user.status.clone(),
        plus: user.plus_active != 0,
        admin,
        profile_complete,
    }
}

fn bearer(req: &Request) -> Option<String> {
    let value = req.headers().get("authorization").ok().flatten()?;
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_string())
}

/// WebSocket clients in browsers cannot set headers; they may pass the token
/// as `?token=` on the upgrade request only.
fn ws_token(req: &Request) -> Option<String> {
    let upgrade = req.headers().get("upgrade").ok().flatten()?;
    if !upgrade.eq_ignore_ascii_case("websocket") {
        return None;
    }
    crate::app::query(req).get("token").cloned()
}

fn new_token() -> ApiResult<String> {
    let bytes = random_bytes(32)?;
    Ok(format!("blv_{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)))
}

fn hash_code(id: &str, email: &str, code: &str) -> String {
    sha256_hex(&format!("{id}:{email}:{code}"))
}

async fn user_by_email(db: &crate::db::Db, email: &str) -> ApiResult<Option<UserRow>> {
    db.first("SELECT * FROM users WHERE email = ?1", &[V::S(email.to_string())]).await
}
