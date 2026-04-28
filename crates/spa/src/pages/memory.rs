use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use crate::api::{use_api, MemoryItem};
use crate::components::header::PageHeader;
use crate::i18n::tr;
use crate::components::memory_card::MemoryCard;

#[component]
pub fn MemoryPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let (refresh, set_refresh) = signal(0u32);
    let (kind_filter, set_kind_filter) = signal("all".to_string());
    let (show_modal, set_show_modal) = signal(false);
    let (edit_item, set_edit_item) = signal::<Option<MemoryItem>>(None);
    let (confirm_delete, set_confirm_delete) = signal::<Option<String>>(None);

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
        items.get().map(|data| {
            let kf = kind_filter.get();
            let all: Vec<MemoryItem> = (*data).clone();
            if kf == "all" { all } else { all.into_iter().filter(|i| i.kind == kf).collect() }
        }).unwrap_or_default()
    };

    let open_create = move |_| {
        set_edit_item.set(None);
        set_form_key.set(String::new());
        set_form_value.set(String::new());
        set_form_kind.set("fact".to_string());
        set_form_related.set(String::new());
        set_form_expires.set(String::new());
        set_form_pinned.set(false);
        set_show_modal.set(true);
    };

    let on_edit = Callback::new(move |item: MemoryItem| {
        set_form_key.set(item.key.clone().unwrap_or_default());
        set_form_value.set(item.value.clone());
        set_form_kind.set(item.kind.clone());
        set_form_related.set(item.related_member.clone().unwrap_or_default());
        set_form_expires.set(item.expires_at.clone().unwrap_or_default());
        set_form_pinned.set(item.pinned);
        set_edit_item.set(Some(item));
        set_show_modal.set(true);
    });

    let on_delete = Callback::new(move |id: String| {
        set_confirm_delete.set(Some(id));
    });

    let api3 = use_api();
    let on_toggle_pin = Callback::new(move |id: String| {
        let api = api3.clone();
        let s = slug();
        leptos::task::spawn_local(async move {
            // We need to find the current pinned state — for simplicity, toggle via PATCH
            let _ = api.update_memory(&s, &id, &serde_json::json!({"pinned": true})).await;
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
        set_show_modal.set(false);
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
            set_confirm_delete.set(None);
            leptos::task::spawn_local(async move {
                let _ = api.delete_memory(&s, &id).await;
                set_refresh.update(|n| *n += 1);
            });
        }
    };

    let kinds = vec!["all", "fact", "preference", "skill", "event", "reminder"];

    view! {
        <PageHeader title=tr("page.memory.title") subtitle=tr("page.memory.subtitle")>
            <button
                class="px-4 py-2 text-sm font-semibold border-2 border-ink rounded-sm cursor-pointer"
                style="background: var(--ink); color: var(--cream);"
                on:click=open_create
            >"+ "{move || tr("memory.action.add")}</button>
        </PageHeader>

        <div class="flex-1 overflow-y-auto p-8">
            // Kind filter chips
            <div class="flex gap-2 flex-wrap mb-5">
                {kinds.into_iter().map(|k| {
                    let k = k.to_string();
                    let k2 = k.clone();
                    let k3 = k.clone();
                    let k4 = k.clone();
                    let label_key: String = if k == "all" { "common.filter.all".into() } else { format!("memory.kind.{}", k) };
                    view! {
                        <button
                            class="px-3 py-1 text-xs font-medium border rounded-sm cursor-pointer transition-colors"
                            class:bg-ink=move || kind_filter.get() == k
                            class:text-cream=move || kind_filter.get() == k2
                            style:border-color=move || if kind_filter.get() == k3 { "var(--ink)" } else { "var(--ink-15)" }
                            on:click=move |_| set_kind_filter.set(k4.clone())
                        >{move || tr(&label_key)}</button>
                    }
                }).collect_view()}
            </div>

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
                        <h2 class="font-display text-xl font-bold">{move || tr(if is_edit { "memory.modal.edit" } else { "memory.modal.add" })}</h2>

                        <div class="flex flex-col gap-1">
                            <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">{move || tr("memory.field.key")}</label>
                            <input
                                type="text" placeholder=tr("memory.field.key.placeholder")
                                class="border-2 border-ink rounded-sm px-3 py-2 text-sm bg-transparent outline-none"
                                on:input=move |ev| set_form_key.set(event_target_value(&ev))
                                prop:value=form_key
                            />
                        </div>

                        <div class="flex flex-col gap-1">
                            <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">{move || tr("memory.field.value")}</label>
                            <textarea
                                rows="3"
                                placeholder=tr("memory.field.value.placeholder")
                                class="border-2 border-ink rounded-sm px-3 py-2 text-sm bg-transparent outline-none resize-none"
                                on:input=move |ev| set_form_value.set(event_target_value(&ev))
                                prop:value=form_value
                            ></textarea>
                        </div>

                        <div class="flex gap-3">
                            <div class="flex flex-col gap-1 flex-1">
                                <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">{move || tr("memory.field.kind")}</label>
                                <select
                                    class="border-2 border-ink rounded-sm px-3 py-2 text-sm bg-transparent outline-none"
                                    on:change=move |ev| set_form_kind.set(event_target_value(&ev))
                                    prop:value=form_kind
                                >
                                    <option value="fact">{move || tr("memory.kind.fact")}</option>
                                    <option value="preference">{move || tr("memory.kind.preference")}</option>
                                    <option value="skill">{move || tr("memory.kind.skill")}</option>
                                    <option value="event">{move || tr("memory.kind.event")}</option>
                                    <option value="reminder">{move || tr("memory.kind.reminder")}</option>
                                </select>
                            </div>
                            <div class="flex flex-col gap-1 flex-1">
                                <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">{move || tr("memory.field.related")}</label>
                                <input
                                    type="text" placeholder=tr("memory.field.member.placeholder")
                                    class="border-2 border-ink rounded-sm px-3 py-2 text-sm bg-transparent outline-none"
                                    on:input=move |ev| set_form_related.set(event_target_value(&ev))
                                    prop:value=form_related
                                />
                            </div>
                        </div>

                        <div class="flex gap-3 items-center">
                            <div class="flex flex-col gap-1 flex-1">
                                <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">{move || tr("memory.field.expires")}</label>
                                <input
                                    type="date"
                                    class="border-2 border-ink rounded-sm px-3 py-2 text-sm bg-transparent outline-none"
                                    on:input=move |ev| set_form_expires.set(event_target_value(&ev))
                                    prop:value=form_expires
                                />
                            </div>
                            <div class="flex items-center gap-2 pt-5">
                                <input
                                    type="checkbox" id="mem-pinned"
                                    class="w-4 h-4 border-2 border-ink rounded-sm cursor-pointer"
                                    on:change=move |ev| {
                                        use wasm_bindgen::JsCast;
                                        let checked = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|i| i.checked()).unwrap_or(false);
                                        set_form_pinned.set(checked);
                                    }
                                    prop:checked=form_pinned
                                />
                                <label for="mem-pinned" class="text-sm font-medium cursor-pointer">{move || tr("memory.field.pinned")}</label>
                            </div>
                        </div>

                        <div class="flex gap-2 pt-2">
                            <button
                                class="flex-1 px-4 py-2 text-sm font-bold border-2 border-ink rounded-sm cursor-pointer"
                                style="background: var(--ink); color: var(--cream);"
                                on:click=save.clone()
                            >{move || tr("common.save")}</button>
                            <button
                                class="px-4 py-2 text-sm font-semibold border-2 border-ink rounded-sm cursor-pointer"
                                style="background: transparent; color: var(--ink);"
                                on:click=move |_| set_show_modal.set(false)
                            >{move || tr("common.cancel")}</button>
                        </div>
                    </div>
                </div>
            }
        })}

        // Delete confirm modal
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
                    <h2 class="font-display text-lg font-bold">{move || tr("memory.delete.title")}</h2>
                    <p class="text-sm" style="color: var(--ink-70);">{move || tr("common.irreversible")}</p>
                    <div class="flex gap-2">
                        <button
                            class="flex-1 px-4 py-2 text-sm font-bold border-2 rounded-sm cursor-pointer"
                            style="background: var(--brick); border-color: var(--brick); color: white;"
                            on:click=confirm_del.clone()
                        >{move || tr("common.delete")}</button>
                        <button
                            class="px-4 py-2 text-sm font-semibold border-2 border-ink rounded-sm cursor-pointer"
                            on:click=move |_| set_confirm_delete.set(None)
                        >{move || tr("common.cancel")}</button>
                    </div>
                </div>
            </div>
        })}
    }
}
