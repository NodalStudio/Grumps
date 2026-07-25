use crate::i18n::tr;
use gloo_net::http::Request;
use leptos::prelude::*;
use leptos_router::hooks::use_query_map;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[component]
pub fn LoginPage() -> impl IntoView {
    let query = use_query_map();
    let redirect = Signal::derive(move || {
        query
            .read()
            .get("redirect")
            .unwrap_or_else(|| "/dashboard".to_string())
    });
    // Set by `/auth/link`'s uniform-failure redirect (invalid, expired, or
    // already-redeemed magic-link token) — see routes/auth.rs::handle_link_redeem.
    // Any non-empty `error` value is treated the same way; the worker only
    // ever sends `link_invalid`, but an unrecognized value still shows the
    // generic line rather than nothing.
    let link_error = Signal::derive(move || {
        query
            .read()
            .get("error")
            .filter(|e| !e.is_empty())
            .is_some()
    });

    Effect::new(move |_| {
        let redirect_to = redirect.get();
        install_tg_callback(redirect_to);
    });

    view! {
        <div class="min-h-screen flex items-center justify-center" style="background: var(--cream);">
            <div class="w-96 border-2 border-ink rounded-xs" style="background: var(--cream-light);">
                <div class="px-8 pt-8 pb-6 text-center border-b-2 border-ink">
                    <h1 class="font-display text-3xl font-extrabold uppercase tracking-tight">
                        "GRUMPS"<span class="text-brick">"."</span>
                    </h1>
                    <p class="text-xs uppercase tracking-widest mt-1" style="color: var(--ink-40);">
                        {move || tr("brand.tagline")}
                    </p>
                </div>

                <div class="px-8 py-7 space-y-3">
                    <Show when=move || link_error.get()>
                        <p
                            class="text-xs text-center p-2 border-2 border-brick"
                            style="color: var(--brick); line-height: 1.5;"
                        >
                            {move || tr("auth.error.link_invalid")}
                        </p>
                    </Show>

                    <div id="tg-widget-container" class="flex justify-center"></div>

                    <div class="p-3 border-2 border-ink" style="background: var(--cream);">
                        <p class="text-xs uppercase tracking-widest font-bold">
                            {move || tr("login.wa_hint_title")}
                        </p>
                        <p class="text-xs mt-1" style="color: var(--ink-40); line-height: 1.5;">
                            {move || tr("login.wa_hint_body")}
                        </p>
                    </div>

                    <p class="text-center text-xs mt-6 pt-6 border-t-2 border-ink" style="color: var(--ink-40); line-height: 1.5;">
                        {move || tr("login.footer")}
                    </p>
                </div>
            </div>
        </div>
    }
}

fn install_tg_callback(redirect_to: String) {
    let cb = Closure::wrap(Box::new(move |user: JsValue| {
        let redirect_to = redirect_to.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = verify_tg_widget(user, &redirect_to).await {
                web_sys::console::error_1(&format!("TG login failed: {}", e).into());
            }
        });
    }) as Box<dyn FnMut(JsValue)>);
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let _ = js_sys::Reflect::set(
        &window,
        &"__grumpsTgAuth".into(),
        cb.as_ref().unchecked_ref(),
    );
    cb.forget();

    if let Some(doc) = window.document() {
        if let Some(container) = doc.get_element_by_id("tg-widget-container") {
            container.set_inner_html("");
            if let Ok(script) = doc.create_element("script") {
                let _ = script.set_attribute("async", "");
                let _ =
                    script.set_attribute("src", "https://telegram.org/js/telegram-widget.js?22");
                let _ = script.set_attribute("data-telegram-login", "HeyGrumpsBot");
                let _ = script.set_attribute("data-size", "large");
                let _ = script.set_attribute("data-radius", "2");
                let _ = script.set_attribute("data-onauth", "window.__grumpsTgAuth(user)");
                let _ = script.set_attribute("data-request-access", "write");
                let _ = container.append_child(&script);
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct VerifyBody {
    id: i64,
    first_name: Option<String>,
    last_name: Option<String>,
    username: Option<String>,
    photo_url: Option<String>,
    auth_date: i64,
    hash: String,
}

#[derive(Deserialize)]
struct VerifyResponseDto {
    user_id: String,
    display_name: String,
}

async fn verify_tg_widget(user: JsValue, redirect_to: &str) -> Result<(), String> {
    let body: VerifyBody = serde_wasm_bindgen::from_value(user).map_err(|e| e.to_string())?;
    let base = crate::api::api_base();
    let resp = Request::post(&format!("{}/auth/telegram/verify", base))
        .credentials(web_sys::RequestCredentials::Include)
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.status() != 200 {
        return Err(format!("status {}", resp.status()));
    }
    let _: VerifyResponseDto = resp.json().await.map_err(|e| e.to_string())?;

    // Hard navigation so AuthGate re-runs and reads cookies fresh.
    if let Some(win) = web_sys::window() {
        let _ = win.location().set_href(redirect_to);
    }
    Ok(())
}
