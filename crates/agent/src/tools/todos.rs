//! Mutating todo tools: complete_todo / reopen_todo.
//!
//! Completion reuses the fuzzy matcher (`grumps_nlu::matcher`) so the model can
//! say "mark the trash one done" and we resolve it against the open list.
//! Results are returned as JSON for the model to phrase; ambiguous matches come
//! back as candidates so it can ask the user which one.

use super::{args, parse_args, ToolContext};
use grumps_nlu::matcher::{self, MatchResult};
use serde_json::{json, Value};

/// Resolve a `(seq_num | query)` argument pair against a `(id, title, seq)`
/// list, returning the matched todo id+seq+title, or an outcome JSON otherwise.
enum Pick {
    Hit {
        id: String,
        seq_num: i64,
        title: String,
    },
    Outcome(Value),
}

fn pick(
    seq_num: Option<i64>,
    query: Option<&str>,
    todos: &[(String, String, i64)],
    empty_reason: &str,
) -> Pick {
    if todos.is_empty() {
        return Pick::Outcome(json!({ "ok": false, "reason": empty_reason }));
    }
    if let Some(seq) = seq_num {
        return match todos.iter().find(|(_, _, n)| *n == seq) {
            Some((id, title, n)) => Pick::Hit {
                id: id.clone(),
                seq_num: *n,
                title: title.clone(),
            },
            None => {
                Pick::Outcome(json!({ "ok": false, "reason": "seq_not_found", "seq_num": seq }))
            }
        };
    }
    let query = query.unwrap_or("").trim();
    if query.is_empty() {
        return Pick::Outcome(json!({ "ok": false, "reason": "missing_query" }));
    }
    match matcher::match_done(query, todos) {
        MatchResult::Exact(m) => Pick::Hit {
            id: m.todo_id,
            seq_num: m.seq_num,
            title: m.title,
        },
        MatchResult::Fuzzy(cands) => Pick::Outcome(json!({
            "ok": false,
            "reason": "ambiguous",
            "candidates": cands.iter()
                .map(|c| json!({ "seq_num": c.seq_num, "title": c.title }))
                .collect::<Vec<_>>(),
        })),
        MatchResult::NoMatch => {
            Pick::Outcome(json!({ "ok": false, "reason": "no_match", "query": query }))
        }
    }
}

pub async fn complete_todo(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::CompleteTodoArgs = parse_args(raw, "complete_todo")?;
    let todos = ctx.db.list_open_todos().await?;
    match pick(a.seq_num, a.query.as_deref(), &todos, "no_open_todos") {
        Pick::Hit { id, seq_num, title } => {
            ctx.db.complete_todo(&id, ctx.member_id).await?;
            Ok(json!({ "ok": true, "completed": true, "seq_num": seq_num, "title": title }))
        }
        Pick::Outcome(v) => Ok(v),
    }
}

pub async fn reopen_todo(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::ReopenTodoArgs = parse_args(raw, "reopen_todo")?;
    let todos = ctx.db.list_done_todos().await?;
    match pick(a.seq_num, a.query.as_deref(), &todos, "no_done_todos") {
        Pick::Hit { id, seq_num, title } => {
            ctx.db.reopen_todo(&id).await?;
            Ok(json!({ "ok": true, "reopened": true, "seq_num": seq_num, "title": title }))
        }
        Pick::Outcome(v) => Ok(v),
    }
}

/// Read-only status lookup, so the agent can judge "only if not done"-style
/// instructions at fire time without mutating anything.
pub async fn get_todo_status(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::GetTodoStatusArgs = parse_args(raw, "get_todo_status")?;
    let open = ctx.db.list_open_todos().await?;
    let done = ctx.db.list_done_todos().await?;
    // Match across both lists; status is decided by which list the hit came from.
    let mut all = open.clone();
    all.extend(done.iter().cloned());
    match pick(a.seq_num, a.query.as_deref(), &all, "no_todos") {
        Pick::Hit { id, seq_num, title } => {
            let status = if open.iter().any(|(i, _, _)| i == &id) {
                "open"
            } else {
                "done"
            };
            Ok(
                json!({ "ok": true, "found": true, "status": status, "seq_num": seq_num, "title": title }),
            )
        }
        // Not found or ambiguous — surface the outcome so the model can refine.
        Pick::Outcome(v) => Ok(v),
    }
}
