use leptos::prelude::*;
use leptos_router::components::A;
use crate::auth::use_auth;
use crate::components::Icon;

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
        <aside class="w-64 min-w-[260px] flex flex-col overflow-y-auto border-r-2 border-strong bg-surface-raised">
            // Brand
            <div class="px-5 pt-6 pb-5 border-b-2 border-strong">
                <h1 class="font-display text-xl font-extrabold uppercase tracking-tight">
                    "GRUMPS"<span class="text-accent">"."</span>
                </h1>
                <p class="text-meta mt-0.5 text-muted">
                    "Gets it done. No small talk."
                </p>
            </div>

            // Workspace selector
            <div class="px-5 py-4 border-b border-subtle">
                <div class="flex items-center gap-2 font-display font-bold text-sm cursor-pointer">
                    <span class="w-2 h-2 rounded-full bg-teal flex-shrink-0"></span>
                    {slug}
                    <Icon name="chevron-down" class="size-3 text-muted ml-auto"/>
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
                                <div class="px-5 pt-3 pb-1.5 text-eyebrow text-accent">"Super Admin"</div>
                                <A href="/admin/observability"
                                   attr:class="flex items-center gap-2.5 px-5 py-2 text-body-sm font-medium cursor-pointer transition-all border-l-[3px] border-transparent hover:bg-hover-tint text-secondary">
                                    <Icon name="globe" class="size-4 flex-shrink-0"/>
                                    "Observabilité globale"
                                </A>
                            </div>
                        }.into_any()
                    } else {
                        view! { <span></span> }.into_any()
                    }
                }}
                <div class="px-5 pt-4 pb-1.5 text-eyebrow text-muted">"Workspace"</div>
                <NavItem href=base.clone() label="Overview" icon="overview" />
                <NavItem href=format!("{}/todos", base) label="Todos" icon="todos" />
                <NavItem href=format!("{}/notes", base) label="Notes" icon="notes" />
                <NavItem href=format!("{}/files", base) label="Files" icon="files" />
                <NavItem href=format!("{}/history", base) label="History" icon="history" />
                <NavItem href=format!("{}/calendar", base) label="Calendar" icon="calendar" />
                <NavItem href=format!("{}/memory", base) label="Memory" icon="memory" />
                <NavItem href=format!("{}/scheduled", base) label="Scheduled" icon="scheduled" />

                <div class="px-5 pt-4 pb-1.5 text-eyebrow text-muted">"Manage"</div>
                <NavItem href=format!("{}/settings", base) label="Settings" icon="settings" />
                <A href="/dashboard"
                   attr:class="flex items-center gap-2.5 px-5 py-2 text-body-sm font-medium cursor-pointer transition-all border-l-[3px] border-transparent hover:bg-hover-tint text-secondary">
                    <Icon name="workspaces" class="size-4 flex-shrink-0"/>
                    "My Workspaces"
                </A>
            </nav>

            // User footer
            <div class="px-5 py-4 flex items-center gap-2.5 border-t border-subtle">
                <div class="w-8 h-8 rounded-full flex items-center justify-center text-sm font-display font-bold bg-primary text-surface">
                    "A"
                </div>
                <div class="flex-1 min-w-0">
                    <div class="text-body-sm font-semibold truncate">"User"</div>
                    <div class="text-meta text-muted">"Member"</div>
                </div>
            </div>
        </aside>
    }
}

#[component]
fn NavItem(href: String, label: &'static str, icon: &'static str) -> impl IntoView {
    view! {
        <A href=href
           attr:class="flex items-center gap-2.5 px-5 py-2 text-body-sm font-medium cursor-pointer transition-all border-l-[3px] border-transparent hover:bg-hover-tint text-secondary">
            <Icon name=icon class="size-4 flex-shrink-0"/>
            {label}
        </A>
    }
}
