// WAHA (WhatsApp HTTP API) transport adapter. DB `platform` stays
// "whatsapp" — WAHA is transport only (decision recorded in
// docs/superpowers/specs/2026-07-05-whatsapp-waha-groups-design.md).

use crate::adapter::*;
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha512;
use std::collections::HashMap;

type HmacSha512 = Hmac<Sha512>;

pub struct WahaAdapter {
    pub base_url: String,
    pub session: String,
    pub api_key: String,
    pub webhook_hmac: String,
}

impl WahaAdapter {
    pub fn new(base_url: String, session: String, api_key: String, webhook_hmac: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            session,
            api_key,
            webhook_hmac,
        }
    }
}

/// `payload.replyTo.id` (and `payload.id`) arrive in WAHA's serialized form
/// `{fromMe}_{chatJid}_{rawId}`, but `POST /api/sendText` returns the raw id
/// in `key.id`. Normalize to raw so inbound quotes and outbound sent-ids
/// compare equal downstream (see plan Task 2).
fn normalize_quote_id(id: &str) -> String {
    id.rsplit('_').next().unwrap_or(id).to_string()
}

impl MessagingPlatform for WahaAdapter {
    fn platform_id(&self) -> &str {
        "whatsapp"
    }

    fn verify_signature(&self, payload: &[u8], signature: &str) -> Result<(), MessagingError> {
        let expected = hex::decode(signature).map_err(|_| MessagingError::InvalidSignature)?;
        let mut mac = HmacSha512::new_from_slice(self.webhook_hmac.as_bytes())
            .map_err(|_| MessagingError::InvalidSignature)?;
        mac.update(payload);
        mac.verify_slice(&expected)
            .map_err(|_| MessagingError::InvalidSignature)
    }

    fn parse_webhook(&self, payload: &[u8]) -> Result<Option<InboundMessage>, MessagingError> {
        // Check `event` before committing to the full `WahaWebhook` shape:
        // non-message events (e.g. "session.status") carry an unrelated
        // `payload` shape that would otherwise fail deserialization.
        let event_check: WahaEventOnly = serde_json::from_slice(payload)
            .map_err(|e| MessagingError::InvalidPayload(e.to_string()))?;
        if event_check.event != "message" && event_check.event != "message.any" {
            return Ok(None);
        }

        let envelope: WahaWebhook = serde_json::from_slice(payload)
            .map_err(|e| MessagingError::InvalidPayload(e.to_string()))?;
        let msg = match envelope.payload {
            Some(p) => p,
            None => return Ok(None),
        };

        // Echo loop guard: NOWEB reflects the bot's own sends back through
        // the webhook with fromMe=true. This is the single most important
        // line in this file — drop them before they can re-enter the pipeline.
        if msg.from_me {
            return Ok(None);
        }

        let is_direct_message = !msg.from.ends_with("@g.us");
        let sender_id = if is_direct_message {
            msg.from.clone()
        } else {
            match msg.participant.clone() {
                Some(p) => p,
                None => return Ok(None), // group system event, no participant
            }
        };

        let sender_name = msg
            .data
            .as_ref()
            .and_then(|d| d.push_name.clone())
            .unwrap_or_else(|| {
                sender_id
                    .split('@')
                    .next()
                    .unwrap_or(&sender_id)
                    .to_string()
            });

        let text = if msg.body.is_empty() {
            None
        } else {
            Some(msg.body.clone())
        };

        let mentioned_bot = msg.mentioned_ids.iter().any(|m| {
            Some(m.as_str()) == envelope.me.id.as_deref()
                || Some(m.as_str()) == envelope.me.lid.as_deref()
        });
        let text_mentions_grumps = text
            .as_deref()
            .map(|t| t.to_lowercase().contains("@grumps"))
            .unwrap_or(false);
        let is_mention = mentioned_bot || text_mentions_grumps;

        // Reply-to-bot is NOT folded into is_mention_to_bot here (telegram.rs
        // precedent) — the worker resolves "is this reply addressed to
        // Grumps" via its own bot-message tracking.
        let (quoted_message_id, quoted_message_text) = match msg.reply_to {
            Some(r) => (Some(normalize_quote_id(&r.id)), r.body),
            None => (None, None),
        };

        let timestamp =
            chrono::DateTime::from_timestamp(msg.timestamp, 0).unwrap_or_else(chrono::Utc::now);

        Ok(Some(InboundMessage {
            platform: "whatsapp".into(),
            channel_id: msg.from.clone(),
            message_id: msg.id.clone(),
            sender_id,
            sender_name,
            text,
            timestamp,
            is_mention_to_bot: is_mention || is_direct_message,
            is_direct_message,
            quoted_message_id,
            quoted_message_text,
        }))
    }

    fn build_send_request(
        &self,
        recipient: &str,
        message: &OutboundMessage,
    ) -> Result<(String, String), MessagingError> {
        let url = format!("{}/api/sendText", self.base_url);
        let mut body = serde_json::json!({
            "session": self.session,
            "chatId": recipient,
            "text": message.text,
        });
        if let Some(ref r) = message.reply_to {
            body["reply_to"] = serde_json::json!(r);
        }
        Ok((url, body.to_string()))
    }

    fn handle_verification_challenge(
        &self,
        _params: &HashMap<String, String>,
    ) -> Result<String, MessagingError> {
        Err(MessagingError::VerificationFailed(
            "waha has no challenge".into(),
        ))
    }
}

// WAHA webhook deserialization types
#[derive(Deserialize)]
struct WahaEventOnly {
    event: String,
}

#[derive(Deserialize)]
struct WahaWebhook {
    #[serde(default)]
    me: WahaMe,
    #[serde(default)]
    payload: Option<WahaPayload>,
}

#[derive(Deserialize, Default)]
struct WahaMe {
    id: Option<String>,
    lid: Option<String>,
}

#[derive(Deserialize)]
struct WahaPayload {
    id: String,
    from: String,
    #[serde(rename = "fromMe")]
    from_me: bool,
    #[serde(default)]
    body: String,
    #[serde(default)]
    participant: Option<String>,
    timestamp: i64,
    #[serde(rename = "mentionedIds", default)]
    mentioned_ids: Vec<String>,
    #[serde(rename = "replyTo", default)]
    reply_to: Option<WahaReplyTo>,
    #[serde(rename = "_data", default)]
    data: Option<WahaData>,
}

#[derive(Deserialize)]
struct WahaReplyTo {
    id: String,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Deserialize, Default)]
struct WahaData {
    #[serde(rename = "pushName")]
    push_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha512;

    fn adapter() -> WahaAdapter {
        WahaAdapter::new(
            "http://localhost:3000".into(),
            "default".into(),
            "test-api-key".into(),
            "test-hmac-key".into(),
        )
    }

    fn make_sig(key: &str, payload: &[u8]) -> String {
        let mut mac = Hmac::<Sha512>::new_from_slice(key.as_bytes()).unwrap();
        mac.update(payload);
        hex::encode(mac.finalize().into_bytes())
    }

    // Builds a webhook envelope mirroring the captured ground-truth shape.
    // `me.id` = "10000000001@c.us", `me.lid` = "111111111111111@lid" (fixed,
    // so mention tests can target them).
    #[allow(clippy::too_many_arguments)]
    fn waha_payload(
        event: &str,
        from: &str,
        participant: Option<&str>,
        body: &str,
        from_me: bool,
        mentioned: Vec<&str>,
        reply_to: Option<serde_json::Value>,
    ) -> Vec<u8> {
        serde_json::json!({
            "event": event,
            "session": "default",
            "me": {"id": "10000000001@c.us", "pushName": "Bot", "lid": "111111111111111@lid"},
            "engine": "NOWEB",
            "timestamp": 1784884628000i64,
            "payload": {
                "id": format!("{}_{}_RAWID123", from_me, from),
                "from": from,
                "fromMe": from_me,
                "source": "app",
                "body": body,
                "timestamp": 1784884627,
                "participant": participant,
                "hasMedia": false,
                "media": null,
                "replyTo": reply_to,
                "mentionedIds": mentioned,
                "_data": {"pushName": "Test User"}
            }
        })
        .to_string()
        .into_bytes()
    }

    // --- verify_signature ---

    #[test]
    fn verify_signature_ok() {
        let a = adapter();
        let payload = b"hello waha";
        let sig = make_sig("test-hmac-key", payload);
        assert!(a.verify_signature(payload, &sig).is_ok());
    }

    #[test]
    fn verify_signature_wrong_key() {
        let a = adapter();
        let payload = b"hello waha";
        let sig = make_sig("wrong-key", payload);
        assert!(a.verify_signature(payload, &sig).is_err());
    }

    #[test]
    fn verify_signature_bad_hex() {
        let a = adapter();
        assert!(a.verify_signature(b"hello waha", "not-hex!!").is_err());
    }

    #[test]
    fn verify_signature_tampered_body() {
        let a = adapter();
        let sig = make_sig("test-hmac-key", b"original body");
        assert!(a.verify_signature(b"tampered body", &sig).is_err());
    }

    // --- parse_webhook: routing / echo guard ---

    #[test]
    fn from_me_message_is_dropped() {
        let a = adapter();
        let payload = waha_payload(
            "message.any",
            "120363000000000001@g.us",
            Some("10000000001@c.us"),
            "echo",
            true,
            vec![],
            None,
        );
        assert!(a.parse_webhook(&payload).unwrap().is_none());
    }

    #[test]
    fn non_message_event_is_dropped() {
        let a = adapter();
        let payload = serde_json::json!({
            "event": "session.status",
            "session": "default",
            "payload": {"status": "WORKING"}
        })
        .to_string()
        .into_bytes();
        assert!(a.parse_webhook(&payload).unwrap().is_none());
    }

    #[test]
    fn group_message_without_participant_is_dropped() {
        let a = adapter();
        let payload = waha_payload(
            "message.any",
            "120363000000000001@g.us",
            None,
            "system event text",
            false,
            vec![],
            None,
        );
        assert!(a.parse_webhook(&payload).unwrap().is_none());
    }

    // --- parse_webhook: mapping ---

    #[test]
    fn parse_group_message_maps_channel_sender_name() {
        let a = adapter();
        let payload = waha_payload(
            "message.any",
            "120363000000000001@g.us",
            Some("10000000002@c.us"),
            "hello group",
            false,
            vec![],
            None,
        );
        let msg = a.parse_webhook(&payload).unwrap().unwrap();
        assert_eq!(msg.platform, "whatsapp");
        assert_eq!(msg.channel_id, "120363000000000001@g.us");
        assert_eq!(msg.sender_id, "10000000002@c.us");
        assert_eq!(msg.sender_name, "Test User");
        assert_eq!(msg.text.as_deref(), Some("hello group"));
        assert!(!msg.is_direct_message);
    }

    #[test]
    fn parse_dm_message() {
        let a = adapter();
        let payload = waha_payload(
            "message",
            "10000000002@c.us",
            None,
            "hi bot",
            false,
            vec![],
            None,
        );
        let msg = a.parse_webhook(&payload).unwrap().unwrap();
        assert!(msg.is_direct_message);
        assert_eq!(msg.sender_id, "10000000002@c.us");
        assert!(msg.is_mention_to_bot); // DM always counts as mention
    }

    #[test]
    fn sender_name_falls_back_to_number_without_push_name() {
        let a = adapter();
        let payload = serde_json::json!({
            "event": "message.any",
            "session": "default",
            "me": {"id": "10000000001@c.us", "pushName": "Bot", "lid": "111111111111111@lid"},
            "payload": {
                "id": "false_120363000000000001@g.us_RAWID999",
                "from": "120363000000000001@g.us",
                "fromMe": false,
                "body": "hi",
                "timestamp": 1784884627,
                "participant": "10000000002@c.us"
            }
        })
        .to_string()
        .into_bytes();
        let msg = a.parse_webhook(&payload).unwrap().unwrap();
        assert_eq!(msg.sender_name, "10000000002");
    }

    #[test]
    fn empty_body_yields_none_text() {
        let a = adapter();
        let payload = waha_payload(
            "message.any",
            "10000000002@c.us",
            None,
            "",
            false,
            vec![],
            None,
        );
        let msg = a.parse_webhook(&payload).unwrap().unwrap();
        assert!(msg.text.is_none());
    }

    #[test]
    fn has_media_with_empty_body_yields_none_text() {
        let a = adapter();
        let payload = serde_json::json!({
            "event": "message.any",
            "session": "default",
            "me": {"id": "10000000001@c.us", "pushName": "Bot", "lid": "111111111111111@lid"},
            "payload": {
                "id": "false_10000000002@c.us_RAWID111",
                "from": "10000000002@c.us",
                "fromMe": false,
                "body": "",
                "timestamp": 1784884627,
                "participant": null,
                "hasMedia": true,
                "media": {"url": "http://localhost:3000/api/files/default/x.oga", "mimetype": "audio/ogg"}
            }
        })
        .to_string()
        .into_bytes();
        let msg = a.parse_webhook(&payload).unwrap().unwrap();
        assert!(msg.text.is_none());
    }

    #[test]
    fn timestamp_maps_from_epoch_seconds() {
        let a = adapter();
        let payload = waha_payload(
            "message.any",
            "10000000002@c.us",
            None,
            "hi",
            false,
            vec![],
            None,
        );
        let msg = a.parse_webhook(&payload).unwrap().unwrap();
        assert_eq!(msg.timestamp.timestamp(), 1784884627);
    }

    // --- parse_webhook: mentions ---

    #[test]
    fn mention_via_mentioned_ids_me_id() {
        let a = adapter();
        let payload = waha_payload(
            "message.any",
            "120363000000000001@g.us",
            Some("10000000002@c.us"),
            "hey there",
            false,
            vec!["10000000001@c.us"], // == me.id in fixture
            None,
        );
        let msg = a.parse_webhook(&payload).unwrap().unwrap();
        assert!(msg.is_mention_to_bot);
    }

    #[test]
    fn mention_via_mentioned_ids_me_lid() {
        let a = adapter();
        let payload = waha_payload(
            "message.any",
            "120363000000000001@g.us",
            Some("10000000002@c.us"),
            "hey there",
            false,
            vec!["111111111111111@lid"], // == me.lid in fixture
            None,
        );
        let msg = a.parse_webhook(&payload).unwrap().unwrap();
        assert!(msg.is_mention_to_bot);
    }

    #[test]
    fn mention_via_grumps_text_bonus() {
        let a = adapter();
        let payload = waha_payload(
            "message.any",
            "120363000000000001@g.us",
            Some("10000000002@c.us"),
            "@grumps aide moi",
            false,
            vec![],
            None,
        );
        let msg = a.parse_webhook(&payload).unwrap().unwrap();
        assert!(msg.is_mention_to_bot);
    }

    #[test]
    fn no_mention_on_plain_group_text() {
        let a = adapter();
        let payload = waha_payload(
            "message.any",
            "120363000000000001@g.us",
            Some("10000000002@c.us"),
            "just chatting",
            false,
            vec![],
            None,
        );
        let msg = a.parse_webhook(&payload).unwrap().unwrap();
        assert!(!msg.is_mention_to_bot);
    }

    // --- parse_webhook: quotes ---

    #[test]
    fn quote_id_normalized_from_serialized_form() {
        let a = adapter();
        let reply_to = serde_json::json!({
            "id": "false_120363000000000001@g.us_A55836D391A4D69FAE65489C8D04C52D",
            "participant": "10000000002@c.us",
            "body": "original task text"
        });
        let payload = waha_payload(
            "message.any",
            "120363000000000001@g.us",
            Some("10000000002@c.us"),
            "done",
            false,
            vec![],
            Some(reply_to),
        );
        let msg = a.parse_webhook(&payload).unwrap().unwrap();
        assert_eq!(
            msg.quoted_message_id.as_deref(),
            Some("A55836D391A4D69FAE65489C8D04C52D")
        );
        assert_eq!(
            msg.quoted_message_text.as_deref(),
            Some("original task text")
        );
    }

    #[test]
    fn quote_id_passthrough_when_already_raw() {
        let a = adapter();
        let reply_to = serde_json::json!({
            "id": "3EB0EE5AB4F5577CF92858",
            "body": "already raw"
        });
        let payload = waha_payload(
            "message.any",
            "120363000000000001@g.us",
            Some("10000000002@c.us"),
            "done",
            false,
            vec![],
            Some(reply_to),
        );
        let msg = a.parse_webhook(&payload).unwrap().unwrap();
        assert_eq!(
            msg.quoted_message_id.as_deref(),
            Some("3EB0EE5AB4F5577CF92858")
        );
    }

    // --- build_send_request ---

    #[test]
    fn build_send_request_without_reply_to() {
        let a = adapter();
        let msg = OutboundMessage::text("Hello from Grumps".into());
        let (url, body) = a
            .build_send_request("120363000000000001@g.us", &msg)
            .unwrap();
        assert_eq!(url, "http://localhost:3000/api/sendText");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            parsed,
            serde_json::json!({
                "session": "default",
                "chatId": "120363000000000001@g.us",
                "text": "Hello from Grumps"
            })
        );
    }

    #[test]
    fn build_send_request_with_reply_to() {
        let a = adapter();
        let msg = OutboundMessage {
            text: "Done.".into(),
            reply_to: Some("RAWID123".into()),
            todo_id: None,
        };
        let (_, body) = a.build_send_request("10000000002@c.us", &msg).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            parsed,
            serde_json::json!({
                "session": "default",
                "chatId": "10000000002@c.us",
                "text": "Done.",
                "reply_to": "RAWID123"
            })
        );
    }

    #[test]
    fn new_trims_trailing_slash_from_base_url() {
        let a = WahaAdapter::new(
            "http://localhost:3000/".into(),
            "default".into(),
            "key".into(),
            "hmac".into(),
        );
        let msg = OutboundMessage::text("hi".into());
        let (url, _) = a.build_send_request("x", &msg).unwrap();
        assert_eq!(url, "http://localhost:3000/api/sendText");
    }

    // --- misc trait surface ---

    #[test]
    fn verification_challenge_always_errs() {
        let a = adapter();
        assert!(a.handle_verification_challenge(&HashMap::new()).is_err());
    }

    #[test]
    fn platform_id_is_whatsapp() {
        assert_eq!(adapter().platform_id(), "whatsapp");
    }
}
