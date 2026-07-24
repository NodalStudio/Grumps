// crates/worker/src/routes/export.rs
use crate::{d1_rest, db, middleware};
use worker::*;

/// GET /api/w/:slug/export/todos?format=csv|json
pub async fn export_todos(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let slug = ctx.param("slug").unwrap();
    if !claims.workspaces.contains(&slug.to_string()) {
        return middleware::error_with_cors(
            &req,
            403,
            "auth.not_member",
            "not a member of this workspace",
        );
    }

    let index_db = db::get_index_db(&ctx.env)?;
    let ws = match db::lookup_workspace_by_slug(&index_db, slug).await? {
        Some(w) => w,
        None => {
            return middleware::error_with_cors(
                &req,
                404,
                "workspace.not_found",
                "workspace not found",
            )
        }
    };
    let d1_client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&d1_client, ws.d1_database_id);

    let todos = ws_db.get_todos_filtered("all", None).await?;

    let url = req.url()?;
    let params: std::collections::HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let format = params.get("format").map(|s| s.as_str()).unwrap_or("json");

    let (content, content_type, filename) = match format {
        "csv" => {
            let mut csv = String::from("seq_num,title,status,assignee,priority,tags\n");
            for (_id, seq, title, status, assignee, priority, tags, _deadline) in &todos {
                let a = assignee.as_deref().unwrap_or("");
                csv.push_str(&format!(
                    "{},\"{}\",{},{},{},{}\n",
                    seq,
                    title.replace('"', "\"\""),
                    status,
                    a,
                    priority,
                    tags
                ));
            }
            (csv, "text/csv", "todos.csv")
        }
        _ => {
            let json_todos: Vec<serde_json::Value> = todos
                .iter()
                .map(
                    |(_id, seq, title, status, assignee, priority, tags, deadline)| {
                        serde_json::json!({
                            "seq_num": seq,
                            "title": title,
                            "status": status,
                            "assignee": assignee,
                            "priority": priority,
                            "tags": tags,
                            "deadline": deadline
                        })
                    },
                )
                .collect();
            (
                serde_json::to_string_pretty(&json_todos).unwrap(),
                "application/json",
                "todos.json",
            )
        }
    };

    let mut resp = Response::ok(content)?;
    resp.headers_mut().set("Content-Type", content_type)?;
    resp.headers_mut().set(
        "Content-Disposition",
        &format!("attachment; filename=\"{}\"", filename),
    )?;
    middleware::with_cors(&req, resp)
}

/// GET /api/w/:slug/export/notes?format=json|md
pub async fn export_notes(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let slug = ctx.param("slug").unwrap();
    if !claims.workspaces.contains(&slug.to_string()) {
        return middleware::error_with_cors(
            &req,
            403,
            "auth.not_member",
            "not a member of this workspace",
        );
    }

    let index_db = db::get_index_db(&ctx.env)?;
    let ws = match db::lookup_workspace_by_slug(&index_db, slug).await? {
        Some(w) => w,
        None => {
            return middleware::error_with_cors(
                &req,
                404,
                "workspace.not_found",
                "workspace not found",
            )
        }
    };
    let d1_client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&d1_client, ws.d1_database_id);

    let notes = ws_db.get_notes().await?;

    let url = req.url()?;
    let params: std::collections::HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let format = params.get("format").map(|s| s.as_str()).unwrap_or("json");

    let (content, content_type, filename) = match format {
        "md" => {
            let mut md = String::new();
            for (_, title, source, created_at) in &notes {
                let t = title.as_deref().unwrap_or("Untitled");
                md.push_str(&format!(
                    "# {}\n\n_Created: {} (Source: {})_\n\n---\n\n",
                    t, created_at, source
                ));
            }
            (md, "text/markdown", "notes.md")
        }
        _ => {
            let json_notes: Vec<serde_json::Value> = notes
                .iter()
                .map(|(id, title, source, created_at)| {
                    serde_json::json!({
                        "id": id,
                        "title": title,
                        "source": source,
                        "created_at": created_at
                    })
                })
                .collect();
            (
                serde_json::to_string_pretty(&json_notes).unwrap(),
                "application/json",
                "notes.json",
            )
        }
    };

    let mut resp = Response::ok(content)?;
    resp.headers_mut().set("Content-Type", content_type)?;
    resp.headers_mut().set(
        "Content-Disposition",
        &format!("attachment; filename=\"{}\"", filename),
    )?;
    middleware::with_cors(&req, resp)
}
