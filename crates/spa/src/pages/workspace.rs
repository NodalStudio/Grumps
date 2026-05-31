use crate::components::sidebar::Sidebar;
use leptos::prelude::*;
use leptos_router::components::Outlet;
use leptos_router::hooks::use_params_map;

#[component]
pub fn WorkspaceLayout() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    view! {
        <div class="flex h-screen overflow-hidden">
            <Sidebar slug=slug() />
            <main class="flex-1 flex flex-col overflow-hidden">
                <Outlet />
            </main>
        </div>
    }
}
