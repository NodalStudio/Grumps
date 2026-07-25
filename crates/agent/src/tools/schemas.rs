//! Static JSON schemas for the agent tools sent to Anthropic API.
//! See spec § 8.4.

use serde_json::{json, Value};

pub fn all_tools() -> Vec<Value> {
    vec![
        query_memory(),
        query_chat_history(),
        read_chat_around(),
        save_memory(),
        create_todo(),
        create_note(),
        create_event(),
        create_reminder(),
        complete_todo(),
        update_todo(),
        delete_todo(),
        update_note(),
        delete_note(),
        schedule_action(),
        cancel_scheduled(),
        update_scheduled(),
        list_calendar(),
        list_todos(),
        list_notes(),
        web_search(),
        send_message(),
    ]
}

pub fn query_memory() -> Value {
    json!({
        "name": "query_memory",
        "description": "Search the workspace's structured memory (facts, people, decisions, preferences). Use when the user asks about something the group knows or to recall who/what/when.",
        "input_schema": {
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Free-text search query" },
                "kind": {
                    "type": "string",
                    "enum": ["fact", "person", "decision", "preference", "place", "other"],
                    "description": "Optional filter by memory kind"
                },
                "limit": { "type": "integer", "default": 10 }
            },
            "required": ["query"]
        }
    })
}

pub fn query_chat_history() -> Value {
    json!({
        "name": "query_chat_history",
        "description": "Semantic search over this group's past chat messages. Call this WHENEVER the answer depends on something said earlier that is not in the messages currently in front of you — e.g. the user refers to a past discussion, asks 'what did we decide/say about X', 'when did X happen', 'what was that thing about…', or mentions a person/plan/topic you have no current context for. When in doubt, search rather than answering from memory — the recent messages you can see are only a small window of the full history. Each result is a matching message PLUS the surrounding conversation (a context window) and an `anchor_id`; if that window is still not enough, call `read_chat_around` with the `anchor_id` to pull more.",
        "input_schema": {
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "from": { "type": "string", "format": "date-time", "description": "Optional ISO 8601 lower bound" },
                "to": { "type": "string", "format": "date-time", "description": "Optional ISO 8601 upper bound" },
                "limit": { "type": "integer", "default": 8 }
            },
            "required": ["query"]
        }
    })
}

pub fn read_chat_around() -> Value {
    json!({
        "name": "read_chat_around",
        "description": "Read the messages immediately before and after a known message, in chronological order. Use to expand the context around a `query_chat_history` result (pass its `anchor_id`) when the answer might be in a neighbouring message.",
        "input_schema": {
            "type": "object",
            "properties": {
                "anchor_id": { "type": "string", "description": "The anchor_id of a message returned by query_chat_history" },
                "before": { "type": "integer", "default": 5, "description": "How many earlier messages to include (max 25)" },
                "after": { "type": "integer", "default": 5, "description": "How many later messages to include (max 25)" }
            },
            "required": ["anchor_id"]
        }
    })
}

pub fn save_memory() -> Value {
    json!({
        "name": "save_memory",
        "description": "Persist a fact, decision, person info, or preference for this workspace. Use sparingly — only for things that will matter later.",
        "input_schema": {
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "Short slug, e.g. 'wifi-bureau'" },
                "value": { "type": "string", "description": "The actual content" },
                "kind": {
                    "type": "string",
                    "enum": ["fact", "person", "decision", "preference", "place", "other"]
                },
                "related_member": { "type": "string", "description": "member_id if about a person" },
                "expires_at": { "type": "string", "format": "date-time", "description": "Optional TTL, e.g. for vacation status" },
                "pinned": { "type": "boolean", "default": false }
            },
            "required": ["value", "kind"]
        }
    })
}

pub fn create_todo() -> Value {
    json!({
        "name": "create_todo",
        "description": "Create a new todo item in the group's list.",
        "input_schema": {
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "assignee": { "type": "string", "description": "member_id or display_name" },
                "deadline": { "type": "string", "format": "date-time" },
                "priority": { "type": "integer", "minimum": 1, "maximum": 3, "description": "1=high, 2=normal, 3=low" },
                "tags": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["title"]
        }
    })
}

pub fn create_note() -> Value {
    json!({
        "name": "create_note",
        "description": "Create a new note (markdown content) in the group's notes.",
        "input_schema": {
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "content": { "type": "string", "description": "Markdown body" }
            },
            "required": ["content"]
        }
    })
}

pub fn create_event() -> Value {
    // No `attendees`/`recurrence` fields — the implementation ignores both
    // (grumps_calendar::NewEvent is built with `attendees: vec![]` and
    // `recurrence: None` regardless of input). Advertising fields the tool
    // silently drops would make the model promise something it can't keep.
    json!({
        "name": "create_event",
        "description": "Create a calendar event (meeting, appointment, birthday, etc). Does not support attendees or recurrence.",
        "input_schema": {
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "starts_at": { "type": "string", "format": "date-time", "description": "Local wall-clock time in the group's timezone (see CURRENT DATETIME in your context), ISO-8601 with NO timezone suffix, e.g. 2026-05-31T20:00:00. Do not convert to UTC." },
                "ends_at": { "type": "string", "format": "date-time", "description": "Optional end time, same local-wall-clock format as starts_at." },
                "all_day": { "type": "boolean", "default": false },
                "location": { "type": "string" }
            },
            "required": ["title", "starts_at"]
        }
    })
}

pub fn create_reminder() -> Value {
    json!({
        "name": "create_reminder",
        "description": "Schedule a passive reminder message in the group at a future time.",
        "input_schema": {
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "The reminder message text" },
                "trigger_at": { "type": "string", "format": "date-time", "description": "Local wall-clock time in the group's timezone (see CURRENT DATETIME in your context), ISO-8601 with NO timezone suffix, e.g. 2026-05-31T20:00:00. Do not convert to UTC." },
                "recurrence": { "type": "string", "description": "Optional RRULE" }
            },
            "required": ["text", "trigger_at"]
        }
    })
}

pub fn complete_todo() -> Value {
    json!({
        "name": "complete_todo",
        "description": "Mark an existing todo done, by its seq number (the # shown on its card, e.g. #3). If the todo recurs, its next occurrence is created automatically. Only call this for a todo the user clearly identified — if you're not sure which one they mean, call list_todos first and ask.",
        "input_schema": {
            "type": "object",
            "properties": {
                "seq": { "type": "integer", "description": "The todo's seq number, e.g. 3 for '#3'" }
            },
            "required": ["seq"]
        }
    })
}

pub fn update_todo() -> Value {
    json!({
        "name": "update_todo",
        "description": "Change one or more fields of an existing todo, by its seq number. Only set the fields that changed. `tags` are appended to the existing tags (not replaced). To mark it done, use complete_todo instead (it also spawns the next occurrence for a recurring todo).",
        "input_schema": {
            "type": "object",
            "properties": {
                "seq": { "type": "integer", "description": "The todo's seq number, e.g. 3 for '#3'" },
                "title": { "type": "string" },
                "deadline": { "type": "string", "format": "date-time", "description": "New deadline. Pass an empty string to clear it." },
                "priority": { "type": "integer", "minimum": 1, "maximum": 3, "description": "1=high, 2=normal, 3=low" },
                "assignee": { "type": "string", "description": "member_id or display_name" },
                "tags": { "type": "array", "items": { "type": "string" }, "description": "Tags to add" },
                "status": { "type": "string", "description": "Free-form status label (e.g. 'in_progress', 'blocked') — not for marking done, see complete_todo." }
            },
            "required": ["seq"]
        }
    })
}

pub fn delete_todo() -> Value {
    json!({
        "name": "delete_todo",
        "description": "Permanently remove a todo, by its seq number. Destructive — only call this when the user explicitly asks to delete/remove that specific todo, never proactively. If you're not sure which one they mean, call list_todos first and ask.",
        "input_schema": {
            "type": "object",
            "properties": {
                "seq": { "type": "integer", "description": "The todo's seq number, e.g. 3 for '#3'" }
            },
            "required": ["seq"]
        }
    })
}

pub fn update_note() -> Value {
    json!({
        "name": "update_note",
        "description": "Change an existing note's title and/or content, identified by its id (from list_notes) or its exact title. Only set the fields that changed.",
        "input_schema": {
            "type": "object",
            "properties": {
                "id_or_title": { "type": "string", "description": "The note's id, or its exact title" },
                "title": { "type": "string" },
                "content": { "type": "string", "description": "Markdown body" }
            },
            "required": ["id_or_title"]
        }
    })
}

pub fn delete_note() -> Value {
    json!({
        "name": "delete_note",
        "description": "Permanently remove a note, identified by its id (from list_notes) or its exact title. Destructive — only call this when the user explicitly asks to delete/remove that specific note, never proactively.",
        "input_schema": {
            "type": "object",
            "properties": {
                "id_or_title": { "type": "string", "description": "The note's id, or its exact title" }
            },
            "required": ["id_or_title"]
        }
    })
}

pub fn schedule_action() -> Value {
    json!({
        "name": "schedule_action",
        "description": "Schedule a complex agentic task to run later (e.g. weekly recap, conditional follow-up). The agent will run autonomously at trigger_at.",
        "input_schema": {
            "type": "object",
            "properties": {
                "action_type": {
                    "type": "string",
                    "enum": ["reminder", "follow_up", "recap", "agent_task", "event_notify"]
                },
                "title": { "type": "string", "description": "Human summary" },
                "trigger_at": { "type": "string", "format": "date-time", "description": "Local wall-clock time in the group's timezone (see CURRENT DATETIME in your context), ISO-8601 with NO timezone suffix, e.g. 2026-05-31T20:00:00. Do not convert to UTC." },
                "recurrence": { "type": "string" },
                "condition": { "type": "object", "description": "Optional condition JSON" },
                "payload": { "type": "object", "description": "Action payload (instruction string for agent_task, etc.)" }
            },
            "required": ["action_type", "title", "trigger_at", "payload"]
        }
    })
}

pub fn cancel_scheduled() -> Value {
    json!({
        "name": "cancel_scheduled",
        "description": "Cancel a scheduled action (reminder or agentic task) created by schedule_action or create_reminder, by its id. Destructive — only call this when the user explicitly asks to cancel/stop it, never proactively.",
        "input_schema": {
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The scheduled action's id" }
            },
            "required": ["id"]
        }
    })
}

pub fn update_scheduled() -> Value {
    json!({
        "name": "update_scheduled",
        "description": "Change one or more fields of an existing scheduled action (reminder or agentic task), by its id. Only set the fields that changed.",
        "input_schema": {
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The scheduled action's id" },
                "title": { "type": "string" },
                "trigger_at": { "type": "string", "format": "date-time", "description": "New local wall-clock time in the group's timezone, ISO-8601 with NO timezone suffix. Do not convert to UTC." },
                "recurrence": { "type": "string", "description": "New RRULE. Pass an empty string to clear it." }
            },
            "required": ["id"]
        }
    })
}

pub fn list_calendar() -> Value {
    json!({
        "name": "list_calendar",
        "description": "Read upcoming todos, reminders, events for a date range.",
        "input_schema": {
            "type": "object",
            "properties": {
                "from": { "type": "string", "format": "date-time" },
                "to": { "type": "string", "format": "date-time" },
                "types": {
                    "type": "array",
                    "items": { "type": "string", "enum": ["todo", "event", "reminder", "scheduled"] }
                }
            },
            "required": ["from", "to"]
        }
    })
}

pub fn list_todos() -> Value {
    json!({
        "name": "list_todos",
        "description": "Read the group's existing todos. Call this whenever the user asks what's on the todo list, what's pending/done, or anything about existing todos — never guess or claim the list is empty without calling this first.",
        "input_schema": {
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["open", "done", "all"],
                    "default": "open",
                    "description": "Which todos to return. Defaults to open (not done)."
                }
            }
        }
    })
}

pub fn list_notes() -> Value {
    json!({
        "name": "list_notes",
        "description": "Read the group's existing notes. Call this whenever the user asks what notes exist or to recall a note's content — never guess or claim there are no notes without calling this first.",
        "input_schema": {
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "default": 20,
                    "maximum": 50,
                    "description": "Max notes to return, most recent first."
                }
            }
        }
    })
}

pub fn web_search() -> Value {
    json!({
        "name": "web_search",
        "description": "Search the web. Use for current information not in workspace memory (restaurants, addresses, news, prices, hours).",
        "input_schema": {
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "count": { "type": "integer", "default": 5, "maximum": 10 },
                "freshness": {
                    "type": "string",
                    "enum": ["pd", "pw", "pm", "py", "all"],
                    "description": "past day/week/month/year/all"
                }
            },
            "required": ["query"]
        }
    })
}

pub fn send_message() -> Value {
    json!({
        "name": "send_message",
        "description": "Send a formatted message to the group chat. Most responses are returned implicitly via your final text — use this only to send extra messages mid-flow.",
        "input_schema": {
            "type": "object",
            "properties": {
                "text": { "type": "string" }
            },
            "required": ["text"]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tools_count_is_21() {
        assert_eq!(all_tools().len(), 21);
    }

    #[test]
    fn each_tool_has_name_and_input_schema() {
        for tool in all_tools() {
            assert!(
                tool.get("name").and_then(|n| n.as_str()).is_some(),
                "missing name in {tool}"
            );
            assert!(
                tool.get("description").is_some(),
                "missing description in {tool}"
            );
            let schema = tool.get("input_schema").expect("missing input_schema");
            assert_eq!(schema["type"], "object");
            assert!(schema.get("properties").is_some());
        }
    }

    #[test]
    fn tool_names_unique() {
        let names: Vec<String> = all_tools()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "duplicate tool names");
    }

    /// The set of top-level `input_schema.properties` keys a tool advertises.
    fn schema_keys(tool: &Value) -> std::collections::HashSet<String> {
        tool["input_schema"]["properties"]
            .as_object()
            .expect("properties object")
            .keys()
            .cloned()
            .collect()
    }

    fn set(keys: &[&str]) -> std::collections::HashSet<String> {
        keys.iter().map(|s| s.to_string()).collect()
    }

    // ── schema-vs-impl field-name parity for every new mutation tool ──────
    // Each asserted set must exactly match the `args.get("...")` calls in
    // the corresponding `tools::crud`/`tools::scheduler` implementation —
    // catches a schema field the impl never reads (or vice versa).

    #[test]
    fn complete_todo_schema_matches_impl_fields() {
        assert_eq!(schema_keys(&complete_todo()), set(&["seq"]));
    }

    #[test]
    fn update_todo_schema_matches_impl_fields() {
        assert_eq!(
            schema_keys(&update_todo()),
            set(&["seq", "title", "deadline", "priority", "assignee", "tags", "status"])
        );
    }

    #[test]
    fn delete_todo_schema_matches_impl_fields() {
        assert_eq!(schema_keys(&delete_todo()), set(&["seq"]));
    }

    #[test]
    fn update_note_schema_matches_impl_fields() {
        assert_eq!(
            schema_keys(&update_note()),
            set(&["id_or_title", "title", "content"])
        );
    }

    #[test]
    fn delete_note_schema_matches_impl_fields() {
        assert_eq!(schema_keys(&delete_note()), set(&["id_or_title"]));
    }

    #[test]
    fn cancel_scheduled_schema_matches_impl_fields() {
        assert_eq!(schema_keys(&cancel_scheduled()), set(&["id"]));
    }

    #[test]
    fn update_scheduled_schema_matches_impl_fields() {
        assert_eq!(
            schema_keys(&update_scheduled()),
            set(&["id", "title", "trigger_at", "recurrence"])
        );
    }

    #[test]
    fn create_event_schema_does_not_advertise_ignored_fields() {
        // Honesty fix: the implementation never reads `attendees`/`recurrence`
        // (grumps_calendar::NewEvent is built with attendees: vec![],
        // recurrence: None regardless of input) — the schema must not
        // promise fields the tool silently drops.
        let keys = schema_keys(&create_event());
        assert!(!keys.contains("attendees"));
        assert!(!keys.contains("recurrence"));
        assert_eq!(
            keys,
            set(&["title", "starts_at", "ends_at", "all_day", "location"])
        );
    }
}
