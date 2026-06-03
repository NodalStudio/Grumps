use crate::auth::{use_session, WorkspaceRef};
use crate::components::ui::dropdown_menu::{DropdownMenu, MenuAlign};
use crate::i18n::tr;
use leptos::prelude::*;

#[component]
pub fn WorkspaceSwitcher(current_slug: String) -> impl IntoView {
    let session = use_session().unwrap_or_default();
    let open = RwSignal::new(false);
    let workspaces = session.workspaces.clone();
    let current = workspaces
        .iter()
        .find(|w| w.slug == current_slug)
        .cloned()
        .unwrap_or_default();
    let current_label = current
        .name
        .clone()
        .map(|n| tr(&n))
        .unwrap_or_else(|| current.slug.clone());
    let cur_for_render = current_slug.clone();
    // Park the owned data in `Copy` stores so the trigger/children closures
    // (`ViewFn` / `ChildrenFn`, both `Fn`) can read them on every call without
    // moving the captures out of the surrounding closure.
    let current_label = StoredValue::new(current_label);
    let workspaces = StoredValue::new(workspaces);
    let cur_for_render = StoredValue::new(cur_for_render);

    view! {
        <DropdownMenu
            open=open
            align=MenuAlign::Start
            container_class="flex w-full"
            aria_label=Signal::derive(|| tr("workspace.switch_label"))
            trigger_class="w-full text-left p-3 border-2 border-ink rounded-xs cursor-pointer flex justify-between items-center"
            trigger_style="background: var(--cream-light);"
            menu_class="inset-inline-0 top-full mt-1 border-2 border-ink rounded-xs"
            menu_style="background: var(--cream-light);"
            trigger=move || {
                let label = current_label.get_value();
                view! {
                    <span class="font-display text-sm font-bold">{label}</span>
                    <span class="text-xs" style="color: var(--ink-40);">
                        "▼"
                    </span>
                }
                    .into_any()
            }
        >
            {move || {
                let cur = cur_for_render.get_value();
                let rows = workspaces
                    .get_value()
                    .into_iter()
                    .map(|ws| view_row(ws, &cur))
                    .collect_view();
                view! {
                    {rows}
                    <div class="border-t-2 border-ink"></div>
                    <a
                        href="https://t.me/HeyGrumpsBot?startgroup=true"
                        target="_blank"
                        role="menuitem"
                        tabindex="-1"
                        class="block px-3 py-2 text-xs font-bold uppercase tracking-wider hover-tint"
                    >
                        {move || tr("workspace.add_to_group")}
                    </a>
                }
            }}
        </DropdownMenu>
    }
}

fn view_row(ws: WorkspaceRef, current_slug: &str) -> impl IntoView {
    let is_current = ws.slug == current_slug;
    let plat = match ws.platform.as_str() {
        "telegram" => "TG",
        "whatsapp" => "WA",
        "discord" => "DC",
        _ => "",
    };
    // Platform codes (TG/WA/DC) are brand abbreviations, not prose — left
    // hardcoded. The shape label is a common noun, so it goes through i18n.
    let shape = if ws.is_dm {
        tr("workspace.type_dm")
    } else {
        tr("workspace.type_group")
    };
    let badge = format!("{} {}", plat, shape);
    let name = ws
        .name
        .clone()
        .map(|n| tr(&n))
        .unwrap_or_else(|| ws.slug.clone());
    // Prepend the demo router base (empty in the real app) so a switch in the
    // embedded demo iframe stays within the router instead of escaping `/demo/`.
    let href = format!("{}/w/{}", crate::demo::router_base(), ws.slug);
    view! {
        <a
            href=href
            role="menuitem"
            tabindex="-1"
            data-current=is_current.to_string()
            class="block px-3 py-2 flex justify-between items-center hover-tint"
            class:font-bold=is_current
        >
            <span class="text-sm">{name}</span>
            <span class="text-[10px] uppercase tracking-wider" style="color: var(--ink-40);">
                {badge}
            </span>
        </a>
    }
}
