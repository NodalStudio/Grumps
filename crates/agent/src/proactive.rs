//! Proactive mode (c) — agent intervenes spontaneously without @mention.
//! Opt-in by workspace, rate-limited 3/hour with 60s cooldown.
//! See spec § 11.

use worker::*;
use crate::db::AgentDb;
use crate::router::MessagingSink;

/// Run the proactive intervention pipeline on an incoming message.
/// Returns Ok(()) regardless of whether the agent intervened.
/// Errors are logged but never propagated to the caller (chat flow must not break).
pub async fn maybe_intervene<'a>(
    env: &'a Env,
    db: &'a dyn AgentDb,
    sink: &'a dyn MessagingSink,
    workspace_slug: &str,
    member_id: &str,
    text: &str,
) -> Result<()> {
    if text.split_whitespace().count() < 4 { return Ok(()); }

    let kv = match env.kv("KV") { Ok(k) => k, Err(_) => return Ok(()) };

    // Silence override check
    let silence_key = format!("proactive:{workspace_slug}:silence_until");
    if kv.get(&silence_key).text().await.ok().flatten().is_some() {
        return Ok(());
    }

    // Cooldown 60s (KV)
    let cooldown_key = format!("proactive:{workspace_slug}:lastfire");
    if kv.get(&cooldown_key).text().await.ok().flatten().is_some() {
        return Ok(());
    }

    // Rate limit : 3/hour
    let hour_key = format!("proactive:{workspace_slug}:{}", chrono::Utc::now().format("%Y-%m-%d-%H"));
    let count: u32 = kv.get(&hour_key).text().await.ok().flatten()
        .and_then(|s| s.parse().ok()).unwrap_or(0);
    let max_per_hour = db.get_int_setting("proactive_max_per_hour").await.unwrap_or(3) as u32;
    if count >= max_per_hour { return Ok(()); }

    // Step 1 : Gemini classifier "should_intervene?"
    let pinned = db.list_pinned_memory().await.unwrap_or_default();
    let pinned_summary = if pinned.is_empty() {
        "(no pinned facts)".to_string()
    } else {
        pinned.iter().take(20).map(|m| format!("- {}", m.value)).collect::<Vec<_>>().join("\n")
    };

    let api_key = match env.secret("GEMINI_API_KEY") { Ok(k) => k.to_string(), Err(_) => return Ok(()) };
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={api_key}");
    let prompt = format!(
        "An assistant 'Grumps' lives in a group chat. You decide if it should chime in on a message.\n\
         The assistant knows these workspace facts:\n{pinned_summary}\n\n\
         Message:\n{text}\n\n\
         Should Grumps intervene with something SPECIFIC, USEFUL, NON-INTRUSIVE?\n\
         Skip if: small talk, ongoing emotional conversation, conflict, routine chatter.\n\
         Return ONLY JSON: {{\"should_intervene\": true|false, \"reason\": \"why\"}}"
    );
    let req_body = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": { "temperature": 0.1, "responseMimeType": "application/json", "maxOutputTokens": 128 }
    });
    let mut headers = Headers::new();
    headers.set("content-type", "application/json")?;
    let request = Request::new_with_init(&url, RequestInit::new()
        .with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(req_body.to_string().into())))?;
    let mut resp = match Fetch::Request(request).send().await { Ok(r) => r, Err(_) => return Ok(()) };
    if resp.status_code() >= 400 { return Ok(()); }
    let parsed: serde_json::Value = match resp.json().await { Ok(p) => p, Err(_) => return Ok(()) };
    let text_resp = parsed.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()).unwrap_or("");
    let classifier: serde_json::Value = match serde_json::from_str(text_resp.trim()) { Ok(v) => v, Err(_) => return Ok(()) };
    if !classifier.get("should_intervene").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Ok(());
    }

    // Step 2 : Sonnet filter — give it the message + pinned + last 10 chat messages, ask for SPECIFIC reply or empty
    let db_members = db.list_active_members().await.unwrap_or_default();
    let members: Vec<crate::prompt::MemberShort> = db_members
        .into_iter()
        .map(|m| crate::prompt::MemberShort {
            display_name: m.display_name.unwrap_or_else(|| "unknown".to_string()),
            role: m.role,
        })
        .collect();

    let prompt_ctx = crate::prompt::PromptContext {
        workspace_name: workspace_slug.to_string(),
        platform: "telegram".to_string(),
        member_count: members.len(),
        persona: "default".to_string(),
        language: "fr".to_string(),
        pinned_memories: pinned,
        members,
        now_local: chrono::Utc::now().to_rfc3339(),
        timezone: "UTC".to_string(),
        proactive_mode: true,
        auto_memory: false,
        agent_calls_remaining: 1000,
        agent_calls_quota: 1000,
        web_search_remaining: 50,
        web_search_quota: 50,
    };
    let system = format!(
        "{}\n\nADDITIONAL RULE FOR THIS CALL: You may stay silent. Only respond if you have a SPECIFIC, VERIFIABLE, CONCISE thing to add to this conversation. If unsure, return empty text.\nClassifier hinted: {}",
        crate::prompt::build_system_prompt(&prompt_ctx),
        classifier.get("reason").and_then(|v| v.as_str()).unwrap_or("")
    );

    let req = crate::llm::anthropic::AnthropicRequest {
        system: Some(system),
        messages: vec![crate::llm::anthropic::Message {
            role: "user".into(),
            content: serde_json::Value::String(format!("Group message from member {member_id}: {text}")),
        }],
        ..Default::default()
    };
    let resp = match crate::llm::anthropic::call(env, &req).await { Ok(r) => r, Err(_) => return Ok(()) };
    let final_text = resp.content.iter().filter_map(|b| match b {
        crate::llm::anthropic::ContentBlock::Text { text } => Some(text.as_str()),
        _ => None,
    }).collect::<Vec<_>>().join(" ").trim().to_string();
    if final_text.is_empty() || final_text.len() < 5 { return Ok(()); }

    // Send + bump counters
    let _ = sink.send(&final_text).await;
    let _ = kv.put(&cooldown_key, "1").map(|p| p.expiration_ttl(60).execute());
    let _ = kv.put(&hour_key, &(count + 1).to_string()).map(|p| p.expiration_ttl(7200).execute());

    Ok(())
}
