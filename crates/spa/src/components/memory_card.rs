use leptos::prelude::*;
use crate::api::MemoryItem;
use crate::i18n::tr;

fn kind_color(kind: &str) -> &'static str {
    match kind {
        "fact"        => "var(--teal)",
        "preference"  => "var(--brick)",
        "skill"       => "#7C5CCC",
        "event"       => "#2B7FD4",
        "reminder"    => "#D4892B",
        _             => "var(--ink-40)",
    }
}

#[component]
pub fn MemoryCard(
    item: MemoryItem,
    on_edit: Callback<MemoryItem>,
    on_delete: Callback<String>,
    on_toggle_pin: Callback<String>,
) -> impl IntoView {
    let item_clone_edit = item.clone();
    let item_id_delete = item.id.clone();
    let item_id_pin = item.id.clone();
    let kind = item.kind.clone();
    let kind2 = item.kind.clone();
    let source = item.source.clone();
    let is_auto = source == "chat-auto";
    let pinned = item.pinned;

    view! {
        <div
            class="flex flex-col gap-2 px-4 py-3 border-2 border-ink rounded-sm"
            style="background: var(--cream-light); box-shadow: 3px 3px 0 #1A1A1A;"
        >
            <div class="flex items-start gap-2">
                // Kind badge
                <span
                    class="px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider rounded-sm flex-shrink-0 mt-0.5"
                    style:background=move || kind_color(&kind)
                    style="color: white;"
                >{let key = format!("memory.kind.{}", kind2); move || tr(&key)}</span>

                // Value text
                <p class="flex-1 text-sm font-medium leading-snug" style="color: var(--ink);">
                    {let v = item.value.clone(); move || tr(&v)}
                </p>
            </div>

            // Meta row
            <div class="flex items-center gap-2 text-[11px]" style="color: var(--ink-40);">
                {item.key.clone().map(|k| view! {
                    <span class="font-mono font-semibold" style="color: var(--ink-70);">{k}</span>
                    <span>"·"</span>
                })}
                {item.related_member.clone().map(|m| view! {
                    <span>{format!("@{}", m)}</span>
                    <span>"·"</span>
                })}
                <span>{item.source.clone()}</span>

                // AUTO badge
                {if is_auto {
                    view! {
                        <span
                            class="px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-widest rounded-sm"
                            style="background: var(--ink-08); color: var(--ink-70);"
                        >"AUTO"</span>
                    }.into_any()
                } else {
                    view! { <span></span> }.into_any()
                }}
            </div>

            // Action row
            <div class="flex items-center gap-2 pt-1 border-t" style="border-color: var(--ink-08);">
                <button
                    class="text-[11px] font-semibold px-2 py-0.5 border border-ink rounded-sm cursor-pointer"
                    style="color: var(--ink); background: transparent;"
                    on:click=move |_| on_edit.run(item_clone_edit.clone())
                >{move || tr("common.edit")}</button>

                <button
                    class="text-[11px] font-semibold px-2 py-0.5 border border-ink rounded-sm cursor-pointer"
                    on:click=move |_| on_toggle_pin.run(item_id_pin.clone())
                    style:background=move || if pinned { "var(--teal)" } else { "transparent" }
                    style:color=move || if pinned { "white" } else { "var(--ink)" }
                >
                    {move || if pinned { format!("📌 {}", tr("memory.pinned")) } else { tr("memory.action.pin") }}
                </button>

                <button
                    class="ml-auto text-[11px] font-semibold px-2 py-0.5 border rounded-sm cursor-pointer"
                    style="border-color: var(--brick); color: var(--brick); background: transparent;"
                    on:click=move |_| on_delete.run(item_id_delete.clone())
                >{move || tr("common.delete")}</button>
            </div>
        </div>
    }
}
