use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
#[allow(unused_imports)]
use js_sys;
use crate::auth::use_auth;
use crate::api::CalendarItem;
use crate::components::calendar::month::MonthView;
use crate::components::calendar::week::WeekView;
use crate::components::calendar::agenda::AgendaView;

fn month_name(m: u32) -> &'static str {
    match m {
        1=>"January",2=>"February",3=>"March",4=>"April",
        5=>"May",6=>"June",7=>"July",8=>"August",
        9=>"September",10=>"October",11=>"November",_=>"December",
    }
}

/// Get today's date from the JS Date API
fn today() -> (i32, u32, u32) {
    let d = js_sys::Date::new_0();
    (d.get_full_year() as i32, d.get_month() + 1, d.get_date())
}

fn pad2(n: u32) -> String { format!("{:02}", n) }

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1|3|5|7|8|10|12 => 31,
        4|6|9|11 => 30,
        2 => if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 29 } else { 28 },
        _ => 30,
    }
}

#[component]
pub fn CalendarPage() -> impl IntoView {
    let auth = use_auth();
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let (today_y, today_m, today_d) = today();
    let (year, set_year) = signal(today_y);
    let (month, set_month) = signal(today_m);
    let (view_mode, set_view_mode) = signal("month".to_string());

    // Fetch calendar items for the visible month
    let api = auth.api.clone();
    let cal_items = LocalResource::new(move || {
        let api = api.clone();
        let s = slug();
        let y = year.get();
        let m = month.get();
        let from = format!("{}-{}-01", y, pad2(m));
        let last = days_in_month(y, m);
        let to   = format!("{}-{}-{}", y, pad2(m), pad2(last));
        async move { api.list_calendar(&s, &from, &to).await.unwrap_or_default() }
    });

    // Derived signal for items (empty while loading)
    let (items_sig, set_items_sig) = signal(Vec::<CalendarItem>::new());
    Effect::new(move |_| {
        if let Some(data) = cal_items.get() {
            set_items_sig.set((*data).clone());
        }
    });

    let prev_month = move |_| {
        let m = month.get();
        let y = year.get();
        if m == 1 { set_month.set(12); set_year.set(y - 1); }
        else { set_month.set(m - 1); }
    };
    let next_month = move |_| {
        let m = month.get();
        let y = year.get();
        if m == 12 { set_month.set(1); set_year.set(y + 1); }
        else { set_month.set(m + 1); }
    };

    let view_tabs = vec![("month", "Month"), ("week", "Week"), ("agenda", "Agenda")];

    view! {
        <div class="px-8 pt-6 pb-5 border-b-2 border-ink flex items-end justify-between gap-4" style="background: var(--cream-light);">
            <div>
                <h2 class="font-display text-2xl font-extrabold tracking-tight">
                    {move || format!("{} {}", month_name(month.get()), year.get())}
                </h2>
                <p class="text-[13px] mt-0.5" style="color: var(--ink-40);">"Calendar"</p>
            </div>
            // Navigation + view tabs
            <div class="flex items-center gap-2">
                <button
                    class="px-3 py-1.5 text-sm font-bold border-2 border-ink rounded-sm cursor-pointer"
                    on:click=prev_month
                >"< Prev"</button>
                <button
                    class="px-3 py-1.5 text-sm font-bold border-2 border-ink rounded-sm cursor-pointer"
                    on:click=next_month
                >"Next >"</button>
                <div class="flex border-2 border-ink rounded-sm overflow-hidden ml-2">
                    {view_tabs.into_iter().map(|(val, label)| {
                        let val = val.to_string();
                        let val2 = val.clone();
                        let val3 = val.clone();
                        view! {
                            <button
                                class="px-3 py-1.5 text-xs font-bold cursor-pointer transition-colors"
                                class:bg-ink=move || view_mode.get() == val
                                class:text-cream=move || view_mode.get() == val2
                                on:click=move |_| set_view_mode.set(val3.clone())
                            >{label}</button>
                        }
                    }).collect_view()}
                </div>
            </div>
        </div>

        // Calendar body
        <div class="flex-1 flex flex-col overflow-hidden">
            // TODO: drag & drop
            {move || {
                match view_mode.get().as_str() {
                    "week" => view! {
                        <WeekView
                            year=year
                            month=month
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
