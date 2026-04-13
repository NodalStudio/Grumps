use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use crate::auth::use_auth;
use crate::components::header::PageHeader;

#[component]
pub fn HistoryPage() -> impl IntoView {
    let auth = use_auth();
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let history = LocalResource::new(move || {
        let api = auth.api.clone();
        let s = slug();
        async move { api.get_history(&s).await.unwrap_or_default() }
    });

    view! {
        <PageHeader title="History".into() subtitle="Everything that happened".to_string() />
        <div class="flex-1 overflow-y-auto p-8">
            <Suspense fallback=|| view! { <div style="color: var(--ink-40);">"Loading..."</div> }>
                {move || history.get().map(|data| {
                    let items: Vec<_> = (*data).clone();
                    if items.is_empty() {
                        return view! {
                            <div class="text-center py-16">
                                <p class="font-display text-lg font-bold">"Nothing happened yet."</p>
                                <p class="text-sm mt-1" style="color: var(--ink-40);">"Eerily quiet."</p>
                            </div>
                        }.into_any();
                    }
                    view! {
                        <div class="flex flex-col">
                            <For
                                each=move || items.clone()
                                key=|a| a.id.clone()
                                children=move |activity| {
                                    let dot_color = match activity.action.split('.').last().unwrap_or("") {
                                        "created" => "var(--teal)",
                                        "completed" => "var(--teal)",
                                        "updated" => "var(--ochre)",
                                        "deleted" => "var(--brick)",
                                        _ => "var(--ink-15)",
                                    };
                                    view! {
                                        <div class="flex gap-3.5 py-3.5 items-start" style="border-bottom: 1px solid var(--ink-08);">
                                            <div class="w-2 h-2 rounded-full flex-shrink-0 mt-1.5" style:background=dot_color></div>
                                            <div class="flex-1 text-[13px]">
                                                <strong>{activity.actor.clone().unwrap_or("Someone".into())}</strong>
                                                " "
                                                {activity.action.clone()}
                                                {activity.target_id.clone().map(|id| view! { <span class="font-display font-semibold">{format!(" {}", id)}</span> })}
                                                <span class="ml-2 text-[10px] font-semibold uppercase tracking-wider px-1.5 py-0.5 border rounded-sm" style="color: var(--ink-40); border-color: var(--ink-15);">
                                                    {activity.source.clone()}
                                                </span>
                                            </div>
                                            <div class="text-[11px] flex-shrink-0" style="color: var(--ink-40);">{activity.created_at.clone()}</div>
                                        </div>
                                    }
                                }
                            />
                        </div>
                    }.into_any()
                })}
            </Suspense>
        </div>
    }
}
