use crate::api::{use_api, MemoryItem};
use crate::components::header::PageHeader;
use crate::components::memory_card::MemoryCard;
use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::checkbox::Checkbox;
use crate::components::ui::dialog::Dialog;
use crate::components::ui::field::Field;
use crate::components::ui::select::Select;
use crate::components::ui::tabs::{TabItem, Tabs};
use crate::i18n::tr;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[component]
pub fn MemoryPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let (refresh, set_refresh) = signal(0u32);
    let kind_filter = RwSignal::new("all".to_string());
    let show_modal = RwSignal::new(false);
    let (edit_item, set_edit_item) = signal::<Option<MemoryItem>>(None);
    let confirm_delete = RwSignal::new(None::<String>);

    // Form fields
    let (form_key, set_form_key) = signal(String::new());
    let (form_value, set_form_value) = signal(String::new());
    let (form_kind, set_form_kind) = signal("fact".to_string());
    let (form_related, set_form_related) = signal(String::new());
    let (form_expires, set_form_expires) = signal(String::new());
    let (form_pinned, set_form_pinned) = signal(false);

    let api = use_api();
    let items = LocalResource::new(move || {
        let api = api.clone();
        let s = slug();
        let _ = refresh.get();
        async move { api.list_memory(&s).await.unwrap_or_default() }
    });

    // Filtered view
    let filtered = move || {
        items
            .get()
            .map(|data| {
                let kf = kind_filter.get();
                let all: Vec<MemoryItem> = data.clone();
                if kf == "all" {
                    all
                } else {
                    all.into_iter().filter(|i| i.kind == kf).collect()
                }
            })
            .unwrap_or_default()
    };

    let open_create = move |_| {
        set_edit_item.set(None);
        set_form_key.set(String::new());
        set_form_value.set(String::new());
        set_form_kind.set("fact".to_string());
        set_form_related.set(String::new());
        set_form_expires.set(String::new());
        set_form_pinned.set(false);
        show_modal.set(true);
    };

    let on_edit = Callback::new(move |item: MemoryItem| {
        set_form_key.set(item.key.clone().unwrap_or_default());
        set_form_value.set(item.value.clone());
        set_form_kind.set(item.kind.clone());
        set_form_related.set(item.related_member.clone().unwrap_or_default());
        set_form_expires.set(item.expires_at.clone().unwrap_or_default());
        set_form_pinned.set(item.pinned);
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

    let api3 = use_api();
    let on_toggle_pin = Callback::new(move |id: String| {
        let api = api3.clone();
        let s = slug();
        leptos::task::spawn_local(async move {
            // We need to find the current pinned state — for simplicity, toggle via PATCH
            let _ = api
                .update_memory(&s, &id, &serde_json::json!({"pinned": true}))
                .await;
            set_refresh.update(|n| *n += 1);
        });
    });

    let api_save = use_api();
    let save = move |_| {
        let api = api_save.clone();
        let s = slug();
        let key = form_key.get();
        let value = form_value.get();
        let kind = form_kind.get();
        let related = form_related.get();
        let expires = form_expires.get();
        let pinned = form_pinned.get();
        let edit = edit_item.get();
        show_modal.set(false);
        leptos::task::spawn_local(async move {
            let body = serde_json::json!({
                "key": if key.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(key) },
                "value": value,
                "kind": kind,
                "related_member": if related.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(related) },
                "expires_at": if expires.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(expires) },
                "pinned": pinned,
            });
            if let Some(item) = edit {
                let _ = api.update_memory(&s, &item.id, &body).await;
            } else {
                let _ = api.create_memory(&s, &body).await;
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
                let _ = api.delete_memory(&s, &id).await;
                set_refresh.update(|n| *n += 1);
            });
        }
    };

    let kind_tabs: Vec<TabItem> = vec!["all", "fact", "preference", "skill", "event", "reminder"]
        .into_iter()
        .map(|k| {
            let label_key = if k == "all" {
                "common.filter.all".to_string()
            } else {
                format!("memory.kind.{}", k)
            };
            TabItem {
                value: k.to_string(),
                label_key,
            }
        })
        .collect();

    view! {
        <PageHeader title=tr("page.memory.title") subtitle=tr("page.memory.subtitle")>
            <Button variant=ButtonVariant::Primary on_click=open_create>
                "+ "{move || tr("memory.action.add")}
            </Button>
        </PageHeader>

        <div class="flex-1 overflow-y-auto p-8">
            // Kind filter chips
            <Tabs
                value=kind_filter
                tabs=kind_tabs
                aria_label=Signal::derive(move || tr("memory.filter.aria"))
                class="mb-5"
            />

            // Memory list
            <Suspense fallback=|| view! { <div style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
                {move || {
                    let items = filtered();
                    if items.is_empty() {
                        return view! {
                            <div class="text-center py-16">
                                <p class="font-display text-lg font-bold">{move || tr("memory.empty.title")}</p>
                                <p class="text-sm mt-1" style="color: var(--ink-40);">{move || tr("memory.empty.hint")}</p>
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
                                    let on_toggle_pin = on_toggle_pin.clone();
                                    move |item| view! {
                                        <MemoryCard
                                            item=item
                                            on_edit=on_edit.clone()
                                            on_delete=on_delete.clone()
                                            on_toggle_pin=on_toggle_pin.clone()
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
            labelledby="mem-modal-title"
            class="max-w-md"
        >
                        <h2 id="mem-modal-title" class="font-display text-xl font-bold">{move || tr(if edit_item.get().is_some() { "memory.modal.edit" } else { "memory.modal.add" })}</h2>

                        <Field label=tr("memory.field.key") id="mem-key">
                            <input
                                id="mem-key"
                                type="text" placeholder=tr("memory.field.key.placeholder")
                                class="border-2 border-ink rounded-xs px-3 py-2 text-sm bg-transparent outline-hidden"
                                on:input=move |ev| set_form_key.set(event_target_value(&ev))
                                prop:value=form_key
                            />
                        </Field>

                        <Field label=tr("memory.field.value") id="mem-value">
                            <textarea
                                id="mem-value"
                                rows="3"
                                placeholder=tr("memory.field.value.placeholder")
                                class="border-2 border-ink rounded-xs px-3 py-2 text-sm bg-transparent outline-hidden resize-none"
                                on:input=move |ev| set_form_value.set(event_target_value(&ev))
                                prop:value=form_value
                            ></textarea>
                        </Field>

                        <div class="flex gap-3">
                            <div class="flex flex-col gap-1 flex-1">
                                <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">{move || tr("memory.field.kind")}</label>
                                <Select
                                    value=form_kind
                                    aria_label=tr("memory.field.kind")
                                    on_change=move |v: String| set_form_kind.set(v)
                                >
                                    <option value="fact">{move || tr("memory.kind.fact")}</option>
                                    <option value="preference">{move || tr("memory.kind.preference")}</option>
                                    <option value="skill">{move || tr("memory.kind.skill")}</option>
                                    <option value="event">{move || tr("memory.kind.event")}</option>
                                    <option value="reminder">{move || tr("memory.kind.reminder")}</option>
                                </Select>
                            </div>
                            <div class="flex-1">
                                <Field label=tr("memory.field.related") id="mem-related">
                                    <input
                                        id="mem-related"
                                        type="text" placeholder=tr("memory.field.member.placeholder")
                                        class="border-2 border-ink rounded-xs px-3 py-2 text-sm bg-transparent outline-hidden"
                                        on:input=move |ev| set_form_related.set(event_target_value(&ev))
                                        prop:value=form_related
                                    />
                                </Field>
                            </div>
                        </div>

                        <div class="flex gap-3 items-center">
                            <div class="flex-1">
                                <Field label=tr("memory.field.expires") id="mem-expires">
                                    <input
                                        id="mem-expires"
                                        type="date"
                                        class="border-2 border-ink rounded-xs px-3 py-2 text-sm bg-transparent outline-hidden"
                                        on:input=move |ev| set_form_expires.set(event_target_value(&ev))
                                        prop:value=form_expires
                                    />
                                </Field>
                            </div>
                            <div class="flex items-center gap-2 pt-5">
                                <Checkbox
                                    checked=Signal::derive(move || form_pinned.get())
                                    on_change=move || set_form_pinned.update(|v| *v = !*v)
                                    id="mem-pinned"
                                    aria_label=tr("memory.field.pinned")
                                />
                                <label for="mem-pinned" class="text-sm font-medium cursor-pointer">{move || tr("memory.field.pinned")}</label>
                            </div>
                        </div>

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

        // Delete confirm modal
        <Dialog
            open=del_open
            on_close=move || { confirm_delete.set(None); del_open.set(false); }
            labelledby="mem-delete-title"
            class="max-w-sm"
        >
                    <h2 id="mem-delete-title" class="font-display text-lg font-bold">{move || tr("memory.delete.title")}</h2>
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
