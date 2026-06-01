use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TelegramWidgetPayload {
    pub id: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub photo_url: Option<String>,
    pub auth_date: i64,
    pub hash: String,
    #[serde(default)]
    pub dev_bypass: Option<bool>,
}

impl TelegramWidgetPayload {
    /// Display name: "first last" trim → username → "telegram:<id>".
    pub fn display_name(&self) -> String {
        let joined = format!(
            "{} {}",
            self.first_name.as_deref().unwrap_or(""),
            self.last_name.as_deref().unwrap_or("")
        );
        let trimmed = joined.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
        if let Some(u) = &self.username {
            if !u.is_empty() {
                return u.clone();
            }
        }
        format!("telegram:{}", self.id)
    }
}

/// Build data_check_string per https://core.telegram.org/widgets/login#checking-authorization
fn data_check_string(p: &TelegramWidgetPayload) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(6);
    parts.push(format!("auth_date={}", p.auth_date));
    if let Some(v) = &p.first_name {
        if !v.is_empty() {
            parts.push(format!("first_name={}", v));
        }
    }
    parts.push(format!("id={}", p.id));
    if let Some(v) = &p.last_name {
        if !v.is_empty() {
            parts.push(format!("last_name={}", v));
        }
    }
    if let Some(v) = &p.photo_url {
        if !v.is_empty() {
            parts.push(format!("photo_url={}", v));
        }
    }
    if let Some(v) = &p.username {
        if !v.is_empty() {
            parts.push(format!("username={}", v));
        }
    }
    parts.sort();
    parts.join("\n")
}

/// Verify that the Widget payload is signed by the owning bot.
/// Returns true iff HMAC matches.
pub fn verify_widget_hash(payload: &TelegramWidgetPayload, bot_token: &str) -> bool {
    let data = data_check_string(payload);
    let secret_key = {
        let mut h = Sha256::new();
        h.update(bot_token.as_bytes());
        h.finalize()
    };
    let mut mac = match Hmac::<Sha256>::new_from_slice(&secret_key) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(data.as_bytes());
    let tag = mac.finalize().into_bytes();
    let expected = hex::encode(tag);
    constant_time_eq(expected.as_bytes(), payload.hash.as_bytes())
}

pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut r = 0u8;
    for i in 0..a.len() {
        r |= a[i] ^ b[i];
    }
    r == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compute_hash(bot_token: &str, payload: &mut TelegramWidgetPayload) {
        let data = data_check_string(payload);
        let secret_key = {
            let mut h = Sha256::new();
            h.update(bot_token.as_bytes());
            h.finalize()
        };
        let mut mac = Hmac::<Sha256>::new_from_slice(&secret_key).unwrap();
        mac.update(data.as_bytes());
        payload.hash = hex::encode(mac.finalize().into_bytes());
    }

    fn sample_payload() -> TelegramWidgetPayload {
        TelegramWidgetPayload {
            id: 1234567890,
            first_name: Some("Test".into()),
            last_name: Some("User".into()),
            username: Some("testuser".into()),
            photo_url: None,
            auth_date: 1714000000,
            hash: String::new(),
            dev_bypass: None,
        }
    }

    #[test]
    fn valid_payload_verifies() {
        let token = "1234:FAKETESTTOKEN";
        let mut p = sample_payload();
        compute_hash(token, &mut p);
        assert!(verify_widget_hash(&p, token));
    }

    #[test]
    fn tampered_id_fails() {
        let token = "1234:FAKETESTTOKEN";
        let mut p = sample_payload();
        compute_hash(token, &mut p);
        p.id += 1;
        assert!(!verify_widget_hash(&p, token));
    }

    #[test]
    fn wrong_token_fails() {
        let mut p = sample_payload();
        compute_hash("1234:FAKETESTTOKEN", &mut p);
        assert!(!verify_widget_hash(&p, "9999:OTHER"));
    }

    #[test]
    fn utf8_first_name_ok() {
        let token = "1234:FAKETESTTOKEN";
        let mut p = sample_payload();
        p.first_name = Some("François".into());
        compute_hash(token, &mut p);
        assert!(verify_widget_hash(&p, token));
    }

    #[test]
    fn empty_optional_fields_excluded() {
        let mut p = sample_payload();
        p.last_name = Some(String::new());
        p.photo_url = Some(String::new());
        let data = data_check_string(&p);
        assert!(!data.contains("last_name="));
        assert!(!data.contains("photo_url="));
    }

    #[test]
    fn display_name_prefers_first_last() {
        let p = sample_payload();
        assert_eq!(p.display_name(), "Test User");
    }

    #[test]
    fn display_name_falls_back_to_username() {
        let mut p = sample_payload();
        p.first_name = None;
        p.last_name = None;
        assert_eq!(p.display_name(), "testuser");
    }

    #[test]
    fn display_name_falls_back_to_telegram_id() {
        let mut p = sample_payload();
        p.first_name = None;
        p.last_name = None;
        p.username = None;
        assert_eq!(p.display_name(), "telegram:1234567890");
    }

    // constant_time_eq also backs the /internal/migrate-workspaces secret gate.
    #[test]
    fn constant_time_eq_matches_and_rejects() {
        assert!(constant_time_eq(b"s3cr3t", b"s3cr3t"));
        assert!(!constant_time_eq(b"s3cr3t", b"S3cr3t"));
        assert!(!constant_time_eq(b"s3cr3t", b"s3cr3t-longer"));
        // An empty provided secret must never match a real one.
        assert!(!constant_time_eq(b"", b"s3cr3t"));
    }
}
