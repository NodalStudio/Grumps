use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use leptos::prelude::*;

const API_BASE: &str = "http://localhost:8787"; // TODO: make configurable

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
    pub fn new(token: ReadSignal<Option<String>>) -> Self {
        Self { token }
    }

    fn auth_header(&self) -> Option<String> {
        self.token.get().map(|t| format!("Bearer {}", t))
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", API_BASE, path);
        let mut req = Request::get(&url);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", &auth);
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() {
            return Err(format!("HTTP {}: {}", resp.status(), resp.status_text()));
        }
        resp.json().await.map_err(|e| e.to_string())
    }

    async fn post<B: Serialize, T: for<'de> Deserialize<'de>>(&self, path: &str, body: &B) -> Result<T, String> {
        let url = format!("{}{}", API_BASE, path);
        let mut req = Request::post(&url).header("Content-Type", "application/json");
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", &auth);
        }
        let resp = req.json(body).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() {
            return Err(format!("HTTP {}", resp.status()));
        }
        resp.json().await.map_err(|e| e.to_string())
    }

    async fn put<B: Serialize>(&self, path: &str, body: &B) -> Result<(), String> {
        let url = format!("{}{}", API_BASE, path);
        let mut req = Request::put(&url).header("Content-Type", "application/json");
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", &auth);
        }
        let resp = req.json(body).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() { return Err(format!("HTTP {}", resp.status())); }
        Ok(())
    }

    async fn patch<B: Serialize>(&self, path: &str, body: &B) -> Result<(), String> {
        let url = format!("{}{}", API_BASE, path);
        let mut req = Request::patch(&url).header("Content-Type", "application/json");
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", &auth);
        }
        let resp = req.json(body).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() { return Err(format!("HTTP {}", resp.status())); }
        Ok(())
    }

    async fn delete(&self, path: &str) -> Result<(), String> {
        let url = format!("{}{}", API_BASE, path);
        let mut req = Request::delete(&url);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", &auth);
        }
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
        self.get("/api/workspaces").await
    }

    pub async fn get_workspace_info(&self, slug: &str) -> Result<StatusCounts, String> {
        self.get(&format!("/api/w/{}", slug)).await
    }

    // =====================
    // Todos
    // =====================

    pub async fn get_todos(&self, slug: &str, filter: &str) -> Result<Vec<TodoItem>, String> {
        self.get(&format!("/api/w/{}/todos?status={}", slug, filter)).await
    }

    pub async fn create_todo(&self, slug: &str, title: &str, priority: i32) -> Result<TodoItem, String> {
        self.post(&format!("/api/w/{}/todos", slug), &serde_json::json!({"title": title, "priority": priority})).await
    }

    pub async fn update_todo(&self, slug: &str, id: &str, updates: &serde_json::Value) -> Result<(), String> {
        self.patch(&format!("/api/w/{}/todos/{}", slug, id), updates).await
    }

    pub async fn delete_todo(&self, slug: &str, id: &str) -> Result<(), String> {
        self.delete(&format!("/api/w/{}/todos/{}", slug, id)).await
    }

    // =====================
    // Notes
    // =====================

    pub async fn get_notes(&self, slug: &str) -> Result<Vec<NoteItem>, String> {
        self.get(&format!("/api/w/{}/notes", slug)).await
    }

    pub async fn get_note(&self, slug: &str, id: &str) -> Result<NoteItem, String> {
        self.get(&format!("/api/w/{}/notes/{}", slug, id)).await
    }

    pub async fn create_note(&self, slug: &str, title: &str, content: &str) -> Result<NoteItem, String> {
        self.post(&format!("/api/w/{}/notes", slug), &serde_json::json!({"title": title, "content": content})).await
    }

    pub async fn update_note(&self, slug: &str, id: &str, title: &str, content: &str) -> Result<(), String> {
        self.put(&format!("/api/w/{}/notes/{}", slug, id), &serde_json::json!({"title": title, "content": content})).await
    }

    pub async fn delete_note(&self, slug: &str, id: &str) -> Result<(), String> {
        self.delete(&format!("/api/w/{}/notes/{}", slug, id)).await
    }

    // =====================
    // History + Members
    // =====================

    pub async fn get_history(&self, slug: &str) -> Result<Vec<ActivityItem>, String> {
        self.get(&format!("/api/w/{}/history", slug)).await
    }

    pub async fn get_members(&self, slug: &str) -> Result<Vec<MemberItem>, String> {
        self.get(&format!("/api/w/{}/members", slug)).await
    }
}
