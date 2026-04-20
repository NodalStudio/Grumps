use leptos::prelude::*;
use crate::api::CalendarItem;
use super::item::CalItem;

fn parse_hour(s: &str) -> Option<u32> {
    // e.g. "2026-04-19T14:30:00Z" -> 14
    s.find('T').and_then(|i| s[i+1..].splitn(2, ':').next()).and_then(|h| h.parse().ok())
}

fn parse_date_parts(s: &str) -> Option<(i32, u32, u32)> {
    let parts: Vec<&str> = s.splitn(3, '-').collect();
    if parts.len() < 3 { return None; }
    let y = parts[0].parse().ok()?;
    let m = parts[1].parse().ok()?;
    let d = parts[2][..2.min(parts[2].len())].parse().ok()?;
    Some((y, m, d))
}

#[component]
pub fn WeekView(
    year: ReadSignal<i32>,
    month: ReadSignal<u32>,
    items: ReadSignal<Vec<CalendarItem>>,
    today_year: i32,
    today_month: u32,
    today_day: u32,
) -> impl IntoView {
    let day_names = ["Mon","Tue","Wed","Thu","Fri","Sat","Sun"];

    view! {
        <div class="flex flex-col flex-1 overflow-hidden">
            // Day headers (simplified — show day names for current week)
            <div class="grid border-b-2 border-ink" style="grid-template-columns: 40px repeat(7, 1fr);">
                <div></div>
                {day_names.iter().map(|d| view! {
                    <div class="py-2 text-center text-meta" style="color: var(--ink-40);">{*d}</div>
                }).collect_view()}
            </div>

            // Hour slots
            <div class="flex-1 overflow-y-auto">
                {(0..24u32).map(|hour| {
                    view! {
                        <div class="grid border-b border-ink/10" style="grid-template-columns: 40px repeat(7, 1fr); min-height: 48px;">
                            <div class="text-eyebrow font-mono pt-1 px-1" style="color: var(--ink-30);">
                                {format!("{:02}:00", hour)}
                            </div>
                            {(0..7u32).map(|_col| {
                                // TODO: place items in their correct hour slot based on date+hour
                                view! {
                                    <div class="border-r border-ink/10 last:border-r-0 p-0.5 flex flex-col gap-0.5">
                                        {move || {
                                            let y = year.get();
                                            let m = month.get();
                                            let all = items.get();
                                            let day_items: Vec<CalendarItem> = all.into_iter().filter(|i| {
                                                parse_date_parts(&i.starts_at).map(|(iy, im, _id)| iy == y && im == m).unwrap_or(false)
                                                && parse_hour(&i.starts_at).map(|h| h == hour).unwrap_or(false)
                                            }).collect();
                                            view! {
                                                <For
                                                    each=move || day_items.clone()
                                                    key=|i| i.id.clone()
                                                    children=|item| view! { <CalItem item=item compact=true /> }
                                                />
                                            }
                                        }}
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}
