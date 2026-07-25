//! Tool implementation: list_calendar.
//! Aggregates events + todos-with-deadline + reminders + scheduled actions in a date range.

use super::ToolContext;
use serde_json::Value;

/// Parse the optional `types` filter into the set of categories to include.
/// Absent/empty → include everything (matches the schema's implied default,
/// since `types` isn't `required`). An unrecognized entry is ignored rather
/// than rejected — the model shouldn't fail the whole call over one typo.
fn requested_types(args: &Value) -> std::collections::HashSet<String> {
    match args.get("types").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::to_string)
            .collect(),
        _ => ["todo", "event", "reminder", "scheduled"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    }
}

pub async fn list_calendar(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let from = args
        .get("from")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("list_calendar: missing 'from'".into()))?;
    let to = args
        .get("to")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("list_calendar: missing 'to'".into()))?;
    let types = requested_types(&args);

    let events = if types.contains("event") {
        ctx.db.list_events_in_range(from, to).await?
    } else {
        vec![]
    };
    let todos = if types.contains("todo") {
        ctx.db.list_todos_with_deadline(from, to).await?
    } else {
        vec![]
    };
    let reminders = if types.contains("reminder") {
        ctx.db.list_reminders_in_range(from, to).await?
    } else {
        vec![]
    };
    let scheduled = if types.contains("scheduled") {
        ctx.db.list_scheduled_in_range(from, to).await?
    } else {
        vec![]
    };

    Ok(serde_json::json!({
        "events": events.iter().map(|e| serde_json::json!({
            "id": e.id,
            "title": e.title,
            "starts_at": e.starts_at.to_rfc3339(),
            "ends_at": e.ends_at.map(|d| d.to_rfc3339()),
            "all_day": e.all_day,
            "location": e.location,
        })).collect::<Vec<_>>(),
        "todos": todos,
        "reminders": reminders,
        "scheduled": scheduled.iter().map(|a| serde_json::json!({
            "id": a.id,
            "title": a.title,
            "trigger_at": a.trigger_at.to_rfc3339(),
            "action_type": serde_json::to_value(&a.action_type).unwrap_or_default(),
        })).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::requested_types;

    #[test]
    fn no_types_key_includes_everything() {
        let t = requested_types(&serde_json::json!({}));
        assert_eq!(t.len(), 4);
        for k in ["todo", "event", "reminder", "scheduled"] {
            assert!(t.contains(k), "missing {k}");
        }
    }

    #[test]
    fn empty_types_array_includes_everything() {
        let t = requested_types(&serde_json::json!({ "types": [] }));
        assert_eq!(t.len(), 4);
    }

    #[test]
    fn explicit_types_filters_to_only_those() {
        let t = requested_types(&serde_json::json!({ "types": ["todo", "event"] }));
        assert_eq!(t.len(), 2);
        assert!(t.contains("todo"));
        assert!(t.contains("event"));
        assert!(!t.contains("reminder"));
        assert!(!t.contains("scheduled"));
    }

    #[test]
    fn unrecognized_type_entry_is_ignored_not_rejected() {
        // A typo'd entry doesn't reject the whole call and doesn't smuggle in
        // any of the four real categories — it's simply never matched by the
        // `.contains("event")`-style checks in `list_calendar`.
        let t = requested_types(&serde_json::json!({ "types": ["todo", "bogus"] }));
        assert!(t.contains("todo"));
        assert!(!t.contains("event"));
        assert!(!t.contains("reminder"));
        assert!(!t.contains("scheduled"));
    }
}
