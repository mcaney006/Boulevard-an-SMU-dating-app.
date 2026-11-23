//! Request-scoped context, errors, and small helpers shared by every handler.

use serde::Serialize;
use sha2::{Digest, Sha256};
use worker::*;

use crate::db::Db;

pub struct App {
    pub env: Env,
}

impl App {
    pub fn new(env: Env) -> Self {
        Self { env }
    }

    pub fn db(&self) -> std::result::Result<Db, ApiError> {
        Ok(Db::new(self.env.d1("DB")?))
    }

    /// Unix millis.
    pub fn now(&self) -> i64 {
        Date::now().as_millis() as i64
    }

    pub fn var(&self, name: &str) -> Option<String> {
        self.env.var(name).ok().map(|v| v.to_string())
    }

    pub fn var_bool(&self, name: &str) -> bool {
        matches!(
            self.var(name).as_deref().map(|s| s.trim().to_ascii_lowercase()).as_deref(),
            Some("true") | Some("1") | Some("yes")
        )
    }

    pub fn secret(&self, name: &str) -> Option<String> {
        self.env.secret(name).ok().map(|v| v.to_string())
    }

    pub fn admin_emails(&self) -> Vec<String> {
        self.var("ADMIN_EMAILS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }

    pub fn json<T: Serialize>(&self, value: T) -> std::result::Result<Response, ApiError> {
        Ok(Response::from_json(&value)?)
    }

    pub fn json_status<T: Serialize>(&self, value: T, status: u16) -> std::result::Result<Response, ApiError> {
        Ok(Response::from_json(&value)?.with_status(status))
    }

    pub fn cors(&self, response: Response) -> Response {
        if response.status_code() == 101 {
            return response;
        }
        let origins = self.var("CORS_ORIGINS").unwrap_or_else(|| "*".into());
        let headers = response.headers().clone();
        let _ = headers.set("access-control-allow-origin", &origins);
        let _ = headers.set("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS");
        let _ = headers.set("access-control-allow-headers", "authorization, content-type");
        let _ = headers.set("access-control-max-age", "86400");
        let _ = headers.set("vary", "origin");
        response.with_headers(headers)
    }
}

/// Every failure a handler can return, mapped to one JSON shape:
/// `{"error": {"code": "...", "message": "...", "fields": [...]?}}`.
#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Fields(Vec<boulevard_core::FieldError>),
    Unauthorized,
    Forbidden(String),
    NotFound,
    RateLimited { message: String, retry_after_s: i64 },
    LikeLimit { resets_at: i64, limit: u32 },
    Unavailable(String),
    Internal(String),
    NeedProfile,
    Custom { status: u16, code: &'static str, message: String },
}

impl ApiError {
    pub fn bad(msg: impl Into<String>) -> Self {
        ApiError::BadRequest(msg.into())
    }

    pub fn into_response(self) -> Response {
        let (status, code, message, extra) = match &self {
            ApiError::BadRequest(m) => (400, "bad_request", m.clone(), None),
            ApiError::Fields(_) => (400, "invalid_fields", "Fix the highlighted fields.".into(), None),
            ApiError::Unauthorized => (401, "unauthorized", "Sign in again.".into(), None),
            ApiError::Forbidden(m) => (403, "forbidden", m.clone(), None),
            ApiError::NotFound => (404, "not_found", "Not found.".into(), None),
            ApiError::RateLimited { message, retry_after_s } => (
                429,
                "rate_limited",
                message.clone(),
                Some(serde_json::json!({ "retry_after_s": retry_after_s })),
            ),
            ApiError::LikeLimit { resets_at, limit } => (
                429,
                "like_limit",
                format!("You have used your {limit} likes for today. Boulevard+ removes the limit."),
                Some(serde_json::json!({ "resets_at": resets_at, "limit": limit })),
            ),
            ApiError::Unavailable(m) => (503, "unavailable", m.clone(), None),
            ApiError::NeedProfile => (409, "need_profile", "Finish your profile first.".into(), None),
            ApiError::Custom { status, code, message } => (*status, *code, message.clone(), None),
            ApiError::Internal(m) => {
                console_error!("internal error: {m}");
                (500, "internal", "Something broke on our side.".into(), None)
            }
        };
        let mut error = serde_json::json!({ "code": code, "message": message });
        if let ApiError::Fields(fields) = &self {
            error["fields"] = serde_json::to_value(fields).unwrap_or_default();
        }
        if let Some(extra) = extra {
            if let (Some(e), Some(x)) = (error.as_object_mut(), extra.as_object()) {
                for (k, v) in x {
                    e.insert(k.clone(), v.clone());
                }
            }
        }
        Response::from_json(&serde_json::json!({ "error": error }))
            .map(|r| r.with_status(status))
            .unwrap_or_else(|_| Response::error("error", status).unwrap())
    }
}

impl From<worker::Error> for ApiError {
    fn from(e: worker::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        ApiError::BadRequest(format!("Invalid JSON: {e}"))
    }
}

pub type ApiResult<T = Response> = std::result::Result<T, ApiError>;

/// Parse a JSON body with a friendly error.
pub async fn body<T: serde::de::DeserializeOwned>(req: &mut Request) -> ApiResult<T> {
    req.json::<T>()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Invalid request body: {e}")))
}

/// Query string as a map.
pub fn query(req: &Request) -> std::collections::HashMap<String, String> {
    req.url()
        .map(|u| {
            u.query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn client_ip(req: &Request) -> String {
    req.headers()
        .get("cf-connecting-ip")
        .ok()
        .flatten()
        .unwrap_or_else(|| "unknown".into())
}

pub fn sha256_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    hex::encode(h.finalize())
}

pub fn random_bytes(n: usize) -> ApiResult<Vec<u8>> {
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf).map_err(|e| ApiError::Internal(format!("getrandom: {e}")))?;
    Ok(buf)
}

/// Opaque, URL-safe id: 16 random bytes, hex.
pub fn new_id(prefix: &str) -> ApiResult<String> {
    Ok(format!("{prefix}_{}", hex::encode(random_bytes(16)?)))
}

/// Six-digit code, uniformly drawn.
pub fn otp_code() -> ApiResult<String> {
    let b = random_bytes(4)?;
    let n = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) % 1_000_000;
    Ok(format!("{n:06}"))
}
