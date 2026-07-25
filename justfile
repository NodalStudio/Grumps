# Grumps task runner. Every recipe wraps a command already verified to work
# in .github/workflows/ci.yml, crates/spa/Trunk.toml, wrangler.toml,
# replay-webhook.sh, or landing/build.mjs — nothing here is invented.
#
# `just` itself is assumed already installed. Run `just` with no arguments
# to list every recipe.

default:
    @just --list

# ── Local dev ────────────────────────────────────────────────────────────

# Warn (don't fail) if .dev.vars is missing — wrangler dev / the webhook
# scripts need it but this repo has no way to generate it automatically.
_check-devvars:
    #!/usr/bin/env bash
    if [ ! -f .dev.vars ]; then
      echo "WARNING: .dev.vars not found at repo root." >&2
      echo "  wrangler dev / replay-webhook.sh / test-webhook.sh need it" >&2
      echo "  (TG_BOT_TOKEN, TG_WEBHOOK_SECRET, WA_APP_SECRET, JWT_SECRET," >&2
      echo "  ANTHROPIC_API_KEY, GEMINI_API_KEY, CF_API_TOKEN, CF_ACCOUNT_ID)." >&2
      echo "  Continuing anyway." >&2
    fi

# Install root npm deps (tailwindcss/@tailwindcss/cli, wrangler) so Trunk's
# pre_build hook can resolve `@import "tailwindcss"` in crates/spa/input.css.
setup:
    npm install
    just _check-devvars

# Terminal 1: the worker. Proxied by trunk from :8080 to :8787.
dev-worker: _check-devvars
    npx wrangler dev

# Terminal 2: the SPA. Trunk.toml's own pre/post-build hooks handle Tailwind
# and static-asset copying on every rebuild — this IS the local dev build
# path, distinct from the release-mode Pages bundles built below.
dev-spa:
    cd crates/spa && trunk serve

# Demo mode bypasses auth entirely and shows seeded data (crates/spa/src/demo.rs)
# — dev-worker isn't even required since demo mode short-circuits API calls
# client-side. Only echoes the URL: no cross-platform "open browser" command.
demo:
    @echo "Open http://localhost:8080/dashboard?demo=1 (needs: just dev-spa running)"

# ./replay-webhook.sh tg "@grumps list" | ./replay-webhook.sh tg "list" --dm
# ./replay-webhook.sh wa "TODO: buy bread" | ./replay-webhook.sh health
#
# Named params (not a *args catch-all) because `text` must survive as ONE
# shell word — just space-joins variadic args before substitution, which
# would re-split a quoted multi-word string once the recipe body hits the
# shell. text/dm default to "" so `just replay health` also works untouched
# (the script's health branch ignores $2/$3 entirely).
replay platform text="" dm="": _check-devvars
    ./replay-webhook.sh {{platform}} "{{text}}" {{dm}}

# ./replay-waha.sh "TODO: buy bread" | ./replay-waha.sh "list" --dm
# ./replay-waha.sh "@grumps aide" --mention | ./replay-waha.sh health
#
# Named params (not a *args catch-all) — same reasoning as `replay` above:
# `text` must survive as ONE shell word. dm/mention/reply_to default to ""
# so `just replay-waha health` also works untouched. Pass reply_to as the
# full "--reply-to <id>" string (two words) — just substitutes it unquoted
# here, so the shell re-splits it into the two args the script expects.
replay-waha text="" dm="" mention="" reply_to="": _check-devvars
    ./replay-waha.sh "{{text}}" {{dm}} {{mention}} {{reply_to}}

# Fixed-sequence WhatsApp smoke test (TODO block, note, list, dedup, help...).
test-webhook: _check-devvars
    ./test-webhook.sh

# ── Build & test (mirrors .github/workflows/ci.yml) ─────────────────────

fmt:
    cargo fmt --all --check

# .cargo/config.toml pins the workspace default target to wasm32-unknown-unknown,
# so native checks must override it explicitly or dev-only native deps
# (e.g. rusqlite) fail to compile.
clippy-native:
    cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu

# worker + spa are wasm-only (Cloudflare Workers / browser) — checked
# separately, each pinned to the wasm32 target.
clippy-wasm:
    cargo clippy -p grumps-worker --target wasm32-unknown-unknown
    cargo clippy -p grumps-spa --target wasm32-unknown-unknown

clippy: clippy-native clippy-wasm

# --doc can't combine with --lib/--tests in one invocation (CI splits them too).
test:
    cargo test --workspace --lib --tests --target x86_64-unknown-linux-gnu
    cargo test --workspace --doc --target x86_64-unknown-linux-gnu

check: fmt clippy test

# What `wrangler dev`/`wrangler deploy` already run internally via
# wrangler.toml's [build] command — useful standalone for a quick
# "does the worker compile to wasm cleanly" check.
build-worker:
    cargo install -q --locked worker-build@0.8.3
    cd crates/worker && worker-build --release

# The two SPA bundles below are NOT interchangeable via a flag: build-spa-real
# wipes dist/ first, build-spa-demo doesn't. build-pages relies on that
# ordering (demo bundle built and copied out BEFORE the real build's `rm -rf`).

build-spa-demo:
    #!/usr/bin/env bash
    set -euo pipefail
    cd crates/spa
    mkdir -p dist
    npx @tailwindcss/cli -i ./input.css -o ./dist/styles.css --minify
    trunk build --release --public-url /demo/

build-spa-real:
    #!/usr/bin/env bash
    set -euo pipefail
    cd crates/spa
    rm -rf dist
    mkdir -p dist
    npx @tailwindcss/cli -i ./input.css -o ./dist/styles.css --minify
    trunk build --release --public-url /

# Auto-detects crates/spa/dist/ (populated by build-spa-demo) and copies it
# into the root-level dist/demo/.
build-landing:
    CANONICAL_BASE=https://grumps.app SITE_PATH="" node landing/build.mjs

# Full GitHub Pages artifact (dist/) — order matters, see comment above.
# This only builds the artifact; publishing happens via actions/deploy-pages@v4
# on push to main, which has no local CLI equivalent.
build-pages: build-spa-demo build-landing build-spa-real
    #!/usr/bin/env bash
    set -euo pipefail
    mv crates/spa/dist/index.html dist/404.html
    cp -r crates/spa/dist/. dist/
    if [ -f dist/demo/index.html ]; then
      cp dist/demo/index.html dist/demo/404.html
    fi

# ── Migrations ───────────────────────────────────────────────────────────

migrate-index-local:
    npx wrangler d1 migrations apply grumps-index --local

# Other fixtures: scripts/seed-test-arc1.sql, seed-test-dm1.sql, seed-test-grp1.sql
seed-local file="scripts/seed-local-dev.sql":
    npx wrangler d1 execute grumps-index --local --file={{file}}

migrate-index-remote:
    npx wrangler d1 migrations apply grumps-index --remote

# Workspace-tier migrations only run via the worker's own runtime runner,
# triggered through this secret-gated endpoint. env_var() aborts with a
# clear error before the curl runs if MIGRATE_SECRET isn't set — never
# hardcode it. Usage: MIGRATE_SECRET=... just migrate-workspaces-remote
migrate-workspaces-remote:
    curl -fsS -X POST https://api.grumps.app/internal/migrate-workspaces -H "X-Migrate-Secret: {{env_var("MIGRATE_SECRET")}}"

# ── Deploy ───────────────────────────────────────────────────────────────

# Mirrors CI's deploy-worker job: deploy, apply index migrations remotely,
# backfill workspace DBs. Gated on `check` as a solo-maintainer safety net
# (drop the dependency and run `just check` manually first if that's too
# slow for a quick redeploy). Requires MIGRATE_SECRET in the shell env.
deploy: check
    npx wrangler deploy
    npx wrangler d1 migrations apply grumps-index --remote
    just migrate-workspaces-remote

# ── E2E (Playwright, tests/) ──────────────────────────────────────────────

# npm install (not npm ci): tests/package-lock.json is gitignored.
test-e2e:
    #!/usr/bin/env bash
    set -euo pipefail
    cd tests
    npm install
    npx playwright test

test-e2e-headed:
    cd tests && npm run test:headed

test-e2e-ui:
    cd tests && npm run test:ui

test-e2e-report:
    cd tests && npm run report
