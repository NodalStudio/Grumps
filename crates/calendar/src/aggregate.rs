//! Calendar aggregation : union of todos+events+reminders+scheduled actions.
//! See spec § 9.1.

use crate::Event;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CalendarSource {
    Todo,
    Event,
    Reminder,
    ScheduledAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarItem {
    pub id: String,
    pub source: CalendarSource,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub all_day: bool,
    pub location: Option<String>,
    pub color: String,
    pub member_id: Option<String>,
    pub recurrence: Option<String>,
    pub editable: bool,
    pub url: String,
}

/// Aggregate the 4 sources into a single sorted list of CalendarItems.
/// Recurrent items are NOT expanded here (we emit one item with the RRULE field — the SPA / iCal client expands).
pub fn aggregate(
    events: Vec<Event>,
    todos_json: Vec<serde_json::Value>,
    reminders_json: Vec<serde_json::Value>,
    scheduled_json: Vec<serde_json::Value>,
    ws_slug: &str,
) -> Vec<CalendarItem> {
    let mut items: Vec<CalendarItem> = vec![];

    for e in events {
        items.push(CalendarItem {
            id: format!("evt:{}", e.id),
            source: CalendarSource::Event,
            title: e.title,
            starts_at: e.starts_at,
            ends_at: e.ends_at,
            all_day: e.all_day,
            location: e.location,
            color: e.color,
            member_id: e.attendees.first().cloned(),
            recurrence: e.recurrence,
            editable: true,
            url: format!("/w/{}/events/{}", ws_slug, e.id),
        });
    }

    for t in todos_json {
        if let (Some(id), Some(title), Some(deadline)) = (
            t.get("id").and_then(|v| v.as_str()),
            t.get("title").and_then(|v| v.as_str()),
            t.get("deadline").and_then(|v| v.as_str()),
        ) {
            if let Ok(dt) = DateTime::parse_from_rfc3339(deadline) {
                items.push(CalendarItem {
                    id: format!("todo:{id}"),
                    source: CalendarSource::Todo,
                    title: title.to_string(),
                    starts_at: dt.with_timezone(&Utc),
                    ends_at: None,
                    all_day: true,
                    location: None,
                    color: "brick".into(),
                    member_id: t
                        .get("assigned_to")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    recurrence: None,
                    editable: true,
                    url: format!("/w/{}/todos", ws_slug),
                });
            }
        }
    }

    for r in reminders_json {
        if let (Some(id), Some(title), Some(remind_at)) = (
            r.get("id").and_then(|v| v.as_str()),
            r.get("title").and_then(|v| v.as_str()),
            r.get("remind_at").and_then(|v| v.as_str()),
        ) {
            if let Ok(dt) = DateTime::parse_from_rfc3339(remind_at) {
                items.push(CalendarItem {
                    id: format!("rmd:{id}"),
                    source: CalendarSource::Reminder,
                    title: title.to_string(),
                    starts_at: dt.with_timezone(&Utc),
                    ends_at: None,
                    all_day: false,
                    location: None,
                    color: "cream".into(),
                    member_id: r
                        .get("target_member")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    recurrence: r
                        .get("recurrence")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    editable: true,
                    url: format!("/w/{}/scheduled", ws_slug),
                });
            }
        }
    }

    for s in scheduled_json {
        if let (Some(id), Some(title), Some(trigger_at)) = (
            s.get("id").and_then(|v| v.as_str()),
            s.get("title").and_then(|v| v.as_str()),
            s.get("trigger_at").and_then(|v| v.as_str()),
        ) {
            if let Ok(dt) = DateTime::parse_from_rfc3339(trigger_at) {
                items.push(CalendarItem {
                    id: format!("sch:{id}"),
                    source: CalendarSource::ScheduledAction,
                    title: title.to_string(),
                    starts_at: dt.with_timezone(&Utc),
                    ends_at: None,
                    all_day: false,
                    location: None,
                    color: "slate-300".into(),
                    member_id: s
                        .get("created_by")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    recurrence: s
                        .get("recurrence")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    editable: false,
                    url: format!("/w/{}/scheduled/{}", ws_slug, id),
                });
            }
        }
    }

    items.sort_by_key(|i| i.starts_at);
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn empty_aggregation() {
        let items = aggregate(vec![], vec![], vec![], vec![], "test");
        assert!(items.is_empty());
    }

    #[test]
    fn aggregates_all_4_sources_sorted() {
        let evt = Event {
            id: "e1".into(),
            title: "Meeting".into(),
            description: None,
            starts_at: Utc.with_ymd_and_hms(2026, 4, 22, 10, 0, 0).unwrap(),
            ends_at: None,
            all_day: false,
            location: None,
            recurrence: None,
            attendees: vec![],
            color: "teal".into(),
            source: crate::EventSource::Web,
            related_todo_id: None,
            created_by: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let todo = serde_json::json!({
            "id": "t1",
            "title": "Buy gifts",
            "deadline": "2026-04-20T00:00:00Z",
            "status": "open",
        });
        let items = aggregate(vec![evt], vec![todo], vec![], vec![], "ws1");
        assert_eq!(items.len(), 2);
        // Sorted : todo (Apr 20) before event (Apr 22)
        assert_eq!(items[0].id, "todo:t1");
        assert_eq!(items[1].id, "evt:e1");
    }
}
