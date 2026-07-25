use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Fact,
    Person,
    Decision,
    Preference,
    Place,
    #[default]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MemorySource {
    ChatExplicit,
    ChatAuto,
    #[default]
    Web,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub key: Option<String>,
    pub value: String,
    pub kind: MemoryKind,
    pub related_member: Option<String>,
    pub tags: Vec<String>,
    pub source: MemorySource,
    pub confidence: f64,
    pub pinned: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NewMemoryEntry {
    pub key: Option<String>,
    pub value: String,
    pub kind: MemoryKind,
    pub related_member: Option<String>,
    pub tags: Vec<String>,
    pub source: MemorySource,
    pub confidence: Option<f64>,
    pub pinned: Option<bool>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_serializes_snake_case() {
        let json = serde_json::to_string(&MemoryKind::Fact).unwrap();
        assert_eq!(json, r#""fact""#);
        let json = serde_json::to_string(&MemoryKind::Other).unwrap();
        assert_eq!(json, r#""other""#);
    }

    #[test]
    fn source_serializes_kebab_case() {
        let json = serde_json::to_string(&MemorySource::ChatExplicit).unwrap();
        assert_eq!(json, r#""chat-explicit""#);
        let json = serde_json::to_string(&MemorySource::ChatAuto).unwrap();
        assert_eq!(json, r#""chat-auto""#);
    }

    #[test]
    fn entry_roundtrips_json() {
        let now = Utc::now();
        let e = MemoryEntry {
            id: "m1".into(),
            key: Some("wifi".into()),
            value: "abc".into(),
            kind: MemoryKind::Fact,
            related_member: None,
            tags: vec!["home".into()],
            source: MemorySource::ChatExplicit,
            confidence: 1.0,
            pinned: true,
            expires_at: None,
            created_by: Some("u1".into()),
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.value, "abc");
        assert!(back.pinned);
    }
}
