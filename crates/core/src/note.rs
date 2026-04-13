use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: Option<String>,
    pub content: String,
    pub pinned: bool,
    pub source: String,
    pub created_by: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateNote {
    pub title: Option<String>,
    pub content: String,
    pub pinned: bool,
    pub source: String,
    pub created_by: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_create_note(content: &str) -> CreateNote {
        CreateNote {
            title: None,
            content: content.to_string(),
            pinned: false,
            source: "slack".to_string(),
            created_by: "user1".to_string(),
        }
    }

    #[test]
    fn create_note_empty_content_is_allowed() {
        // CreateNote has no validate method; empty content is structurally valid
        let n = make_create_note("");
        assert!(n.content.is_empty());
    }

    #[test]
    fn create_note_valid_content() {
        let n = make_create_note("Some important note");
        assert_eq!(n.content, "Some important note");
    }

    #[test]
    fn note_serialization_roundtrip() {
        let note = Note {
            id: "n-1".to_string(),
            title: Some("Meeting notes".to_string()),
            content: "Discussed quarterly goals".to_string(),
            pinned: true,
            source: "slack".to_string(),
            created_by: Some("user-1".to_string()),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&note).expect("serialize");
        let back: Note = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, note.id);
        assert_eq!(back.pinned, true);
        assert_eq!(back.title, Some("Meeting notes".to_string()));
    }
}
