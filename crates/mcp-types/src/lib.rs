//! Transport-free data types for the **Model Context Protocol** (MCP).
//!
//! This crate reproduces the MCP *model* — the wire types a tool provider and
//! an LLM exchange (`Tool` descriptors, `ToolAnnotations`, `CallToolResult`,
//! `Content`, …) — plus optional JSON-Schema generation from Rust types. It is
//! deliberately **transport-free**: no JSON-RPC framing, no networking, no
//! async runtime. That keeps it dependency-light and lets it compile to
//! `wasm32` (e.g. Cloudflare Workers), where the official `rmcp` SDK does not,
//! because `rmcp`'s schema generation sits behind a `server` feature that pulls
//! in tokio.
//!
//! All structs are wire-compatible with the MCP specification: the same field
//! names, `camelCase` renaming, `skip_serializing_if` on optionals, and the
//! `_meta` key. Unlike `rmcp` these types are **not** `#[non_exhaustive]` — this
//! is the source crate, so callers may build them with struct literals or the
//! provided builders.
//!
//! With the `schemars` feature, [`schema_for_type`] and the
//! [`Tool::with_input_schema`] / [`Tool::with_output_schema`] builders generate
//! an MCP `inputSchema`/`outputSchema` directly from a Rust type that derives
//! [`schemars::JsonSchema`], so a tool's parameter schema and its argument
//! struct never drift apart.

use std::borrow::Cow;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A JSON object, as used for schemas and metadata bags.
pub type JsonObject = serde_json::Map<String, Value>;

/// Description of a tool the model may call.
///
/// Mirrors the MCP `Tool` shape. `input_schema` holds a JSON Schema describing
/// the tool's **call arguments** (not the fields of this struct).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Tool {
    /// The programmatic name of the tool.
    pub name: Cow<'static, str>,
    /// A human-readable title for the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A description of what the tool does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
    /// JSON Schema describing the tool's expected parameters.
    pub input_schema: Arc<JsonObject>,
    /// Optional JSON Schema describing the structure of the tool's output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Arc<JsonObject>>,
    /// Optional behavioural hints (read-only, destructive, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
    /// Execution-related configuration including task support mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<ToolExecution>,
    /// Optional list of icons for the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<Icon>>,
    /// Optional additional metadata for this tool.
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

impl Tool {
    /// Construct a tool from a name, description and an already-built input schema.
    pub fn new<N, D, S>(name: N, description: D, input_schema: S) -> Self
    where
        N: Into<Cow<'static, str>>,
        D: Into<Cow<'static, str>>,
        S: Into<Arc<JsonObject>>,
    {
        Tool {
            name: name.into(),
            title: None,
            description: Some(description.into()),
            input_schema: input_schema.into(),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        }
    }

    /// Attach behavioural annotations (builder style).
    pub fn with_annotations(mut self, annotations: ToolAnnotations) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Generate the `input_schema` from a Rust type `T` (builder style).
    ///
    /// `T` is supplied as a type parameter — there is no value argument; the
    /// schema is derived from the type's shape via [`schemars`].
    #[cfg(feature = "schemars")]
    pub fn with_input_schema<T: schemars::JsonSchema + 'static>(mut self) -> Self {
        self.input_schema = schema_for_type::<T>();
        self
    }

    /// Generate the `output_schema` from a Rust type `T` (builder style).
    #[cfg(feature = "schemars")]
    pub fn with_output_schema<T: schemars::JsonSchema + 'static>(mut self) -> Self {
        self.output_schema = Some(schema_for_type::<T>());
        self
    }
}

/// Behavioural hints about a tool. All are advisory and optional.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ToolAnnotations {
    /// A human-readable title for the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// If true, the tool does not modify its environment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    /// If true, the tool may perform destructive (non-additive) updates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    /// If true, repeated calls with the same arguments have no additional effect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,
    /// If true, the tool may interact with an open, unbounded world (e.g. the web).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
}

impl ToolAnnotations {
    /// A read-only tool (no environment mutation).
    pub fn read_only() -> Self {
        ToolAnnotations { read_only_hint: Some(true), ..Default::default() }
    }
}

/// Execution-related configuration for a tool.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ToolExecution {
    /// Whether the tool supports running as a long-lived task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_support: Option<TaskSupport>,
}

/// Whether a tool supports task-style (long-running) execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum TaskSupport {
    /// Task execution is not supported.
    #[default]
    Forbidden,
    /// Task execution is supported but not required.
    Optional,
    /// Task execution is required.
    Required,
}

/// An icon associated with a tool, following the MCP icon shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Icon {
    /// A URI (or data URI) pointing at the icon.
    pub src: String,
    /// Optional MIME type, e.g. `image/png`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Optional size descriptor, e.g. `48x48`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizes: Option<String>,
}

/// A bag of protocol-level metadata, serialized under the `_meta` key.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Meta(pub JsonObject);

/// Parameters of a `tools/call` request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CallToolRequestParam {
    /// The name of the tool to call.
    pub name: Cow<'static, str>,
    /// The arguments to pass, matching the tool's `input_schema`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<JsonObject>,
}

/// The result of a `tools/call`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CallToolResult {
    /// The content returned by the tool (text, images, …).
    pub content: Vec<Content>,
    /// Optional structured (JSON) result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
    /// Whether this result represents an error condition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    /// Optional protocol-level metadata.
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

impl CallToolResult {
    /// A successful result containing the given content blocks.
    pub fn success(content: Vec<Content>) -> Self {
        CallToolResult { content, is_error: Some(false), ..Default::default() }
    }

    /// An error result containing the given content blocks.
    pub fn error(content: Vec<Content>) -> Self {
        CallToolResult { content, is_error: Some(true), ..Default::default() }
    }
}

/// A single content block in a tool result.
///
/// Only the `text` variant is modelled today (the form Grumps feeds back into
/// the LLM loop); `image`/`resource` variants can be added without breaking the
/// `{"type": ...}` tagging.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum Content {
    /// A plain-text block.
    Text {
        /// The text payload.
        text: String,
    },
}

impl Content {
    /// Construct a text content block.
    pub fn text(text: impl Into<String>) -> Self {
        Content::Text { text: text.into() }
    }
}

/// Generate (and cache) the MCP `inputSchema` for a Rust type `T`.
///
/// The schema is produced from `T`'s shape via [`schemars`] using the JSON
/// Schema 2020-12 dialect (the version MCP aligns to), serialized to a
/// [`JsonObject`], and memoised per `TypeId` so repeated `tools/list` builds are
/// cheap. `T: 'static` is required because the cache keys on `TypeId::of::<T>()`.
#[cfg(feature = "schemars")]
pub fn schema_for_type<T: schemars::JsonSchema + 'static>() -> Arc<JsonObject> {
    use std::any::TypeId;
    use std::collections::HashMap;
    use std::sync::RwLock;

    thread_local! {
        static CACHE_FOR_TYPE: RwLock<HashMap<TypeId, Arc<JsonObject>>> =
            RwLock::new(HashMap::new());
    }

    CACHE_FOR_TYPE.with(|cache| {
        if let Some(schema) = cache
            .read()
            .expect("schema cache lock poisoned")
            .get(&TypeId::of::<T>())
        {
            return schema.clone();
        }
        // Align to the official MCP JSON Schema dialect (2020-12). `nullable`
        // (an OpenAPI 3.0 extension) is intentionally not used so strict
        // validators accept the output. `inline_subschemas` keeps the result
        // self-contained — enum/struct fields are inlined rather than emitted as
        // `$ref` into `$defs`, which is what LLM tool-use APIs (Anthropic,
        // OpenAI) expect for an `input_schema`. (Safe for non-recursive types.)
        let mut settings = schemars::generate::SchemaSettings::draft2020_12();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let schema = generator.into_root_schema_for::<T>();
        let object = match serde_json::to_value(schema).expect("schema serializes") {
            Value::Object(object) => object,
            other => panic!("schema serialization produced a non-object value: {other:?}"),
        };
        let schema = Arc::new(object);
        cache
            .write()
            .expect("schema cache lock poisoned")
            .insert(TypeId::of::<T>(), schema.clone());
        schema
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_serializes_with_camelcase_and_meta() {
        let mut input = JsonObject::new();
        input.insert("type".into(), Value::String("object".into()));
        let tool = Tool::new("create_todo", "Create a todo", Arc::new(input))
            .with_annotations(ToolAnnotations {
                read_only_hint: Some(false),
                destructive_hint: Some(false),
                ..Default::default()
            });
        let v = serde_json::to_value(&tool).unwrap();
        assert_eq!(v["name"], "create_todo");
        assert_eq!(v["inputSchema"]["type"], "object");
        // camelCase rename on annotation hints.
        assert_eq!(v["annotations"]["readOnlyHint"], false);
        // Unset optionals are skipped, not null.
        assert!(v.get("outputSchema").is_none());
        assert!(v.get("_meta").is_none());
        assert!(v["annotations"].get("idempotentHint").is_none());
    }

    #[test]
    fn tool_round_trips() {
        let tool = Tool::new("x", "y", Arc::new(JsonObject::new()));
        let s = serde_json::to_string(&tool).unwrap();
        let back: Tool = serde_json::from_str(&s).unwrap();
        assert_eq!(tool, back);
    }

    #[test]
    fn call_tool_result_text() {
        let r = CallToolResult::success(vec![Content::text("done")]);
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["content"][0]["type"], "text");
        assert_eq!(v["content"][0]["text"], "done");
        assert_eq!(v["isError"], false);
    }

    #[test]
    fn task_support_lowercase() {
        assert_eq!(serde_json::to_value(TaskSupport::Forbidden).unwrap(), "forbidden");
    }

    #[cfg(feature = "schemars")]
    #[test]
    fn schema_for_type_produces_object_and_caches() {
        #[derive(schemars::JsonSchema)]
        #[allow(dead_code)]
        struct CreateTodoArgs {
            title: String,
            deadline: Option<String>,
        }

        let a = schema_for_type::<CreateTodoArgs>();
        assert_eq!(a.get("type").and_then(|v| v.as_str()), Some("object"));
        assert!(a.get("properties").and_then(|p| p.get("title")).is_some());
        let required = a.get("required").and_then(|v| v.as_array()).unwrap();
        assert!(required.iter().any(|v| v == "title"));
        // `Option<String>` field is not required.
        assert!(!required.iter().any(|v| v == "deadline"));

        // Second call returns the same cached Arc.
        let b = schema_for_type::<CreateTodoArgs>();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[cfg(feature = "schemars")]
    #[test]
    fn schema_inlines_enums_without_defs() {
        #[derive(schemars::JsonSchema)]
        #[allow(dead_code)]
        struct WithEnum {
            kind: Kind,
        }
        #[derive(schemars::JsonSchema)]
        #[schemars(rename_all = "snake_case")]
        #[allow(dead_code)]
        enum Kind { Fact, Person }

        let s = schema_for_type::<WithEnum>();
        // Self-contained: no `$defs`, and the enum is inlined on the field.
        assert!(s.get("$defs").is_none(), "should inline, not emit $defs: {s:?}");
        let kind = s.get("properties").and_then(|p| p.get("kind")).expect("kind property");
        let values = kind.get("enum").and_then(|e| e.as_array()).expect("inline enum values");
        assert!(values.iter().any(|v| v == "fact"));
        assert!(values.iter().any(|v| v == "person"));
    }
}
