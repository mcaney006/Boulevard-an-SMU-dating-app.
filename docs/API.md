# Boulevard API

Base URL: `https://boulevard-api.boulevard.workers.dev` (local: `http://localhost:8787`).
JSON in, JSON out. Times are Unix milliseconds. Authenticated routes take
`Authorization: Bearer <token>`; the WebSocket route also accepts `?token=`.

Errors always look like:

```json
{ "error": { "code": "need_profile", "message": "Finish your profile first." } }
```

Codes: `bad_request`, `invalid_fields` (with `fields: [{field, message}]`), `unauthorized`,
`forbidden`, `not_found`, `rate_limited` (`retry_after_s`), `like_limit` (`resets_at`, `limit`),
`need_profile`, `match_closed`, `photos_disabled`, `unavailable`, `internal`.

## Public

| Method | Path | Notes |
|--------|------|-------|
| GET | `/v1/health` | `{ok, service, version, rules_version}` |
| GET | `/v1/rules` | The published rules, Plus features, catalogs (majors, houses, enums) |

## Sign-in

| Method | Path | Body | Response |
|--------|------|------|----------|
| POST | `/v1/auth/start` | `{email}` | `{ok, email, expires_in_s, dev_code?}` Only `@smu.edu`; no `+` tags. 5 per address and 20 per IP per hour. |
| POST | `/v1/auth/verify` | `{email, code}` | `{token, expires_at, user}` Five wrong tries burn the code. Sessions last 30 days. |
| POST | `/v1/auth/logout` | | Revokes the token. |

`user`: `{id, email, status, plus, admin, profile_complete}`.

## Me

| Method | Path | Body | Notes |
|--------|------|------|-------|
| GET | `/v1/me` | | `{user, profile, filters, likes_today, like_limit, likes_reset_at, photos, standins_enabled, plus_preview_unlock}` |
| PUT | `/v1/me/profile` | `{display_name, gender, year, major, greek, intent, looking_for, bio}` | Validated by the shared core; all errors returned at once. |
| PUT | `/v1/me/filters` | `{intent?, year?, greek?, major?}` | `greek` and `major` need Plus. Omit a key to clear it. |
| POST | `/v1/me/plus` | `{active}` | Preview unlock; 403 when `PLUS_PREVIEW_UNLOCK` is off. |
| DELETE | `/v1/me` | | Wipes profile, swipes, photos; closes matches; revokes sessions; keeps a tombstone. |

Enum values: `gender` woman/man/nonbinary, `year` first-year/sophomore/junior/senior/graduate,
`intent` casual/dating/relationship, `looking_for` women/men/everyone. `greek` is a house from the
catalog or `Independent`. Lists come from `/v1/rules`.

## Deck and likes

| Method | Path | Notes |
|--------|------|-------|
| GET | `/v1/deck?limit=20` | `{cards, total, reveal_likes, rules_version}` ranked by the score. |
| GET | `/v1/likes` | `{count, hidden, people}`; `people` only on Plus. Likes ignore your filters. |
| POST | `/v1/swipes` | `{target_id, direction: "like"\|"pass", note?}` → `{ok, already, matched, match_id, likes_today, like_limit, likes_reset_at}` |

A card:

```json
{
  "id": "usr_…", "display_name": "Ava Chen", "gender": "woman", "year": "junior",
  "major": "Finance", "greek": "Kappa Kappa Gamma", "intent": "dating", "intent_label": "Actually dating",
  "bio": "…", "photos": ["pho_…"], "standin": false,
  "score": { "total": 80, "reasons": [ { "label": "Same intent", "points": 40 }, … ] },
  "likes_you": true, "note": "…"            // Plus only
}
```

Stand-ins have ids like `standin:ava`, `standin: true`, and a `photo` path instead of `photos`.

## Matches and chat

| Method | Path | Notes |
|--------|------|-------|
| GET | `/v1/matches` | `{matches: [{id, created_at, other: card, last_message: {at, preview, from_me}?}]}` |
| DELETE | `/v1/matches/:id` | Unmatch. |
| GET | `/v1/matches/:id/messages?after=0&limit=50` | `{messages: [{id, sender_id, body, kind, created_at}], members: [{user_id, last_read_id}]}` |
| POST | `/v1/matches/:id/messages` | `{body}` → `{ok, message}`. 500 chars, 30 per minute. |
| POST | `/v1/matches/:id/read` | `{last_read_id}` |
| GET | `/v1/matches/:id/ws` | WebSocket upgrade. |

WebSocket frames are JSON. Client → server: `{"type":"message","body":"…"}`, `{"type":"typing"}`,
`{"type":"read","last_read_id":n}`, `{"type":"ping"}`. Server → client: `hello` (with members),
`message` (with the message), `typing`, `read`, `presence` (`online: bool`), `pong`, `error`, `closed`.

## Safety

| Method | Path | Body |
|--------|------|------|
| POST | `/v1/blocks` | `{user_id}` Closes any open match. |
| DELETE | `/v1/blocks/:user_id` | |
| POST | `/v1/reports` | `{target_id, reason, details?, match_id?, block?}` reason ∈ fake_profile, harassment, inappropriate_content, underage, spam, other. 10 per day. |

## Photos (needs R2)

| Method | Path | Notes |
|--------|------|-------|
| GET | `/v1/me/photos` | `{photos, max, enabled}` |
| POST | `/v1/me/photos` | Raw JPEG/PNG/WebP body, ≤ 5 MB, up to 6. → `{id, url, position}` |
| DELETE | `/v1/me/photos/:id` | |
| GET | `/v1/photos/:id` | The bytes; any signed-in member. |

## Moderation (`ADMIN_EMAILS` or `users.is_admin`)

| Method | Path | Notes |
|--------|------|-------|
| GET | `/v1/admin/reports?status=open` | Reports with reporter and target cards. |
| POST | `/v1/admin/reports/:id/resolve` | `{action: dismiss\|warn\|suspend\|ban, note?}`. Suspend/ban revoke sessions; ban closes matches. |
| GET | `/v1/admin/users/:id` | User, profile, counts, last 50 events. |
| GET | `/v1/admin/matches/:id/transcript` | Full chat history. Logged as an event. |
