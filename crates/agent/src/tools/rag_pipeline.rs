//! RAG : embed chat messages via Workers AI bge-m3, upsert to Vectorize.
//! See spec § 6.3.
//!
//! Workers AI (`env.ai("AI")`) is supported in workers-rs 0.8.
//! Vectorize has NO binding in workers-rs 0.8 — we call the CF REST API directly,
//! same pattern as D1RestClient: CF_ACCOUNT_ID + CF_API_TOKEN secrets.

use worker::*;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatVectorMetadata {
    pub workspace_slug: String,
    pub platform: String,
    pub sender_member_id: String,
    pub sender_name: String,
    pub text: String,
    pub timestamp: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct QueryHit {
    pub sender_name: String,
    pub timestamp: String,
    pub text: String,
    pub score: f32,
}

// ---------------------------------------------------------------------------
// Internal AI request / response shapes for bge-m3
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct EmbedRequest<'a> {
    text: Vec<&'a str>,
}

#[derive(Deserialize)]
struct EmbedResponse {
    /// bge-m3 returns shape: { "data": [[f32; 1024]] }
    data: Vec<Vec<f32>>,
}

// ---------------------------------------------------------------------------
// Internal Vectorize REST shapes
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct VectorizeVector<'a> {
    id: &'a str,
    values: &'a [f32],
    metadata: &'a ChatVectorMetadata,
    namespace: &'a str,
}

#[derive(Serialize)]
struct VectorizeUpsertBody<'a> {
    vectors: Vec<VectorizeVector<'a>>,
}

#[derive(Serialize)]
struct VectorizeQueryBody<'a> {
    vector: &'a [f32],
    #[serde(rename = "topK")]
    top_k: u32,
    filter: serde_json::Value,
    #[serde(rename = "returnMetadata")]
    return_metadata: bool,
}

#[derive(Deserialize)]
struct VectorizeQueryResponse {
    result: VectorizeQueryResult,
}

#[derive(Deserialize)]
struct VectorizeQueryResult {
    matches: Vec<VectorizeMatch>,
}

#[derive(Deserialize)]
struct VectorizeMatch {
    score: f32,
    metadata: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn vectorize_base(account_id: &str, index: &str) -> String {
    format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/vectorize/v2/indexes/{index}"
    )
}

async fn cf_post(url: &str, token: &str, body: &str) -> Result<String> {
    let mut headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {token}"))?;
    headers.set("Content-Type", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(body)));

    let req = Request::new_with_init(url, &init)?;
    let mut resp = Fetch::Request(req).send().await?;
    if resp.status_code() >= 400 {
        let text = resp.text().await.unwrap_or_default();
        return Err(Error::RustError(format!(
            "CF API error {}: {}",
            resp.status_code(),
            text
        )));
    }
    resp.text().await
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Embed a text into a 1024-dim vector via Workers AI bge-m3.
///
/// Uses `env.ai("AI")` binding (supported in workers-rs 0.8).
pub async fn embed(env: &Env, text: &str) -> Result<Vec<f32>> {
    let ai = env.ai("AI")?;
    let input = EmbedRequest { text: vec![text] };
    let resp: EmbedResponse = ai.run("@cf/baai/bge-m3", input).await?;
    resp.data
        .into_iter()
        .next()
        .ok_or_else(|| Error::RustError("bge-m3 returned empty data array".into()))
}

/// Ingest a chat message: embed + upsert to Vectorize via CF REST API.
///
/// Skips messages shorter than 20 chars.
/// Vector ID: `{workspace_slug}:{sender_member_id}:{timestamp}`.
pub async fn ingest_message(env: &Env, meta: &ChatVectorMetadata) -> Result<()> {
    if meta.text.len() < 20 {
        return Ok(());
    }

    let vector = embed(env, &meta.text).await?;

    // Vectorize REST — no workers-rs 0.8 binding exists for Vectorize.
    let account_id = env.secret("CF_ACCOUNT_ID")?.to_string();
    let token = env.secret("CF_API_TOKEN")?.to_string();

    let id = format!(
        "{}:{}:{}",
        meta.workspace_slug, meta.sender_member_id, meta.timestamp
    );

    let body = VectorizeUpsertBody {
        vectors: vec![VectorizeVector {
            id: &id,
            values: &vector,
            metadata: meta,
            namespace: &meta.workspace_slug,
        }],
    };

    let url = format!("{}/upsert", vectorize_base(&account_id, "CHAT_RAG"));
    let body_str = serde_json::to_string(&body)
        .map_err(|e| Error::RustError(e.to_string()))?;

    cf_post(&url, &token, &body_str).await?;

    console_log!(
        "RAG ingest ok: ws={} sender={}",
        meta.workspace_slug,
        meta.sender_name
    );
    Ok(())
}

/// Query Vectorize for top-K most similar messages in a workspace.
///
/// Embeds the query string, then calls Vectorize REST query endpoint.
pub async fn query_chat_history(
    env: &Env,
    workspace_slug: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<QueryHit>> {
    let vector = embed(env, query).await?;

    let account_id = env.secret("CF_ACCOUNT_ID")?.to_string();
    let token = env.secret("CF_API_TOKEN")?.to_string();

    let body = VectorizeQueryBody {
        vector: &vector,
        top_k: limit,
        filter: serde_json::json!({ "workspace_slug": workspace_slug }),
        return_metadata: true,
    };

    let url = format!("{}/query", vectorize_base(&account_id, "CHAT_RAG"));
    let body_str = serde_json::to_string(&body)
        .map_err(|e| Error::RustError(e.to_string()))?;

    let resp_text = cf_post(&url, &token, &body_str).await?;

    let parsed: VectorizeQueryResponse = serde_json::from_str(&resp_text)
        .map_err(|e| Error::RustError(format!("Vectorize query parse error: {e}")))?;

    let hits: Vec<QueryHit> = parsed
        .result
        .matches
        .into_iter()
        .filter_map(|m| {
            let meta = m.metadata?;
            Some(QueryHit {
                sender_name: meta["sender_name"].as_str().unwrap_or("").to_string(),
                timestamp: meta["timestamp"].as_str().unwrap_or("").to_string(),
                text: meta["text"].as_str().unwrap_or("").to_string(),
                score: m.score,
            })
        })
        .collect();

    console_log!(
        "RAG query ok: ws={} q='{}' hits={}",
        workspace_slug,
        query,
        hits.len()
    );
    Ok(hits)
}
