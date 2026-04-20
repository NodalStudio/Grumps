use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub member_id: String,
    pub last_message_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub messages: Vec<SessionMessage>,
    pub pending_action: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Mirrors Anthropic's Message shape for lossless session storage.
/// `content` may be a plain string (simple turns) or a JSON array (tool_use / tool_result turns).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String,           // "user" | "assistant"
    pub content: serde_json::Value, // String OR Vec of content blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_serializes_role() {
        let m = SessionMessage { role: "user".into(), content: serde_json::Value::String("hi".into()) };
        let j = serde_json::to_value(&m).unwrap();
        assert_eq!(j["role"], "user");
        assert_eq!(j["content"], "hi");
    }

    #[test]
    fn assistant_with_tool_array_roundtrips() {
        let content = serde_json::json!([{"type": "tool_use", "id": "x", "name": "foo", "input": {}}]);
        let m = SessionMessage { role: "assistant".into(), content: content.clone() };
        let j = serde_json::to_string(&m).unwrap();
        let back: SessionMessage = serde_json::from_str(&j).unwrap();
        assert_eq!(back.role, "assistant");
        assert_eq!(back.content, content);
    }
}
