use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use crate::auth::use_auth;
use crate::components::header::PageHeader;
use crate::i18n::tr;

#[component]
pub fn TodosPage() -> impl IntoView {
    let auth = use_auth();
    let params = use_params_map();
    let slug = Memo::new(move |_| params.read().get("slug").unwrap_or_default());
    let (filter, set_filter) = signal("open".to_string());
    let (refresh_counter, set_refresh) = signal(0u32);

    let api = auth.api.clone();
    let api2 = auth.api.clone();
    let api3 = auth.api.clone();
    let todos = LocalResource::new(move || {
        let api = api.clone();
        let s = slug.get();
        let f = filter.get();
        let _ = refresh_counter.get();
        async move { api.get_todos(&s, &f).await.unwrap_or_default() }
    });

    let (new_title, set_new_title) = signal(String::new());
    let on_create = move |_: web_sys::KeyboardEvent| {
        let api = api2.clone();
        let s = slug.get();
        let title = new_title.get();
        if title.trim().is_empty() { return; }
        set_new_title.set(String::new());
        leptos::task::spawn_local(async move {
            let _ = api.create_todo(&s, &title, 2).await;
            set_refresh.update(|n| *n += 1);
        });
    };

    // Click handler closure cloned per row inside the `For` children.
    let _ = &api3;

    let filters: Vec<(&'static str, &'static str)> = vec![
        ("open", "todo.filter.open"), ("all", "todo.filter.all"),
        ("done", "todo.filter.done"), ("mine", "todo.filter.mine"),
    ];

    view! {
        <PageHeader title=tr("page.todos.title") subtitle=tr("page.todos.subtitle")>
            <button
                class="px-4 py-2 text-sm font-semibold border-2 border-ink rounded-sm cursor-pointer"
                style="background: var(--ink); color: var(--cream);"
            >"+ "{move || tr("todo.action.add")}</button>
        </PageHeader>
        <div class="flex-1 overflow-y-auto p-8">
            // Filter bar
            <div class="flex gap-2 items-center mb-5 flex-wrap">
                {filters.into_iter().map(|(val, label_key)| {
                    let val = val.to_string();
                    let val2 = val.clone();
                    let val3 = val.clone();
                    let val4 = val.clone();
                    view! {
                        <button
                            class="px-3 py-1 text-xs font-medium border rounded-sm cursor-pointer transition-colors"
                            class:bg-ink=move || filter.get() == val
                            class:text-cream=move || filter.get() == val2
                            style:border-color=move || if filter.get() == val3 { "var(--ink)" } else { "var(--ink-15)" }
                            on:click=move |_| set_filter.set(val4.clone())
                        >{move || tr(label_key)}</button>
                    }
                }).collect_view()}
            </div>

            // Todo list
            <Suspense fallback=|| view! { <div style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
                {move || todos.get().map(|data| {
                    let items: Vec<_> = (*data).clone();
                    if items.is_empty() {
                        return view! {
                            <div class="text-center py-16">
                                <p class="font-display text-lg font-bold">{move || tr("todo.empty.title")}</p>
                                <p class="text-sm mt-1" style="color: var(--ink-40);">{move || tr("todo.empty.sub")}</p>
                            </div>
                        }.into_any();
                    }
                    let api_for_children = api3.clone();
                    view! {
                        <div class="flex flex-col gap-1.5">
                            <For
                                each=move || items.clone()
                                key=|t| t.seq_num
                                children=move |todo| {
                                    let is_done = todo.status == "done";
                                    let is_high = todo.priority == 1;
                                    let id = todo.id.clone();
                                    let api_for_click = api_for_children.clone();
                                    let slug_for_click = slug.get();
                                    view! {
                                        <div
                                            class="flex items-start gap-3 px-4 py-3 border-2 border-ink rounded-sm cursor-pointer transition-transform hover:translate-x-0.5"
                                            class:border-l-4=is_high
                                            style:border-left-color=move || if is_high { "var(--brick)" } else { "" }
                                            style="background: var(--cream-light);"
                                        >
                                            <div
                                                class="w-5 h-5 border-2 border-ink rounded-sm flex-shrink-0 mt-0.5 flex items-center justify-center cursor-pointer"
                                                style:background=move || if is_done { "var(--teal)" } else { "var(--cream-light)" }
                                                style:border-color=move || if is_done { "var(--teal)" } else { "var(--ink)" }
                                                on:click=move |ev| {
                                                    ev.stop_propagation();
                                                    let api = api_for_click.clone();
                                                    let s = slug_for_click.clone();
                                                    let id_c = id.clone();
                                                    let next_status = if is_done { "open" } else { "done" };
                                                    leptos::task::spawn_local(async move {
                                                        let _ = api.update_todo(&s, &id_c, &serde_json::json!({ "status": next_status })).await;
                                                        set_refresh.update(|n| *n += 1);
                                                    });
                                                }
                                            >
                                                {if is_done { "✓" } else { "" }}
                                            </div>
                                            <div class="flex-1 min-w-0">
                                                <div class="font-semibold text-sm" class:line-through=is_done style:color=move || if is_done { "var(--ink-40)" } else { "var(--ink)" }>
                                                    {let t = todo.title.clone(); move || tr(&t)}
                                                </div>
                                                <div class="flex items-center gap-2.5 mt-1 text-xs" style="color: var(--ink-40);">
                                                    <span class="font-display text-[11px] font-bold">{format!("#{}", todo.seq_num)}</span>
                                                    {todo.assigned_name.clone().map(|a| view! { <span class="font-semibold" style="color: var(--ink-70);">{format!("-> @{}", a)}</span> })}
                                                </div>
                                            </div>
                                        </div>
                                    }
                                }
                            />
                        </div>
                    }.into_any()
                })}
            </Suspense>

            // Inline create
            <div class="flex items-center gap-3 px-4 py-2.5 mt-3 border-2 border-dashed rounded-sm" style="border-color: var(--ink-15);">
                <span class="text-lg" style="color: var(--ink-40);">"+"</span>
                <input
                    type="text"
                    class="flex-1 border-none bg-transparent text-sm outline-none"
                    style="font-family: var(--font-body); color: var(--ink);"
                    prop:placeholder=move || tr("todo.add.placeholder")
                    on:input=move |ev| set_new_title.set(event_target_value(&ev))
                    on:keydown=move |ev| { if ev.key() == "Enter" { on_create(ev); } }
                    prop:value=new_title
                />
            </div>
        </div>
    }
}
