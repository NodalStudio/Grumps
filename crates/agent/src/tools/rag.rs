//! Tool implementation: query_chat_history.

use super::{args, parse_args, ToolContext};
use serde_json::Value;

pub async fn query_chat_history(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::QueryChatHistoryArgs = parse_args(raw, "query_chat_history")?;
    let limit = a.limit.unwrap_or(5) as u32;

    let hits =
        super::rag_pipeline::query_chat_history(ctx.env, ctx.workspace_slug, &a.query, limit)
            .await?;

    Ok(serde_json::json!({
        "ok": true,
        "results": hits.iter().map(|h| serde_json::json!({
            "sender_name": h.sender_name,
            "timestamp": h.timestamp,
            "text": h.text,
            "score": h.score,
        })).collect::<Vec<_>>()
    }))
}
