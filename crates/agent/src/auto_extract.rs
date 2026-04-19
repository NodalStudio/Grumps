//! Auto-extract pipeline (mode 2, opt-in).
//! Reads chat messages, classifies via Gemini Flash, persists "memorable" content
//! to memory_entries with source=chat-auto + confidence < 1.
//! See spec § 6.2.

use worker::*;
use serde::Deserialize;
use crate::db::AgentDb;

const PROMPT: &str = r#"You analyze a chat message from a group conversation and decide if it contains
a fact, decision, preference, or info about a person, that would be useful to remember.

Return ONLY JSON:
{
  "is_memorable": true|false,
  "kind": "fact" | "person" | "decision" | "preference" | "place" | "other",
  "key": "short slug like 'wifi-bureau' or null",
  "value": "the extracted content as a clean sentence",
  "related_member": "member name if about a person, or null",
  "confidence": 0.0..1.0
}

Skip if: small talk, joke, question, ambiguous statement, anything personal/sensitive (health, money,
conflict, romance), commands to the bot. Be conservative — when in doubt, is_memorable=false.

Message:
"#;

#[derive(Debug, Deserialize)]
pub struct AutoExtractResult {
    pub is_memorable: bool,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub related_member: Option<String>,
    #[serde(default)]
    pub confidence: f32,
}

/// Process a single chat message for auto-memory extraction.
/// Returns Ok(Some(memory_id)) if something was saved, Ok(None) otherwise.
pub async fn process_message(
    env: &Env,
    db: &(dyn AgentDb + 'static),
    text: &str,
    sender_member_id: &str,
) -> Result<Option<String>> {
    if text.split_whitespace().count() < 4 { return Ok(None); }
    if text.len() > 2000 { return Ok(None); }   // skip very long messages — likely not a fact

    // Daily cap : 1000 classifications/workspace/day (KV-backed)
    let kv = match env.kv("KV") { Ok(k) => k, Err(_) => return Ok(None) };
    let workspace_slug = "default";   // we don't have it directly here; caller can pass it if needed
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let cap_key = format!("ax:{workspace_slug}:{day}");
    let used: u32 = kv.get(&cap_key).text().await.ok().flatten()
        .and_then(|s| s.parse().ok()).unwrap_or(0);
    if used >= 1000 {
        return Ok(None);
    }

    // Call Gemini Flash with the extract prompt
    let api_key = env.secret("GEMINI_API_KEY")?.to_string();
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={api_key}");

    let req_body = serde_json::json!({
        "contents": [{
            "role": "user",
            "parts": [{ "text": format!("{PROMPT}{text}") }]
        }],
        "generationConfig": {
            "temperature": 0.1,
            "responseMimeType": "application/json",
            "maxOutputTokens": 256,
        }
    });

    let mut headers = Headers::new();
    headers.set("content-type", "application/json")?;
    let request = Request::new_with_init(&url, RequestInit::new()
        .with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(req_body.to_string().into())))?;

    let mut resp = match Fetch::Request(request).send().await {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    if resp.status_code() >= 400 { return Ok(None); }

    let parsed: serde_json::Value = match resp.json().await {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    let text_response = parsed.pointer("/candidates/0/content/parts/0/text")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let extract: AutoExtractResult = match serde_json::from_str(text_response.trim()) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };

    if !extract.is_memorable || extract.confidence < 0.5 { return Ok(None); }

    // Bump KV counter
    let _ = kv.put(&cap_key, &(used + 1).to_string()).map(|p| p.expiration_ttl(86400 * 2).execute());

    // Persist to memory_entries
    let kind = match extract.kind.as_str() {
        "person" => grumps_memory::MemoryKind::Person,
        "decision" => grumps_memory::MemoryKind::Decision,
        "preference" => grumps_memory::MemoryKind::Preference,
        "place" => grumps_memory::MemoryKind::Place,
        "fact" => grumps_memory::MemoryKind::Fact,
        _ => grumps_memory::MemoryKind::Other,
    };
    let entry = grumps_memory::NewMemoryEntry {
        key: extract.key,
        value: extract.value,
        kind,
        related_member: extract.related_member,
        tags: vec![],
        source: grumps_memory::MemorySource::ChatAuto,
        confidence: Some(extract.confidence as f64),
        pinned: Some(false),
        expires_at: None,
        created_by: Some(sender_member_id.to_string()),
    };
    let id = db.create_memory(&entry).await?;
    Ok(Some(id))
}
