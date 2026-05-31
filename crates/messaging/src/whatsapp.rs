// TODO(onboarding-parity): localised welcome flow with admin-promotion
// detection, workspace locale resolution, and setChatDescription-equivalent
// group metadata update. Mirror the Telegram implementation once WhatsApp
// Business API surfaces group admin events reliably.
// Reference spec: docs/superpowers/specs/2026-04-21-telegram-onboarding-ux-design.md

use crate::adapter::*;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::Deserialize;
use std::collections::HashMap;

type HmacSha256 = Hmac<Sha256>;

pub struct WhatsAppAdapter {
    pub phone_number_id: String,
    pub verify_token: String,
    pub app_secret: String,
    pub access_token: String,
}

impl WhatsAppAdapter {
    pub fn new(phone_number_id: String, verify_token: String, app_secret: String, access_token: String) -> Self {
        Self { phone_number_id, verify_token, app_secret, access_token }
    }
}

impl MessagingPlatform for WhatsAppAdapter {
    fn platform_id(&self) -> &str { "whatsapp" }

    fn verify_signature(&self, payload: &[u8], signature: &str) -> Result<(), MessagingError> {
        let hex_sig = signature.strip_prefix("sha256=").ok_or(MessagingError::InvalidSignature)?;
        let expected = hex::decode(hex_sig).map_err(|_| MessagingError::InvalidSignature)?;
        let mut mac = HmacSha256::new_from_slice(self.app_secret.as_bytes()).map_err(|_| MessagingError::InvalidSignature)?;
        mac.update(payload);
        mac.verify_slice(&expected).map_err(|_| MessagingError::InvalidSignature)
    }

    fn parse_webhook(&self, payload: &[u8]) -> Result<Option<InboundMessage>, MessagingError> {
        let body: WaWebhook = serde_json::from_slice(payload).map_err(|e| MessagingError::InvalidPayload(e.to_string()))?;
        let entry = match body.entry.first() { Some(e) => e, None => return Ok(None) };
        let change = match entry.changes.first() { Some(c) => c, None => return Ok(None) };
        let v = &change.value;
        let msg = match v.messages.as_ref().and_then(|m| m.first()) { Some(m) => m, None => return Ok(None) };

        let text = msg.text.as_ref().map(|t| t.body.clone());
        let sender_name = v.contacts.as_ref().and_then(|c| c.first()).and_then(|c| c.profile.as_ref()).map(|p| p.name.clone()).unwrap_or_else(|| msg.from.clone());
        let is_group = v.metadata.as_ref().and_then(|m| m.display_phone_number.as_ref()).is_some();
        let is_mention = text.as_ref().map(|t| { let l = t.to_lowercase(); l.contains("@grumps") || l.starts_with("grumps ") || l.starts_with("grumps,") }).unwrap_or(false);
        let (quoted_id, quoted_text) = msg.context.as_ref().map(|ctx| (Some(ctx.message_id.clone()), ctx.quoted_text.clone())).unwrap_or((None, None));
        let ts = chrono::DateTime::from_timestamp(msg.timestamp.parse::<i64>().unwrap_or(0), 0).unwrap_or_else(chrono::Utc::now);

        Ok(Some(InboundMessage {
            platform: "whatsapp".into(),
            channel_id: v.metadata.as_ref().and_then(|m| m.phone_number_id.clone()).unwrap_or_else(|| entry.id.clone()),
            message_id: msg.id.clone(), sender_id: msg.from.clone(), sender_name, text, timestamp: ts,
            is_mention_to_bot: is_mention || !is_group, is_direct_message: !is_group,
            quoted_message_id: quoted_id, quoted_message_text: quoted_text,
        }))
    }

    fn build_send_request(&self, recipient: &str, message: &OutboundMessage) -> Result<(String, String), MessagingError> {
        let url = format!("https://graph.facebook.com/v21.0/{}/messages", self.phone_number_id);
        let mut body = serde_json::json!({"messaging_product":"whatsapp","to":recipient,"type":"text","text":{"body":message.text}});
        if let Some(ref r) = message.reply_to { body["context"] = serde_json::json!({"message_id":r}); }
        Ok((url, body.to_string()))
    }

    fn handle_verification_challenge(&self, params: &HashMap<String, String>) -> Result<String, MessagingError> {
        let mode = params.get("hub.mode").ok_or_else(|| MessagingError::VerificationFailed("missing hub.mode".into()))?;
        let token = params.get("hub.verify_token").ok_or_else(|| MessagingError::VerificationFailed("missing token".into()))?;
        let challenge = params.get("hub.challenge").ok_or_else(|| MessagingError::VerificationFailed("missing challenge".into()))?;
        if mode != "subscribe" || token != &self.verify_token { return Err(MessagingError::VerificationFailed("mismatch".into())); }
        Ok(challenge.clone())
    }
}

// Meta webhook deserialization types
#[derive(Deserialize)] pub struct WaWebhook { pub entry: Vec<WaEntry> }
#[derive(Deserialize)] pub struct WaEntry { pub id: String, pub changes: Vec<WaChange> }
#[derive(Deserialize)] pub struct WaChange { pub value: WaValue }
#[derive(Deserialize)] pub struct WaValue { pub metadata: Option<WaMeta>, pub contacts: Option<Vec<WaContact>>, pub messages: Option<Vec<WaMsg>> }
#[derive(Deserialize)] pub struct WaMeta { pub display_phone_number: Option<String>, pub phone_number_id: Option<String> }
#[derive(Deserialize)] pub struct WaContact { pub profile: Option<WaProfile> }
#[derive(Deserialize)] pub struct WaProfile { pub name: String }
#[derive(Deserialize)] pub struct WaMsg { pub from: String, pub id: String, pub timestamp: String, pub text: Option<WaText>, pub context: Option<WaCtx> }
#[derive(Deserialize)] pub struct WaText { pub body: String }
#[derive(Deserialize)] pub struct WaCtx { pub message_id: String, pub quoted_text: Option<String> }

#[cfg(test)]
mod tests {
    use super::*;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    fn adapter() -> WhatsAppAdapter {
        WhatsAppAdapter::new(
            "12345678".into(),
            "myverifytoken".into(),
            "mysecret".into(),
            "myaccesstoken".into(),
        )
    }

    fn make_sig(secret: &str, payload: &[u8]) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload);
        let result = mac.finalize().into_bytes();
        format!("sha256={}", hex::encode(result))
    }

    fn simple_payload(text: &str, from: &str, phone_number_id: Option<&str>, display_phone_number: Option<&str>) -> Vec<u8> {
        let meta = match (phone_number_id, display_phone_number) {
            (Some(pid), Some(dpn)) => serde_json::json!({"phone_number_id": pid, "display_phone_number": dpn}),
            (Some(pid), None) => serde_json::json!({"phone_number_id": pid}),
            _ => serde_json::json!({}),
        };
        serde_json::json!({
            "entry": [{
                "id": "entry1",
                "changes": [{
                    "value": {
                        "metadata": meta,
                        "contacts": [{"profile": {"name": "Alice"}}],
                        "messages": [{
                            "from": from,
                            "id": "msg1",
                            "timestamp": "1700000000",
                            "text": {"body": text}
                        }]
                    }
                }]
            }]
        }).to_string().into_bytes()
    }

    // Test 1: HMAC verification succeeds with correct signature
    #[test]
    fn test_verify_signature_correct() {
        let a = adapter();
        let payload = b"hello world";
        let sig = make_sig("mysecret", payload);
        assert!(a.verify_signature(payload, &sig).is_ok());
    }

    // Test 2: HMAC verification fails with wrong signature
    #[test]
    fn test_verify_signature_wrong() {
        let a = adapter();
        let payload = b"hello world";
        let sig = make_sig("wrongsecret", payload);
        assert!(a.verify_signature(payload, &sig).is_err());
    }

    // Test 3: HMAC verification fails without sha256= prefix
    #[test]
    fn test_verify_signature_no_prefix() {
        let a = adapter();
        let payload = b"hello world";
        assert!(a.verify_signature(payload, "deadbeef").is_err());
    }

    // Test 4: Parse text message — extracts sender name, text, channel_id
    #[test]
    fn test_parse_text_message() {
        let a = adapter();
        let payload = simple_payload("Hello!", "15551234567", Some("12345678"), Some("+1 555 1234567"));
        let result = a.parse_webhook(&payload).unwrap().unwrap();
        assert_eq!(result.sender_name, "Alice");
        assert_eq!(result.text.as_deref(), Some("Hello!"));
        assert_eq!(result.channel_id, "12345678");
        assert_eq!(result.platform, "whatsapp");
    }

    // Test 5: Parse message with @grumps mention — is_mention_to_bot = true
    #[test]
    fn test_parse_mention_grumps() {
        let a = adapter();
        let payload = simple_payload("Hey @grumps what time?", "15551234567", Some("12345678"), Some("+1 555 1234567"));
        let result = a.parse_webhook(&payload).unwrap().unwrap();
        assert!(result.is_mention_to_bot);
    }

    // Test 6: Parse message without mention — is_mention_to_bot depends on group/DM
    #[test]
    fn test_parse_no_mention_group() {
        let a = adapter();
        // Group message (has display_phone_number), no mention
        let payload = simple_payload("just chatting", "15551234567", Some("12345678"), Some("+1 555 1234567"));
        let result = a.parse_webhook(&payload).unwrap().unwrap();
        assert!(!result.is_mention_to_bot);
        assert!(!result.is_direct_message);
    }

    // Test 6b: DM (no display_phone_number) — is_mention_to_bot = true
    #[test]
    fn test_parse_dm_is_mention() {
        let a = adapter();
        let payload = simple_payload("just chatting", "15551234567", Some("12345678"), None);
        let result = a.parse_webhook(&payload).unwrap().unwrap();
        assert!(result.is_mention_to_bot);
        assert!(result.is_direct_message);
    }

    // Test 7: Parse message with quoted context
    #[test]
    fn test_parse_quoted_context() {
        let a = adapter();
        let payload = serde_json::json!({
            "entry": [{
                "id": "entry1",
                "changes": [{
                    "value": {
                        "metadata": {"phone_number_id": "12345678", "display_phone_number": "+1"},
                        "contacts": [{"profile": {"name": "Bob"}}],
                        "messages": [{
                            "from": "15559876543",
                            "id": "msg2",
                            "timestamp": "1700000001",
                            "text": {"body": "Replying"},
                            "context": {"message_id": "origmsg1", "quoted_text": "original text"}
                        }]
                    }
                }]
            }]
        }).to_string().into_bytes();
        let result = a.parse_webhook(&payload).unwrap().unwrap();
        assert_eq!(result.quoted_message_id.as_deref(), Some("origmsg1"));
        assert_eq!(result.quoted_message_text.as_deref(), Some("original text"));
    }

    // Test 8: Parse empty entry → None
    #[test]
    fn test_parse_empty_entry() {
        let a = adapter();
        let payload = serde_json::json!({"entry": []}).to_string().into_bytes();
        let result = a.parse_webhook(&payload).unwrap();
        assert!(result.is_none());
    }

    // Test 9: Parse status update (no messages) → None
    #[test]
    fn test_parse_status_update() {
        let a = adapter();
        let payload = serde_json::json!({
            "entry": [{
                "id": "entry1",
                "changes": [{
                    "value": {
                        "metadata": {"phone_number_id": "12345678"},
                        "statuses": [{"id": "msg1", "status": "delivered"}]
                    }
                }]
            }]
        }).to_string().into_bytes();
        let result = a.parse_webhook(&payload).unwrap();
        assert!(result.is_none());
    }

    // Test 10: Verification challenge success
    #[test]
    fn test_verification_challenge_success() {
        let a = adapter();
        let mut params = HashMap::new();
        params.insert("hub.mode".into(), "subscribe".into());
        params.insert("hub.verify_token".into(), "myverifytoken".into());
        params.insert("hub.challenge".into(), "challenge123".into());
        let result = a.handle_verification_challenge(&params).unwrap();
        assert_eq!(result, "challenge123");
    }

    // Test 11: Verification challenge wrong token → error
    #[test]
    fn test_verification_challenge_wrong_token() {
        let a = adapter();
        let mut params = HashMap::new();
        params.insert("hub.mode".into(), "subscribe".into());
        params.insert("hub.verify_token".into(), "wrongtoken".into());
        params.insert("hub.challenge".into(), "challenge123".into());
        assert!(a.handle_verification_challenge(&params).is_err());
    }

    // Test 12: Verification challenge missing params → error
    #[test]
    fn test_verification_challenge_missing_params() {
        let a = adapter();
        let params = HashMap::new();
        assert!(a.handle_verification_challenge(&params).is_err());
    }

    // Test 13: build_send_request — URL contains phone_number_id
    #[test]
    fn test_build_send_request_url() {
        let a = adapter();
        let msg = OutboundMessage { text: "Hi there".into(), reply_to: None, reply_markup: None };
        let (url, _) = a.build_send_request("15551234567", &msg).unwrap();
        assert!(url.contains("12345678"));
        assert!(url.contains("graph.facebook.com"));
    }

    // Test 14: build_send_request — body contains message text
    #[test]
    fn test_build_send_request_body_text() {
        let a = adapter();
        let msg = OutboundMessage { text: "Hello from Grumps".into(), reply_to: None, reply_markup: None };
        let (_, body) = a.build_send_request("15551234567", &msg).unwrap();
        assert!(body.contains("Hello from Grumps"));
    }

    // Test 15: build_send_request with reply_to — body contains context
    #[test]
    fn test_build_send_request_reply_to() {
        let a = adapter();
        let msg = OutboundMessage { text: "Reply text".into(), reply_to: Some("origmsg99".into()), reply_markup: None };
        let (_, body) = a.build_send_request("15551234567", &msg).unwrap();
        assert!(body.contains("origmsg99"));
        assert!(body.contains("context"));
    }
}
