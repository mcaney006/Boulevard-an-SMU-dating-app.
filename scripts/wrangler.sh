#!/usr/bin/env bash
# Run the repo's pinned wrangler with a real Node.js.
# On some machines `node` on PATH is Bun, and wrangler's local proxy cannot forward
# requests to the Worker under Bun (every request answers 503 with an explanation).
set -euo pipefail
cd "$(dirname "$0")/.."
pick_node() {
  local n
  for n in "$( (command -v mise >/dev/null 2>&1 && mise which node) 2>/dev/null || true)" "$(command -v node || true)"; do
    [ -n "$n" ] && [ -x "$n" ] || continue
    if "$n" -e 'process.exit(process.versions.bun ? 1 : 0)' 2>/dev/null; then echo "$n"; return 0; fi
  done
  echo "No real Node.js found (node on PATH is Bun). Install Node 22+ (e.g. mise use node@lts)." >&2
  return 1
}
NODE="$(pick_node)"
exec "$NODE" node_modules/wrangler/bin/wrangler.js "$@"
