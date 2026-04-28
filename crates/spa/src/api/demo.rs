//! Demo seed-data impl of the Api trait. Used when the SPA loads
//! with `?demo=1`. Delegates to the existing crate::demo::* helpers;
//! mutations not modelled by the seed return Ok(()); auth methods
//! return Err since auth is bypassed entirely in demo mode.

use crate::api::*;

pub struct DemoApi;

#[async_trait::async_trait(?Send)]
impl Api for DemoApi {
    // Auth — never reachable in demo because the gate skips the login
    // flow. Returning Err here surfaces a callsite bug rather than
    // masking it.
    async fn send_otp(&self, _phone: &str) -> Result<OtpResponse, String> {
        Err("auth not available in demo mode".into())
    }
    async fn verify_otp(&self, _phone: &str, _code: &str) -> Result<VerifyResponse, String> {
        Err("auth not available in demo mode".into())
    }

    async fn get_workspaces(&self) -> Result<Vec<WorkspaceInfo>, String> {
        Ok(crate::demo::workspaces())
    }
    async fn get_workspace_info(&self, slug: &str) -> Result<WorkspaceOverview, String> {
        Ok(WorkspaceOverview {
            slug: slug.into(),
            name: Some("seed.workspace.name".into()),
            plan: "free".into(),
            stats: crate::demo::status_counts(),
        })
    }

    async fn get_todos(&self, _slug: &str, filter: &str) -> Result<Vec<TodoItem>, String> {
        let mut items = crate::demo::todos();
        if filter != "all" { items.retain(|t| t.status == filter); }
        Ok(items)
    }
    async fn create_todo(&self, _slug: &str, title: &str, priority: i32) -> Result<TodoItem, String> {
        Ok(crate::demo::new_todo(title, priority))
    }
    async fn update_todo(&self, _slug: &str, _id: &str, _updates: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
    async fn delete_todo(&self, _slug: &str, _id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn get_notes(&self, _slug: &str) -> Result<Vec<NoteItem>, String> {
        Ok(crate::demo::notes())
    }
    async fn get_note(&self, _slug: &str, id: &str) -> Result<NoteItem, String> {
        crate::demo::notes().into_iter()
            .find(|n| n.id == id)
            .ok_or_else(|| "demo: note not found".into())
    }
    async fn create_note(&self, _slug: &str, title: &str, content: &str) -> Result<NoteItem, String> {
        Ok(crate::demo::new_note(title, content))
    }
    async fn update_note(&self, _slug: &str, _id: &str, _title: &str, _content: &str) -> Result<(), String> {
        Ok(())
    }
    async fn delete_note(&self, _slug: &str, _id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn get_history(&self, _slug: &str) -> Result<Vec<ActivityItem>, String> {
        Ok(crate::demo::activity())
    }
    async fn get_members(&self, _slug: &str) -> Result<Vec<MemberItem>, String> {
        Ok(crate::demo::members())
    }

    async fn list_memory(&self, _slug: &str) -> Result<Vec<MemoryItem>, String> {
        Ok(crate::demo::memories())
    }
    async fn create_memory(&self, _slug: &str, body: &serde_json::Value) -> Result<MemoryItem, String> {
        Ok(crate::demo::new_memory(body))
    }
    async fn update_memory(&self, _slug: &str, _id: &str, _body: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
    async fn delete_memory(&self, _slug: &str, _id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn list_events(&self, _slug: &str) -> Result<Vec<EventItem>, String> {
        Ok(crate::demo::events())
    }
    async fn create_event(&self, _slug: &str, body: &serde_json::Value) -> Result<EventItem, String> {
        Ok(crate::demo::new_event(body))
    }
    async fn update_event(&self, _slug: &str, _id: &str, _body: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
    async fn delete_event(&self, _slug: &str, _id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn list_scheduled_actions(&self, _slug: &str) -> Result<Vec<ScheduledActionItem>, String> {
        Ok(crate::demo::scheduled_actions())
    }
    async fn create_scheduled_action(&self, _slug: &str, body: &serde_json::Value) -> Result<ScheduledActionItem, String> {
        Ok(crate::demo::new_scheduled(body))
    }
    async fn update_scheduled_action(&self, _slug: &str, _id: &str, _body: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
    async fn delete_scheduled_action(&self, _slug: &str, _id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn list_calendar(&self, _slug: &str, _from: &str, _to: &str) -> Result<Vec<CalendarItem>, String> {
        Ok(crate::demo::calendar_items())
    }
    async fn get_settings(&self, _slug: &str) -> Result<WorkspaceSettings, String> {
        Ok(crate::demo::settings())
    }
    async fn update_settings(&self, _slug: &str, _body: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
    async fn update_workspace_locale(&self, _slug: &str, _locale: &str) -> Result<(), String> {
        Ok(())
    }
    async fn regenerate_ical_token(&self, _slug: &str) -> Result<ICalTokenResponse, String> {
        Ok(ICalTokenResponse {
            token: "demo-ical-token".into(),
            url: "/demo/calendar.ics".into(),
        })
    }

    async fn get_observability(&self, _slug: &str) -> Result<ObservabilityData, String> {
        Ok(ObservabilityData::default())
    }
    async fn get_admin_me(&self) -> Result<AdminMe, String> {
        Err("auth not available in demo mode".into())
    }
    async fn get_global_observability(&self) -> Result<GlobalObservabilityData, String> {
        Err("auth not available in demo mode".into())
    }
}
