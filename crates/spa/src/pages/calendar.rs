use crate::api::{use_api, CalendarItem};
use crate::components::calendar::agenda::AgendaView;
use crate::components::calendar::month::MonthView;
use crate::components::calendar::week::WeekView;
use crate::components::ui::tabs::{TabItem, Tabs};
use crate::i18n::tr;
#[allow(unused_imports)]
use js_sys;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

fn month_key(m: u32) -> &'static str {
    match m {
        1 => "month.jan",
        2 => "month.feb",
        3 => "month.mar",
        4 => "month.apr",
        5 => "month.may",
        6 => "month.jun",
        7 => "month.jul",
        8 => "month.aug",
        9 => "month.sep",
        10 => "month.oct",
        11 => "month.nov",
        _ => "month.dec",
    }
}

/// Today's date in the workspace timezone (never the browser's), so "today"
/// matches how calendar events are bucketed. See crate::datetime.
fn today() -> (i32, u32, u32) {
    crate::datetime::today_in_tz(&crate::datetime::use_timezone())
}

fn pad2(n: u32) -> String {
    format!("{:02}", n)
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

/// Weekday 0=Mon..6=Sun for a civil date (Sakamoto's algorithm).
fn weekday_mon0(y: i32, m: u32, d: u32) -> u32 {
    let t = [0i64, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y } as i64;
    let dow = (yy + yy / 4 - yy / 100 + yy / 400 + t[(m - 1) as usize] + d as i64) % 7; // 0=Sun
    ((dow + 6) % 7) as u32
}

/// Shift a civil date by `delta` days, rolling over month/year boundaries.
fn shift_days(mut y: i32, mut m: u32, d: u32, delta: i32) -> (i32, u32, u32) {
    let mut day = d as i32 + delta;
    loop {
        if day < 1 {
            m = if m == 1 {
                y -= 1;
                12
            } else {
                m - 1
            };
            day += days_in_month(y, m) as i32;
        } else if day > days_in_month(y, m) as i32 {
            day -= days_in_month(y, m) as i32;
            m = if m == 12 {
                y += 1;
                1
            } else {
                m + 1
            };
        } else {
            break;
        }
    }
    (y, m, day as u32)
}

/// The Monday of the week containing the given civil date.
fn week_monday(y: i32, m: u32, d: u32) -> (i32, u32, u32) {
    shift_days(y, m, d, -(weekday_mon0(y, m, d) as i32))
}

#[component]
pub fn CalendarPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let (today_y, today_m, today_d) = today();
    let (year, set_year) = signal(today_y);
    let (month, set_month) = signal(today_m);
    // Anchor day within the focused week (only meaningful in week view).
    let (day, set_day) = signal(today_d);
    let view_mode = RwSignal::new("month".to_string());

    // The 7 dates (Mon..Sun) of the week containing the anchor.
    let week_days = Memo::new(move |_| {
        let (my, mm, md) = week_monday(year.get(), month.get(), day.get());
        (0..7)
            .map(|i| shift_days(my, mm, md, i))
            .collect::<Vec<_>>()
    });

    // Fetch calendar items for the visible month
    let api = use_api();
    let cal_items = LocalResource::new(move || {
        let api = api.clone();
        let s = slug();
        let (from, to) = if view_mode.get() == "week" {
            // Cover the visible week plus a one-day margin so tz-shifted
            // boundary instants aren't missed; WeekView filters precisely.
            let days = week_days.get();
            let (fy, fm, fd) = shift_days(days[0].0, days[0].1, days[0].2, -1);
            let (ty, tm, td) = shift_days(days[6].0, days[6].1, days[6].2, 1);
            (
                format!("{}-{}-{}", fy, pad2(fm), pad2(fd)),
                format!("{}-{}-{}", ty, pad2(tm), pad2(td)),
            )
        } else {
            let y = year.get();
            let m = month.get();
            (
                format!("{}-{}-01", y, pad2(m)),
                format!("{}-{}-{}", y, pad2(m), pad2(days_in_month(y, m))),
            )
        };
        async move { api.list_calendar(&s, &from, &to).await.unwrap_or_default() }
    });

    // Derived signal for items (empty while loading)
    let (items_sig, set_items_sig) = signal(Vec::<CalendarItem>::new());
    Effect::new(move |_| {
        if let Some(data) = cal_items.get() {
            set_items_sig.set(data.clone());
        }
    });

    // Prev/Next move by one week in week view, otherwise by one month.
    let step = move |delta: i32| {
        if view_mode.get_untracked() == "week" {
            let (y, m, d) = shift_days(year.get(), month.get(), day.get(), delta * 7);
            set_year.set(y);
            set_month.set(m);
            set_day.set(d);
        } else {
            let (m, y) = (month.get(), year.get());
            let (ny, nm) = if delta < 0 {
                if m == 1 {
                    (y - 1, 12)
                } else {
                    (y, m - 1)
                }
            } else if m == 12 {
                (y + 1, 1)
            } else {
                (y, m + 1)
            };
            set_year.set(ny);
            set_month.set(nm);
        }
    };
    let prev_month = move |_| step(-1);
    let next_month = move |_| step(1);

    let view_tabs = vec![
        TabItem {
            value: "month".into(),
            label_key: "calendar.view.month".into(),
        },
        TabItem {
            value: "week".into(),
            label_key: "calendar.view.week".into(),
        },
        TabItem {
            value: "agenda".into(),
            label_key: "calendar.view.agenda".into(),
        },
    ];

    view! {
        <div class="px-8 pt-6 pb-5 border-b-2 border-ink flex items-end justify-between gap-4" style="background: var(--cream-light);">
            <div>
                <h2 class="font-display text-2xl font-extrabold tracking-tight">
                    {move || {
                        if view_mode.get() == "week" {
                            let days = week_days.get();
                            let (sy, sm, sd) = days[0];
                            let (ey, em, ed) = days[6];
                            if sm == em {
                                format!("{} {}–{}, {}", tr(month_key(sm)), sd, ed, sy)
                            } else {
                                format!("{} {} – {} {}, {}", tr(month_key(sm)), sd, tr(month_key(em)), ed, ey)
                            }
                        } else {
                            format!("{} {}", tr(month_key(month.get())), year.get())
                        }
                    }}
                </h2>
                <p class="text-[13px] mt-0.5" style="color: var(--ink-40);">{move || tr("page.calendar.title")}</p>
            </div>
            // Navigation + view tabs
            <div class="flex items-center gap-2">
                <button
                    class="px-3 py-1.5 text-sm font-bold border-2 border-ink rounded-xs cursor-pointer"
                    on:click=prev_month
                >"< "{move || tr("calendar.prev")}</button>
                <button
                    class="px-3 py-1.5 text-sm font-bold border-2 border-ink rounded-xs cursor-pointer"
                    on:click=next_month
                >{move || tr("calendar.next")}" >"</button>
                <Tabs
                    value=view_mode
                    tabs=view_tabs
                    aria_label=Signal::derive(move || tr("calendar.view.aria"))
                    class="gap-0 flex-nowrap border-2 border-ink rounded-xs overflow-hidden ms-2"
                    tab_class="py-1.5 font-bold border-0 rounded-none"
                />
            </div>
        </div>

        // Calendar body
        <div class="flex-1 flex flex-col overflow-hidden">
            // TODO: drag & drop
            {move || {
                match view_mode.get().as_str() {
                    "week" => view! {
                        <WeekView
                            week_days=week_days
                            items=items_sig
                            today_year=today_y
                            today_month=today_m
                            today_day=today_d
                        />
                    }.into_any(),
                    "agenda" => view! {
                        <AgendaView items=items_sig />
                    }.into_any(),
                    _ => view! {
                        <MonthView
                            year=year
                            month=month
                            items=items_sig
                            today_year=today_y
                            today_month=today_m
                            today_day=today_d
                        />
                    }.into_any(),
                }
            }}
        </div>
    }
}
