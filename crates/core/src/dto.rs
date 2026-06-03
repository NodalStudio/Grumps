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
//! `validate(...)` attributes; with the feature off neither exists.
//!
//! Error codes (`todo.title_invalid`, …) are i18n keys, not English prose —
//! the SPA renders them via `tr()`.

#[cfg(feature = "validation")]
use validator::Validate;

/// Body of `POST /api/w/:slug/todos`.
#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
#[cfg_attr(feature = "validation", derive(Validate))]
pub struct CreateTodoRequest {
    #[cfg_attr(
        feature = "validation",
        validate(length(min = 1, max = 500, code = "todo.title_invalid"))
    )]
    pub title: String,

    #[cfg_attr(
        feature = "validation",
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
#[cfg_attr(feature = "validation", derive(Validate))]
pub struct UpdateTodoRequest {
    #[cfg_attr(
        feature = "validation",
        validate(length(min = 1, max = 500, code = "todo.title_invalid"))
    )]
    pub title: Option<String>,

    pub status: Option<String>,

    #[cfg_attr(
        feature = "validation",
        validate(range(min = 1, max = 3, code = "todo.priority_invalid"))
    )]
    pub priority: Option<i32>,

    pub tags: Option<Vec<String>>,
    pub assigned_to: Option<String>,
    pub assigned_name: Option<String>,
}

#[cfg(all(test, feature = "validation"))]
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
}
