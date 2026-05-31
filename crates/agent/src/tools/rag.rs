//! Tool implementation: query_chat_history.

use super::ToolContext;
use serde_json::Value;

pub async fn query_chat_history(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("query_chat_history: missing 'query'".into()))?;
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as u32;

    let hits =
        super::rag_pipeline::query_chat_history(ctx.env, ctx.workspace_slug, query, limit).await?;

    Ok(serde_json::json!({
        "results": hits.iter().map(|h| serde_json::json!({
            "sender_name": h.sender_name,
            "timestamp": h.timestamp,
            "text": h.text,
            "score": h.score,
        })).collect::<Vec<_>>()
    }))
}
