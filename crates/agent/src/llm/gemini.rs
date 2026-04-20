//! Gemini 2.5 Flash classifier client (HTTP via Fetch).
//! Used by the cascade router to decide CRUD-direct vs Sonnet escalation.
//! See spec § 8.2.

use worker::*;
use serde::{Deserialize, Serialize};

const GEMINI_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";

const CLASSIFIER_PROMPT: &str = r#"You classify a chat message addressed to an assistant in a group.
Return ONLY JSON (no prose, no markdown fence) matching this schema:
{
  "intent": "create_todo" | "create_note" | "create_event" | "create_reminder"
          | "list_todos" | "list_notes" | "list_events" | "list_reminders"
          | "mark_done" | "delete_item" | "workspace_link"
          | "complex_agent_task",
  "confidence": number 0..1,
  "args": { ... extracted fields if simple intent, or {} }
}

Use 'complex_agent_task' if any of:
- needs to search the web
- needs to recall past conversations or workspace memory
- needs multi-step reasoning or planning
- is ambiguous (could mean several things)
- is a question that needs reasoning beyond a list lookup
- creates something with conditions or follow-ups
- is creative/open-ended

Members in this group: "#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedIntent {
    pub intent: String,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    #[serde(default)]
    pub args: serde_json::Value,
}

fn default_confidence() -> f32 { 0.0 }

impl ClassifiedIntent {
    pub fn is_complex(&self) -> bool {
        self.intent == "complex_agent_task"
    }
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.85
    }
}

#[derive(Serialize)]
struct GeminiRequest<'a> {
    contents: Vec<GeminiContent<'a>>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct GeminiContent<'a> {
    role: &'a str,
    parts: Vec<GeminiPart<'a>>,
}

#[derive(Serialize)]
struct GeminiPart<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "responseMimeType")]
    response_mime_type: String,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Deserialize, Debug)]
struct GeminiResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize, Debug)]
struct GeminiCandidate {
    #[serde(default)]
    content: Option<GeminiContentResp>,
}

#[derive(Deserialize, Debug)]
struct GeminiContentResp {
    #[serde(default)]
    parts: Vec<GeminiPartResp>,
}

#[derive(Deserialize, Debug)]
struct GeminiPartResp {
    #[serde(default)]
    text: String,
}

/// classify_intent with telemetry recording.
pub async fn classify_intent_with_telemetry(
    env: &Env,
    db: &dyn crate::db::AgentDb,
    invocation_type: &str,
    member_id: Option<&str>,
    message: &str,
    members: &[String],
) -> Result<ClassifiedIntent> {
    let t0 = worker::Date::now().as_millis();
    let result = classify_intent(env, message, members).await;
    let latency_ms = (worker::Date::now().as_millis().saturating_sub(t0)) as u32;

    // Gemini doesn't return token counts in the response we parse — use 0 as placeholder.
    // The actual token cost is very small (classifier prompt ~256 tokens out).
    let (success, error) = match &result {
        Ok(_) => (true, None),
        Err(e) => (false, Some(e.to_string())),
    };
    let record = crate::telemetry::LlmCallRecord::build(
        invocation_type,
        "gemini",
        "gemini-2.5-flash",
        0, // input tokens not returned by Gemini REST in our parse
        0, // output tokens
        0,
        0,
        latency_ms,
        success,
        error,
        member_id.map(|s| s.to_string()),
        None,
        vec![],
    );
    crate::telemetry::record_call(db, record).await;

    result
}

/// Classify the intent of a chat message.
/// Returns `complex_agent_task` with confidence 0.0 if the API fails or returns junk
/// (caller will then escalate to Sonnet, which is the safe fallback).
pub async fn classify_intent(env: &Env, message: &str, members: &[String]) -> Result<ClassifiedIntent> {
    let api_key = env.secret("GEMINI_API_KEY")?.to_string();
    let url = format!("{GEMINI_URL}?key={api_key}");

    let members_str = if members.is_empty() {
        "(none provided)".to_string()
    } else {
        members.join(", ")
    };

    let system_text = format!("{CLASSIFIER_PROMPT}{members_str}\n\nMessage to classify:\n{message}");

    let req = GeminiRequest {
        contents: vec![GeminiContent {
            role: "user",
            parts: vec![GeminiPart { text: &system_text }],
        }],
        generation_config: GenerationConfig {
            temperature: 0.1,
            response_mime_type: "application/json".into(),
            max_output_tokens: 256,
        },
    };

    let body = serde_json::to_string(&req)
        .map_err(|e| Error::RustError(format!("gemini req serialize: {e}")))?;

    let mut headers = Headers::new();
    headers.set("content-type", "application/json")?;

    let request = Request::new_with_init(&url, RequestInit::new()
        .with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(body.into())))?;

    let mut resp = match Fetch::Request(request).send().await {
        Ok(r) => r,
        Err(e) => {
            console_log!("gemini classifier fetch error: {e} — defaulting to complex");
            return Ok(ClassifiedIntent {
                intent: "complex_agent_task".into(),
                confidence: 0.0,
                args: serde_json::Value::Null,
            });
        }
    };

    let status = resp.status_code();
    if status >= 400 {
        let err_body = resp.text().await.unwrap_or_default();
        console_log!("gemini classifier {status}: {err_body} — defaulting to complex");
        return Ok(ClassifiedIntent {
            intent: "complex_agent_task".into(),
            confidence: 0.0,
            args: serde_json::Value::Null,
        });
    }

    let parsed: GeminiResponse = match resp.json().await {
        Ok(p) => p,
        Err(e) => {
            console_log!("gemini classifier parse error: {e} — defaulting to complex");
            return Ok(ClassifiedIntent {
                intent: "complex_agent_task".into(),
                confidence: 0.0,
                args: serde_json::Value::Null,
            });
        }
    };

    let text = parsed.candidates.first()
        .and_then(|c| c.content.as_ref())
        .and_then(|c| c.parts.first())
        .map(|p| p.text.as_str())
        .unwrap_or("");

    // Try to parse JSON from the text (Gemini with responseMimeType=application/json should give clean JSON)
    let intent: ClassifiedIntent = serde_json::from_str(text.trim()).unwrap_or_else(|e| {
        console_log!("gemini classifier JSON parse failed: {e}, text was: {text} — defaulting to complex");
        ClassifiedIntent {
            intent: "complex_agent_task".into(),
            confidence: 0.0,
            args: serde_json::Value::Null,
        }
    });

    Ok(intent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classified_intent_high_confidence() {
        let c = ClassifiedIntent {
            intent: "create_todo".into(),
            confidence: 0.92,
            args: serde_json::json!({"title": "test"}),
        };
        assert!(c.is_high_confidence());
        assert!(!c.is_complex());
    }

    #[test]
    fn classified_intent_complex() {
        let c = ClassifiedIntent {
            intent: "complex_agent_task".into(),
            confidence: 0.95,
            args: serde_json::Value::Null,
        };
        assert!(c.is_complex());
    }

    #[test]
    fn classified_intent_low_confidence_should_escalate() {
        let c = ClassifiedIntent {
            intent: "create_todo".into(),
            confidence: 0.5,
            args: serde_json::Value::Null,
        };
        assert!(!c.is_high_confidence());
    }
}
