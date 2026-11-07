//! Verification mail. `log` is for local development; `resend` is a real
//! provider that works without a Cloudflare zone. Cloudflare Email Service
//! can be added as a third variant once Boulevard owns a domain.

use serde::Serialize;
use worker::*;

use crate::app::{ApiError, ApiResult, App};

pub async fn send_code(app: &App, to: &str, code: &str) -> ApiResult<()> {
    let provider = app.var("EMAIL_PROVIDER").unwrap_or_else(|| "log".into());
    match provider.as_str() {
        "resend" => send_resend(app, to, code).await,
        _ => {
            console_log!("[email:log] verification code for {to}: {code}");
            Ok(())
        }
    }
}

#[derive(Serialize)]
struct ResendBody<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: &'a str,
    text: String,
}

async fn send_resend(app: &App, to: &str, code: &str) -> ApiResult<()> {
    let key = app
        .secret("RESEND_API_KEY")
        .ok_or_else(|| ApiError::Unavailable("Email is not configured.".into()))?;
    let from = app
        .var("EMAIL_FROM")
        .unwrap_or_else(|| "Boulevard <onboarding@resend.dev>".into());
    let body = ResendBody {
        from: &from,
        to: [to],
        subject: "Your Boulevard code",
        text: format!(
            "Your Boulevard verification code is {code}. It expires in 10 minutes.\n\nIf you did not ask for this, ignore it."
        ),
    };
    let headers = Headers::new();
    headers.set("authorization", &format!("Bearer {key}"))?;
    headers.set("content-type", "application/json")?;
    let init = RequestInit {
        method: Method::Post,
        headers,
        body: Some(worker::wasm_bindgen::JsValue::from_str(&serde_json::to_string(&body)?)),
        ..Default::default()
    };
    let req = Request::new_with_init("https://api.resend.com/emails", &init)?;
    let mut res = Fetch::Request(req).send().await?;
    if res.status_code() >= 300 {
        let text = res.text().await.unwrap_or_default();
        console_error!("resend failed: {} {}", res.status_code(), text);
        return Err(ApiError::Unavailable("Could not send the code. Try again in a minute.".into()));
    }
    Ok(())
}
