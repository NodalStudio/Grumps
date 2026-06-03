use crate::components::sidebar::Sidebar;
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
        if s.is_empty() {
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
    }
}
