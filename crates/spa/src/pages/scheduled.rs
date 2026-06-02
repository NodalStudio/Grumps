use crate::api::{use_api, ScheduledActionItem};
use crate::components::header::PageHeader;
use crate::components::scheduled_card::ScheduledCard;
use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::dialog::Dialog;
use crate::components::ui::field::Field;
use crate::components::ui::select::Select;
use crate::i18n::tr;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[component]
pub fn ScheduledActionsPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let (refresh, set_refresh) = signal(0u32);
    let (type_filter, set_type_filter) = signal("all".to_string());
    let (status_filter, set_status_filter) = signal("all".to_string());
    let show_modal = RwSignal::new(false);
    let (edit_item, set_edit_item) = signal::<Option<ScheduledActionItem>>(None);
    let confirm_delete = RwSignal::new(None::<String>);

    // Workspace timezone signal, captured once (Copy) so event handlers can read
    // the current zone — use_context is unreliable inside callbacks.
    let tz_sig = use_context::<crate::datetime::TimezoneSignal>();
    let read_tz = move || {
        tz_sig
            .map(|s| s.0.get_untracked())
            .unwrap_or_else(|| "UTC".to_string())
    };

    // Form fields
    let (form_title, set_form_title) = signal(String::new());
    let (form_type, set_form_type) = signal("reminder".to_string());
    // form_trigger holds a `datetime-local` value (workspace-local wall clock);
    // it is converted to/from the stored UTC instant at load/save.
    let (form_trigger, set_form_trigger) = signal(String::new());
    let (form_recurrence, set_form_recurrence) = signal(String::new());
    let (form_payload, set_form_payload) = signal(String::new());

    let api = use_api();
    let items = LocalResource::new(move || {
        let api = api.clone();
        let s = slug();
        let _ = refresh.get();
        async move { api.list_scheduled_actions(&s).await.unwrap_or_default() }
    });

    let filtered = move || {
        items
            .get()
            .map(|data| {
                let tf = type_filter.get();
                let sf = status_filter.get();
                let all: Vec<ScheduledActionItem> = data.clone();
                all.into_iter()
                    .filter(|i| {
                        (tf == "all" || i.action_type == tf) && (sf == "all" || i.status == sf)
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let open_create = move |_| {
        set_edit_item.set(None);
        set_form_title.set(String::new());
        set_form_type.set("reminder".to_string());
        set_form_trigger.set(String::new());
        set_form_recurrence.set(String::new());
        set_form_payload.set(String::new());
        show_modal.set(true);
    };

    let on_edit = Callback::new(move |item: ScheduledActionItem| {
        set_form_title.set(item.title.clone());
        set_form_type.set(item.action_type.clone());
        // Stored as a UTC instant → show it as a workspace-local wall clock.
        set_form_trigger.set(crate::datetime::to_input_local(
            &item.trigger_at,
            &read_tz(),
        ));
        set_form_recurrence.set(item.recurrence.clone().unwrap_or_default());
        set_form_payload.set(String::new());
        set_edit_item.set(Some(item));
        show_modal.set(true);
    });

    // Delete-confirm dialog visibility, kept in sync with `confirm_delete`
    // (the id under consideration). The `Dialog` primitive reads/writes a bool.
    let del_open = RwSignal::new(false);
    let on_delete = Callback::new(move |id: String| {
        confirm_delete.set(Some(id));
        del_open.set(true);
    });

    let api_save = use_api();
    let save = move |_| {
        let api = api_save.clone();
        let s = slug();
        let title = form_title.get();
        let atype = form_type.get();
        // The datetime-local value is a workspace-local wall clock → convert it
        // back to a UTC instant before sending.
        let trigger = crate::datetime::input_local_to_utc(&form_trigger.get(), &read_tz());
        let recurrence = form_recurrence.get();
        let payload = form_payload.get();
        let edit = edit_item.get();
        show_modal.set(false);
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

    let api_del = use_api();
    let confirm_del = move |_| {
        if let Some(id) = confirm_delete.get() {
            let api = api_del.clone();
            let s = slug();
            confirm_delete.set(None);
            del_open.set(false);
            leptos::task::spawn_local(async move {
                let _ = api.delete_scheduled_action(&s, &id).await;
                set_refresh.update(|n| *n += 1);
            });
        }
    };

    let type_opts = vec![
        "all",
        "reminder",
        "event_notify",
        "recap",
        "follow_up",
        "agent_task",
    ];
    let status_opts = vec!["all", "pending", "firing", "done", "cancelled", "failed"];

    view! {
        <PageHeader title=tr("page.scheduled.title") subtitle=tr("page.scheduled.subtitle")>
            <Button variant=ButtonVariant::Primary on_click=open_create>
                "+ "{move || tr("schedule.action.new")}
            </Button>
        </PageHeader>

        <div class="flex-1 overflow-y-auto p-8">
            // Filters
            <div class="flex gap-4 items-center mb-5 flex-wrap">
                <div class="flex gap-2 flex-wrap">
                    <span class="text-[10px] uppercase tracking-wider font-bold self-center" style="color: var(--ink-40);">{move || tr("schedule.filter.type")}":"</span>
                    {type_opts.into_iter().map(|k| {
                        let k = k.to_string();
                        let k2 = k.clone(); let k3 = k.clone(); let k4 = k.clone();
                        let lk: String = if k == "all" { "common.filter.all".into() } else { format!("schedule.type.{}", k) };
                        view! {
                            <button
                                class="px-3 py-1 text-xs font-medium border rounded-xs cursor-pointer"
                                class:bg-ink=move || type_filter.get() == k
                                class:text-cream=move || type_filter.get() == k2
                                style:border-color=move || if type_filter.get() == k3 { "var(--ink)" } else { "var(--ink-15)" }
                                on:click=move |_| set_type_filter.set(k4.clone())
                            >{move || tr(&lk)}</button>
                        }
                    }).collect_view()}
                </div>
                <div class="flex gap-2 flex-wrap">
                    <span class="text-[10px] uppercase tracking-wider font-bold self-center" style="color: var(--ink-40);">{move || tr("schedule.filter.status")}":"</span>
                    {status_opts.into_iter().map(|k| {
                        let k = k.to_string();
                        let k2 = k.clone(); let k3 = k.clone(); let k4 = k.clone();
                        let lk: String = if k == "all" { "common.filter.all".into() } else { format!("schedule.status.{}", k) };
                        view! {
                            <button
                                class="px-3 py-1 text-xs font-medium border rounded-xs cursor-pointer"
                                class:bg-ink=move || status_filter.get() == k
                                class:text-cream=move || status_filter.get() == k2
                                style:border-color=move || if status_filter.get() == k3 { "var(--ink)" } else { "var(--ink-15)" }
                                on:click=move |_| set_status_filter.set(k4.clone())
                            >{move || tr(&lk)}</button>
                        }
                    }).collect_view()}
                </div>
            </div>

            // List
            <Suspense fallback=|| view! { <div style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
                {move || {
                    let items: Vec<ScheduledActionItem> = filtered();
                    if items.is_empty() {
                        return view! {
                            <div class="text-center py-16">
                                <p class="font-display text-lg font-bold">{move || tr("schedule.empty.title")}</p>
                                <p class="text-sm mt-1" style="color: var(--ink-40);">{move || tr("schedule.empty.hint")}</p>
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
                                    move |item| view! {
                                        <ScheduledCard
                                            item=item
                                            on_edit=on_edit.clone()
                                            on_delete=on_delete.clone()
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
        <Dialog
            open=show_modal
            on_close=move || show_modal.set(false)
            labelledby="sched-modal-title"
            class="max-w-md"
        >
                        <h2 id="sched-modal-title" class="font-display text-xl font-bold">{move || tr(if edit_item.get().is_some() { "schedule.modal.edit" } else { "schedule.modal.add" })}</h2>

                        <Field label=tr("schedule.field.title") id="sched-title">
                            <input id="sched-title" type="text" placeholder=tr("schedule.field.title.placeholder")
                                class="border-2 border-ink rounded-xs px-3 py-2 text-sm bg-transparent outline-hidden"
                                on:input=move |ev| set_form_title.set(event_target_value(&ev))
                                prop:value=form_title
                            />
                        </Field>

                        <div class="flex gap-3">
                            <div class="flex flex-col gap-1 flex-1">
                                <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">{move || tr("schedule.field.type")}</label>
                                <Select
                                    value=form_type
                                    aria_label=tr("schedule.field.type")
                                    on_change=move |v: String| set_form_type.set(v)
                                >
                                    <option value="reminder">{move || tr("schedule.type.reminder")}</option>
                                    <option value="event_notify">{move || tr("schedule.type.event_notify")}</option>
                                    <option value="recap">{move || tr("schedule.type.recap")}</option>
                                    <option value="follow_up">{move || tr("schedule.type.follow_up")}</option>
                                    <option value="agent_task">{move || tr("schedule.type.agent_task")}</option>
                                </Select>
                            </div>
                            <div class="flex-1">
                                <Field label=tr("schedule.field.trigger_at") id="sched-trigger">
                                    <input id="sched-trigger" type="datetime-local"
                                        class="border-2 border-ink rounded-xs px-3 py-2 text-sm bg-transparent outline-hidden"
                                        on:input=move |ev| set_form_trigger.set(event_target_value(&ev))
                                        prop:value=form_trigger
                                    />
                                </Field>
                            </div>
                        </div>

                        <Field label=tr("schedule.field.recurrence") id="sched-recurrence">
                            <input id="sched-recurrence" type="text" placeholder=tr("schedule.field.recurrence.placeholder")
                                class="border-2 border-ink rounded-xs px-3 py-2 text-sm bg-transparent outline-hidden font-mono"
                                on:input=move |ev| set_form_recurrence.set(event_target_value(&ev))
                                prop:value=form_recurrence
                            />
                        </Field>

                        <Field label=tr("schedule.field.payload") id="sched-payload">
                            <textarea id="sched-payload" rows="3" placeholder="{}"
                                class="border-2 border-ink rounded-xs px-3 py-2 text-sm bg-transparent outline-hidden resize-none font-mono"
                                on:input=move |ev| set_form_payload.set(event_target_value(&ev))
                                prop:value=form_payload
                            ></textarea>
                        </Field>

                        <div class="flex gap-2 pt-2">
                            <Button
                                variant=ButtonVariant::Primary
                                class="flex-1"
                                on_click=save.clone()
                            >{move || tr("common.save")}</Button>
                            <Button
                                variant=ButtonVariant::Secondary
                                on_click=move |_| show_modal.set(false)
                            >{move || tr("common.cancel")}</Button>
                        </div>
        </Dialog>

        // Delete confirm
        <Dialog
            open=del_open
            on_close=move || { confirm_delete.set(None); del_open.set(false); }
            labelledby="sched-delete-title"
            class="max-w-sm"
        >
                    <h2 id="sched-delete-title" class="font-display text-lg font-bold">{move || tr("schedule.delete.title")}</h2>
                    <p class="text-sm" style="color: var(--ink-70);">{move || tr("common.irreversible")}</p>
                    <div class="flex gap-2">
                        <Button
                            variant=ButtonVariant::Danger
                            class="flex-1 bg-brick text-white border-2 border-brick"
                            on_click=confirm_del.clone()
                        >{move || tr("common.delete")}</Button>
                        <Button
                            variant=ButtonVariant::Secondary
                            on_click=move |_| { confirm_delete.set(None); del_open.set(false); }
                        >{move || tr("common.cancel")}</Button>
                    </div>
        </Dialog>
    }
}
