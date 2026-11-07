# Boulevard

Verified SMU dating app. The order you see people in is a published score. Nothing else moves it.

## The product

- Sign in with your `@smu.edu` address and a six-digit code. No other domain gets in.
- A profile: name, year, major, house, intent, who you want to see, a bio.
- The deck is ranked by the published score, and every card shows why:
  same intent +40, same major +25, same house +20 (both in a house), year within one step +15.
- "Already liked you" (+15) is visible before you swipe only on Boulevard+.
- Free: 8 likes a day (Chicago calendar day), intent and year filters, chat after a mutual.
- Boulevard+ ($12 / month): unlimited likes, see who liked you with their note, house and major
  filters. The preview unlock in this build does not charge a card; billing is not wired yet.
- Chats open only after a mutual like. If either side left a note with the like, it is the first message.
- Block, report, delete your account, from inside the app. Moderators get a queue and a transcript.
- A deck of eight fictional stand-in students keeps the product usable before the campus is live.
  They are labeled as scripted and never reply.

The rules are served verbatim at `GET /v1/rules` and executed by one function,
`boulevard_core::score`, that both the API and the mobile apps call.

## The stack

Rust end to end on Cloudflare, nothing to babysit:

| Layer | Choice |
|-------|--------|
| API | Rust on Cloudflare Workers (`workers-rs`), compiled to WebAssembly |
| Data | D1 (SQLite): users, profiles, swipes, matches, reports, an append-only `events` log |
| Real time | One SQLite-backed Durable Object per match: history, hibernating WebSockets, typing, read receipts |
| Photos | R2, behind a binding that can be absent (endpoints answer 503 until R2 is enabled) |
| Verification | One-time codes to `@smu.edu`, hashed at rest, rate limited per address and IP |
| Shared core | `boulevard-core`: types, catalogs, the score, validation, the stand-ins; UniFFI for Swift and Kotlin |

Why not the usual React Native + Node + Firebase: weaker identity guarantees, no audit trail,
and a matching rule that lives in three places. Here every swipe, match, message, report, and
moderation action is an event row, the rule lives once, and the whole thing runs on the Free plan
at SMU scale (about 12k students).

## Repository

```
crates/boulevard-core/     domain core (pure Rust, tested natively, UniFFI behind `ffi`)
crates/boulevard-worker/   the Cloudflare Worker, wrangler.jsonc, ChatRoom Durable Object
migrations/                D1 migrations
scripts/smoke.sh           end-to-end test that walks the product against a running API
docs/                      API reference, architecture, deploy runbook, mobile plan
legacy/tanstack-prototype/ the TypeScript/Postgres prototype this build replaced (spec only)
```

## Run it locally

Prerequisites: Rust stable with the `wasm32-unknown-unknown` target, `worker-build`
(`cargo install worker-build`), Node 22+, pnpm.

```bash
pnpm install
cp crates/boulevard-worker/.dev.vars.example crates/boulevard-worker/.dev.vars
pnpm migrate:local
pnpm dev                       # http://localhost:8787
pnpm smoke                     # in another shell: 50 checks across the whole flow
```

With `DEV_RETURN_OTP=true` the sign-in code comes back in the `/v1/auth/start` response, so
nothing needs an inbox locally.

## Tests

```bash
cargo test -p boulevard-core                          # the rules, validation, DST math
cargo check -p boulevard-worker --target wasm32-unknown-unknown
pnpm smoke                                            # against a running dev server
```

## Deploy

See [docs/DEPLOY.md](docs/DEPLOY.md). Short version: `pnpm migrate:remote && pnpm deploy`.
The production Worker lives at `https://boulevard-api.boulevard.workers.dev`.

## Status

Built and verified in this repository:

- The full API ([docs/API.md](docs/API.md)) with a passing end-to-end smoke test.
- Durable Object chat with live WebSocket delivery.
- The shared Rust core with the UniFFI surface compiling.

Not done yet, in order of what unblocks real users:

1. A sending domain for verification mail. Until then codes go to the Worker log
   (`EMAIL_PROVIDER=log`); set `EMAIL_PROVIDER=resend` and `RESEND_API_KEY` to send for real.
2. R2 for photos: enable R2 once in the Cloudflare dashboard, create the bucket, uncomment the binding.
3. Billing for Boulevard+ (App Store / Play in-app purchase, or Stripe for web). The preview
   unlock is gated by `PLUS_PREVIEW_UNLOCK`.
4. The native apps. The core already exposes its API to Swift and Kotlin; see [docs/MOBILE.md](docs/MOBILE.md).
