use gloo_net::http::Request;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use super::{provide_session, SessionContext};

#[component]
pub fn AuthGate(children: ChildrenFn) -> impl IntoView {
    // Demo bypass — existing demo mode disables real auth.
    if crate::demo::is_demo() {
        provide_session(SessionContext::default());
        return children().into_any();
    }

    let session: RwSignal<Option<Result<SessionContext, ()>>> = RwSignal::new(None);

    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            match fetch_me().await {
                Ok(s) => session.set(Some(Ok(s))),
                Err(_) => session.set(Some(Err(()))),
            }
        });
    });

    let navigate = use_navigate();

    view! {
        {move || match session.get() {
            None => view! { <div class="auth-splash"><h1>"GRUMPS."</h1></div> }.into_any(),
            Some(Err(())) => {
                let nav = navigate.clone();
                let current = web_sys::window()
                    .and_then(|w| w.location().pathname().ok())
                    .unwrap_or_default();
                let target = format!("/login?redirect={}", urlencoding::encode(&current));
                nav(&target, Default::default());
                view! { <div class="auth-splash"><h1>"GRUMPS."</h1></div> }.into_any()
            }
            Some(Ok(s)) => {
                provide_session(s);
                children().into_any()
            }
        }}
    }
    .into_any()
}

async fn fetch_me() -> Result<SessionContext, String> {
    let base = crate::api::api_base();
    let resp = Request::get(&format!("{}/auth/me", base))
        .credentials(web_sys::RequestCredentials::Include)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.status() != 200 {
        return Err(format!("status {}", resp.status()));
    }
    resp.json::<SessionContext>()
        .await
        .map_err(|e| e.to_string())
}
