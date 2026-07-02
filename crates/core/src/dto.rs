//! Shared request DTOs — the wire contract between the SPA and the worker.
//!
//! Both crates depend on `grumps-core`, so these structs are the single source
//! of truth for request bodies: the SPA serializes them, the worker
//! deserializes and validates them. `slug` and other path parameters are NOT
//! part of these DTOs — they travel in the URL, not the body.
//!
//! Validation rules live behind the `validation` feature so the `validator`
//! crate only compiles into the worker, never into the SPA WASM bundle. The
//! `#[cfg_attr]` gates both the `derive(Validate)` and the per-field
//! `validate(...)` attributes; with neither the feature nor `cfg(test)` set,
//! neither exists. (`cfg(test)` is included so the validation rules are
//! exercised by `cargo test` without `--features validation`, via the
//! `validator` dev-dependency.)
//!
//! Strings that must be non-empty are trimmed at deserialization
//! (`deserialize_with = "de_trim"`), so the validated value equals the stored
//! value: `length(min = 1)` then correctly rejects whitespace-only input, and
//! handlers don't need to trim again before persisting.
//!
//! Error codes (`todo.title_invalid`, …) are i18n keys, not English prose —
//! the SPA renders them via `tr()`.

use serde::Deserialize;

#[cfg(any(feature = "validation", test))]
use validator::Validate;

/// Trim surrounding whitespace at deserialize time. Makes the persisted value
/// equal the validated value, so a whitespace-only input collapses to `""` and
/// fails `length(min = 1)` instead of slipping through as a blank string.
pub fn de_trim<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(String::deserialize(d)?.trim().to_string())
}

/// `de_trim` for optional fields — `None` stays `None`, `Some` is trimmed.
pub fn de_trim_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(d)?.map(|s| s.trim().to_string()))
}

/// Body of `POST /api/w/:slug/todos`.
#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#[cfg_attr(any(feature = "validation", test), derive(Validate))]
pub struct CreateTodoRequest {
    #[serde(deserialize_with = "de_trim")]
    #[cfg_attr(
        any(feature = "validation", test),
        validate(length(min = 1, max = 500, code = "todo.title_invalid"))
    )]
    pub title: String,

    #[cfg_attr(
        any(feature = "validation", test),
        validate(range(min = 1, max = 3, code = "todo.priority_invalid"))
    )]
    pub priority: Option<i32>,

    pub tags: Option<Vec<String>>,
    pub assigned_to: Option<String>,
    pub assigned_name: Option<String>,
    pub deadline: Option<String>,
}

/// Body of `PATCH /api/w/:slug/todos/:id`. Every field is optional — only the
/// present ones are updated. `status` is checked in the handler against the
/// closed set (`open|in_progress|done`) since it is a domain enum, not a shape
/// constraint.
#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#[cfg_attr(any(feature = "validation", test), derive(Validate))]
pub struct UpdateTodoRequest {
    #[serde(default, deserialize_with = "de_trim_opt")]
    #[cfg_attr(
        any(feature = "validation", test),
        validate(length(min = 1, max = 500, code = "todo.title_invalid"))
    )]
    pub title: Option<String>,

    pub status: Option<String>,

    #[cfg_attr(
        any(feature = "validation", test),
        validate(range(min = 1, max = 3, code = "todo.priority_invalid"))
    )]
    pub priority: Option<i32>,

    pub tags: Option<Vec<String>>,
    pub assigned_to: Option<String>,
    pub assigned_name: Option<String>,
}

/// Body of `POST /api/w/:slug/notes`.
#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#[cfg_attr(any(feature = "validation", test), derive(Validate))]
pub struct CreateNoteRequest {
    pub title: Option<String>,
    #[serde(deserialize_with = "de_trim")]
    #[cfg_attr(
        any(feature = "validation", test),
        validate(length(min = 1, code = "note.content_required"))
    )]
    pub content: String,
}

/// Body of `PUT /api/w/:slug/notes/:id`.
#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#[cfg_attr(any(feature = "validation", test), derive(Validate))]
pub struct UpdateNoteRequest {
    pub title: Option<String>,
    #[serde(deserialize_with = "de_trim")]
    #[cfg_attr(
        any(feature = "validation", test),
        validate(length(min = 1, code = "note.content_required"))
    )]
    pub content: String,
}

/// Response of `GET /api/w/:slug/notes/:id/links`: backlink target ref.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct LinkRef {
    pub id: String,
    pub title: Option<String>,
}

/// One outgoing wikilink edge, resolved (or not) to a note id.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OutgoingLink {
    pub display: String,
    pub id: Option<String>,
}

/// Response of `GET /api/w/:slug/notes/:id/links`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct NoteLinksResponse {
    pub backlinks: Vec<LinkRef>,
    pub outgoing: Vec<OutgoingLink>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn create_accepts_valid() {
        let r = CreateTodoRequest {
            title: "buy milk".into(),
            priority: Some(2),
            ..Default::default()
        };
        assert!(r.validate().is_ok());
    }

    #[test]
    fn create_rejects_empty_title() {
        let r = CreateTodoRequest {
            title: String::new(),
            ..Default::default()
        };
        let err = r.validate().unwrap_err();
        assert!(err.field_errors().contains_key("title"));
    }

    #[test]
    fn create_rejects_out_of_range_priority() {
        let r = CreateTodoRequest {
            title: "ok".into(),
            priority: Some(9),
            ..Default::default()
        };
        let err = r.validate().unwrap_err();
        assert!(err.field_errors().contains_key("priority"));
    }

    #[test]
    fn create_skips_priority_when_none() {
        let r = CreateTodoRequest {
            title: "ok".into(),
            priority: None,
            ..Default::default()
        };
        assert!(r.validate().is_ok());
    }

    #[test]
    fn update_all_none_is_valid() {
        assert!(UpdateTodoRequest::default().validate().is_ok());
    }

    #[test]
    fn update_rejects_long_title() {
        let r = UpdateTodoRequest {
            title: Some("x".repeat(501)),
            ..Default::default()
        };
        assert!(r.validate().is_err());
    }

    // The trim fix: a whitespace-only title arrives over the wire, is trimmed
    // to "" at deserialize, and then fails `length(min = 1)` — instead of
    // passing validation and being persisted as a blank string.
    #[test]
    fn create_rejects_whitespace_only_title_after_trim() {
        let r: CreateTodoRequest = serde_json::from_str(r#"{"title":"   "}"#).unwrap();
        assert_eq!(r.title, "");
        assert!(r.validate().is_err());
    }

    #[test]
    fn create_trims_surrounding_whitespace() {
        let r: CreateTodoRequest = serde_json::from_str(r#"{"title":"  buy milk  "}"#).unwrap();
        assert_eq!(r.title, "buy milk");
        assert!(r.validate().is_ok());
    }

    #[test]
    fn note_links_response_serializes() {
        let r = super::NoteLinksResponse {
            backlinks: vec![super::LinkRef {
                id: "n1".into(),
                title: Some("Wifi".into()),
            }],
            outgoing: vec![
                super::OutgoingLink {
                    display: "Wifi".into(),
                    id: Some("n1".into()),
                },
                super::OutgoingLink {
                    display: "Ghost".into(),
                    id: None,
                },
            ],
        };
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["backlinks"][0]["id"], "n1");
        assert_eq!(j["outgoing"][1]["id"], serde_json::Value::Null);
    }
}
