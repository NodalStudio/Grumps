use leptos::prelude::*;
use crate::api::ScheduledActionItem;
use crate::components::Icon;

fn type_icon(action_type: &str) -> &'static str {
    match action_type {
        "message"    => "message",
        "reminder"   => "scheduled",
        "recap"      => "recap",
        "task"       => "task-check",
        "webhook"    => "webhook",
        _            => "bolt",
    }
}

fn status_color(status: &str) -> &'static str {
    match status {
        "active"    => "var(--teal)",
        "paused"    => "var(--ink-40)",
        "done"      => "var(--ink-15)",
        "failed"    => "var(--brick)",
        _           => "var(--ink-40)",
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
                <Icon name=type_icon(&atype) class="size-4 text-muted flex-shrink-0"/>
                <div class="flex-1 min-w-0">
                    <div class="font-semibold text-sm" style="color: var(--ink);">{item.title.clone()}</div>
                    <div class="flex items-center gap-2 mt-0.5 text-[11px]" style="color: var(--ink-40);">
                        <span class="capitalize font-medium">{atype.clone()}</span>
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
                >{status2.clone()}</span>
            </div>

            <div class="text-[11px]" style="color: var(--ink-40);">
                {format!("Fired {} time{}", item.fire_count, if item.fire_count == 1 { "" } else { "s" })}
            </div>

            <div class="flex items-center gap-2 pt-1 border-t" style="border-color: var(--ink-08);">
                <button
                    class="text-[11px] font-semibold px-2 py-0.5 border border-ink rounded-sm cursor-pointer"
                    on:click=move |_| on_edit.run(item_edit.clone())
                >"Edit"</button>
                <button
                    class="text-[11px] font-semibold px-2 py-0.5 border rounded-sm cursor-pointer"
                    style="border-color: var(--teal); color: var(--teal); background: transparent;"
                    on:click=move |_| on_execute.run(item_id_exec.clone())
                >"Execute now"</button>
                <button
                    class="ml-auto text-[11px] font-semibold px-2 py-0.5 border rounded-sm cursor-pointer"
                    style="border-color: var(--brick); color: var(--brick); background: transparent;"
                    on:click=move |_| on_delete.run(item_id_del.clone())
                >"Delete"</button>
            </div>
        </div>
    }
}
