use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub id: String,
    pub actor: Option<String>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub source: String,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_entry_serialization_roundtrip() {
        let entry = ActivityEntry {
            id: "a-1".to_string(),
            actor: Some("user-1".to_string()),
            action: "todo.created".to_string(),
            target_type: Some("todo".to_string()),
            target_id: Some("todo-1".to_string()),
            source: "slack".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: ActivityEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, entry.id);
        assert_eq!(back.action, entry.action);
        assert_eq!(back.actor, Some("user-1".to_string()));
    }

    #[test]
    fn activity_entry_optional_fields_none() {
        let entry = ActivityEntry {
            id: "a-2".to_string(),
            actor: None,
            action: "workspace.created".to_string(),
            target_type: None,
            target_id: None,
            source: "system".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: ActivityEntry = serde_json::from_str(&json).expect("deserialize");
        assert!(back.actor.is_none());
        assert!(back.target_type.is_none());
        assert!(back.target_id.is_none());
    }
}
