pub mod types;
pub use types::*;

use std::sync::Arc;

/// Shared SPA API client interface. One impl for live HTTP (`LiveApi`),
/// one for demo seed data (`DemoApi`). The choice between them happens
/// once at startup based on `crate::demo::is_demo()`.
///
/// The trait makes demo coverage type-checked: every method must be
/// implemented by both impls, so a forgotten demo branch becomes a
/// compile error rather than a silent fall-through to the network.
///
/// `Send + Sync` bounds are trivially satisfied in the WASM single-threaded
/// runtime but are required by Leptos context storage.
#[async_trait::async_trait(?Send)]
pub trait Api: Send + Sync {
    // Auth
    async fn send_otp(&self, phone: &str) -> Result<OtpResponse, String>;
    async fn verify_otp(&self, phone: &str, code: &str) -> Result<VerifyResponse, String>;

    // Workspaces
    async fn get_workspaces(&self) -> Result<Vec<WorkspaceInfo>, String>;
    async fn get_workspace_info(&self, slug: &str) -> Result<WorkspaceOverview, String>;

    // Todos
    async fn get_todos(&self, slug: &str, filter: &str) -> Result<Vec<TodoItem>, String>;
    async fn create_todo(&self, slug: &str, title: &str, priority: i32)
        -> Result<TodoItem, String>;
    async fn update_todo(
        &self,
        slug: &str,
        id: &str,
        updates: &serde_json::Value,
    ) -> Result<(), String>;
    async fn delete_todo(&self, slug: &str, id: &str) -> Result<(), String>;

    // Notes
    async fn get_notes(&self, slug: &str) -> Result<Vec<NoteItem>, String>;
    async fn get_note(&self, slug: &str, id: &str) -> Result<NoteItem, String>;
    async fn create_note(&self, slug: &str, title: &str, content: &str)
        -> Result<NoteItem, String>;
    async fn update_note(
        &self,
        slug: &str,
        id: &str,
        title: &str,
        content: &str,
    ) -> Result<(), String>;
    async fn delete_note(&self, slug: &str, id: &str) -> Result<(), String>;

    // History + Members
    async fn get_history(&self, slug: &str) -> Result<Vec<ActivityItem>, String>;
    async fn get_members(&self, slug: &str) -> Result<Vec<MemberItem>, String>;

    // Memory
    async fn list_memory(&self, slug: &str) -> Result<Vec<MemoryItem>, String>;
    async fn create_memory(
        &self,
        slug: &str,
        body: &serde_json::Value,
    ) -> Result<MemoryItem, String>;
    async fn update_memory(
        &self,
        slug: &str,
        id: &str,
        body: &serde_json::Value,
    ) -> Result<(), String>;
    async fn delete_memory(&self, slug: &str, id: &str) -> Result<(), String>;

    // Events
    async fn list_events(&self, slug: &str) -> Result<Vec<EventItem>, String>;
    async fn create_event(&self, slug: &str, body: &serde_json::Value)
        -> Result<EventItem, String>;
    async fn update_event(
        &self,
        slug: &str,
        id: &str,
        body: &serde_json::Value,
    ) -> Result<(), String>;
    async fn delete_event(&self, slug: &str, id: &str) -> Result<(), String>;

    // Scheduled actions
    async fn list_scheduled_actions(&self, slug: &str) -> Result<Vec<ScheduledActionItem>, String>;
    async fn create_scheduled_action(
        &self,
        slug: &str,
        body: &serde_json::Value,
    ) -> Result<ScheduledActionItem, String>;
    async fn update_scheduled_action(
        &self,
        slug: &str,
        id: &str,
        body: &serde_json::Value,
    ) -> Result<(), String>;
    async fn delete_scheduled_action(&self, slug: &str, id: &str) -> Result<(), String>;

    // Calendar + Settings
    async fn list_calendar(
        &self,
        slug: &str,
        from: &str,
        to: &str,
    ) -> Result<Vec<CalendarItem>, String>;
    async fn get_settings(&self, slug: &str) -> Result<WorkspaceSettings, String>;
    async fn update_settings(&self, slug: &str, body: &serde_json::Value) -> Result<(), String>;
    async fn update_workspace_locale(&self, slug: &str, locale: &str) -> Result<(), String>;
    /// Set the workspace timezone (auto-detected from the browser; server
    /// adopts it only if not already explicitly configured).
    async fn update_timezone(&self, slug: &str, tz: &str) -> Result<(), String>;
    async fn regenerate_ical_token(&self, slug: &str) -> Result<ICalTokenResponse, String>;

    // Observability + Admin
    async fn get_observability(&self, slug: &str) -> Result<ObservabilityData, String>;
    async fn get_admin_me(&self) -> Result<AdminMe, String>;
    async fn get_global_observability(&self) -> Result<GlobalObservabilityData, String>;
}

/// Shared handle to whichever Api impl is in use for this session.
pub type ApiHandle = Arc<dyn Api + Send + Sync>;

/// Construct the right Api impl for the current mode and stash it in
/// Leptos context. Call once from `App` at startup, before any
/// component reads it via `use_api()`.
pub fn provide_api() {
    let api: ApiHandle = if crate::demo::is_demo() {
        Arc::new(demo::DemoApi)
    } else {
        Arc::new(live::LiveApi)
    };
    leptos::prelude::provide_context(api);
}

/// Fetch the Api handle from Leptos context. Panics if `provide_api`
/// was never called — same failure mode as any unprovided context.
pub fn use_api() -> ApiHandle {
    leptos::prelude::expect_context::<ApiHandle>()
}

pub mod demo;
pub mod live;
