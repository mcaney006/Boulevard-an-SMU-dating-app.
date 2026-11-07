//! Likes, passes, the free daily limit, and match creation.
//!
//! A swipe, the event, and (on a mutual like) the match row all land in one
//! D1 batch, so there is never a like without its event or a match without
//! both likes.

use serde::Deserialize;
use worker::*;

use boulevard_core::rules::FREE_DAILY_LIKES;
use boulevard_core::standins::{is_standin_id, standin_by_id};
use boulevard_core::time::{chicago_day_start_ms, chicago_next_day_start_ms};
use boulevard_core::validate::clean_note;

use crate::app::{body, new_id, ApiError, ApiResult, App};
use crate::auth::require;
use crate::db::{is_blocked_either_way, load_profile, load_user, pair, Db, V};
use crate::events;

#[derive(Deserialize)]
struct SwipeBody {
    target_id: String,
    direction: String,
    note: Option<String>,
}

/// Likes since Chicago midnight, and when the window resets.
pub async fn likes_today(db: &Db, me: &str, now: i64) -> ApiResult<(i64, i64)> {
    let start = chicago_day_start_ms(now);
    #[derive(Deserialize)]
    struct Row {
        n: i64,
    }
    let row: Option<Row> = db
        .first(
            "SELECT COUNT(*) AS n FROM swipes WHERE user_id = ?1 AND direction = 'like' AND created_at >= ?2",
            &[me.into(), start.into()],
        )
        .await?;
    Ok((row.map(|r| r.n).unwrap_or(0), chicago_next_day_start_ms(now)))
}

#[derive(Deserialize)]
struct ExistingSwipe {
    direction: String,
}

#[derive(Deserialize)]
struct ReciprocalLike {
    note: Option<String>,
    created_at: i64,
}

#[derive(Deserialize)]
struct OpenMatch {
    id: String,
}

pub async fn swipe(req: &mut Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    let input: SwipeBody = body(req).await?;
    let direction = match input.direction.as_str() {
        "like" | "pass" => input.direction.clone(),
        _ => return Err(ApiError::bad("direction must be like or pass")),
    };
    let note = if direction == "like" {
        clean_note(input.note.as_deref()).map_err(|e| ApiError::Fields(vec![e]))?
    } else {
        None
    };
    let db = app.db()?;
    let now = app.now();
    let me = cur.id().to_string();
    let target = input.target_id.trim().to_string();
    if target.is_empty() || target == me {
        return Err(ApiError::bad("Pick someone else."));
    }
    load_profile(&db, &me).await?.ok_or(ApiError::NeedProfile)?;

    // Resolve the target: a scripted stand-in or a live member.
    let standin = if is_standin_id(&target) {
        if !app.var_bool("STANDINS_ENABLED") {
            return Err(ApiError::NotFound);
        }
        Some(standin_by_id(&target).ok_or(ApiError::NotFound)?)
    } else {
        let user = load_user(&db, &target).await?.ok_or(ApiError::NotFound)?;
        if user.status != "active" || load_profile(&db, &target).await?.is_none() {
            return Err(ApiError::NotFound);
        }
        if is_blocked_either_way(&db, &me, &target).await? {
            return Err(ApiError::NotFound);
        }
        None
    };

    // Idempotent: a repeat swipe reports the current state without charging a like.
    let existing: Option<ExistingSwipe> = db
        .first(
            "SELECT direction FROM swipes WHERE user_id = ?1 AND target_id = ?2",
            &[me.as_str().into(), target.as_str().into()],
        )
        .await?;
    if let Some(e) = existing {
        let (a, b) = pair(&me, &target);
        let m: Option<OpenMatch> = db
            .first(
                "SELECT id FROM matches WHERE user_a = ?1 AND user_b = ?2 AND closed_at IS NULL",
                &[a.into(), b.into()],
            )
            .await?;
        let (likes, reset) = likes_today(&db, &me, now).await?;
        return app.json(serde_json::json!({
            "ok": true, "already": true, "direction": e.direction,
            "matched": m.is_some(), "match_id": m.map(|m| m.id),
            "likes_today": likes, "like_limit": (!cur.plus()).then_some(FREE_DAILY_LIKES), "likes_reset_at": reset,
        }));
    }

    let (likes, reset) = likes_today(&db, &me, now).await?;
    if direction == "like" && !cur.plus() && likes >= FREE_DAILY_LIKES as i64 {
        return Err(ApiError::LikeLimit { resets_at: reset, limit: FREE_DAILY_LIKES });
    }

    // Does the other side already like me?
    let reciprocal: Option<ReciprocalLike> = match &standin {
        Some(s) => s.likes_you.then(|| ReciprocalLike {
            note: (!s.note.is_empty()).then(|| s.note.clone()),
            created_at: now - 1,
        }),
        None => {
            db.first(
                "SELECT note, created_at FROM swipes WHERE user_id = ?1 AND target_id = ?2 AND direction = 'like'",
                &[target.as_str().into(), me.as_str().into()],
            )
            .await?
        }
    };
    let matched = direction == "like" && reciprocal.is_some();

    let mut batch = vec![
        db.stmt(
            "INSERT INTO swipes (user_id, target_id, direction, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            &[me.as_str().into(), target.as_str().into(), direction.as_str().into(), note.clone().into(), now.into()],
        )?,
        events::record(
            &db,
            now,
            "swipe.recorded",
            Some(&me),
            Some(&target),
            serde_json::json!({ "direction": direction, "has_note": note.is_some(), "standin": standin.is_some() }),
        )?,
    ];
    let match_id = if matched {
        let id = new_id("mch")?;
        let (a, b) = pair(&me, &target);
        batch.push(db.stmt(
            "INSERT INTO matches (id, user_a, user_b, created_at) VALUES (?1, ?2, ?3, ?4)",
            &[id.as_str().into(), a.into(), b.into(), now.into()],
        )?);
        batch.push(events::record(
            &db,
            now,
            "match.created",
            Some(&me),
            Some(&id),
            serde_json::json!({ "members": [me, target], "standin": standin.is_some() }),
        )?);
        Some(id)
    } else {
        None
    };
    db.batch(batch).await?;

    // Seed the chat with the notes: theirs first (they liked first), then mine.
    if let Some(mid) = &match_id {
        let mut seed = Vec::new();
        if let Some(r) = &reciprocal {
            if let Some(n) = &r.note {
                seed.push(crate::chat::SeedMessage { sender_id: target.clone(), body: n.clone(), created_at: r.created_at.min(now - 1) });
            }
        }
        if let Some(n) = &note {
            seed.push(crate::chat::SeedMessage { sender_id: me.clone(), body: n.clone(), created_at: now });
        }
        if let Err(e) = crate::chat::seed_room(app, mid, &[me.clone(), target.clone()], &seed).await {
            console_error!("seed room {mid}: {e:?}");
        }
        if let Some(last) = seed.last() {
            let _ = db
                .run(
                    "UPDATE matches SET last_message_at = ?1, last_message_preview = ?2, last_message_from = ?3 WHERE id = ?4",
                    &[last.created_at.into(), V::S(preview(&last.body)), last.sender_id.as_str().into(), mid.as_str().into()],
                )
                .await;
        }
    }

    let (likes, reset) = likes_today(&db, &me, now).await?;
    app.json(serde_json::json!({
        "ok": true, "already": false, "direction": direction,
        "matched": matched, "match_id": match_id,
        "likes_today": likes, "like_limit": (!cur.plus()).then_some(FREE_DAILY_LIKES), "likes_reset_at": reset,
    }))
}

pub fn preview(body: &str) -> String {
    let mut s: String = body.chars().take(80).collect();
    if body.chars().count() > 80 {
        s.push('…');
    }
    s
}
