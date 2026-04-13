# Grumps — Design Document

## Overview

AI bot for messaging groups (WhatsApp first, then Telegram/Discord) that turns group conversations into a collaborative workspace: todos, notes, files, reminders, recaps. Each group gets a private web workspace accessible only to verified members.

**Full technical specs**: see `SPECS.md` at project root (canonical reference for all architecture, API, data model, and feature decisions).

**Visual design reference**: see `workspace.html` and `index.html` (validated HTML prototypes with the complete design system).

## Design System (Validated)

- **Aesthetic**: Warm brutalism — confident, opinionated, no generic SaaS feel
- **Colors**: Cream base (#F5F0E8), brick red (#C0392B) for actions/urgency, teal (#1B6B5A) for success, ochre (#D4940A) for warnings, 2px dark borders for structure
- **Typography**: Bitter (slab serif) for headings, DM Sans for body
- **Layout**: Chunky bordered elements, grain texture overlay, no shadows
- **Voice**: Terse, dry. "Done." not "Successfully completed!"

## Tech Stack

- **Frontend**: Rust/WASM via Leptos 0.7+ (CSR mode), Tailwind CSS, deployed to CF Pages
- **Backend**: Rust via workers-rs, deployed as CF Worker
- **Database**: D1 (SQLite) — one per workspace + shared Index DB
- **Storage**: R2 for files
- **Cache/Sessions**: KV for OTP, rate limiting, cache
- **LLM**: Gemini 2.5 Flash (primary NLU), Claude Haiku 4.5 (fallback)
- **Messaging**: WhatsApp Business Cloud API with adapter pattern for multi-platform
- **Billing**: Stripe Checkout + Customer Portal
- **Landing page**: Static site on GitHub Pages

## Architecture

100% Cloudflare serverless, zero servers. See SPECS.md Section 3 for full architecture diagram, data flow, and crate workspace layout.

### Crate Workspace

```
grumps/
├── Cargo.toml              (workspace)
├── crates/
│   ├── core/               # Shared domain types + validation
│   ├── nlu/                # Regex parsing + LLM prompt construction
│   ├── messaging/          # Adapter trait + WhatsApp/TG/Discord impls
│   ├── worker/             # CF Worker API backend
│   └── spa/                # Leptos CSR frontend
```

## Implementation Phases

### Phase 1 — Chat MVP
Rust crate workspace, WhatsApp webhook, messaging adapter, D1 schema, regex parsing for TODO:/DONE:/NOTE:, CRUD todos, task cards with reply handling, fuzzy DONE matching, basic notes. Deploy Worker.

### Phase 2 — Web Workspace
WhatsApp OTP auth, Leptos CSR SPA with all pages (dashboard, todos list+kanban, notes list+editor, files grid+upload+preview, history, settings). Deploy to CF Pages. Apply the validated design system.

### Phase 3 — Intelligence + Product
Gemini/Haiku NLU integration, natural language commands, assignments/deadlines/priorities/tags, reminders via Cron Triggers, automatic recaps, Stripe billing with freemium plans.

### Phase 4 — Scale + Multi-platform
Telegram/Discord adapters, D1 REST API for scale, recurring todos, export, PWA, public API, full i18n, marketing landing page.

## Key Design Decisions

1. **1 D1 database per workspace** — isolation, simplicity, scales to 50K DBs/account
2. **Regex fast path** — ~70% of messages parsed without LLM, major cost savings
3. **No WebSocket** — polling every 30s is sufficient for a group tool
4. **JWT in memory** — not localStorage, for security
5. **Adapter pattern from day 1** — multi-platform without touching core logic
6. **Gemini primary, Haiku fallback** — best cost/quality ratio, free tier covers MVP
