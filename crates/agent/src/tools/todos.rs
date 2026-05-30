//! Mutating todo tools: complete_todo / reopen_todo.
//!
//! Completion reuses the fuzzy matcher (`grumps_nlu::matcher`) so the model can
//! say "mark the trash one done" and we resolve it against the open list.
//! Results are returned as JSON for the model to phrase; ambiguous matches come
//! back as candidates so it can ask the user which one.

use serde_json::{json, Value};
use grumps_nlu::matcher::{self, MatchResult};
use super::ToolContext;

/// Resolve a `(seq_num | query)` argument pair against a `(id, title, seq)`
/// list, returning the matched todo id+seq+title, or an outcome JSON otherwise.
enum Pick {
    Hit { id: String, seq_num: i64, title: String },
    Outcome(Value),
}

fn pick(args: &Value, todos: &[(String, String, i64)], empty_reason: &str) -> Pick {
    if todos.is_empty() {
        return Pick::Outcome(json!({ "ok": false, "reason": empty_reason }));
    }
    if let Some(seq) = args.get("seq_num").and_then(|v| v.as_i64()) {
        return match todos.iter().find(|(_, _, n)| *n == seq) {
            Some((id, title, n)) => Pick::Hit { id: id.clone(), seq_num: *n, title: title.clone() },
            None => Pick::Outcome(json!({ "ok": false, "reason": "seq_not_found", "seq_num": seq })),
        };
    }
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").trim();
    if query.is_empty() {
        return Pick::Outcome(json!({ "ok": false, "reason": "missing_query" }));
    }
    match matcher::match_done(query, todos) {
        MatchResult::Exact(m) => Pick::Hit { id: m.todo_id, seq_num: m.seq_num, title: m.title },
        MatchResult::Fuzzy(cands) => Pick::Outcome(json!({
            "ok": false,
            "reason": "ambiguous",
            "candidates": cands.iter()
                .map(|c| json!({ "seq_num": c.seq_num, "title": c.title }))
                .collect::<Vec<_>>(),
        })),
        MatchResult::NoMatch => Pick::Outcome(json!({ "ok": false, "reason": "no_match", "query": query })),
    }
}

pub async fn complete_todo(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let todos = ctx.db.list_open_todos().await?;
    match pick(&args, &todos, "no_open_todos") {
        Pick::Hit { id, seq_num, title } => {
            ctx.db.complete_todo(&id, ctx.member_id).await?;
            Ok(json!({ "ok": true, "completed": true, "seq_num": seq_num, "title": title }))
        }
        Pick::Outcome(v) => Ok(v),
    }
}

pub async fn reopen_todo(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let todos = ctx.db.list_done_todos().await?;
    match pick(&args, &todos, "no_done_todos") {
        Pick::Hit { id, seq_num, title } => {
            ctx.db.reopen_todo(&id).await?;
            Ok(json!({ "ok": true, "reopened": true, "seq_num": seq_num, "title": title }))
        }
        Pick::Outcome(v) => Ok(v),
    }
}
