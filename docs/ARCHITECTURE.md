# Architecture

## Shape

```
iOS (SwiftUI) ─┐                       ┌─ D1 "boulevard" (SQLite)
Android (Compose) ─┼─ HTTPS/WSS ─▶ Worker ─┼─ Durable Object ChatRoom (one per match)
web (later) ───┘   boulevard-api        └─ R2 "boulevard-photos" (optional binding)
        │
        └─ boulevard-core via UniFFI: same score, same validation, same catalogs
```

One Worker handles every route. It authenticates, validates with `boulevard-core`, writes to D1
in a batch (D1 batches are transactional), appends to `events`, and answers. Chat is delegated to a
Durable Object keyed by the match id, so each conversation is single-threaded and strongly
consistent without a Redis or a socket fleet.

## The published score

`boulevard_core::score::score(me, them, reveal_like)` is the only ranking code. It returns the total
and the reasons shown on the card. The deck sorts by score, then most recent activity, then earliest
member, then id: deterministic and documented in `/v1/rules`. Filters remove cards; they never
reorder them. Boulevard+ changes exactly two things: `reveal_like` becomes true, and house/major
filters are allowed.

## Verification

- `normalize_email` accepts only `name@smu.edu`, lower-cased, without `+` tags.
- A code is six digits, stored as `sha256(id:email:code)`, valid 10 minutes, five attempts, single use.
- Codes are rate limited (5 per address, 20 per IP, per hour) through `rate_limits`, a fixed-window
  counter in D1 updated with one `INSERT … ON CONFLICT … RETURNING`.
- Sessions are 32 random bytes; D1 keeps only the SHA-256. Suspended and banned users are refused at
  `require()`, so a moderation action takes effect on their next request.

## Data

D1 tables: `users`, `profiles`, `user_filters`, `otp_codes`, `sessions`, `swipes`, `matches`,
`blocks`, `reports`, `photos`, `events`, `rate_limits`. `events` is append-only (triggers refuse
UPDATE and DELETE). Every handler that changes state puts its event in the same batch as the change.

Match rows store the pair in canonical order (`user_a < user_b`) with a unique index, so a mutual
like from either side lands on the same row. The swipe, its event, the match, and the match event
are one batch.

Stand-ins live in code (`boulevard_core::standins`). Swipes on them use `standin:<id>` as the target
and matches with them work like any other, except the other side never types.

## Chat

`ChatRoom` is a SQLite-backed Durable Object (`new_sqlite_classes`). Tables: `messages`, `members`,
`meta`. The Worker verifies membership against D1, then forwards with an `x-user` header. The DO:

- persists first, then broadcasts to every hibernating socket (`accept_websocket_with_tags`, with
  the user id attached to the socket so wake-ups know who is talking);
- rate limits to 30 messages a minute per member by counting its own rows;
- mirrors the last message onto `matches` and appends a `message.sent` event (length and hash, not
  the body) to D1 so the audit trail stays complete for messages that never touch the Worker;
- keeps the transcript after a match closes, for moderation; `/purge` runs only on account deletion.

## Safety

Blocks close the open match and hide both members from each other's deck. Reports are rate limited
and land in a queue; resolving one can warn, suspend (sessions revoked), or ban (sessions revoked,
matches closed). Deleting an account wipes profile, swipes, photos, and filters, closes matches,
revokes sessions, and keeps a tombstone row with the email hash.

## Photos

The `PHOTOS` R2 binding is optional. When it is missing, photo routes answer 503 `photos_disabled`
and cards simply have no photos. Uploads are sniffed by magic bytes (JPEG, PNG, WebP), capped at
5 MB and six per member, and served only to signed-in members.

## Limits that matter at SMU scale

Workers Free: 100k requests a day, D1 5 GB, Durable Objects with SQLite included. Queues are not
used (paid plan). The Worker bundle is under 1 MB. Nothing here needs the paid plan until the campus
is well past a few thousand daily actives.
