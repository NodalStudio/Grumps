//! Account settings as a global slide-over **drawer**, not a full page.
//!
//! Account settings are cross-workspace, so they must be reachable from both the
//! workspace shell and the global dashboard. A drawer overlays whatever context
//! you're in, so a single shared open-signal (provided at `App` level) lets any
//! "Account" trigger open it. The drawer body itself is rendered inside an
//! authenticated layout (so `use_session()` resolves) — see `AccountDrawer`.

use crate::auth::{read_csrf_cookie, use_session, SessionContext};
use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::dialog::{Dialog, DialogSide};
use crate::components::ui::field::Field;
use crate::components::ui::select::Select;
use crate::i18n::{tr, tr_p};
use gloo_net::http::Request;
use leptos::prelude::*;
use serde::Deserialize;

/// App-level open-signal for the account drawer. Provided once near the root so
/// every "Account" trigger (sidebar avatar menu, dashboard header) shares it.
#[derive(Clone, Copy)]
pub struct AccountDrawerOpen(pub RwSignal<bool>);

/// Install the shared open-signal. Call once from `App`.
pub fn provide_account_drawer() {
    provide_context(AccountDrawerOpen(RwSignal::new(false)));
}

/// Read the shared open-signal. Falls back to a detached signal if the provider
/// is missing (keeps callers panic-free).
pub fn use_account_drawer() -> RwSignal<bool> {
    use_context::<AccountDrawerOpen>()
        .map(|c| c.0)
        .unwrap_or_else(|| RwSignal::new(false))
}

/// The drawer itself. Render once inside each authenticated layout (workspace
/// shell and dashboard) so its body has the session context; the open-signal is
/// shared, so only the currently-mounted instance shows.
#[component]
pub fn AccountDrawer() -> impl IntoView {
    let open = use_account_drawer();
    let session = use_session().unwrap_or_default();

    view! {
        <Dialog
            open=open
            on_close=move || open.set(false)
            labelledby="account-drawer-title"
            side=DialogSide::End
        >
            <div class="flex items-center justify-between">
                <h2 id="account-drawer-title" class="font-display text-xl font-bold">
                    {move || tr("settings.account")}
                </h2>
                <button
                    type="button"
                    aria-label=move || tr("common.close")
                    class="hover-tint text-2xl leading-none w-8 h-8 flex items-center justify-center rounded-xs cursor-pointer"
                    on:click=move |_| open.set(false)
                >
                    "\u{00D7}"
                </button>
            </div>

            <AccountForm session=session.clone() />

            <section>
                <h3 class="font-display text-sm font-bold uppercase tracking-wider mb-3" style="color: var(--ink-70);">
                    {move || tr("settings.linked_accounts")}
                </h3>
                <LinkedAccounts />
            </section>

            <section>
                <h3 class="font-display text-sm font-bold uppercase tracking-wider mb-3" style="color: var(--ink-70);">
                    {move || tr("settings.sessions")}
                </h3>
                <SessionList />
            </section>
        </Dialog>
    }
}

#[component]
fn AccountForm(session: SessionContext) -> impl IntoView {
    let (name, set_name) = signal(session.display_name.clone());
    let initial_locale = session
        .default_locale
        .clone()
        .unwrap_or_else(|| "en".into());
    let (locale, set_locale) = signal(initial_locale);

    let save = move |_| {
        let n = name.get();
        let l = locale.get();
        if crate::demo::is_demo() {
            return;
        }
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
        <div class="flex flex-col gap-4">
            <Field label=tr("settings.display_name") id="acct-display-name">
                <input id="acct-display-name" type="text" class="w-full p-2 border-2 border-ink rounded-xs" style="background: var(--cream);"
                    prop:value=name on:input=move |ev| set_name.set(event_target_value(&ev))/>
            </Field>
            <label class="block">
                <span class="block text-xs font-semibold uppercase tracking-wider mb-1" style="color: var(--ink-70);">{move || tr("settings.default_locale")}</span>
                <Select
                    value=locale
                    aria_label=tr("settings.default_locale")
                    full_width=true
                    on_change=move |v: String| set_locale.set(v)
                >
                    {["en","es","pt-BR","fr","de","it","ru","tr","ar","hi","zh-CN","ja","ko","id"].iter().map(|code| {
                        let code = *code;
                        view! { <option value=code>{code}</option> }
                    }).collect_view()}
                </Select>
            </label>
            <Button variant=ButtonVariant::Primary class="uppercase tracking-wider self-start" on_click=save>
                {move || tr("common.save")}
            </Button>
        </div>
    }
}

#[component]
fn LinkedAccounts() -> impl IntoView {
    view! {
        <ul class="space-y-2">
            <li class="text-sm">"Telegram — " {move || tr("settings.linked")}</li>
            <li class="text-sm" style="color: var(--ink-40);">"WhatsApp — " {move || tr("login.coming_soon")}</li>
            <li class="text-sm" style="color: var(--ink-40);">"Discord — " {move || tr("login.coming_soon")}</li>
        </ul>
    }
}

#[derive(Deserialize, Default, Clone)]
struct SessionsResponse {
    sessions: Vec<SessionDto>,
}

#[derive(Deserialize, Clone)]
struct SessionDto {
    id: String,
    device_label: Option<String>,
    country_hint: Option<String>,
    #[allow(dead_code)]
    created_at: String,
    last_seen_at: String,
    is_current: bool,
}

#[component]
fn SessionList() -> impl IntoView {
    let list: RwSignal<Vec<SessionDto>> = RwSignal::new(Vec::new());

    Effect::new(move |_| {
        // Demo mode short-circuits the API; seed a single current session so the
        // drawer renders without hitting (and 404-ing on) the network.
        if crate::demo::is_demo() {
            list.set(vec![SessionDto {
                id: "demo-session".into(),
                device_label: Some("Chrome".into()),
                country_hint: Some("FR".into()),
                created_at: String::new(),
                last_seen_at: "2026-06-02".into(),
                is_current: true,
            }]);
            return;
        }
        leptos::task::spawn_local(async move {
            let base = crate::api::api_base();
            if let Ok(resp) = Request::get(&format!("{}/auth/sessions", base))
                .credentials(web_sys::RequestCredentials::Include)
                .send()
                .await
            {
                if let Ok(data) = resp.json::<SessionsResponse>().await {
                    list.set(data.sessions);
                }
            }
        });
    });

    let revoke = move |sid: String| {
        if crate::demo::is_demo() {
            return;
        }
        leptos::task::spawn_local(async move {
            let base = crate::api::api_base();
            let _ = Request::delete(&format!("{}/auth/sessions/{}", base, sid))
                .credentials(web_sys::RequestCredentials::Include)
                .header("X-CSRF-Token", &read_csrf_cookie())
                .send()
                .await;
            if let Some(win) = web_sys::window() {
                let _ = win.location().reload();
            }
        });
    };

    let revoke_all = move |_| {
        if crate::demo::is_demo() {
            return;
        }
        leptos::task::spawn_local(async move {
            let base = crate::api::api_base();
            let _ = Request::post(&format!("{}/auth/sessions/revoke-all", base))
                .credentials(web_sys::RequestCredentials::Include)
                .header("X-CSRF-Token", &read_csrf_cookie())
                .send()
                .await;
            if let Some(win) = web_sys::window() {
                let _ = win.location().reload();
            }
        });
    };

    view! {
        <ul class="space-y-3 mb-4">
            <For each=move || list.get() key=|s| s.id.clone() let:s>
                <li class="p-3 border-2 border-ink rounded-xs flex justify-between items-center gap-2">
                    <div class="min-w-0">
                        <div class="font-bold truncate">{s.device_label.clone().unwrap_or_default()} " · " {s.country_hint.clone().unwrap_or_default()}</div>
                        <div class="text-xs" style="color: var(--ink-40);">{move || tr_p("settings.last_active", &[("date", &s.last_seen_at)])}</div>
                    </div>
                    {if s.is_current {
                        view! { <span class="text-xs font-bold uppercase tracking-wider px-2 py-1 border-2 border-ink rounded-xs shrink-0">{move || tr("settings.this_device")}</span> }.into_any()
                    } else {
                        let sid = s.id.clone();
                        view! { <button class="hover-tint text-xs font-bold uppercase tracking-wider px-2 py-1 border-2 border-ink rounded-xs cursor-pointer shrink-0" on:click=move |_| revoke(sid.clone())>{move || tr("settings.log_out")}</button> }.into_any()
                    }}
                </li>
            </For>
        </ul>
        <Button variant=ButtonVariant::Primary class="uppercase tracking-wider" on_click=revoke_all>
            {move || tr("settings.log_out_all_others")}
        </Button>
    }
}
