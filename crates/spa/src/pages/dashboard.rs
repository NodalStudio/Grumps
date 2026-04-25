use leptos::prelude::*;
use crate::auth::{use_session, WorkspaceRef};
use crate::components::header::PageHeader;
use crate::i18n::tr;

#[component]
pub fn DashboardPage() -> impl IntoView {
    let session = use_session().unwrap_or_default();
    let workspaces = session.workspaces.clone();

    view! {
        <div class="min-h-screen" style="background: var(--cream);">
            <PageHeader title=tr("sidebar.my_workspaces") subtitle=tr("page.dashboard.subtitle") />
            <div class="p-8">
                {if workspaces.is_empty() {
                    view! { <EmptyState/> }.into_any()
                } else {
                    view! { <Grid workspaces=workspaces.clone()/> }.into_any()
                }}
            </div>
        </div>
    }
}

#[component]
fn EmptyState() -> impl IntoView {
    view! {
        <div class="max-w-2xl mx-auto text-center py-16">
            <h2 class="font-display text-2xl font-bold mb-2">{move || tr("dashboard.empty.title")}</h2>
            <div class="grid gap-4 md:grid-cols-2 mt-8">
                <div class="p-6 border-2 border-ink rounded-sm" style="background: var(--cream-light);">
                    <h3 class="font-display text-sm font-bold uppercase tracking-wider mb-3">{move || tr("dashboard.empty.dm_heading")}</h3>
                    <a href="tg://resolve?domain=HeyGrumpsBot&start=hello"
                       class="inline-block px-4 py-3 text-sm font-bold uppercase tracking-wider border-2 border-ink rounded-sm cursor-pointer"
                       style="background: var(--ink); color: var(--cream); font-family: var(--font-body);">
                        {move || tr("dashboard.empty.dm_cta")}
                    </a>
                </div>
                <div class="p-6 border-2 border-ink rounded-sm" style="background: var(--cream-light);">
                    <h3 class="font-display text-sm font-bold uppercase tracking-wider mb-3">{move || tr("dashboard.empty.group_heading")}</h3>
                    <ol class="text-sm text-left list-decimal list-inside space-y-1" style="color: var(--ink-70);">
                        <li>{move || tr("dashboard.empty.group_step1")}</li>
                        <li>{move || tr("dashboard.empty.group_step2")}</li>
                        <li>{move || tr("dashboard.empty.group_step3")}</li>
                    </ol>
                </div>
            </div>
        </div>
    }
}

#[component]
fn Grid(workspaces: Vec<WorkspaceRef>) -> impl IntoView {
    view! {
        <div class="grid gap-4" style="grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));">
            {workspaces.into_iter().map(|ws| {
                let prefix = crate::demo::router_base();
                let href = format!("{}/w/{}", prefix, ws.slug);
                let title = ws.name.clone().unwrap_or_else(|| ws.slug.clone());
                let role = ws.role.clone();
                view! {
                    <a href=href class="block p-6 border-2 border-ink rounded-sm cursor-pointer transition-transform hover:-translate-y-0.5" style="background: var(--cream-light);">
                        <h3 class="font-display text-lg font-bold">{title}</h3>
                        <p class="text-[11px] font-semibold uppercase tracking-wider mt-1" style="color: var(--ink-40);">{format_shape(&ws)}</p>
                        <p class="text-[11px] uppercase tracking-wider mt-2" style="color: var(--ink-40);">{role}</p>
                    </a>
                }
            }).collect_view()}
            <a href="/dashboard" class="block p-6 border-2 border-dashed rounded-sm text-center" style="border-color: var(--ink-15);">
                <h3 class="font-display text-sm font-bold uppercase tracking-wider" style="color: var(--ink-40);">{move || tr("dashboard.add_another")}</h3>
            </a>
        </div>
    }
}

fn format_shape(ws: &WorkspaceRef) -> String {
    let plat = match ws.platform.as_str() {
        "telegram" => "TELEGRAM", "whatsapp" => "WHATSAPP", "discord" => "DISCORD",
        x if !x.is_empty() => return x.to_uppercase(),
        _ => "WORKSPACE",
    };
    let shape = if ws.is_dm { "DM · just you" } else { "GROUP" };
    format!("{} {}", plat, shape)
}
