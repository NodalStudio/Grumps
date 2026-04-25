use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use leptos::prelude::*;

pub fn api_base() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .map(|origin| {
            if origin.contains("localhost") || origin.contains("127.0.0.1") {
                "http://localhost:8787".to_string()
            } else {
                // Worker is served at api.grumps.app; SPA lives at grumps.app
                origin.replace("grumps.app", "api.grumps.app")
            }
        })
        .unwrap_or_else(|| "http://localhost:8787".to_string())
}

// =====================
// Response types
// =====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub seq_num: i64,
    pub title: String,
    pub status: String,
    pub assigned_name: Option<String>,
    pub priority: i32,
    pub tags: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteItem {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub pinned: Option<i32>,
    pub source: String,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberItem {
    pub id: String,
    pub platform_user_id: String,
    pub display_name: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityItem {
    pub id: String,
    pub actor: Option<String>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub slug: String,
    pub name: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusCounts {
    pub open_todos: i64,
    pub done_this_week: i64,
    pub notes: i64,
    pub files: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub key: Option<String>,
    pub value: String,
    pub kind: String,
    pub related_member: Option<String>,
    pub source: String,
    pub confidence: f64,
    pub pinned: bool,
    pub expires_at: Option<String>,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventItem {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub all_day: bool,
    pub location: Option<String>,
    pub recurrence: Option<String>,
    pub attendees: Vec<String>,
    pub color: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledActionItem {
    pub id: String,
    pub action_type: String,
    pub title: String,
    pub trigger_at: String,
    pub recurrence: Option<String>,
    pub status: String,
    pub fire_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarItem {
    pub id: String,
    pub source: String,
    pub title: String,
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub all_day: bool,
    pub location: Option<String>,
    pub color: String,
    pub member_id: Option<String>,
    pub recurrence: Option<String>,
    pub editable: bool,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSettings {
    pub language: Option<String>,
    pub timezone: Option<String>,
    pub quiet_mode: Option<bool>,
    pub auto_recap: Option<bool>,
    pub persona: Option<String>,
    pub proactive_mode: Option<bool>,
    pub auto_memory: Option<bool>,
    pub ical_token: Option<String>,
    pub agent_calls_used: Option<i64>,
    pub agent_calls_limit: Option<i64>,
    pub web_search_used: Option<i64>,
    pub web_search_limit: Option<i64>,
    pub storage_used_mb: Option<f64>,
    pub storage_limit_mb: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ICalTokenResponse {
    pub token: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtpResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResponse {
    pub token: String,
    pub user_id: String,
    pub workspaces: Vec<WorkspaceInfo>,
}

// =====================
// API Client
// =====================

#[derive(Clone)]
pub struct ApiClient {
    token: ReadSignal<Option<String>>,
}

impl ApiClient {
    pub fn new(_token: ReadSignal<Option<String>>) -> Self {
        // The token signal is no longer used (cookies carry auth). Kept in the
        // signature for backward-compat with existing callers — remove in a
        // follow-up task once all sites stop passing a token.
        Self { token: _token }
    }

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

    // =====================
    // Auth
    // =====================

    pub async fn send_otp(&self, phone: &str) -> Result<OtpResponse, String> {
        self.post("/auth/otp", &serde_json::json!({"phone": phone})).await
    }

    pub async fn verify_otp(&self, phone: &str, code: &str) -> Result<VerifyResponse, String> {
        self.post("/auth/verify", &serde_json::json!({"phone": phone, "code": code})).await
    }

    // =====================
    // Workspaces
    // =====================

    pub async fn get_workspaces(&self) -> Result<Vec<WorkspaceInfo>, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::workspaces()); }
        self.get("/api/workspaces").await
    }

    pub async fn get_workspace_info(&self, slug: &str) -> Result<StatusCounts, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::status_counts()); }
        self.get(&format!("/api/w/{}", slug)).await
    }

    // =====================
    // Todos
    // =====================

    pub async fn get_todos(&self, slug: &str, filter: &str) -> Result<Vec<TodoItem>, String> {
        if crate::demo::is_demo() {
            let mut items = crate::demo::todos();
            if filter != "all" { items.retain(|t| t.status == filter); }
            return Ok(items);
        }
        self.get(&format!("/api/w/{}/todos?status={}", slug, filter)).await
    }

    pub async fn create_todo(&self, slug: &str, title: &str, priority: i32) -> Result<TodoItem, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::new_todo(title, priority)); }
        self.post(&format!("/api/w/{}/todos", slug), &serde_json::json!({"title": title, "priority": priority})).await
    }

    pub async fn update_todo(&self, slug: &str, id: &str, updates: &serde_json::Value) -> Result<(), String> {
        if crate::demo::is_demo() { return Ok(()); }
        self.patch(&format!("/api/w/{}/todos/{}", slug, id), updates).await
    }

    pub async fn delete_todo(&self, slug: &str, id: &str) -> Result<(), String> {
        if crate::demo::is_demo() { return Ok(()); }
        self.delete(&format!("/api/w/{}/todos/{}", slug, id)).await
    }

    // =====================
    // Notes
    // =====================

    pub async fn get_notes(&self, slug: &str) -> Result<Vec<NoteItem>, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::notes()); }
        self.get(&format!("/api/w/{}/notes", slug)).await
    }

    pub async fn get_note(&self, slug: &str, id: &str) -> Result<NoteItem, String> {
        if crate::demo::is_demo() {
            return crate::demo::notes().into_iter()
                .find(|n| n.id == id)
                .ok_or_else(|| "demo: note not found".into());
        }
        self.get(&format!("/api/w/{}/notes/{}", slug, id)).await
    }

    pub async fn create_note(&self, slug: &str, title: &str, content: &str) -> Result<NoteItem, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::new_note(title, content)); }
        self.post(&format!("/api/w/{}/notes", slug), &serde_json::json!({"title": title, "content": content})).await
    }

    pub async fn update_note(&self, slug: &str, id: &str, title: &str, content: &str) -> Result<(), String> {
        if crate::demo::is_demo() { return Ok(()); }
        self.put(&format!("/api/w/{}/notes/{}", slug, id), &serde_json::json!({"title": title, "content": content})).await
    }

    pub async fn delete_note(&self, slug: &str, id: &str) -> Result<(), String> {
        if crate::demo::is_demo() { return Ok(()); }
        self.delete(&format!("/api/w/{}/notes/{}", slug, id)).await
    }

    // =====================
    // History + Members
    // =====================

    pub async fn get_history(&self, slug: &str) -> Result<Vec<ActivityItem>, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::activity()); }
        self.get(&format!("/api/w/{}/history", slug)).await
    }

    pub async fn get_members(&self, slug: &str) -> Result<Vec<MemberItem>, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::members()); }
        self.get(&format!("/api/w/{}/members", slug)).await
    }

    // =====================
    // Memory
    // =====================

    pub async fn list_memory(&self, slug: &str) -> Result<Vec<MemoryItem>, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::memories()); }
        self.get(&format!("/api/w/{}/memory", slug)).await
    }

    pub async fn create_memory(&self, slug: &str, body: &serde_json::Value) -> Result<MemoryItem, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::new_memory(body)); }
        self.post(&format!("/api/w/{}/memory", slug), body).await
    }

    pub async fn update_memory(&self, slug: &str, id: &str, body: &serde_json::Value) -> Result<(), String> {
        if crate::demo::is_demo() { return Ok(()); }
        self.patch(&format!("/api/w/{}/memory/{}", slug, id), body).await
    }

    pub async fn delete_memory(&self, slug: &str, id: &str) -> Result<(), String> {
        if crate::demo::is_demo() { return Ok(()); }
        self.delete(&format!("/api/w/{}/memory/{}", slug, id)).await
    }

    // =====================
    // Events
    // =====================

    pub async fn list_events(&self, slug: &str) -> Result<Vec<EventItem>, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::events()); }
        self.get(&format!("/api/w/{}/events", slug)).await
    }

    pub async fn create_event(&self, slug: &str, body: &serde_json::Value) -> Result<EventItem, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::new_event(body)); }
        self.post(&format!("/api/w/{}/events", slug), body).await
    }

    pub async fn update_event(&self, slug: &str, id: &str, body: &serde_json::Value) -> Result<(), String> {
        if crate::demo::is_demo() { return Ok(()); }
        self.patch(&format!("/api/w/{}/events/{}", slug, id), body).await
    }

    pub async fn delete_event(&self, slug: &str, id: &str) -> Result<(), String> {
        if crate::demo::is_demo() { return Ok(()); }
        self.delete(&format!("/api/w/{}/events/{}", slug, id)).await
    }

    // =====================
    // Scheduled Actions
    // =====================

    pub async fn list_scheduled_actions(&self, slug: &str) -> Result<Vec<ScheduledActionItem>, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::scheduled_actions()); }
        self.get(&format!("/api/w/{}/scheduled-actions", slug)).await
    }

    pub async fn create_scheduled_action(&self, slug: &str, body: &serde_json::Value) -> Result<ScheduledActionItem, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::new_scheduled(body)); }
        self.post(&format!("/api/w/{}/scheduled-actions", slug), body).await
    }

    pub async fn update_scheduled_action(&self, slug: &str, id: &str, body: &serde_json::Value) -> Result<(), String> {
        if crate::demo::is_demo() { return Ok(()); }
        self.patch(&format!("/api/w/{}/scheduled-actions/{}", slug, id), body).await
    }

    pub async fn delete_scheduled_action(&self, slug: &str, id: &str) -> Result<(), String> {
        if crate::demo::is_demo() { return Ok(()); }
        self.delete(&format!("/api/w/{}/scheduled-actions/{}", slug, id)).await
    }

    // =====================
    // Calendar (aggregated)
    // =====================

    pub async fn list_calendar(&self, slug: &str, from: &str, to: &str) -> Result<Vec<CalendarItem>, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::calendar_items()); }
        self.get(&format!("/api/w/{}/calendar?from={}&to={}", slug, from, to)).await
    }

    // =====================
    // Workspace Settings
    // =====================

    pub async fn get_settings(&self, slug: &str) -> Result<WorkspaceSettings, String> {
        if crate::demo::is_demo() { return Ok(crate::demo::settings()); }
        self.get(&format!("/api/w/{}/settings", slug)).await
    }

    pub async fn update_settings(&self, slug: &str, body: &serde_json::Value) -> Result<(), String> {
        if crate::demo::is_demo() { return Ok(()); }
        self.put(&format!("/api/w/{}/settings", slug), body).await
    }

    pub async fn update_workspace_locale(&self, slug: &str, locale: &str) -> Result<(), String> {
        if crate::demo::is_demo() { return Ok(()); }
        self.patch(&format!("/api/w/{}/settings/locale", slug), &serde_json::json!({"locale": locale})).await
    }

    pub async fn regenerate_ical_token(&self, slug: &str) -> Result<ICalTokenResponse, String> {
        self.post(&format!("/api/w/{}/calendar/ical-token", slug), &serde_json::json!({})).await
    }

    // =====================
    // Observability
    // =====================

    pub async fn get_observability(&self, slug: &str) -> Result<ObservabilityData, String> {
        self.get(&format!("/api/w/{}/admin/observability", slug)).await
    }

    // =====================
    // Super admin
    // =====================

    pub async fn get_admin_me(&self) -> Result<AdminMe, String> {
        self.get("/api/admin/me").await
    }

    pub async fn get_global_observability(&self) -> Result<GlobalObservabilityData, String> {
        self.get("/api/admin/observability").await
    }
}

// ── Observability types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmCostByModel {
    pub provider: String,
    pub model: String,
    pub cost_usd: f64,
    pub call_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmLatencyByModel {
    pub provider: String,
    pub model: String,
    pub p50_ms: i64,
    pub p95_ms: i64,
    pub p99_ms: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmInvocationCount {
    pub invocation_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmErrorEntry {
    pub created_at: String,
    pub provider: String,
    pub model: String,
    pub error: String,
    pub invocation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QualitySignalCount {
    pub signal_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CascadeEfficiency {
    pub classifier_resolved: i64,
    pub sonnet_escalated: i64,
    pub saved_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObservabilityData {
    pub month: String,
    pub total_cost_usd: f64,
    pub total_calls: i64,
    pub median_latency_ms: i64,
    pub quality_score: f64,
    pub cost_by_model: Vec<LlmCostByModel>,
    pub latency_by_model: Vec<LlmLatencyByModel>,
    pub invocation_types: Vec<LlmInvocationCount>,
    pub cascade_efficiency: CascadeEfficiency,
    pub top_tools: Vec<serde_json::Value>,
    pub recent_errors: Vec<LlmErrorEntry>,
    pub quality_signals: Vec<QualitySignalCount>,
}

// ── Admin / Super admin types ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdminMe {
    pub is_super_admin: bool,
    pub phone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalWorkspaceStats {
    pub slug: String,
    pub name: Option<String>,
    pub plan: String,
    pub cost_usd: f64,
    pub calls: i64,
    pub quality_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalModelCostAgg {
    pub provider: String,
    pub model: String,
    pub cost_usd: f64,
    pub call_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalError {
    pub workspace_slug: String,
    pub created_at: String,
    pub provider: String,
    pub model: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalObservabilityData {
    pub generated_at: String,
    pub workspaces_count: usize,
    pub total_cost_usd: f64,
    pub total_calls: i64,
    pub by_workspace: Vec<GlobalWorkspaceStats>,
    pub cost_by_model: Vec<GlobalModelCostAgg>,
    pub recent_errors: Vec<GlobalError>,
    pub quality_signals: Vec<QualitySignalCount>,
}
