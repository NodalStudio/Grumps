use crate::auth::{use_session, WorkspaceRef};
use crate::components::header::PageHeader;
use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::dialog::Dialog;
use crate::i18n::tr;
use leptos::prelude::*;

#[component]
pub fn DashboardPage() -> impl IntoView {
    let session = use_session().unwrap_or_default();
    let workspaces = session.workspaces.clone();
    let drawer = crate::components::account_drawer::use_account_drawer();

    view! {
        <div class="min-h-screen" style="background: var(--cream);">
            <PageHeader title=move || tr("sidebar.my_workspaces") subtitle=Signal::derive(move || tr("page.dashboard.subtitle"))>
                <button type="button"
                   on:click=move |_| drawer.set(true)
                   class="hover-tint px-4 py-2 text-sm font-semibold border-2 border-ink rounded-xs cursor-pointer"
                   style="color: var(--ink);">
                    {move || tr("settings.account")}
                </button>
            </PageHeader>
            <div class="p-8">
                {if workspaces.is_empty() {
                    view! { <EmptyState/> }.into_any()
                } else {
                    view! { <Grid workspaces=workspaces.clone()/> }.into_any()
                }}
            </div>
            <crate::components::account_drawer::AccountDrawer />
        </div>
    }
}

#[component]
fn EmptyState() -> impl IntoView {
    view! {
        <div class="max-w-2xl mx-auto text-center py-16">
            <h2 class="font-display text-2xl font-bold mb-2">{move || tr("dashboard.empty.title")}</h2>
            <div class="grid gap-4 md:grid-cols-2 mt-8">
                <div class="p-6 border-2 border-ink rounded-xs" style="background: var(--cream-light);">
                    <h3 class="font-display text-sm font-bold uppercase tracking-wider mb-3">{move || tr("dashboard.empty.dm_heading")}</h3>
                    <a href="tg://resolve?domain=HeyGrumpsBot&start=hello"
                       class="inline-block px-4 py-3 text-sm font-bold uppercase tracking-wider border-2 border-ink rounded-xs cursor-pointer"
                       style="background: var(--ink); color: var(--cream); font-family: var(--font-body);">
                        {move || tr("dashboard.empty.dm_cta")}
                    </a>
                </div>
                <div class="p-6 border-2 border-ink rounded-xs" style="background: var(--cream-light);">
                    <h3 class="font-display text-sm font-bold uppercase tracking-wider mb-3">{move || tr("dashboard.empty.group_heading")}</h3>
                    <a href="https://t.me/HeyGrumpsBot?startgroup=true" target="_blank"
                       class="inline-block px-4 py-3 text-sm font-bold uppercase tracking-wider border-2 border-ink rounded-xs cursor-pointer mb-3"
                       style="background: var(--ink); color: var(--cream); font-family: var(--font-body);">
                        {move || tr("dashboard.empty.group_step2")}
                    </a>
                    <p class="text-xs" style="color: var(--ink-40);">{move || tr("dashboard.empty.group_step3")}</p>
                </div>
            </div>
        </div>
    }
}

#[component]
fn Grid(workspaces: Vec<WorkspaceRef>) -> impl IntoView {
    let show_help = RwSignal::new(false);
    view! {
        <div class="grid gap-4" style="grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));">
            {workspaces.into_iter().map(|ws| {
                let prefix = crate::demo::router_base();
                let href = format!("{}/w/{}", prefix, ws.slug);
                let title = ws.name.clone().map(|n| tr(&n)).unwrap_or_else(|| ws.slug.clone());
                let role = ws.role.clone().to_uppercase();
                let archived = ws.archived;
                let card_class = if archived {
                    "block p-6 border-2 border-dashed rounded-xs cursor-pointer transition-transform hover:-translate-y-0.5 opacity-60"
                } else {
                    "block p-6 border-2 border-ink rounded-xs cursor-pointer transition-transform hover:-translate-y-0.5"
                };
                view! {
                    <a href=href class=card_class style="background: var(--cream-light);">
                        <div class="flex items-start justify-between gap-2 mb-1">
                            <h3 class="font-display text-lg font-bold">{title}</h3>
                            {if archived {
                                Some(view! { <span class="text-[9px] uppercase tracking-wider font-bold px-1.5 py-0.5 border" style="color: var(--ink-40); border-color: var(--ink-40);">{move || tr("dashboard.workspace.archived")}</span> })
                            } else { None }}
                        </div>
                        <p class="text-[11px] font-semibold uppercase tracking-wider" style="color: var(--ink-40);">{format_shape(&ws)}</p>
                        <p class="text-[11px] uppercase tracking-wider mt-2" style="color: var(--ink-40);">{role}</p>
                    </a>
                }
            }).collect_view()}
            <button
                type="button"
                on:click=move |_| show_help.set(true)
                class="block p-6 border-2 border-dashed rounded-xs text-center cursor-pointer transition-colors hover:border-ink w-full"
                style="border-color: var(--ink-15); background: transparent;">
                <h3 class="font-display text-sm font-bold uppercase tracking-wider" style="color: var(--ink-40);">{move || tr("dashboard.add_another")}</h3>
            </button>
        </div>
        <AddGroupHelp open=show_help />
    }
}

#[component]
fn AddGroupHelp(open: RwSignal<bool>) -> impl IntoView {
    view! {
        <Dialog
            open=open
            on_close=move || open.set(false)
            labelledby="add-group-title"
        >
            <h2 id="add-group-title" class="font-display text-xl font-bold">{move || tr("dashboard.add_modal.title")}</h2>
            <p class="text-sm" style="color: var(--ink-70);">{move || tr("dashboard.add_modal.intro")}</p>

            <div class="border-2 border-ink rounded-xs p-4 flex flex-col gap-2" style="background: var(--cream-light);">
                <h3 class="font-display text-sm font-bold uppercase tracking-wider">{move || tr("dashboard.add_modal.tg_heading")}</h3>
                <ol class="list-decimal ps-5 text-sm flex flex-col gap-1" style="color: var(--ink-70);">
                    <li>{move || tr("dashboard.add_modal.tg_step1")}</li>
                    <li>{move || tr("dashboard.add_modal.tg_step2")}</li>
                    <li>{move || tr("dashboard.add_modal.tg_step3")}</li>
                    <li>{move || tr("dashboard.add_modal.tg_step4")}</li>
                </ol>
                <a href="https://t.me/HeyGrumpsBot" target="_blank" rel="noopener"
                   class="self-start mt-2 px-3 py-2 text-xs font-bold uppercase tracking-wider border-2 border-ink rounded-xs cursor-pointer"
                   style="background: var(--ink); color: var(--cream); font-family: var(--font-body);">
                    {move || tr("dashboard.add_modal.tg_open")}
                </a>
            </div>

            <div class="border-2 border-dashed rounded-xs p-4 flex justify-between items-center" style="border-color: var(--ink-15); color: var(--ink-40);">
                <h3 class="font-display text-sm font-bold uppercase tracking-wider">{move || tr("dashboard.add_modal.wa_heading")}</h3>
                <span class="text-xs font-semibold uppercase tracking-wider">{move || tr("dashboard.add_modal.wa_soon")}</span>
            </div>
            <div class="border-2 border-dashed rounded-xs p-4 flex justify-between items-center" style="border-color: var(--ink-15); color: var(--ink-40);">
                <h3 class="font-display text-sm font-bold uppercase tracking-wider">{move || tr("dashboard.add_modal.dc_heading")}</h3>
                <span class="text-xs font-semibold uppercase tracking-wider">{move || tr("dashboard.add_modal.dc_soon")}</span>
            </div>

            <div class="flex justify-end">
                <Button
                    variant=ButtonVariant::Secondary
                    class="bg-cream"
                    on_click=move |_| open.set(false)
                >
                    {move || tr("common.close")}
                </Button>
            </div>
        </Dialog>
    }
}

fn format_shape(ws: &WorkspaceRef) -> String {
    // Platform names are brand nouns (left as-is, like the switcher's TG/WA/DC
    // codes); the shape word is prose, so it goes through i18n. The label is
    // CSS-uppercased at the call site.
    let shape = if ws.is_dm {
        tr("workspace.type_dm")
    } else {
        tr("workspace.type_group")
    };
    let plat = match ws.platform.as_str() {
        "telegram" => "Telegram",
        "whatsapp" => "WhatsApp",
        "discord" => "Discord",
        x if !x.is_empty() => return format!("{} {}", x, shape),
        _ => return shape,
    };
    format!("{} {}", plat, shape)
}
