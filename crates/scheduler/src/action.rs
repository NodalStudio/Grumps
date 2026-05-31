use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Reminder,
    FollowUp,
    Recap,
    AgentTask,
    EventNotify,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Pending,
    Firing,
    Done,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledAction {
    pub id: String,
    pub action_type: ActionType,
    pub title: String,
    pub trigger_at: DateTime<Utc>,
    pub recurrence: Option<String>, // RRULE
    pub condition: Option<serde_json::Value>,
    pub payload: serde_json::Value,
    pub target_chat: String, // "group" only at launch
    pub status: ActionStatus,
    pub last_fired_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub fire_count: i64,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewScheduledAction {
    pub action_type: ActionType,
    pub title: String,
    pub trigger_at: DateTime<Utc>,
    pub recurrence: Option<String>,
    pub condition: Option<serde_json::Value>,
    pub payload: serde_json::Value,
    pub target_chat: Option<String>,
    pub created_by: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_type_snake() {
        assert_eq!(
            serde_json::to_string(&ActionType::AgentTask).unwrap(),
            r#""agent_task""#
        );
        assert_eq!(
            serde_json::to_string(&ActionType::EventNotify).unwrap(),
            r#""event_notify""#
        );
    }

    #[test]
    fn status_snake() {
        assert_eq!(
            serde_json::to_string(&ActionStatus::Pending).unwrap(),
            r#""pending""#
        );
        assert_eq!(
            serde_json::to_string(&ActionStatus::Firing).unwrap(),
            r#""firing""#
        );
    }
}
