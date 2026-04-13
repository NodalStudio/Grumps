use leptos::prelude::*;
use leptos_router::components::Outlet;

#[component]
pub fn WorkspaceLayout() -> impl IntoView {
    view! {
        <div class="flex min-h-screen">
            <div class="flex-1">
                <Outlet />
            </div>
        </div>
    }
}
