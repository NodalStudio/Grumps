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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub text: String,
    pub reply_to: Option<String>,
    /// Optional platform-native interactive markup (e.g. a Telegram
    /// `reply_markup` inline keyboard). Serialized verbatim into the send body
    /// by the platform adapter; ignored by platforms that don't support it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<serde_json::Value>,
}

pub trait MessagingPlatform {
    fn platform_id(&self) -> &str;
    fn parse_webhook(&self, payload: &[u8]) -> Result<Option<InboundMessage>, MessagingError>;
    fn verify_signature(&self, payload: &[u8], signature: &str) -> Result<(), MessagingError>;
    fn build_send_request(&self, recipient: &str, message: &OutboundMessage) -> Result<(String, String), MessagingError>;
    fn handle_verification_challenge(&self, params: &std::collections::HashMap<String, String>) -> Result<String, MessagingError>;
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
