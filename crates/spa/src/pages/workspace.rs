use crate::components::sidebar::Sidebar;
use crate::i18n::tr;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::Outlet;
use leptos_router::hooks::use_params_map;

#[component]
pub fn WorkspaceLayout() -> impl IntoView {
    let params = use_params_map();
    // Memoize the slug so dependents only react to a genuine workspace change,
    // not to every child-route param update. The parent route `/w/:slug` stays
    // mounted across workspace switches (only the param signal updates), so any
    // derived state captured by value here would otherwise go stale.
    let slug = Memo::new(move |_| params.read().get("slug").unwrap_or_default());

    // The session's workspace list is fixed at login (refreshed on
    // `/auth/me`) — a slug outside it means either a typo'd URL or a
    // workspace the caller was removed from. Without this check the page
    // just renders silently empty/erroring child fetches (every workspace
    // API 403s server-side, but nothing tells the user why).
    let session = crate::auth::use_session();
    // `Memo` (not a plain closure) so the `Copy` handle can be captured by
    // the several independent `move` closures below without fighting over
    // ownership of the non-`Copy` `SessionContext`.
    let has_access = Memo::new(move |_| {
        let s = slug.get();
        session
            .as_ref()
            .map(|sess| sess.workspaces.iter().any(|w| w.slug == s))
            .unwrap_or(true)
    });

    // Provide the workspace timezone to the whole subtree (default UTC until
    // the workspace info loads). Every timestamp renders against this zone.
    let tz_sig = crate::datetime::provide_timezone("UTC");
    let api = crate::api::use_api();

    // Load the workspace timezone whenever the active workspace changes; on
    // first visit (no explicit source yet), adopt the browser's timezone so
    // chat-only groups still get a sane default. An Effect (not a one-shot
    // spawn) so switching workspace re-reads the new group's zone.
    Effect::new(move |_| {
        let s = slug.get();
        if s.is_empty() || !has_access.get() {
            return;
        }
        let api = api.clone();
        spawn_local(async move {
            if let Ok(info) = api.get_workspace_info(&s).await {
                let tz = info
                    .timezone
                    .clone()
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| "UTC".into());
                tz_sig.set(tz.clone());
                let source = info.timezone_source.clone().unwrap_or_default();
                if source.is_empty() || source == "default" {
                    if let Some(btz) = crate::datetime::browser_timezone() {
                        if btz != tz && api.update_timezone(&s, &btz).await.is_ok() {
                            tz_sig.set(btz);
                        }
                    }
                }
            }
        });
    });

    view! {
        {move || if has_access.get() {
            view! {
                <div class="flex h-screen overflow-hidden">
                    // Rebuild the sidebar when the workspace changes so its nav links,
                    // footer role and switcher label track the new slug (the parent
                    // route view itself is not re-created on an in-place slug change).
                    {move || view! { <Sidebar slug=slug.get() /> }}
                    <main class="flex-1 flex flex-col overflow-hidden">
                        <Outlet />
                    </main>
                    <crate::components::account_drawer::AccountDrawer />
                </div>
            }.into_any()
        } else {
            view! {
                <div class="flex h-screen items-center justify-center p-8">
                    <div class="text-center max-w-sm">
                        <p class="font-display text-xl font-bold mb-2">{move || tr("workspace.no_access.title")}</p>
                        <p class="text-sm mb-6" style="color: var(--ink-40);">{move || tr("auth.error.not_member")}</p>
                        <a
                            href=format!("{}/dashboard", crate::demo::router_base())
                            class="inline-block px-4 py-2 text-sm font-semibold border-2 border-ink rounded-xs"
                            style="background: var(--ink); color: var(--cream);"
                        >{move || tr("workspace.no_access.action")}</a>
                    </div>
                </div>
            }.into_any()
        }}
    }
}
