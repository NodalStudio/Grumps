//! Anthropic Sonnet 4.6 client (HTTP via Fetch).
//! Used by the agent loop for tool use + reasoning.

use worker::*;
use serde::{Deserialize, Serialize};

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MODEL: &str = "claude-sonnet-4-6";

#[derive(Serialize, Debug, Clone)]
pub struct AnthropicRequest {
    pub model: String,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

impl Default for AnthropicRequest {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            max_tokens: 1024,
            system: None,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            temperature: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub role: String,                    // "user" or "assistant"
    pub content: serde_json::Value,      // string OR array of content blocks
}

#[derive(Deserialize, Debug, Clone)]
pub struct AnthropicResponse {
    pub id: String,
    pub model: String,
    pub role: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: String,             // "end_turn" | "tool_use" | "max_tokens"
    pub usage: Usage,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
}

#[derive(Deserialize, Debug, Clone)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
}

/// Call the Anthropic API with telemetry recording.
/// Times the call, builds a LlmCallRecord and persists it to the DB.
pub async fn call_with_telemetry(
    env: &Env,
    db: &dyn crate::db::AgentDb,
    invocation_type: &str,
    member_id: Option<&str>,
    parent_call_id: Option<&str>,
    req: &AnthropicRequest,
) -> Result<AnthropicResponse> {
    let t0 = worker::Date::now().as_millis();
    let result = call(env, req).await;
    let latency_ms = (worker::Date::now().as_millis().saturating_sub(t0)) as u32;

    match result {
        Ok(ref resp) => {
            let record = crate::telemetry::LlmCallRecord::build(
                invocation_type,
                "anthropic",
                &resp.model,
                resp.usage.input_tokens,
                resp.usage.output_tokens,
                resp.usage.cache_read_input_tokens,
                resp.usage.cache_creation_input_tokens,
                latency_ms,
                true,
                None,
                member_id.map(|s| s.to_string()),
                parent_call_id.map(|s| s.to_string()),
                vec![],
            );
            crate::telemetry::record_call(db, record).await;
        }
        Err(ref e) => {
            let record = crate::telemetry::LlmCallRecord::build(
                invocation_type,
                "anthropic",
                &req.model,
                0,
                0,
                0,
                0,
                latency_ms,
                false,
                Some(e.to_string()),
                member_id.map(|s| s.to_string()),
                parent_call_id.map(|s| s.to_string()),
                vec![],
            );
            crate::telemetry::record_call(db, record).await;
        }
    }

    result
}

/// Call the Anthropic API. Reads ANTHROPIC_API_KEY secret.
pub async fn call(env: &Env, req: &AnthropicRequest) -> Result<AnthropicResponse> {
    let api_key = env.secret("ANTHROPIC_API_KEY")?.to_string();
    let body = serde_json::to_string(req)
        .map_err(|e| Error::RustError(format!("anthropic request serialize: {e}")))?;

    let headers = Headers::new();
    headers.set("x-api-key", &api_key)?;
    headers.set("anthropic-version", "2023-06-01")?;
    headers.set("content-type", "application/json")?;
    headers.set("anthropic-beta", "prompt-caching-2024-07-31")?;

    let request = Request::new_with_init(ANTHROPIC_URL, RequestInit::new()
        .with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(body.into())))?;

    let mut response = Fetch::Request(request).send().await?;
    let status = response.status_code();
    if status >= 400 {
        let err_body = response.text().await.unwrap_or_default();
        return Err(Error::RustError(format!("anthropic API {status}: {err_body}")));
    }
    response.json::<AnthropicResponse>().await
        .map_err(|e| Error::RustError(format!("anthropic response parse: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serializes_with_skip_empty() {
        let req = AnthropicRequest {
            model: "claude-sonnet-4-6".into(),
            max_tokens: 100,
            system: Some("You are helpful.".into()),
            messages: vec![Message {
                role: "user".into(),
                content: serde_json::json!("hello"),
            }],
            ..Default::default()
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["model"], "claude-sonnet-4-6");
        assert_eq!(json["system"], "You are helpful.");
        assert!(json.get("tools").is_none() || json["tools"].as_array().map(|a| a.is_empty()).unwrap_or(false));
        assert!(json.get("tool_choice").is_none());
    }

    #[test]
    fn content_block_deserializes_text() {
        let json = r#"{"type":"text","text":"Hello"}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn content_block_deserializes_tool_use() {
        let json = r#"{"type":"tool_use","id":"tu_1","name":"create_todo","input":{"title":"x"}}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        match block {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "tu_1");
                assert_eq!(name, "create_todo");
                assert_eq!(input["title"], "x");
            }
            _ => panic!("wrong variant"),
        }
    }
}
