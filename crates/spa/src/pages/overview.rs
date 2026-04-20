use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
#[allow(unused_imports)]
use js_sys;
use crate::auth::use_auth;
use crate::api::{StatusCounts, CalendarItem, MemoryItem};
use crate::components::header::PageHeader;
use crate::i18n::tr;

fn today_str() -> (i32, u32, u32) {
    let d = js_sys::Date::new_0();
    (d.get_full_year() as i32, d.get_month() + 1, d.get_date())
}

fn parse_date_key(s: &str) -> String {
    s.get(..10).unwrap_or(s).to_string()
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1|3|5|7|8|10|12 => 31,
        4|6|9|11 => 30,
        2 => if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 29 } else { 28 },
        _ => 30,
    }
}

fn pad2(n: u32) -> String { format!("{:02}", n) }

#[component]
pub fn OverviewPage() -> impl IntoView {
    let auth = use_auth();
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let (today_y, today_m, today_d) = today_str();

    // Status counts
    let api1 = auth.api.clone();
    let counts = LocalResource::new(move || {
        let api = api1.clone();
        let s = slug();
        async move { api.get_workspace_info(&s).await.ok() }
    });

    // Week calendar items
    let api2 = auth.api.clone();
    let week_items = LocalResource::new(move || {
        let api = api2.clone();
        let s = slug();
        let y = today_y;
        let m = today_m;
        let last = days_in_month(y, m);
        let from = format!("{}-{}-01", y, pad2(m));
        let to   = format!("{}-{}-{}", y, pad2(m), pad2(last));
        async move { api.list_calendar(&s, &from, &to).await.unwrap_or_default() }
    });

    // Pinned memories
    let api3 = auth.api.clone();
    let memories = LocalResource::new(move || {
        let api = api3.clone();
        let s = slug();
        async move {
            let all = api.list_memory(&s).await.unwrap_or_default();
            let pinned: Vec<MemoryItem> = all.into_iter().filter(|m| m.pinned).take(5).collect();
            pinned
        }
    });

    // Build 7-day grid (Mon..Sun of the current week)
    // We find what day of week today is, then figure Mon offset
    let js_dow = js_sys::Date::new_0().get_day(); // 0=Sun..6=Sat
    let mon_offset = ((js_dow as i32 + 6) % 7) as u32; // 0=today is Mon..6=today is Sun

    view! {
        <PageHeader title=slug() subtitle=tr("page.overview.title") />
        <div class="flex-1 overflow-y-auto p-8">
            // Stat blocks
            <Suspense fallback=|| view! { <div class="text-sm" style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
                {move || counts.get().map(|data| {
                    let c = (*data).clone().unwrap_or(StatusCounts { open_todos: 0, done_this_week: 0, notes: 0, files: 0 });
                    view! {
                        <div class="flex border-2 border-ink rounded-sm overflow-hidden mb-6" style="background: var(--cream-light);">
                            <StatBlock number=c.open_todos label=tr("overview.stat.open_todos") color="var(--brick)" />
                            <StatBlock number=c.notes label=tr("overview.stat.notes") color="var(--ink)" />
                            <StatBlock number=c.files label=tr("overview.stat.files") color="var(--ink)" />
                            <StatBlock number=c.done_this_week label=tr("overview.stat.done_this_week") color="var(--teal)" />
                        </div>
                    }
                })}
            </Suspense>

            // Bottom widgets row
            <div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
                // Widget: this week's calendar
                <div class="border-2 border-ink rounded-sm p-4" style="background: var(--cream-light); box-shadow: 3px 3px 0 #1A1A1A;">
                    <h3 class="font-display text-base font-bold mb-3">{move || tr("overview.this_week")}</h3>
                    <Suspense fallback=|| view! { <div class="text-sm" style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
                        {move || week_items.get().map(|data| {
                            let items: Vec<CalendarItem> = (*data).clone();
                            let day_names = ["L","M","M","J","V","S","D"];

                            view! {
                                <div class="grid grid-cols-7 gap-1">
                                    {(0..7u32).map(|col| {
                                        // Calculate actual date for this column
                                        let items = items.clone();
                                        let offset = col as i32 - mon_offset as i32;
                                        let actual_day = today_d as i32 + offset;
                                        let (cell_m, cell_d) = if actual_day < 1 {
                                            let prev_last = days_in_month(today_y, if today_m == 1 { 12 } else { today_m - 1 }) as i32;
                                            (if today_m == 1 { 12 } else { today_m - 1 }, (prev_last + actual_day) as u32)
                                        } else {
                                            let last = days_in_month(today_y, today_m) as i32;
                                            if actual_day > last {
                                                (if today_m == 12 { 1 } else { today_m + 1 }, (actual_day - last) as u32)
                                            } else {
                                                (today_m, actual_day as u32)
                                            }
                                        };
                                        let is_today = cell_m == today_m && cell_d == today_d;
                                        let date_key = format!("{}-{}-{}", today_y, pad2(cell_m), pad2(cell_d));
                                        let day_items: Vec<CalendarItem> = items.into_iter()
                                            .filter(|i| parse_date_key(&i.starts_at) == date_key)
                                            .collect();

                                        view! {
                                            <div class="flex flex-col items-center gap-1">
                                                <div
                                                    class="w-6 h-6 rounded-full flex items-center justify-center text-[11px] font-bold"
                                                    style:background=if is_today { "var(--brick)" } else { "transparent" }
                                                    style:color=if is_today { "white" } else { "var(--ink-40)" }
                                                >{day_names[col as usize]}</div>
                                                <div
                                                    class="w-6 h-6 rounded-full flex items-center justify-center text-[11px] font-semibold"
                                                    style:color=if is_today { "var(--brick)" } else { "var(--ink-70)" }
                                                >{cell_d}</div>
                                                // Color dots for items
                                                <div class="flex flex-col gap-0.5 w-full">
                                                    {day_items.into_iter().take(3).map(|item| {
                                                        let color = item.color.clone();
                                                        view! {
                                                            <div
                                                                class="w-full h-1 rounded-full"
                                                                style:background=color
                                                                title=item.title.clone()
                                                            ></div>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }
                        })}
                    </Suspense>
                </div>

                // Widget: Ce que je sais sur le groupe
                <div class="border-2 border-ink rounded-sm p-4" style="background: var(--cream-light); box-shadow: 3px 3px 0 #1A1A1A;">
                    <h3 class="font-display text-base font-bold mb-3">{move || tr("overview.what_i_know")}</h3>
                    <Suspense fallback=|| view! { <div class="text-sm" style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
                        {move || memories.get().map(|data| {
                            let items: Vec<MemoryItem> = (*data).clone();
                            if items.is_empty() {
                                return view! {
                                    <p class="text-sm" style="color: var(--ink-40);">{move || tr("overview.nothing_pinned")}</p>
                                }.into_any();
                            }
                            view! {
                                <div class="flex flex-col gap-2">
                                    {items.into_iter().map(|item| view! {
                                        <div class="flex items-start gap-2 text-sm">
                                            <span
                                                class="px-1.5 py-0.5 text-[9px] font-bold uppercase rounded-sm flex-shrink-0"
                                                style="background: var(--brick); color: white;"
                                            >{item.kind.clone()}</span>
                                            <span style="color: var(--ink);">{item.value.clone()}</span>
                                        </div>
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        })}
                    </Suspense>
                </div>
            </div>
        </div>
    }
}

#[component]
fn StatBlock(number: i64, label: String, color: &'static str) -> impl IntoView {
    view! {
        <div class="flex-1 p-4 border-r-2 border-ink last:border-r-0">
            <div class="font-display text-3xl font-extrabold" style:color=color>{number}</div>
            <div class="text-[11px] uppercase tracking-wider font-semibold mt-1" style="color: var(--ink-40);">{label}</div>
        </div>
    }
}
