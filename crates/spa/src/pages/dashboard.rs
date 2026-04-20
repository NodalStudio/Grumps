use leptos::prelude::*;
use crate::auth::use_auth;
use crate::components::header::PageHeader;
use crate::i18n::tr;

#[component]
pub fn DashboardPage() -> impl IntoView {
    let auth = use_auth();
    let workspaces = auth.workspaces;

    view! {
        <div class="min-h-screen" style="background: var(--cream);">
            <PageHeader title=tr("sidebar.my_workspaces") subtitle=tr("page.dashboard.subtitle") />
            <div class="p-8">
                <div class="grid gap-4" style="grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));">
                    <For
                        each=move || workspaces.get()
                        key=|ws| ws.slug.clone()
                        children=move |ws| {
                            let prefix = crate::demo::router_base();
                            let href = format!("{}/w/{}", prefix, ws.slug);
                            view! {
                                <a href=href class="block p-6 border-2 border-ink rounded-sm cursor-pointer transition-transform hover:-translate-y-0.5" style="background: var(--cream-light);">
                                    <h3 class="font-display text-lg font-bold">{
                                        let name = ws.name.clone().unwrap_or_else(|| ws.slug.clone());
                                        move || tr(&name)
                                    }</h3>
                                    <p class="text-[11px] font-semibold uppercase tracking-wider mt-1" style="color: var(--ink-40);">"WhatsApp"</p>
                                </a>
                            }
                        }
                    />
                    // Add group placeholder
                    <div class="p-6 border-2 border-dashed rounded-sm opacity-50 text-center" style="border-color: var(--ink-15);">
                        <h3 class="font-display text-lg font-bold" style="color: var(--ink-40);">{move || tr("dashboard.add_group.title")}</h3>
                        <p class="text-[11px] uppercase tracking-wider mt-1" style="color: var(--ink-40);">{move || tr("dashboard.add_group.help")}</p>
                        <div class="text-3xl mt-4" style="color: var(--ink-15);">"+"</div>
                    </div>
                </div>
            </div>
        </div>
    }
}
