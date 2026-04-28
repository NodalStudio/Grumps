//! Request/response DTOs shared between the live HTTP impl and the demo
//! seed impl. Pure data — no client logic.

use serde::{Deserialize, Serialize};

pub fn api_base() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .map(|origin| {
            if origin.contains("localhost") || origin.contains("127.0.0.1") {
                // Local dev: use a relative URL so trunk's dev server proxies
                // /auth/* and /api/* through to the worker on :8787 — same
                // origin means cookies (esp. grumps_csrf) are readable.
                String::new()
            } else {
                // Worker is served at api.grumps.app; SPA lives at grumps.app
                origin.replace("grumps.app", "api.grumps.app")
            }
        })
        .unwrap_or_default()
}

// =====================
// Response types
// =====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    #[serde(default)]
    pub id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatusCounts {
    #[serde(default)]
    pub open_todos: i64,
    #[serde(default)]
    pub done_this_week: i64,
    #[serde(default)]
    pub notes: i64,
    #[serde(default)]
    pub files: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceOverview {
    pub slug: String,
    pub name: Option<String>,
    pub plan: String,
    #[serde(default)]
    pub stats: StatusCounts,
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
