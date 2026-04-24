# Grumps — Technical & Product Specifications (v4)

> AI agent for messaging groups: todos, notes, files, reminders + collaborative web workspace.
> Stack: Rust/WASM (Leptos CSR SPA) · Cloudflare Workers (API) · D1 (SQLite) · R2 · KV · Gemini 2.5 Flash
> Architecture: 100% Cloudflare serverless, zero servers
> Domain: grumps.app

---

## 1. Product Vision

A bot you add to any WhatsApp group (then Telegram, Discord, Slack) that turns the conversation into a **lightweight collaborative workspace**: assignable todos, shared notes, files, reminders, automatic recaps.

Each group gets a **private web workspace** accessible only to verified group members, featuring a rich note editor, todo views, shared file storage, and full activity history.

**Positioning**: the anti-Notion — zero friction in chat, but a real web workspace when you need one. TaskRio does chat; we do chat **+ web**.

**Tagline**: "Gets it done. No small talk."

### Differentiation vs TaskRio

| Feature | TaskRio | Grumps |
|---|---|---|
| Group todos via @mention | ✅ | ✅ |
| Task cards with reply (done/snooze) | ✅ | ✅ |
| NLU / natural language | ✅ | ✅ |
| Assignment, deadlines, priorities | ✅ | ✅ |
| Private DM for personal lists | ✅ | ✅ |
| **Private web workspace** | ❌ | ✅ |
| **Markdown + rich text note editor** | ❌ | ✅ |
| **File sharing + inline preview** | ❌ | ✅ |
| **Scheduled automatic recaps** | ❌ | ✅ |
| **Multi-platform (Telegram, Discord)** | ❌ | ✅ |
| **Public API** | ❌ | ✅ (phase 4) |

---

## 2. WhatsApp Integration

### Decision: WhatsApp Business Cloud API + multi-platform architecture

| Criterion | WA Business Cloud API | Baileys / whatsapp-web.js | Telegram Bot API |
|---|---|---|---|
| Stability | ✅ Official Meta | ❌ Reverse-engineered | ✅ Official |
| Groups | ✅ Supported | ✅ Native | ✅ Native, first-class |
| Cost | ~$0.005–0.08/conv (24h) | Free | Free |
| Ban risk | None | High | None |
| Webhook | ✅ Native | ❌ Fragile | ✅ Native |

Baileys is eliminated for any SaaS (guaranteed ban at scale). We start with the official Meta API, with an **adapter pattern** from day 1 to plug in Telegram/Discord later without touching the core.

**WhatsApp cost**: ~$1.50/month per active group. Compatible with a Pro plan at €5/month.

### Meta Prerequisites
- Create a Meta Business App
- Business verification (KYC: identity + address) — **start on day 1**
- Dedicated phone number
- HTTPS webhook (= Worker route)

---

## 3. Global Architecture

### Principle

Zero servers. Everything is Cloudflare serverless:
- **CF Pages**: static SPA (Leptos CSR compiled to WASM)
- **CF Workers**: serverless API/BFF (webhooks, auth, CRUD, LLM proxy)
- **D1**: one SQLite database per workspace (group)
- **R2**: file storage (zero egress fees)
- **KV**: OTP sessions, cache, rate limiting
- **Cron Triggers**: reminders and automatic recaps

The marketing landing page is a separate static site on GitHub Pages.

### Schema

```
GitHub Pages              CF Pages (SPA)               WhatsApp
  Landing                 Leptos CSR/WASM               Groups
  static                       │                          │
                                │ fetch()                  │ Webhook POST
                                ▼                          ▼
                   ┌──────────────────────────────────────────┐
                   │       Cloudflare Workers (API)           │
                   │                                          │
                   │  ┌──────┐ ┌──────┐ ┌────────┐ ┌──────┐  │
                   │  │ Auth │ │ CRUD │ │Webhook │ │ LLM  │  │
                   │  │ OTP  │ │ API  │ │Handler │ │Proxy │  │
                   │  └──────┘ └──────┘ └────────┘ └──────┘  │
                   │                                          │
                   │  ┌────────────┐  ┌───────────────────┐   │
                   │  │Cron Trigger│  │Messaging Adapter  │   │
                   │  │Remind/Recap│  │WA / TG / Discord  │   │
                   │  └────────────┘  └───────────────────┘   │
                   └───────┬──────────────┬──────────┬────────┘
                           │              │          │
                   Native bindings (zero network latency)
                           │              │          │
                   ┌───────▼───┐  ┌───────▼───┐ ┌───▼────┐
                   │ D1 (SQLite)│  │ R2 (S3)   │ │  KV    │
                   │ 1 DB/group │  │ Files     │ │Sessions│
                   └───────┬───┘  └───────────┘ └────────┘
                           │
                   ┌───────▼──────────┐
                   │ Index DB (D1)    │
                   │ phone→workspaces │
                   │ users, billing   │
                   └──────────────────┘
```

### Data Flow

**Chat → Bot**: WhatsApp message → Meta sends HTTP POST to Worker webhook → Worker parses, writes to workspace D1, responds via Meta API. 100% request/response, no persistent connections.

**Web → API**: SPA calls fetch() to the Worker API → Worker verifies JWT, resolves the workspace D1 binding, executes the query, returns JSON. Optional polling (setInterval 30s) to pick up changes from chat.

### Rust Crate Workspace

```
grumps/
├── Cargo.toml                  (workspace)
├── crates/
│   ├── core/                   # Pure domain logic, shared between Worker and SPA
│   │   ├── todo.rs             # Types, validation, serialization
│   │   ├── note.rs
│   │   ├── reminder.rs
│   │   ├── file_meta.rs
│   │   └── workspace.rs
│   ├── nlu/                    # Regex parsing + LLM prompt construction
│   ├── messaging/              # Adapter trait + implementations
│   │   ├── adapter.rs
│   │   ├── whatsapp.rs
│   │   ├── telegram.rs         # (stub phase 1)
│   │   └── discord.rs          # (stub phase 1)
│   ├── worker/                 # Cloudflare Worker (API backend)
│   │   ├── routes/
│   │   │   ├── auth.rs         # OTP + JWT
│   │   │   ├── todos.rs        # CRUD todos
│   │   │   ├── notes.rs        # CRUD notes
│   │   │   ├── files.rs        # Upload/download/list
│   │   │   ├── webhook.rs      # WhatsApp webhook handler
│   │   │   └── workspace.rs    # Settings, members
│   │   ├── d1.rs               # D1 query helpers
│   │   ├── middleware.rs       # JWT auth verification
│   │   └── lib.rs              # Worker entrypoint
│   └── spa/                    # Leptos CSR app (compiles to WASM)
│       ├── pages/
│       │   ├── workspace.rs    # /w/:slug
│       │   ├── todos.rs        # /w/:slug/todos
│       │   ├── notes.rs        # /w/:slug/notes
│       │   ├── note_editor.rs  # /w/:slug/notes/:id/edit
│       │   ├── files.rs        # /w/:slug/files
│       │   ├── history.rs      # /w/:slug/history
│       │   ├── settings.rs     # /w/:slug/settings
│       │   ├── dashboard.rs    # /dashboard (my workspaces)
│       │   └── login.rs        # /login
│       └── components/
│           ├── todo_card.rs
│           ├── kanban_board.rs
│           ├── note_editor.rs
│           ├── file_preview.rs
│           └── ...
```

---

## 4. Messaging Adapter Pattern

```rust
#[async_trait]
pub trait MessagingPlatform: Send + Sync {
    fn platform_id(&self) -> &str;
    async fn send_message(&self, channel_id: &str, message: OutboundMessage) -> Result<MessageId>;
    async fn send_media(&self, channel_id: &str, media: OutboundMedia) -> Result<MessageId>;
    fn parse_webhook(&self, payload: &[u8], headers: &HeaderMap) -> Result<InboundMessage>;
    fn verify_signature(&self, payload: &[u8], headers: &HeaderMap) -> Result<()>;
    async fn send_otp(&self, recipient_id: &str, code: &str) -> Result<()>;
}

pub struct InboundMessage {
    pub platform: String,
    pub channel_id: String,
    pub message_id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub text: Option<String>,
    pub media: Option<InboundMedia>,
    pub timestamp: DateTime<Utc>,
    pub is_mention_to_bot: bool,
    pub is_direct_message: bool,
    pub quoted_message_id: Option<String>,
    pub quoted_message_text: Option<String>,
}

pub struct InboundMedia {
    pub media_type: MediaType,   // Image, Document, Audio, Video
    pub mime_type: String,
    pub filename: Option<String>,
    pub url: String,             // Temporary Meta URL for download
    pub size_bytes: Option<u64>,
}

pub struct OutboundMessage {
    pub text: String,
    pub reply_to: Option<String>,
    pub buttons: Vec<QuickReply>,
    pub formatting: MessageFormatting,
}
```

---

## 5. Chat Command Protocol

### Philosophy (TaskRio parity)

- **In groups**: the bot ONLY responds when @mentioned or when someone replies to one of its task cards. Normal human conversations are ignored.
- **In DMs**: the bot responds to everything, no @mention needed.
- **Task cards**: the bot sends a formatted message. Users can reply directly with `done`, `snooze`, `edit`, `@reassign`.

### 5.1 Todos

| Action | Syntax | Example |
|---|---|---|
| **Add** (block) | `TODO:` followed by a list | `TODO:`<br>`• Buy toilet paper`<br>`• Call the plumber` |
| **Add** (mention) | `@grumps <free instruction>` | `@grumps add "book restaurant" for friday` |
| **Add by reply** | Reply to a msg + `@grumps todo` | Quoted message content becomes a todo |
| **Add with assignment** | `@member` in text | `@grumps Follow up with client @Pierre` |
| **Add with deadline** | Natural language dates | `@grumps todo buy gifts before dec 24` |
| **Add with priority** | `!high` / `!low` or `!!!` | `@grumps todo Ship the project !high` |
| **Add with tag** | `#tag` | `@grumps todo Fix CSS #frontend #urgent` |
| **Complete** (reply card) | Reply `done` to a task card | Marks as done |
| **Complete** (block) | `DONE:` followed by a list | `DONE:`<br>`• bread`<br>`• plumber` |
| **Complete** (mention) | `@grumps done <description>` | `@grumps done the bread is bought` |
| **Snooze** (reply card) | Reply `tomorrow` / `monday` / `in 2h` | Postpones deadline |
| **Reassign** (reply card) | Reply `@NewMember` | Reassigns |
| **Edit** (reply card) | Reply `edit New title here` | Updates the title |
| **Delete** | `@grumps delete #ID` | |
| **List in group** | `@grumps show` / `@grumps show open` | Displays todos in group |
| **List in DM** | `list` / `list mine` / `list open` | Private interactive list |
| **Search** | `@grumps search <query>` | |
| **Filter** | `@grumps todos @Pierre` / `@grumps todos #frontend` | |
| **AI summary** | `@grumps summarize` | AI summary of conversation + action item extraction |

### 5.2 Notes

| Action | Syntax | Example |
|---|---|---|
| **Create** | `NOTE:` or `@grumps note` | `NOTE: the office wifi password is XYZ123` |
| **Create named** | `NOTE [title]:` | `NOTE [wifi]: password = XYZ123` |
| **Create by reply** | Reply + `@grumps note` or `@grumps pin` | Quoted message becomes a note |
| **Search** | `@grumps note <search>` | `@grumps note wifi` |
| **List** | `@grumps notes` | |
| **Delete** | `@grumps delete note "wifi"` | |
| **Edit** | Via the web workspace only (rich editor) | |

### 5.3 Files

| Action | Syntax | Example |
|---|---|---|
| **Store** | `@grumps store` as reply to a message with media | Quote a PDF + `@grumps store` |
| **Store named** | `@grumps store [name]` | `@grumps store [january invoice]` |
| **List** | `@grumps files` | |
| **Search** | `@grumps file <query>` | `@grumps file invoice` |
| **Delete** | `@grumps delete file "invoice"` | |

### 5.4 Reminders

| Action | Syntax | Example |
|---|---|---|
| **Create** | `@grumps remind` | `@grumps remind tomorrow 9am call the dentist` |
| **Create for someone** | `@grumps remind @Pierre` | `@grumps remind @Pierre monday 10am standup` |
| **Recurring** | NLU detection | `@grumps remind every monday take out the trash` |
| **List** | `@grumps reminders` | |
| **Cancel** | `@grumps cancel reminder #ID` | |

### 5.5 Utility Commands

| Command | Description |
|---|---|
| `@grumps help` | Contextual help |
| `@grumps status` | Summary: X todos, Y notes, Z files, W reminders |
| `@grumps recap` | AI recap sent to the chat |
| `@grumps workspace` / `@grumps link` | Sends the workspace URL |
| `@grumps lang EN\|FR\|ES` | Changes the bot language |
| `@grumps quiet` | Quiet mode: minimal confirmations |
| `@grumps summarize` | AI summary of recent conversation |

---

## 6. Task Cards (TaskRio Parity)

Format of a task card sent by the bot:

```
📋 Task #42
━━━━━━━━━━━━━━━━━━━
Follow up with client Dupont

👤 @Pierre
⏰ Friday April 18
🔴 High priority
🏷️ #sales #client

━━━━━━━━━━━━━━━━━━━
Reply: done · snooze · edit · @reassign
```

### Interactions via WhatsApp reply (no @mention needed)

| Reply | Action |
|---|---|
| `done` | Marks complete, notifies creator |
| `tomorrow` / `monday` / `in 3h` | Snooze: postpones deadline |
| `edit New title here` | Updates the title |
| `@Alice` | Reassigns to Alice |
| `!high` / `!low` | Changes priority |
| `#newtag` | Adds a tag |
| `cancel` / `delete` | Deletes the todo |
| `status: blocked` | Changes status, notifies creator |

---

## 7. DONE Matching Flow

```
User: DONE:
      • bread
      • plumber

Bot:
  ✅ "Buy bread" (#23) → marked as done
  🔍 "plumber" — 2 close matches:
      1. "Call the plumber about the leak" (#12)
      2. "Pay plumber invoice" (#15)
  → Which one? Reply 1 or 2 (or "all")
```

### Matching Algorithm

1. **Exact match**: normalized comparison (lowercase, accents stripped, stopwords removed)
2. **Fuzzy match**: Jaro-Winkler distance + token overlap
3. **Semantic match** (LLM): if fuzzy < threshold, send to LLM with the list of open todos
4. **Thresholds**:
   - \>90% → auto-confirm
   - 50-90% → ask confirmation with the most likely candidate
   - <50% → list candidates
   - 0 candidates → "No todo found for 'plumber'. Want to create one?"

---

## 8. LLM Integration (NLU Engine)

### Model Strategy

**Primary model: Google Gemini 2.5 Flash** — best cost/quality ratio for simple NLU tasks.

| | Gemini 2.5 Flash | Claude Haiku 4.5 (fallback) |
|---|---|---|
| Input / 1M tokens | $0.15 | $0.25 |
| Output / 1M tokens | $0.60 | $1.25 |
| Cost per NLU call* | ~$0.00018 | ~$0.00034 |
| Latency | ~200ms | ~300ms |
| French NLU quality | Very good | Very good |
| Structured JSON output | Native | Native |
| Free tier | Yes (generous) | No |

*Estimated NLU call: ~600 tokens input (message + system prompt) + ~150 tokens output (structured JSON).

**Model routing strategy**:
- **Gemini 2.5 Flash** (default): intent detection, entity extraction, date parsing, simple DONE matching
- **Claude Haiku 4.5** (fallback): complex DONE matching when fuzzy confidence is low, conversation summarization, ambiguous multi-intent messages

The routing logic lives in the Worker. If Gemini returns a low-confidence match or fails to parse, the Worker retries with Haiku before asking the user for clarification.

### Responsibilities

1. **Intent detection**: classify message (add_todo, complete_todo, add_note, store_file, set_reminder, query, summarize, irrelevant)
2. **Entity extraction**: title, assignee, deadline, priority, tags
3. **Fuzzy DONE matching**: match free text → existing open todos
4. **Date parsing**: "next tuesday", "in 3 days", "before end of month"
5. **Summarize**: summarize conversation + extract action items
6. **Multilingual**: FR, EN, ES, DE, PT minimum

### Call Architecture

```
Incoming message
    │
    ├─── Reply to a bot task card?
    │    YES → Parse the reply (done/snooze/edit) — no LLM
    │
    ├─── Structured message (TODO:/DONE:/NOTE:/REMIND:)?
    │    YES → Regex fast path — no LLM
    │
    ├─── @mention of bot or DM?
    │    YES → Gemini 2.5 Flash for full NLU
    │    │     └─── Low confidence? → Retry with Claude Haiku 4.5
    │
    └─── Otherwise → IGNORE (bot stays silent)
```

### Cost Optimization

- **Regex fast path**: ~70% of interactions handled without any LLM call
- **Model routing**: Gemini Flash (default) → Haiku (fallback for complex cases, ~5% of LLM calls)
- **KV cache**: same intent within a short window = same response
- **Prompt caching**: system prompt cached across calls (Gemini supports this natively)
- **Cost estimate**: ~$0.00018 per Gemini call. 100 active groups × 50 msg/day × 30% to LLM = ~1,500 calls/day = **~$8/month**
- The Gemini free tier (1,500 requests/day free) covers most of the MVP with zero cost

---

## 9. Web Workspace (Leptos CSR SPA)

### 9.1 Frontend Architecture

The SPA is compiled to WASM by Leptos in CSR (Client-Side Rendering) mode. It's served as a static bundle by CF Pages. No server-side rendering — everything runs in the browser. The SPA communicates with the Worker API via `fetch()`.

```
spa/
├── index.html              # Minimal HTML shell
├── pkg/                    # WASM bundle (Leptos CSR)
│   ├── grumps_spa_bg.wasm
│   └── grumps_spa.js
├── assets/
│   └── styles.css          # Tailwind CSS (build-time)
└── _routes.json            # CF Pages routing (SPA fallback)
```

**Data refresh**: no WebSocket. The SPA does a `fetch()` on each page load. Optionally, polling every 30 seconds on the active page to pick up changes from chat. Simple, reliable, and sufficient for a group tool.

### 9.2 Authentication & Access Control

**Goal**: only verified WhatsApp group members can access the web workspace.

#### WhatsApp OTP Flow

```
1. User opens grumps.app/w/x7k9m2p4
2. No JWT in memory → redirect to /login
3. Login page: enter WhatsApp phone number
4. SPA calls POST api.grumps.app/auth/otp { phone, workspace_slug }
5. Worker checks:
   a. Is this number known in the Index DB?
   b. Is this number a member of the requested workspace?
   → If not: error "Number not recognized in this group"
6. Worker generates 6-digit OTP, stores in KV (TTL 5 min)
7. Worker sends OTP via WhatsApp (Meta message template)
8. User enters the code in the SPA
9. SPA calls POST api.grumps.app/auth/verify { phone, code, workspace_slug }
10. Worker verifies code in KV, issues a signed JWT:
    {
      sub: user_id,
      phone: "+33612345678",
      workspaces: ["x7k9m2p4", "y8l0n3q5"],
      exp: now + 7 days
    }
11. SPA stores JWT in memory (not localStorage)
12. Every API call: Authorization: Bearer <jwt>
13. Worker verifies signature + workspace membership
```

#### Member Management

The member list is built **organically**:
- Every message in the group → upsert sender in the workspace DB AND in the Index DB
- No need for a "list group members" API (not always available)
- An admin can revoke web access from settings

#### Roles

| Permission | Admin | Member |
|---|---|---|
| View todos/notes/files | ✅ | ✅ |
| Create/edit todos | ✅ | ✅ |
| Create/edit notes | ✅ | ✅ |
| Upload files | ✅ | ✅ |
| Delete own items | ✅ | ✅ |
| Delete others' items | ✅ | ❌ |
| Complete others' todos | ✅ | ❌ (configurable) |
| Configure workspace | ✅ | ❌ |
| Manage billing | ✅ | ❌ |
| Remove a member | ✅ | ❌ |

The first user who adds the bot to the group = automatic admin.

### 9.3 Pages & Routes

| Route | Description | Auth |
|---|---|---|
| `/login` | WhatsApp OTP auth | No |
| `/dashboard` | My workspaces (group list) | Yes |
| `/dashboard/billing` | Stripe subscription management | Yes |
| `/w/:slug` | Workspace: overview | Yes (member) |
| `/w/:slug/todos` | Todos: list + Kanban + filters | Yes |
| `/w/:slug/notes` | Notes: list + search | Yes |
| `/w/:slug/notes/:id` | Note: read view (rich render) | Yes |
| `/w/:slug/notes/:id/edit` | Note: markdown editor + preview | Yes |
| `/w/:slug/notes/new` | Create a note | Yes |
| `/w/:slug/files` | Files: grid with previews | Yes |
| `/w/:slug/files/:id` | File preview (image, PDF) | Yes |
| `/w/:slug/history` | Activity history | Yes |
| `/w/:slug/settings` | Workspace config (admin) | Yes (admin) |

### 9.4 Note Editor

Two modes, storage in Markdown (source of truth):

**Markdown mode**: monospace text editor, syntax highlighting, live preview in split-screen. Full support: headings, lists, checkboxes, code blocks, tables, images, links.

**Rich text mode**: WYSIWYG block-based view. Blocks: paragraph, heading, list, checklist, code block, blockquote, image, callout. Contextual toolbar. Drag & drop to reorder.

**Implementation**: `pulldown-cmark` compiled to WASM for Markdown parsing → AST → rendered as Leptos components. The rich editor manipulates an AST that serializes to Markdown.

**Chat ↔ web sync**:
- Chat → Web: `NOTE: wifi ABC123` → creates a note in D1, visible on next SPA fetch/poll
- Web → Chat: creating/editing a note on the web → optionally the Worker sends a notification to the group via Meta API

### 9.5 File Management

#### Web Upload

```
1. Drag & drop or file selection on /w/:slug/files
2. SPA sends multipart POST to the Worker
3. Worker uploads to R2 (key = workspace_slug/uuid/filename)
4. Worker writes metadata to the workspace D1
5. Optional: Worker notifies the group ("📎 @Alice uploaded invoice.pdf")
```

#### Chat Capture

```
1. Someone sends a PDF/image in the group
2. Reply with @grumps store [optional name]
3. Worker downloads media via temporary Meta URL
4. Uploads to R2 + metadata in D1
5. Confirmation: "📎 File stored: invoice.pdf"
```

#### Inline Preview

| Type | Preview |
|---|---|
| Images (jpg, png, gif, webp) | Thumbnail + fullscreen lightbox |
| PDF | Inline render via pdf.js (CDN) |
| Video (mp4, webm) | HTML5 player |
| Audio (mp3, ogg) | HTML5 audio player |
| Other | Icon + info + download |

#### File Access

Files in R2 are never public. The SPA requests access from the Worker, which generates a signed R2 URL with expiration (1h). The Worker verifies JWT and membership before signing.

### 9.6 Todo Views

- **List view**: filters (assignee, priority, deadline, tag, status)
- **Kanban view**: configurable columns (To do / In progress / Done), drag & drop between columns
- **Inline creation**: "+" button to create directly from the web
- **Inline editing**: click → side panel with all fields
- **Bulk actions**: multi-select → mark done / delete / reassign

### 9.7 Cross-cutting Features

- **Search**: FTS5 SQLite on todos (title) + notes (title + content) + files (name)
- **Dark mode**
- **PWA**: installable on mobile, push notifications for reminders
- **Export**: CSV (todos), JSON (todos + notes), Markdown (individual notes)
- **Activity log**: who did what, when (audit trail)
- **Optional polling**: `setInterval(30s)` on the active page to pick up changes from chat

### 9.8 Frontend Stack

```
Leptos 0.7+ (CSR mode, compiles to WASM)
├── leptos_router           (client-side routing)
├── leptos_use              (reactive utilities)
├── gloo-net                (fetch API bindings for WASM)
├── pulldown-cmark          (Markdown parsing, compiled to WASM)
├── Tailwind CSS            (build-time)
└── pdf.js                  (CDN, for PDF preview)
```

---

## 10. Worker API (Backend)

### 10.1 Stack

The Worker is written in Rust with `workers-rs`. It compiles to WASM and runs in Cloudflare's V8 runtime.

```toml
[dependencies]
worker = "0.4"
worker-macros = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = "0.4"
jsonwebtoken = "9"
uuid = { version = "1", features = ["v4", "js"] }
strsim = "0.11"
```

### 10.2 API Routes

```
POST   /webhook/whatsapp          # Meta webhook (signature verified)
GET    /webhook/whatsapp           # Meta verification challenge

POST   /auth/otp                   # Send OTP { phone, workspace_slug }
POST   /auth/verify                # Verify OTP { phone, code }

GET    /api/workspaces             # List my workspaces (from JWT)
GET    /api/w/:slug                # Workspace info

GET    /api/w/:slug/todos          # List todos (filters via query params)
POST   /api/w/:slug/todos          # Create todo
PATCH  /api/w/:slug/todos/:id      # Update todo
DELETE /api/w/:slug/todos/:id      # Delete todo

GET    /api/w/:slug/notes          # List notes
POST   /api/w/:slug/notes          # Create note
GET    /api/w/:slug/notes/:id      # Read note
PUT    /api/w/:slug/notes/:id      # Update note (Markdown content)
DELETE /api/w/:slug/notes/:id      # Delete note

GET    /api/w/:slug/files          # List files
POST   /api/w/:slug/files          # Upload file
GET    /api/w/:slug/files/:id      # Get signed R2 URL
DELETE /api/w/:slug/files/:id      # Delete file

GET    /api/w/:slug/history        # Audit log
GET    /api/w/:slug/members        # List members
PATCH  /api/w/:slug/members/:id    # Update role / remove
GET    /api/w/:slug/settings       # Workspace config
PUT    /api/w/:slug/settings       # Update config
```

### 10.3 Dynamic D1 Resolution

Each workspace has its own D1 database. The Worker resolves the binding dynamically:

```
Option A: Static bindings (simple, up to ~100 workspaces)
  wrangler.toml lists bindings: DB_ws_x7k9m2p4, DB_ws_y8l0n3q5, ...

Option B: D1 REST API (scalable, unlimited)
  The Worker calls the Cloudflare D1 REST API with the database_id
  stored in the Index DB. No static binding needed.
  Slightly higher latency (~50-100ms vs ~1ms for bindings)
  but scales to thousands of workspaces.
```

Recommendation: **Option A** for MVP (< 100 workspaces), **Option B** for scale. The switch from A to B is transparent to the SPA.

### 10.4 Auth Middleware

```rust
fn verify_jwt(req: &Request, env: &Env) -> Result<Claims> {
    let token = req.headers()
        .get("Authorization")?
        .strip_prefix("Bearer ")?;
    let secret = env.secret("JWT_SECRET")?.to_string();
    let claims = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default()
    )?;
    Ok(claims.claims)
}

fn verify_membership(claims: &Claims, workspace_slug: &str) -> Result<()> {
    if !claims.workspaces.contains(&workspace_slug.to_string()) {
        return Err(Error::Forbidden);
    }
    Ok(())
}
```

---

## 11. Data Model (D1 SQLite)

### Index DB (single, shared)

```sql
CREATE TABLE users (
    id              TEXT PRIMARY KEY,    -- UUID
    phone           TEXT UNIQUE NOT NULL,
    created_at      TEXT DEFAULT (datetime('now'))
);

CREATE TABLE user_workspaces (
    user_id         TEXT NOT NULL REFERENCES users(id),
    workspace_slug  TEXT NOT NULL,
    workspace_name  TEXT,
    platform        TEXT NOT NULL,       -- 'whatsapp'
    role            TEXT DEFAULT 'member',
    d1_database_id  TEXT NOT NULL,       -- D1 database ID for this workspace
    created_at      TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (user_id, workspace_slug)
);
CREATE INDEX idx_uw_user ON user_workspaces(user_id);

CREATE TABLE workspaces_meta (
    slug            TEXT PRIMARY KEY,
    platform        TEXT NOT NULL,
    platform_channel_id TEXT NOT NULL,
    name            TEXT,
    plan            TEXT DEFAULT 'free',
    d1_database_id  TEXT NOT NULL,
    stripe_customer_id TEXT,
    created_at      TEXT DEFAULT (datetime('now')),
    UNIQUE(platform, platform_channel_id)
);

CREATE TABLE billing (
    workspace_slug  TEXT PRIMARY KEY REFERENCES workspaces_meta(slug),
    plan            TEXT DEFAULT 'free',
    stripe_subscription_id TEXT,
    current_period_end TEXT,
    storage_used_bytes INTEGER DEFAULT 0,
    llm_calls_this_month INTEGER DEFAULT 0
);
```

### Workspace DB (one per group, created dynamically)

```sql
CREATE TABLE members (
    id              TEXT PRIMARY KEY,
    platform_user_id TEXT NOT NULL,
    display_name    TEXT,
    role            TEXT DEFAULT 'member',
    last_seen_at    TEXT,
    created_at      TEXT DEFAULT (datetime('now')),
    UNIQUE(platform_user_id)
);

CREATE TABLE todos (
    id              TEXT PRIMARY KEY,
    seq_num         INTEGER NOT NULL,
    title           TEXT NOT NULL,
    description     TEXT,
    status          TEXT DEFAULT 'open',       -- open, in_progress, done, blocked, deleted
    priority        INTEGER DEFAULT 2,         -- 1=high, 2=normal, 3=low
    tags            TEXT DEFAULT '[]',          -- JSON array
    deadline        TEXT,                       -- ISO 8601
    assigned_to     TEXT REFERENCES members(id),
    created_by      TEXT REFERENCES members(id),
    completed_at    TEXT,
    completed_by    TEXT REFERENCES members(id),
    position        INTEGER DEFAULT 0,
    source          TEXT DEFAULT 'chat',        -- 'chat' or 'web'
    message_id      TEXT,                       -- WhatsApp message ID (for replies)
    created_at      TEXT DEFAULT (datetime('now')),
    updated_at      TEXT DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX idx_todos_seq ON todos(seq_num);
CREATE INDEX idx_todos_status ON todos(status);
CREATE INDEX idx_todos_assigned ON todos(assigned_to) WHERE status != 'done';

CREATE TABLE notes (
    id              TEXT PRIMARY KEY,
    title           TEXT,
    content         TEXT NOT NULL,              -- Markdown source
    pinned          INTEGER DEFAULT 0,
    tags            TEXT DEFAULT '[]',
    source          TEXT DEFAULT 'chat',
    created_by      TEXT REFERENCES members(id),
    last_edited_by  TEXT REFERENCES members(id),
    created_at      TEXT DEFAULT (datetime('now')),
    updated_at      TEXT DEFAULT (datetime('now'))
);

-- FTS5 for full-text search on notes
CREATE VIRTUAL TABLE notes_fts USING fts5(title, content, content=notes, content_rowid=rowid);

CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts(rowid, title, content) VALUES (new.rowid, new.title, new.content);
END;
CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, content) VALUES('delete', old.rowid, old.title, old.content);
END;
CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, content) VALUES('delete', old.rowid, old.title, old.content);
    INSERT INTO notes_fts(rowid, title, content) VALUES (new.rowid, new.title, new.content);
END;

CREATE TABLE files (
    id              TEXT PRIMARY KEY,
    filename        TEXT NOT NULL,
    display_name    TEXT,
    mime_type       TEXT NOT NULL,
    size_bytes      INTEGER NOT NULL,
    r2_key          TEXT NOT NULL,
    source          TEXT DEFAULT 'chat',
    uploaded_by     TEXT REFERENCES members(id),
    created_at      TEXT DEFAULT (datetime('now'))
);
CREATE INDEX idx_files_created ON files(created_at DESC);

CREATE TABLE reminders (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    remind_at       TEXT NOT NULL,              -- ISO 8601
    recurrence      TEXT,                       -- cron: "0 9 * * 1"
    target_member   TEXT REFERENCES members(id),
    status          TEXT DEFAULT 'active',
    created_by      TEXT REFERENCES members(id),
    created_at      TEXT DEFAULT (datetime('now'))
);
CREATE INDEX idx_reminders_fire ON reminders(remind_at) WHERE status = 'active';

CREATE TABLE activity_log (
    id              TEXT PRIMARY KEY,
    actor           TEXT REFERENCES members(id),
    action          TEXT NOT NULL,              -- 'todo.created', 'note.updated', etc.
    target_type     TEXT,
    target_id       TEXT,
    metadata        TEXT DEFAULT '{}',          -- JSON
    source          TEXT DEFAULT 'chat',
    created_at      TEXT DEFAULT (datetime('now'))
);
CREATE INDEX idx_activity_created ON activity_log(created_at DESC);

CREATE TABLE settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL
);
-- Pre-populate:
-- INSERT INTO settings VALUES ('language', 'en');
-- INSERT INTO settings VALUES ('timezone', 'Europe/Paris');
-- INSERT INTO settings VALUES ('quiet_mode', 'false');
-- INSERT INTO settings VALUES ('recap_cron', '0 9 * * 1');
-- INSERT INTO settings VALUES ('recap_enabled', 'true');
```

---

## 12. Automatic Recaps (Cron Triggers)

A Cloudflare Cron Trigger runs periodically (configurable). It iterates over workspaces via the Index DB, checks which ones have a recap to send, and sends the message via Meta API.

### Recap Format

```
📋 Grumps Recap — Monday April 14

🔴 High priority (2)
  • #12 Ship the project — @Pierre — ⏰ tomorrow
  • #15 Fix the prod bug — @Sarah

📌 Open todos: 7 (3 assigned)
✅ Completed this week: 5
📝 New notes: 2
📎 Files added: 3
⏰ Upcoming reminders: 1

🔗 Workspace: grumps.app/w/x7k9m2p4
```

### Default Schedule
- Free: weekly (Monday 9am)
- Pro: daily + custom
- Configurable via `@grumps recap daily 8:30am` or via web settings

---

## 13. Security

- **Webhook**: Meta signature verification (X-Hub-Signature-256) in the Worker
- **Web auth**: WhatsApp OTP → signed JWT (7d), stored in memory on the SPA side
- **Membership check**: every API request verifies that the user is a member of the workspace
- **R2 files**: never public, signed URLs with 1h expiration
- **Rate limiting**: KV counters per IP and per workspace
- **CORS**: Worker only accepts origins from `grumps.app`
- **GDPR**: deletion on request, JSON export, no unnecessary data retention

---

## 14. Freemium Model

| Feature | Free | Pro (€5/mo) | Business (€15/mo) |
|---|---|---|---|
| Groups | 1 | 5 | Unlimited |
| Open todos | 25 | 200 | Unlimited |
| Notes | 10 | 100 | Unlimited |
| Active reminders | 5 | 50 | Unlimited |
| File storage (R2) | 100 MB | 5 GB | 50 GB |
| Max file size | 10 MB | 50 MB | 200 MB |
| Members per group | 20 | 50 | 200 |
| History | 30 days | 1 year | Unlimited |
| Auto recap | Weekly | Daily + custom | Daily + custom |
| Web workspace | Read-only | Full CRUD + editor | CRUD + editor + export |
| LLM (@grumps messages) | 50/month | 500/month | 2,000/month |
| Support | Community | Email | Priority |

### Billing: Stripe
- Checkout for onboarding, Customer Portal for management
- Stripe webhooks → dedicated Worker
- Grace period: 7 days after payment failure

---

## 15. Estimated Infrastructure Costs

### MVP (< 100 active groups)

| Service | Free tier | Estimated cost |
|---|---|---|
| CF Pages (SPA) | Unlimited | $0 |
| CF Workers | 100K req/day | $0 (or $5/mo paid plan) |
| D1 | 5 GB, 5M reads/day | $0 |
| R2 | 10 GB, 10M ops/month | $0 |
| KV | 100K reads/day | $0 |
| Gemini 2.5 Flash | 1,500 req/day free | $0 (free tier covers MVP) |
| WhatsApp API | ~$1.50/group/month | ~$150/mo for 100 groups |
| Claude Haiku (fallback) | ~5% of LLM calls | ~$2-5/mo |
| GitHub Pages (landing) | Unlimited | $0 |
| **Total** | | **~$155-160/month** |

The dominant cost is WhatsApp API, not infra or LLM. Cloudflare infra is effectively free. Gemini free tier covers the MVP LLM needs entirely.

### Scale (1,000+ groups)

Workers Paid ($5/mo) + D1 overages (~$10-20/mo) + R2 overages (~$5-10/mo) + Gemini (~$80/mo) + Haiku fallback (~$15/mo) = ~$115-130/month infra + LLM. WhatsApp API scales linearly (~$1,500/mo for 1,000 groups).

---

## 16. Deployment

### CI/CD (GitHub Actions)

```yaml
# Build and deploy in 2 separate pipelines

# 1. Worker API
- cargo install worker-build
- worker-build --release
- wrangler deploy

# 2. SPA (CF Pages)
- trunk build --release
- wrangler pages deploy dist/
```

### Environments
- **dev**: local D1 (wrangler dev), local Worker, local SPA (trunk serve)
- **staging**: CF Workers + D1 + R2 on a subdomain
- **prod**: CF Workers + D1 + R2 on the main domain

### Monitoring
- Structured logging via `console_log!` in the Worker
- Workers metrics: Cloudflare dashboard (requests, errors, latency)
- D1 metrics: rows read/written, storage
- LLM metrics: calls/day tracked in KV, cost per workspace
- Alerting: CF Notifications for Worker errors, D1 overloaded

---

## 17. Development Phases

### Phase 1 — Chat MVP (6-8 weeks)
- [ ] Setup Rust crate workspace
- [ ] Meta Business App + verification (start immediately)
- [ ] Worker: WhatsApp webhook handler (signature verification)
- [ ] Messaging adapter trait + WhatsApp implementation
- [ ] D1: workspace schema + Index DB
- [ ] Dynamic D1 provisioning per workspace
- [ ] Regex parsing for TODO:/DONE:/NOTE:
- [ ] CRUD todos in D1
- [ ] Task cards with reply handling (done/snooze/edit)
- [ ] DONE matching (exact + fuzzy Jaro-Winkler)
- [ ] Basic notes (create, list, search)
- [ ] Deploy Worker to CF

### Phase 2 — Web Workspace (6-8 weeks)
- [ ] WhatsApp OTP auth (Worker + KV)
- [ ] Index DB: phone → workspaces
- [ ] Leptos CSR SPA: shell + routing
- [ ] /dashboard page (workspace list)
- [ ] /w/:slug/todos page (list + inline creation)
- [ ] /w/:slug/notes page (list + read view)
- [ ] Markdown note editor + rich preview (pulldown-cmark WASM)
- [ ] File upload → R2 + inline preview
- [ ] FTS5 search on notes
- [ ] Deploy SPA to CF Pages

### Phase 3 — Intelligence + Product (4-6 weeks)
- [ ] Gemini 2.5 Flash integration for full NLU
- [ ] Claude Haiku 4.5 fallback for complex matching
- [ ] Natural language commands via @grumps
- [ ] Assignment, deadlines, priorities, tags
- [ ] Reminders (Cron Triggers)
- [ ] Automatic recaps
- [ ] Summarize (AI conversation summary)
- [ ] Kanban view with drag & drop
- [ ] Stripe billing (freemium plans + quotas)

### Phase 4 — Scale & Multi-platform (ongoing)
- [ ] Telegram adapter
- [ ] Discord adapter
- [ ] D1 REST API for dynamic resolution (scale > 100 workspaces)
- [ ] Recurring todos
- [ ] Export CSV/JSON/Markdown
- [ ] PWA + push notifications
- [ ] Public API
- [ ] Full i18n (EN, FR, ES, DE, PT)
- [ ] Marketing landing page (GitHub Pages)

---

## 18. Risks & Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Meta Business Verification slow/rejected | Blocking | Start on day 1 + Telegram adapter in parallel |
| LLM costs spike | Financial | Regex fast path (70%), Gemini free tier, hard caps per plan |
| D1 REST API latency (scale) | Performance | Aggressive KV caching, static bindings as long as possible |
| WASM compatibility (Rust crates) | Technical | Test every dependency on wasm32 from the start |
| workers-rs API instability | Technical | Thin abstraction layer, integration tests on each release |
| FTS5 limited for French | Functional | Sufficient for MVP, Meilisearch as option if needed |
| Competitor TaskRio evolves | Market | Web workspace = primary moat |
| WhatsApp OTP cost | Financial | ~$0.05/OTP, negligible |
| Scaling D1 (1 DB per workspace) | Operational | D1 supports 50K DBs/account, REST API for provisioning |
| Gemini API changes/deprecation | Technical | Adapter pattern makes swapping models trivial |

---

## 19. Complete Flow Example

### Scenario A: todo in chat → web view

```
[WhatsApp Group "Roommates"]

Alice: TODO:
       • Buy toilet paper
       • Pay the electricity bill @Bob

→ POST api.grumps.app/webhook/whatsapp (by Meta)
→ Worker verifies signature, parses message
→ Regex detects "TODO:", extracts items
→ Resolves D1 for workspace "x7k9m2p4"
→ INSERT INTO todos (2 rows) + INSERT INTO activity_log
→ Upsert Alice in members + Index DB
→ POST api.whatsapp.com (response):

Grumps: ✅ 2 todos added:
        📋 #23 Buy toilet paper
        📋 #24 Pay the electricity bill → @Bob
        🔗 grumps.app/w/x7k9m2p4

[Bob opens the link]
→ SPA loads, no JWT → /login
→ Enters his number → OTP received on WhatsApp
→ Verified → JWT issued
→ SPA fetch GET api.grumps.app/api/w/x7k9m2p4/todos
→ Worker verifies JWT + membership → queries D1 → JSON
→ Bob sees both todos in the list view
```

### Scenario B: file in chat → web preview

```
Alice: [sends invoice.pdf in the group]
Alice: @grumps store [EDF invoice March]

→ Webhook → Worker detects @mention + media
→ Worker downloads PDF via temporary Meta URL
→ Worker uploads to R2 (key: x7k9m2p4/uuid/invoice.pdf)
→ INSERT INTO files in D1

Grumps: 📎 File stored: EDF invoice March (245 KB)

[Bob opens the workspace → /w/x7k9m2p4/files]
→ SPA fetches file list
→ Clicks invoice → SPA requests signed URL from Worker
→ Worker verifies membership → signs R2 URL (1h)
→ pdf.js renders the PDF inline
```

### Scenario C: natural language via @grumps (LLM flow)

```
Alice: @grumps remind Bob to pay the electricity bill before friday,
       it's getting urgent

→ Webhook → Worker detects @mention
→ Not a structured command → send to Gemini 2.5 Flash
→ Gemini returns JSON:
  {
    "intent": "set_reminder",
    "title": "Pay the electricity bill",
    "target": "Bob",
    "deadline": "2026-04-17T09:00:00",
    "priority": 1
  }
→ Worker creates reminder in D1
→ Worker responds in group:

Grumps: ⏰ Reminder set for @Bob:
        "Pay the electricity bill"
        🔴 High priority — Friday April 17, 9:00 AM
```

---

*Grumps — grumps.app — "Gets it done. No small talk."*
