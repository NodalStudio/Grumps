# Grumps Agent — Plan B : Agent loop + tool use

> **For agentic workers:** Use superpowers:subagent-driven-development to execute this plan.

**Goal:** Implement the agent's core "brain" : Sonnet client + cascade routing (Gemini classifier first) + tool dispatch with 11 tools + multi-turn sessions + wiring into the existing fast-path. Fills the `agent_task` and `follow_up` placeholders left by Plan A.

**Architecture:** New `grumps-agent` crate. Two LLM clients (Anthropic Sonnet 4.6 via Fetch, Gemini 2.5 Flash via Fetch). Tool registry + dispatch in agent crate. Cascade router : regex fast-path → Gemini classifier → CRUD direct OR Sonnet agent loop. Sessions persisted in `agent_sessions` table (already created by Plan A T13).

**Tech Stack:** Rust + workers-rs 0.8, `worker::Fetch`, JSON via serde_json, all LLM calls via HTTP. No new external deps.

**Spec sections:** 8 (agent loop & tool use), 11 (proactive — but only the reactive path here, proactive is Plan E), 13.1-13.2 (LLM strategy + cache).

---

## Toolchain reminder

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe ...
```

Working dir : `C:\Users\mayer\Documents\Grumps\.worktrees\agent-foundation` (continuing on `feat/agent-foundation`).

## File Structure

```
crates/agent/                       ★ NEW
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── llm/
    │   ├── mod.rs
    │   ├── anthropic.rs            # Sonnet client + tool use loop
    │   └── gemini.rs               # Gemini Flash classifier client
    ├── prompt.rs                    # system prompt builder (workspace-aware)
    ├── router.rs                    # cascade routing logic
    ├── session.rs                   # session helpers (wraps WorkspaceDb sessions)
    ├── loop.rs                      # the agent loop (call Sonnet, dispatch tools, repeat)
    └── tools/
        ├── mod.rs                   # registry + dispatch
        ├── schemas.rs               # static tool schemas (JSON for Anthropic API)
        ├── memory.rs                # query_memory, save_memory wrappers
        ├── rag.rs                   # query_chat_history wrapper
        ├── crud.rs                  # create_todo / create_note / create_event / create_reminder
        ├── scheduler.rs             # schedule_action wrapper
        ├── calendar.rs              # list_calendar wrapper
        ├── web.rs                   # web_search (stub here, real impl in Plan D)
        └── chat.rs                  # send_message
```

Worker (modifications) :
- `crates/worker/Cargo.toml` — add `grumps-agent` path dep
- `crates/worker/src/handler.rs` — add agent dispatch path after existing fast-path
- `crates/worker/src/scheduler_executor.rs` — fill in `agent_task` and `follow_up` to call agent loop

---

## Task B1 : Scaffold grumps-agent crate

Add `crates/agent` to workspace `Cargo.toml` members. Create the crate with deps :

```toml
[package]
name = "grumps-agent"
version = "0.1.0"
edition = "2021"

[dependencies]
grumps-core = { path = "../core" }
grumps-memory = { path = "../memory" }
grumps-scheduler = { path = "../scheduler" }
grumps-calendar = { path = "../calendar" }
worker = { version = "0.8", features = ["d1"] }
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
thiserror.workspace = true
```

Create `src/lib.rs` :
```rust
//! Grumps agent : cascade router + Sonnet tool use + multi-turn sessions.
pub mod llm;
pub mod prompt;
pub mod router;
pub mod session;
pub mod loop_;   // `loop` is reserved
pub mod tools;
```

Create empty stubs for each module file. Verify `cargo check`. Commit.

## Task B2 : Anthropic client (Sonnet wrapper)

`crates/agent/src/llm/anthropic.rs` — POST to `https://api.anthropic.com/v1/messages` via `worker::Fetch`. Reads `ANTHROPIC_API_KEY` secret. Function :

```rust
pub async fn call(env: &Env, req: AnthropicRequest) -> Result<AnthropicResponse>
```

Where `AnthropicRequest` = `{ model, max_tokens, system, messages, tools, tool_choice }` and `AnthropicResponse` = `{ id, content: Vec<ContentBlock>, stop_reason: String, usage }`. `ContentBlock` is enum `Text { text }` or `ToolUse { id, name, input }`.

Headers : `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`, `anthropic-beta: prompt-caching-2024-07-31`.

Implement minimal types, parse minimal response shape (only what we use : content blocks, stop_reason). Compile-check. Commit.

## Task B3 : Gemini client (Flash classifier)

`crates/agent/src/llm/gemini.rs` — POST to `https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={GEMINI_API_KEY}` via Fetch.

Function :
```rust
pub async fn classify_intent(env: &Env, message: &str, members: &[String]) -> Result<ClassifiedIntent>
```

Returns :
```rust
pub struct ClassifiedIntent {
    pub intent: String,         // "create_todo" | "create_note" | ... | "complex_agent_task"
    pub confidence: f32,
    pub args: serde_json::Value,
}
```

The Gemini prompt = system instruction asking for JSON output matching the spec § 8.2. Parse the response, extract the JSON from the text reply (Gemini doesn't natively support JSON mode in v1beta the same way Anthropic does — extract JSON via regex or by trimming text wrappers). Compile + commit.

## Task B4 : System prompt builder

`crates/agent/src/prompt.rs` — function `build_system_prompt(ctx: PromptContext) -> String` per spec § 8.5.

```rust
pub struct PromptContext {
    pub workspace_name: String,
    pub platform: String,
    pub member_count: usize,
    pub persona: String,           // "default" | "playful" | "formal"
    pub language: String,
    pub pinned_memories: Vec<MemoryEntry>,   // formatted as bullets
    pub members: Vec<MemberShort>,
    pub now_local: String,
    pub timezone: String,
    pub proactive_mode: bool,
    pub auto_memory: bool,
    pub agent_calls_remaining: u32,
    pub web_search_remaining: u32,
}
```

Returns a string template assembled per § 8.5. Add 1 test verifying the prompt contains key sections (PERSONA, MEMORY POLICY, RULES) and a sample memory entry.

Compile + test + commit.

## Task B5 : Tool schemas

`crates/agent/src/tools/schemas.rs` — static array of 11 tool definitions per spec § 8.4. Each tool = `{ name, description, input_schema: { type, properties, required } }`. Function `pub fn all_tools() -> serde_json::Value`. No tests needed — these are static JSON.

Compile + commit.

## Task B6 : Tool dispatch framework

`crates/agent/src/tools/mod.rs` — central dispatch :

```rust
pub async fn dispatch(env: &Env, ws_slug: &str, member_id: &str, tool_name: &str, args: serde_json::Value) -> Result<serde_json::Value>
```

Match on `tool_name`, delegate to module-specific function. Each tool function returns a `serde_json::Value` that becomes the `tool_result` content for the next Sonnet call.

Stub all 11 tool functions with `Err("not yet implemented")` — they're filled in B7-B14.

Compile + commit.

## Task B7 : Tool — query_memory + save_memory

`crates/agent/src/tools/memory.rs` — wraps WorkspaceDb. Args : `{ query: string, kind?: string, limit?: int }` for query, `{ key, value, kind, ... }` for save. Returns the entries / new id as JSON.

These are the simplest tools — just call existing WorkspaceDb methods, serialize results. Compile + commit.

## Task B8 : Tool — query_chat_history

`crates/agent/src/tools/rag.rs` — wraps `crate::rag::query_chat_history` (worker module from Plan A T21). Args : `{ query, from?, to?, limit? }`. Returns list of `QueryHit`s.

Note : this tool calls into the WORKER crate, but agent crate doesn't depend on worker. Need to refactor : either move `rag` into agent crate (cleaner) OR define the function as a public extern point. **Decision** : move `rag.rs` from worker to agent (`crates/agent/src/tools/rag_pipeline.rs`). The webhook handlers in worker will import it from agent.

Compile + commit (move + new tool wrapper).

## Task B9 : Tool — create_todo / create_note / create_event / create_reminder

`crates/agent/src/tools/crud.rs` — 4 functions wrapping WorkspaceDb's existing creates. For `create_reminder`, builds a `NewScheduledAction` of type `Reminder` and calls `db.create_scheduled_action`.

Args validation : reasonable defaults, member references checked against `members` table.

Compile + commit.

## Task B10 : Tool — schedule_action + list_calendar

`crates/agent/src/tools/scheduler.rs` and `calendar.rs` — wrappers. `schedule_action` takes the full payload schema and creates a `NewScheduledAction`. `list_calendar` takes `from`/`to`/`types?` and aggregates from todos + reminders + events + scheduled (this is the Calendar aggregation — full impl in Plan C, here a basic union).

Compile + commit.

## Task B11 : Tool — web_search (stub) + send_message

`crates/agent/src/tools/web.rs` — STUB returning `{ results: [], note: "web search arrives in Plan D" }`. Compile-only, ready for D.

`crates/agent/src/tools/chat.rs` — `send_message` calls `crate::messaging_dispatch::send_to_workspace` from worker. Same cross-crate issue as B8 — the agent needs to call into worker. **Decision** : factor out the send to a small `grumps-messaging` extension OR pass a callback into the agent. Simplest : pass `send_to_workspace` as a function pointer in a context struct. For now, create `MessagingSink` trait in agent and have worker provide an impl.

Compile + commit.

## Task B12 : Cascade router

`crates/agent/src/router.rs` — main routing function :

```rust
pub async fn route_message(
    env: &Env,
    ws_slug: &str,
    member_id: &str,
    text: &str,
    has_active_session: bool,
    sink: &impl MessagingSink,
) -> Result<RouteResult>
```

Logic per spec § 8.1 :
1. If `has_active_session` → straight to AGENT LOOP (B13)
2. Else call Gemini classifier
3. If `confidence > 0.85` and `intent != complex_agent_task` → CRUD direct via tool dispatch
4. Else → AGENT LOOP

Compile + commit.

## Task B13 : Agent loop

`crates/agent/src/loop_.rs` — the Sonnet tool-use loop :

```rust
pub async fn run_loop(env, ws_slug, member_id, user_message, sink) -> Result<LoopResult>
```

1. Load session if exists (`db.get_active_agent_session`)
2. Build system prompt (B4)
3. Append user message to session.messages
4. Call Sonnet (B2) with tools (B5)
5. If `stop_reason == "end_turn"` → done, send final text via sink, persist session
6. If `stop_reason == "tool_use"` → for each tool_use block, dispatch (B6), append result to messages, GOTO 4
7. Max 5 iterations, max 50K cumulative tokens, 30s timeout

Compile + commit.

## Task B14 : Wire into handler.rs

In `crates/worker/src/handler.rs`, after the existing fast-path checks for structured commands (TODO:/DONE:/NOTE:/REMIND:) and reply-to-task-card, ADD a new branch :

```rust
// If text starts with @grumps mention OR is in a DM context (not in scope here, future),
// route through the agent
if text_contains_mention(&text) {
    let sink = WorkerMessagingSink::new(env);
    let result = grumps_agent::router::route_message(env, slug, member_id, text, has_session, &sink).await?;
    return Ok(HandlerResult::from_agent(result));
}
```

Adapt `text_contains_mention`, `WorkerMessagingSink`, and the call to fit the actual handler.rs flow. The agent's response (from `sink.send`) replaces the existing return path for this branch.

Compile (both worker check and `worker-build --release` if available). Commit.

## Task B15 : Fill scheduler_executor for agent_task + follow_up

In `crates/worker/src/scheduler_executor.rs`, replace the current placeholder for `ActionType::FollowUp | ActionType::AgentTask` with calls to `grumps_agent::loop_::run_oneshot(...)` (a one-shot variant of the agent loop that doesn't persist session state, used for autonomous executions).

Add `run_oneshot` to `crates/agent/src/loop_.rs` if not already there. Compile + commit.

## Task B16 : Final verify

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --workspace
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check -p grumps-worker --target wasm32-unknown-unknown
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --workspace --target x86_64-pc-windows-msvc --exclude grumps-spa
```

Tag `plan-B-agent`. Commit any final cleanup.

---

*Plan B complete. ~16 tasks, focuses on agent core. Plans C-G follow.*
