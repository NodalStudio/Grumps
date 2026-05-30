// crates/worker/src/llm_client.rs
use worker::*;
use grumps_nlu::llm::{NluResponse, NLU_SYSTEM_PROMPT, build_user_prompt, parse_llm_response};

pub struct LlmClient {
    gemini_api_key: String,
    anthropic_api_key: String,
}

impl LlmClient {
    pub fn from_env(env: &Env) -> Result<Self> {
        Ok(Self {
            gemini_api_key: env.secret("GEMINI_API_KEY")?.to_string(),
            anthropic_api_key: env.secret("ANTHROPIC_API_KEY")?.to_string(),
        })
    }

    /// Call Gemini 2.5 Flash (primary). Falls back to Haiku on low confidence or error.
    pub async fn classify(
        &self,
        message: &str,
        sender_name: &str,
        existing_todos: &[(i64, String)],
        now_local: &str,
        timezone: &str,
    ) -> Result<NluResponse> {
        let user_prompt = build_user_prompt(message, sender_name, existing_todos, now_local, timezone);

        // Try Gemini first
        match self.call_gemini(&user_prompt).await {
            Ok(resp) if resp.confidence >= 0.5 => return Ok(resp),
            Ok(resp) => {
                console_log!("Gemini low confidence ({:.2}), falling back to Haiku", resp.confidence);
            }
            Err(e) => {
                console_log!("Gemini error: {}, falling back to Haiku", e);
            }
        }

        // Fallback to Haiku
        self.call_haiku(&user_prompt).await
    }

    async fn call_gemini(&self, user_prompt: &str) -> Result<NluResponse> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}",
            self.gemini_api_key
        );

        let body = serde_json::json!({
            "contents": [{
                "parts": [{"text": user_prompt}]
            }],
            "systemInstruction": {
                "parts": [{"text": NLU_SYSTEM_PROMPT}]
            },
            "generationConfig": {
                "responseMimeType": "application/json",
                "temperature": 0.1,
                "maxOutputTokens": 256
            }
        });

        let headers = Headers::new();
        headers.set("Content-Type", "application/json")?;

        let mut init = RequestInit::new();
        init.with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(body.to_string().into()));

        let req = Request::new_with_init(&url, &init)?;
        let mut resp = Fetch::Request(req).send().await?;

        if resp.status_code() != 200 {
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::RustError(format!("Gemini API error {}: {}", resp.status_code(), text)));
        }

        let json: serde_json::Value = resp.json().await?;

        // Extract text from Gemini response
        let text = json.pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::RustError("No text in Gemini response".into()))?;

        parse_llm_response(text).map_err(|e| Error::RustError(e))
    }

    async fn call_haiku(&self, user_prompt: &str) -> Result<NluResponse> {
        let url = "https://api.anthropic.com/v1/messages";

        let body = serde_json::json!({
            "model": "claude-haiku-4-5-20251001",
            "max_tokens": 256,
            "system": NLU_SYSTEM_PROMPT,
            "messages": [{
                "role": "user",
                "content": user_prompt
            }]
        });

        let headers = Headers::new();
        headers.set("Content-Type", "application/json")?;
        headers.set("x-api-key", &self.anthropic_api_key)?;
        headers.set("anthropic-version", "2023-06-01")?;

        let mut init = RequestInit::new();
        init.with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(body.to_string().into()));

        let req = Request::new_with_init(url, &init)?;
        let mut resp = Fetch::Request(req).send().await?;

        if resp.status_code() != 200 {
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::RustError(format!("Haiku API error {}: {}", resp.status_code(), text)));
        }

        let json: serde_json::Value = resp.json().await?;

        let text = json.pointer("/content/0/text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::RustError("No text in Haiku response".into()))?;

        parse_llm_response(text).map_err(|e| Error::RustError(e))
    }
}
