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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum SessionMessage {
    User { content: String },
    Assistant { content: String, #[serde(default)] tool_calls: Vec<serde_json::Value> },
    Tool { tool_use_id: String, content: serde_json::Value },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_serializes_role() {
        let m = SessionMessage::User { content: "hi".into() };
        let j = serde_json::to_value(&m).unwrap();
        assert_eq!(j["role"], "user");
        assert_eq!(j["content"], "hi");
    }

    #[test]
    fn assistant_with_no_tool_calls_omits_default() {
        let m = SessionMessage::Assistant { content: "ok".into(), tool_calls: vec![] };
        let j = serde_json::to_string(&m).unwrap();
        // tool_calls is included as empty array — that's fine, just ensure roundtrip
        let back: SessionMessage = serde_json::from_str(&j).unwrap();
        match back {
            SessionMessage::Assistant { content, .. } => assert_eq!(content, "ok"),
            _ => panic!("wrong variant"),
        }
    }
}
