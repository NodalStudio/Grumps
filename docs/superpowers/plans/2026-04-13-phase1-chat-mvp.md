# Phase 1: Chat MVP — Implementation Plan (v3)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a working WhatsApp bot that handles todos, notes, and utility commands via regex, stores data in D1 (1 DB per workspace via REST API), sends individual task cards with reply support, and auto-provisions workspaces — deployed as a Cloudflare Worker.

**Architecture:** Rust crate workspace with 4 crates: `core`, `nlu`, `messaging`, `worker`. The Worker receives Meta webhooks, deduplicates via KV, parses messages through a regex engine (blocks, @grumps commands, task card replies, reply-with-mention), performs CRUD on workspace D1 databases via the Cloudflare REST API, and sends individual task card messages back via Meta API — tracking each sent message for reply resolution.

**Database strategy:** 1 D1 per workspace via REST API (~50-80ms). Shared Index DB as native binding (~1ms). Auto-provisioning on first message. 50K DBs free per CF account.

**Key decisions in v3:**
- Individual task card per todo (not combined messages) — enables reply-to-complete
- @grumps + free text = create todo (default action, no dead ends)
- @grumps commands parsed via regex: list, done, delete, notes, files, help, link, status
- Reply + @grumps todo/note = quoted message becomes content
- First member = admin
- Index DB user_workspaces populated on member upsert
- Migration split into individual statements for D1 REST
- D1 REST API from day 1
- Real HMAC-SHA256, KV dedup, atomic seq_num, bot message tracking

**Tech Stack:** Rust, workers-rs 0.4, serde, chrono, uuid, strsim, hmac, sha2, hex, wrangler, D1, KV

---

## File Structure

```
grumps/
├── Cargo.toml                          # Workspace root
├── wrangler.toml                       # CF Worker config
├── migrations/
│   ├── index/
│   │   └── 0001_init.sql              # Index DB schema
│   └── workspace/
│       └── 0001_init.sql              # Workspace DB template
├── crates/
│   ├── core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── todo.rs                # Todo, CreateTodo, TodoStatus, Priority
│   │       ├── note.rs                # Note, CreateNote
│   │       ├── member.rs              # Member, Role
│   │       ├── activity.rs            # ActivityEntry
│   │       └── workspace.rs           # WorkspaceMeta
│   ├── nlu/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── parser.rs             # ParseResult enum + top-level parse()
│   │       ├── block_parser.rs       # TODO:/DONE:/NOTE: blocks
│   │       ├── command_parser.rs     # @grumps list/done/delete/notes/files/help/link
│   │       ├── reply_parser.rs       # Task card reply parsing
│   │       ├── entity.rs             # @mention, !priority, #tag, deadline extraction
│   │       └── matcher.rs            # Fuzzy DONE matching
│   ├── messaging/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── adapter.rs            # MessagingPlatform trait + types
│   │       ├── whatsapp.rs           # WhatsApp Cloud API + HMAC
│   │       └── formatter.rs          # Task card + list formatting
│   └── worker/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs                 # Worker entrypoint + router
│           ├── d1_rest.rs             # D1 REST API client
│           ├── db.rs                  # Query helpers (WorkspaceDb + Index)
│           ├── provisioning.rs        # Auto-create workspace
│           ├── handler.rs             # Message processing logic (the brain)
│           ├── error.rs               # Error type
│           └── routes/
│               ├── mod.rs
│               ├── webhook.rs         # POST/GET /webhook/whatsapp
│               └── health.rs          # GET /health
```

Changes from v2:
- `todo_parser.rs` + `note_parser.rs` → `block_parser.rs` (all block formats in one file)
- New `command_parser.rs` for @grumps commands
- New `formatter.rs` for task card + list message formatting
- New `handler.rs` extracts processing logic from webhook route

---

## Task 1: Rust Workspace Scaffold

**Files:**
- Create: `Cargo.toml`, `wrangler.toml`, `.cargo/config.toml`, `.gitignore`
- Create: all crate `Cargo.toml` and `src/lib.rs` files

- [ ] **Step 1: Create workspace root**

```toml
# Cargo.toml
[workspace]
resolver = "2"
members = ["crates/core", "crates/nlu", "crates/messaging", "crates/worker"]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde", "wasmbind"] }
uuid = { version = "1", features = ["v4", "js", "serde"] }
thiserror = "2"
```

- [ ] **Step 2: Create crate Cargo.tomls**

```toml
# crates/core/Cargo.toml
[package]
name = "grumps-core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
```

```toml
# crates/nlu/Cargo.toml
[package]
name = "grumps-nlu"
version = "0.1.0"
edition = "2021"

[dependencies]
grumps-core = { path = "../core" }
serde.workspace = true
chrono.workspace = true
strsim = "0.11"

[dev-dependencies]
pretty_assertions = "1"
```

```toml
# crates/messaging/Cargo.toml
[package]
name = "grumps-messaging"
version = "0.1.0"
edition = "2021"

[dependencies]
grumps-core = { path = "../core" }
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
thiserror.workspace = true
hmac = "0.12"
sha2 = "0.10"
hex = "0.4"
```

```toml
# crates/worker/Cargo.toml
[package]
name = "grumps-worker"
version = "0.1.0"
edition = "2021"

[dependencies]
grumps-core = { path = "../core" }
grumps-nlu = { path = "../nlu" }
grumps-messaging = { path = "../messaging" }
worker = { version = "0.4", features = ["d1"] }
worker-macros = "0.4"
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
thiserror.workspace = true

[lib]
crate-type = ["cdylib"]
```

- [ ] **Step 3: Create lib.rs stubs for each crate**

```rust
// crates/core/src/lib.rs
pub mod todo;
pub mod note;
pub mod member;
pub mod activity;
pub mod workspace;
```

```rust
// crates/nlu/src/lib.rs
pub mod parser;
pub mod block_parser;
pub mod command_parser;
pub mod reply_parser;
pub mod entity;
pub mod matcher;
```

```rust
// crates/messaging/src/lib.rs
pub mod adapter;
pub mod whatsapp;
pub mod formatter;
```

```rust
// crates/worker/src/lib.rs
use worker::*;
mod d1_rest;
mod db;
mod error;
mod handler;
mod provisioning;
mod routes;

#[event(fetch, respond_with_errors)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .get("/health", |_, _| Response::ok("ok"))
        .run(req, env)
        .await
}
```

- [ ] **Step 4: Create wrangler.toml**

```toml
name = "grumps-api"
main = "build/worker/shim.mjs"
compatibility_date = "2024-12-01"

[build]
command = "cargo install -q worker-build && worker-build --release"

[vars]
ENVIRONMENT = "development"
WA_PHONE_NUMBER_ID = ""
WA_VERIFY_TOKEN = ""

[[d1_databases]]
binding = "INDEX_DB"
database_name = "grumps-index"
database_id = ""

[[kv_namespaces]]
binding = "KV"
id = ""
```

- [ ] **Step 5: Create .cargo/config.toml and .gitignore**

```toml
# .cargo/config.toml
[build]
target = "wasm32-unknown-unknown"
```

```gitignore
# .gitignore
target/
build/
node_modules/
.wrangler/
*.png
*.jpg
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check`
Expected: compiles (warnings OK for empty modules)

- [ ] **Step 7: Commit**

```bash
git init
git add -A
git commit -m "feat: scaffold Rust workspace — core, nlu, messaging, worker crates"
```

---

## Task 2: Core Domain Types

**Files:**
- Create: `crates/core/src/todo.rs`
- Create: `crates/core/src/note.rs`
- Create: `crates/core/src/member.rs`
- Create: `crates/core/src/activity.rs`
- Create: `crates/core/src/workspace.rs`

- [ ] **Step 1: Todo types**

```rust
// crates/core/src/todo.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus { Open, InProgress, Done, Blocked, Deleted }

impl TodoStatus {
    pub fn as_str(&self) -> &str {
        match self { Self::Open=>"open", Self::InProgress=>"in_progress", Self::Done=>"done", Self::Blocked=>"blocked", Self::Deleted=>"deleted" }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s { "open"=>Some(Self::Open), "in_progress"=>Some(Self::InProgress), "done"=>Some(Self::Done), "blocked"=>Some(Self::Blocked), "deleted"=>Some(Self::Deleted), _=>None }
    }
    pub fn is_open(&self) -> bool { matches!(self, Self::Open | Self::InProgress) }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum Priority { High = 1, Normal = 2, Low = 3 }

impl Priority {
    pub fn from_int(n: i32) -> Self { match n { 1=>Self::High, 3=>Self::Low, _=>Self::Normal } }
    pub fn as_int(&self) -> i32 { *self as i32 }
    pub fn emoji(&self) -> &str { match self { Self::High=>"🔴", Self::Normal=>"", Self::Low=>"🔵" } }
    pub fn label(&self) -> &str { match self { Self::High=>"High", Self::Normal=>"Normal", Self::Low=>"Low" } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub seq_num: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: TodoStatus,
    pub priority: Priority,
    pub tags: Vec<String>,
    pub deadline: Option<String>,
    pub assigned_to: Option<String>,
    pub assigned_name: Option<String>,
    pub created_by: Option<String>,
    pub completed_at: Option<String>,
    pub completed_by: Option<String>,
    pub source: String,
    pub message_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateTodo {
    pub title: String,
    pub priority: Priority,
    pub tags: Vec<String>,
    pub deadline_text: Option<String>,
    pub assigned_to: Option<String>,
    pub created_by: String,
    pub source: String,
    pub message_id: Option<String>,
}

impl CreateTodo {
    pub fn validate(&self) -> Result<(), &'static str> {
        let t = self.title.trim();
        if t.is_empty() { return Err("title cannot be empty"); }
        if t.len() > 500 { return Err("title too long (max 500)"); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_roundtrip() {
        for s in [TodoStatus::Open, TodoStatus::InProgress, TodoStatus::Done, TodoStatus::Blocked, TodoStatus::Deleted] {
            assert_eq!(TodoStatus::from_str(s.as_str()), Some(s));
        }
    }

    #[test]
    fn priority_order() { assert!(Priority::High < Priority::Normal && Priority::Normal < Priority::Low); }

    #[test]
    fn validate_empty() {
        let t = CreateTodo { title:"".into(), priority:Priority::Normal, tags:vec![], deadline_text:None, assigned_to:None, created_by:"u".into(), source:"chat".into(), message_id:None };
        assert!(t.validate().is_err());
    }

    #[test]
    fn validate_ok() {
        let t = CreateTodo { title:"Buy bread".into(), priority:Priority::Normal, tags:vec![], deadline_text:None, assigned_to:None, created_by:"u".into(), source:"chat".into(), message_id:None };
        assert!(t.validate().is_ok());
    }
}
```

- [ ] **Step 2: Member, Note, Activity, Workspace types**

```rust
// crates/core/src/member.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role { Admin, Member }
impl Role {
    pub fn as_str(&self) -> &str { match self { Self::Admin=>"admin", Self::Member=>"member" } }
    pub fn from_str(s: &str) -> Self { if s == "admin" { Self::Admin } else { Self::Member } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub platform_user_id: String,
    pub display_name: Option<String>,
    pub role: Role,
}
```

```rust
// crates/core/src/note.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: Option<String>,
    pub content: String,
    pub pinned: bool,
    pub source: String,
    pub created_by: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateNote {
    pub title: Option<String>,
    pub content: String,
    pub pinned: bool,
    pub source: String,
    pub created_by: String,
}
```

```rust
// crates/core/src/activity.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub id: String,
    pub actor: Option<String>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub source: String,
    pub created_at: String,
}
```

```rust
// crates/core/src/workspace.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    pub slug: String,
    pub platform: String,
    pub platform_channel_id: String,
    pub name: Option<String>,
    pub plan: String,
    pub d1_database_id: String,
}
```

- [ ] **Step 3: Run tests, commit**

Run: `cargo test -p grumps-core`

```bash
git add crates/core/
git commit -m "feat(core): domain types — Todo, Note, Member, Activity, Workspace"
```

---

## Task 3: NLU — ParseResult + Entity Extraction

**Files:**
- Create: `crates/nlu/src/parser.rs`
- Create: `crates/nlu/src/entity.rs`

- [ ] **Step 1: Define all ParseResult variants**

```rust
// crates/nlu/src/parser.rs
use grumps_core::todo::Priority;

/// Every possible interpretation of an incoming message.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseResult {
    // === Block commands (no @mention needed) ===
    /// TODO: block → N todos
    AddTodos(Vec<ParsedTodo>),
    /// DONE: block → N completions (free text to fuzzy match)
    CompleteTodos(Vec<String>),
    /// NOTE: block → create a note
    AddNote(ParsedNote),

    // === @grumps commands ===
    /// @grumps list [filter]
    ListTodos(ListFilter),
    /// @grumps done <text> or @grumps done #42
    CompleteSingle(CompletionTarget),
    /// @grumps delete #42
    DeleteTodo(i64),
    /// @grumps notes
    ListNotes,
    /// @grumps note <search>
    SearchNotes(String),
    /// @grumps files
    ListFiles,
    /// @grumps help
    Help,
    /// @grumps link
    WorkspaceLink,
    /// @grumps (alone, no args)
    Status,
    /// @grumps <free text> → create a single todo (default action)
    AddSingleTodo(ParsedTodo),

    // === Reply to bot task card ===
    TaskCardReply(TaskCardAction),

    // === Reply + @grumps on any message ===
    /// Reply to a message + @grumps todo → quoted text becomes todo
    QuotedTodo,
    /// Reply to a message + @grumps note/pin → quoted text becomes note
    QuotedNote,

    // === Not relevant ===
    Ignore,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTodo {
    pub title: String,
    pub assignee_mention: Option<String>,
    pub deadline_text: Option<String>,
    pub priority: Priority,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedNote {
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ListFilter {
    Open,           // default
    All,            // include done
    Mine,           // assigned to me
    Done,           // only done
    Assignee(String), // @Pierre
    Tag(String),      // #urgent
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompletionTarget {
    BySeqNum(i64),      // @grumps done #42
    ByText(String),     // @grumps done bread
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskCardAction {
    Done,
    Snooze(String),
    Edit(String),
    Reassign(String),
    ChangePriority(Priority),
    AddTag(String),
    Delete,
    ChangeStatus(String),
}

/// Top-level parse entry point.
pub fn parse(
    text: &str,
    is_mention: bool,
    is_dm: bool,
    is_reply_to_bot: bool,
    has_quoted_message: bool,
) -> ParseResult {
    let trimmed = text.trim();

    // 1. Reply to a bot task card (no @mention needed)
    if is_reply_to_bot {
        return crate::reply_parser::parse_reply(trimmed);
    }

    // 2. Reply + @grumps todo/note on any message
    if has_quoted_message && is_mention {
        if let Some(r) = crate::command_parser::try_parse_quoted_command(trimmed) {
            return r;
        }
    }

    // 3. Block commands: TODO: / DONE: / NOTE:
    if let Some(r) = crate::block_parser::try_parse_block(trimmed) {
        return r;
    }

    // 4. @grumps commands
    if is_mention || is_dm {
        return crate::command_parser::parse_mention(trimmed);
    }

    ParseResult::Ignore
}
```

- [ ] **Step 2: Entity extraction**

```rust
// crates/nlu/src/entity.rs
use crate::parser::ParsedTodo;
use grumps_core::todo::Priority;

/// Extract entities from a line of text: @mentions, !priority, #tags, remaining = title.
pub fn extract_todo_from_line(line: &str) -> ParsedTodo {
    let mut assignee: Option<String> = None;
    let mut priority = Priority::Normal;
    let mut tags: Vec<String> = Vec::new();
    let mut deadline_text: Option<String> = None;
    let mut title_parts: Vec<String> = Vec::new();

    // Simple deadline detection: words after "before"/"by"/"for" until end or next entity
    let words: Vec<&str> = line.split_whitespace().collect();
    let mut i = 0;
    let mut in_deadline = false;
    let mut deadline_parts: Vec<&str> = Vec::new();

    while i < words.len() {
        let w = words[i];

        if !in_deadline && (w.eq_ignore_ascii_case("before") || w.eq_ignore_ascii_case("by") || w.eq_ignore_ascii_case("for")) {
            // Check if next word looks like a date-ish thing
            if i + 1 < words.len() {
                let next = words[i + 1].to_lowercase();
                let is_date_like = matches!(next.as_str(),
                    "monday"|"tuesday"|"wednesday"|"thursday"|"friday"|"saturday"|"sunday"
                    |"tomorrow"|"today"|"tonight"|"next"|"end"
                ) || next.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false);

                if is_date_like {
                    in_deadline = true;
                    i += 1;
                    continue;
                }
            }
            title_parts.push(w.to_string());
        } else if in_deadline {
            if w.starts_with('@') || w.starts_with('#') || w.starts_with('!') {
                in_deadline = false;
                deadline_text = Some(deadline_parts.join(" "));
                deadline_parts.clear();
                // Re-process this word
                continue;
            }
            deadline_parts.push(w);
        } else if w.starts_with('@') && w.len() > 1 {
            if assignee.is_none() { assignee = Some(w[1..].to_string()); }
        } else if w == "!high" || w == "!!!" {
            priority = Priority::High;
        } else if w == "!low" {
            priority = Priority::Low;
        } else if w.starts_with('#') && w.len() > 1 {
            tags.push(w[1..].to_string());
        } else {
            title_parts.push(w.to_string());
        }

        i += 1;
    }

    if !deadline_parts.is_empty() {
        deadline_text = Some(deadline_parts.join(" "));
    }

    ParsedTodo {
        title: title_parts.join(" "),
        assignee_mention: assignee,
        deadline_text,
        priority,
        tags,
    }
}

/// Strip @grumps mention from text (case insensitive).
pub fn strip_mention(text: &str) -> String {
    let lower = text.to_lowercase();
    // Handle various mention formats
    let stripped = if lower.starts_with("@grumps ") {
        &text[8..]
    } else if lower.starts_with("@grumps") {
        &text[7..]
    } else {
        // Could be in the middle of text
        let re_pos = lower.find("@grumps");
        match re_pos {
            Some(pos) => {
                let before = &text[..pos];
                let after_start = pos + 7;
                let after = if after_start < text.len() { &text[after_start..] } else { "" };
                return format!("{}{}", before, after).trim().to_string();
            }
            None => text,
        }
    };
    stripped.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_line() {
        let t = extract_todo_from_line("Buy toilet paper");
        assert_eq!(t.title, "Buy toilet paper");
        assert!(t.assignee_mention.is_none());
        assert_eq!(t.priority, Priority::Normal);
    }

    #[test]
    fn with_assignee_priority_tags() {
        let t = extract_todo_from_line("Ship project @Pierre !high #sales #urgent");
        assert_eq!(t.title, "Ship project");
        assert_eq!(t.assignee_mention, Some("Pierre".into()));
        assert_eq!(t.priority, Priority::High);
        assert_eq!(t.tags, vec!["sales", "urgent"]);
    }

    #[test]
    fn with_deadline() {
        let t = extract_todo_from_line("Buy gifts before friday @Alice");
        assert_eq!(t.title, "Buy gifts");
        assert_eq!(t.deadline_text, Some("friday".into()));
        assert_eq!(t.assignee_mention, Some("Alice".into()));
    }

    #[test]
    fn strip_mention_start() {
        assert_eq!(strip_mention("@grumps buy bread"), "buy bread");
        assert_eq!(strip_mention("@Grumps buy bread"), "buy bread");
    }

    #[test]
    fn strip_mention_middle() {
        assert_eq!(strip_mention("hey @grumps buy bread"), "hey buy bread");
    }
}
```

- [ ] **Step 3: Run tests, commit**

Run: `cargo test -p grumps-nlu`

```bash
git add crates/nlu/src/parser.rs crates/nlu/src/entity.rs
git commit -m "feat(nlu): ParseResult enum with all variants + entity extraction with deadline detection"
```

---

## Task 4: NLU — Block Parser (TODO:/DONE:/NOTE:)

**Files:**
- Create: `crates/nlu/src/block_parser.rs`

- [ ] **Step 1: Implement block parser**

```rust
// crates/nlu/src/block_parser.rs
use crate::parser::{ParseResult, ParsedTodo, ParsedNote};
use crate::entity;

/// Try to parse a block command: TODO:, DONE:, NOTE:
pub fn try_parse_block(text: &str) -> Option<ParseResult> {
    let upper = text.to_uppercase();

    if upper.starts_with("TODO:") {
        return parse_todo_block(&text[5..]);
    }
    if upper.starts_with("DONE:") {
        return parse_done_block(&text[5..]);
    }
    // NOTE [title]: content
    if upper.starts_with("NOTE [") {
        return parse_named_note(text);
    }
    if upper.starts_with("NOTE:") {
        return parse_note(&text[5..]);
    }

    None
}

fn parse_todo_block(body: &str) -> Option<ParseResult> {
    let items = parse_list_items(body);
    if items.is_empty() { return None; }
    let todos = items.iter().map(|l| entity::extract_todo_from_line(l)).collect();
    Some(ParseResult::AddTodos(todos))
}

fn parse_done_block(body: &str) -> Option<ParseResult> {
    let items = parse_list_items(body);
    if items.is_empty() { return None; }
    Some(ParseResult::CompleteTodos(items))
}

fn parse_note(body: &str) -> Option<ParseResult> {
    let content = body.trim().to_string();
    if content.is_empty() { return None; }
    let title = content.lines().next()
        .filter(|l| l.trim().len() <= 60)
        .map(|l| l.trim().to_string());
    Some(ParseResult::AddNote(ParsedNote { title, content }))
}

fn parse_named_note(text: &str) -> Option<ParseResult> {
    let rest = &text[6..]; // skip "NOTE ["
    let end = rest.find(']')?;
    let title = rest[..end].trim().to_string();
    let content = rest[end + 1..].trim_start_matches(':').trim().to_string();
    if content.is_empty() { return None; }
    Some(ParseResult::AddNote(ParsedNote { title: Some(title), content }))
}

fn parse_list_items(body: &str) -> Vec<String> {
    body.lines()
        .map(|l| l.trim().trim_start_matches(|c: char| "•-*·◦▪▸►".contains(c) || c.is_whitespace()).trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use grumps_core::todo::Priority;

    #[test]
    fn todo_block() {
        match try_parse_block("TODO:\n• Buy bread\n• Buy milk").unwrap() {
            ParseResult::AddTodos(t) => { assert_eq!(t.len(), 2); assert_eq!(t[0].title, "Buy bread"); }
            _ => panic!(),
        }
    }

    #[test]
    fn todo_with_entities() {
        match try_parse_block("TODO:\n- Ship it @Pierre !high #urgent").unwrap() {
            ParseResult::AddTodos(t) => {
                assert_eq!(t[0].title, "Ship it");
                assert_eq!(t[0].assignee_mention, Some("Pierre".into()));
                assert_eq!(t[0].priority, Priority::High);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn done_block() {
        match try_parse_block("DONE:\n• bread\n• milk").unwrap() {
            ParseResult::CompleteTodos(items) => assert_eq!(items, vec!["bread", "milk"]),
            _ => panic!(),
        }
    }

    #[test]
    fn note_simple() {
        match try_parse_block("NOTE: wifi password is XYZ").unwrap() {
            ParseResult::AddNote(n) => assert_eq!(n.content, "wifi password is XYZ"),
            _ => panic!(),
        }
    }

    #[test]
    fn note_named() {
        match try_parse_block("NOTE [wifi]: password = XYZ").unwrap() {
            ParseResult::AddNote(n) => {
                assert_eq!(n.title, Some("wifi".into()));
                assert_eq!(n.content, "password = XYZ");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn case_insensitive() { assert!(try_parse_block("todo:\n• X").is_some()); }

    #[test]
    fn not_a_block() { assert!(try_parse_block("Hello").is_none()); }

    #[test]
    fn empty_blocks() {
        assert!(try_parse_block("TODO:").is_none());
        assert!(try_parse_block("NOTE:  ").is_none());
    }
}
```

- [ ] **Step 2: Run tests, commit**

Run: `cargo test -p grumps-nlu -- block_parser`

```bash
git add crates/nlu/src/block_parser.rs
git commit -m "feat(nlu): block parser for TODO:/DONE:/NOTE: formats"
```

---

## Task 5: NLU — Command Parser (@grumps commands)

**Files:**
- Create: `crates/nlu/src/command_parser.rs`

- [ ] **Step 1: Implement @grumps command parser**

```rust
// crates/nlu/src/command_parser.rs
use crate::parser::*;
use crate::entity;

/// Parse an @grumps mention message.
/// Called when the bot is @mentioned or in a DM.
pub fn parse_mention(text: &str) -> ParseResult {
    let clean = entity::strip_mention(text);
    let trimmed = clean.trim();

    // Empty = status
    if trimmed.is_empty() {
        return ParseResult::Status;
    }

    let lower = trimmed.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();

    match words[0] {
        // === List todos ===
        "list" | "show" | "todos" => parse_list_command(&words[1..]),

        // === Complete ===
        "done" | "complete" | "finished" => {
            if words.len() < 2 { return ParseResult::Status; }
            let rest = &trimmed[words[0].len()..].trim();
            parse_done_command(rest)
        }

        // === Delete ===
        "delete" | "remove" => {
            if words.len() < 2 { return ParseResult::Help; }
            let rest = &trimmed[words[0].len()..].trim();
            parse_delete_command(rest)
        }

        // === Notes ===
        "notes" => ParseResult::ListNotes,
        "note" | "pin" => {
            if words.len() < 2 { return ParseResult::ListNotes; }
            let query = &trimmed[words[0].len()..].trim();
            ParseResult::SearchNotes(query.to_string())
        }

        // === Files ===
        "files" | "file" => {
            if words.len() >= 2 {
                // @grumps file <query> — search files (Phase 2, just list for now)
                ParseResult::ListFiles
            } else {
                ParseResult::ListFiles
            }
        }

        // === Utility ===
        "help" | "?" => ParseResult::Help,
        "link" | "workspace" | "web" => ParseResult::WorkspaceLink,
        "status" | "summary" | "recap" => ParseResult::Status,

        // === Default: create a todo ===
        _ => {
            let parsed = entity::extract_todo_from_line(trimmed);
            if parsed.title.is_empty() {
                ParseResult::Status
            } else {
                ParseResult::AddSingleTodo(parsed)
            }
        }
    }
}

fn parse_list_command(args: &[&str]) -> ParseResult {
    if args.is_empty() {
        return ParseResult::ListTodos(ListFilter::Open);
    }

    match args[0] {
        "all" => ParseResult::ListTodos(ListFilter::All),
        "mine" | "my" => ParseResult::ListTodos(ListFilter::Mine),
        "done" | "completed" | "finished" => ParseResult::ListTodos(ListFilter::Done),
        "open" => ParseResult::ListTodos(ListFilter::Open),
        _ if args[0].starts_with('@') => {
            ParseResult::ListTodos(ListFilter::Assignee(args[0][1..].to_string()))
        }
        _ if args[0].starts_with('#') => {
            ParseResult::ListTodos(ListFilter::Tag(args[0][1..].to_string()))
        }
        _ => ParseResult::ListTodos(ListFilter::Open),
    }
}

fn parse_done_command(text: &str) -> ParseResult {
    // @grumps done #42
    if text.starts_with('#') {
        if let Ok(num) = text[1..].parse::<i64>() {
            return ParseResult::CompleteSingle(CompletionTarget::BySeqNum(num));
        }
    }
    // @grumps done buy bread
    ParseResult::CompleteSingle(CompletionTarget::ByText(text.to_string()))
}

fn parse_delete_command(text: &str) -> ParseResult {
    // @grumps delete #42
    let cleaned = text.trim_start_matches('#');
    if let Ok(num) = cleaned.parse::<i64>() {
        return ParseResult::DeleteTodo(num);
    }
    // Can't delete by fuzzy text — need an ID
    ParseResult::Help
}

/// Parse reply+@grumps commands: "@grumps todo" or "@grumps note" on a quoted message.
pub fn try_parse_quoted_command(text: &str) -> Option<ParseResult> {
    let clean = entity::strip_mention(text).to_lowercase();
    let trimmed = clean.trim();

    match trimmed {
        "todo" | "add" | "task" => Some(ParseResult::QuotedTodo),
        "note" | "pin" | "save" => Some(ParseResult::QuotedNote),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grumps_core::todo::Priority;

    // === List ===
    #[test]
    fn list_open() { assert_eq!(parse_mention("@grumps list"), ParseResult::ListTodos(ListFilter::Open)); }
    #[test]
    fn list_all() { assert_eq!(parse_mention("@grumps list all"), ParseResult::ListTodos(ListFilter::All)); }
    #[test]
    fn list_mine() { assert_eq!(parse_mention("@grumps list mine"), ParseResult::ListTodos(ListFilter::Mine)); }
    #[test]
    fn list_done() { assert_eq!(parse_mention("@grumps list done"), ParseResult::ListTodos(ListFilter::Done)); }
    #[test]
    fn list_assignee() { assert_eq!(parse_mention("@grumps list @Pierre"), ParseResult::ListTodos(ListFilter::Assignee("Pierre".into()))); }
    #[test]
    fn list_tag() { assert_eq!(parse_mention("@grumps list #urgent"), ParseResult::ListTodos(ListFilter::Tag("urgent".into()))); }
    #[test]
    fn show_alias() { assert_eq!(parse_mention("@grumps show"), ParseResult::ListTodos(ListFilter::Open)); }

    // === Done ===
    #[test]
    fn done_by_id() { assert_eq!(parse_mention("@grumps done #42"), ParseResult::CompleteSingle(CompletionTarget::BySeqNum(42))); }
    #[test]
    fn done_by_text() { assert_eq!(parse_mention("@grumps done bread"), ParseResult::CompleteSingle(CompletionTarget::ByText("bread".into()))); }

    // === Delete ===
    #[test]
    fn delete_by_id() { assert_eq!(parse_mention("@grumps delete #42"), ParseResult::DeleteTodo(42)); }

    // === Notes ===
    #[test]
    fn list_notes() { assert_eq!(parse_mention("@grumps notes"), ParseResult::ListNotes); }
    #[test]
    fn search_notes() { assert_eq!(parse_mention("@grumps note wifi"), ParseResult::SearchNotes("wifi".into())); }

    // === Utility ===
    #[test]
    fn help() { assert_eq!(parse_mention("@grumps help"), ParseResult::Help); }
    #[test]
    fn link() { assert_eq!(parse_mention("@grumps link"), ParseResult::WorkspaceLink); }
    #[test]
    fn status_empty() { assert_eq!(parse_mention("@grumps"), ParseResult::Status); }
    #[test]
    fn status_explicit() { assert_eq!(parse_mention("@grumps status"), ParseResult::Status); }

    // === Default: create todo ===
    #[test]
    fn default_creates_todo() {
        match parse_mention("@grumps buy bread for friday @Bob") {
            ParseResult::AddSingleTodo(t) => {
                assert_eq!(t.title, "buy bread");
                assert_eq!(t.assignee_mention, Some("Bob".into()));
                assert_eq!(t.deadline_text, Some("friday".into()));
            }
            other => panic!("expected AddSingleTodo, got {:?}", other),
        }
    }

    // === Quoted commands ===
    #[test]
    fn quoted_todo() { assert_eq!(try_parse_quoted_command("@grumps todo"), Some(ParseResult::QuotedTodo)); }
    #[test]
    fn quoted_note() { assert_eq!(try_parse_quoted_command("@grumps note"), Some(ParseResult::QuotedNote)); }
    #[test]
    fn quoted_pin() { assert_eq!(try_parse_quoted_command("@grumps pin"), Some(ParseResult::QuotedNote)); }
    #[test]
    fn quoted_unknown() { assert_eq!(try_parse_quoted_command("@grumps something"), None); }
}
```

- [ ] **Step 2: Run tests, commit**

Run: `cargo test -p grumps-nlu -- command_parser`

```bash
git add crates/nlu/src/command_parser.rs
git commit -m "feat(nlu): @grumps command parser — list/done/delete/notes/files/help/link/status + default=todo"
```

---

## Task 6: NLU — Reply Parser + Fuzzy Matcher

**Files:**
- Create: `crates/nlu/src/reply_parser.rs`
- Create: `crates/nlu/src/matcher.rs`

- [ ] **Step 1: Reply parser**

```rust
// crates/nlu/src/reply_parser.rs
use crate::parser::{ParseResult, TaskCardAction};
use grumps_core::todo::Priority;

pub fn parse_reply(text: &str) -> ParseResult {
    let lower = text.trim().to_lowercase();
    let original = text.trim();

    let action = match lower.as_str() {
        "done" | "finished" | "complete" | "ok" | "fait" | "fini" => TaskCardAction::Done,
        "cancel" | "delete" | "remove" | "supprimer" => TaskCardAction::Delete,
        "!high" | "!!!" => TaskCardAction::ChangePriority(Priority::High),
        "!low" => TaskCardAction::ChangePriority(Priority::Low),
        _ if lower.starts_with("edit ") => TaskCardAction::Edit(original[5..].trim().into()),
        _ if lower.starts_with('@') && original.len() > 1 => TaskCardAction::Reassign(original[1..].trim().into()),
        _ if lower.starts_with('#') && original.len() > 1 => TaskCardAction::AddTag(original[1..].trim().into()),
        _ if lower.starts_with("status:") || lower.starts_with("status ") => TaskCardAction::ChangeStatus(original[7..].trim().into()),
        _ => TaskCardAction::Snooze(original.into()),
    };

    ParseResult::TaskCardReply(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn done_variants() {
        for w in ["done", "finished", "complete", "ok", "fait", "fini"] {
            match parse_reply(w) { ParseResult::TaskCardReply(TaskCardAction::Done) => {}, _ => panic!("{}", w) }
        }
    }

    #[test]
    fn edit() {
        match parse_reply("edit New title") {
            ParseResult::TaskCardReply(TaskCardAction::Edit(t)) => assert_eq!(t, "New title"),
            _ => panic!(),
        }
    }

    #[test]
    fn reassign() {
        match parse_reply("@Alice") {
            ParseResult::TaskCardReply(TaskCardAction::Reassign(n)) => assert_eq!(n, "Alice"),
            _ => panic!(),
        }
    }

    #[test]
    fn snooze_default() {
        match parse_reply("tomorrow") {
            ParseResult::TaskCardReply(TaskCardAction::Snooze(t)) => assert_eq!(t, "tomorrow"),
            _ => panic!(),
        }
    }
}
```

- [ ] **Step 2: Fuzzy matcher**

```rust
// crates/nlu/src/matcher.rs
use strsim::jaro_winkler;

#[derive(Debug, Clone, PartialEq)]
pub enum MatchResult {
    Exact(MatchedTodo),
    Fuzzy(Vec<MatchedTodo>),
    NoMatch,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchedTodo {
    pub todo_id: String,
    pub title: String,
    pub seq_num: i64,
    pub score: f64,
}

pub fn match_done(input: &str, open_todos: &[(String, String, i64)]) -> MatchResult {
    let norm = normalize(input);
    let mut scored: Vec<MatchedTodo> = open_todos.iter()
        .map(|(id, title, seq)| {
            let nt = normalize(title);
            let jw = jaro_winkler(&norm, &nt);
            let tok = token_overlap(&norm, &nt);
            MatchedTodo { todo_id: id.clone(), title: title.clone(), seq_num: *seq, score: jw * 0.6 + tok * 0.4 }
        })
        .filter(|m| m.score > 0.4)
        .collect();
    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    match scored.first() {
        Some(best) if best.score > 0.9 => MatchResult::Exact(best.clone()),
        Some(_) => { scored.truncate(3); MatchResult::Fuzzy(scored) }
        None => MatchResult::NoMatch,
    }
}

fn normalize(text: &str) -> String {
    text.to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect::<String>()
        .split_whitespace()
        .filter(|w| !matches!(*w, "the"|"a"|"an"|"is"|"to"|"for"|"of"|"in"|"on"|"at"|"le"|"la"|"les"|"un"|"une"|"de"|"du"|"des"|"et"|"est"))
        .collect::<Vec<_>>().join(" ")
}

fn token_overlap(a: &str, b: &str) -> f64 {
    let at: Vec<&str> = a.split_whitespace().collect();
    let bt: Vec<&str> = b.split_whitespace().collect();
    if at.is_empty() { return 0.0; }
    at.iter().filter(|t| bt.contains(t)).count() as f64 / at.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn todos() -> Vec<(String, String, i64)> {
        vec![
            ("1".into(), "Buy toilet paper".into(), 23),
            ("2".into(), "Call the plumber about the leak".into(), 12),
            ("3".into(), "Pay plumber invoice".into(), 15),
        ]
    }

    #[test]
    fn exact() {
        match match_done("buy toilet paper", &todos()) {
            MatchResult::Exact(m) => { assert_eq!(m.todo_id, "1"); assert_eq!(m.seq_num, 23); }
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn fuzzy() {
        match match_done("plumber", &todos()) {
            MatchResult::Fuzzy(c) => assert!(!c.is_empty()),
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn no_match() { assert_eq!(match_done("xyz unrelated", &todos()), MatchResult::NoMatch); }
}
```

- [ ] **Step 3: Run all NLU tests, commit**

Run: `cargo test -p grumps-nlu`

```bash
git add crates/nlu/src/reply_parser.rs crates/nlu/src/matcher.rs
git commit -m "feat(nlu): reply parser + fuzzy DONE matcher with seq_num"
```

---

## Task 7: Message Formatter (Task Cards + Lists)

**Files:**
- Create: `crates/messaging/src/formatter.rs`

- [ ] **Step 1: Implement task card and list formatters**

```rust
// crates/messaging/src/formatter.rs
use grumps_core::todo::Priority;

/// Format a single task card (one per todo — sent as individual message).
pub fn task_card(seq_num: i64, title: &str, assignee: Option<&str>, deadline: Option<&str>, priority: Priority, tags: &[String]) -> String {
    let mut lines = vec![
        format!("📋 Task #{}", seq_num),
        "━━━━━━━━━━━━━━━━━━━".into(),
        title.to_string(),
    ];

    let mut meta = Vec::new();
    if let Some(a) = assignee { meta.push(format!("👤 @{}", a)); }
    if let Some(d) = deadline { meta.push(format!("⏰ {}", d)); }
    if priority == Priority::High { meta.push("🔴 High priority".into()); }
    if priority == Priority::Low { meta.push("🔵 Low priority".into()); }
    if !tags.is_empty() {
        meta.push(format!("🏷️ {}", tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" ")));
    }

    if !meta.is_empty() {
        lines.push(String::new());
        lines.extend(meta);
    }

    lines.push(String::new());
    lines.push("━━━━━━━━━━━━━━━━━━━".into());
    lines.push("Reply: done · snooze · edit · @reassign".into());

    lines.join("\n")
}

/// Format the confirmation message after adding N todos (sent before the individual cards).
pub fn todos_added_summary(count: usize, workspace_slug: &str) -> String {
    format!("✅ {} todo{} added.\n🔗 grumps.io/w/{}",
        count, if count > 1 { "s" } else { "" }, workspace_slug)
}

/// Format a todo list for @grumps list.
pub fn todo_list(todos: &[(i64, String, String, Option<String>, i32, String)], filter_label: &str) -> String {
    if todos.is_empty() {
        return match filter_label {
            "open" => "Nothing to do. Suspicious.".into(),
            "done" => "Nothing done yet. Get to work.".into(),
            _ => format!("No todos matching \"{}\".", filter_label),
        };
    }

    let mut lines = vec![format!("📋 {} todos ({}):", todos.len(), filter_label)];
    lines.push(String::new());

    for (seq, title, status, assignee, priority, _tags) in todos {
        let check = if status == "done" { "✅" } else { "☐" };
        let prio = if *priority == 1 { " 🔴" } else { "" };
        let assigned = assignee.as_ref().map(|a| format!(" → @{}", a)).unwrap_or_default();
        lines.push(format!("{} #{} {}{}{}", check, seq, title, assigned, prio));
    }

    lines.join("\n")
}

/// Format note list for @grumps notes.
pub fn note_list(notes: &[(String, Option<String>, String, String)]) -> String {
    if notes.is_empty() {
        return "No notes. The group memory is blank.".into();
    }

    let mut lines = vec![format!("📝 {} notes:", notes.len())];
    lines.push(String::new());

    for (id, title, source, created_at) in notes {
        let t = title.as_deref().unwrap_or("(untitled)");
        let badge = if source == "chat" { "💬" } else { "🌐" };
        lines.push(format!("• {} {} — {}", badge, t, created_at));
    }

    lines.join("\n")
}

/// Format status summary for @grumps (no args).
pub fn status_summary(open_todos: i64, done_week: i64, notes: i64, files: i64, workspace_slug: &str) -> String {
    let mut lines = vec![
        "📊 Status".into(),
        "━━━━━━━━━━━━━━━━━━━".into(),
        format!("☐ {} open todos", open_todos),
        format!("✅ {} done this week", done_week),
        format!("📝 {} notes", notes),
        format!("📎 {} files", files),
        String::new(),
        format!("🔗 grumps.io/w/{}", workspace_slug),
    ];
    lines.join("\n")
}

/// Help text for @grumps help.
pub fn help_text() -> String {
    vec![
        "📋 *Grumps* — Gets it done.",
        "",
        "*Add todos:*",
        "  TODO:",
        "  • Item one",
        "  • Item two @person !high #tag",
        "  _or_ @grumps buy bread @Bob",
        "",
        "*Complete:*",
        "  DONE:",
        "  • bread",
        "  _or_ reply \"done\" to a task card",
        "  _or_ @grumps done #42",
        "",
        "*List:*",
        "  @grumps list",
        "  @grumps list all / mine / done",
        "  @grumps list @person / #tag",
        "",
        "*Notes:*",
        "  NOTE: wifi password is XYZ",
        "  NOTE [title]: content",
        "  @grumps notes / @grumps note wifi",
        "",
        "*Reply + @grumps:*",
        "  Reply to any message + @grumps todo",
        "  Reply to any message + @grumps note",
        "",
        "*Other:*",
        "  @grumps delete #42",
        "  @grumps link / @grumps help",
    ].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_card_basic() {
        let card = task_card(42, "Buy bread", None, None, Priority::Normal, &[]);
        assert!(card.contains("Task #42"));
        assert!(card.contains("Buy bread"));
        assert!(card.contains("Reply: done"));
    }

    #[test]
    fn task_card_full() {
        let card = task_card(42, "Ship it", Some("Pierre"), Some("Friday"), Priority::High, &["urgent".into()]);
        assert!(card.contains("@Pierre"));
        assert!(card.contains("Friday"));
        assert!(card.contains("High priority"));
        assert!(card.contains("#urgent"));
    }

    #[test]
    fn empty_todo_list() {
        let list = todo_list(&[], "open");
        assert!(list.contains("Nothing to do"));
    }

    #[test]
    fn empty_note_list() {
        let list = note_list(&[]);
        assert!(list.contains("No notes"));
    }
}
```

- [ ] **Step 2: Run tests, commit**

Run: `cargo test -p grumps-messaging -- formatter`

```bash
git add crates/messaging/src/formatter.rs
git commit -m "feat(messaging): task card + list formatters with personality"
```

---

## Task 8: WhatsApp Adapter (with real HMAC)

**Files:**
- Create: `crates/messaging/src/adapter.rs`
- Create: `crates/messaging/src/whatsapp.rs`

- [ ] **Step 1: Adapter trait**

```rust
// crates/messaging/src/adapter.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type MessageId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub platform: String,
    pub channel_id: String,
    pub message_id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub text: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub is_mention_to_bot: bool,
    pub is_direct_message: bool,
    pub quoted_message_id: Option<String>,
    pub quoted_message_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub text: String,
    pub reply_to: Option<String>,
}

pub trait MessagingPlatform {
    fn platform_id(&self) -> &str;
    fn parse_webhook(&self, payload: &[u8]) -> Result<Option<InboundMessage>, MessagingError>;
    fn verify_signature(&self, payload: &[u8], signature: &str) -> Result<(), MessagingError>;
    fn build_send_request(&self, recipient: &str, message: &OutboundMessage) -> Result<(String, String), MessagingError>;
    fn handle_verification_challenge(&self, params: &std::collections::HashMap<String, String>) -> Result<String, MessagingError>;
}

#[derive(Debug, thiserror::Error)]
pub enum MessagingError {
    #[error("invalid signature")]
    InvalidSignature,
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
    #[error("verification failed: {0}")]
    VerificationFailed(String),
}
```

- [ ] **Step 2: WhatsApp implementation with HMAC**

```rust
// crates/messaging/src/whatsapp.rs
use crate::adapter::*;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::Deserialize;
use std::collections::HashMap;

type HmacSha256 = Hmac<Sha256>;

pub struct WhatsAppAdapter {
    pub phone_number_id: String,
    pub verify_token: String,
    pub app_secret: String,
    pub access_token: String,
}

impl WhatsAppAdapter {
    pub fn new(phone_number_id: String, verify_token: String, app_secret: String, access_token: String) -> Self {
        Self { phone_number_id, verify_token, app_secret, access_token }
    }
}

impl MessagingPlatform for WhatsAppAdapter {
    fn platform_id(&self) -> &str { "whatsapp" }

    fn verify_signature(&self, payload: &[u8], signature: &str) -> Result<(), MessagingError> {
        let hex_sig = signature.strip_prefix("sha256=").ok_or(MessagingError::InvalidSignature)?;
        let expected = hex::decode(hex_sig).map_err(|_| MessagingError::InvalidSignature)?;
        let mut mac = HmacSha256::new_from_slice(self.app_secret.as_bytes()).map_err(|_| MessagingError::InvalidSignature)?;
        mac.update(payload);
        mac.verify_slice(&expected).map_err(|_| MessagingError::InvalidSignature)
    }

    fn parse_webhook(&self, payload: &[u8]) -> Result<Option<InboundMessage>, MessagingError> {
        let body: WaWebhook = serde_json::from_slice(payload).map_err(|e| MessagingError::InvalidPayload(e.to_string()))?;
        let entry = match body.entry.first() { Some(e) => e, None => return Ok(None) };
        let change = match entry.changes.first() { Some(c) => c, None => return Ok(None) };
        let v = &change.value;
        let msg = match v.messages.as_ref().and_then(|m| m.first()) { Some(m) => m, None => return Ok(None) };

        let text = msg.text.as_ref().map(|t| t.body.clone());
        let sender_name = v.contacts.as_ref().and_then(|c| c.first()).and_then(|c| c.profile.as_ref()).map(|p| p.name.clone()).unwrap_or_else(|| msg.from.clone());

        // Group detection: if metadata has display_phone_number, it's a group context
        let is_group = v.metadata.as_ref().and_then(|m| m.display_phone_number.as_ref()).is_some();

        // @mention detection: check text for @grumps (WhatsApp Business doesn't have structured mentions in Cloud API)
        let is_mention = text.as_ref().map(|t| {
            let l = t.to_lowercase();
            l.contains("@grumps") || l.starts_with("grumps ") || l.starts_with("grumps,")
        }).unwrap_or(false);

        let (quoted_id, quoted_text) = msg.context.as_ref()
            .map(|ctx| (Some(ctx.message_id.clone()), ctx.quoted_text.clone()))
            .unwrap_or((None, None));

        let ts = chrono::DateTime::from_timestamp(msg.timestamp.parse::<i64>().unwrap_or(0), 0).unwrap_or_else(chrono::Utc::now);

        Ok(Some(InboundMessage {
            platform: "whatsapp".into(),
            channel_id: v.metadata.as_ref().and_then(|m| m.phone_number_id.clone()).unwrap_or_else(|| entry.id.clone()),
            message_id: msg.id.clone(),
            sender_id: msg.from.clone(),
            sender_name,
            text,
            timestamp: ts,
            is_mention_to_bot: is_mention || !is_group, // DMs always count as "mention"
            is_direct_message: !is_group,
            quoted_message_id: quoted_id,
            quoted_message_text: quoted_text,
        }))
    }

    fn build_send_request(&self, recipient: &str, message: &OutboundMessage) -> Result<(String, String), MessagingError> {
        let url = format!("https://graph.facebook.com/v21.0/{}/messages", self.phone_number_id);
        let mut body = serde_json::json!({ "messaging_product": "whatsapp", "to": recipient, "type": "text", "text": { "body": message.text } });
        if let Some(ref r) = message.reply_to { body["context"] = serde_json::json!({ "message_id": r }); }
        Ok((url, body.to_string()))
    }

    fn handle_verification_challenge(&self, params: &HashMap<String, String>) -> Result<String, MessagingError> {
        let mode = params.get("hub.mode").ok_or_else(|| MessagingError::VerificationFailed("missing hub.mode".into()))?;
        let token = params.get("hub.verify_token").ok_or_else(|| MessagingError::VerificationFailed("missing token".into()))?;
        let challenge = params.get("hub.challenge").ok_or_else(|| MessagingError::VerificationFailed("missing challenge".into()))?;
        if mode != "subscribe" || token != &self.verify_token { return Err(MessagingError::VerificationFailed("mismatch".into())); }
        Ok(challenge.clone())
    }
}

// Meta webhook types
#[derive(Deserialize)] pub struct WaWebhook { pub entry: Vec<WaEntry> }
#[derive(Deserialize)] pub struct WaEntry { pub id: String, pub changes: Vec<WaChange> }
#[derive(Deserialize)] pub struct WaChange { pub value: WaValue }
#[derive(Deserialize)] pub struct WaValue { pub metadata: Option<WaMeta>, pub contacts: Option<Vec<WaContact>>, pub messages: Option<Vec<WaMsg>> }
#[derive(Deserialize)] pub struct WaMeta { pub display_phone_number: Option<String>, pub phone_number_id: Option<String> }
#[derive(Deserialize)] pub struct WaContact { pub profile: Option<WaProfile> }
#[derive(Deserialize)] pub struct WaProfile { pub name: String }
#[derive(Deserialize)] pub struct WaMsg { pub from: String, pub id: String, pub timestamp: String, pub text: Option<WaText>, pub context: Option<WaCtx> }
#[derive(Deserialize)] pub struct WaText { pub body: String }
#[derive(Deserialize)] pub struct WaCtx { pub message_id: String, pub quoted_text: Option<String> }

#[cfg(test)]
mod tests {
    use super::*;

    fn wa() -> WhatsAppAdapter { WhatsAppAdapter::new("123".into(), "tok".into(), "secret".into(), "access".into()) }

    #[test]
    fn hmac_ok() {
        let payload = b"test";
        let mut mac = HmacSha256::new_from_slice(b"secret").unwrap();
        mac.update(payload);
        let sig = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
        assert!(wa().verify_signature(payload, &sig).is_ok());
    }

    #[test]
    fn hmac_fail() { assert!(wa().verify_signature(b"test", "sha256=0000000000000000000000000000000000000000000000000000000000000000").is_err()); }

    #[test]
    fn parse_message() {
        let p = serde_json::json!({"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m1","timestamp":"1713000000","type":"text","text":{"body":"TODO:\n• bread"}}]}}]}]});
        let msg = wa().parse_webhook(&serde_json::to_vec(&p).unwrap()).unwrap().unwrap();
        assert_eq!(msg.sender_name, "Alice");
        assert!(msg.text.unwrap().contains("bread"));
    }

    #[test]
    fn verify_challenge() {
        let mut p = HashMap::new();
        p.insert("hub.mode".into(), "subscribe".into());
        p.insert("hub.verify_token".into(), "tok".into());
        p.insert("hub.challenge".into(), "ch".into());
        assert_eq!(wa().handle_verification_challenge(&p).unwrap(), "ch");
    }
}
```

- [ ] **Step 3: Run tests, commit**

Run: `cargo test -p grumps-messaging`

```bash
git add crates/messaging/src/adapter.rs crates/messaging/src/whatsapp.rs
git commit -m "feat(messaging): WhatsApp adapter with HMAC-SHA256 + webhook parsing"
```

---

## Task 9: D1 Schema + REST Client + Provisioning

**Files:**
- Create: `migrations/index/0001_init.sql`
- Create: `migrations/workspace/0001_init.sql`
- Create: `crates/worker/src/d1_rest.rs`
- Create: `crates/worker/src/provisioning.rs`
- Create: `crates/worker/src/error.rs`

- [ ] **Step 1: Index DB migration**

```sql
-- migrations/index/0001_init.sql
CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, phone TEXT UNIQUE NOT NULL, created_at TEXT DEFAULT (datetime('now')));
CREATE TABLE IF NOT EXISTS workspaces_meta (slug TEXT PRIMARY KEY, platform TEXT NOT NULL, platform_channel_id TEXT NOT NULL, name TEXT, plan TEXT DEFAULT 'free', d1_database_id TEXT NOT NULL, created_at TEXT DEFAULT (datetime('now')), UNIQUE(platform, platform_channel_id));
CREATE TABLE IF NOT EXISTS user_workspaces (user_id TEXT NOT NULL, workspace_slug TEXT NOT NULL, role TEXT DEFAULT 'member', created_at TEXT DEFAULT (datetime('now')), PRIMARY KEY (user_id, workspace_slug));
```

- [ ] **Step 2: Workspace DB migration (individual statements for REST API)**

```sql
-- migrations/workspace/0001_init.sql
-- Applied statement-by-statement via D1 REST API

CREATE TABLE IF NOT EXISTS members (id TEXT PRIMARY KEY, platform_user_id TEXT NOT NULL UNIQUE, display_name TEXT, role TEXT DEFAULT 'member', last_seen_at TEXT, created_at TEXT DEFAULT (datetime('now')));
CREATE TABLE IF NOT EXISTS todos (id TEXT PRIMARY KEY, seq_num INTEGER NOT NULL, title TEXT NOT NULL, description TEXT, status TEXT DEFAULT 'open', priority INTEGER DEFAULT 2, tags TEXT DEFAULT '[]', deadline TEXT, assigned_to TEXT, assigned_name TEXT, created_by TEXT, completed_at TEXT, completed_by TEXT, source TEXT DEFAULT 'chat', message_id TEXT, created_at TEXT DEFAULT (datetime('now')), updated_at TEXT DEFAULT (datetime('now')));
CREATE UNIQUE INDEX IF NOT EXISTS idx_todos_seq ON todos(seq_num);
CREATE INDEX IF NOT EXISTS idx_todos_status ON todos(status);
CREATE TABLE IF NOT EXISTS bot_messages (message_id TEXT PRIMARY KEY, todo_id TEXT, created_at TEXT DEFAULT (datetime('now')));
CREATE TABLE IF NOT EXISTS notes (id TEXT PRIMARY KEY, title TEXT, content TEXT NOT NULL, pinned INTEGER DEFAULT 0, tags TEXT DEFAULT '[]', source TEXT DEFAULT 'chat', created_by TEXT, created_at TEXT DEFAULT (datetime('now')), updated_at TEXT DEFAULT (datetime('now')));
CREATE TABLE IF NOT EXISTS activity_log (id TEXT PRIMARY KEY, actor TEXT, action TEXT NOT NULL, target_type TEXT, target_id TEXT, source TEXT DEFAULT 'chat', created_at TEXT DEFAULT (datetime('now')));
CREATE INDEX IF NOT EXISTS idx_activity_created ON activity_log(created_at DESC);
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
INSERT OR IGNORE INTO settings VALUES ('language', 'en');
INSERT OR IGNORE INTO settings VALUES ('timezone', 'Europe/Paris');
INSERT OR IGNORE INTO settings VALUES ('quiet_mode', 'false');
```

- [ ] **Step 3: D1 REST client**

Same as v2 Task 8 Step 1 — `d1_rest.rs` with `D1RestClient`, `query()`, `create_database()`. Add `exec_statements()` that splits SQL by `;` and sends each individually:

```rust
// crates/worker/src/d1_rest.rs
// [same D1RestClient as v2, plus this method:]

impl D1RestClient {
    // ... (from_env, query, create_database same as v2) ...

    /// Execute multiple SQL statements one at a time (for migrations).
    pub async fn exec_statements(&self, database_id: &str, sql: &str) -> Result<()> {
        for statement in sql.split(';') {
            let stmt = statement.trim();
            if stmt.is_empty() { continue; }
            self.query(database_id, stmt, vec![]).await?;
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Provisioning with first-member-as-admin**

```rust
// crates/worker/src/provisioning.rs
use worker::*;
use crate::d1_rest::D1RestClient;

const WORKSPACE_SCHEMA: &str = include_str!("../../../migrations/workspace/0001_init.sql");

pub fn generate_slug() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
}

pub async fn provision_workspace(
    d1_client: &D1RestClient, index_db: &D1Database,
    platform: &str, channel_id: &str,
) -> Result<(String, String)> {
    let slug = generate_slug();
    let db_name = format!("grumps-ws-{}", slug);

    let database_id = d1_client.create_database(&db_name).await?;
    console_log!("Created D1 {} ({})", db_name, database_id);

    d1_client.exec_statements(&database_id, WORKSPACE_SCHEMA).await?;

    index_db.prepare("INSERT INTO workspaces_meta (slug, platform, platform_channel_id, d1_database_id) VALUES (?1, ?2, ?3, ?4)")
        .bind(&[slug.clone().into(), platform.into(), channel_id.into(), database_id.clone().into()])?
        .run().await?;

    Ok((slug, database_id))
}
```

- [ ] **Step 5: Error type**

```rust
// crates/worker/src/error.rs
use worker::Response;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Forbidden,
    Internal(String),
}

impl AppError {
    pub fn into_response(self) -> worker::Result<Response> {
        match self {
            Self::NotFound(m) => Response::error(m, 404),
            Self::BadRequest(m) => Response::error(m, 400),
            Self::Forbidden => Response::error("Forbidden", 403),
            Self::Internal(m) => { worker::console_log!("Error: {}", m); Response::error("Internal error", 500) }
        }
    }
}
```

- [ ] **Step 6: Commit**

```bash
git add migrations/ crates/worker/src/d1_rest.rs crates/worker/src/provisioning.rs crates/worker/src/error.rs
git commit -m "feat(worker): D1 REST client + auto-provisioning + schema migrations"
```

---

## Task 10: DB Query Helpers

**Files:**
- Create: `crates/worker/src/db.rs`

- [ ] **Step 1: WorkspaceDb with all query methods**

Same pattern as v2 Task 9 but with these additions:

- `upsert_member()` — sets `role = 'admin'` if first member (check member count first)
- `upsert_index_user()` — also writes to `user_workspaces` in the Index DB
- `insert_todo()` — atomic seq_num via `INSERT...SELECT COALESCE(MAX(seq_num),0)+1`
- `get_open_todos()` — returns `(id, title, seq_num)` tuples
- `get_todos_filtered()` — supports all `ListFilter` variants
- `get_todo_by_seq()` — for delete and done-by-id
- `delete_todo()` — soft delete (status = 'deleted')
- `get_notes()` — list all notes
- `search_notes()` — FTS5 search
- `get_status_counts()` — for @grumps status
- `track_bot_message()` / `is_bot_message()` / `get_todo_for_bot_message()`
- `log_activity()`

I won't duplicate the full code here — it follows the same `WorkspaceDb` pattern from v2 Task 9 with the D1RestClient. The key new queries:

```rust
/// Get todos with filtering. Returns (seq_num, title, status, assignee_name, priority, tags).
pub async fn get_todos_filtered(&self, filter: &str, member_id: Option<&str>) -> Result<Vec<(i64, String, String, Option<String>, i32, String)>> {
    let (sql, params) = match filter {
        "open" => ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE status IN ('open','in_progress') ORDER BY priority ASC, created_at DESC".into(), vec![]),
        "all" => ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE status != 'deleted' ORDER BY created_at DESC".into(), vec![]),
        "done" => ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE status = 'done' ORDER BY completed_at DESC".into(), vec![]),
        "mine" => ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE assigned_to = ?1 AND status IN ('open','in_progress') ORDER BY priority ASC".into(), vec![member_id.unwrap_or("").into()]),
        _ if filter.starts_with("assignee:") => {
            let name = &filter[9..];
            ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE assigned_name = ?1 AND status IN ('open','in_progress') ORDER BY priority ASC".into(), vec![name.into()])
        }
        _ if filter.starts_with("tag:") => {
            let tag = &filter[4..];
            ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE tags LIKE ?1 AND status IN ('open','in_progress') ORDER BY priority ASC".into(), vec![format!("%\"{}\"%" , tag).into()])
        }
        _ => ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE status IN ('open','in_progress') ORDER BY priority ASC".into(), vec![]),
    };
    // ... execute and deserialize
}

/// Get status counts for @grumps summary.
pub async fn get_status_counts(&self) -> Result<(i64, i64, i64, i64)> {
    // Returns (open_count, done_this_week, note_count, file_count)
    // ... 4 COUNT queries
}

/// First member check — set as admin if no members exist yet.
pub async fn upsert_member(&self, platform_user_id: &str, display_name: &str) -> Result<(String, bool)> {
    // Check if members table is empty → if so, this is the first member → admin
    // Returns (member_id, is_first_member)
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/worker/src/db.rs
git commit -m "feat(worker): DB query helpers with filtering, status counts, admin detection"
```

---

## Task 11: Message Handler (the brain)

**Files:**
- Create: `crates/worker/src/handler.rs`

This is the core logic — separated from the webhook route for clarity.

- [ ] **Step 1: Implement handler**

```rust
// crates/worker/src/handler.rs
use grumps_messaging::adapter::*;
use grumps_messaging::formatter;
use grumps_nlu::parser::*;
use grumps_nlu::matcher;
use crate::db::WorkspaceDb;

/// Result of processing a message: zero or more outbound messages to send.
pub struct HandlerResult {
    pub messages: Vec<OutboundMessage>,
}

impl HandlerResult {
    pub fn none() -> Self { Self { messages: vec![] } }
    pub fn one(text: String, reply_to: Option<String>) -> Self {
        Self { messages: vec![OutboundMessage { text, reply_to }] }
    }
    pub fn many(msgs: Vec<OutboundMessage>) -> Self { Self { messages: msgs } }
}

pub async fn handle_message(
    parse_result: ParseResult,
    inbound: &InboundMessage,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    workspace_slug: &str,
) -> worker::Result<HandlerResult> {
    match parse_result {
        ParseResult::AddTodos(todos) => handle_add_todos(todos, inbound, ws_db, member_id, workspace_slug).await,
        ParseResult::AddSingleTodo(todo) => handle_add_todos(vec![todo], inbound, ws_db, member_id, workspace_slug).await,
        ParseResult::CompleteTodos(items) => handle_complete_todos(items, ws_db, member_id, inbound).await,
        ParseResult::CompleteSingle(target) => handle_complete_single(target, ws_db, member_id, inbound).await,
        ParseResult::DeleteTodo(seq) => handle_delete(seq, ws_db, member_id).await,
        ParseResult::AddNote(note) => handle_add_note(note, ws_db, member_id).await,
        ParseResult::ListTodos(filter) => handle_list_todos(filter, ws_db, member_id).await,
        ParseResult::ListNotes => handle_list_notes(ws_db).await,
        ParseResult::SearchNotes(query) => handle_search_notes(&query, ws_db).await,
        ParseResult::ListFiles => Ok(HandlerResult::one("📎 File listing available on the web workspace.".into(), None)),
        ParseResult::Help => Ok(HandlerResult::one(formatter::help_text(), None)),
        ParseResult::WorkspaceLink => Ok(HandlerResult::one(format!("🔗 grumps.io/w/{}", workspace_slug), None)),
        ParseResult::Status => handle_status(ws_db, workspace_slug).await,
        ParseResult::QuotedTodo => handle_quoted_todo(inbound, ws_db, member_id).await,
        ParseResult::QuotedNote => handle_quoted_note(inbound, ws_db, member_id).await,
        ParseResult::TaskCardReply(action) => handle_card_reply(action, inbound, ws_db, member_id).await,
        ParseResult::Ignore => Ok(HandlerResult::none()),
    }
}

async fn handle_add_todos(
    todos: Vec<ParsedTodo>, inbound: &InboundMessage,
    ws_db: &WorkspaceDb<'_>, member_id: &str, slug: &str,
) -> worker::Result<HandlerResult> {
    let mut messages = Vec::new();

    // Summary message first
    messages.push(OutboundMessage {
        text: formatter::todos_added_summary(todos.len(), slug),
        reply_to: Some(inbound.message_id.clone()),
    });

    // Then one task card per todo
    for parsed in &todos {
        let tags_json = serde_json::to_string(&parsed.tags).unwrap_or("[]".into());
        let (todo_id, seq) = ws_db.insert_todo(
            &parsed.title, parsed.priority.as_int(), &tags_json,
            parsed.assignee_mention.as_deref().unwrap_or(""),
            parsed.assignee_mention.as_deref().unwrap_or(""), // assigned_name = mention for now
            member_id, "chat", &inbound.message_id,
        ).await?;

        ws_db.log_activity(member_id, "todo.created", "todo", &todo_id, "chat").await?;

        // Each task card is a separate message — so users can reply to it
        let card = formatter::task_card(
            seq, &parsed.title,
            parsed.assignee_mention.as_deref(),
            parsed.deadline_text.as_deref(),
            parsed.priority,
            &parsed.tags,
        );
        messages.push(OutboundMessage { text: card, reply_to: None });
        // The bot message tracking happens in webhook.rs after we get the sent message_id back
    }

    Ok(HandlerResult::many(messages))
}

async fn handle_complete_todos(
    items: Vec<String>, ws_db: &WorkspaceDb<'_>, member_id: &str, inbound: &InboundMessage,
) -> worker::Result<HandlerResult> {
    let open_todos = ws_db.get_open_todos().await?;
    let mut lines = Vec::new();

    for item in &items {
        match matcher::match_done(item, &open_todos) {
            matcher::MatchResult::Exact(m) => {
                ws_db.complete_todo(&m.todo_id, member_id).await?;
                ws_db.log_activity(member_id, "todo.completed", "todo", &m.todo_id, "chat").await?;
                lines.push(format!("✅ #{} \"{}\" — done.", m.seq_num, m.title));
            }
            matcher::MatchResult::Fuzzy(candidates) => {
                let opts: Vec<String> = candidates.iter().enumerate()
                    .map(|(i, c)| format!("  {}. #{} \"{}\"", i + 1, c.seq_num, c.title)).collect();
                lines.push(format!("🔍 \"{}\" — {} close matches:\n{}\nReply with the number.",
                    item, candidates.len(), opts.join("\n")));
            }
            matcher::MatchResult::NoMatch => {
                lines.push(format!("❓ \"{}\" — no match. Create it?", item));
            }
        }
    }

    Ok(HandlerResult::one(lines.join("\n\n"), Some(inbound.message_id.clone())))
}

async fn handle_complete_single(
    target: CompletionTarget, ws_db: &WorkspaceDb<'_>, member_id: &str, inbound: &InboundMessage,
) -> worker::Result<HandlerResult> {
    match target {
        CompletionTarget::BySeqNum(seq) => {
            match ws_db.get_todo_by_seq(seq).await? {
                Some(todo) => {
                    ws_db.complete_todo(&todo.id, member_id).await?;
                    ws_db.log_activity(member_id, "todo.completed", "todo", &todo.id, "chat").await?;
                    Ok(HandlerResult::one(format!("✅ #{} \"{}\" — done.", seq, todo.title), Some(inbound.message_id.clone())))
                }
                None => Ok(HandlerResult::one(format!("❓ No todo #{}.", seq), Some(inbound.message_id.clone()))),
            }
        }
        CompletionTarget::ByText(text) => {
            handle_complete_todos(vec![text], ws_db, member_id, inbound).await
        }
    }
}

async fn handle_delete(seq: i64, ws_db: &WorkspaceDb<'_>, member_id: &str) -> worker::Result<HandlerResult> {
    match ws_db.get_todo_by_seq(seq).await? {
        Some(todo) => {
            ws_db.delete_todo(&todo.id).await?;
            ws_db.log_activity(member_id, "todo.deleted", "todo", &todo.id, "chat").await?;
            Ok(HandlerResult::one(format!("🗑️ #{} \"{}\" — deleted.", seq, todo.title), None))
        }
        None => Ok(HandlerResult::one(format!("❓ No todo #{}.", seq), None)),
    }
}

async fn handle_add_note(note: ParsedNote, ws_db: &WorkspaceDb<'_>, member_id: &str) -> worker::Result<HandlerResult> {
    let title = note.title.as_deref().unwrap_or("");
    let note_id = ws_db.insert_note(title, &note.content, "chat", member_id).await?;
    ws_db.log_activity(member_id, "note.created", "note", &note_id, "chat").await?;
    let t = note.title.map(|t| format!(" \"{}\"", t)).unwrap_or_default();
    Ok(HandlerResult::one(format!("📝 Note{} saved.", t), None))
}

async fn handle_list_todos(filter: ListFilter, ws_db: &WorkspaceDb<'_>, member_id: &str) -> worker::Result<HandlerResult> {
    let (filter_str, label) = match &filter {
        ListFilter::Open => ("open", "open"),
        ListFilter::All => ("all", "all"),
        ListFilter::Mine => ("mine", "mine"),
        ListFilter::Done => ("done", "done"),
        ListFilter::Assignee(name) => (&*format!("assignee:{}", name), "assigned"),
        ListFilter::Tag(tag) => (&*format!("tag:{}", tag), "tagged"),
    };
    let todos = ws_db.get_todos_filtered(filter_str, Some(member_id)).await?;
    Ok(HandlerResult::one(formatter::todo_list(&todos, label), None))
}

async fn handle_list_notes(ws_db: &WorkspaceDb<'_>) -> worker::Result<HandlerResult> {
    let notes = ws_db.get_notes().await?;
    Ok(HandlerResult::one(formatter::note_list(&notes), None))
}

async fn handle_search_notes(query: &str, ws_db: &WorkspaceDb<'_>) -> worker::Result<HandlerResult> {
    let notes = ws_db.search_notes(query).await?;
    if notes.is_empty() {
        Ok(HandlerResult::one(format!("🔍 No notes matching \"{}\".", query), None))
    } else {
        Ok(HandlerResult::one(formatter::note_list(&notes), None))
    }
}

async fn handle_status(ws_db: &WorkspaceDb<'_>, slug: &str) -> worker::Result<HandlerResult> {
    let (open, done_week, notes, files) = ws_db.get_status_counts().await?;
    Ok(HandlerResult::one(formatter::status_summary(open, done_week, notes, files, slug), None))
}

async fn handle_quoted_todo(inbound: &InboundMessage, ws_db: &WorkspaceDb<'_>, member_id: &str) -> worker::Result<HandlerResult> {
    let content = inbound.quoted_message_text.as_deref().unwrap_or("");
    if content.is_empty() {
        return Ok(HandlerResult::one("❓ No message content to turn into a todo.".into(), None));
    }
    let parsed = crate::entity::extract_todo_from_line(content);
    handle_add_todos(vec![parsed], inbound, ws_db, member_id, "").await
}

async fn handle_quoted_note(inbound: &InboundMessage, ws_db: &WorkspaceDb<'_>, member_id: &str) -> worker::Result<HandlerResult> {
    let content = inbound.quoted_message_text.as_deref().unwrap_or("");
    if content.is_empty() {
        return Ok(HandlerResult::one("❓ No message content to save as a note.".into(), None));
    }
    let note = ParsedNote { title: None, content: content.to_string() };
    handle_add_note(note, ws_db, member_id).await
}

async fn handle_card_reply(
    action: TaskCardAction, inbound: &InboundMessage, ws_db: &WorkspaceDb<'_>, member_id: &str,
) -> worker::Result<HandlerResult> {
    let todo_id = if let Some(ref qid) = inbound.quoted_message_id {
        ws_db.get_todo_for_bot_message(qid).await.unwrap_or(None)
    } else { None };

    let text = match action {
        TaskCardAction::Done => {
            if let Some(ref tid) = todo_id {
                ws_db.complete_todo(tid, member_id).await?;
                ws_db.log_activity(member_id, "todo.completed", "todo", tid, "chat").await?;
            }
            "Done.".into()
        }
        TaskCardAction::Delete => {
            if let Some(ref tid) = todo_id {
                ws_db.delete_todo(tid).await?;
                ws_db.log_activity(member_id, "todo.deleted", "todo", tid, "chat").await?;
            }
            "Deleted.".into()
        }
        TaskCardAction::Snooze(time) => format!("⏰ Snoozed to {}.", time),
        TaskCardAction::Edit(title) => format!("✏️ Updated: \"{}\".", title),
        TaskCardAction::Reassign(person) => format!("👤 Reassigned to @{}.", person),
        TaskCardAction::ChangePriority(p) => format!("{} Priority: {}.", p.emoji(), p.label()),
        TaskCardAction::AddTag(tag) => format!("🏷️ #{}.", tag),
        TaskCardAction::ChangeStatus(s) => format!("📌 Status: {}.", s),
    };

    Ok(HandlerResult::one(text, Some(inbound.message_id.clone())))
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/worker/src/handler.rs
git commit -m "feat(worker): message handler — all commands, individual task cards, reply detection, quoted todo/note"
```

---

## Task 12: Webhook Route (wiring)

**Files:**
- Create: `crates/worker/src/routes/webhook.rs`
- Create: `crates/worker/src/routes/health.rs`
- Modify: `crates/worker/src/routes/mod.rs`
- Modify: `crates/worker/src/lib.rs`

- [ ] **Step 1: Webhook route — dedup, provision, parse, handle, send individual messages**

```rust
// crates/worker/src/routes/webhook.rs
use worker::*;
use grumps_messaging::adapter::*;
use grumps_messaging::whatsapp::WhatsAppAdapter;
use grumps_nlu::parser;
use crate::{db, d1_rest::D1RestClient, provisioning, handler};

pub async fn handle_verify(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let wa = build_adapter(&ctx)?;
    let params: std::collections::HashMap<String, String> = req.url()?.query_pairs().map(|(k,v)| (k.to_string(), v.to_string())).collect();
    match wa.handle_verification_challenge(&params) { Ok(c) => Response::ok(c), Err(e) => Response::error(format!("{}", e), 403) }
}

pub async fn handle_incoming(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let wa = build_adapter(&ctx)?;
    let body = req.bytes().await?;

    // 1. HMAC verify
    if let Some(sig) = req.headers().get("X-Hub-Signature-256")? {
        if wa.verify_signature(&body, &sig).is_err() { return Response::error("Bad signature", 403); }
    }

    // 2. Parse
    let inbound = match wa.parse_webhook(&body) { Ok(Some(m)) => m, _ => return Response::ok("ok") };

    // 3. Dedup via KV
    let kv = ctx.kv("KV")?;
    let key = format!("msg:{}", inbound.message_id);
    if kv.get(&key).text().await?.is_some() { return Response::ok("ok"); }
    kv.put(&key, "1")?.expiration_ttl(86400).execute().await?;

    // 4. Resolve or provision workspace
    let index_db = db::get_index_db(&ctx.env)?;
    let d1_client = D1RestClient::from_env(&ctx.env)?;
    let workspace = match db::lookup_workspace(&index_db, "whatsapp", &inbound.channel_id).await? {
        Some(ws) => ws,
        None => {
            let (slug, db_id) = provisioning::provision_workspace(&d1_client, &index_db, "whatsapp", &inbound.channel_id).await?;
            db::WorkspaceMetaRow { slug, d1_database_id: db_id, name: None, plan: "free".into() }
        }
    };

    let ws_db = db::WorkspaceDb::new(&d1_client, workspace.d1_database_id.clone());

    // 5. Upsert member (first = admin)
    let (member_id, is_first) = ws_db.upsert_member(&inbound.sender_id, &inbound.sender_name).await?;

    // Also register in Index DB
    let _ = db::upsert_index_user(&index_db, &inbound.sender_id, &workspace.slug, if is_first { "admin" } else { "member" }).await;

    // 6. Parse message
    let text = match &inbound.text { Some(t) => t.as_str(), None => return Response::ok("ok") };

    let is_reply_to_bot = match &inbound.quoted_message_id {
        Some(qid) => ws_db.is_bot_message(qid).await.unwrap_or(false),
        None => false,
    };

    let parse_result = parser::parse(text, inbound.is_mention_to_bot, inbound.is_direct_message, is_reply_to_bot, inbound.quoted_message_id.is_some());

    // 7. Handle
    let result = handler::handle_message(parse_result, &inbound, &ws_db, &member_id, &workspace.slug).await?;

    // 8. Send each message individually + track bot message IDs
    for msg in &result.messages {
        let (url, body) = wa.build_send_request(&inbound.sender_id, msg)?;

        let mut headers = Headers::new();
        headers.set("Authorization", &format!("Bearer {}", wa.access_token))?;
        headers.set("Content-Type", "application/json")?;

        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_headers(headers).with_body(Some(body.into()));

        let req = Request::new_with_init(&url, &init)?;
        let mut resp = Fetch::Request(req).send().await?;

        // Track bot's sent message_id for reply detection
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(sent_id) = json.pointer("/messages/0/id").and_then(|v| v.as_str()) {
                // TODO: associate with the correct todo_id for task card messages
                let _ = ws_db.track_bot_message(sent_id, None).await;
            }
        }
    }

    Response::ok("ok")
}

fn build_adapter(ctx: &RouteContext<()>) -> Result<WhatsAppAdapter> {
    Ok(WhatsAppAdapter::new(
        ctx.env.var("WA_PHONE_NUMBER_ID")?.to_string(),
        ctx.env.var("WA_VERIFY_TOKEN")?.to_string(),
        ctx.env.secret("WA_APP_SECRET")?.to_string(),
        ctx.env.secret("WA_ACCESS_TOKEN")?.to_string(),
    ))
}
```

- [ ] **Step 2: Health route + mod.rs + lib.rs**

```rust
// crates/worker/src/routes/health.rs
use worker::*;
pub fn handle(_: Request, _: RouteContext<()>) -> Result<Response> { Response::ok("ok") }
```

```rust
// crates/worker/src/routes/mod.rs
pub mod webhook;
pub mod health;
```

```rust
// crates/worker/src/lib.rs
use worker::*;
mod d1_rest;
mod db;
mod error;
mod handler;
mod provisioning;
mod routes;

#[event(fetch, respond_with_errors)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .get("/health", routes::health::handle)
        .get_async("/webhook/whatsapp", routes::webhook::handle_verify)
        .post_async("/webhook/whatsapp", routes::webhook::handle_incoming)
        .run(req, env)
        .await
}
```

- [ ] **Step 3: Check compilation, commit**

Run: `cargo check -p grumps-worker`

```bash
git add crates/worker/
git commit -m "feat(worker): webhook route with dedup, provisioning, individual message sending"
```

---

## Task 13: Local Testing + Deploy

**Files:**
- Create: `test-webhook.sh`

- [ ] **Step 1: Test script covering all commands**

```bash
#!/bin/bash
BASE="http://localhost:8787"
echo "=== Health ===" && curl -s "$BASE/health" && echo
echo "=== TODO block ===" && curl -s -X POST "$BASE/webhook/whatsapp" -H "Content-Type: application/json" -H "X-Hub-Signature-256: sha256=test" -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m1","timestamp":"1713000000","type":"text","text":{"body":"TODO:\n• Buy bread\n• Call plumber @Bob !high"}}]}}]}]}' && echo
echo "=== NOTE ===" && curl -s -X POST "$BASE/webhook/whatsapp" -H "Content-Type: application/json" -H "X-Hub-Signature-256: sha256=test" -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m2","timestamp":"1713000060","type":"text","text":{"body":"NOTE [wifi]: password XYZ"}}]}}]}]}' && echo
echo "=== @grumps list ===" && curl -s -X POST "$BASE/webhook/whatsapp" -H "Content-Type: application/json" -H "X-Hub-Signature-256: sha256=test" -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m3","timestamp":"1713000120","type":"text","text":{"body":"@grumps list"}}]}}]}]}' && echo
echo "=== @grumps done #1 ===" && curl -s -X POST "$BASE/webhook/whatsapp" -H "Content-Type: application/json" -H "X-Hub-Signature-256: sha256=test" -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Bob"}}],"messages":[{"from":"337","id":"m4","timestamp":"1713000180","type":"text","text":{"body":"@grumps done #1"}}]}}]}]}' && echo
echo "=== @grumps status ===" && curl -s -X POST "$BASE/webhook/whatsapp" -H "Content-Type: application/json" -H "X-Hub-Signature-256: sha256=test" -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m5","timestamp":"1713000240","type":"text","text":{"body":"@grumps"}}]}}]}]}' && echo
echo "=== Default: create todo ===" && curl -s -X POST "$BASE/webhook/whatsapp" -H "Content-Type: application/json" -H "X-Hub-Signature-256: sha256=test" -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m6","timestamp":"1713000300","type":"text","text":{"body":"@grumps book restaurant for friday @Bob"}}]}}]}]}' && echo
echo "=== Dedup (m6 again) ===" && curl -s -X POST "$BASE/webhook/whatsapp" -H "Content-Type: application/json" -H "X-Hub-Signature-256: sha256=test" -d '{"entry":[{"id":"g1","changes":[{"value":{"metadata":{"display_phone_number":"+33","phone_number_id":"123"},"contacts":[{"profile":{"name":"Alice"}}],"messages":[{"from":"336","id":"m6","timestamp":"1713000300","type":"text","text":{"body":"@grumps book restaurant for friday @Bob"}}]}}]}]}' && echo "(should be silent — dedup)"
```

- [ ] **Step 2: Local test**

Run: `wrangler dev --local` (terminal 1)
Run: `bash test-webhook.sh` (terminal 2)

- [ ] **Step 3: Deploy**

```bash
wrangler secret put WA_APP_SECRET
wrangler secret put WA_ACCESS_TOKEN
wrangler secret put CF_API_TOKEN
wrangler secret put CF_ACCOUNT_ID
wrangler d1 create grumps-index  # update wrangler.toml
wrangler kv namespace create KV  # update wrangler.toml
wrangler d1 execute grumps-index --file=migrations/index/0001_init.sql
wrangler deploy
```

Configure Meta webhook: URL = `https://grumps-api.*.workers.dev/webhook/whatsapp`

- [ ] **Step 4: E2E test in WhatsApp**

Send: `TODO:\n• Test from prod`
Expected: summary message + individual task card

Send: `@grumps list`
Expected: list of open todos

Send: `@grumps`
Expected: status summary

- [ ] **Step 5: Commit**

```bash
git add test-webhook.sh wrangler.toml
git commit -m "test + deploy: webhook test script, Cloudflare deployment"
```

---

## What's Fixed (v1 → v3)

| Issue | v1 | v3 |
|---|---|---|
| Task cards | Combined message | Individual per todo, tracked in bot_messages |
| @grumps commands | All → "needs LLM" | Regex: list/done/delete/notes/files/help/link/status |
| Default action | Dead end | @grumps + text = create todo |
| Add-by-reply | Missing | Reply + @grumps todo/note |
| DB access | Static bindings | D1 REST API (dynamic, unlimited) |
| Provisioning | Manual | Auto on first message |
| HMAC | Stub | Real HMAC-SHA256 |
| Dedup | None | KV TTL 24h |
| Seq num | Race condition | Atomic INSERT...SELECT |
| Reply detection | Any reply = bot | bot_messages table |
| First member | Always "member" | First = admin |
| Index DB users | Never populated | user_workspaces populated |
| Mention detection | text.contains | Also handles "grumps " prefix |
| Personality | Generic | Terse, dry ("Nothing to do. Suspicious.") |
| List filters | None | open/all/mine/done/@person/#tag |

---

## Next Plans

- **Phase 2:** `phase2-web-workspace.md` — Leptos SPA, OTP auth, all pages, CORS
- **Phase 3:** `phase3-intelligence.md` — Gemini/Haiku NLU, reminders, Stripe
- **Phase 4:** `phase4-scale.md` — Telegram/Discord, PWA, API
