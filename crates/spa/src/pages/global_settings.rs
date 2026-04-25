use leptos::prelude::*;
use gloo_net::http::Request;
use serde::Deserialize;
use crate::auth::{use_session, read_csrf_cookie};
use crate::i18n::tr;

#[component]
pub fn GlobalSettingsPage() -> impl IntoView {
    let session = use_session().unwrap_or_default();

    view! {
        <div class="min-h-screen p-8" style="background: var(--cream);">
            <h1 class="font-display text-3xl font-bold mb-6">{move || tr("settings.title")}</h1>

            <section class="max-w-2xl border-2 border-ink rounded-sm p-6 mb-6" style="background: var(--cream-light);">
                <h2 class="font-display text-lg font-bold mb-4">{move || tr("settings.account")}</h2>
                <AccountForm session=session.clone()/>
            </section>

            <section class="max-w-2xl border-2 border-ink rounded-sm p-6 mb-6" style="background: var(--cream-light);">
                <h2 class="font-display text-lg font-bold mb-4">{move || tr("settings.linked_accounts")}</h2>
                <LinkedAccounts/>
            </section>

            <section class="max-w-2xl border-2 border-ink rounded-sm p-6" style="background: var(--cream-light);">
                <h2 class="font-display text-lg font-bold mb-4">{move || tr("settings.sessions")}</h2>
                <SessionList/>
            </section>
        </div>
    }
}

#[component]
fn AccountForm(session: crate::auth::SessionContext) -> impl IntoView {
    let (name, set_name) = signal(session.display_name.clone());
    let initial_locale = session.default_locale.clone().unwrap_or_else(|| "en".into());
    let (locale, set_locale) = signal(initial_locale);

    let save = move |_| {
        let n = name.get();
        let l = locale.get();
        leptos::task::spawn_local(async move {
            let base = crate::api::api_base();
            if let Ok(rb) = Request::patch(&format!("{}/api/me", base))
                .credentials(web_sys::RequestCredentials::Include)
                .header("X-CSRF-Token", &read_csrf_cookie())
                .header("Content-Type", "application/json")
                .json(&serde_json::json!({ "display_name": n, "default_locale": l }))
            {
                let _ = rb.send().await;
            }
        });
    };

    view! {
        <label class="block mb-4">
            <span class="block text-xs font-semibold uppercase tracking-wider mb-1" style="color: var(--ink-70);">{move || tr("settings.display_name")}</span>
            <input type="text" class="w-full p-2 border-2 border-ink rounded-sm" style="background: var(--cream);"
                prop:value=name on:input=move |ev| set_name.set(event_target_value(&ev))/>
        </label>
        <label class="block mb-4">
            <span class="block text-xs font-semibold uppercase tracking-wider mb-1" style="color: var(--ink-70);">{move || tr("settings.default_locale")}</span>
            <select class="w-full p-2 border-2 border-ink rounded-sm" style="background: var(--cream);"
                on:change=move |ev| set_locale.set(event_target_value(&ev))>
                {["en","es","pt-BR","fr","de","it","ru","tr","ar","hi","zh-CN","ja","ko","id"].iter().map(|code| {
                    let code = *code;
                    view! { <option value=code selected=move || locale.get() == code>{code}</option> }
                }).collect_view()}
            </select>
        </label>
        <button class="px-4 py-2 text-sm font-bold uppercase tracking-wider border-2 border-ink rounded-sm cursor-pointer"
            style="background: var(--ink); color: var(--cream);"
            on:click=save>"Save"</button>
    }
}

#[derive(Deserialize, Default, Clone)]
struct SessionsResponse { sessions: Vec<SessionDto> }

#[derive(Deserialize, Clone)]
struct SessionDto {
    id: String,
    device_label: Option<String>,
    country_hint: Option<String>,
    created_at: String,
    last_seen_at: String,
    is_current: bool,
}

#[component]
fn SessionList() -> impl IntoView {
    let list: RwSignal<Vec<SessionDto>> = RwSignal::new(Vec::new());

    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            let base = crate::api::api_base();
            if let Ok(resp) = Request::get(&format!("{}/auth/sessions", base))
                .credentials(web_sys::RequestCredentials::Include).send().await {
                if let Ok(data) = resp.json::<SessionsResponse>().await {
                    list.set(data.sessions);
                }
            }
        });
    });

    let revoke = move |sid: String| {
        leptos::task::spawn_local(async move {
            let base = crate::api::api_base();
            let _ = Request::delete(&format!("{}/auth/sessions/{}", base, sid))
                .credentials(web_sys::RequestCredentials::Include)
                .header("X-CSRF-Token", &read_csrf_cookie())
                .send().await;
            if let Some(win) = web_sys::window() { let _ = win.location().reload(); }
        });
    };

    let revoke_all = move |_| {
        leptos::task::spawn_local(async move {
            let base = crate::api::api_base();
            let _ = Request::post(&format!("{}/auth/sessions/revoke-all", base))
                .credentials(web_sys::RequestCredentials::Include)
                .header("X-CSRF-Token", &read_csrf_cookie())
                .send().await;
            if let Some(win) = web_sys::window() { let _ = win.location().reload(); }
        });
    };

    view! {
        <ul class="space-y-3 mb-4">
            <For each=move || list.get() key=|s| s.id.clone() let:s>
                <li class="p-3 border-2 border-ink rounded-sm flex justify-between items-center">
                    <div>
                        <div class="font-bold">{s.device_label.clone().unwrap_or_default()} " · " {s.country_hint.clone().unwrap_or_default()}</div>
                        <div class="text-xs" style="color: var(--ink-40);">{"Last active "}{s.last_seen_at.clone()}</div>
                    </div>
                    {if s.is_current {
                        view! { <span class="text-xs font-bold uppercase tracking-wider px-2 py-1 border-2 border-ink rounded-sm">{move || tr("settings.this_device")}</span> }.into_any()
                    } else {
                        let sid = s.id.clone();
                        view! { <button class="text-xs font-bold uppercase tracking-wider px-2 py-1 border-2 border-ink rounded-sm cursor-pointer" on:click=move |_| revoke(sid.clone())>{move || tr("settings.log_out")}</button> }.into_any()
                    }}
                </li>
            </For>
        </ul>
        <button class="px-4 py-2 text-sm font-bold uppercase tracking-wider border-2 border-ink rounded-sm cursor-pointer"
            style="background: var(--ink); color: var(--cream);"
            on:click=revoke_all>{move || tr("settings.log_out_all_others")}</button>
    }
}

#[component]
fn LinkedAccounts() -> impl IntoView {
    view! {
        <ul class="space-y-2">
            <li class="text-sm">"Telegram — linked"</li>
            <li class="text-sm" style="color: var(--ink-40);">"WhatsApp — " {move || tr("login.coming_soon")}</li>
            <li class="text-sm" style="color: var(--ink-40);">"Discord — " {move || tr("login.coming_soon")}</li>
        </ul>
    }
}
