# SPA Leptos 0.8 + rust-ui + Tailwind v4 + workspace dependency bump — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate the SPA to Leptos 0.8, replace the faked `<div>` toggle with a real accessible rust-ui Switch, upgrade the SPA to Tailwind v4, and bump every workspace dependency to its latest — one coordinated move that also dissolves the `worker-build@0.8.1` stop-gap pin.

**Architecture:** The workspace is a single Cargo lockfile, so `wasm-bindgen`/`js-sys`/`web-sys` resolve to ONE shared version across `grumps-spa` and `grumps-worker`. The whole plan pivots on landing that shared version at **0.2.122 / 0.3.99 / 0.3.99** — the value that simultaneously satisfies `worker`/`worker-build 0.8.3` (floor `^0.2.121`) and `leptos 0.8` (open `^0.2`). Work proceeds in three shippable phases: (0) the atomic dependency bump + resulting code fixes, (1) Tailwind v3→v4 in the SPA, (2) rust-ui Switch adoption replacing the toggle.

**Tech Stack:** Rust (stable, `x86_64-pc-windows-msvc` for tests, `wasm32-unknown-unknown` for the worker), Leptos 0.8 CSR + Trunk, Cloudflare Workers (`worker` 0.8.3 + `worker-build`), Tailwind CSS v4 (standalone CLI / `@tailwindcss/cli`), rust-ui copy-paste components (`tw_merge`).

## Locked version targets (verified on crates.io 2026-06-01)

| Crate | From | To | Notes |
|---|---|---|---|
| wasm-bindgen | 0.2 (pinned 0.2.118) | **=0.2.122** | the lynchpin; pin exact |
| js-sys | 0.3 | **=0.3.99** | lockstep |
| web-sys | 0.3 | **=0.3.99** | lockstep; keep feature list |
| wasm-bindgen-futures | 0.4 | **=0.4.72** | lockstep |
| worker | 0.8 | **0.8.3** | skip yanked 0.8.2 |
| worker-build (wrangler `[build]`) | pinned 0.8.1 | **0.8.3** | matches worker |
| leptos / leptos_router | 0.7 | **0.8** (0.8.19 / 0.8.13) | `csr` feature |
| jsonwebtoken | 9 | **10** | API breakage — auth path |
| sha2 / hmac | 0.10 / 0.12 | **0.11 / 0.13** | RustCrypto pair — bump together |
| gloo-net / gloo-storage / gloo-timers | 0.6 / 0.3 / 0.3 | **0.7 / 0.4 / 0.4** | recompile + minor churn |
| rusqlite (dev) | 0.32 | **0.40** | tests only |
| reqwest (dev) | 0.12 | **0.13** | tests only; TLS features |
| chrono-tz | 0.10 | **0.10.4** | stay 0.10.x (worker pins `^0.10.3`) |
| serde / serde_json / chrono / uuid / thiserror / async-trait / hex / log / futures / urlencoding / strsim / once_cell / pretty_assertions / serde-wasm-bindgen / console_error_panic_hook / wasm-logger | various | latest in-range | no-op recompiles |
| Tailwind | 3.4.19 | **4.3.0** | standalone binary or `@tailwindcss/cli` |
| rust-ui deps | — | `tw_merge` 0.1.21 | `leptos_ui`/`icons` NOT used (avoid `nightly`) |

## File structure (what changes and why)

- `Cargo.toml` (workspace root) — bump `[workspace.dependencies]` to latest in-range.
- `crates/worker/Cargo.toml` — worker 0.8.3, jsonwebtoken 10, sha2 0.11, hmac 0.13, rusqlite 0.40, reqwest 0.13.
- `crates/messaging/Cargo.toml` — sha2 0.11, hmac 0.13 (paired with worker).
- `crates/spa/Cargo.toml` — leptos/leptos_router 0.8, gloo majors, pinned wasm chain, add `tw_merge`.
- `crates/{agent,core,nlu,memory,scheduler,calendar,i18n}/Cargo.toml` — inherit workspace bumps; `nlu` `strsim`, `i18n` `once_cell` stay latest-in-range (no change needed).
- `wrangler.toml` — `[build]` command: `worker-build@0.8.1` → `0.8.3`; comment updated.
- `crates/worker/src/middleware.rs` — jsonwebtoken 10 call-site fixups (lines ~67-71, 101-102, 128-129, 454-460).
- `crates/spa/input.css` — Tailwind v4: `@import` + `@source` + `@theme inline`; keep existing `@layer` blocks.
- `crates/spa/tailwind.config.js` — deleted (config moves into CSS).
- `crates/spa/Trunk.toml` — pre_build hook uses the v4 CLI.
- `CLAUDE.md` — Tailwind v4 build commands (demo-mode section).
- `crates/spa/src/components/switch.rs` — NEW: accessible controlled Switch primitive.
- `crates/spa/src/components/mod.rs` — export `switch`.
- `crates/spa/src/pages/settings.rs` — `Toggle` reimplemented on `Switch`; call sites unchanged.
- SPA-wide — drop `.as_deref()` on `LocalResource` reads where present (Leptos 0.8).

Commit convention (feature branch): gitmoji, bypassing commitizen:
`SKIP=commitizen git commit --no-verify -m "⬆️ <subject>"`.

Test/build gate commands (used throughout):
- Workspace tests: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc`
- Worker wasm build: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe clippy -p grumps-worker --target wasm32-unknown-unknown`
- Worker deploy dry-run: `MSYS_NO_PATHCONV=1 npx wrangler deploy --dry-run`
- SPA build: `cd crates/spa && MSYS_NO_PATHCONV=1 trunk build --release`

We are already on branch `feat/spa-leptos08-rust-ui`.

---

# Phase 0 — Coordinated dependency bump (atomic; one PR boundary)

This phase MUST land as one unit — the shared wasm-bindgen version couples SPA and worker. End state: everything green on the new versions, pin gone.

### Task 0.1: Bump workspace-root shared dependencies

**Files:**
- Modify: `Cargo.toml:16-22`

- [ ] **Step 1: Edit `[workspace.dependencies]`**

```toml
[workspace.dependencies]
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.150"
chrono = { version = "0.4.44", features = ["serde", "wasmbind"] }
chrono-tz = "0.10.4"
uuid = { version = "1.23", features = ["v4", "js", "serde"] }
thiserror = "2.0.18"
```

- [ ] **Step 2: Resolve the lockfile**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe update`
Expected: lockfile updates within semver; no errors. (Major bumps in later tasks are pulled by editing each crate's manifest, not here.)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
SKIP=commitizen git commit --no-verify -m "⬆️ Bump workspace shared deps to latest in-range"
```

### Task 0.2: Bump the wasm-bindgen chain + leptos 0.8 in the SPA

This is the lynchpin edit. Pin the wasm trio exactly so the worker and SPA cannot drift.

**Files:**
- Modify: `crates/spa/Cargo.toml:8-22`

- [ ] **Step 1: Edit SPA dependencies**

```toml
leptos = { version = "0.8", features = ["csr"] }
leptos_router = "0.8"
gloo-net = { version = "0.7", features = ["http"] }
async-trait = "0.1"
gloo-storage = "0.4"
gloo-timers = { version = "0.4", features = ["futures"] }
futures = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde-wasm-bindgen = "0.6"
urlencoding = "2"
web-sys = { version = "=0.3.99", features = ["Window", "Document", "HtmlDocument", "HtmlInputElement", "Navigator", "Clipboard", "Location", "RequestCredentials"] }
wasm-bindgen = "=0.2.122"
wasm-bindgen-futures = "=0.4.72"
js-sys = "=0.3.99"
console_error_panic_hook = "0.1"
log = "0.4"
wasm-logger = "0.2"
```

- [ ] **Step 2: Force the wasm trio to resolve**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe update -p wasm-bindgen -p js-sys -p web-sys -p wasm-bindgen-futures`
Expected: resolves to 0.2.122 / 0.3.99 / 0.3.99 / 0.4.72. Verify with `~/.cargo/bin/cargo.exe tree -i wasm-bindgen` showing a single 0.2.122.

- [ ] **Step 3: Do NOT build yet** — the worker still references worker 0.8 / old worker-build; build after Task 0.3 so the whole lock is coherent. Proceed.

### Task 0.3: Bump the worker to 0.8.3 and lift the worker-build pin

**Files:**
- Modify: `crates/worker/Cargo.toml:15` and `:22`, `:29`, `:32`
- Modify: `crates/agent/Cargo.toml:15`
- Modify: `wrangler.toml:10-15`

- [ ] **Step 1: Edit `crates/worker/Cargo.toml`**

```toml
worker = { version = "0.8.3", features = ["d1"] }
```
and the JWT + crypto + dev deps:
```toml
jsonwebtoken = { version = "10", default-features = false }
```
```toml
sha2 = "0.11"
hmac = "0.13"
```
```toml
[dev-dependencies]
reqwest = { version = "0.13", features = ["blocking", "json"] }
rusqlite = { version = "0.40", features = ["bundled"] }
```

- [ ] **Step 2: Edit `crates/agent/Cargo.toml:15`** (it also depends on `worker`)

```toml
worker = { version = "0.8.3", features = ["d1"] }
```

- [ ] **Step 3: Edit `wrangler.toml` `[build]` (lines 10-15)** — lift the pin to 0.8.3

```toml
[build]
# worker-build is pinned to match the `worker` 0.8.3 crate. The coordinated
# wasm-bindgen 0.2.122 bump (shared with the SPA's leptos 0.8) satisfies
# worker-build 0.8.3's wasm-bindgen >= 0.2.121 floor, so the old 0.8.1
# stop-gap is gone. Bump this in lockstep when upgrading the worker crate.
command = "cargo install -q --locked worker-build@0.8.3 && cd crates/worker && worker-build --release"
```

- [ ] **Step 4: Resolve and inspect the lock**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe update -p worker`
Expected: `worker 0.8.3`, `worker-sys 0.8.3`. Re-confirm `cargo tree -i wasm-bindgen` is still a single 0.2.122 (worker 0.8.3 floor `^0.2.121` is satisfied).

- [ ] **Step 5: Commit (manifests only; code fixes follow)**

```bash
git add Cargo.lock crates/worker/Cargo.toml crates/agent/Cargo.toml crates/spa/Cargo.toml crates/messaging/Cargo.toml wrangler.toml
SKIP=commitizen git commit --no-verify -m "⬆️ Move wasm-bindgen chain to 0.2.122, worker 0.8.3, leptos 0.8; lift worker-build pin"
```
(`crates/messaging/Cargo.toml` is edited in Task 0.4 — include it here only if already staged; otherwise commit it with 0.4. Prefer committing 0.4 separately.)

### Task 0.4: Bump sha2/hmac in messaging (paired with worker)

**Files:**
- Modify: `crates/messaging/Cargo.toml:12-13`

- [ ] **Step 1: Edit**

```toml
hmac = "0.13"
sha2 = "0.11"
```

- [ ] **Step 2: Build messaging to surface any RustCrypto API drift**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe build -p grumps-messaging --target x86_64-pc-windows-msvc`
Expected: PASS. The code uses `Hmac::<Sha256>::new_from_slice` + `.update()` + `.finalize().into_bytes()` (`crates/messaging/src/whatsapp.rs:8-13,48-50,222-225`) and `worker`/`auth_telegram.rs` uses `Sha256::new()/update()/finalize()` — all stable across the 0.11/0.13 generation. If the compiler flags the finalize array type, change `hex::encode(result)` to `hex::encode(result.as_slice())` (the only anticipated fixup).

- [ ] **Step 3: Run messaging tests**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test -p grumps-messaging --target x86_64-pc-windows-msvc`
Expected: `test_verify_signature_correct/wrong/no_prefix` PASS — proves the WhatsApp HMAC path survived the bump.

- [ ] **Step 4: Commit**

```bash
git add crates/messaging/Cargo.toml
SKIP=commitizen git commit --no-verify -m "⬆️ Bump sha2 0.11 + hmac 0.13 in messaging"
```

### Task 0.5: Fix jsonwebtoken 9 → 10 call sites in the worker

The v10 defaults are stricter (exp validation on by default; algorithms default to `[HS256]`, matching `Header::default()`). The encode/decode + `from_secret` shapes are unchanged; the only anticipated source change is the now-redundant `validate_exp = true` line if the field was removed.

**Files:**
- Modify: `crates/worker/src/middleware.rs:67-71`, `:454-460`

- [ ] **Step 1: Build the worker for wasm to surface jsonwebtoken errors**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe build -p grumps-worker --target wasm32-unknown-unknown`
Expected: either PASS, or an error on the two `validation.validate_exp = true;` lines.

- [ ] **Step 2: If `validate_exp` no longer exists, remove the redundant lines**

In `middleware.rs` `verify_session`/decode path (≈67-71):
```rust
    let key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
    let validation = jsonwebtoken::Validation::default();
    let data = jsonwebtoken::decode::<Claims>(token, &key, &validation)
        .map_err(|e| format!("invalid token: {}", e))?;
    Ok(data.claims)
```
And in `decode_jwt_internal` (≈454-460):
```rust
fn decode_jwt_internal(jwt: &str, secret: &str) -> std::result::Result<Claims, String> {
    let key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
    let validation = jsonwebtoken::Validation::default();
    jsonwebtoken::decode::<Claims>(jwt, &key, &validation)
        .map(|d| d.claims)
        .map_err(|e| format!("{}", e))
}
```
(If the field still exists in v10, leave the original lines and skip this step. `Validation::default()` already enables exp validation, so removing the explicit assignment is behavior-preserving.) `encode(&Header::default(), &claims, &key)` at lines 101-102 and 128-129 needs no change.

- [ ] **Step 3: Rebuild the worker for wasm**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe clippy -p grumps-worker --target wasm32-unknown-unknown`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/worker/src/middleware.rs
SKIP=commitizen git commit --no-verify -m "⬆️ Adapt JWT call sites to jsonwebtoken 10"
```

### Task 0.6: Fix the SPA for Leptos 0.8 (`LocalResource` SendWrapper)

Leptos 0.8's only CSR-facing breaking change: `LocalResource` reads no longer return a `SendWrapper`, so any `.as_deref()` peeling on a resource read is removed. There are 38 `LocalResource` sites.

**Files:**
- Modify: any of the 38 `LocalResource` read sites across `crates/spa/src/pages/*.rs` and `components/sidebar.rs` that use `.as_deref()` / `SendWrapper` peeling.

- [ ] **Step 1: Build the SPA to surface 0.8 breakage**

Run: `cd crates/spa && PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe build --target wasm32-unknown-unknown`
Expected: PASS, or type errors at `LocalResource` `.get()` sites that previously deref'd a `SendWrapper`.

- [ ] **Step 2: For each flagged site, drop the `.as_deref()` / `SendWrapper` peel**

Pattern transform (apply only where the compiler points):
```rust
// before (0.7): peeled the SendWrapper
move || res.get().as_deref().map(|d| view! { ... d.clone() ... })
// after (0.8): direct access
move || res.get().map(|d| view! { ... d ... })
```
If a site already compiles, leave it untouched. Do not change signal/`view!`/router/context code — those are unchanged in 0.8.

- [ ] **Step 3: Rebuild SPA to green**

Run: `cd crates/spa && PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe build --target wasm32-unknown-unknown`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/spa/src
SKIP=commitizen git commit --no-verify -m "⬆️ Adapt SPA to Leptos 0.8 (LocalResource SendWrapper drop)"
```

### Task 0.7: Bump rusqlite/reqwest test surface to green

**Files:**
- Modify (if compiler requires): `crates/worker/tests/sql_rigor.rs`

- [ ] **Step 1: Run the full workspace test suite**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc`
Expected: PASS. rusqlite 0.40 keeps `Connection::open_in_memory`, `execute_batch`, `prepare`, `query_map` for basic usage; the `bundled` feature is retained.

- [ ] **Step 2: If `sql_rigor.rs` fails to compile, fix the flagged rusqlite call sites**

Common 0.32→0.40 fixups (apply only where flagged): `Row::get` index/closure type inference may need explicit turbofish, and `params![]` import path is unchanged (`rusqlite::params`). reqwest 0.13 in `[dev-dependencies]` keeps `blocking`+`json`; if a TLS feature error appears, add `features = ["blocking", "json", "rustls-tls"]`.

- [ ] **Step 3: Re-run tests**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc`
Expected: PASS.

- [ ] **Step 4: Commit (only if files changed)**

```bash
git add crates/worker/tests/sql_rigor.rs
SKIP=commitizen git commit --no-verify -m "⬆️ Adapt worker tests to rusqlite 0.40 / reqwest 0.13"
```

### Task 0.8: Phase-0 green gate (worker deploy dry-run)

- [ ] **Step 1: Worker wasm build + clippy**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe clippy -p grumps-worker --target wasm32-unknown-unknown`
Expected: PASS.

- [ ] **Step 2: wrangler deploy dry-run (proves the lifted pin holds end-to-end)**

Run: `MSYS_NO_PATHCONV=1 npx wrangler deploy --dry-run`
Expected: the `[build]` command installs `worker-build@0.8.3`, builds the worker to wasm with wasm-bindgen 0.2.122, and the dry-run completes with no schema-version mismatch.

- [ ] **Step 3: Full workspace test suite once more**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc`
Expected: PASS.

- [ ] **Step 4: SPA build**

Run: `cd crates/spa && MSYS_NO_PATHCONV=1 trunk build --release`
Expected: PASS (Tailwind still v3 here — that's fine; Phase 1 swaps it). If Trunk reports a wasm-bindgen CLI/crate schema mismatch, run `cargo install -f wasm-bindgen-cli --version 0.2.122` and rebuild.

Phase 0 is complete and independently shippable when all four gates are green.

---

# Phase 1 — Tailwind v3 → v4 (SPA only)

The landing does not use Tailwind, so this is scoped to `crates/spa`. rust-ui (Phase 2) requires v4, so this must precede it.

### Task 1.1: Install the Tailwind v4 CLI

- [ ] **Step 1: Confirm the v4 CLI is reachable via npx**

Run: `npx @tailwindcss/cli@4.3.0 --help`
Expected: prints v4 CLI help. (The standalone `tailwindcss-windows-x64.exe` v4.3.0 from the tailwindlabs release is an alternative; the Trunk hook uses `npx` so we stay on `@tailwindcss/cli`.)

- [ ] **Step 2: No commit** (no repo change yet).

### Task 1.2: Convert `input.css` to v4

**Files:**
- Modify: `crates/spa/input.css:1-3`

- [ ] **Step 1: Replace the three `@tailwind` directives (lines 1-3) with the v4 import + source + theme block**

Replace lines 1-3 only; leave lines 5-99 (`@layer base { ... }` and `@layer utilities { ... }`) exactly as they are.

```css
@import "tailwindcss";

/* v4 auto-detection skips .rs files and gitignored dist/ — declare sources. */
@source "./src/**/*.rs";
@source "./index.html";

/* Expose the existing :root design tokens (defined in @layer base below) as
   Tailwind color/font utilities. @theme inline keeps the var() references so
   inline `style="color: var(--ink)"` usages keep working unchanged. */
@theme inline {
  --color-cream: var(--cream);
  --color-cream-light: var(--cream-light);
  --color-ink: var(--ink);
  --color-brick: var(--brick);
  --color-brick-hover: var(--brick-hover);
  --color-teal: var(--teal);
  --color-teal-light: var(--teal-light);
  --color-ochre: var(--ochre);
  --color-ochre-light: var(--ochre-light);
  --color-warm-gray: var(--warm-gray);
  --color-warm-gray-light: var(--warm-gray-light);

  --font-display: "Bitter", Georgia, serif;
  --font-body: "DM Sans", -apple-system, sans-serif;

  --border-width-grumps: 2px;
}
```

- [ ] **Step 2: No build yet** (the Trunk hook + config deletion come next).

### Task 1.3: Delete the JS config and update the Trunk build hook

**Files:**
- Delete: `crates/spa/tailwind.config.js`
- Modify: `crates/spa/Trunk.toml` (the `pre_build` hook)

- [ ] **Step 1: Delete the JS config**

```bash
git rm crates/spa/tailwind.config.js
```
(The `content`, `colors`, `fontFamily`, and `borderWidth` it held are now in `@theme`/`@source`.)

- [ ] **Step 2: Edit the `pre_build` hook in `Trunk.toml`** to call the v4 CLI

```toml
[[hooks]]
stage = "pre_build"
command = "sh"
command_arguments = ["-c", "npx @tailwindcss/cli -i ./input.css -o ./tailwind.out.css --minify 2>/dev/null || npx @tailwindcss/cli -i ./input.css -o ./tailwind.out.css"]
```

- [ ] **Step 3: Build the SPA and verify Tailwind compiles under v4**

Run: `cd crates/spa && MSYS_NO_PATHCONV=1 trunk build --release`
Expected: PASS; `tailwind.out.css` is generated by the v4 CLI. If utility classes appear missing in the output, confirm the `@source` globs match (`./src/**/*.rs`).

### Task 1.4: Fix v4 utility renames across the SPA

**Files:**
- Modify (only where matches found): `crates/spa/src/**/*.rs`, `crates/spa/index.html`

- [ ] **Step 1: Find renamed/removed v4 utilities**

Use Grep across `crates/spa/src` and `index.html` for these v3 class tokens (whole-word, within `class="..."`):
`shadow-sm`, `shadow ` (bare), `rounded-sm`, `rounded ` (bare), `ring ` (bare), `outline-none`, `bg-opacity-`, `text-opacity-`, `border-opacity-`, `ring-opacity-`, `flex-grow`, `flex-shrink`, `overflow-ellipsis`.

- [ ] **Step 2: Apply the v3→v4 mapping at each hit**

| v3 | v4 |
|---|---|
| `shadow-sm` | `shadow-xs` |
| `shadow` (bare) | `shadow-sm` |
| `rounded-sm` | `rounded-xs` |
| `rounded` (bare) | `rounded-sm` |
| `ring` (bare) | `ring-3` |
| `outline-none` | `outline-hidden` |
| `bg-opacity-N` / `text-opacity-N` | use slash opacity, e.g. `bg-black/50` |
| `flex-grow` / `flex-shrink` | `grow` / `shrink` |
| `overflow-ellipsis` | `text-ellipsis` |

Leave `shadow-lg`, `rounded-full`, `rounded-lg`, `ring-2` etc. unchanged (those names are stable). Note: v4's default border color is now `currentColor` (was gray-200) — the codebase always sets an explicit `border-ink`, so no action; if a bare `border` with no color shows wrong, add `border-ink`.

- [ ] **Step 3: Rebuild + visual smoke**

Run: `cd crates/spa && MSYS_NO_PATHCONV=1 trunk build --release`
Expected: PASS. Then `trunk serve` and eyeball the dashboard, settings, and an `?lang=ar` (RTL) and `?lang=tr` (long strings) view — no broken radii/borders/spacing.

- [ ] **Step 4: Commit Phase 1**

```bash
git add crates/spa/input.css crates/spa/Trunk.toml crates/spa/src crates/spa/index.html
git rm --cached crates/spa/tailwind.config.js 2>/dev/null || true
SKIP=commitizen git commit --no-verify -m "⬆️ Migrate SPA to Tailwind v4 (@theme + @source, v4 CLI)"
```

### Task 1.5: Update CLAUDE.md build commands to v4

**Files:**
- Modify: `CLAUDE.md` (Demo mode section — the `tailwindcss -i ./input.css ...` line)

- [ ] **Step 1: Replace the v3 invocation**

Change:
```bash
tailwindcss -i ./input.css -o ./dist/styles.css --minify
```
to:
```bash
npx @tailwindcss/cli -i ./input.css -o ./dist/styles.css --minify
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
SKIP=commitizen git commit --no-verify -m "📝 Update demo-build Tailwind command to v4 CLI"
```

---

# Phase 2 — rust-ui Switch primitive replacing the faked toggle

We do NOT run `ui init` (it would scaffold shadcn semantic tokens and pull `leptos_ui`, which enables leptos `nightly`). Instead we copy the Switch source, adapt it to be **controlled** (our `Toggle` owns the state), strip the `leptos_ui`/`clx!` dependency, and re-style to warm-brutalism tokens. Only the stable `tw_merge` crate is added.

### Task 2.1: Add the `tw_merge` dependency to the SPA

**Files:**
- Modify: `crates/spa/Cargo.toml`

- [ ] **Step 1: Add under `[dependencies]`**

```toml
tw_merge = { version = "0.1.21", features = ["variant"] }
```

- [ ] **Step 2: Resolve**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe update -p tw_merge`
Expected: `tw_merge 0.1.21` resolves; no leptos `nightly` feature pulled (we are not adding `leptos_ui`).

### Task 2.2: Create the controlled Switch primitive

**Files:**
- Create: `crates/spa/src/components/switch.rs`

- [ ] **Step 1: Write the component**

Vertical centering is handled by flexbox (`items-center`) — no magic-pixel `top:`. Horizontal travel uses a single symmetric translate. The element is a real `<button role="switch">`, so keyboard (Space/Enter) and focus come for free.

```rust
use leptos::prelude::*;
use tw_merge::tw_merge;

/// Accessible, **controlled** switch styled to the Grumps design system.
/// The caller owns the state: `checked` is read reactively and `on_change`
/// fires on activation (click / Space / Enter). Vertical centering is done
/// by `items-center` (no hand-positioned knob).
#[component]
pub fn Switch(
    /// Current on/off state, owned by the parent.
    #[prop(into)] checked: Signal<bool>,
    /// Fired when the user toggles the control.
    on_change: impl Fn() + 'static,
    /// Accessible label (already localized by the caller).
    #[prop(into, optional, default = String::new())] aria_label: String,
    /// Extra classes merged onto the track.
    #[prop(into, optional, default = String::new())] class: String,
) -> impl IntoView {
    let state = move || if checked.get() { "checked" } else { "unchecked" };
    let track = tw_merge!(
        "relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center \
         rounded-full border-2 border-ink px-[3px] transition-colors \
         focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-teal \
         data-[state=checked]:bg-teal data-[state=unchecked]:bg-cream",
        class
    );
    view! {
        <button
            type="button"
            role="switch"
            aria-checked=move || checked.get().to_string()
            aria-label=aria_label
            data-state=state
            class=track
            on:click=move |_| on_change()
        >
            <span
                data-state=state
                class="pointer-events-none block size-4 rounded-full transition-transform \
                       data-[state=checked]:translate-x-[18px] \
                       data-[state=checked]:bg-cream data-[state=unchecked]:bg-ink"
            ></span>
        </button>
    }
}
```

- [ ] **Step 2: No build yet** (needs the module export — next task).

### Task 2.3: Export the switch module

**Files:**
- Modify: `crates/spa/src/components/mod.rs`

- [ ] **Step 1: Add the module declaration** (alongside the existing `pub mod` lines)

```rust
pub mod switch;
```

- [ ] **Step 2: Build the SPA to type-check the primitive**

Run: `cd crates/spa && PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe build --target wasm32-unknown-unknown`
Expected: PASS (the component is defined but not yet used — `#[component]` generates a `pub fn`, so no dead-code error; if a warning appears it's non-fatal).

- [ ] **Step 3: Commit the primitive**

```bash
git add crates/spa/Cargo.toml crates/spa/src/components/switch.rs crates/spa/src/components/mod.rs
SKIP=commitizen git commit --no-verify -m "✨ Add accessible controlled Switch primitive (rust-ui, re-styled)"
```

### Task 2.4: Reimplement `Toggle` on top of `Switch`

The wrapper keeps the exact `label_key` / `desc_key` / `value` / `on_toggle` signature, so the four call sites at `settings.rs:123,124,144-149,150-155` do not change. `ReadSignal<bool>` converts into `Signal<bool>` via `into`, satisfying the `#[prop(into)] checked` prop.

**Files:**
- Modify: `crates/spa/src/pages/settings.rs:238-264`

- [ ] **Step 1: Add the import** near the top of `settings.rs` (with the other `use` lines)

```rust
use crate::components::switch::Switch;
```

- [ ] **Step 2: Replace the `Toggle` component body (lines 238-264)**

```rust
#[component]
fn Toggle(
    label_key: &'static str,
    desc_key: &'static str,
    value: ReadSignal<bool>,
    on_toggle: impl Fn() + 'static,
) -> impl IntoView {
    view! {
        <div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
            <div>
                <div class="font-medium text-sm">{move || tr(label_key)}</div>
                <div class="text-xs" style="color: var(--ink-40);">{move || tr(desc_key)}</div>
            </div>
            <Switch checked=value on_change=on_toggle aria_label=tr(label_key) />
        </div>
    }
}
```

- [ ] **Step 3: Build the SPA**

Run: `cd crates/spa && PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe build --target wasm32-unknown-unknown`
Expected: PASS. The four call sites are unchanged and still type-check.

- [ ] **Step 4: Visual verification (the original bug)**

Run: `cd crates/spa && MSYS_NO_PATHCONV=1 trunk serve` and open `/w/<slug>/settings` (or demo mode).
Expected: the four toggles render as switches with the knob **vertically centered**, teal when on / cream when off, ink knob when off / cream knob when on; Tab focuses the switch and Space toggles it. Check `?lang=ar` mirrors correctly (logical translate) and `?lang=tr` labels don't clip.

- [ ] **Step 5: Commit**

```bash
git add crates/spa/src/pages/settings.rs
SKIP=commitizen git commit --no-verify -m "♻️ Reimplement settings Toggle on the accessible Switch primitive"
```

### Task 2.5: Final full-stack green gate

- [ ] **Step 1: Workspace tests**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc`
Expected: PASS.

- [ ] **Step 2: Worker wasm clippy + deploy dry-run**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe clippy -p grumps-worker --target wasm32-unknown-unknown`
then `MSYS_NO_PATHCONV=1 npx wrangler deploy --dry-run`
Expected: both PASS.

- [ ] **Step 3: SPA release build**

Run: `cd crates/spa && MSYS_NO_PATHCONV=1 trunk build --release`
Expected: PASS.

- [ ] **Step 4: Open the PR**

```bash
git push -u origin feat/spa-leptos08-rust-ui
gh pr create --title "feat(spa): Leptos 0.8 + Tailwind v4 + rust-ui Switch, workspace dep bump" \
  --body "Migrates the SPA to Leptos 0.8, upgrades it to Tailwind v4, replaces the faked div toggle with an accessible rust-ui Switch, and bumps the whole workspace to latest — lifting the worker-build@0.8.1 pin. See docs/superpowers/specs/2026-06-01-spa-components-deps-bump-design.md."
```
**PR title is conventional-commits** (it becomes the squash-commit subject on `main`).

---

## Notes on phase boundaries & merge strategy

- **Phase 0 must be atomic** — the shared wasm-bindgen version means SPA + worker bumps cannot be split across PRs. Merge Phases 0+1+2 together, or merge Phase 0 first (it is independently green) then Phases 1+2 in a follow-up PR. Either way, never merge a state where the SPA wants leptos 0.8's wasm-bindgen floor while the worker manifest still pins the old chain.
- CI auto-deploy (`ci.yml` → `deploy-worker` + pages) runs on merge to `main`; the deploy dry-run in Tasks 0.8/2.5 de-risks it.

## Self-review notes (author)

- **Spec coverage:** Leptos 0.8 (Tasks 0.2, 0.6) ✓; rust-ui Switch replacing the toggle (2.2-2.4) ✓; Tailwind v4 SPA-only (Phase 1) ✓; coordinated dep bump + pin lift (Phase 0) ✓; i18n API preserved in the Toggle wrapper (2.4) ✓; verification gates incl. RTL/tr visual + wrangler dry-run (0.8, 1.4, 2.4, 2.5) ✓; worker-pin fallback documented in the spec (not needed — 0.2.122 satisfies both, confirmed) ✓.
- **Type consistency:** `Switch` prop `checked: Signal<bool>` accepts the `ReadSignal<bool>` `value` via `#[prop(into)]`; `on_change: impl Fn()` matches the `on_toggle` closure passed from call sites; module path `crate::components::switch::Switch` matches the `pub mod switch;` export.
- **No placeholders:** dep-bump "fix" tasks are compiler/test-guided with exact file:line anchors and the single anticipated change shown; this is the correct shape for a version migration rather than fabricated diffs.
</content>
