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

pub trait MessagingPlatform {
    fn platform_id(&self) -> &str;
    fn parse_webhook(&self, payload: &[u8]) -> Result<Option<InboundMessage>, MessagingError>;
    fn verify_signature(&self, payload: &[u8], signature: &str) -> Result<(), MessagingError>;
    fn build_send_request(
        &self,
        recipient: &str,
        message: &OutboundMessage,
    ) -> Result<(String, String), MessagingError>;
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
