// crates/nlu/src/llm.rs
use serde::{Deserialize, Serialize};

/// The structured output we ask the LLM to return.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NluResponse {
    pub intent: NluIntent,
    pub confidence: f32, // 0.0 - 1.0
    pub entities: NluEntities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NluIntent {
    AddTodo,
    CompleteTodo,
    AddNote,
    SetReminder,
    ListTodos,
    ListNotes,
    SearchNotes,
    DeleteTodo,
    Summarize,
    Help,
    Status,
    Irrelevant,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NluEntities {
    pub title: Option<String>,
    pub assignee: Option<String>,
    pub deadline: Option<String>, // ISO 8601 or relative like "tomorrow"
    pub priority: Option<String>, // "high", "normal", "low"
    pub tags: Vec<String>,
    pub search_query: Option<String>,
    pub target_id: Option<i64>,     // #42
    pub recurrence: Option<String>, // "every monday", "daily"
}

/// System prompt for NLU. Sent with every request.
pub const NLU_SYSTEM_PROMPT: &str = r#"You are the NLU engine for Grumps, a WhatsApp group task bot. Parse the user's message and return a JSON object.

Context: The user is chatting in a group where Grumps manages todos, notes, files, and reminders. The message was directed at the bot (@grumps mention or DM).

Return EXACTLY this JSON structure:
{
  "intent": "<one of: add_todo, complete_todo, add_note, set_reminder, list_todos, list_notes, search_notes, delete_todo, summarize, help, status, irrelevant>",
  "confidence": <0.0 to 1.0>,
  "entities": {
    "title": "<extracted task/note title or null>",
    "assignee": "<@mentioned person or null>",
    "deadline": "<concrete local datetime, ISO-8601 'YYYY-MM-DDTHH:MM:SS' with NO timezone suffix, or null>",
    "priority": "<high/normal/low or null>",
    "tags": ["<extracted #tags>"],
    "search_query": "<search text or null>",
    "target_id": <todo number like 42, or null>,
    "recurrence": "<recurrence pattern like 'every monday' or null>"
  }
}

Rules:
- If the message is clearly a task/action item, intent = "add_todo"
- If the user wants to mark something done, intent = "complete_todo"
- If the user is sharing info worth pinning, intent = "add_note"
- If the user asks to be reminded, intent = "set_reminder"
- If the message is casual chat not meant for the bot, intent = "irrelevant"
- Extract ALL entities you can find, even if the intent is simple
- The user message starts with the current local date/time and timezone. Resolve EVERY date/time against it.
- Output "deadline" as a CONCRETE local wall-clock timestamp, ISO-8601 "YYYY-MM-DDTHH:MM:SS" with NO timezone suffix and NO offset. Never return relative text ("tomorrow", "friday", "in 2 hours") and never convert to UTC. E.g. if it's 2026-05-30 and the user says "tomorrow at 9", return "2026-05-31T09:00:00".
- confidence < 0.5 means you're guessing — the system will ask for clarification
- Always respond with valid JSON only, no markdown, no explanation"#;

/// Build the user message for the LLM.
///
/// `now_local` is the current wall-clock time in the workspace timezone
/// (ISO-8601, no offset); it anchors the model's relative-date resolution.
pub fn build_user_prompt(
    message: &str,
    sender_name: &str,
    existing_todos: &[(i64, String)],
    now_local: &str,
    timezone: &str,
) -> String {
    let mut prompt = format!(
        "Current local time: {} ({}). Resolve all relative dates against this.\nSender: {}\nMessage: {}",
        now_local, timezone, sender_name, message
    );
    if !existing_todos.is_empty() {
        prompt.push_str("\n\nOpen todos in this group:");
        for (seq, title) in existing_todos.iter().take(20) {
            prompt.push_str(&format!("\n  #{} {}", seq, title));
        }
    }
    prompt
}

/// Parse the LLM's JSON response into our structured type.
pub fn parse_llm_response(text: &str) -> Result<NluResponse, String> {
    // Strip markdown code fences if present
    let clean = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str(clean).map_err(|e| format!("Failed to parse LLM response: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_response() {
        let json = r#"{"intent":"add_todo","confidence":0.95,"entities":{"title":"Buy groceries","assignee":"Bob","deadline":"2026-04-18","priority":"high","tags":["shopping"],"search_query":null,"target_id":null,"recurrence":null}}"#;
        let resp = parse_llm_response(json).unwrap();
        assert_eq!(resp.intent, NluIntent::AddTodo);
        assert!(resp.confidence > 0.9);
        assert_eq!(resp.entities.title, Some("Buy groceries".into()));
        assert_eq!(resp.entities.assignee, Some("Bob".into()));
        assert_eq!(resp.entities.tags, vec!["shopping"]);
    }

    #[test]
    fn parse_with_code_fences() {
        let json = "```json\n{\"intent\":\"help\",\"confidence\":1.0,\"entities\":{\"title\":null,\"assignee\":null,\"deadline\":null,\"priority\":null,\"tags\":[],\"search_query\":null,\"target_id\":null,\"recurrence\":null}}\n```";
        let resp = parse_llm_response(json).unwrap();
        assert_eq!(resp.intent, NluIntent::Help);
    }

    #[test]
    fn parse_reminder() {
        let json = r#"{"intent":"set_reminder","confidence":0.9,"entities":{"title":"Take out the trash","assignee":"Bob","deadline":"2026-04-14T09:00:00","priority":null,"tags":[],"search_query":null,"target_id":null,"recurrence":"every monday"}}"#;
        let resp = parse_llm_response(json).unwrap();
        assert_eq!(resp.intent, NluIntent::SetReminder);
        assert_eq!(resp.entities.recurrence, Some("every monday".into()));
    }

    #[test]
    fn parse_complete_by_id() {
        let json = r#"{"intent":"complete_todo","confidence":0.95,"entities":{"title":null,"assignee":null,"deadline":null,"priority":null,"tags":[],"search_query":null,"target_id":42,"recurrence":null}}"#;
        let resp = parse_llm_response(json).unwrap();
        assert_eq!(resp.intent, NluIntent::CompleteTodo);
        assert_eq!(resp.entities.target_id, Some(42));
    }

    #[test]
    fn parse_irrelevant() {
        let json = r#"{"intent":"irrelevant","confidence":0.8,"entities":{"title":null,"assignee":null,"deadline":null,"priority":null,"tags":[],"search_query":null,"target_id":null,"recurrence":null}}"#;
        let resp = parse_llm_response(json).unwrap();
        assert_eq!(resp.intent, NluIntent::Irrelevant);
    }

    #[test]
    fn invalid_json_errors() {
        assert!(parse_llm_response("not json").is_err());
    }

    #[test]
    fn build_prompt_with_todos() {
        let todos = vec![(1, "Buy bread".into()), (2, "Call plumber".into())];
        let prompt = build_user_prompt(
            "remind bob to pay rent",
            "Alice",
            &todos,
            "2026-05-30T14:00:00",
            "Europe/Paris",
        );
        assert!(prompt.contains("Alice"));
        assert!(prompt.contains("remind bob to pay rent"));
        assert!(prompt.contains("#1 Buy bread"));
    }

    #[test]
    fn build_prompt_includes_current_time() {
        let prompt = build_user_prompt(
            "remind me tomorrow",
            "Alice",
            &[],
            "2026-05-30T14:00:00",
            "Europe/Paris",
        );
        assert!(prompt.contains("2026-05-30T14:00:00"));
        assert!(prompt.contains("Europe/Paris"));
    }

    #[test]
    fn build_prompt_empty_todos() {
        let prompt = build_user_prompt("hello", "Alice", &[], "2026-05-30T14:00:00", "UTC");
        assert!(!prompt.contains("Open todos"));
    }
}
