use crate::auth::{use_session, WorkspaceRef};
use crate::i18n::tr;
use leptos::prelude::*;

#[component]
pub fn WorkspaceSwitcher(current_slug: String) -> impl IntoView {
    let session = use_session().unwrap_or_default();
    let (open, set_open) = signal(false);
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

    view! {
        <div class="relative">
            <button class="w-full text-left p-3 border-2 border-ink rounded-xs cursor-pointer flex justify-between items-center"
                style="background: var(--cream-light);"
                on:click=move |_| set_open.update(|v| *v = !*v)>
                <span class="font-display text-sm font-bold">{current_label}</span>
                <span class="text-xs" style="color: var(--ink-40);">"▼"</span>
            </button>
            <Show when=move || open.get()>
                <ul class="absolute top-full left-0 right-0 mt-1 border-2 border-ink rounded-xs z-10" style="background: var(--cream-light);">
                    {workspaces.iter().cloned().map(|ws| view_row(ws, &cur_for_render)).collect_view()}
                    <li class="border-t-2 border-ink"></li>
                    <li><a href="https://t.me/HeyGrumpsBot?startgroup=true" target="_blank" class="block px-3 py-2 text-xs font-bold uppercase tracking-wider hover:underline">"+ Add Grumps to a group"</a></li>
                </ul>
            </Show>
        </div>
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
    let shape = if ws.is_dm { "DM" } else { "GROUP" };
    let badge = format!("{} {}", plat, shape);
    let name = ws
        .name
        .clone()
        .map(|n| tr(&n))
        .unwrap_or_else(|| ws.slug.clone());
    let href = format!("/w/{}", ws.slug);
    view! {
        <li>
            <a href=href class="block px-3 py-2 flex justify-between items-center" class:font-bold=is_current>
                <span class="text-sm">{name}</span>
                <span class="text-[10px] uppercase tracking-wider" style="color: var(--ink-40);">{badge}</span>
            </a>
        </li>
    }
}
