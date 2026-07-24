# WhatsApp support via WAHA — real group chats

**Status:** planned · **Date:** 2026-07-05 · **Revised:** 2026-07-24

## Revisions (2026-07-24) — corrections after live WAHA deployment

A local WAHA (2026.7.1 CORE, NOWEB engine) was deployed and linked;
checking the spec against the running gateway and the current codebase
surfaced these corrections. They override the sections below where they
conflict.

1. **WAHA webhooks are NOT Cloud-API-shaped.** The "Why WAHA" claim is
   wrong: WAHA's format is its own (`{event, session, payload: {id,
   from, participant, fromMe, body, ...}}`). What is reused is the
   downstream pipeline (handler/NLU/DB), not the parsing. Phase 2's
   full `parse_webhook` stands.
2. **Mention detection cannot rely on `@grumps` text.** WhatsApp
   renders mentions as `@<number>`, never a nickname. Reliable path:
   `mentionedIds` compared against the bot's own JID, passed as config
   (`WAHA_BOT_JID`). WhatsApp's LID migration means participants and
   mentions may use either `...@c.us` or `...@lid` — both forms of the
   bot identity must be configured and compared.
3. **`fromMe: true` events MUST be dropped in `parse_webhook`** or the
   bot replies to its own echoes in an infinite loop.
4. **Replies must target `channel_id` (the chat JID), not
   `sender_id`.** The Meta route's send loop targets the sender, which
   is wrong in groups — a third latent defect to not copy.
5. **Webhook HMAC is SHA-512** (`X-Webhook-Hmac` +
   `X-Webhook-Hmac-Algorithm: sha512`), not the Meta-style
   `sha256=`-prefixed `X-Hub-Signature-256`.
6. **Message-id format consistency must be verified live**: reply-to-
   card detection needs the id returned by `/api/sendText` to equal the
   quote id in later webhooks. Validate with captured traffic before
   freezing the adapter.
7. **`pushName` is not guaranteed** — fall back to the JID's number
   part for `sender_name`.
8. **Decision: DB `platform` string stays `"whatsapp"`.** WAHA is a
   transport; workspaces, messages, dispatch and the SPA keep the
   existing WhatsApp channel. Only the route (`/webhook/waha`), the
   adapter and its secrets are WAHA-named. The `"whatsapp"` arm of
   `messaging_dispatch` is wired to WAHA REST.
9. **"Media for free" needs verification on the CORE tier** before
   Phase 3 is promised (our instance reports `tier: CORE`).
10. **Webhook config is per-session via the API** (`config.webhooks` on
    the session), not a dashboard toggle. Local dev: WAHA runs in
    Docker, so the worker URL is `http://host.docker.internal:8787`.
11. **Test fixtures come from captured real payloads** (live gateway →
    local listener), not hand-written JSON.

## Why this exists

The whole product thesis for Grumps is "an AI agent living in your group
chat", and WhatsApp is where the real groups are (family, friends, work) —
far more than Telegram/Discord for that use case. But the **official
WhatsApp Cloud API structurally forbids what we need**:

- The classic Cloud API is 1-to-1 (business ↔ customer) only. Group
  messages **never arrive** in the webhook — there is no group concept.
- The newer official **Groups API** exists but is unusable for us: max
  **8 participants**, requires an **Official Business Account (OBA)**,
  **invite-only** (the business creates the group, users join by link —
  you cannot add the bot to an *existing* group), and is **not available
  on the free test number**.
- Adding a bot to an existing WhatsApp group is a deliberate Meta
  no — impossible through any official channel.

The only way to get the Telegram-style experience ("add Grumps to the
existing group, it reads and replies, handles voice notes and images") is
an **unofficial gateway** that speaks the WhatsApp Web multi-device
protocol. We choose **WAHA** (self-hosted, now fully free and
open-source as of 2026.6.1) wrapping the Baileys engine.

### Trade-offs accepted

- **Meta ToS violation** → account-ban risk. Our usage profile is the
  *low-risk* end (reactive replies inside a group we were invited to →
  high reply-ratio, in-group contacts, no cold outbound — the opposite of
  what Meta's ban ML targets), but the risk is not zero. Mitigated by a
  dedicated number, warm-up, jittered rate limits, and a residential
  proxy.
- **Protocol fragility** → when Meta changes the protocol it breaks until
  the community patches. Mitigated by WAHA's multiple engines (Baileys
  websocket ↔ browser-based) — switchable without touching our code.

### Why WAHA over the alternatives

Compared against Evolution API, raw Baileys, whatsapp-web.js, and managed
SaaS (Whapi etc.):

- WAHA emits webhooks **shaped like the Meta Cloud API**, so our existing
  `webhook_whatsapp.rs` pipeline is reusable almost verbatim.
- It exposes the **real group JID** (`...@g.us`) as the chat id, which
  makes our multi-workspace model work (it was structurally broken on the
  official API — see "Bugs this fixes").
- **Media included for free** (voice/images) → transcription pipeline →
  our differentiator.
- **Free**, self-hostable on a ~$5/mo VPS, multi-engine for resilience.
- Evolution API does the same but heavier/more complex; its multi-instance
  edge only matters if we run several bot numbers from day one. Raw
  Baileys / whatsapp-web.js mean building session, reconnect, webhooks and
  media download ourselves. Managed SaaS defeats the low-cost goal.

## Bugs this fixes for free

The current Meta-oriented `WhatsAppAdapter` has two latent defects that
the WAHA adapter avoids by design:

1. `channel_id = metadata.phone_number_id` (the **bot's own number**),
   which is identical for every inbound message → *all* WhatsApp traffic
   collapses onto a single workspace. WAHA uses the per-chat JID instead.
2. `is_group = display_phone_number.is_some()` — but that field (the bot's
   number) is present in *every* webhook, so `is_group` is effectively
   always true and DM detection never works (the deferred "#5"). WAHA
   distinguishes DM vs group by the JID suffix (`@g.us`).

## Architecture

```
Existing WhatsApp group (Grumps added like any contact)
   │
   ▼
[Pixel 7 + Simbye eSIM]  ← the bot's "primary" WhatsApp (kept online)
   │  (linked/companion device)
   ▼
[VPS ~$5/mo: WAHA in Docker, behind a residential proxy]
   │  webhook (chatId=JID, text, media, mentions)     ▲ REST /api/sendText
   ▼                                                   │
Cloudflare Worker  /webhook/waha  ──►  handler::handle_message  ──►  agent/NLU/DB
                                                                     [UNCHANGED]
```

Reused as-is: `handler::handle_message`, `parser`, `grumps_agent`, D1/DB
layer, provisioning, RAG ingest, ambient classifier, i18n. New surface:
one adapter + one route + secrets + the external gateway.

## Phase 0 — Number & session (no code)

1. Buy the **Simbye US eSIM** (real number + SMS receive), install as a
   third eSIM profile on the Pixel 7 (only two SIMs can be active at once,
   so temporarily disable one to receive the OTP).
2. Register **WhatsApp** on that number (as a second WhatsApp account on
   the Pixel), receive the OTP.
3. **Warm up 3–7 days**: light, natural manual use before automating —
   avoids the "new number instantly botting" ban trigger.
4. Keep the Pixel **online** (charging, on wifi): the primary must connect
   at least once per ~14 days or WAHA's linked session is logged out.

## Phase 1 — Deploy WAHA (infra, outside the repo)

1. **VPS** (Hetzner/Fly/Railway, ~$5/mo). Docker Compose: `waha` +
   optional `redis` for session persistence.
2. **Engine**: start on **NOWEB** (Baileys, lightweight). Keep the
   browser engine as a fallback if the protocol breaks — no code change on
   our side.
3. **Residential/mobile proxy** in front of WAHA (datacenter IPs are
   flagged by the ban ML; residential survives).
4. **Link the session**: scan the QR from the WAHA dashboard with the
   bot's WhatsApp (Pixel 7).
5. **Configure** in WAHA:
   - Webhook URL → `https://<worker>/webhook/waha`, `message` event
     (plus `message.any` if needed).
   - Webhook **HMAC secret** so the worker can authenticate that inbound
     calls really come from our WAHA (the endpoint is public).
6. **Add Grumps to the test group** like any contact.

## Phase 2 — Adapter + worker route (bulk of the code, all reused downstream)

1. **`crates/messaging/src/waha.rs`** — new `WahaAdapter: MessagingPlatform`:
   - `parse_webhook` maps WAHA JSON → `InboundMessage`:
     - `channel_id = chatId` (the JID; `...@g.us` for a group).
     - `sender_id` = participant JID; `sender_name` = pushName.
     - `is_direct_message = !chatId.ends_with("@g.us")` — real DM
       detection (closes the deferred #5).
     - `is_mention_to_bot` = `@grumps` in text **or** bot JID present in
       WAHA `mentionedIds` — real mention detection.
     - `quoted_message_id` from WAHA's quote field.
   - `verify_signature`: HMAC of the WAHA webhook secret (public endpoint
     → mandatory).
   - `build_send_request`: `POST {WAHA_URL}/api/sendText`
     `{session, chatId, text, reply_to?}` with the API-key header.
   - `handle_verification_challenge`: no-op (WAHA has no Meta challenge).
2. **`crates/worker/src/routes/webhook_waha.rs`** — mirror of
   `webhook_whatsapp.rs` but through `WahaAdapter`: HMAC → parse → KV
   dedup → provision workspace (by JID) → upsert member → `parser::parse`
   → `handler::handle_message` → send loop via WAHA REST →
   `track_bot_message(sent_id, todo_id)`.
3. **`lib.rs`**: register `POST /webhook/waha`.
4. **Secrets** (via `wrangler secret put`, never in `wrangler.toml`):
   `WAHA_URL`, `WAHA_API_KEY`, `WAHA_WEBHOOK_HMAC`.
5. **Out-of-webhook sends** (reminders, agent replies): extend
   `messaging_dispatch` to send to a WhatsApp group via WAHA REST — also
   resolves the leftover cron-WhatsApp tracking TODO.
6. Keep the old Meta `webhook_whatsapp.rs` in place (future official path),
   inactive.

## Phase 3 — Media & voice notes (the differentiator)

1. WAHA delivers media (URL/base64); in `parse_webhook` detect `type`
   (audio/image/document…).
2. **Voice notes**: download the audio → **Gemini transcription** (we
   already have `GEMINI_API_KEY`) → feed the transcript into
   `handle_message` as a normal message.
3. **Images**: download → vision model for extraction ("add this receipt
   to the list").
4. Respect the i18n hard rule: no user-facing strings in source; all bot
   text via `t(locale, …)`.

## Phase 3b — Group onboarding & metadata (WAHA-only capability)

WAHA exposes full group *management* (not just send/receive), which
unblocks the long-standing onboarding TODO at the top of
`crates/messaging/src/whatsapp.rs` ("localised welcome flow …
setChatDescription-equivalent group metadata update"). This is impossible
on the official Meta API and is a genuine parity win with Telegram.

Relevant endpoints (all keyed by the group JID `...@g.us`):

- Set description — `PUT /api/{session}/groups/{groupId}/description`
- Set name/subject — `PUT /api/{session}/groups/{groupId}/subject`
- Create group — `POST /api/{session}/groups`
- Add/remove participants; promote/demote admins; list participants
- Lock info to admins — `PUT .../settings/security/info-admin-only`
- Lock messaging to admins — `PUT .../settings/security/messages-admin-only`

Onboarding flow (mirror the Telegram one where possible):

1. Detect the bot being added to a group (WAHA `group.v2.join` /
   participants-update event).
2. Post a **localised** welcome message (i18n — no hardcoded strings) and
   ask an admin to **promote Grumps to admin**.
3. Once admin, optionally set a localised group description
   (setChatDescription-equivalent).

**Hard constraint:** metadata/participant mutations require Grumps to be a
**group admin** — the API returns `false` otherwise. Unlike Telegram
(where onboarding auto-promotes the first member), WhatsApp has **no
auto-promotion of the bot**: whoever adds Grumps must make it admin
manually. The onboarding UX must guide them through this, and metadata
writes must degrade gracefully (skip + inform) when the bot is not admin.

This phase is optional/incremental — the core agent works without admin
rights; only group-metadata features need them.

## Phase 4 — Anti-ban & robustness

1. Outbound **rate limiting with gaussian jitter** + typing simulation
   (in dispatch or via WAHA config).
2. **Warm-up policy**: gradual volume ramp in the first days.
3. **Session persistence** (Redis/volume) + auto **reconnect**;
   disconnect monitoring.
4. **Alert** if the WAHA session drops (Pixel offline > 14 days, etc.).

## Phase 5 — Testing

1. **Unit** `waha.rs`: parse group (`@g.us` → `channel_id`,
   `is_direct_message=false`), DM, mention via `mentionedIds`, quote,
   HMAC.
2. **Replay**: a `replay-waha.sh` (modelled on `replay-webhook.sh`) that
   forges a signed WAHA webhook → local worker.
3. **Real e2e** in the test group: `@grumps list`, create a todo,
   **reply "done" to the card** (validates the recent card-reply fixes),
   send a **voice note** → transcription → todo.

## Cost estimate

Simbye (number) ~$15 · WAHA VPS ~$5 · residential proxy ~few $ →
**~$20–25/month**.

## Recommended starting point

Phase 2 (adapter `waha.rs` + route) is testable locally via a signed
replay script **without the real infra**, so it can proceed in parallel
while the Simbye/WAHA setup is arranged.

## References

- WAHA — <https://waha.devlike.pro/> (self-hosted, free since 2026.6.1)
- Baileys — <https://github.com/whiskeysockets/Baileys>
- Meta Groups API limits (why the official path fails) —
  <https://developers.facebook.com/documentation/business-messaging/whatsapp/groups>
- Ban-risk overview — <https://blog.kraya-ai.com/whatsapp-automation-ban-risk>
