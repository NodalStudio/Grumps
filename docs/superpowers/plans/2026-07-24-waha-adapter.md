# WAHA WhatsApp Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire real WhatsApp group chats into Grumps through a self-hosted WAHA gateway: new `WahaAdapter`, new `/webhook/waha` route, dispatch arm, replay script — then a live e2e pass over every bot feature.

**Architecture:** WAHA (Docker, NOWEB engine, already running locally on :3000, session linked) POSTs HMAC-SHA512-signed webhooks to the worker. A new pure/sync `WahaAdapter: MessagingPlatform` parses them into `InboundMessage`; the route mirrors `webhook_whatsapp.rs` (dedup → provision → member → parser → handler → send). DB `platform` string stays `"whatsapp"` (decision: WAHA is transport only). Downstream (handler/NLU/agent/D1/RAG/i18n) unchanged.

**Tech Stack:** Rust (wasm32 worker), existing deps only: `hmac 0.13`, `sha2 0.11`, `hex 0.4`, `serde_json`, `chrono`. No new crates.

**Spec:** `docs/superpowers/specs/2026-07-05-whatsapp-waha-groups-design.md` — **read the "Revisions (2026-07-24)" section first**, it overrides the body.

---

## Ground truth from the live gateway (captured 2026-07-24)

Real webhook envelope (HMAC verified against raw bytes):

```json
{
  "event": "message.any",
  "session": "default",
  "me": { "id": "10000000001@c.us", "pushName": "Bot", "lid": "111111111111111@lid" },
  "engine": "NOWEB",
  "timestamp": 1784884628000,
  "payload": {
    "id": "true_111111111111111@lid_A55836D391A4D69FAE65489C8D04C52D",
    "from": "111111111111111@lid",
    "fromMe": true,
    "source": "app",
    "body": "text here",
    "timestamp": 1784884627,
    "participant": null,
    "hasMedia": false,
    "media": null,
    "replyTo": null,
    "_data": { "key": { "remoteJid": "111111111111111@lid", "remoteJidAlt": "10000000001@s.whatsapp.net", "fromMe": true, "id": "A55836D391A4D69FAE65489C8D04C52D", "participant": "", "addressingMode": "lid" }, "pushName": "Bot" }
  }
}
```

Verified facts:
- Headers: `X-Webhook-Hmac` (hex), `X-Webhook-Hmac-Algorithm: sha512`, `X-Webhook-Timestamp`, `X-Webhook-Request-Id`. HMAC-SHA512 of the **raw body bytes**, hex-encoded, **no prefix**.
- `payload.id` is serialized `{fromMe}_{chatJid}_{rawId}`; `POST /api/sendText` response returns the **raw** id in `key.id` (e.g. `3EB0EE5AB4F5577CF92858`).
- JIDs arrive in `@lid` OR `@c.us` form; `me` carries both (`id`, `lid`). Groups end `@g.us`.
- Media works on CORE tier: `payload.media.url` = `http://localhost:3000/api/files/default/<id>.oga`, `mimetype` present. (Media handling is a follow-up phase — this plan only parses `hasMedia` gracefully.)
- Group messages: `payload.from` = group JID `...@g.us`, `payload.participant` = sender JID.
- Mentions: `payload.mentionedIds` (list of JIDs) when present; body text contains `@<number>`, never `@grumps` — text matching on `@grumps` kept only as a bonus, JID match is the real path.
- Quotes: `payload.replyTo` = `{ id, participant, body }` (id in serialized or raw form — normalize, see Task 2).

Local wiring: worker `wrangler dev` on :8787; WAHA reaches it at `http://host.docker.internal:8787/webhook/waha`.

## Existing interfaces to code against (do not redesign)

- Trait `MessagingPlatform` — `crates/messaging/src/adapter.rs:43-56`. Sync, pure. `build_send_request` returns `(url, json_body)`; headers set by caller.
- `InboundMessage` / `OutboundMessage` — `adapter.rs:6-41`.
- Reference impls: `crates/messaging/src/whatsapp.rs` (HMAC pattern — but sha256/meta-prefixed; WAHA differs), `crates/messaging/src/telegram.rs` (no-challenge pattern, reply-to-bot deliberately NOT a mention — resolved worker-side via `is_bot_message`).
- Route model: `crates/worker/src/routes/webhook_whatsapp.rs` (full flow), registration `crates/worker/src/lib.rs:59-71`, module list `routes/mod.rs`.
- Dispatch: `crates/worker/src/messaging_dispatch.rs:24-33` — `"whatsapp"` arm currently errors "not wired"; this plan wires it to WAHA.
- Test style: inline `#[cfg(test)] mod tests` at file bottom, `serde_json::json!` fixtures, helpers `adapter()` / `make_sig()` (see `whatsapp.rs:206-467`).
- Native test invocation on this machine (macOS arm64): `cargo test -p <crate> --lib --target aarch64-apple-darwin` (workspace default target is wasm32 via `.cargo/config.toml`; the justfile's linux-gnu target is CI-only).

---

### Task 1: `WahaAdapter` + exhaustive unit tests

**Files:**
- Create: `crates/messaging/src/waha.rs`
- Modify: `crates/messaging/src/lib.rs` (add `pub mod waha;`)

Struct:

```rust
pub struct WahaAdapter {
    pub base_url: String,     // e.g. http://localhost:3000 (no trailing slash; trim it in new())
    pub session: String,      // e.g. "default"
    pub api_key: String,      // X-Api-Key header value (set by caller)
    pub webhook_hmac: String, // HMAC-SHA512 key for inbound verification
}
impl WahaAdapter { pub fn new(base_url, session, api_key, webhook_hmac) -> Self }
```

Behavior contract (TDD each bullet):
- `platform_id()` → `"whatsapp"` (DB platform decision; transport naming stays in route/module names).
- `verify_signature(payload, signature)`: HMAC-SHA512(webhook_hmac, raw bytes), hex, no prefix, constant-time via `mac.verify_slice(&hex::decode(sig)?)`. `type HmacSha512 = Hmac<Sha512>;`.
- `parse_webhook`:
  - Accept `event` values `"message"` and `"message.any"`; anything else → `Ok(None)`.
  - **`payload.fromMe == true` → `Ok(None)`** (echo loop guard — the single most important line).
  - `channel_id = payload.from`; `is_direct_message = !from.ends_with("@g.us")`.
  - `sender_id`: group → `payload.participant`; DM → `payload.from`. Missing participant in a group → `Ok(None)` (system events).
  - `sender_name`: `payload._data.pushName` → else digits before `@` of sender_id.
  - `message_id = payload.id` (serialized form, as-is — dedup key and reply tracking use it).
  - `text = payload.body` (empty string → `None`); `hasMedia && body empty` → `None` text (media phase later).
  - Mention: true iff any of `payload.mentionedIds[]` equals `me.id` or `me.lid` (compare full JID strings), OR body lowercased contains `@grumps`. `is_mention_to_bot = mention || is_direct_message` (match telegram.rs convention).
  - Quote: `payload.replyTo` → `quoted_message_id = replyTo.id` **normalized to raw form** (if it contains `_`, take the last `_`-segment; the route stores raw ids — see Task 2), `quoted_message_text = replyTo.body`.
  - `timestamp`: numeric epoch seconds via `chrono::DateTime::from_timestamp`.
  - Reply-to-bot is NOT a mention here (telegram.rs precedent — worker resolves via `is_bot_message`).
- `build_send_request(recipient, message)` → `(format!("{}/api/sendText", base_url), body)` with body `{"session": self.session, "chatId": recipient, "text": message.text}` + `"reply_to": message.reply_to` when `Some`.
- `handle_verification_challenge` → `Err(MessagingError::VerificationFailed("waha has no challenge".into()))` (telegram.rs pattern).

Steps:
- [ ] **Step 1:** Write the test module first: fixtures as `json!` builders mirroring the captured envelope above (helper `waha_payload(event, from, participant, body, from_me, mentioned, reply_to)`); `make_sig(key, payload)` computing hex SHA-512. Tests (≥16): sig ok / bad key / bad hex / tampered body; parse group text (channel/sender/name mapping); parse DM; fromMe dropped; event `session.status` dropped; mention via mentionedIds ==me.id; mention via me.lid; `@grumps` text bonus; no false mention on plain text; group without participant dropped; quote id normalization (serialized→raw, already-raw passthrough) + quoted text; pushName fallback to number; empty body → text None; hasMedia empty-body → text None; timestamp mapping; build_send_request with/without reply_to (assert exact JSON); challenge → Err; platform_id.
- [ ] **Step 2:** `cargo test -p grumps-messaging --lib --target aarch64-apple-darwin` → all new tests FAIL to compile (module absent). Add `pub mod waha;` + skeleton; tests fail on behavior.
- [ ] **Step 3:** Implement until green.
- [ ] **Step 4:** `cargo fmt --all` then full crate test green.
- [ ] **Step 5:** Commit: `SKIP=commitizen git commit --no-verify -m "✨ Add WAHA messaging adapter with signed-webhook parsing"`

### Task 2: worker route `/webhook/waha` + dispatch arm

**Files:**
- Create: `crates/worker/src/routes/webhook_waha.rs`
- Modify: `crates/worker/src/routes/mod.rs` (add module), `crates/worker/src/lib.rs` (register `.post_async("/webhook/waha", routes::webhook_waha::handle_incoming)` next to the other webhooks — no GET verify route), `crates/worker/src/messaging_dispatch.rs` (wire `"whatsapp"` arm)

`handle_incoming` mirrors `webhook_whatsapp.rs:20-283` step-for-step with these deltas:
1. Adapter: `build_adapter(env)` reading secrets `WAHA_URL`, `WAHA_SESSION` (var, default `"default"`), `WAHA_API_KEY`, `WAHA_WEBHOOK_HMAC` (all via `env.secret()` except session).
2. Signature header: `X-Webhook-Hmac` (mandatory → missing = `Error::RustError`, same hard-fail comment as whatsapp route).
3. Dedup key: `msg:whatsapp:{message_id}` (platform string is whatsapp; serialized ids are unique).
4. Workspace lookup/provision: platform `"whatsapp"`, `channel_id` = JID — per-group workspaces now work.
5. observability/log_inbound, insert_message, RAG meta: platform `"whatsapp"`.
6. **Send loop targets `inbound.channel_id` (chat JID), NOT `sender_id`.** Headers: `X-Api-Key: {api_key}` + `Content-Type: application/json` (no bearer).
7. Sent-id tracking: response JSON `key.id` is the **raw** id → `ws_db.track_bot_message(raw_id, todo_id)`. (Adapter normalizes inbound quote ids to raw — formats meet.)
8. No `handle_verify` function at all.

Dispatch (`messaging_dispatch.rs`): replace the `"whatsapp"` error arm with a WAHA send mirroring the telegram arm: build adapter from env, `build_send_request(channel_id, out)`, POST with `X-Api-Key`, return sent raw id from `key.id`.

Steps:
- [ ] **Step 1:** Write the route + registrations + dispatch arm (no unit tests here — wasm-only code path; covered by replay + e2e).
- [ ] **Step 2:** `cargo clippy -p grumps-worker --target wasm32-unknown-unknown` clean; `cargo fmt --all --check`.
- [ ] **Step 3:** Add secrets to `.dev.vars` (values already in `.env` at repo root — copy `WAHA_URL` as `http://localhost:3000`, `WAHA_API_KEY`, `WAHA_WEBHOOK_HMAC`; add `WAHA_SESSION=default`). **Never commit values.**
- [ ] **Step 4:** Commit: `SKIP=commitizen git commit --no-verify -m "✨ Route WAHA webhooks into the message pipeline"`

### Task 3: `replay-waha.sh`

**Files:** Create `replay-waha.sh` (mode of `replay-webhook.sh`), Modify `justfile` (recipe `replay-waha text="" dm=""`).

- [ ] **Step 1:** Script forges a WAHA envelope (group JID `120363000000000001@g.us`, participant `10000000002@c.us`, mentionedIds containing the me.id when `--mention`), signs it HMAC-SHA512 with `WAHA_WEBHOOK_HMAC` from `.dev.vars`, POSTs to `http://localhost:8787/webhook/waha`. Flags: `--dm`, `--mention`, `--reply-to <id>`.
- [ ] **Step 2:** With `npx wrangler dev` running: `./replay-waha.sh "@grumps aide" --mention` → 200 "ok" and worker logs show pipeline.
- [ ] **Step 3:** Commit: `SKIP=commitizen git commit --no-verify -m "🔧 Add signed WAHA webhook replay script"`

### Task 4: live wiring + full-feature e2e matrix (with the operator)

Not delegated — run interactively.

- [ ] **Step 1:** Point the WAHA session webhook at the worker: `PUT /api/sessions/default` with `url: http://host.docker.internal:8787/webhook/waha`, events `["message"]`, same hmac key. Start `npx wrangler dev`.
- [ ] **Step 2:** Feature matrix in a real test group (each row: send → expected reply → checked):

| # | Feature | Message | Expect |
|---|---|---|---|
| 1 | Onboarding/provision | first group message | workspace auto-provisioned, sender = admin |
| 2 | Todo create | `TODO: acheter du pain` | localised task card |
| 3 | List | `@grumps liste` | todo list |
| 4 | Reply-done | reply "done" on the card | todo completed (validates id normalization) |
| 5 | Note | `NOTE: code wifi 1234` | note stored |
| 6 | Reminder | `REMIND: 20h sortir poubelles` | scheduled action + cron fire via dispatch arm |
| 7 | Agent chat | `@grumps c'est quoi la prochaine échéance ?` | agent answer with RAG context |
| 8 | Mention detection | @-mention via contact picker | bot replies (mentionedIds path) |
| 9 | DM | DM the bot number | DM workspace + reply |
| 10 | Locale | messages in French | replies in French (Gemini locale hint) |
| 11 | Dedup | replay same webhook | no double reply |
| 12 | fromMe guard | bot's own reply echo | no self-loop |
| 13 | Ambient/memory | natural message with a fact, auto_memory on | memory captured |

- [ ] **Step 3:** Fix whatever the matrix breaks; re-run failed rows.
- [ ] **Step 4:** `just check` green (CI parity: fmt + clippy native/wasm + tests).
- [ ] **Step 5:** Squash-merge to main as `feat(whatsapp): route real group chats through the WAHA gateway`.

### Follow-up (separate plan, do NOT start): Phase 3 media/voice
Voice-note transcription via `media.url` download (X-Api-Key) → Gemini; image vision. Media serving verified working on CORE.
