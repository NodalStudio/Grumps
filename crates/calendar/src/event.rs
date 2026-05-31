use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    Chat,
    Web,
    Agent,
}

impl Default for EventSource {
    fn default() -> Self {
        EventSource::Web
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub all_day: bool,
    pub location: Option<String>,
    pub recurrence: Option<String>, // RRULE
    pub attendees: Vec<String>,     // member.id
    pub color: String,
    pub source: EventSource,
    pub related_todo_id: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NewEvent {
    pub title: String,
    pub description: Option<String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub all_day: bool,
    pub location: Option<String>,
    pub recurrence: Option<String>,
    pub attendees: Vec<String>,
    pub color: Option<String>,
    pub source: EventSource,
    pub related_todo_id: Option<String>,
    pub created_by: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_source_serializes_snake() {
        assert_eq!(
            serde_json::to_string(&EventSource::Web).unwrap(),
            r#""web""#
        );
        assert_eq!(
            serde_json::to_string(&EventSource::Chat).unwrap(),
            r#""chat""#
        );
    }

    #[test]
    fn event_default_color_teal() {
        let e = NewEvent {
            title: "test".into(),
            starts_at: Utc::now(),
            color: Some("teal".into()),
            ..Default::default()
        };
        assert_eq!(e.color.as_deref(), Some("teal"));
    }
}
