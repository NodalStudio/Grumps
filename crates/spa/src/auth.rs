use leptos::prelude::*;
use crate::api::{ApiClient, VerifyResponse, WorkspaceInfo};

/// Auth state provided via Leptos context.
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

        Self { token, set_token, user_id, set_user_id, workspaces, set_workspaces, api }
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

/// Provide auth context at app root. Call this in App component.
///
/// In demo mode (`/demo/...` or `?demo=1`) the OTP gate is skipped:
/// the auth state is pre-populated with a sentinel token + the seed
/// workspace so the iframe lands on content immediately.
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

/// Use auth context from any child component.
pub fn use_auth() -> AuthState {
    expect_context::<AuthState>()
}
