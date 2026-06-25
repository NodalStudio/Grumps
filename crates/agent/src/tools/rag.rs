//! Tool implementation: query_chat_history + read_chat_around.

use super::ToolContext;
use crate::db::ChatMessage;
use serde_json::Value;

/// Generous per-side fetch from D1; the result is then trimmed by char budget.
const MAX_SIDE_MSGS: i64 = 15;
/// Approximate context budget per side of a match (in characters). Adapts the
/// window to message length: many short messages vs few long ones.
const SIDE_BUDGET_CHARS: usize = 1200;

/// Semantic search over chat history, each hit expanded into the surrounding
/// conversation. A single matching message rarely carries the full meaning in a
/// group chat (the question and its answer are usually separate messages), so we
/// return a context window around each match.
pub async fn query_chat_history(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("query_chat_history: missing 'query'".into()))?;
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(8) as u32;

    let hits =
        super::rag_pipeline::query_chat_history(ctx.env, ctx.workspace_slug, query, limit).await?;

    let mut results = Vec::with_capacity(hits.len());
    for h in &hits {
        // Expand the match into a conversational window (best-effort: a missing
        // anchor — e.g. a legacy vector — just yields the match alone).
        let context = if h.anchor_id.is_empty() {
            Vec::new()
        } else {
            let window = ctx
                .db
                .get_messages_around(&h.anchor_id, MAX_SIDE_MSGS, MAX_SIDE_MSGS)
                .await
                .unwrap_or_default();
            trim_to_budget(&window, &h.anchor_id, SIDE_BUDGET_CHARS)
        };

        results.push(serde_json::json!({
            "match": {
                "sender_name": h.sender_name,
                "timestamp": h.timestamp,
                "text": h.text,
                "score": h.score,
                "anchor_id": h.anchor_id,
            },
            "context": context.iter().map(|m| serde_json::json!({
                "sender_name": m.sender_name,
                "text": m.text,
                "created_at": m.created_at,
                "is_match": m.id == h.anchor_id,
            })).collect::<Vec<_>>(),
        }));
    }

    Ok(serde_json::json!({ "results": results }))
}

/// Agentic context expansion: let the model pull more messages around a known
/// anchor (the `anchor_id` returned by `query_chat_history`) when one window
/// isn't enough (multi-hop reasoning).
pub async fn read_chat_around(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let anchor_id = args
        .get("anchor_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("read_chat_around: missing 'anchor_id'".into()))?;
    // Bound each side to keep the tool from dragging the whole history in.
    let before = args.get("before").and_then(|v| v.as_i64()).unwrap_or(5).clamp(0, 25);
    let after = args.get("after").and_then(|v| v.as_i64()).unwrap_or(5).clamp(0, 25);

    let window = ctx.db.get_messages_around(anchor_id, before, after).await?;

    Ok(serde_json::json!({
        "messages": window.iter().map(|m| serde_json::json!({
            "sender_name": m.sender_name,
            "text": m.text,
            "created_at": m.created_at,
            "is_anchor": m.id == anchor_id,
        })).collect::<Vec<_>>()
    }))
}

/// Keep the anchor plus as many neighbours as fit within `budget` characters on
/// each side. `window` is chronological and includes the anchor.
fn trim_to_budget(window: &[ChatMessage], anchor_id: &str, budget: usize) -> Vec<ChatMessage> {
    let Some(anchor_idx) = window.iter().position(|m| m.id == anchor_id) else {
        return window.to_vec(); // anchor not in window: return what we have
    };

    // Walk left from the anchor, accumulating until the budget is exceeded.
    let mut start = anchor_idx;
    let mut acc = 0usize;
    while start > 0 {
        acc += window[start - 1].text.chars().count();
        if acc > budget {
            break;
        }
        start -= 1;
    }

    // Walk right from the anchor.
    let mut end = anchor_idx; // inclusive index of last kept message
    acc = 0;
    while end + 1 < window.len() {
        acc += window[end + 1].text.chars().count();
        if acc > budget {
            break;
        }
        end += 1;
    }

    window[start..=end].to_vec()
}
