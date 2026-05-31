//! Tool implementations: query_memory, save_memory.

use super::ToolContext;
use grumps_memory::{MemoryKind, MemorySource, NewMemoryEntry};
use serde_json::Value;

pub async fn query_memory(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("query_memory: missing 'query'".into()))?;
    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(10);

    let entries = ctx.db.search_memory_fts(query, limit).await?;
    Ok(serde_json::json!({
        "results": entries.iter().map(|e| serde_json::json!({
            "id": e.id,
            "key": e.key,
            "value": e.value,
            "kind": serde_json::to_value(&e.kind).unwrap_or_default(),
            "pinned": e.pinned,
            "tags": e.tags,
        })).collect::<Vec<_>>()
    }))
}

pub async fn save_memory(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let value = args
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("save_memory: missing 'value'".into()))?;

    let key = args.get("key").and_then(|v| v.as_str()).map(String::from);
    let kind_str = args.get("kind").and_then(|v| v.as_str()).unwrap_or("other");
    let kind: MemoryKind =
        serde_json::from_value(Value::String(kind_str.to_string())).unwrap_or(MemoryKind::Other);
    let tags: Vec<String> = args
        .get("tags")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let pinned = args.get("pinned").and_then(|v| v.as_bool());

    let entry = NewMemoryEntry {
        key,
        value: value.to_string(),
        kind,
        source: MemorySource::Agent,
        tags,
        pinned,
        created_by: Some(ctx.member_id.to_string()),
        ..Default::default()
    };

    let id = ctx.db.create_memory(&entry).await?;
    Ok(serde_json::json!({ "id": id, "saved": true }))
}
