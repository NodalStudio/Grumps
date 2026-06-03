use crate::api::use_api;
use crate::auth::use_session;
use crate::components::lang_switcher::LangSwitcher;
use crate::components::ui::dropdown_menu::{DropdownMenu, MenuAlign};
use crate::components::workspace_switcher::WorkspaceSwitcher;
use crate::i18n::tr;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::{use_location, use_navigate};

#[component]
pub fn Sidebar(slug: String) -> impl IntoView {
    // In demo mode the SPA is mounted under `/demo/` (or `/Grumps/demo/` on GH
    // Pages). Links must include that prefix; active-link matching strips it so
    // the comparison is always against base-relative paths.
    let rbase = crate::demo::router_base();
    let wbase = format!("/w/{}", slug); // base-relative workspace root

    // Check super admin status once on mount.
    let api = use_api();
    let admin_me = LocalResource::new(move || {
        let api = api.clone();
        async move { api.get_admin_me().await.ok() }
    });

    // Reactive, base-relative current path. `strip_prefix` is a no-op when the
    // platform reports an already-base-relative path, so this is correct whether
    // or not `location.pathname` carries the router base.
    let location = use_location();
    let rbase_cur = rbase.clone();
    let cur = Memo::new(move |_| {
        let p = location.pathname.get();
        p.strip_prefix(&rbase_cur).unwrap_or(&p).to_string()
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

            // Nav — a single flat list (no section headers). The active page is
            // marked with a brick leading border + tint via `aria-current`.
            <nav class="py-3 flex-1">
                // Super-admin entry — rendered only for super admins. English-only
                // (sole user is the platform owner), so no i18n key.
                {
                    let rbase = rbase.clone();
                    move || {
                        let is_super = admin_me.get()
                            .and_then(|m| m.clone())
                            .map(|m| m.is_super_admin)
                            .unwrap_or(false);
                        is_super.then(|| view! {
                            <NavItem rel="/admin/observability".to_string() text="Global observability" icon="\u{2295}" rbase=rbase.clone() cur=cur exact=true />
                        })
                    }
                }
                <NavItem rel=wbase.clone() i18n_key="sidebar.nav.overview" icon="\u{25EB}" rbase=rbase.clone() cur=cur exact=true />
                <NavItem rel=format!("{}/todos", wbase) i18n_key="sidebar.nav.todos" icon="\u{2610}" rbase=rbase.clone() cur=cur />
                <NavItem rel=format!("{}/notes", wbase) i18n_key="sidebar.nav.notes" icon="\u{00B6}" rbase=rbase.clone() cur=cur />
                <NavItem rel=format!("{}/files", wbase) i18n_key="sidebar.nav.files" icon="\u{25F0}" rbase=rbase.clone() cur=cur />
                <NavItem rel=format!("{}/history", wbase) i18n_key="sidebar.nav.history" icon="\u{21BB}" rbase=rbase.clone() cur=cur />
                <NavItem rel=format!("{}/calendar", wbase) i18n_key="sidebar.nav.calendar" icon="\u{25A6}" rbase=rbase.clone() cur=cur />
                <NavItem rel=format!("{}/memory", wbase) i18n_key="sidebar.nav.memory" icon="\u{25C9}" rbase=rbase.clone() cur=cur />
                <NavItem rel=format!("{}/scheduled", wbase) i18n_key="sidebar.nav.scheduled" icon="\u{25F7}" rbase=rbase.clone() cur=cur />

                // Settings sits slightly apart from the content pages.
                <div class="mx-5 my-2 border-t" style="border-color: var(--ink-15);"></div>
                <NavItem rel=format!("{}/settings", wbase) i18n_key="sidebar.nav.settings" icon="\u{2699}" rbase=rbase.clone() cur=cur />
            </nav>

            // User menu — account, workspaces and logout live behind the avatar.
            <UserMenu slug=slug.clone() rbase=rbase.clone() />

            // Lang switcher
            <div class="px-5 py-3 border-t flex items-center justify-between gap-2" style="border-color: var(--ink-15);">
                <span class="text-[10px] uppercase tracking-[1.5px] font-bold" style="color: var(--ink-40);">{move || tr("sidebar.section.language")}</span>
                <LangSwitcher />
            </div>
        </aside>
    }
}

#[component]
fn NavItem(
    /// Base-relative target, e.g. `/w/trip/todos`. The router base is prepended
    /// for the `href`; matching is done against the base-relative current path.
    rel: String,
    icon: &'static str,
    rbase: String,
    cur: Memo<String>,
    /// i18n key for the label; when empty, `text` is used verbatim.
    #[prop(optional)]
    i18n_key: &'static str,
    /// Literal label, used only when `i18n_key` is empty (super-admin link).
    #[prop(optional)]
    text: &'static str,
    /// Match the path exactly rather than as a prefix. Needed for the workspace
    /// root so it doesn't stay highlighted on every sub-page.
    #[prop(optional)]
    exact: bool,
) -> impl IntoView {
    let href = format!("{}{}", rbase, rel);
    let rel_match = rel;
    // `Memo<bool>` is `Copy`, so the three attribute closures below can each read
    // it; a plain closure owning `rel_match` would be moved by the first use.
    let active = Memo::new(move |_| {
        let c = cur.get();
        let c = c.trim_end_matches('/');
        let r = rel_match.trim_end_matches('/');
        if exact {
            c == r
        } else {
            c == r || c.starts_with(&format!("{}/", r))
        }
    });
    let label = move || {
        if i18n_key.is_empty() {
            text.to_string()
        } else {
            tr(i18n_key)
        }
    };
    view! {
        <a
            href=href
            aria-current=move || active.get().then_some("page")
            class=move || {
                let b = "flex items-center gap-2.5 px-5 py-2 text-sm font-medium cursor-pointer border-l-[3px] hover-tint";
                if active.get() {
                    format!("{b} border-brick bg-black/[0.04]")
                } else {
                    format!("{b} border-transparent")
                }
            }
            style=move || if active.get() { "color: var(--ink);" } else { "color: var(--ink-70);" }
        >
            <span class="w-[18px] text-center text-[15px]">{icon}</span>
            {label}
        </a>
    }
}

#[component]
fn UserMenu(slug: String, rbase: String) -> impl IntoView {
    let open = RwSignal::new(false);
    let session = use_session().unwrap_or_default();
    let display = if session.display_name.is_empty() {
        tr("sidebar.user.you")
    } else {
        session.display_name.clone()
    };
    let initial = display
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "A".into());
    let role = session
        .workspaces
        .iter()
        .find(|w| w.slug == slug)
        .map(|w| w.role.clone())
        .unwrap_or_else(|| "member".into())
        .to_uppercase();

    let workspaces_href = format!("{}/dashboard", rbase);
    let login_href = format!("{}/login", rbase);

    // Account settings live in a global drawer, not a page — open the shared
    // signal instead of navigating.
    let drawer = crate::components::account_drawer::use_account_drawer();

    // Park owned data in `Copy` stores so the trigger (`ViewFn`) and children
    // (`ChildrenFn`) closures can read it on every call (mirrors WorkspaceSwitcher).
    let display = StoredValue::new(display);
    let initial = StoredValue::new(initial);
    let role = StoredValue::new(role);
    let workspaces_href = StoredValue::new(workspaces_href);
    let login_href = StoredValue::new(login_href);
    let navigate = StoredValue::new_local(use_navigate());

    view! {
        <div class="border-t" style="border-color: var(--ink-15);">
            <DropdownMenu
                open=open
                align=MenuAlign::Start
                container_class="flex w-full"
                aria_label=Signal::derive(|| tr("settings.account"))
                trigger_class="w-full flex items-center gap-2.5 px-5 py-3 cursor-pointer text-left"
                trigger_style="background: transparent;"
                menu_class="inset-inline-0 bottom-full mb-1 border-2 border-ink rounded-xs"
                menu_style="background: var(--cream-light);"
                trigger=move || {
                    view! {
                        <div class="w-8 h-8 rounded-full flex items-center justify-center text-sm font-display font-bold shrink-0" style="background: var(--ink); color: var(--cream);">
                            {initial.get_value()}
                        </div>
                        <div class="flex-1 min-w-0">
                            <div class="text-[13px] font-semibold truncate">{display.get_value()}</div>
                            <div class="text-[11px] uppercase tracking-wider font-medium" style="color: var(--ink-40);">
                                {role.get_value()}
                            </div>
                        </div>
                        <span class="text-xs" style="color: var(--ink-40);">"\u{25BE}"</span>
                    }
                        .into_any()
                }
            >
                <button
                    type="button"
                    role="menuitem"
                    tabindex="-1"
                    class="w-full text-left flex items-center gap-2.5 px-4 py-2 text-sm hover-tint"
                    on:click=move |_| {
                        open.set(false);
                        drawer.set(true);
                    }
                >
                    <span class="w-[18px] text-center text-[15px]">"\u{25CB}"</span>
                    {move || tr("settings.account")}
                </button>
                <a
                    href=workspaces_href.get_value()
                    role="menuitem"
                    tabindex="-1"
                    class="flex items-center gap-2.5 px-4 py-2 text-sm hover-tint"
                >
                    <span class="w-[18px] text-center text-[15px]">"\u{229E}"</span>
                    {move || tr("sidebar.my_workspaces")}
                </a>
                <div class="border-t-2 border-ink"></div>
                <button
                    type="button"
                    role="menuitem"
                    tabindex="-1"
                    class="w-full text-left flex items-center gap-2.5 px-4 py-2 text-sm hover-tint"
                    style="color: var(--brick);"
                    on:click=move |_| {
                        open.set(false);
                        let redirect = login_href.get_value();
                        if crate::demo::is_demo() {
                            navigate.get_value()(&redirect, Default::default());
                        } else {
                            spawn_local(async move {
                                let url = format!("{}/auth/logout", crate::api::api_base());
                                let _ = gloo_net::http::Request::post(&url)
                                    .credentials(web_sys::RequestCredentials::Include)
                                    .header("X-CSRF-Token", &crate::auth::read_csrf_cookie())
                                    .send()
                                    .await;
                                if let Some(w) = web_sys::window() {
                                    let _ = w.location().set_href(&redirect);
                                }
                            });
                        }
                    }
                >
                    <span class="w-[18px] text-center text-[15px]">"\u{23FB}"</span>
                    {move || tr("settings.log_out")}
                </button>
            </DropdownMenu>
        </div>
    }
}
