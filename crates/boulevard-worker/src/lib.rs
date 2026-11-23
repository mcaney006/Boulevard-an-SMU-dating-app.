//! Boulevard API. One Worker, one D1 database, one Durable Object per chat.
//!
//! Routing is a plain match on method and path so every handler reads the
//! same way: authenticate, validate with `boulevard_core`, write to D1 in a
//! batch (atomic), append an event, respond with JSON.

mod admin;
mod app;
mod auth;
mod chat;
mod chat_room;
mod db;
mod deck;
mod email;
mod events;
mod matches;
mod me;
mod media;
mod safety;
mod swipes;

use worker::*;

use crate::app::{ApiError, App};

pub use chat_room::ChatRoom;

#[event(fetch)]
pub async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    let started = Date::now().as_millis();
    let method = req.method();
    let path = req.path();
    let app = App::new(env);

    if method == Method::Options {
        return Ok(app.cors(Response::empty()?.with_status(204)));
    }

    let result = route(req, &app).await;
    let response = match result {
        Ok(r) => r,
        Err(e) => e.into_response(),
    };
    let status = response.status_code();
    console_log!(
        "{} {} -> {} ({}ms)",
        method,
        path,
        status,
        Date::now().as_millis().saturating_sub(started)
    );
    Ok(app.cors(response))
}

async fn route(mut req: Request, app: &App) -> std::result::Result<Response, ApiError> {
    let method = req.method();
    let path = req.path();
    let segs: Vec<&str> = path.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();

    use Method::*;
    match (method, segs.as_slice()) {
        (Get, ["v1", "health"]) => app.json(serde_json::json!({
            "ok": true,
            "service": "boulevard-api",
            "version": env!("CARGO_PKG_VERSION"),
            "rules_version": boulevard_core::RULES_VERSION,
        })),
        (Get, ["v1", "rules"]) => me::rules(app),
        (Get, ["v1", "catalog"]) => me::rules(app),

        (Post, ["v1", "auth", "start"]) => auth::start(&mut req, app).await,
        (Post, ["v1", "auth", "verify"]) => auth::verify(&mut req, app).await,
        (Post, ["v1", "auth", "logout"]) => auth::logout(&req, app).await,

        (Get, ["v1", "me"]) => me::get_me(&req, app).await,
        (Put, ["v1", "me", "profile"]) => me::put_profile(&mut req, app).await,
        (Put, ["v1", "me", "filters"]) => me::put_filters(&mut req, app).await,
        (Post, ["v1", "me", "plus"]) => me::set_plus(&mut req, app).await,
        (Delete, ["v1", "me"]) => me::delete_account(&req, app).await,

        (Get, ["v1", "deck"]) => deck::deck(&req, app).await,
        (Get, ["v1", "likes"]) => deck::likes(&req, app).await,
        (Post, ["v1", "swipes"]) => swipes::swipe(&mut req, app).await,

        (Get, ["v1", "matches"]) => matches::list(&req, app).await,
        (Delete, ["v1", "matches", id]) => matches::unmatch(&req, app, id).await,
        (Get, ["v1", "matches", id, "messages"]) => chat::history(&req, app, id).await,
        (Post, ["v1", "matches", id, "messages"]) => chat::send(&mut req, app, id).await,
        (Post, ["v1", "matches", id, "read"]) => chat::mark_read(&mut req, app, id).await,
        (Get, ["v1", "matches", id, "ws"]) => chat::websocket(&req, app, id).await,

        (Post, ["v1", "blocks"]) => safety::block(&mut req, app).await,
        (Delete, ["v1", "blocks", id]) => safety::unblock(&req, app, id).await,
        (Post, ["v1", "reports"]) => safety::report(&mut req, app).await,

        (Get, ["v1", "me", "photos"]) => media::list_mine(&req, app).await,
        (Post, ["v1", "me", "photos"]) => media::upload(&mut req, app).await,
        (Delete, ["v1", "me", "photos", id]) => media::delete(&req, app, id).await,
        (Get, ["v1", "photos", id]) => media::serve(&req, app, id).await,

        (Get, ["v1", "admin", "reports"]) => admin::list_reports(&req, app).await,
        (Post, ["v1", "admin", "reports", id, "resolve"]) => admin::resolve_report(&mut req, app, id).await,
        (Get, ["v1", "admin", "users", id]) => admin::get_user(&req, app, id).await,
        (Get, ["v1", "admin", "matches", id, "transcript"]) => admin::transcript(&req, app, id).await,

        _ => Err(ApiError::NotFound),
    }
}
