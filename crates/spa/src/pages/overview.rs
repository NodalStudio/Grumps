use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use crate::auth::use_auth;
use crate::api::StatusCounts;
use crate::components::header::PageHeader;

#[component]
pub fn OverviewPage() -> impl IntoView {
    let auth = use_auth();
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let counts = LocalResource::new(move || {
        let api = auth.api.clone();
        let s = slug();
        async move { api.get_workspace_info(&s).await.ok() }
    });

    view! {
        <PageHeader title=slug() subtitle="Workspace overview".to_string() />
        <div class="flex-1 overflow-y-auto p-8">
            <Suspense fallback=|| view! { <div class="text-sm" style="color: var(--ink-40);">"Loading..."</div> }>
                {move || counts.get().map(|data| {
                    let c = (*data).clone().unwrap_or(StatusCounts { open_todos: 0, done_this_week: 0, notes: 0, files: 0 });
                    view! {
                        <div class="flex border-2 border-ink rounded-sm overflow-hidden mb-6" style="background: var(--cream-light);">
                            <StatBlock number=c.open_todos label="Open todos" color="var(--brick)" />
                            <StatBlock number=c.notes label="Notes" color="var(--ink)" />
                            <StatBlock number=c.files label="Files" color="var(--ink)" />
                            <StatBlock number=c.done_this_week label="Done this week" color="var(--teal)" />
                        </div>
                    }
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn StatBlock(number: i64, label: &'static str, color: &'static str) -> impl IntoView {
    view! {
        <div class="flex-1 p-4 border-r-2 border-ink last:border-r-0">
            <div class="font-display text-3xl font-extrabold" style:color=color>{number}</div>
            <div class="text-[11px] uppercase tracking-wider font-semibold mt-1" style="color: var(--ink-40);">{label}</div>
        </div>
    }
}
