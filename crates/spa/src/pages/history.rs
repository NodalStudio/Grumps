use crate::api::use_api;
use crate::components::header::PageHeader;
use crate::i18n::tr;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[component]
pub fn HistoryPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let history = LocalResource::new(move || {
        let api = use_api();
        let s = slug();
        async move { api.get_history(&s).await.unwrap_or_default() }
    });

    view! {
        <PageHeader title=tr("page.history.title") subtitle=tr("history.subtitle") />
        <div class="flex-1 overflow-y-auto p-8">
            <Suspense fallback=|| view! { <div style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
                {move || history.get().map(|data| {
                    let items: Vec<_> = data.clone();
                    if items.is_empty() {
                        return view! {
                            <div class="text-center py-16">
                                <p class="font-display text-lg font-bold">{move || tr("history.empty.title")}</p>
                                <p class="text-sm mt-1" style="color: var(--ink-40);">{move || tr("history.empty.hint")}</p>
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
                                            <div class="w-2 h-2 rounded-full shrink-0 mt-1.5" style:background=dot_color></div>
                                            <div class="flex-1 text-[13px]">
                                                <strong>{activity.actor.clone().unwrap_or_else(|| tr("common.someone"))}</strong>
                                                " "
                                                {let a = activity.action.clone(); move || tr(&a)}
                                                {activity.target_id.clone().map(|id| view! { <span class="font-display font-semibold">" "{move || tr(&id)}</span> })}
                                                <span class="ml-2 text-[10px] font-semibold uppercase tracking-wider px-1.5 py-0.5 border rounded-xs" style="color: var(--ink-40); border-color: var(--ink-15);">
                                                    {let s = activity.source.clone(); move || tr(&s)}
                                                </span>
                                            </div>
                                            <div class="text-[11px] shrink-0" style="color: var(--ink-40);">{let t = activity.created_at.clone(); move || crate::datetime::format_instant(&t, &crate::datetime::use_timezone(), crate::i18n::use_locale().code())}</div>
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
