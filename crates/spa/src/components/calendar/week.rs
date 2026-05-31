use super::item::CalItem;
use crate::api::CalendarItem;
use leptos::prelude::*;

const DAY_KEYS: [&str; 7] = [
    "dow.mon", "dow.tue", "dow.wed", "dow.thu", "dow.fri", "dow.sat", "dow.sun",
];

fn date_key(y: i32, m: u32, d: u32) -> String {
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Week grid: 7 day-columns (the dates in `week_days`, Mon..Sun) × 24 hour-rows,
/// plus an all-day strip. Items are placed by their **workspace-tz** calendar
/// day and hour (all-day items are floating civil dates) — see crate::datetime.
#[component]
pub fn WeekView(
    week_days: Memo<Vec<(i32, u32, u32)>>,
    items: ReadSignal<Vec<CalendarItem>>,
    today_year: i32,
    today_month: u32,
    today_day: u32,
) -> impl IntoView {
    view! {
        <div class="flex flex-col flex-1 overflow-hidden">
            // Day headers with each column's date.
            <div class="grid border-b-2 border-ink" style="grid-template-columns: 52px repeat(7, 1fr);">
                <div></div>
                {move || {
                    let days = week_days.get();
                    (0..7usize).map(|i| {
                        let (cy, cm, cd) = days[i];
                        let is_today = cy == today_year && cm == today_month && cd == today_day;
                        view! {
                            <div class="py-2 text-center border-l border-ink/10">
                                <div class="text-[10px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">
                                    {move || crate::i18n::tr(DAY_KEYS[i])}
                                </div>
                                <div
                                    class="text-sm font-bold mt-0.5 mx-auto inline-flex items-center justify-center w-6 h-6 rounded-full"
                                    style:background=if is_today { "var(--brick)" } else { "transparent" }
                                    style:color=if is_today { "white" } else { "var(--ink)" }
                                >{cd}</div>
                            </div>
                        }
                    }).collect_view()
                }}
            </div>

            // All-day strip.
            <div class="grid border-b border-ink/20" style="grid-template-columns: 52px repeat(7, 1fr);">
                <div class="text-[9px] font-mono pt-1.5 px-1 uppercase" style="color: var(--ink-30);">
                    {move || crate::i18n::tr("calendar.allday")}
                </div>
                {move || {
                    let days = week_days.get();
                    let tz = crate::datetime::use_timezone();
                    let all = items.get();
                    (0..7usize).map(|i| {
                        let (cy, cm, cd) = days[i];
                        let key = date_key(cy, cm, cd);
                        let cell: Vec<CalendarItem> = all.iter()
                            .filter(|it| it.all_day
                                && crate::datetime::item_day_key(&it.starts_at, true, &tz) == key)
                            .cloned()
                            .collect();
                        view! {
                            <div class="border-l border-ink/10 p-0.5 flex flex-col gap-0.5 min-h-[28px]">
                                {cell.into_iter().map(|item| view! { <CalItem item=item compact=true /> }).collect_view()}
                            </div>
                        }
                    }).collect_view()
                }}
            </div>

            // Hour grid.
            <div class="flex-1 overflow-y-auto">
                {move || {
                    let days = week_days.get();
                    let tz = crate::datetime::use_timezone();
                    let all = items.get();
                    (0..24u32).map(|hour| {
                        let days = days.clone();
                        let tz = tz.clone();
                        let all = all.clone();
                        view! {
                            <div class="grid border-b border-ink/10" style="grid-template-columns: 52px repeat(7, 1fr); min-height: 44px;">
                                <div class="text-[10px] font-mono pt-1 px-1" style="color: var(--ink-30);">
                                    {format!("{:02}:00", hour)}
                                </div>
                                {(0..7usize).map(|i| {
                                    let (cy, cm, cd) = days[i];
                                    let key = date_key(cy, cm, cd);
                                    let cell: Vec<CalendarItem> = all.iter()
                                        .filter(|it| !it.all_day
                                            && crate::datetime::item_day_key(&it.starts_at, false, &tz) == key
                                            && crate::datetime::hour_in_tz(&it.starts_at, &tz) == Some(hour))
                                        .cloned()
                                        .collect();
                                    view! {
                                        <div class="border-l border-ink/10 p-0.5 flex flex-col gap-0.5">
                                            {cell.into_iter().map(|item| view! { <CalItem item=item compact=true /> }).collect_view()}
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        }
                    }).collect_view()
                }}
            </div>
        </div>
    }
}
