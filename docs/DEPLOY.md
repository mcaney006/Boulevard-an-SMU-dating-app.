# Deploy

Everything runs in the Cloudflare account named **Boulevard** (`CLOUDFLARE_ACCOUNT_ID` in your
shell). The API token lives in the macOS Keychain; `cfenv` loads it.

## Already provisioned

- D1 database `boulevard` (id in `crates/boulevard-worker/wrangler.jsonc`).
- workers.dev subdomain `boulevard` → the Worker answers at `https://boulevard-api.boulevard.workers.dev`.
- Durable Object class `ChatRoom` is declared in the config; the first deploy creates the namespace.

## Deploy

```bash
pnpm migrate:remote                 # apply migrations/ to the remote D1
pnpm deploy                         # builds the wasm (worker-build) and uploads
curl https://boulevard-api.boulevard.workers.dev/v1/health
```

`wrangler` uses `CLOUDFLARE_API_TOKEN` from the environment, so this works in a shell that ran
`cfenv`, and in CI with the token as a secret.

## Configuration

Vars live in `wrangler.jsonc`; secrets are set with `wrangler secret put`.

| Name | Production value | Why |
|------|------------------|-----|
| `DEV_RETURN_OTP` | `false` | Never return codes from the API in production. |
| `STANDINS_ENABLED` | `true` until launch | Scripted deck so the app is not empty on day one. |
| `PLUS_PREVIEW_UNLOCK` | `true` until billing exists | Lets anyone switch Plus on for free. |
| `ADMIN_EMAILS` | your `@smu.edu` addresses, comma-separated | Moderation endpoints. |
| `EMAIL_PROVIDER` | `log` → `resend` | `log` writes codes to `wrangler tail`; `resend` sends mail. |
| `EMAIL_FROM` | a verified sender | Resend needs a verified domain for real recipients. |
| `RESEND_API_KEY` (secret) | | `wrangler secret put RESEND_API_KEY --config crates/boulevard-worker/wrangler.jsonc` |
| `CORS_ORIGINS` | the web app's origin | Only needed for browser clients. |

## Turning on the pieces that need account changes

1. **Verification mail.** Boulevard has no zone in Cloudflare yet, so Cloudflare Email Service
   cannot send. Options: add a domain to the account and use Email Service (a `send_email`
   binding), or create a Resend account, verify a sending domain, and set `EMAIL_PROVIDER=resend`
   plus the secret. Until then, read codes from `pnpm tail`.
2. **Photos.** R2 must be enabled once in the dashboard (it asks for a payment method even for the
   free tier). Then `cf r2 buckets create --name boulevard-photos`, uncomment `r2_buckets` in
   `wrangler.jsonc`, and redeploy.
3. **Custom domain.** Add a zone, then a custom domain route for the Worker.

## Operate

```bash
pnpm tail                                           # live logs, including dev codes when EMAIL_PROVIDER=log
cf d1 query <id> --command "SELECT type, COUNT(*) FROM events GROUP BY 1"
cf durable-objects namespaces list
```

Backups: D1 keeps 30 days of point-in-time history (`cf d1 time-travel`). The event log can rebuild
`matches`, `blocks`, and `reports` if a projection ever needs repair.
