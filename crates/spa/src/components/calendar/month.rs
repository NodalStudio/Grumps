use leptos::prelude::*;
use crate::api::CalendarItem;
use super::item::CalItem;

/// Returns (year, month, day) from an ISO date string "YYYY-MM-DD..."
fn parse_date(s: &str) -> Option<(i32, u32, u32)> {
    let parts: Vec<&str> = s.splitn(3, '-').collect();
    if parts.len() < 3 { return None; }
    let y = parts[0].parse().ok()?;
    let m = parts[1].parse().ok()?;
    let d = parts[2][..2.min(parts[2].len())].parse().ok()?;
    Some((y, m, d))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1|3|5|7|8|10|12 => 31,
        4|6|9|11 => 30,
        2 => if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 29 } else { 28 },
        _ => 30,
    }
}

/// Returns weekday 0=Mon..6=Sun for the 1st of a given month (Zeller-ish, simplified)
fn first_weekday(year: i32, month: u32) -> u32 {
    // Use Tomohiko Sakamoto's algorithm
    let y = if month < 3 { year - 1 } else { year } as u32;
    let m = month;
    let d = 1u32;
    let t = [0u32, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let dow = (y + y/4 - y/100 + y/400 + t[(m-1) as usize] + d) % 7;
    // dow: 0=Sun, convert to Mon=0
    (dow + 6) % 7
}

#[component]
pub fn MonthView(
    year: ReadSignal<i32>,
    month: ReadSignal<u32>,
    items: ReadSignal<Vec<CalendarItem>>,
    today_year: i32,
    today_month: u32,
    today_day: u32,
) -> impl IntoView {
    const DAY_KEYS: [&str; 7] = ["dow.mon", "dow.tue", "dow.wed", "dow.thu", "dow.fri", "dow.sat", "dow.sun"];

    view! {
        <div class="flex flex-col flex-1 overflow-hidden">
            // Day headers
            <div class="grid grid-cols-7 border-b-2 border-ink">
                {DAY_KEYS.iter().copied().map(|k| view! {
                    <div class="py-2 text-center text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">{move || crate::i18n::tr(k)}</div>
                }).collect_view()}
            </div>

            // Cells
            {move || {
                let y = year.get();
                let m = month.get();
                let days = days_in_month(y, m);
                let first_wd = first_weekday(y, m);
                let total_cells = first_wd + days;
                let rows = (total_cells + 6) / 7;
                let all_items = items.get();

                (0..rows).map(|row| {
                    let all_items = all_items.clone();
                    view! {
                        <div class="grid grid-cols-7 flex-1 border-b border-ink/20 min-h-[80px]">
                            {(0..7u32).map(|col| {
                                let cell = row * 7 + col;
                                let all_items = all_items.clone();
                                if cell < first_wd || cell >= first_wd + days {
                                    view! { <div class="border-r border-ink/10 last:border-r-0" style="background: var(--cream-light); opacity: 0.4;"></div> }.into_any()
                                } else {
                                    let day = cell - first_wd + 1;
                                    let is_today = y == today_year && m == today_month && day == today_day;
                                    let day_items: Vec<CalendarItem> = all_items.into_iter().filter(|i| {
                                        parse_date(&i.starts_at).map(|(iy, im, id)| iy == y && im == m && id == day).unwrap_or(false)
                                    }).collect();

                                    view! {
                                        <div class="border-r border-ink/10 last:border-r-0 p-1 flex flex-col gap-0.5 min-h-[80px]">
                                            <div
                                                class="text-xs font-bold self-start px-1 rounded-sm"
                                                style:background=if is_today { "var(--brick)" } else { "transparent" }
                                                style:color=if is_today { "white" } else { "var(--ink)" }
                                            >{day}</div>
                                            <div class="flex flex-col gap-0.5 overflow-hidden">
                                                <For
                                                    each=move || day_items.clone()
                                                    key=|i| i.id.clone()
                                                    children=|item| view! { <CalItem item=item compact=true /> }
                                                />
                                            </div>
                                        </div>
                                    }.into_any()
                                }
                            }).collect_view()}
                        </div>
                    }
                }).collect_view()
            }}
        </div>
    }
}
