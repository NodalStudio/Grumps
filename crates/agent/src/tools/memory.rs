//! Tool implementations: query_memory, save_memory.

use super::{args, parse_args, ToolContext};
use grumps_memory::{MemorySource, NewMemoryEntry};
use serde_json::Value;

pub async fn query_memory(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::QueryMemoryArgs = parse_args(raw, "query_memory")?;
    let limit = a.limit.unwrap_or(10);

    let entries = ctx.db.search_memory_fts(&a.query, limit).await?;
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

pub async fn save_memory(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::SaveMemoryArgs = parse_args(raw, "save_memory")?;

    let entry = NewMemoryEntry {
        key: a.key,
        value: a.value,
        kind: a.kind.into(),
        source: MemorySource::Agent,
        tags: a.tags.unwrap_or_default(),
        pinned: a.pinned,
        created_by: Some(ctx.member_id.to_string()),
        ..Default::default()
    };

    let id = ctx.db.create_memory(&entry).await?;
    Ok(serde_json::json!({ "id": id, "saved": true }))
}
