use crate::components::sidebar::Sidebar;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::Outlet;
use leptos_router::hooks::use_params_map;

#[component]
pub fn WorkspaceLayout() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    // Provide the workspace timezone to the whole subtree (default UTC until
    // the workspace info loads). Every timestamp renders against this zone.
    let tz_sig = crate::datetime::provide_timezone("UTC");
    let api = crate::api::use_api();

    // Load the workspace timezone; on first visit (no explicit source yet),
    // adopt the browser's timezone so chat-only groups still get a sane default.
    {
        let api = api.clone();
        let s = slug();
        spawn_local(async move {
            if let Ok(info) = api.get_workspace_info(&s).await {
                let tz = info.timezone.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "UTC".into());
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
    }

    view! {
        <div class="flex h-screen overflow-hidden">
            <Sidebar slug=slug() />
            <main class="flex-1 flex flex-col overflow-hidden">
                <Outlet />
            </main>
        </div>
    }
}
