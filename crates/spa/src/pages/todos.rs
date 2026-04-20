use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use crate::auth::use_auth;
use crate::components::header::PageHeader;

#[component]
pub fn TodosPage() -> impl IntoView {
    let auth = use_auth();
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();
    let (filter, set_filter) = signal("open".to_string());
    let (refresh_counter, set_refresh) = signal(0u32);

    let api = auth.api.clone();
    let api2 = auth.api.clone();
    let todos = LocalResource::new(move || {
        let api = api.clone();
        let s = slug();
        let f = filter.get();
        let _ = refresh_counter.get();
        async move { api.get_todos(&s, &f).await.unwrap_or_default() }
    });

    let (new_title, set_new_title) = signal(String::new());
    let on_create = move |_: web_sys::KeyboardEvent| {
        let api = api2.clone();
        let s = slug();
        let title = new_title.get();
        if title.trim().is_empty() { return; }
        set_new_title.set(String::new());
        leptos::task::spawn_local(async move {
            let _ = api.create_todo(&s, &title, 2).await;
            set_refresh.update(|n| *n += 1);
        });
    };

    let filters = vec![
        ("open", "Open"), ("all", "All"), ("done", "Done"), ("mine", "Mine"),
    ];

    view! {
        <PageHeader title="Todos".into() subtitle="Manage your tasks".to_string()>
            <button
                class="px-4 py-2 text-sm font-semibold border-2 border-ink rounded-sm cursor-pointer"
                style="background: var(--ink); color: var(--cream);"
            >"+ Add todo"</button>
        </PageHeader>
        <div class="flex-1 overflow-y-auto p-8">
            // Filter bar
            <div class="flex gap-2 items-center mb-5 flex-wrap">
                {filters.into_iter().map(|(val, label)| {
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
                        >{label}</button>
                    }
                }).collect_view()}
            </div>

            // Todo list
            <Suspense fallback=|| view! { <div style="color: var(--ink-40);">"Loading..."</div> }>
                {move || todos.get().map(|data| {
                    let items: Vec<_> = (*data).clone();
                    if items.is_empty() {
                        return view! {
                            <div class="text-center py-16">
                                <p class="font-display text-lg font-bold">"Nothing to do."</p>
                                <p class="text-sm mt-1" style="color: var(--ink-40);">"Suspicious."</p>
                            </div>
                        }.into_any();
                    }
                    view! {
                        <div class="flex flex-col gap-1.5">
                            <For
                                each=move || items.clone()
                                key=|t| t.seq_num
                                children=move |todo| {
                                    let is_done = todo.status == "done";
                                    let is_high = todo.priority == 1;
                                    view! {
                                        <div
                                            class="flex items-start gap-3 px-4 py-3 border-2 border-ink rounded-sm cursor-pointer transition-transform hover:translate-x-0.5"
                                            class:border-l-4=is_high
                                            style:border-left-color=move || if is_high { "var(--brick)" } else { "" }
                                            style="background: var(--cream-light);"
                                        >
                                            <div
                                                class="w-5 h-5 border-2 border-ink rounded-sm flex-shrink-0 mt-0.5 flex items-center justify-center"
                                                style:background=move || if is_done { "var(--teal)" } else { "var(--cream-light)" }
                                                style:border-color=move || if is_done { "var(--teal)" } else { "var(--ink)" }
                                            >
                                                {if is_done { "✓" } else { "" }}
                                            </div>
                                            <div class="flex-1 min-w-0">
                                                <div class="font-semibold text-sm" class:line-through=is_done style:color=move || if is_done { "var(--ink-40)" } else { "var(--ink)" }>
                                                    {todo.title.clone()}
                                                </div>
                                                <div class="flex items-center gap-2.5 mt-1 text-xs" style="color: var(--ink-40);">
                                                    <span class="text-body-sm font-bold">{format!("#{}", todo.seq_num)}</span>
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
                    placeholder="Add a todo..."
                    on:input=move |ev| set_new_title.set(event_target_value(&ev))
                    on:keydown=move |ev| { if ev.key() == "Enter" { on_create(ev); } }
                    prop:value=new_title
                />
            </div>
        </div>
    }
}
