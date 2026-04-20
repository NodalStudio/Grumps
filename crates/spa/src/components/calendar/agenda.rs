use leptos::prelude::*;
use crate::api::CalendarItem;
use super::item::CalItem;

fn parse_date_key(s: &str) -> String {
    s.get(..10).unwrap_or(s).to_string()
}

#[component]
pub fn AgendaView(items: ReadSignal<Vec<CalendarItem>>) -> impl IntoView {
    view! {
        <div class="flex-1 overflow-y-auto p-4">
            {move || {
                let all = items.get();
                if all.is_empty() {
                    return view! {
                        <div class="text-center py-16">
                            <p class="font-display text-lg font-bold">"Nothing scheduled."</p>
                        </div>
                    }.into_any();
                }

                // Group by date
                let mut groups: Vec<(String, Vec<CalendarItem>)> = Vec::new();
                for item in all {
                    let key = parse_date_key(&item.starts_at);
                    if let Some(last) = groups.last_mut() {
                        if last.0 == key {
                            last.1.push(item);
                            continue;
                        }
                    }
                    groups.push((key, vec![item]));
                }

                view! {
                    <div class="flex flex-col gap-6">
                        {groups.into_iter().map(|(date, day_items)| {
                            view! {
                                <div>
                                    <div
                                        class="text-meta pb-2 mb-2 border-b-2 border-ink"
                                        style="color: var(--ink-70);"
                                    >{date}</div>
                                    <div class="flex flex-col gap-2">
                                        {day_items.into_iter().map(|item| view! {
                                            <CalItem item=item compact=false />
                                        }).collect_view()}
                                    </div>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}
