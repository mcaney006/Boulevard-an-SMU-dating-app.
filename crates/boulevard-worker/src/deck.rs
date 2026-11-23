//! The deck and the likes tab. The order is the published score and nothing
//! else; see `boulevard_core::rules`.

use serde::{Deserialize, Serialize};
use worker::*;

use boulevard_core::standins::{is_standin_id, standins, StandIn};
use boulevard_core::{score, Gender, Intent, Looking, Score, ScoreCandidate, ScoreProfile, Year};

use crate::app::{query, ApiError, ApiResult, App};
use crate::auth::{require, Current};
use crate::db::{load_filters, load_profile, Db, FiltersRow, ProfileRow, V};

#[derive(Debug, Clone, Serialize)]
pub struct Card {
    pub id: String,
    pub display_name: String,
    pub gender: String,
    pub year: String,
    pub major: String,
    pub greek: String,
    pub intent: String,
    pub intent_label: String,
    pub bio: String,
    pub photos: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub school: Option<String>,
    pub score: Score,
    /// Only present for Boulevard+ members.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub likes_you: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub standin: bool,
    #[serde(skip)]
    pub last_active_at: i64,
    #[serde(skip)]
    pub created_at: i64,
}

#[derive(Deserialize)]
struct CandRow {
    user_id: String,
    display_name: String,
    gender: String,
    year: String,
    major: String,
    greek: String,
    intent: String,
    looking_for: String,
    bio: String,
    last_active_at: i64,
    created_at: i64,
    likes_you: i64,
    note: Option<String>,
}

const CANDIDATES_SQL: &str = "
SELECT p.user_id, p.display_name, p.gender, p.year, p.major, p.greek, p.intent, p.looking_for, p.bio,
       u.last_active_at, u.created_at,
       EXISTS(SELECT 1 FROM swipes s2 WHERE s2.user_id = p.user_id AND s2.target_id = ?1 AND s2.direction = 'like') AS likes_you,
       (SELECT s3.note FROM swipes s3 WHERE s3.user_id = p.user_id AND s3.target_id = ?1 AND s3.direction = 'like') AS note
FROM profiles p
JOIN users u ON u.id = p.user_id
WHERE u.id <> ?1 AND u.status = 'active' AND p.completed = 1
  AND NOT EXISTS (SELECT 1 FROM swipes s WHERE s.user_id = ?1 AND s.target_id = u.id)
  AND NOT EXISTS (SELECT 1 FROM blocks b WHERE (b.blocker_id = ?1 AND b.blocked_id = u.id) OR (b.blocker_id = u.id AND b.blocked_id = ?1))
  AND NOT EXISTS (SELECT 1 FROM matches m WHERE ((m.user_a = ?1 AND m.user_b = u.id) OR (m.user_a = u.id AND m.user_b = ?1)) AND m.closed_at IS NULL)
LIMIT 500";

pub struct Viewer {
    pub id: String,
    pub plus: bool,
    pub me: ScoreProfile,
    pub gender: Gender,
    pub looking_for: Looking,
    pub filters: FiltersRow,
}

pub async fn viewer(db: &Db, cur: &Current) -> ApiResult<Viewer> {
    let row = load_profile(db, cur.id()).await?.ok_or(ApiError::NeedProfile)?;
    let p = row.to_profile().ok_or(ApiError::Internal("corrupt profile".into()))?;
    Ok(Viewer {
        id: cur.id().to_string(),
        plus: cur.plus(),
        me: ScoreProfile { intent: p.intent, major: p.major, greek: p.greek, year: p.year },
        gender: p.gender,
        looking_for: p.looking_for,
        filters: load_filters(db, cur.id()).await?,
    })
}

fn passes_filters(v: &Viewer, intent: Intent, year: Year, greek: &str, major: &str) -> bool {
    if let Some(f) = v.filters.intent.as_deref().and_then(Intent::parse) {
        if f != intent {
            return false;
        }
    }
    if let Some(f) = v.filters.year.as_deref().and_then(Year::parse) {
        if f != year {
            return false;
        }
    }
    if v.plus {
        if let Some(f) = v.filters.greek.as_deref() {
            if boulevard_core::catalog::normalize_greek(f) != boulevard_core::catalog::normalize_greek(greek) {
                return false;
            }
        }
        if let Some(f) = v.filters.major.as_deref() {
            if f != major {
                return false;
            }
        }
    }
    true
}

fn card_from_row(v: &Viewer, r: &CandRow, photos: Vec<String>) -> Option<Card> {
    let gender = Gender::parse(&r.gender)?;
    let looking = Looking::parse(&r.looking_for)?;
    let intent = Intent::parse(&r.intent)?;
    let year = Year::parse(&r.year)?;
    if !v.looking_for.accepts(gender) || !looking.accepts(v.gender) {
        return None;
    }
    if !passes_filters(v, intent, year, &r.greek, &r.major) {
        return None;
    }
    let likes_you = r.likes_you != 0;
    let cand = ScoreCandidate { intent, major: r.major.clone(), greek: r.greek.clone(), year, likes_you };
    Some(Card {
        id: r.user_id.clone(),
        display_name: r.display_name.clone(),
        gender: gender.as_str().into(),
        year: year.as_str().into(),
        major: r.major.clone(),
        greek: r.greek.clone(),
        intent: intent.as_str().into(),
        intent_label: intent.label().into(),
        bio: r.bio.clone(),
        photos,
        photo: None,
        school: None,
        score: score(&v.me, &cand, v.plus),
        likes_you: v.plus.then_some(likes_you),
        note: if v.plus && likes_you { r.note.clone() } else { None },
        standin: false,
        last_active_at: r.last_active_at,
        created_at: r.created_at,
    })
}

pub fn card_from_standin(v: &Viewer, s: &StandIn) -> Option<Card> {
    if !v.looking_for.accepts(s.gender) {
        return None;
    }
    if !passes_filters(v, s.intent, s.year, &s.greek, &s.major) {
        return None;
    }
    let cand = ScoreCandidate { intent: s.intent, major: s.major.clone(), greek: s.greek.clone(), year: s.year, likes_you: s.likes_you };
    Some(Card {
        id: s.id.clone(),
        display_name: s.name.clone(),
        gender: s.gender.as_str().into(),
        year: s.year.as_str().into(),
        major: s.major.clone(),
        greek: s.greek.clone(),
        intent: s.intent.as_str().into(),
        intent_label: s.intent.label().into(),
        bio: s.bio.clone(),
        photos: vec![],
        photo: Some(s.photo.clone()),
        school: Some(s.school.clone()),
        score: score(&v.me, &cand, v.plus),
        likes_you: v.plus.then_some(s.likes_you),
        note: if v.plus && s.likes_you && !s.note.is_empty() { Some(s.note.clone()) } else { None },
        standin: true,
        last_active_at: 0,
        created_at: 0,
    })
}

/// Deterministic order: score, then recency, then seniority, then id.
pub fn sort_cards(cards: &mut [Card]) {
    cards.sort_by(|a, b| {
        b.score
            .total
            .cmp(&a.score.total)
            .then(b.last_active_at.cmp(&a.last_active_at))
            .then(a.created_at.cmp(&b.created_at))
            .then(a.id.cmp(&b.id))
    });
}

async fn photos_for(db: &Db, ids: &[String]) -> ApiResult<std::collections::HashMap<String, Vec<String>>> {
    let mut out: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    if ids.is_empty() {
        return Ok(out);
    }
    #[derive(Deserialize)]
    struct Row {
        id: String,
        user_id: String,
    }
    for chunk in ids.chunks(50) {
        let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "SELECT id, user_id FROM photos WHERE user_id IN ({}) ORDER BY user_id, position",
            placeholders.join(", ")
        );
        let params: Vec<V> = chunk.iter().map(|s| V::S(s.clone())).collect();
        for r in db.all::<Row>(&sql, &params).await? {
            out.entry(r.user_id).or_default().push(r.id);
        }
    }
    Ok(out)
}

async fn swiped_standins(db: &Db, me: &str) -> ApiResult<std::collections::HashSet<String>> {
    #[derive(Deserialize)]
    struct Row {
        target_id: String,
    }
    Ok(db
        .all::<Row>(
            "SELECT target_id FROM swipes WHERE user_id = ?1 AND target_id LIKE 'standin:%'",
            &[me.into()],
        )
        .await?
        .into_iter()
        .map(|r| r.target_id)
        .collect())
}

pub async fn deck(req: &Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    let db = app.db()?;
    let v = viewer(&db, &cur).await?;
    let q = query(req);
    let limit: usize = q.get("limit").and_then(|s| s.parse().ok()).unwrap_or(20).clamp(1, 50);

    let rows: Vec<CandRow> = db.all(CANDIDATES_SQL, &[v.id.as_str().into()]).await?;
    let ids: Vec<String> = rows.iter().map(|r| r.user_id.clone()).collect();
    let mut photos = photos_for(&db, &ids).await?;
    let mut cards: Vec<Card> = rows
        .iter()
        .filter_map(|r| card_from_row(&v, r, photos.remove(&r.user_id).unwrap_or_default()))
        .collect();

    if app.var_bool("STANDINS_ENABLED") {
        let swiped = swiped_standins(&db, &v.id).await?;
        for s in standins() {
            if swiped.contains(&s.id) {
                continue;
            }
            if let Some(c) = card_from_standin(&v, &s) {
                cards.push(c);
            }
        }
    }

    sort_cards(&mut cards);
    let total = cards.len();
    cards.truncate(limit);
    app.json(serde_json::json!({
        "cards": cards,
        "total": total,
        "reveal_likes": v.plus,
        "rules_version": boulevard_core::RULES_VERSION,
    }))
}

#[derive(Deserialize)]
struct LikeRow {
    user_id: String,
    display_name: String,
    gender: String,
    year: String,
    major: String,
    greek: String,
    intent: String,
    looking_for: String,
    bio: String,
    last_active_at: i64,
    created_at: i64,
    note: Option<String>,
    liked_at: i64,
}

const LIKES_SQL: &str = "
SELECT p.user_id, p.display_name, p.gender, p.year, p.major, p.greek, p.intent, p.looking_for, p.bio,
       u.last_active_at, u.created_at, s.note, s.created_at AS liked_at
FROM swipes s
JOIN profiles p ON p.user_id = s.user_id
JOIN users u ON u.id = s.user_id
WHERE s.target_id = ?1 AND s.direction = 'like' AND u.status = 'active'
  AND NOT EXISTS (SELECT 1 FROM swipes s2 WHERE s2.user_id = ?1 AND s2.target_id = s.user_id)
  AND NOT EXISTS (SELECT 1 FROM blocks b WHERE (b.blocker_id = ?1 AND b.blocked_id = s.user_id) OR (b.blocker_id = s.user_id AND b.blocked_id = ?1))
ORDER BY s.created_at DESC
LIMIT 200";

/// Who liked you. Free members get the count; Boulevard+ gets the people and their notes.
pub async fn likes(req: &Request, app: &App) -> ApiResult {
    let cur = require(req, app).await?;
    let db = app.db()?;
    let v = viewer(&db, &cur).await?;

    let rows: Vec<LikeRow> = db.all(LIKES_SQL, &[v.id.as_str().into()]).await?;
    let mut people: Vec<serde_json::Value> = Vec::new();
    let mut count = 0usize;
    let ids: Vec<String> = rows.iter().map(|r| r.user_id.clone()).collect();
    let mut photos = photos_for(&db, &ids).await?;
    for r in &rows {
        let cand = CandRow {
            user_id: r.user_id.clone(),
            display_name: r.display_name.clone(),
            gender: r.gender.clone(),
            year: r.year.clone(),
            major: r.major.clone(),
            greek: r.greek.clone(),
            intent: r.intent.clone(),
            looking_for: r.looking_for.clone(),
            bio: r.bio.clone(),
            last_active_at: r.last_active_at,
            created_at: r.created_at,
            likes_you: 1,
            note: r.note.clone(),
        };
        // Likes ignore the viewer's filters on purpose: someone already chose you.
        let unfiltered = Viewer { filters: FiltersRow::default(), ..clone_viewer(&v) };
        if let Some(mut card) = card_from_row(&unfiltered, &cand, photos.remove(&r.user_id).unwrap_or_default()) {
            count += 1;
            if v.plus {
                card.likes_you = Some(true);
                card.note = r.note.clone();
                let mut val = serde_json::to_value(&card).unwrap_or_default();
                val["liked_at"] = serde_json::json!(r.liked_at);
                people.push(val);
            }
        }
    }
    if app.var_bool("STANDINS_ENABLED") {
        let swiped = swiped_standins(&db, &v.id).await?;
        let unfiltered = Viewer { filters: FiltersRow::default(), ..clone_viewer(&v) };
        for s in standins().into_iter().filter(|s| s.likes_you && !swiped.contains(&s.id)) {
            if let Some(mut card) = card_from_standin(&unfiltered, &s) {
                count += 1;
                if v.plus {
                    card.likes_you = Some(true);
                    card.note = (!s.note.is_empty()).then(|| s.note.clone());
                    people.push(serde_json::to_value(&card).unwrap_or_default());
                }
            }
        }
    }
    app.json(serde_json::json!({
        "count": count,
        "hidden": !v.plus,
        "people": people,
    }))
}

fn clone_viewer(v: &Viewer) -> Viewer {
    Viewer {
        id: v.id.clone(),
        plus: v.plus,
        me: v.me.clone(),
        gender: v.gender,
        looking_for: v.looking_for,
        filters: v.filters.clone(),
    }
}

/// A compact card for another member, used by the matches list and admin.
pub async fn peer_card(db: &Db, id: &str) -> ApiResult<Option<serde_json::Value>> {
    if is_standin_id(id) {
        return Ok(boulevard_core::standins::standin_by_id(id).map(|s| {
            serde_json::json!({
                "id": s.id, "display_name": s.name, "gender": s.gender.as_str(), "year": s.year.as_str(),
                "major": s.major, "greek": s.greek, "intent": s.intent.as_str(), "intent_label": s.intent.label(),
                "bio": s.bio, "photos": [], "photo": s.photo, "school": s.school, "standin": true,
            })
        }));
    }
    let Some(p): Option<ProfileRow> = load_profile(db, id).await? else { return Ok(None) };
    let photos = crate::media::photo_ids(db, id).await?;
    let intent_label = Intent::parse(&p.intent).map(|i| i.label()).unwrap_or("");
    Ok(Some(serde_json::json!({
        "id": p.user_id, "display_name": p.display_name, "gender": p.gender, "year": p.year,
        "major": p.major, "greek": p.greek, "intent": p.intent, "intent_label": intent_label,
        "bio": p.bio, "photos": photos, "standin": false,
    })))
}
