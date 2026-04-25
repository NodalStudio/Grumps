pub mod gate;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;

use crate::api::{ApiClient, VerifyResponse, WorkspaceInfo};

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

// ─────────────────────────────────────────────────────────────────────────────
// Backwards-compat shims
//
// Existing pages expect `AuthState` with token/workspaces signals + an
// `ApiClient`. Task 22 will refactor those callsites to use `SessionContext`
// directly. Until then, keep the old API surface intact so the SPA compiles.
// ─────────────────────────────────────────────────────────────────────────────

/// Legacy auth state — preserved so existing pages keep compiling. The
/// underlying request layer no longer reads `token` (cookies + CSRF carry
/// auth), but the signal is kept for ABI compatibility.
#[derive(Clone)]
pub struct AuthState {
    pub token: ReadSignal<Option<String>>,
    pub set_token: WriteSignal<Option<String>>,
    pub user_id: ReadSignal<Option<String>>,
    pub set_user_id: WriteSignal<Option<String>>,
    pub workspaces: ReadSignal<Vec<WorkspaceInfo>>,
    pub set_workspaces: WriteSignal<Vec<WorkspaceInfo>>,
    pub api: ApiClient,
}

impl AuthState {
    pub fn new() -> Self {
        let (token, set_token) = signal(None::<String>);
        let (user_id, set_user_id) = signal(None::<String>);
        let (workspaces, set_workspaces) = signal(Vec::<WorkspaceInfo>::new());
        let api = ApiClient::new(token);
        Self {
            token,
            set_token,
            user_id,
            set_user_id,
            workspaces,
            set_workspaces,
            api,
        }
    }

    pub fn is_logged_in(&self) -> bool {
        self.token.get().is_some()
    }

    pub fn login(&self, response: VerifyResponse) {
        self.set_token.set(Some(response.token));
        self.set_user_id.set(Some(response.user_id));
        self.set_workspaces.set(response.workspaces);
    }

    pub fn logout(&self) {
        self.set_token.set(None);
        self.set_user_id.set(None);
        self.set_workspaces.set(vec![]);
    }
}

/// Provide auth context at app root.
///
/// In demo mode the auth state is pre-populated so the iframe lands on
/// content immediately. In real mode this is now a no-op placeholder —
/// `<AuthGate>` (Task 22) is responsible for populating session state.
pub fn provide_auth() -> AuthState {
    let auth = AuthState::new();
    if crate::demo::is_demo() {
        auth.set_token.set(Some(crate::demo::DEMO_TOKEN.to_string()));
        auth.set_user_id.set(Some(crate::demo::DEMO_MEMBER_ID.to_string()));
        auth.set_workspaces.set(crate::demo::workspaces());
    }
    provide_context(auth.clone());
    auth
}

pub fn use_auth() -> AuthState {
    expect_context::<AuthState>()
}
