// crates/worker/src/routes/todos.rs
use worker::*;
use serde::Deserialize;
use crate::{db, middleware, d1_rest};

// ── helpers ──────────────────────────────────────────────────────────────────

async fn resolve_workspace(ctx: &RouteContext<()>) -> Result<db::WorkspaceMetaRow> {
    let slug = ctx.param("slug").ok_or_else(|| Error::RustError("missing slug".into()))?;
    let index_db = db::get_index_db(&ctx.env)?;
    db::lookup_workspace_by_slug(&index_db, slug).await?
        .ok_or_else(|| Error::RustError("workspace not found".into()))
}

// ── GET /api/w/:slug/todos ────────────────────────────────────────────────────

pub async fn list_todos(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let ws = match resolve_workspace(&ctx).await {
        Ok(w) => w,
        Err(_) => return middleware::error_with_cors(&req, 404, "workspace.not_found", "workspace not found"),
    };
    if !claims.workspaces.contains(&ws.slug) {
        return middleware::error_with_cors(&req, 403, "auth.not_member", "not a member of this workspace");
    }

    let url = req.url()?;
    let params: std::collections::HashMap<String, String> = url.query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let filter = params.get("status").map(String::as_str).unwrap_or("open");
    let member_id = params.get("assignee").map(String::as_str);

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    let todos = ws_db.get_todos_filtered(filter, member_id.or(Some(&claims.sub))).await?;
    let data: Vec<serde_json::Value> = todos.iter().map(|(id, seq, title, status, assignee, priority, tags)| {
        serde_json::json!({
            "id": id,
            "seq_num": seq,
            "title": title,
            "status": status,
            "assigned_name": assignee,
            "priority": priority,
            "tags": tags,
        })
    }).collect();

    middleware::with_cors(&req, Response::from_json(&data)?)
}

// ── POST /api/w/:slug/todos ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateTodo {
    title: String,
    priority: Option<i32>,
    tags: Option<Vec<String>>,
    assigned_to: Option<String>,
    assigned_name: Option<String>,
    deadline: Option<String>,
}

pub async fn create_todo(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let ws = match resolve_workspace(&ctx).await {
        Ok(w) => w,
        Err(_) => return middleware::error_with_cors(&req, 404, "workspace.not_found", "workspace not found"),
    };
    if !claims.workspaces.contains(&ws.slug) {
        return middleware::error_with_cors(&req, 403, "auth.not_member", "not a member of this workspace");
    }

    let body: CreateTodo = req.json().await.map_err(|_| Error::RustError("invalid json".into()))?;
    if body.title.trim().is_empty() {
        return middleware::with_cors(&req, Response::error("title required", 400)?);
    }

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    let tags_json = serde_json::to_string(&body.tags.unwrap_or_default()).unwrap_or_else(|_| "[]".into());
    let (todo_id, seq_num) = ws_db.insert_todo(
        &body.title,
        body.priority.unwrap_or(3),
        &tags_json,
        &body.assigned_to.unwrap_or_default(),
        &body.assigned_name.unwrap_or_default(),
        &claims.sub,
        "api",
        "",
    ).await?;

    let _ = body.deadline; // stored via future migration

    middleware::with_cors(&req, Response::from_json(&serde_json::json!({
        "id": todo_id,
        "seq_num": seq_num,
    }))?)
}

// ── PATCH /api/w/:slug/todos/:id ─────────────────────────────────────────────

#[derive(Deserialize)]
struct UpdateTodo {
    title: Option<String>,
    status: Option<String>,
    priority: Option<i32>,
    tags: Option<Vec<String>>,
    assigned_to: Option<String>,
    assigned_name: Option<String>,
}

pub async fn update_todo(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let ws = match resolve_workspace(&ctx).await {
        Ok(w) => w,
        Err(_) => return middleware::error_with_cors(&req, 404, "workspace.not_found", "workspace not found"),
    };
    if !claims.workspaces.contains(&ws.slug) {
        return middleware::error_with_cors(&req, 403, "auth.not_member", "not a member of this workspace");
    }

    let todo_id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();
    let body: UpdateTodo = req.json().await.map_err(|_| Error::RustError("invalid json".into()))?;

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    ws_db.update_todo(
        &todo_id,
        body.title.as_deref(),
        body.status.as_deref(),
        body.priority,
        body.assigned_to.as_deref(),
        body.assigned_name.as_deref(),
    ).await?;

    let _ = body.tags; // tag updates can be added via a separate migration

    middleware::with_cors(&req, Response::from_json(&serde_json::json!({ "ok": true }))?)
}

// ── DELETE /api/w/:slug/todos/:id ────────────────────────────────────────────

pub async fn delete_todo(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let ws = match resolve_workspace(&ctx).await {
        Ok(w) => w,
        Err(_) => return middleware::error_with_cors(&req, 404, "workspace.not_found", "workspace not found"),
    };
    if !claims.workspaces.contains(&ws.slug) {
        return middleware::error_with_cors(&req, 403, "auth.not_member", "not a member of this workspace");
    }

    let todo_id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    ws_db.delete_todo(&todo_id).await?;

    middleware::with_cors(&req, Response::from_json(&serde_json::json!({ "ok": true }))?)
}
