use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Open,
    InProgress,
    Done,
    Blocked,
    Deleted,
}

impl TodoStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Blocked => "blocked",
            Self::Deleted => "deleted",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "open" => Some(Self::Open),
            "in_progress" => Some(Self::InProgress),
            "done" => Some(Self::Done),
            "blocked" => Some(Self::Blocked),
            "deleted" => Some(Self::Deleted),
            _ => None,
        }
    }

    pub fn is_open(&self) -> bool {
        matches!(self, Self::Open | Self::InProgress)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum Priority {
    High = 1,
    Normal = 2,
    Low = 3,
}

impl Priority {
    pub fn from_int(n: i32) -> Self {
        match n {
            1 => Self::High,
            3 => Self::Low,
            _ => Self::Normal,
        }
    }

    pub fn as_int(&self) -> i32 {
        *self as i32
    }

    pub fn emoji(&self) -> &str {
        match self {
            Self::High => "🔴",
            Self::Normal => "",
            Self::Low => "🔵",
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::High => "High",
            Self::Normal => "Normal",
            Self::Low => "Low",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub seq_num: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: TodoStatus,
    pub priority: Priority,
    pub tags: Vec<String>,
    pub deadline: Option<String>,
    pub assigned_to: Option<String>,
    pub assigned_name: Option<String>,
    pub created_by: Option<String>,
    pub completed_at: Option<String>,
    pub completed_by: Option<String>,
    pub source: String,
    pub message_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateTodo {
    pub title: String,
    pub priority: Priority,
    pub tags: Vec<String>,
    pub deadline_text: Option<String>,
    pub assigned_to: Option<String>,
    pub created_by: String,
    pub source: String,
    pub message_id: Option<String>,
}

impl CreateTodo {
    pub fn validate(&self) -> Result<(), &'static str> {
        let t = self.title.trim();
        if t.is_empty() {
            return Err("title cannot be empty");
        }
        if t.len() > 500 {
            return Err("title too long (max 500)");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_status_roundtrip() {
        let variants = [
            TodoStatus::Open,
            TodoStatus::InProgress,
            TodoStatus::Done,
            TodoStatus::Blocked,
            TodoStatus::Deleted,
        ];
        for v in &variants {
            let s = v.as_str();
            let parsed = TodoStatus::from_str(s).expect("roundtrip should succeed");
            assert_eq!(&parsed, v);
        }
    }

    #[test]
    fn todo_status_from_str_unknown() {
        assert!(TodoStatus::from_str("unknown").is_none());
        assert!(TodoStatus::from_str("").is_none());
    }

    #[test]
    fn todo_status_is_open() {
        assert!(TodoStatus::Open.is_open());
        assert!(TodoStatus::InProgress.is_open());
        assert!(!TodoStatus::Done.is_open());
        assert!(!TodoStatus::Blocked.is_open());
        assert!(!TodoStatus::Deleted.is_open());
    }

    #[test]
    fn priority_ordering() {
        assert!(Priority::High < Priority::Normal);
        assert!(Priority::Normal < Priority::Low);
        assert!(Priority::High < Priority::Low);
    }

    #[test]
    fn priority_from_int_roundtrip() {
        assert_eq!(Priority::from_int(1), Priority::High);
        assert_eq!(Priority::from_int(2), Priority::Normal);
        assert_eq!(Priority::from_int(3), Priority::Low);
        assert_eq!(Priority::from_int(0), Priority::Normal);
        assert_eq!(Priority::from_int(99), Priority::Normal);

        assert_eq!(Priority::High.as_int(), 1);
        assert_eq!(Priority::Normal.as_int(), 2);
        assert_eq!(Priority::Low.as_int(), 3);
    }

    #[test]
    fn priority_emoji_and_label() {
        assert_eq!(Priority::High.emoji(), "🔴");
        assert_eq!(Priority::Normal.emoji(), "");
        assert_eq!(Priority::Low.emoji(), "🔵");

        assert_eq!(Priority::High.label(), "High");
        assert_eq!(Priority::Normal.label(), "Normal");
        assert_eq!(Priority::Low.label(), "Low");
    }

    fn make_create_todo(title: &str) -> CreateTodo {
        CreateTodo {
            title: title.to_string(),
            priority: Priority::Normal,
            tags: vec![],
            deadline_text: None,
            assigned_to: None,
            created_by: "user1".to_string(),
            source: "slack".to_string(),
            message_id: None,
        }
    }

    #[test]
    fn create_todo_validate_empty() {
        let ct = make_create_todo("");
        assert_eq!(ct.validate(), Err("title cannot be empty"));
    }

    #[test]
    fn create_todo_validate_whitespace_only() {
        let ct = make_create_todo("   ");
        assert_eq!(ct.validate(), Err("title cannot be empty"));
    }

    #[test]
    fn create_todo_validate_too_long() {
        let ct = make_create_todo(&"a".repeat(501));
        assert_eq!(ct.validate(), Err("title too long (max 500)"));
    }

    #[test]
    fn create_todo_validate_valid() {
        let ct = make_create_todo("Do the thing");
        assert!(ct.validate().is_ok());
    }

    #[test]
    fn create_todo_validate_exactly_500() {
        let ct = make_create_todo(&"a".repeat(500));
        assert!(ct.validate().is_ok());
    }

    #[test]
    fn todo_serialization_roundtrip() {
        let todo = Todo {
            id: "id-1".to_string(),
            seq_num: 42,
            title: "Test todo".to_string(),
            description: Some("desc".to_string()),
            status: TodoStatus::InProgress,
            priority: Priority::High,
            tags: vec!["tag1".to_string()],
            deadline: None,
            assigned_to: Some("user-1".to_string()),
            assigned_name: Some("Alice".to_string()),
            created_by: Some("user-2".to_string()),
            completed_at: None,
            completed_by: None,
            source: "slack".to_string(),
            message_id: Some("msg-1".to_string()),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&todo).expect("serialize");
        let back: Todo = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(back.id, todo.id);
        assert_eq!(back.seq_num, todo.seq_num);
        assert_eq!(back.status, TodoStatus::InProgress);
        assert_eq!(back.priority, Priority::High);
        assert_eq!(back.tags, vec!["tag1"]);
    }

    #[test]
    fn todo_status_serde_snake_case() {
        let json = serde_json::to_string(&TodoStatus::InProgress).expect("serialize");
        assert_eq!(json, "\"in_progress\"");
        let back: TodoStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, TodoStatus::InProgress);
    }
}
