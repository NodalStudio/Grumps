use leptos::prelude::*;
use leptos_router::components::A;
use crate::auth::use_auth;

#[component]
pub fn Sidebar(slug: String) -> impl IntoView {
    let base = format!("/w/{}", slug);

    // Check super admin status once on mount
    let auth = use_auth();
    let api = auth.api.clone();
    let admin_me = LocalResource::new(move || {
        let api = api.clone();
        async move { api.get_admin_me().await.ok() }
    });

    view! {
        <aside class="w-64 min-w-[260px] flex flex-col overflow-y-auto border-r-2 border-ink" style="background: var(--cream-light);">
            // Brand
            <div class="px-5 pt-6 pb-5 border-b-2 border-ink">
                <h1 class="font-display text-xl font-extrabold uppercase tracking-tight">
                    "GRUMPS"<span class="text-brick">"."</span>
                </h1>
                <p class="text-[11px] uppercase tracking-wider mt-0.5 font-medium" style="color: var(--ink-40);">
                    "Gets it done. No small talk."
                </p>
            </div>

            // Workspace selector
            <div class="px-5 py-4 border-b" style="border-color: var(--ink-15);">
                <div class="flex items-center gap-2 font-display font-bold text-sm cursor-pointer">
                    <span class="w-2 h-2 rounded-full bg-teal flex-shrink-0"></span>
                    {slug}
                    <span class="ml-auto text-[10px]" style="color: var(--ink-40);">{"\u{25BC}"}</span>
                </div>
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
                                <div class="px-5 pt-3 pb-1.5 text-[10px] font-bold uppercase tracking-[1.5px]" style="color: var(--brick);">"Super Admin"</div>
                                <A href="/admin/observability"
                                   attr:class="flex items-center gap-2.5 px-5 py-2 text-sm font-medium cursor-pointer transition-all border-l-[3px] border-transparent hover:bg-black/[0.04]"
                                   attr:style="color: var(--ink);">
                                    <span class="w-[18px] text-center text-[15px]">"🌐"</span>
                                    "Observabilité globale"
                                </A>
                            </div>
                        }.into_any()
                    } else {
                        view! { <span></span> }.into_any()
                    }
                }}
                <div class="px-5 pt-4 pb-1.5 text-[10px] font-bold uppercase tracking-[1.5px]" style="color: var(--ink-40);">"Workspace"</div>
                <NavItem href=base.clone() label="Overview" icon="\u{25EB}" />
                <NavItem href=format!("{}/todos", base) label="Todos" icon="\u{2610}" />
                <NavItem href=format!("{}/notes", base) label="Notes" icon="\u{00B6}" />
                <NavItem href=format!("{}/files", base) label="Files" icon="\u{25F0}" />
                <NavItem href=format!("{}/history", base) label="History" icon="\u{21BB}" />
                <NavItem href=format!("{}/calendar", base) label="Calendar" icon="\u{1F4C5}" />
                <NavItem href=format!("{}/memory", base) label="Memory" icon="\u{1F9E0}" />
                <NavItem href=format!("{}/scheduled", base) label="Scheduled" icon="\u{23F0}" />

                <div class="px-5 pt-4 pb-1.5 text-[10px] font-bold uppercase tracking-[1.5px]" style="color: var(--ink-40);">"Manage"</div>
                <NavItem href=format!("{}/settings", base) label="Settings" icon="\u{2699}" />
                <NavItem href=format!("{}/admin/observability", base) label="Observability" icon="\u{1F4CA}" />
                <A href="/dashboard" attr:class="flex items-center gap-2.5 px-5 py-2 text-sm font-medium cursor-pointer transition-all border-l-[3px] border-transparent hover:bg-black/[0.04]" attr:style="color: var(--ink-70);">
                    <span class="w-[18px] text-center text-[15px]">{"\u{229E}"}</span>
                    "My Workspaces"
                </A>
            </nav>

            // User footer
            <div class="px-5 py-4 flex items-center gap-2.5 border-t" style="border-color: var(--ink-15);">
                <div class="w-8 h-8 rounded-full flex items-center justify-center text-sm font-display font-bold" style="background: var(--ink); color: var(--cream);">
                    "A"
                </div>
                <div class="flex-1 min-w-0">
                    <div class="text-[13px] font-semibold truncate">"User"</div>
                    <div class="text-[11px] uppercase tracking-wider font-medium" style="color: var(--ink-40);">"Member"</div>
                </div>
            </div>
        </aside>
    }
}

#[component]
fn NavItem(href: String, label: &'static str, icon: &'static str) -> impl IntoView {
    view! {
        <A href=href attr:class="flex items-center gap-2.5 px-5 py-2 text-sm font-medium cursor-pointer transition-all border-l-[3px] border-transparent hover:bg-black/[0.04]" attr:style="color: var(--ink-70);">
            <span class="w-[18px] text-center text-[15px]">{icon}</span>
            {label}
        </A>
    }
}
