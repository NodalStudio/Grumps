use crate::api::use_api;
use crate::auth::use_session;
use crate::components::lang_switcher::LangSwitcher;
use crate::components::workspace_switcher::WorkspaceSwitcher;
use crate::i18n::tr;
use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn Sidebar(slug: String) -> impl IntoView {
    // In demo mode the SPA is mounted under `/demo/` (or `/Grumps/demo/`
    // on GH Pages). The router strips that prefix on URL matching, but
    // `<A href>` inputs are passed verbatim — so links must include the
    // prefix to land on the right page after a click that triggers a real
    // navigation (middle-click, opener, etc.).
    let prefix = crate::demo::router_base();
    let base = format!("{}/w/{}", prefix, slug);

    // Check super admin status once on mount.
    let api = use_api();
    let admin_me = LocalResource::new(move || {
        let api = api.clone();
        async move { api.get_admin_me().await.ok() }
    });

    view! {
        <aside class="w-64 min-w-[260px] flex flex-col overflow-y-auto border-r-2 border-ink" style="background: var(--cream-light);">
            // Brand — h-24 matches PageHeader so bottom borders align
            <div class="px-5 h-24 pb-5 border-b-2 border-ink flex flex-col justify-end">
                <h1 class="font-display text-xl font-extrabold uppercase tracking-tight">
                    "GRUMPS"<span class="text-brick">"."</span>
                </h1>
                <p class="text-[11px] uppercase tracking-wider mt-0.5 font-medium" style="color: var(--ink-40);">
                    {move || tr("brand.tagline")}
                </p>
            </div>

            // Workspace selector
            <div class="px-5 py-4 border-b" style="border-color: var(--ink-15);">
                <WorkspaceSwitcher current_slug=slug.clone() />
            </div>

            // Nav
            <nav class="py-3 flex-1">
                // Super admin link — rendered only if user is super admin
                {move || {
                    let is_super = admin_me.get()
                        .and_then(|m| (*m).clone())
                        .map(|m| m.is_super_admin)
                        .unwrap_or(false);
                    if is_super {
                        view! {
                            <div>
                                // Super admin surfaces are English-only (sole user is the platform owner).
                                <div class="px-5 pt-3 pb-1.5 text-[10px] font-bold uppercase tracking-[1.5px]" style="color: var(--brick);">"Super Admin"</div>
                                <A href=format!("{}/admin/observability", crate::demo::router_base())
                                   attr:class="flex items-center gap-2.5 px-5 py-2 text-sm font-medium cursor-pointer transition-all border-l-[3px] border-transparent hover:bg-black/[0.04]"
                                   attr:style="color: var(--ink);">
                                    <span class="w-[18px] text-center text-[15px]">{"\u{2295}"}</span>
                                    "Global observability"
                                </A>
                            </div>
                        }.into_any()
                    } else {
                        view! { <span></span> }.into_any()
                    }
                }}
                <div class="px-5 pt-4 pb-1.5 text-[10px] font-bold uppercase tracking-[1.5px]" style="color: var(--ink-40);">{move || tr("sidebar.section.workspace")}</div>
                <NavItem href=base.clone() i18n_key="sidebar.nav.overview" icon="\u{25EB}" />
                <NavItem href=format!("{}/todos", base) i18n_key="sidebar.nav.todos" icon="\u{2610}" />
                <NavItem href=format!("{}/notes", base) i18n_key="sidebar.nav.notes" icon="\u{00B6}" />
                <NavItem href=format!("{}/files", base) i18n_key="sidebar.nav.files" icon="\u{25F0}" />
                <NavItem href=format!("{}/history", base) i18n_key="sidebar.nav.history" icon="\u{21BB}" />
                <NavItem href=format!("{}/calendar", base) i18n_key="sidebar.nav.calendar" icon="\u{25A6}" />
                <NavItem href=format!("{}/memory", base) i18n_key="sidebar.nav.memory" icon="\u{25C9}" />
                <NavItem href=format!("{}/scheduled", base) i18n_key="sidebar.nav.scheduled" icon="\u{25F7}" />

                <div class="px-5 pt-4 pb-1.5 text-[10px] font-bold uppercase tracking-[1.5px]" style="color: var(--ink-40);">{move || tr("sidebar.section.manage")}</div>
                <NavItem href=format!("{}/settings", base) i18n_key="sidebar.nav.settings" icon="\u{2699}" />
                <A href=format!("{}/settings", crate::demo::router_base()) attr:class="flex items-center gap-2.5 px-5 py-2 text-sm font-medium cursor-pointer transition-all border-l-[3px] border-transparent hover:bg-black/[0.04]" attr:style="color: var(--ink-70);">
                    <span class="w-[18px] text-center text-[15px]">{"\u{25CB}"}</span>
                    {move || tr("settings.account")}
                </A>
                <A href=format!("{}/dashboard", crate::demo::router_base()) attr:class="flex items-center gap-2.5 px-5 py-2 text-sm font-medium cursor-pointer transition-all border-l-[3px] border-transparent hover:bg-black/[0.04]" attr:style="color: var(--ink-70);">
                    <span class="w-[18px] text-center text-[15px]">{"\u{229E}"}</span>
                    {move || tr("sidebar.my_workspaces")}
                </A>
            </nav>

            // User footer — display name + role from session
            {
                let footer_slug = slug.clone();
                view! {
                    <div class="px-5 py-3 flex items-center gap-2.5 border-t" style="border-color: var(--ink-15);">
                        <div class="w-8 h-8 rounded-full flex items-center justify-center text-sm font-display font-bold" style="background: var(--ink); color: var(--cream);">
                            {move || {
                                let session = use_session().unwrap_or_default();
                                session.display_name.chars().next()
                                    .map(|c| c.to_uppercase().to_string())
                                    .unwrap_or_else(|| "A".into())
                            }}
                        </div>
                        <div class="flex-1 min-w-0">
                            <div class="text-[13px] font-semibold truncate">
                                {move || {
                                    let session = use_session().unwrap_or_default();
                                    if session.display_name.is_empty() { tr("sidebar.user.you") } else { session.display_name }
                                }}
                            </div>
                            <div class="text-[11px] uppercase tracking-wider font-medium" style="color: var(--ink-40);">
                                {move || {
                                    let session = use_session().unwrap_or_default();
                                    let role = session.workspaces.iter().find(|w| w.slug == footer_slug)
                                        .map(|w| w.role.clone())
                                        .unwrap_or_else(|| "member".into());
                                    role.to_uppercase()
                                }}
                            </div>
                        </div>
                    </div>
                }
            }
            // Lang switcher
            <div class="px-5 py-3 border-t flex items-center justify-between gap-2" style="border-color: var(--ink-15);">
                <span class="text-[10px] uppercase tracking-[1.5px] font-bold" style="color: var(--ink-40);">{move || tr("sidebar.section.language")}</span>
                <LangSwitcher />
            </div>
        </aside>
    }
}

#[component]
fn NavItem(href: String, i18n_key: &'static str, icon: &'static str) -> impl IntoView {
    view! {
        <A href=href attr:class="flex items-center gap-2.5 px-5 py-2 text-sm font-medium cursor-pointer transition-all border-l-[3px] border-transparent hover:bg-black/[0.04]" attr:style="color: var(--ink-70);">
            <span class="w-[18px] text-center text-[15px]">{icon}</span>
            {move || tr(i18n_key)}
        </A>
    }
}
