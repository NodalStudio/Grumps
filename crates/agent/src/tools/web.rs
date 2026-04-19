//! Tool implementation: web_search (stub — Plan D).

use serde_json::Value;
use super::ToolContext;

pub async fn web_search(_ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    worker::console_log!("web_search stub called with query='{query}'");
    Ok(serde_json::json!({
        "results": [],
        "note": "Web search not yet implemented (arrives in Plan D)",
        "query": query,
    }))
}
