use crate::api::ScheduledActionItem;
use crate::i18n::{tr, tr_n};
use leptos::prelude::*;

fn type_icon(action_type: &str) -> &'static str {
    match action_type {
        "message" => "\u{00B6}",  // ¶
        "reminder" => "\u{25F7}", // ◷
        "recap" => "\u{25A4}",    // ▤
        "task" => "\u{2610}",     // ☐
        "webhook" => "\u{229E}",  // ⊞
        _ => "\u{25CA}",          // ◊
    }
}

fn status_color(status: &str) -> &'static str {
    match status {
        "active" => "var(--teal)",
        "paused" => "var(--ink-40)",
        "done" => "var(--ink-15)",
        "failed" => "var(--brick)",
        _ => "var(--ink-40)",
    }
}

#[component]
pub fn ScheduledCard(
    item: ScheduledActionItem,
    on_edit: Callback<ScheduledActionItem>,
    on_delete: Callback<String>,
    on_execute: Callback<String>,
) -> impl IntoView {
    let item_edit = item.clone();
    let item_id_del = item.id.clone();
    let item_id_exec = item.id.clone();
    let atype = item.action_type.clone();
    let status = item.status.clone();
    let status2 = item.status.clone();

    view! {
        <div
            class="flex flex-col gap-2 px-4 py-3 border-2 border-ink rounded-sm"
            style="background: var(--cream-light); box-shadow: 3px 3px 0 #1A1A1A;"
        >
            <div class="flex items-start gap-2">
                <span class="text-base flex-shrink-0 w-5 text-center" style="color: var(--ink-70);">{type_icon(&atype)}</span>
                <div class="flex-1 min-w-0">
                    <div class="font-semibold text-sm" style="color: var(--ink);">{let t = item.title.clone(); move || tr(&t)}</div>
                    <div class="flex items-center gap-2 mt-0.5 text-[11px]" style="color: var(--ink-40);">
                        <span class="font-medium">{let k = format!("schedule.type.{}", atype); move || tr(&k)}</span>
                        <span>"·"</span>
                        <span>{item.trigger_at.clone()}</span>
                        {item.recurrence.clone().map(|r| view! {
                            <span>"·"</span>
                            <span class="font-mono text-[10px]">{r}</span>
                        })}
                    </div>
                </div>
                // Status badge
                <span
                    class="px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider rounded-sm flex-shrink-0"
                    style:background=move || status_color(&status)
                    style="color: white;"
                >{let k = format!("schedule.status.{}", status2); move || tr(&k)}</span>
            </div>

            <div class="text-[11px]" style="color: var(--ink-40);">
                {let n = item.fire_count; move || tr_n("schedule.fired", n, &[])}
            </div>

            <div class="flex items-center gap-2 pt-1 border-t" style="border-color: var(--ink-08);">
                <button
                    class="text-[11px] font-semibold px-2 py-0.5 border border-ink rounded-sm cursor-pointer"
                    on:click=move |_| on_edit.run(item_edit.clone())
                >{move || tr("common.edit")}</button>
                <button
                    class="text-[11px] font-semibold px-2 py-0.5 border rounded-sm cursor-pointer"
                    style="border-color: var(--teal); color: var(--teal); background: transparent;"
                    on:click=move |_| on_execute.run(item_id_exec.clone())
                >{move || tr("schedule.action.execute")}</button>
                <button
                    class="ml-auto text-[11px] font-semibold px-2 py-0.5 border rounded-sm cursor-pointer"
                    style="border-color: var(--brick); color: var(--brick); background: transparent;"
                    on:click=move |_| on_delete.run(item_id_del.clone())
                >{move || tr("common.delete")}</button>
            </div>
        </div>
    }
}
