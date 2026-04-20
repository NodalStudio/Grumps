use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use crate::auth::use_auth;
use crate::api::ScheduledActionItem;
use crate::components::header::PageHeader;
use crate::components::scheduled_card::ScheduledCard;

#[component]
pub fn ScheduledActionsPage() -> impl IntoView {
    let auth = use_auth();
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let (refresh, set_refresh) = signal(0u32);
    let (type_filter, set_type_filter) = signal("all".to_string());
    let (status_filter, set_status_filter) = signal("all".to_string());
    let (show_modal, set_show_modal) = signal(false);
    let (edit_item, set_edit_item) = signal::<Option<ScheduledActionItem>>(None);
    let (confirm_delete, set_confirm_delete) = signal::<Option<String>>(None);

    // Form fields
    let (form_title, set_form_title) = signal(String::new());
    let (form_type, set_form_type) = signal("message".to_string());
    let (form_trigger, set_form_trigger) = signal(String::new());
    let (form_recurrence, set_form_recurrence) = signal(String::new());
    let (form_payload, set_form_payload) = signal(String::new());

    let api = auth.api.clone();
    let items = LocalResource::new(move || {
        let api = api.clone();
        let s = slug();
        let _ = refresh.get();
        async move { api.list_scheduled_actions(&s).await.unwrap_or_default() }
    });

    let filtered = move || {
        items.get().map(|data| {
            let tf = type_filter.get();
            let sf = status_filter.get();
            let all: Vec<ScheduledActionItem> = (*data).clone();
            all.into_iter().filter(|i| {
                (tf == "all" || i.action_type == tf) &&
                (sf == "all" || i.status == sf)
            }).collect()
        }).unwrap_or_default()
    };

    let open_create = move |_| {
        set_edit_item.set(None);
        set_form_title.set(String::new());
        set_form_type.set("message".to_string());
        set_form_trigger.set(String::new());
        set_form_recurrence.set(String::new());
        set_form_payload.set(String::new());
        set_show_modal.set(true);
    };

    let on_edit = Callback::new(move |item: ScheduledActionItem| {
        set_form_title.set(item.title.clone());
        set_form_type.set(item.action_type.clone());
        set_form_trigger.set(item.trigger_at.clone());
        set_form_recurrence.set(item.recurrence.clone().unwrap_or_default());
        set_form_payload.set(String::new());
        set_edit_item.set(Some(item));
        set_show_modal.set(true);
    });

    let on_delete = Callback::new(move |id: String| {
        set_confirm_delete.set(Some(id));
    });

    let api_exec = auth.api.clone();
    let on_execute = Callback::new(move |id: String| {
        // TODO: POST /api/w/:slug/scheduled-actions/:id/execute
        let _ = (&api_exec, &id);
        web_sys::window()
            .and_then(|w| w.alert_with_message("Execute now — API endpoint TBD").ok());
    });

    let api_save = auth.api.clone();
    let save = move |_| {
        let api = api_save.clone();
        let s = slug();
        let title = form_title.get();
        let atype = form_type.get();
        let trigger = form_trigger.get();
        let recurrence = form_recurrence.get();
        let payload = form_payload.get();
        let edit = edit_item.get();
        set_show_modal.set(false);
        leptos::task::spawn_local(async move {
            let body = serde_json::json!({
                "title": title,
                "action_type": atype,
                "trigger_at": trigger,
                "recurrence": if recurrence.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(recurrence) },
                "payload": if payload.is_empty() { serde_json::Value::Null } else { serde_json::from_str::<serde_json::Value>(&payload).unwrap_or(serde_json::Value::Null) },
            });
            if let Some(item) = edit {
                let _ = api.update_scheduled_action(&s, &item.id, &body).await;
            } else {
                let _ = api.create_scheduled_action(&s, &body).await;
            }
            set_refresh.update(|n| *n += 1);
        });
    };

    let api_del = auth.api.clone();
    let confirm_del = move |_| {
        if let Some(id) = confirm_delete.get() {
            let api = api_del.clone();
            let s = slug();
            set_confirm_delete.set(None);
            leptos::task::spawn_local(async move {
                let _ = api.delete_scheduled_action(&s, &id).await;
                set_refresh.update(|n| *n += 1);
            });
        }
    };

    let type_opts = vec!["all", "message", "reminder", "recap", "task", "webhook"];
    let status_opts = vec!["all", "active", "paused", "done", "failed"];

    view! {
        <PageHeader title="Scheduled Actions".into() subtitle="Automated triggers and reminders".to_string()>
            <button
                class="px-4 py-2 text-sm font-semibold border-2 border-ink rounded-sm cursor-pointer"
                style="background: var(--ink); color: var(--cream);"
                on:click=open_create
            >"+ New action"</button>
        </PageHeader>

        <div class="flex-1 overflow-y-auto p-8">
            // Filters
            <div class="flex gap-4 items-center mb-5 flex-wrap">
                <div class="flex gap-2 flex-wrap">
                    <span class="text-[10px] uppercase tracking-wider font-bold self-center" style="color: var(--ink-40);">"Type:"</span>
                    {type_opts.into_iter().map(|k| {
                        let k = k.to_string();
                        let k2 = k.clone(); let k3 = k.clone(); let k4 = k.clone();
                        view! {
                            <button
                                class="px-3 py-1 text-xs font-medium border rounded-sm cursor-pointer capitalize"
                                class:bg-ink=move || type_filter.get() == k
                                class:text-cream=move || type_filter.get() == k2
                                style:border-color=move || if type_filter.get() == k3 { "var(--ink)" } else { "var(--ink-15)" }
                                on:click=move |_| set_type_filter.set(k4.clone())
                            >{k.clone()}</button>
                        }
                    }).collect_view()}
                </div>
                <div class="flex gap-2 flex-wrap">
                    <span class="text-[10px] uppercase tracking-wider font-bold self-center" style="color: var(--ink-40);">"Status:"</span>
                    {status_opts.into_iter().map(|k| {
                        let k = k.to_string();
                        let k2 = k.clone(); let k3 = k.clone(); let k4 = k.clone();
                        view! {
                            <button
                                class="px-3 py-1 text-xs font-medium border rounded-sm cursor-pointer capitalize"
                                class:bg-ink=move || status_filter.get() == k
                                class:text-cream=move || status_filter.get() == k2
                                style:border-color=move || if status_filter.get() == k3 { "var(--ink)" } else { "var(--ink-15)" }
                                on:click=move |_| set_status_filter.set(k4.clone())
                            >{k.clone()}</button>
                        }
                    }).collect_view()}
                </div>
            </div>

            // List
            <Suspense fallback=|| view! { <div style="color: var(--ink-40);">"Loading..."</div> }>
                {move || {
                    let items: Vec<ScheduledActionItem> = filtered();
                    if items.is_empty() {
                        return view! {
                            <div class="text-center py-16">
                                <p class="font-display text-lg font-bold">"No scheduled actions."</p>
                                <p class="text-sm mt-1" style="color: var(--ink-40);">"Create recurring reminders, recaps, and more."</p>
                            </div>
                        }.into_any();
                    }
                    view! {
                        <div class="flex flex-col gap-3">
                            <For
                                each=move || items.clone()
                                key=|i| i.id.clone()
                                children={
                                    let on_edit = on_edit.clone();
                                    let on_delete = on_delete.clone();
                                    let on_execute = on_execute.clone();
                                    move |item| view! {
                                        <ScheduledCard
                                            item=item
                                            on_edit=on_edit.clone()
                                            on_delete=on_delete.clone()
                                            on_execute=on_execute.clone()
                                        />
                                    }
                                }
                            />
                        </div>
                    }.into_any()
                }}
            </Suspense>
        </div>

        // Create/Edit modal
        {move || show_modal.get().then(|| {
            let is_edit = edit_item.get().is_some();
            view! {
                <div
                    class="fixed inset-0 z-50 flex items-center justify-center"
                    style="background: rgba(26,26,26,0.5);"
                    on:click=move |_| set_show_modal.set(false)
                >
                    <div
                        class="w-full max-w-md mx-4 border-2 border-ink rounded-sm p-6 flex flex-col gap-4"
                        style="background: var(--cream); box-shadow: 6px 6px 0 #1A1A1A;"
                        on:click=|e| e.stop_propagation()
                    >
                        <h2 class="font-display text-xl font-bold">{if is_edit { "Edit Action" } else { "New Scheduled Action" }}</h2>

                        <div class="flex flex-col gap-1">
                            <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">"Title"</label>
                            <input type="text" placeholder="e.g. Weekly recap"
                                class="border-2 border-ink rounded-sm px-3 py-2 text-sm bg-transparent outline-none"
                                on:input=move |ev| set_form_title.set(event_target_value(&ev))
                                prop:value=form_title
                            />
                        </div>

                        <div class="flex gap-3">
                            <div class="flex flex-col gap-1 flex-1">
                                <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">"Type"</label>
                                <select
                                    class="border-2 border-ink rounded-sm px-3 py-2 text-sm bg-transparent outline-none"
                                    on:change=move |ev| set_form_type.set(event_target_value(&ev))
                                    prop:value=form_type
                                >
                                    <option value="message">"message"</option>
                                    <option value="reminder">"reminder"</option>
                                    <option value="recap">"recap"</option>
                                    <option value="task">"task"</option>
                                    <option value="webhook">"webhook"</option>
                                </select>
                            </div>
                            <div class="flex flex-col gap-1 flex-1">
                                <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">"Trigger at"</label>
                                <input type="datetime-local"
                                    class="border-2 border-ink rounded-sm px-3 py-2 text-sm bg-transparent outline-none"
                                    on:input=move |ev| set_form_trigger.set(event_target_value(&ev))
                                    prop:value=form_trigger
                                />
                            </div>
                        </div>

                        <div class="flex flex-col gap-1">
                            <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">"Recurrence (RRULE)"</label>
                            <input type="text" placeholder="e.g. FREQ=WEEKLY;BYDAY=MO"
                                class="border-2 border-ink rounded-sm px-3 py-2 text-sm bg-transparent outline-none font-mono"
                                on:input=move |ev| set_form_recurrence.set(event_target_value(&ev))
                                prop:value=form_recurrence
                            />
                        </div>

                        <div class="flex flex-col gap-1">
                            <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">"Payload (JSON)"</label>
                            <textarea rows="3" placeholder="{}"
                                class="border-2 border-ink rounded-sm px-3 py-2 text-sm bg-transparent outline-none resize-none font-mono"
                                on:input=move |ev| set_form_payload.set(event_target_value(&ev))
                                prop:value=form_payload
                            ></textarea>
                        </div>

                        <div class="flex gap-2 pt-2">
                            <button
                                class="flex-1 px-4 py-2 text-sm font-bold border-2 border-ink rounded-sm cursor-pointer"
                                style="background: var(--ink); color: var(--cream);"
                                on:click=save.clone()
                            >"Save"</button>
                            <button
                                class="px-4 py-2 text-sm font-semibold border-2 border-ink rounded-sm cursor-pointer"
                                on:click=move |_| set_show_modal.set(false)
                            >"Cancel"</button>
                        </div>
                    </div>
                </div>
            }
        })}

        // Delete confirm
        {move || confirm_delete.get().is_some().then(|| view! {
            <div
                class="fixed inset-0 z-50 flex items-center justify-center"
                style="background: rgba(26,26,26,0.5);"
                on:click=move |_| set_confirm_delete.set(None)
            >
                <div
                    class="w-full max-w-sm mx-4 border-2 border-ink rounded-sm p-6 flex flex-col gap-4"
                    style="background: var(--cream); box-shadow: 6px 6px 0 #1A1A1A;"
                    on:click=|e| e.stop_propagation()
                >
                    <h2 class="font-display text-lg font-bold">"Delete scheduled action?"</h2>
                    <p class="text-sm" style="color: var(--ink-70);">"This cannot be undone."</p>
                    <div class="flex gap-2">
                        <button
                            class="flex-1 px-4 py-2 text-sm font-bold border-2 rounded-sm cursor-pointer"
                            style="background: var(--brick); border-color: var(--brick); color: white;"
                            on:click=confirm_del.clone()
                        >"Delete"</button>
                        <button
                            class="px-4 py-2 text-sm font-semibold border-2 border-ink rounded-sm cursor-pointer"
                            on:click=move |_| set_confirm_delete.set(None)
                        >"Cancel"</button>
                    </div>
                </div>
            </div>
        })}
    }
}
