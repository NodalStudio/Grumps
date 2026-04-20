use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    Admin,
    Member,
}

impl Role {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    pub fn from_str(s: &str) -> Self {
        if s == "admin" {
            Self::Admin
        } else {
            Self::Member
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub platform_user_id: String,
    pub display_name: Option<String>,
    pub role: Role,
    /// BCP-47 locale code. `"en"` is the "unknown yet, treat as English"
    /// default — overwritten on the first message the Gemini classifier
    /// processes (it returns a detected language as a free by-product).
    /// See migration `0009_member_locale.sql`.
    #[serde(default = "default_locale")]
    pub locale: String,
}

fn default_locale() -> String { "en".to_string() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_roundtrip() {
        assert_eq!(Role::from_str(Role::Admin.as_str()), Role::Admin);
        assert_eq!(Role::from_str(Role::Member.as_str()), Role::Member);
    }

    #[test]
    fn role_from_str_unknown_defaults_to_member() {
        assert_eq!(Role::from_str("unknown"), Role::Member);
        assert_eq!(Role::from_str(""), Role::Member);
        assert_eq!(Role::from_str("ADMIN"), Role::Member);
    }

    #[test]
    fn member_serialization_roundtrip() {
        let m = Member {
            id: "m-1".to_string(),
            platform_user_id: "U12345".to_string(),
            display_name: Some("Alice".to_string()),
            role: Role::Admin,
            locale: "fr".to_string(),
        };
        let json = serde_json::to_string(&m).expect("serialize");
        let back: Member = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, m.id);
        assert_eq!(back.role, Role::Admin);
        assert_eq!(back.locale, "fr");
    }

    #[test]
    fn member_deserialize_missing_locale_defaults_to_en() {
        let json = r#"{"id":"m-1","platform_user_id":"U12345","display_name":"Bob","role":"Member"}"#;
        let m: Member = serde_json::from_str(json).expect("deserialize");
        assert_eq!(m.locale, "en");
    }
}
