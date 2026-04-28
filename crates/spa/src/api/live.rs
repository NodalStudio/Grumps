//! Live HTTP impl of the Api trait. Cookies + CSRF carry auth.

use crate::api::*;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

pub struct LiveApi;

impl LiveApi {
    fn build_get(url: &str) -> gloo_net::http::RequestBuilder {
        Request::get(url).credentials(web_sys::RequestCredentials::Include)
    }

    fn build_with_csrf(rb: gloo_net::http::RequestBuilder) -> gloo_net::http::RequestBuilder {
        rb.credentials(web_sys::RequestCredentials::Include)
            .header("X-CSRF-Token", &crate::auth::read_csrf_cookie())
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", api_base(), path);
        let resp = Self::build_get(&url).send().await.map_err(|e| e.to_string())?;
        if !resp.ok() {
            return Err(format!("HTTP {}: {}", resp.status(), resp.status_text()));
        }
        resp.json().await.map_err(|e| e.to_string())
    }

    async fn post<B: Serialize, T: for<'de> Deserialize<'de>>(&self, path: &str, body: &B) -> Result<T, String> {
        let url = format!("{}{}", api_base(), path);
        let req = Self::build_with_csrf(Request::post(&url)).header("Content-Type", "application/json");
        let resp = req.json(body).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() { return Err(format!("HTTP {}", resp.status())); }
        resp.json().await.map_err(|e| e.to_string())
    }

    async fn put<B: Serialize>(&self, path: &str, body: &B) -> Result<(), String> {
        let url = format!("{}{}", api_base(), path);
        let req = Self::build_with_csrf(Request::put(&url)).header("Content-Type", "application/json");
        let resp = req.json(body).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() { return Err(format!("HTTP {}", resp.status())); }
        Ok(())
    }

    async fn patch<B: Serialize>(&self, path: &str, body: &B) -> Result<(), String> {
        let url = format!("{}{}", api_base(), path);
        let req = Self::build_with_csrf(Request::patch(&url)).header("Content-Type", "application/json");
        let resp = req.json(body).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() { return Err(format!("HTTP {}", resp.status())); }
        Ok(())
    }

    async fn delete(&self, path: &str) -> Result<(), String> {
        let url = format!("{}{}", api_base(), path);
        let req = Self::build_with_csrf(Request::delete(&url));
        let resp = req.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() { return Err(format!("HTTP {}", resp.status())); }
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Api for LiveApi {
    async fn send_otp(&self, phone: &str) -> Result<OtpResponse, String> {
        self.post("/auth/otp", &serde_json::json!({"phone": phone})).await
    }
    async fn verify_otp(&self, phone: &str, code: &str) -> Result<VerifyResponse, String> {
        self.post("/auth/verify", &serde_json::json!({"phone": phone, "code": code})).await
    }

    async fn get_workspaces(&self) -> Result<Vec<WorkspaceInfo>, String> {
        self.get("/api/workspaces").await
    }
    async fn get_workspace_info(&self, slug: &str) -> Result<WorkspaceOverview, String> {
        self.get(&format!("/api/w/{}", slug)).await
    }

    async fn get_todos(&self, slug: &str, filter: &str) -> Result<Vec<TodoItem>, String> {
        self.get(&format!("/api/w/{}/todos?status={}", slug, filter)).await
    }
    async fn create_todo(&self, slug: &str, title: &str, priority: i32) -> Result<TodoItem, String> {
        self.post(&format!("/api/w/{}/todos", slug), &serde_json::json!({"title": title, "priority": priority})).await
    }
    async fn update_todo(&self, slug: &str, id: &str, updates: &serde_json::Value) -> Result<(), String> {
        self.patch(&format!("/api/w/{}/todos/{}", slug, id), updates).await
    }
    async fn delete_todo(&self, slug: &str, id: &str) -> Result<(), String> {
        self.delete(&format!("/api/w/{}/todos/{}", slug, id)).await
    }

    async fn get_notes(&self, slug: &str) -> Result<Vec<NoteItem>, String> {
        self.get(&format!("/api/w/{}/notes", slug)).await
    }
    async fn get_note(&self, slug: &str, id: &str) -> Result<NoteItem, String> {
        self.get(&format!("/api/w/{}/notes/{}", slug, id)).await
    }
    async fn create_note(&self, slug: &str, title: &str, content: &str) -> Result<NoteItem, String> {
        self.post(&format!("/api/w/{}/notes", slug), &serde_json::json!({"title": title, "content": content})).await
    }
    async fn update_note(&self, slug: &str, id: &str, title: &str, content: &str) -> Result<(), String> {
        self.put(&format!("/api/w/{}/notes/{}", slug, id), &serde_json::json!({"title": title, "content": content})).await
    }
    async fn delete_note(&self, slug: &str, id: &str) -> Result<(), String> {
        self.delete(&format!("/api/w/{}/notes/{}", slug, id)).await
    }

    async fn get_history(&self, slug: &str) -> Result<Vec<ActivityItem>, String> {
        self.get(&format!("/api/w/{}/history", slug)).await
    }
    async fn get_members(&self, slug: &str) -> Result<Vec<MemberItem>, String> {
        self.get(&format!("/api/w/{}/members", slug)).await
    }

    async fn list_memory(&self, slug: &str) -> Result<Vec<MemoryItem>, String> {
        self.get(&format!("/api/w/{}/memory", slug)).await
    }
    async fn create_memory(&self, slug: &str, body: &serde_json::Value) -> Result<MemoryItem, String> {
        self.post(&format!("/api/w/{}/memory", slug), body).await
    }
    async fn update_memory(&self, slug: &str, id: &str, body: &serde_json::Value) -> Result<(), String> {
        self.patch(&format!("/api/w/{}/memory/{}", slug, id), body).await
    }
    async fn delete_memory(&self, slug: &str, id: &str) -> Result<(), String> {
        self.delete(&format!("/api/w/{}/memory/{}", slug, id)).await
    }

    async fn list_events(&self, slug: &str) -> Result<Vec<EventItem>, String> {
        self.get(&format!("/api/w/{}/events", slug)).await
    }
    async fn create_event(&self, slug: &str, body: &serde_json::Value) -> Result<EventItem, String> {
        self.post(&format!("/api/w/{}/events", slug), body).await
    }
    async fn update_event(&self, slug: &str, id: &str, body: &serde_json::Value) -> Result<(), String> {
        self.patch(&format!("/api/w/{}/events/{}", slug, id), body).await
    }
    async fn delete_event(&self, slug: &str, id: &str) -> Result<(), String> {
        self.delete(&format!("/api/w/{}/events/{}", slug, id)).await
    }

    async fn list_scheduled_actions(&self, slug: &str) -> Result<Vec<ScheduledActionItem>, String> {
        self.get(&format!("/api/w/{}/scheduled-actions", slug)).await
    }
    async fn create_scheduled_action(&self, slug: &str, body: &serde_json::Value) -> Result<ScheduledActionItem, String> {
        self.post(&format!("/api/w/{}/scheduled-actions", slug), body).await
    }
    async fn update_scheduled_action(&self, slug: &str, id: &str, body: &serde_json::Value) -> Result<(), String> {
        self.patch(&format!("/api/w/{}/scheduled-actions/{}", slug, id), body).await
    }
    async fn delete_scheduled_action(&self, slug: &str, id: &str) -> Result<(), String> {
        self.delete(&format!("/api/w/{}/scheduled-actions/{}", slug, id)).await
    }

    async fn list_calendar(&self, slug: &str, from: &str, to: &str) -> Result<Vec<CalendarItem>, String> {
        self.get(&format!("/api/w/{}/calendar?from={}&to={}", slug, from, to)).await
    }
    async fn get_settings(&self, slug: &str) -> Result<WorkspaceSettings, String> {
        self.get(&format!("/api/w/{}/settings", slug)).await
    }
    async fn update_settings(&self, slug: &str, body: &serde_json::Value) -> Result<(), String> {
        self.put(&format!("/api/w/{}/settings", slug), body).await
    }
    async fn update_workspace_locale(&self, slug: &str, locale: &str) -> Result<(), String> {
        self.patch(&format!("/api/w/{}/settings/locale", slug), &serde_json::json!({"locale": locale})).await
    }
    async fn regenerate_ical_token(&self, slug: &str) -> Result<ICalTokenResponse, String> {
        self.post(&format!("/api/w/{}/calendar/ical-token", slug), &serde_json::json!({})).await
    }

    async fn get_observability(&self, slug: &str) -> Result<ObservabilityData, String> {
        self.get(&format!("/api/w/{}/admin/observability", slug)).await
    }
    async fn get_admin_me(&self) -> Result<AdminMe, String> {
        self.get("/api/admin/me").await
    }
    async fn get_global_observability(&self) -> Result<GlobalObservabilityData, String> {
        self.get("/api/admin/observability").await
    }
}
