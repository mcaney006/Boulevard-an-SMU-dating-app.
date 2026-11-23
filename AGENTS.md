# Working on Boulevard

- The product spec is the README. The published rules are the product; do not add ranking inputs.
- The score, validation, catalogs, and stand-ins live only in `crates/boulevard-core`. The Worker and
  the apps call them; never duplicate a rule in a handler or a view.
- Every state change in the Worker goes into one D1 batch together with its `events` row.
- Run `cargo test -p boulevard-core` and `cargo check -p boulevard-worker --target
  wasm32-unknown-unknown` before committing; run `pnpm smoke` against `pnpm dev` for anything that
  touches a handler.
- Cloudflare: account "Boulevard", Worker `boulevard-api`, D1 `boulevard`. Use `cf` or `wrangler`
  with the token from `cfenv`. Never commit `.dev.vars` or tokens.
- `legacy/tanstack-prototype/` is read-only history.
