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
}

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
        };
        let json = serde_json::to_string(&m).expect("serialize");
        let back: Member = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, m.id);
        assert_eq!(back.role, Role::Admin);
    }
}
