use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    pub slug: String,
    pub platform: String,
    pub platform_channel_id: String,
    pub name: Option<String>,
    pub plan: String,
    pub d1_database_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_meta_serialization_roundtrip() {
        let ws = WorkspaceMeta {
            slug: "my-team".to_string(),
            platform: "slack".to_string(),
            platform_channel_id: "C12345".to_string(),
            name: Some("My Team".to_string()),
            plan: "free".to_string(),
            d1_database_id: "db-uuid-1".to_string(),
        };
        let json = serde_json::to_string(&ws).expect("serialize");
        let back: WorkspaceMeta = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.slug, ws.slug);
        assert_eq!(back.platform, ws.platform);
        assert_eq!(back.name, Some("My Team".to_string()));
    }

    #[test]
    fn workspace_meta_optional_name_none() {
        let ws = WorkspaceMeta {
            slug: "anon-team".to_string(),
            platform: "whatsapp".to_string(),
            platform_channel_id: "group-123".to_string(),
            name: None,
            plan: "pro".to_string(),
            d1_database_id: "db-uuid-2".to_string(),
        };
        let json = serde_json::to_string(&ws).expect("serialize");
        let back: WorkspaceMeta = serde_json::from_str(&json).expect("deserialize");
        assert!(back.name.is_none());
        assert_eq!(back.plan, "pro");
    }
}
