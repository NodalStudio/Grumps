# Grumps — Deployment Guide

## Prerequisites

- [Cloudflare account](https://dash.cloudflare.com/) (free tier works)
- [Meta Developer account](https://developers.facebook.com/) + WhatsApp Business API access
- [Stripe account](https://stripe.com/) (for billing)
- [Google AI Studio](https://aistudio.google.com/) account (Gemini API key)
- [Anthropic Console](https://console.anthropic.com/) account (Claude API key)
- Node.js 18+ (for wrangler CLI)
- Rust toolchain with `wasm32-unknown-unknown` target
- `wrangler` CLI: `npm install -g wrangler`
- `trunk` CLI: `cargo install trunk`

---

## Step 1: Cloudflare Setup

### 1.1 Login to Wrangler

```bash
wrangler login
```

### 1.2 Get your Account ID

```bash
wrangler whoami
```

Copy your **Account ID** — you'll need it for the D1 REST API.

### 1.3 Create the Index D1 Database

```bash
wrangler d1 create grumps-index
```

Output will show something like:

```
✅ Successfully created DB 'grumps-index'
database_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
```

Copy the `database_id` and update `wrangler.toml`:

```toml
[[d1_databases]]
binding = "INDEX_DB"
database_name = "grumps-index"
database_id = "PASTE_HERE"
```

### 1.4 Apply Index DB Schema

```bash
wrangler d1 execute grumps-index --file=migrations/index/0001_init.sql
```

### 1.4.1 Index DB migrations (apply before deploy of new features)

Apply any pending index migrations before deploying feature changes that depend on schema updates:

```bash
wrangler d1 execute grumps-index --file=migrations/index/0002_workspace_locale.sql
```

Replace `grumps-index` with the actual D1 database name used for `INDEX_DB` (check `wrangler.toml`).

### 1.5 Create KV Namespace

```bash
wrangler kv namespace create KV
```

Copy the `id` and update `wrangler.toml`:

```toml
[[kv_namespaces]]
binding = "KV"
id = "PASTE_HERE"
```

### 1.6 Create a Cloudflare API Token

Go to: https://dash.cloudflare.com/profile/api-tokens

Create a token with these permissions:

- **Account > D1 > Edit** (to create/query workspace databases)
- **Account > Workers KV Storage > Edit**

Copy the token — this is your `CF_API_TOKEN`.

---

## Step 2: Meta / WhatsApp Business API Setup

### 2.1 Create a Meta Business App

1. Go to https://developers.facebook.com/apps/
2. Click "Create App" → "Business" type
3. Add the **WhatsApp** product

### 2.2 Business Verification

**Start this immediately** — it can take days/weeks.

1. Go to Business Settings → Business Info
2. Complete verification (requires ID + proof of address)
3. Without verification, you can only message yourself (test mode)

### 2.3 Get WhatsApp Credentials

In your app dashboard → WhatsApp → API Setup:

- **Phone Number ID**: shown under "From" phone number
- **Permanent Access Token**: generate one under System Users (or use the temporary one for testing)
- **App Secret**: under App Settings → Basic → App Secret

### 2.4 Set a Verify Token

Choose a random string (e.g., `grumps_verify_2026_xyz`). You'll use this when configuring the webhook.

---

## Step 3: Stripe Setup

### 3.1 Create Products

In Stripe Dashboard → Products:

1. **Grumps Pro** — €5/month recurring
2. **Grumps Business** — €15/month recurring

Note the Price IDs (e.g., `price_xxx`).

### 3.2 Get API Keys

Dashboard → Developers → API Keys:

- **Publishable key**: `pk_live_xxx` (for the SPA checkout)
- **Secret key**: `sk_live_xxx` (for the Worker webhook)
- **Webhook signing secret**: created in step 3.3

### 3.3 Configure Stripe Webhook

Dashboard → Developers → Webhooks → Add Endpoint:

- URL: `https://grumps-api.YOUR_SUBDOMAIN.workers.dev/webhook/stripe`
- Events: `checkout.session.completed`, `customer.subscription.deleted`, `customer.subscription.updated`

Copy the **Signing Secret** (`whsec_xxx`).

---

## Step 4: LLM API Keys

### 4.1 Gemini

1. Go to https://aistudio.google.com/apikey
2. Create an API key
3. This is your `GEMINI_API_KEY`

### 4.2 Anthropic (Claude Haiku fallback)

1. Go to https://console.anthropic.com/settings/keys
2. Create an API key
3. This is your `ANTHROPIC_API_KEY`

---

## Step 5: Configure wrangler.toml

Update `wrangler.toml` with your values:

```toml
name = "grumps-api"
main = "build/worker/shim.mjs"
compatibility_date = "2024-12-01"

[build]
command = "cargo install -q worker-build && worker-build --release"

[vars]
ENVIRONMENT = "production"
WA_PHONE_NUMBER_ID = "YOUR_PHONE_NUMBER_ID"
WA_VERIFY_TOKEN = "YOUR_CHOSEN_VERIFY_TOKEN"
# TG_BOT_USERNAME = "grumps_bot"                    # Uncomment when ready
# DISCORD_APPLICATION_ID = "YOUR_DISCORD_APP_ID"    # Uncomment when ready

[[d1_databases]]
binding = "INDEX_DB"
database_name = "grumps-index"
database_id = "YOUR_INDEX_DB_ID"

[[kv_namespaces]]
binding = "KV"
id = "YOUR_KV_ID"

[triggers]
crons = ["*/5 * * * *"]
```

---

## Step 6: Set Secrets

```bash
# WhatsApp
wrangler secret put WA_APP_SECRET
# → paste your Meta App Secret

wrangler secret put WA_ACCESS_TOKEN
# → paste your WhatsApp permanent access token

# Cloudflare (for D1 REST API)
wrangler secret put CF_API_TOKEN
# → paste your Cloudflare API token

wrangler secret put CF_ACCOUNT_ID
# → paste your Cloudflare Account ID

# JWT
wrangler secret put JWT_SECRET
# → paste a random 32+ character string (e.g., openssl rand -hex 32)

# LLM
wrangler secret put GEMINI_API_KEY
# → paste your Gemini API key

wrangler secret put ANTHROPIC_API_KEY
# → paste your Anthropic API key

# Telegram (when ready)
# wrangler secret put TG_BOT_TOKEN
# wrangler secret put TG_WEBHOOK_SECRET

# Discord (when ready)
# wrangler secret put DISCORD_BOT_TOKEN
# wrangler secret put DISCORD_PUBLIC_KEY
```

---

## Step 7: Deploy the Worker

```bash
cd /path/to/grumps
wrangler deploy
```

Note the deployed URL: `https://grumps-api.YOUR_SUBDOMAIN.workers.dev`

### Verify deployment

```bash
curl https://grumps-api.YOUR_SUBDOMAIN.workers.dev/health
# → "ok"
```

---

## Step 8: Configure Meta Webhook

1. Go to your Meta App → WhatsApp → Configuration
2. **Webhook URL**: `https://grumps-api.YOUR_SUBDOMAIN.workers.dev/webhook/whatsapp`
3. **Verify Token**: same as `WA_VERIFY_TOKEN` from wrangler.toml
4. Click "Verify and Save"
5. Subscribe to: **messages**

### Test it

1. Add the bot's phone number to a WhatsApp group
2. Send: `TODO:\n• Test from production`
3. The bot should respond with a task card
4. The workspace is auto-provisioned on first message

---

## Step 9: Deploy the SPA

### 9.1 Install Trunk

```bash
cargo install trunk
```

### 9.2 Install Tailwind CSS

```bash
cd crates/spa
npm init -y
npm install -D tailwindcss
```

### 9.3 Build the SPA

```bash
cd crates/spa
trunk build --release
```

This creates `crates/spa/dist/` with the WASM bundle + HTML + CSS.

### 9.4 Deploy to Cloudflare Pages

```bash
# From the project root
wrangler pages project create grumps-web
wrangler pages deploy crates/spa/dist/ --project-name=grumps-web
```

### 9.5 Update API Base URL

Before building, update `crates/spa/src/api.rs` line 5:

```rust
const API_BASE: &str = "https://grumps-api.YOUR_SUBDOMAIN.workers.dev";
```

Then rebuild and redeploy.

### 9.6 Custom Domain

Landing + SPA are served on GitHub Pages at `grumps.app`. The CNAME file at `landing/CNAME` is copied to `dist/CNAME` by `landing/build.mjs`; configure in GitHub → Settings → Pages → Custom domain: `grumps.app`, then enable Enforce HTTPS once the DNS check passes.

The Worker API is served at `api.grumps.app`. The route is declared in `wrangler.toml` (`[[routes]]` section with `zone_name = "grumps.app"`) and applied on `wrangler deploy`. Requires the `grumps.app` zone to be attached to the Cloudflare account (Cloudflare dashboard → Add a site → `grumps.app` → set nameservers at registrar).

---

## Step 10: Telegram Setup (Optional)

### 10.0 Privacy mode check (one-time per bot)

In a DM with [@BotFather](https://t.me/BotFather), send `/setprivacy`, pick `@grumps_bot`, and verify the current state is **Enabled**. Privacy mode is ON by default on new bots; this is just a verification.

Grumps relies on the group-admin-promotes-the-bot workflow to unlock non-mention message reception. Disabling privacy mode globally would work but removes the per-group opt-in — do not do this.

If you toggle privacy mode after the bot is already in a group, Telegram does not apply the change retroactively. Remove the bot from the group and re-add it.

### 10.1 Create a Bot

1. Message [@BotFather](https://t.me/BotFather) on Telegram
2. `/newbot` → name it "Grumps" → username `grumps_bot`
3. Copy the **bot token**

### 10.2 Set Webhook

```bash
curl -X POST "https://api.telegram.org/bot{YOUR_BOT_TOKEN}/setWebhook" \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://grumps-api.YOUR_SUBDOMAIN.workers.dev/webhook/telegram",
    "secret_token": "YOUR_TG_WEBHOOK_SECRET"
  }'
```

### 10.3 Set Secrets

```bash
wrangler secret put TG_BOT_TOKEN
wrangler secret put TG_WEBHOOK_SECRET
```

Update `wrangler.toml`:

```toml
[vars]
TG_BOT_USERNAME = "grumps_bot"
```

Redeploy: `wrangler deploy`

---

## Step 11: Discord Setup (Optional)

### 11.1 Create a Bot

1. Go to https://discord.com/developers/applications
2. New Application → "Grumps"
3. Bot → Add Bot → copy token
4. OAuth2 → URL Generator → scope: `bot`, permissions: `Send Messages`, `Read Message History`
5. Copy the invite URL and add the bot to a server

### 11.2 Set Secrets

```bash
wrangler secret put DISCORD_BOT_TOKEN
wrangler secret put DISCORD_PUBLIC_KEY
```

Update `wrangler.toml`:

```toml
[vars]
DISCORD_APPLICATION_ID = "YOUR_APP_ID"
```

Redeploy: `wrangler deploy`

---

## Step 12: Landing Page (Optional)

The landing page is a static HTML file (`index.html` in the project root). Deploy to GitHub Pages or CF Pages:

```bash
# Using CF Pages
wrangler pages project create grumps-landing
wrangler pages deploy . --project-name=grumps-landing --include="index.html,workspace.html"
```

Or push to a GitHub repo and enable Pages on the `main` branch.

---

## Environment Summary

| Secret               | Source                   | Purpose                   |
| -------------------- | ------------------------ | ------------------------- |
| `WA_APP_SECRET`      | Meta Developer Dashboard | HMAC webhook verification |
| `WA_ACCESS_TOKEN`    | Meta WhatsApp API Setup  | Send messages             |
| `CF_API_TOKEN`       | Cloudflare API Tokens    | D1 REST API access        |
| `CF_ACCOUNT_ID`      | Cloudflare Dashboard     | D1 REST API URL           |
| `JWT_SECRET`         | Self-generated           | Sign/verify JWT tokens    |
| `GEMINI_API_KEY`     | Google AI Studio         | Primary NLU               |
| `ANTHROPIC_API_KEY`  | Anthropic Console        | Fallback NLU              |
| `TG_BOT_TOKEN`       | Telegram BotFather       | Telegram messaging        |
| `TG_WEBHOOK_SECRET`  | Self-generated           | Telegram webhook auth     |
| `DISCORD_BOT_TOKEN`  | Discord Developer Portal | Discord messaging         |
| `DISCORD_PUBLIC_KEY` | Discord Developer Portal | Discord webhook auth      |

| Variable                 | Value          | Purpose                    |
| ------------------------ | -------------- | -------------------------- |
| `WA_PHONE_NUMBER_ID`     | From Meta      | WhatsApp sender ID         |
| `WA_VERIFY_TOKEN`        | Self-generated | Meta webhook verification  |
| `TG_BOT_USERNAME`        | From BotFather | Telegram mention detection |
| `DISCORD_APPLICATION_ID` | From Discord   | Discord mention detection  |

---

## Monitoring

### Cloudflare Dashboard

- Workers → grumps-api → Metrics (requests, errors, latency)
- D1 → grumps-index → Metrics (rows read/written)
- KV → Metrics (reads/writes)

### Logs

```bash
wrangler tail
```

This streams live logs from your Worker. Look for:

- `Created D1 ...` — workspace provisioning
- `Gemini low confidence ...` — LLM fallback
- `Sent recap for ...` — recap delivery
- `Fired reminder ...` — reminder delivery
- `Cron error: ...` — cron failures

### Cost Tracking

- WhatsApp: ~$1.50/group/month (check Meta billing)
- Gemini: track via KV (`llm_calls_YYYY_MM` in settings)
- Cloudflare: free tier covers MVP easily

---

## Deploying the Agent Layer (Plans A-F)

After the Plan A foundation merge, deploy with this sequence:

### One-shot infrastructure setup

```bash
# 1. Create the Vectorize index (one-shot, idempotent if you ignore "already exists")
wrangler vectorize create grumps-chat-rag --dimensions=1024 --metric=cosine

# 2. Set new secrets
wrangler secret put BRAVE_API_KEY        # web search (optional)
# ANTHROPIC_API_KEY and GEMINI_API_KEY should already be set
```

### Per-deploy

```bash
# 3. Deploy the worker (creates DO classes via wrangler.toml [[migrations]])
wrangler deploy

# 4. Migrate existing workspace databases (idempotent)
./scripts/migrate_workspaces.sh

# 5. Deploy the SPA
cd crates/spa && trunk build --release
wrangler pages deploy ../../dist/
```

### Post-deploy (one-shot, optional)

```bash
# Backfill RAG vectors for past chat messages — improves "what did we say about X" queries
GRUMPS_ADMIN_TOKEN="..." ./scripts/rag_backfill.sh
```

### Rollback

- Worker : `git revert` the offending commit, redeploy
- D1 : new tables stay in place but unused, no data loss
- DOs : alarms persist. Old code may not handle them — only revert if alarm handlers crash on the old version.

## Operating notes

### Quotas to monitor

- `agent_quota_used_month` per workspace : Sonnet calls
- `web_search_quota_used_month` per workspace : Brave calls
- Reset on the 1st of each month via a system scheduled_action

### Cost ceiling

Hard caps per plan (Free/Pro/Business). When exceeded, agent returns honest message with upgrade link.

### Logs

- All errors logged via `console_log!` (visible in `wrangler tail`)
- Auto-extract failures, classifier errors, DO RPC errors, web search timeouts — all logged but never propagate to user

### Telegram bot setup

The bot welcome flow already exists (commit 03363f2). Auto-provision creates the workspace + applies all 4 migrations.

---

## Troubleshooting

| Problem                | Solution                                                      |
| ---------------------- | ------------------------------------------------------------- |
| Webhook returns 403    | Check `WA_APP_SECRET` matches Meta dashboard                  |
| Bot doesn't respond    | Check `wrangler tail` for errors, verify webhook URL in Meta  |
| Workspace not created  | Check `CF_API_TOKEN` has D1 permissions                       |
| OTP not received       | Check `WA_ACCESS_TOKEN` is valid, phone number is correct     |
| SPA shows blank page   | Check `API_BASE` in api.rs matches Worker URL                 |
| CORS errors in browser | Check origin matches `ALLOWED_ORIGINS` in middleware.rs       |
| JWT expired            | Token lasts 7 days, user needs to re-login                    |
| LLM not working        | Check `GEMINI_API_KEY`, look for "Gemini error" in logs       |
| Cron not firing        | Verify `[triggers]` in wrangler.toml, check Workers dashboard |
