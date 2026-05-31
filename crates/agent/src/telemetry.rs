//! LLM call telemetry. Wrappers record cost/latency/tokens to the workspace D1.

use crate::db::AgentDb;
use serde::{Deserialize, Serialize};

/// Pricing in USD per 1M tokens. Update when model prices change.
fn price_per_million(provider: &str, model: &str) -> (f64, f64) {
    // (input_per_1M, output_per_1M)
    match (provider, model) {
        ("anthropic", m) if m.contains("sonnet") => (3.0, 15.0),
        ("anthropic", m) if m.contains("haiku") => (0.25, 1.25),
        ("anthropic", m) if m.contains("opus") => (15.0, 75.0),
        ("gemini", m) if m.contains("flash") => (0.15, 0.60),
        ("gemini", m) if m.contains("pro") => (1.25, 5.00),
        _ => (0.0, 0.0),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCallRecord {
    pub id: String,
    pub member_id: Option<String>,
    pub invocation_type: String,
    pub parent_call_id: Option<String>,
    pub provider: String,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_write_tokens: u32,
    pub cost_usd: f64,
    pub latency_ms: u32,
    pub tool_calls: Vec<serde_json::Value>,
    pub success: bool,
    pub error: Option<String>,
}

impl LlmCallRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        invocation_type: &str,
        provider: &str,
        model: &str,
        input: u32,
        output: u32,
        cache_read: u32,
        cache_write: u32,
        latency_ms: u32,
        success: bool,
        error: Option<String>,
        member_id: Option<String>,
        parent_call_id: Option<String>,
        tool_calls: Vec<serde_json::Value>,
    ) -> Self {
        let (in_price, out_price) = price_per_million(provider, model);
        let cost =
            (input as f64 / 1_000_000.0) * in_price + (output as f64 / 1_000_000.0) * out_price;
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            member_id,
            invocation_type: invocation_type.to_string(),
            parent_call_id,
            provider: provider.to_string(),
            model: model.to_string(),
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cache_read,
            cache_write_tokens: cache_write,
            cost_usd: cost,
            latency_ms,
            tool_calls,
            success,
            error,
        }
    }
}

/// Helper to record an LLM call to the DB. Returns the call id on success.
pub async fn record_call(db: &dyn AgentDb, record: LlmCallRecord) -> Option<String> {
    let id = record.id.clone();
    db.log_llm_call(&record).await.ok()?;
    Some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_calculation_sonnet() {
        let rec = LlmCallRecord::build(
            "agent_react",
            "anthropic",
            "claude-sonnet-4-6",
            1_000_000,
            1_000_000,
            0,
            0,
            1000,
            true,
            None,
            None,
            None,
            vec![],
        );
        // 3.0 input + 15.0 output = 18.0 USD
        assert!((rec.cost_usd - 18.0).abs() < 0.001);
    }

    #[test]
    fn cost_calculation_gemini_flash() {
        let rec = LlmCallRecord::build(
            "classify",
            "gemini",
            "gemini-2.5-flash",
            1_000_000,
            1_000_000,
            0,
            0,
            200,
            true,
            None,
            None,
            None,
            vec![],
        );
        // 0.15 input + 0.60 output = 0.75 USD
        assert!((rec.cost_usd - 0.75).abs() < 0.001);
    }

    #[test]
    fn cost_calculation_unknown_model() {
        let rec = LlmCallRecord::build(
            "classify",
            "unknown",
            "unknown-model",
            1_000_000,
            1_000_000,
            0,
            0,
            200,
            true,
            None,
            None,
            None,
            vec![],
        );
        assert_eq!(rec.cost_usd, 0.0);
    }
}
