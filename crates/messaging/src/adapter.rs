use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type MessageId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub platform: String,
    pub channel_id: String,
    pub message_id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub text: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub is_mention_to_bot: bool,
    pub is_direct_message: bool,
    pub quoted_message_id: Option<String>,
    pub quoted_message_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub text: String,
    pub reply_to: Option<String>,
    /// The todo this message is a card for, if any. Lets the webhook send loop
    /// record `bot_messages.todo_id` so a later reply to the card resolves to
    /// the right todo. `None` for non-card messages.
    #[serde(default)]
    pub todo_id: Option<String>,
    /// Whether the text contains intentional markup (e.g. `*bold*`/`_italic_`
    /// in help text) that a platform should render with its native markdown
    /// parser. Defaults to `false`: most bodies interpolate raw user content
    /// (todo titles, tags, names) where a lone `_` or `*` would otherwise
    /// break rendering (or, on Telegram, get the whole message rejected).
    #[serde(default)]
    pub markdown: bool,
}

impl OutboundMessage {
    /// A plain message not tied to any todo, with no markup.
    pub fn text(text: String) -> Self {
        Self {
            text,
            reply_to: None,
            todo_id: None,
            markdown: false,
        }
    }
}

/// A chat platform adapter: pure protocol knowledge, zero I/O.
///
/// Every method is synchronous and side-effect-free — adapters *translate*
/// between a platform's wire format and our common types; the worker routes
/// perform the actual HTTP calls and hold the credentials. This is what lets
/// this crate compile natively (fast unit tests, no wasm, no network mocks)
/// while the worker targets wasm32.
pub trait MessagingPlatform {
    /// Stable tag persisted in the DB (`"whatsapp"`, `"telegram"`, …).
    /// Identifies the *product channel*, not the transport — e.g. the WAHA
    /// gateway adapter still returns `"whatsapp"`.
    fn platform_id(&self) -> &str;

    /// Normalize a platform webhook body into our common [`InboundMessage`].
    ///
    /// `Ok(None)` means "valid payload, nothing to process" (delivery
    /// receipts, own-message echoes, status events) — routes ack these
    /// silently. `Err` is reserved for genuinely malformed payloads.
    fn parse_webhook(&self, payload: &[u8]) -> Result<Option<InboundMessage>, MessagingError>;

    /// Authenticate a webhook against the *raw* body bytes (re-serialized
    /// JSON would break the MAC). Scheme is per-platform: Meta sends
    /// `sha256=<hex>` HMAC-SHA256, WAHA bare-hex HMAC-SHA512, Telegram a
    /// plain shared-token header.
    fn verify_signature(&self, payload: &[u8], signature: &str) -> Result<(), MessagingError>;

    /// Build the outbound send call as `(url, json_body)` — deliberately
    /// *not* `send_message`: executing the request (fetch, auth headers,
    /// retries, response ids) is the worker's job, so secrets and the
    /// Cloudflare `Fetch` type never leak into this crate.
    fn build_send_request(
        &self,
        recipient: &str,
        message: &OutboundMessage,
    ) -> Result<(String, String), MessagingError>;

    /// Answer a webhook-registration handshake (Meta's `hub.challenge` GET
    /// echo). Platforms without one (Telegram, WAHA) return
    /// `Err(VerificationFailed)` and register no GET route.
    fn handle_verification_challenge(
        &self,
        params: &std::collections::HashMap<String, String>,
    ) -> Result<String, MessagingError>;
}

#[derive(Debug, thiserror::Error)]
pub enum MessagingError {
    #[error("invalid signature")]
    InvalidSignature,
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
    #[error("verification failed: {0}")]
    VerificationFailed(String),
}

/// Decide whether a platform's outbound-send HTTP response counts as
/// success. Pure: takes the already-fetched status code and parsed JSON body
/// so the worker can `console_error!` full context on failure without this
/// crate touching `Fetch`/`Response` (see the trait docs above — zero I/O by
/// design keeps this crate natively testable).
///
/// - Any non-2xx status is always a failure.
/// - Telegram additionally reports failures as `{"ok": false, ...}` inside a
///   2xx body — a missing or `false` `ok` field is treated as failure too.
/// - Other platforms (WAHA, WhatsApp Cloud API) have no universal `ok` flag;
///   a top-level `error` key signals failure even under a 2xx status.
pub fn is_send_ok(platform: &str, status: u16, body: &serde_json::Value) -> bool {
    if !(200..300).contains(&status) {
        return false;
    }
    match platform {
        "telegram" => body.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        _ => body.get("error").is_none(),
    }
}

/// Truncate a response body for a log line. Full bodies can be large; this
/// keeps just enough to diagnose a send failure without blowing out log
/// volume. Splits on a char boundary so multi-byte UTF-8 is never cut mid-
/// codepoint.
pub fn truncate_body(body: &str, max_len: usize) -> String {
    if body.len() <= max_len {
        return body.to_string();
    }
    let mut end = max_len;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &body[..end])
}

#[cfg(test)]
mod send_result_tests {
    use super::*;

    #[test]
    fn non_2xx_status_is_always_a_failure() {
        assert!(!is_send_ok(
            "telegram",
            500,
            &serde_json::json!({"ok": true})
        ));
        assert!(!is_send_ok("whatsapp", 403, &serde_json::json!({})));
    }

    #[test]
    fn telegram_requires_explicit_ok_true() {
        assert!(is_send_ok(
            "telegram",
            200,
            &serde_json::json!({"ok": true})
        ));
        assert!(!is_send_ok(
            "telegram",
            200,
            &serde_json::json!({"ok": false, "description": "bad request"})
        ));
        // Missing `ok` (e.g. unexpected body shape) is treated as a failure,
        // not a silent success.
        assert!(!is_send_ok("telegram", 200, &serde_json::json!({})));
    }

    #[test]
    fn other_platforms_default_ok_unless_error_present() {
        assert!(is_send_ok(
            "whatsapp",
            200,
            &serde_json::json!({"key": {"id": "abc"}})
        ));
        assert!(!is_send_ok(
            "whatsapp",
            200,
            &serde_json::json!({"error": "session not connected"})
        ));
    }

    #[test]
    fn truncate_body_short_is_unchanged() {
        assert_eq!(truncate_body("short", 300), "short");
    }

    #[test]
    fn truncate_body_long_is_cut_with_ellipsis() {
        let long = "a".repeat(400);
        let out = truncate_body(&long, 300);
        assert_eq!(out.chars().count(), 301); // 300 'a' + ellipsis char
        assert!(out.ends_with('…'));
    }

    #[test]
    fn truncate_body_respects_utf8_boundaries() {
        // Each "é" is 2 bytes; cutting at byte 5 would land mid-codepoint.
        let s = "ééééé";
        let out = truncate_body(s, 5);
        assert!(out.is_char_boundary(out.len() - '…'.len_utf8()));
    }
}
