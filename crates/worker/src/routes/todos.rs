// crates/worker/src/routes/todos.rs
use crate::extract::{ApiError, Member};
use crate::{d1_rest, db, middleware};
use grumps_core::dto::{CreateTodoRequest, UpdateTodoRequest};
use worker::*;

// ── GET /api/w/:slug/todos ────────────────────────────────────────────────────

pub async fn list_todos(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let Member { claims, ws } = m;

    let url = req.url()?;
    let params: std::collections::HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let filter = params.get("status").map(String::as_str).unwrap_or("open");
    let assignee_param = params.get("assignee").map(String::as_str);

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    // For "mine", resolve the caller's workspace member_id (different from
    // claims.sub, which is the index-DB user_id) via their Telegram numeric
    // id. Other filters ignore the second argument.
    let resolved_member: Option<String> = if filter == "mine" {
        match assignee_param {
            Some(s) => Some(s.to_string()),
            None => match claims.tg_user_id.as_deref() {
                Some(tg) if !tg.is_empty() => {
                    ws_db.find_member_by_platform_id(tg).await.ok().flatten()
                }
                _ => None,
            },
        }
    } else {
        assignee_param.map(|s| s.to_string())
    };

    let todos = ws_db
        .get_todos_filtered(filter, resolved_member.as_deref())
        .await?;
    let data: Vec<serde_json::Value> = todos
        .iter()
        .map(
            |(id, seq, title, status, assignee, priority, tags, deadline)| {
                serde_json::json!({
                    "id": id,
                    "seq_num": seq,
                    "title": title,
                    "status": status,
                    "assigned_name": assignee,
                    "priority": priority,
                    "tags": tags,
                    "deadline": deadline,
                })
            },
        )
        .collect();

    middleware::with_cors(&req, Response::from_json(&data)?)
}

// ── POST /api/w/:slug/todos ───────────────────────────────────────────────────

pub async fn create_todo(
    req: Request,
    ctx: RouteContext<()>,
    m: Member,
    body: CreateTodoRequest,
) -> Result<Response> {
    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    let tags_json =
        serde_json::to_string(&body.tags.unwrap_or_default()).unwrap_or_else(|_| "[]".into());
    // Persist the deadline only when it's a civil date "YYYY-MM-DD".
    let deadline = body
        .deadline
        .as_deref()
        .filter(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok());
    let assigned_to = body.assigned_to.unwrap_or_default();
    let assigned_name = body.assigned_name.unwrap_or_default();
    let (todo_id, seq_num) = ws_db
        .insert_todo(
            &body.title, // already trimmed at deserialize (CreateTodoRequest)
            body.priority.unwrap_or(2),
            &tags_json,
            &assigned_to,
            &assigned_name,
            &m.claims.sub,
            "api",
            "",
            deadline,
        )
        .await?;

    // Mirror the `TodoItem` shape the SPA deserializes elsewhere (GET
    // /todos), built from the already-known request fields rather than a
    // second read — the SPA now awaits this response instead of discarding
    // it, so a shape mismatch would surface as a spurious error toast on
    // every successful create.
    middleware::with_cors(
        &req,
        Response::from_json(&serde_json::json!({
            "id": todo_id,
            "seq_num": seq_num,
            "title": body.title,
            "status": "open",
            "assigned_name": if assigned_name.is_empty() { None } else { Some(assigned_name) },
            "priority": body.priority.unwrap_or(2),
            "tags": tags_json,
            "deadline": deadline,
        }))?,
    )
}

// ── PATCH /api/w/:slug/todos/:id ─────────────────────────────────────────────

pub async fn update_todo(
    req: Request,
    ctx: RouteContext<()>,
    m: Member,
    body: UpdateTodoRequest,
) -> Result<Response> {
    let todo_id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    // `status` is a closed domain enum, not a shape constraint — checked here
    // rather than in the DTO. Title length / priority range are handled by the
    // DTO's `validate()` before this handler runs.
    if let Some(s) = body.status.as_deref() {
        if !matches!(s, "open" | "in_progress" | "done") {
            return ApiError::bad_request("todo.status_invalid").into_response(&req);
        }
    }

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    ws_db
        .update_todo(
            &todo_id,
            body.title.as_deref(), // already trimmed at deserialize (UpdateTodoRequest)
            body.status.as_deref(),
            body.priority,
            body.assigned_to.as_deref(),
            body.assigned_name.as_deref(),
        )
        .await?;

    // Deadline is a civil date "YYYY-MM-DD". An *absent* field decodes to
    // `None` and leaves it untouched (so a title-only PATCH doesn't wipe the
    // deadline); an explicit empty string clears it; anything else that
    // doesn't parse as a date is silently ignored, mirroring `create_todo`'s
    // parsing so the SPA's `<input type="date">` value round-trips exactly.
    // `set_todo_deadline` is the same helper the chat "snooze"/deadline-edit
    // reply path already uses (see `handler.rs`).
    if let Some(d) = body.deadline.as_deref() {
        if d.is_empty() || chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok() {
            ws_db.set_todo_deadline(&todo_id, d).await?;
        }
    }

    let _ = body.tags; // tag updates can be added via a separate migration

    middleware::with_cors(
        &req,
        Response::from_json(&serde_json::json!({ "ok": true }))?,
    )
}

// ── DELETE /api/w/:slug/todos/:id ────────────────────────────────────────────

pub async fn delete_todo(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let todo_id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);
    ws_db.delete_todo(&todo_id).await?;

    middleware::with_cors(
        &req,
        Response::from_json(&serde_json::json!({ "ok": true }))?,
    )
}
