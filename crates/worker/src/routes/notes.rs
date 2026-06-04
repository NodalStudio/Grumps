// crates/worker/src/routes/notes.rs
use crate::extract::{ApiError, Member};
use crate::{d1_rest, db, middleware};
use grumps_core::dto::{CreateNoteRequest, UpdateNoteRequest};
use worker::*;

// ── GET /api/w/:slug/notes ────────────────────────────────────────────────────

pub async fn list_notes(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);
    let notes = ws_db.get_notes().await?;

    let data: Vec<serde_json::Value> = notes
        .iter()
        .map(|(id, title, source, created_at)| {
            serde_json::json!({
                "id": id,
                "title": title,
                "source": source,
                "created_at": created_at,
            })
        })
        .collect();

    middleware::with_cors(&req, Response::from_json(&data)?)
}

// ── POST /api/w/:slug/notes ───────────────────────────────────────────────────

pub async fn create_note(
    req: Request,
    ctx: RouteContext<()>,
    m: Member,
    body: CreateNoteRequest,
) -> Result<Response> {
    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    let note_id = ws_db
        .insert_note(
            &body.title.unwrap_or_default(),
            &body.content,
            "api",
            &m.claims.sub,
        )
        .await?;

    middleware::with_cors(
        &req,
        Response::from_json(&serde_json::json!({ "id": note_id }))?,
    )
}

// ── GET /api/w/:slug/notes/:id ────────────────────────────────────────────────

pub async fn get_note(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let note_id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    match ws_db.get_note_by_id(&note_id).await? {
        Some(note) => middleware::with_cors(&req, Response::from_json(&note)?),
        None => ApiError::not_found("note.not_found").into_response(&req),
    }
}

// ── PUT /api/w/:slug/notes/:id ────────────────────────────────────────────────

pub async fn update_note(
    req: Request,
    ctx: RouteContext<()>,
    m: Member,
    body: UpdateNoteRequest,
) -> Result<Response> {
    let note_id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    ws_db
        .update_note(
            &note_id,
            &body.title.unwrap_or_default(),
            &body.content,
            &m.claims.sub,
        )
        .await?;

    middleware::with_cors(
        &req,
        Response::from_json(&serde_json::json!({ "ok": true }))?,
    )
}

// ── DELETE /api/w/:slug/notes/:id ────────────────────────────────────────────

pub async fn delete_note(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let note_id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);
    ws_db.delete_note(&note_id).await?;

    middleware::with_cors(
        &req,
        Response::from_json(&serde_json::json!({ "ok": true }))?,
    )
}
