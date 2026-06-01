# Debugging Grumps

How to trace an inbound message (Telegram / WhatsApp / Discord) through the worker, confirm which code branches it takes, and step-debug the logic.

There is **no classic step-debugger** for Rust+WASM on Cloudflare Workers — you cannot set a breakpoint that pauses on an incoming request inside the running worker. Instead, debugging works at three levels: replay + live logs to observe the real path, structured log probes at decision points, and native `cargo test` for true breakpoint step-debugging of the pure logic.

## The path of an inbound message

A Telegram message flows like this:

```
POST /webhook/telegram
  └─ webhook_telegram.rs : handle_incoming()
       ├─ verify X-Telegram-Bot-Api-Secret-Token header   (403 if missing/wrong)
       ├─ my_chat_member ?  → route_chat_member() → FirstAdd / Promotion / archive
       ├─ private chat, unknown ? → provision_workspace_dm()   (DM provisioning)
       └─ normal text message:
            ├─ parser::parse(clean_text, is_mention, is_dm, …)   ← structured classification
            └─ handler::handle_message(…)                         ← the big match
                 ├─ try_route_via_agent()   ← @grumps fast-path (LLM agent)
                 └─ match parse_result {
                       AddTodos / AddSingleTodo(→ LLM classify) / Note / Done / List / Help …
                    }
            └─ for each reply → tg.build_send_request() → Telegram API
```

The two branch points that matter most: `parser::parse` (`crates/nlu`) decides *what kind* of message it is, and `handler::handle_message` (`crates/worker/src/handler.rs`) runs the matching branch. WhatsApp and Discord follow the same shape via their own webhook handlers (`webhook.rs`, `webhook_discord.rs`), converging on the same `handler::handle_message`.

## Level 1 — Replay a message and watch it live

The repo ships `replay-webhook.sh`, which forges a correctly-signed platform webhook (it reads the secret header from `.dev.vars`, the same file `wrangler dev` uses, so they always match) and POSTs it to your local worker. No real Telegram/WhatsApp account needed.

Use two terminals:

```bash
# Terminal A — the local worker, with logs streaming
npx wrangler dev          # → http://localhost:8787

# Terminal B — inject messages
./replay-webhook.sh tg "@grumps list"
./replay-webhook.sh tg "buy bread tomorrow 9am"     # natural language → LLM branch
./replay-webhook.sh tg "list" --dm                  # private chat (DM provisioning)
./replay-webhook.sh wa "TODO: buy bread"            # WhatsApp (needs WA_APP_SECRET in .dev.vars)
./replay-webhook.sh health                          # just hit /health
```

Env overrides for the Telegram replay: `BASE` (target URL), `TG_CHAT_ID`, `TG_USER_ID`, `LANG_CODE`. Each call uses a fresh epoch-based `message_id` so the KV dedup never silently swallows your replay.

Everything that goes through `console_log!` / `log_event()` prints directly in the `wrangler dev` terminal. Structured events appear as `[event] <name> {json}` — easy to grep. Against production, `npx wrangler tail` gives you the same live stream.

`test-webhook.sh` is a related smoke-test that fires a fixed sequence of WhatsApp payloads (TODO block, note, list, dedup check, help, …) — handy for a quick end-to-end sanity pass.

## Level 2 — Drop probes at the branch points

Since there are no breakpoints in the worker, the platform-native technique is **structured logging at decision points**, using the `observability::log_event()` helper. To confirm "the message takes branch X like I think", add a temporary probe:

```rust
// in handle_message, right after parser::parse
crate::observability::log_event("debug.parse", &serde_json::json!({
    "raw": clean_text,
    "variant": format!("{:?}", parse_result),
    "is_mention": inbound.is_mention_to_bot,
}));
```

Replay the message, read the `[event] debug.parse {...}` line in `wrangler dev`, and you know exactly which variant was chosen and which `match` arm will run. This is the practical equivalent of single-stepping on this platform. Remove the probe (or keep it behind a stable event name) once you're done.

## Level 3 — Real step-debugging on the pure logic

This is where an actual breakpoint debugger works. `parser::parse`, `route_chat_member`, and most of the `handler` decision logic are intentionally **pure and I/O-free**, so you can run them natively via `cargo test` on the Windows target — and there a real debugger (LLDB / CodeLLDB in VS Code, breakpoints, variable inspection) works normally:

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test \
  --target x86_64-pc-windows-msvc -p grumps-nlu parse
```

`webhook_telegram.rs` already carries a battery of `route_chat_member` tests as a model. The recommended workflow for chasing a specific branch: write a native test that reproduces your exact input, then step-debug inside it. This isolates the logic from the Workers runtime (which itself is not step-debuggable).

## Quick reference

| Goal | Tool |
|---|---|
| Run the worker locally | `npx wrangler dev` |
| Inject a fake message | `./replay-webhook.sh tg "…"` (`--dm` for private chat) |
| Watch logs locally | the `wrangler dev` terminal |
| Watch logs in production | `npx wrangler tail` |
| Confirm a branch was taken | add a `log_event("debug.…", …)` probe |
| True breakpoint step-debug | native `cargo test` on the pure logic |
