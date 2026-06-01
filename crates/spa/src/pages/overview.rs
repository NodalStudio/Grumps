use crate::api::{use_api, CalendarItem, MemoryItem, StatusCounts};
use crate::components::header::PageHeader;
use crate::i18n::tr;
#[allow(unused_imports)]
use js_sys;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

fn today_str() -> (i32, u32, u32) {
    // Workspace timezone, never the browser's (see crate::datetime).
    crate::datetime::today_in_tz(&crate::datetime::use_timezone())
}

/// Weekday of a civil date, 0=Mon..6=Sun (Sakamoto's algorithm). Derived from
/// the date itself so it stays consistent with the workspace-tz `today`.
fn weekday_mon0(y: i32, m: u32, d: u32) -> u32 {
    let t = [0i64, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y } as i64;
    let dow = (yy + yy / 4 - yy / 100 + yy / 400 + t[(m - 1) as usize] + d as i64) % 7; // 0=Sun
    ((dow + 6) % 7) as u32
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn pad2(n: u32) -> String {
    format!("{:02}", n)
}

#[component]
pub fn OverviewPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let (today_y, today_m, today_d) = today_str();

    // Status counts + workspace name (single fetch).
    let api1 = use_api();
    let info = LocalResource::new(move || {
        let api = api1.clone();
        let s = slug();
        async move { api.get_workspace_info(&s).await.ok() }
    });

    // Week calendar items
    let api2 = use_api();
    let week_items = LocalResource::new(move || {
        let api = api2.clone();
        let s = slug();
        let y = today_y;
        let m = today_m;
        let last = days_in_month(y, m);
        let from = format!("{}-{}-01", y, pad2(m));
        let to = format!("{}-{}-{}", y, pad2(m), pad2(last));
        async move { api.list_calendar(&s, &from, &to).await.unwrap_or_default() }
    });

    // Pinned memories
    let api3 = use_api();
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
    let mon_offset = weekday_mon0(today_y, today_m, today_d); // 0=today is Mon..6=Sun

    view! {
        <PageHeader title=move || {
            let s = slug();
            info.get()
                .and_then(|data| data.clone())
                .and_then(|d| d.name)
                .unwrap_or(s)
        } subtitle=tr("page.overview.title") />
        <div class="flex-1 overflow-y-auto p-8">
            // Stat blocks
            <Suspense fallback=|| view! { <div class="text-sm" style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
                {move || info.get().map(|data| {
                    let c = data.clone().map(|d| d.stats).unwrap_or(StatusCounts { open_todos: 0, done_this_week: 0, notes: 0, files: 0 });
                    view! {
                        <div class="flex border-2 border-ink rounded-xs overflow-hidden mb-6" style="background: var(--cream-light);">
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
                <div class="border-2 border-ink rounded-xs p-4" style="background: var(--cream-light); box-shadow: 3px 3px 0 #1A1A1A;">
                    <h3 class="font-display text-base font-bold mb-3">{move || tr("overview.this_week")}</h3>
                    <Suspense fallback=|| view! { <div class="text-sm" style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
                        {move || week_items.get().map(|data| {
                            let items: Vec<CalendarItem> = data.clone();
                            let tz = crate::datetime::use_timezone();
                            let day_names = [
                                tr("overview.day.short.mon"),
                                tr("overview.day.short.tue"),
                                tr("overview.day.short.wed"),
                                tr("overview.day.short.thu"),
                                tr("overview.day.short.fri"),
                                tr("overview.day.short.sat"),
                                tr("overview.day.short.sun"),
                            ];

                            view! {
                                <div class="grid grid-cols-7 gap-1">
                                    {(0..7u32).map(|col| {
                                        // Calculate actual date for this column
                                        let items = items.clone();
                                        let tz = tz.clone();
                                        let offset = col as i32 - mon_offset as i32;
                                        let actual_day = today_d as i32 + offset;
                                        // Track the cell's YEAR too: days from the
                                        // adjacent month at a Jan/Dec boundary belong to
                                        // the previous/next year, else date_key uses the
                                        // wrong year and never matches an event.
                                        let (cell_y, cell_m, cell_d) = if actual_day < 1 {
                                            let (py, pm) = if today_m == 1 { (today_y - 1, 12) } else { (today_y, today_m - 1) };
                                            let prev_last = days_in_month(py, pm) as i32;
                                            (py, pm, (prev_last + actual_day) as u32)
                                        } else {
                                            let last = days_in_month(today_y, today_m) as i32;
                                            if actual_day > last {
                                                let (ny, nm) = if today_m == 12 { (today_y + 1, 1) } else { (today_y, today_m + 1) };
                                                (ny, nm, (actual_day - last) as u32)
                                            } else {
                                                (today_y, today_m, actual_day as u32)
                                            }
                                        };
                                        let is_today = cell_y == today_y && cell_m == today_m && cell_d == today_d;
                                        let date_key = format!("{}-{}-{}", cell_y, pad2(cell_m), pad2(cell_d));
                                        let day_items: Vec<CalendarItem> = items.into_iter()
                                            .filter(|i| crate::datetime::item_day_key(&i.starts_at, i.all_day, &tz) == date_key)
                                            .collect();

                                        view! {
                                            <div class="flex flex-col items-center gap-1">
                                                <div
                                                    class="w-6 h-6 rounded-full flex items-center justify-center text-[11px] font-bold"
                                                    style:background=if is_today { "var(--brick)" } else { "transparent" }
                                                    style:color=if is_today { "white" } else { "var(--ink-40)" }
                                                >{day_names[col as usize].clone()}</div>
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
                <div class="border-2 border-ink rounded-xs p-4" style="background: var(--cream-light); box-shadow: 3px 3px 0 #1A1A1A;">
                    <h3 class="font-display text-base font-bold mb-3">{move || tr("overview.what_i_know")}</h3>
                    <Suspense fallback=|| view! { <div class="text-sm" style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
                        {move || memories.get().map(|data| {
                            let items: Vec<MemoryItem> = data.clone();
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
                                                class="px-1.5 py-0.5 text-[9px] font-bold uppercase rounded-xs shrink-0"
                                                style="background: var(--brick); color: white;"
                                            >{item.kind.clone()}</span>
                                            <span style="color: var(--ink);">{let v = item.value.clone(); move || tr(&v)}</span>
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
