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
}

impl OutboundMessage {
    /// A plain message not tied to any todo.
    pub fn text(text: String) -> Self {
        Self {
            text,
            reply_to: None,
            todo_id: None,
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
