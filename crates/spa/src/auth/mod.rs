pub mod gate;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WorkspaceRef {
    pub slug: String,
    pub name: Option<String>,
    pub role: String,
    pub platform: String,
    #[serde(default)]
    pub is_dm: bool,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SessionContext {
    pub user_id: String,
    pub display_name: String,
    pub default_locale: Option<String>,
    pub workspaces: Vec<WorkspaceRef>,
    pub csrf_token: String,
}

pub fn provide_session(ctx: SessionContext) {
    provide_context(ctx);
}

pub fn use_session() -> Option<SessionContext> {
    use_context::<SessionContext>()
}

/// Read `grumps_csrf` cookie from the browser (non-HttpOnly). Returns empty string if missing.
pub fn read_csrf_cookie() -> String {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return String::new();
    };
    let Some(html) = doc.dyn_ref::<web_sys::HtmlDocument>() else {
        return String::new();
    };
    let cookies = html.cookie().unwrap_or_default();
    for part in cookies.split(';') {
        let kv = part.trim();
        if let Some(rest) = kv.strip_prefix("grumps_csrf=") {
            return rest.to_string();
        }
    }
    String::new()
}
