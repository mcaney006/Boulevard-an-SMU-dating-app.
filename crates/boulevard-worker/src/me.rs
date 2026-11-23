//! The signed-in member: snapshot, profile, filters, Plus, deletion, rules.

use serde::{Deserialize, Serialize};
use worker::*;

use boulevard_core::catalog::{CATALOG_VERSION, GREEKS, MAJORS};
use boulevard_core::rules::{FREE_DAILY_LIKES, PLUS_PRICE_LABEL, RULES_VERSION};
use boulevard_core::types::ProfileInput;

use crate::app::{body, ApiError, ApiResult, App};
use crate::auth::{require, session_user};
use crate::db::{load_filters, load_profile, FiltersRow, V};
use crate::events;

pub fn rules(app: &App) -> ApiResult {
    app.json(serde_json::json!({
        "rules_version": RULES_VERSION,
        "rules": boulevard_core::published_rules(),
        "free_daily_likes": FREE_DAILY_LIKES,
        "plus_price": PLUS_PRICE_LABEL,
        "plus_features": [
            "Unlimited likes",
            "See who already liked you, with their note",
            "House and major filters",
            "The \"already liked you\" +15 shows before you swipe",
        ],
        "catalog_version": CATALOG_VERSION,
        "majors": MAJORS,
        "greeks": GREEKS,
        "intents": ["casual", "dating", "relationship"],
        "years": ["first-year", "sophomore", "junior", "senior", "graduate"],
        "genders": ["woman", "man", "nonbinary"],
        "looking_for": ["women", "men", "everyone"],
        "standins_enabled": app.var_bool("STANDINS_ENABLED"),
    }))
}

#[derive(Serialize)]
struct Me {
    user: crate::auth::SessionUser,
    profile: Option<boulevard_core::Profile>,
    filters: FiltersRow,
    likes_today: i64,
    like_limit: Option<u32>,
    likes_reset_at: i64,
    photos: Vec<String>,
    standins_enabled: bool,
    plus_preview_unlock: bool,
}

pub async fn get_me(req: &Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    let db = app.db()?;
    let profile = load_profile(&db, cur.id()).await?;
    let filters = load_filters(&db, cur.id()).await?;
    let (likes_today, reset) = crate::swipes::likes_today(&db, cur.id(), app.now()).await?;
    let photos = crate::media::photo_ids(&db, cur.id()).await?;
    app.json(Me {
        user: session_user(app, &cur.user, profile.is_some()),
        profile: profile.and_then(|p| p.to_profile()),
        filters,
        likes_today,
        like_limit: if cur.plus() { None } else { Some(FREE_DAILY_LIKES) },
        likes_reset_at: reset,
        photos,
        standins_enabled: app.var_bool("STANDINS_ENABLED"),
        plus_preview_unlock: app.var_bool("PLUS_PREVIEW_UNLOCK"),
    })
}

pub async fn put_profile(req: &mut Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    let input: ProfileInput = body(req).await?;
    let profile = boulevard_core::validate_profile(&input).map_err(ApiError::Fields)?;
    let db = app.db()?;
    let now = app.now();
    db.batch(vec![
        db.stmt(
            "INSERT INTO profiles (user_id, display_name, gender, year, major, greek, intent, looking_for, bio, completed, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10)
             ON CONFLICT(user_id) DO UPDATE SET
               display_name = excluded.display_name, gender = excluded.gender, year = excluded.year,
               major = excluded.major, greek = excluded.greek, intent = excluded.intent,
               looking_for = excluded.looking_for, bio = excluded.bio, completed = 1, updated_at = excluded.updated_at",
            &[
                cur.id().into(),
                profile.display_name.as_str().into(),
                profile.gender.as_str().into(),
                profile.year.as_str().into(),
                profile.major.as_str().into(),
                profile.greek.as_str().into(),
                profile.intent.as_str().into(),
                profile.looking_for.as_str().into(),
                profile.bio.as_str().into(),
                now.into(),
            ],
        )?,
        events::record(
            &db,
            now,
            "profile.updated",
            Some(cur.id()),
            Some(cur.id()),
            serde_json::json!({
                "year": profile.year.as_str(), "major": profile.major, "greek": profile.greek,
                "intent": profile.intent.as_str(), "looking_for": profile.looking_for.as_str(),
            }),
        )?,
    ])
    .await?;
    app.json(serde_json::json!({ "ok": true, "profile": profile }))
}

#[derive(Deserialize)]
struct FiltersBody {
    intent: Option<String>,
    year: Option<String>,
    greek: Option<String>,
    major: Option<String>,
}

pub async fn put_filters(req: &mut Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    let input: FiltersBody = body(req).await?;
    let clean = |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let intent = clean(input.intent);
    let year = clean(input.year);
    let greek = clean(input.greek);
    let major = clean(input.major);
    let mut errors = vec![];
    if let Some(i) = &intent {
        if boulevard_core::Intent::parse(i).is_none() {
            errors.push(boulevard_core::FieldError { field: "intent".into(), message: "Unknown intent.".into() });
        }
    }
    if let Some(y) = &year {
        if boulevard_core::Year::parse(y).is_none() {
            errors.push(boulevard_core::FieldError { field: "year".into(), message: "Unknown year.".into() });
        }
    }
    if let Some(g) = &greek {
        if !boulevard_core::catalog::is_greek(g) {
            errors.push(boulevard_core::FieldError { field: "greek".into(), message: "Unknown house.".into() });
        }
    }
    if let Some(m) = &major {
        if !boulevard_core::catalog::is_major(m) {
            errors.push(boulevard_core::FieldError { field: "major".into(), message: "Unknown major.".into() });
        }
    }
    if !errors.is_empty() {
        return Err(ApiError::Fields(errors));
    }
    if !cur.plus() && (greek.is_some() || major.is_some()) {
        return Err(ApiError::Forbidden("House and major filters are part of Boulevard+.".into()));
    }
    let db = app.db()?;
    let now = app.now();
    db.run(
        "INSERT INTO user_filters (user_id, intent, year, greek, major, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(user_id) DO UPDATE SET intent = excluded.intent, year = excluded.year,
           greek = excluded.greek, major = excluded.major, updated_at = excluded.updated_at",
        &[
            cur.id().into(),
            intent.clone().into(),
            year.clone().into(),
            greek.clone().into(),
            major.clone().into(),
            now.into(),
        ],
    )
    .await?;
    app.json(serde_json::json!({ "ok": true, "filters": FiltersRow { intent, year, greek, major } }))
}

#[derive(Deserialize)]
struct PlusBody {
    active: bool,
}

/// The prototype's preview unlock: Plus on or off with no card. Gated by
/// `PLUS_PREVIEW_UNLOCK` so it can be switched off the day billing ships.
pub async fn set_plus(req: &mut Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    if !app.var_bool("PLUS_PREVIEW_UNLOCK") {
        return Err(ApiError::Forbidden("Boulevard+ purchases are not open yet.".into()));
    }
    let input: PlusBody = body(req).await?;
    let db = app.db()?;
    let now = app.now();
    db.batch(vec![
        db.stmt(
            "UPDATE users SET plus_active = ?1, plus_source = ?2 WHERE id = ?3",
            &[input.active.into(), if input.active { V::S("preview".into()) } else { V::N }, cur.id().into()],
        )?,
        events::record(
            &db,
            now,
            if input.active { "plus.activated" } else { "plus.deactivated" },
            Some(cur.id()),
            Some(cur.id()),
            serde_json::json!({ "source": "preview" }),
        )?,
    ])
    .await?;
    app.json(serde_json::json!({ "ok": true, "plus": input.active }))
}

/// Delete the account: wipe the profile and photos, close matches, revoke
/// sessions, keep a tombstone (id + email hash) so the row history and a
/// seven-day re-registration cooldown survive.
pub async fn delete_account(req: &Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    let db = app.db()?;
    let now = app.now();
    let me = cur.id();

    crate::media::delete_all_for_user(app, &db, me).await?;

    let open: Vec<crate::db::MatchRow> = db
        .all(
            "SELECT * FROM matches WHERE (user_a = ?1 OR user_b = ?1) AND closed_at IS NULL",
            &[me.into()],
        )
        .await?;
    let mut batch = vec![
        db.stmt("DELETE FROM profiles WHERE user_id = ?1", &[me.into()])?,
        db.stmt("DELETE FROM user_filters WHERE user_id = ?1", &[me.into()])?,
        db.stmt("DELETE FROM swipes WHERE user_id = ?1 OR target_id = ?1", &[me.into()])?,
        db.stmt("DELETE FROM photos WHERE user_id = ?1", &[me.into()])?,
        db.stmt(
            "UPDATE matches SET closed_at = ?1, closed_reason = 'deleted', closed_by = ?2 WHERE (user_a = ?2 OR user_b = ?2) AND closed_at IS NULL",
            &[now.into(), me.into()],
        )?,
        db.stmt(
            "UPDATE sessions SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL",
            &[now.into(), me.into()],
        )?,
        db.stmt(
            "UPDATE users SET status = 'deleted', status_reason = 'self', plus_active = 0, plus_source = NULL WHERE id = ?1",
            &[me.into()],
        )?,
        events::record(&db, now, "account.deleted", Some(me), Some(me), serde_json::json!({ "matches_closed": open.len() }))?,
    ];
    for m in &open {
        batch.push(events::record(&db, now, "match.closed", Some(me), Some(&m.id), serde_json::json!({ "reason": "deleted" }))?);
    }
    db.batch(batch).await?;
    for m in &open {
        let _ = crate::chat::purge_room(app, &m.id).await;
    }
    app.json(serde_json::json!({ "ok": true }))
}
