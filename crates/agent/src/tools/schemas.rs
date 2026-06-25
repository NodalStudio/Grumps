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
        schedule_action(),
        list_calendar(),
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
        "description": "Semantic search over past chat messages in this group. Use when the user asks 'what did we say about...', 'when did X happen', etc. Each result is a matching message PLUS the surrounding conversation (a context window) and an `anchor_id`. If a window is still not enough, call `read_chat_around` with that `anchor_id` to pull more.",
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
    json!({
        "name": "create_event",
        "description": "Create a calendar event (meeting, appointment, birthday, etc).",
        "input_schema": {
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "starts_at": { "type": "string", "format": "date-time", "description": "Local wall-clock time in the group's timezone (see CURRENT DATETIME in your context), ISO-8601 with NO timezone suffix, e.g. 2026-05-31T20:00:00. Do not convert to UTC." },
                "ends_at": { "type": "string", "format": "date-time", "description": "Optional end time, same local-wall-clock format as starts_at." },
                "all_day": { "type": "boolean", "default": false },
                "location": { "type": "string" },
                "attendees": { "type": "array", "items": { "type": "string" }, "description": "member_ids" },
                "recurrence": { "type": "string", "description": "RRULE format, e.g. 'FREQ=WEEKLY;BYDAY=MO'" }
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
    fn all_tools_count_is_11() {
        assert_eq!(all_tools().len(), 11);
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
}
