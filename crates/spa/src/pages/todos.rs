use crate::api::{use_api, MemberItem, TodoItem};
use crate::components::header::PageHeader;
use crate::components::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::components::ui::checkbox::Checkbox;
use crate::components::ui::dialog::Dialog;
use crate::components::ui::field::Field;
use crate::components::ui::select::Select;
use crate::components::ui::tabs::{TabItem, Tabs};
use crate::i18n::{tr, tr_err};
use grumps_core::todo::Priority;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use wasm_bindgen::JsCast;

/// Deadline chip: localized short date (via `format_civil_date`), "today"
/// spelled out, and the brick/urgent accent when overdue — matches the
/// chat card's `⏰ {deadline}` meta field (see `formatter::task_card` /
/// `handler::deadline_display_info`).
fn deadline_chip(deadline: &str, tz: &str, locale: &str) -> impl IntoView {
    let (y, m, d) = crate::datetime::today_in_tz(tz);
    let today_str = format!("{:04}-{:02}-{:02}", y, m, d);
    let overdue = deadline < today_str.as_str();
    let label = if deadline == today_str {
        tr("date.today")
    } else {
        crate::datetime::format_civil_date(deadline, locale)
    };
    view! {
        <span
            class="font-semibold"
            style:color=move || if overdue { "var(--brick)" } else { "var(--ink-70)" }
        >
            "⏰ " {label}
        </span>
    }
}

/// Three-state priority chip: High gets the brick/urgent accent (matching
/// the chat card's 🔴), Low is muted, Normal renders nothing at all — same
/// visual order as `formatter::task_card`.
fn priority_chip(priority: i32) -> Option<impl IntoView> {
    match Priority::from_int(priority) {
        Priority::High => Some(view! {
            <span class="font-bold" style="color: var(--brick);">"🔴 "{move || tr("todo.priority.high")}</span>
        }.into_any()),
        Priority::Low => Some(view! {
            <span style="color: var(--ink-40);">"🔵 "{move || tr("todo.priority.low")}</span>
        }.into_any()),
        Priority::Normal => None,
    }
}

fn parse_tags(tags_json: &str) -> Vec<String> {
    serde_json::from_str(tags_json).unwrap_or_default()
}

#[component]
pub fn TodosPage() -> impl IntoView {
    let params = use_params_map();
    let slug = Memo::new(move |_| params.read().get("slug").unwrap_or_default());
    let filter = RwSignal::new("open".to_string());
    let (refresh_counter, set_refresh) = signal(0u32);
    // Optimistic-toggle set: while the API call is in flight + fade-out runs,
    // these ids render in the "next" status so the user sees the transition
    // even if the request takes longer than the animation.
    let (toggling, set_toggling) = signal::<Vec<String>>(Vec::new());

    let toasts = crate::components::ui::toast::use_toasts();

    let api = use_api();
    let api2 = use_api();
    let api3 = use_api();
    let todos = LocalResource::new(move || {
        let api = api.clone();
        let s = slug.get();
        let f = filter.get();
        let _ = refresh_counter.get();
        let _ = crate::refresh::use_refresh().get();
        async move { api.get_todos(&s, &f).await.unwrap_or_default() }
    });

    // Workspace members, for the assignee picker in the edit modal.
    let api_members = use_api();
    let members = LocalResource::new(move || {
        let api = api_members.clone();
        let s = slug.get();
        async move { api.get_members(&s).await.unwrap_or_default() }
    });

    let (new_title, set_new_title) = signal(String::new());
    let on_create = move |_: web_sys::KeyboardEvent| {
        let api = api2.clone();
        let s = slug.get();
        let title = new_title.get();
        if title.trim().is_empty() {
            return;
        }
        set_new_title.set(String::new());
        let req = crate::api::CreateTodoRequest {
            title: title.trim().to_string(),
            priority: Some(2),
            ..Default::default()
        };
        leptos::task::spawn_local(async move {
            match api.create_todo(&s, req).await {
                Ok(_) => set_refresh.update(|n| *n += 1),
                Err(code) => toasts.error(tr_err(&code)),
            }
        });
    };

    // --- Inline title edit ---------------------------------------------
    let (editing_id, set_editing_id) = signal::<Option<String>>(None);
    let (title_buf, set_title_buf) = signal(String::new());
    let api_title = use_api();
    let start_title_edit = move |id: String, current: String| {
        set_title_buf.set(current);
        set_editing_id.set(Some(id));
    };
    // `StoredValue` (Copy) rather than a bare closure: `save_title` owns an
    // `ApiHandle` (Clone, not Copy), and it's called from several sibling
    // reactive closures nested under the `<For>` row (title `on:blur` /
    // `on:keydown`) that must each independently implement `Fn` — a bare
    // `move` closure can only be captured by ONE of them. Mirrors
    // `note_editor.rs`'s `save` handle.
    let save_title = StoredValue::new(move || {
        let Some(id) = editing_id.get() else {
            return;
        };
        let new_title = title_buf.get();
        set_editing_id.set(None);
        if new_title.trim().is_empty() {
            return;
        }
        let api = api_title.clone();
        let s = slug.get();
        leptos::task::spawn_local(async move {
            let body = serde_json::json!({ "title": new_title.trim() });
            match api.update_todo(&s, &id, &body).await {
                Ok(()) => set_refresh.update(|n| *n += 1),
                Err(code) => toasts.error(tr_err(&code)),
            }
        });
    });

    // --- Edit modal (assignee / deadline / priority) --------------------
    let edit_open = RwSignal::new(false);
    let (edit_todo, set_edit_todo) = signal::<Option<TodoItem>>(None);
    let (form_assignee, set_form_assignee) = signal(String::new()); // member id, "" = unassigned
    let (form_deadline, set_form_deadline) = signal(String::new());
    let (form_priority, set_form_priority) = signal("2".to_string());

    let open_edit = move |t: TodoItem| {
        set_form_assignee.set(String::new()); // no reverse id lookup from a display name; picker starts unset
        set_form_deadline.set(t.deadline.clone().unwrap_or_default());
        set_form_priority.set(t.priority.to_string());
        set_edit_todo.set(Some(t));
        edit_open.set(true);
    };

    let api_edit = use_api();
    // `StoredValue`: same reason as `save_title` — owns an `ApiHandle`, used
    // as `Dialog`'s `ChildrenFn` content (re-rendered on each open).
    let save_edit = StoredValue::new(move || {
        let Some(t) = edit_todo.get() else {
            return;
        };
        let s = slug.get();
        let api = api_edit.clone();
        let priority: i32 = form_priority.get().parse().unwrap_or(2);
        let deadline = form_deadline.get();
        let assignee_id = form_assignee.get();
        let assignee_name = members
            .get()
            .unwrap_or_default()
            .into_iter()
            .find(|m| m.id == assignee_id)
            .and_then(|m| m.display_name);
        edit_open.set(false);
        leptos::task::spawn_local(async move {
            let mut body = serde_json::json!({
                "priority": priority,
                "deadline": deadline,
            });
            // Only touch assignment when the picker moved off its initial
            // "unset" state — an empty id there would otherwise leave the
            // fields alone anyway (empty-string sentinel, see
            // `db::workspace::update_todo`), but skip sending them entirely
            // to keep the intent explicit.
            if !assignee_id.is_empty() {
                body["assigned_to"] = serde_json::Value::String(assignee_id);
                body["assigned_name"] =
                    serde_json::Value::String(assignee_name.unwrap_or_default());
            }
            match api.update_todo(&s, &t.id, &body).await {
                Ok(()) => {
                    toasts.success(tr("toast.todo_updated"));
                    set_refresh.update(|n| *n += 1);
                }
                Err(code) => toasts.error(tr_err(&code)),
            }
        });
    });

    // --- Delete confirm ---------------------------------------------------
    let del_open = RwSignal::new(false);
    let (delete_id, set_delete_id) = signal::<Option<String>>(None);
    let api_del = use_api();
    // `StoredValue`: same reason as `save_title`/`save_edit`.
    let confirm_del = StoredValue::new(move || {
        if let Some(id) = delete_id.get() {
            let api = api_del.clone();
            let s = slug.get();
            set_delete_id.set(None);
            del_open.set(false);
            leptos::task::spawn_local(async move {
                match api.delete_todo(&s, &id).await {
                    Ok(()) => {
                        toasts.success(tr("toast.todo_deleted"));
                        set_refresh.update(|n| *n += 1);
                    }
                    Err(code) => toasts.error(tr_err(&code)),
                }
            });
        }
    });

    let filter_tabs = vec![
        TabItem {
            value: "open".into(),
            label_key: "todo.filter.open".into(),
        },
        TabItem {
            value: "all".into(),
            label_key: "todo.filter.all".into(),
        },
        TabItem {
            value: "done".into(),
            label_key: "todo.filter.done".into(),
        },
        TabItem {
            value: "mine".into(),
            label_key: "todo.filter.mine".into(),
        },
    ];

    let focus_new_todo = move |_| {
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        if let Some(el) = doc.get_element_by_id("new-todo-input") {
            if let Ok(input) = el.dyn_into::<web_sys::HtmlInputElement>() {
                input.scroll_into_view();
                let _ = input.focus();
            }
        }
    };

    view! {
        <PageHeader title=move || tr("page.todos.title") subtitle=Signal::derive(move || tr("page.todos.subtitle"))>
            <Button variant=ButtonVariant::Primary on_click=focus_new_todo>
                "+ "{move || tr("todo.action.add")}
            </Button>
        </PageHeader>
        <div class="flex-1 overflow-y-auto p-8">
            // Filter bar
            <Tabs
                value=filter
                tabs=filter_tabs
                aria_label=Signal::derive(move || tr("todo.filter.aria"))
                class="mb-5"
            />

            // Todo list
            <Suspense fallback=|| view! { <div style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
                {move || todos.get().map(|data| {
                    let items: Vec<_> = data.clone();
                    if items.is_empty() {
                        return view! {
                            <div class="text-center py-16">
                                <p class="font-display text-lg font-bold">{move || tr("todo.empty.title")}</p>
                                <p class="text-sm mt-1" style="color: var(--ink-40);">{move || tr("todo.empty.sub")}</p>
                            </div>
                        }.into_any();
                    }
                    let api_for_children = api3.clone();
                    let tz = crate::datetime::use_timezone();
                    let locale = crate::i18n::use_locale().code().to_string();
                    view! {
                        <div class="flex flex-col gap-1.5">
                            <For
                                each=move || items.clone()
                                key=|t| t.seq_num
                                children=move |todo| {
                                    let was_done = todo.status == "done";
                                    let is_high = todo.priority == 1;
                                    let id = todo.id.clone();
                                    let api_for_click = api_for_children.clone();
                                    let slug_for_click = slug.get();
                                    // Reactive: looks up whether this row is currently mid-toggle.
                                    let id_for_check = id.clone();
                                    let id_for_dom = id.clone();
                                    let is_toggling = Signal::derive(move || toggling.with(|v| v.iter().any(|x| x == &id_for_check)));
                                    // Effective "done" state for visuals = original XOR toggling.
                                    let effective_done = Signal::derive(move || was_done ^ is_toggling.get());

                                    let is_editing_title = {
                                        let id = id.clone();
                                        Signal::derive(move || editing_id.get().as_deref() == Some(id.as_str()))
                                    };
                                    let title_for_start = todo.title.clone();
                                    let id_for_start = id.clone();
                                    let title_for_display = todo.title.clone();

                                    let tags = parse_tags(&todo.tags);
                                    let deadline = todo.deadline.clone();
                                    let assignee = todo.assigned_name.clone();
                                    let priority = todo.priority;
                                    let tz = tz.clone();
                                    let locale = locale.clone();

                                    let todo_for_edit = todo.clone();
                                    let id_for_delete = id.clone();

                                    view! {
                                        <div
                                            class="todo-row flex items-start gap-3 px-4 py-3 border-2 border-ink rounded-xs"
                                            class:border-s-4=is_high
                                            class:todo-row-fading=move || is_toggling.get()
                                            style:border-inline-start-color=move || if is_high { "var(--brick)" } else { "" }
                                            style="background: var(--cream-light); transition: opacity 280ms ease-out, transform 280ms ease-out, max-height 280ms ease-out, padding 280ms ease-out, margin 280ms ease-out, border-width 280ms ease-out;"
                                        >
                                            <Checkbox
                                                checked=effective_done
                                                on_change=move || {
                                                    if is_toggling.get() { return; } // de-bounce double-clicks
                                                    let api = api_for_click.clone();
                                                    let s = slug_for_click.clone();
                                                    let id_c = id.clone();
                                                    let next_status = if was_done { "open" } else { "done" };
                                                    set_toggling.update(|v| v.push(id_c.clone()));
                                                    leptos::task::spawn_local(async move {
                                                        let body = serde_json::json!({ "status": next_status });
                                                        let api_call = api.update_todo(&s, &id_c, &body);
                                                        let timer = gloo_timers::future::TimeoutFuture::new(280);
                                                        let (result, _) = futures::join!(api_call, timer);
                                                        set_refresh.update(|n| *n += 1);
                                                        set_toggling.update(|v| v.retain(|x| x != &id_c));
                                                        if let Err(code) = result {
                                                            toasts.error(tr_err(&code));
                                                        }
                                                    });
                                                }
                                                id=format!("todo-{}", id_for_dom)
                                                aria_label=tr("todos.toggle_done")
                                                class="size-5 shrink-0 mt-0.5"
                                            />
                                            <div class="flex-1 min-w-0">
                                                {move || if is_editing_title.get() {
                                                    view! {
                                                        <input
                                                            type="text"
                                                            class="w-full border-2 border-ink rounded-xs px-2 py-0.5 text-sm font-semibold outline-hidden"
                                                            style="background: var(--cream);"
                                                            aria-label=move || tr("common.title.label")
                                                            prop:value=title_buf
                                                            on:input=move |ev| set_title_buf.set(event_target_value(&ev))
                                                            on:blur=move |_| save_title.with_value(|f| f())
                                                            on:keydown=move |ev| {
                                                                match ev.key().as_str() {
                                                                    "Enter" => save_title.with_value(|f| f()),
                                                                    "Escape" => set_editing_id.set(None),
                                                                    _ => {}
                                                                }
                                                            }
                                                        />
                                                    }.into_any()
                                                } else {
                                                    let title_for_start = title_for_start.clone();
                                                    let id_for_start = id_for_start.clone();
                                                    view! {
                                                        <div
                                                            class="font-semibold text-sm cursor-text"
                                                            class:line-through=move || effective_done.get()
                                                            style="transition: color 180ms ease-out, text-decoration-color 180ms ease-out;"
                                                            style:color=move || if effective_done.get() { "var(--ink-40)" } else { "var(--ink)" }
                                                            on:click=move |_| start_title_edit(id_for_start.clone(), tr(&title_for_start))
                                                        >
                                                            {let t = title_for_display.clone(); move || tr(&t)}
                                                        </div>
                                                    }.into_any()
                                                }}
                                                <div class="flex items-center gap-2.5 mt-1 text-xs flex-wrap" style="color: var(--ink-40);">
                                                    <span class="font-display text-[11px] font-bold">{format!("#{}", todo.seq_num)}</span>
                                                    {assignee.clone().map(|a| view! { <span class="font-semibold" style="color: var(--ink-70);">{format!("@{}", a)}</span> })}
                                                    {deadline.clone().map(|d| deadline_chip(&d, &tz, &locale))}
                                                    {priority_chip(priority)}
                                                    {(!tags.is_empty()).then(|| {
                                                        let joined = tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" ");
                                                        view! { <span>{joined}</span> }
                                                    })}
                                                </div>
                                            </div>
                                            <div class="flex items-center gap-1 shrink-0">
                                                <Button
                                                    variant=ButtonVariant::Ghost
                                                    size=ButtonSize::Sm
                                                    icon_only=true
                                                    aria_label=tr("common.edit")
                                                    on_click=move |_| open_edit(todo_for_edit.clone())
                                                >
                                                    "\u{270E}"
                                                </Button>
                                                <Button
                                                    variant=ButtonVariant::Ghost
                                                    size=ButtonSize::Sm
                                                    icon_only=true
                                                    aria_label=tr("common.delete")
                                                    class="text-brick"
                                                    on_click=move |_| {
                                                        set_delete_id.set(Some(id_for_delete.clone()));
                                                        del_open.set(true);
                                                    }
                                                >
                                                    "\u{2715}"
                                                </Button>
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
            <div class="flex items-center gap-3 px-4 py-2.5 mt-3 border-2 border-dashed rounded-xs" style="border-color: var(--ink-15);">
                <span class="text-lg" style="color: var(--ink-40);">"+"</span>
                <input
                    id="new-todo-input"
                    type="text"
                    class="flex-1 border-none bg-transparent text-sm outline-hidden"
                    style="font-family: var(--font-body); color: var(--ink);"
                    prop:placeholder=move || tr("todo.add.placeholder")
                    on:input=move |ev| set_new_title.set(event_target_value(&ev))
                    on:keydown=move |ev| { if ev.key() == "Enter" { on_create(ev); } }
                    prop:value=new_title
                />
            </div>
        </div>

        // Edit modal: assignee / deadline / priority
        <Dialog
            open=edit_open
            on_close=move || edit_open.set(false)
            labelledby="todo-edit-title"
            class="max-w-sm"
        >
            <h2 id="todo-edit-title" class="font-display text-xl font-bold">{move || tr("todo.modal.edit")}</h2>

            <div class="flex flex-col gap-1.5 py-2">
                <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">{move || tr("todo.field.assignee")}</label>
                <Select
                    value=form_assignee
                    aria_label=tr("todo.field.assignee")
                    full_width=true
                    on_change=move |v: String| set_form_assignee.set(v)
                >
                    <option value="">{move || tr("todo.field.assignee.unassigned")}</option>
                    {move || members.get().unwrap_or_default().into_iter().map(|m: MemberItem| {
                        let label = m.display_name.clone().unwrap_or_else(|| m.platform_user_id.clone());
                        view! { <option value=m.id.clone()>{label}</option> }
                    }).collect_view()}
                </Select>
            </div>

            <Field label=tr("todo.field.deadline") id="todo-deadline">
                <input
                    id="todo-deadline"
                    type="date"
                    class="w-full border-2 border-ink rounded-xs px-3 py-2 text-sm bg-transparent outline-hidden"
                    on:input=move |ev| set_form_deadline.set(event_target_value(&ev))
                    prop:value=form_deadline
                />
            </Field>

            <div class="flex flex-col gap-1.5 py-2">
                <label class="text-[11px] font-bold uppercase tracking-wider" style="color: var(--ink-40);">{move || tr("todo.field.priority")}</label>
                <Select
                    value=form_priority
                    aria_label=tr("todo.field.priority")
                    full_width=true
                    on_change=move |v: String| set_form_priority.set(v)
                >
                    <option value="1">{move || tr("todo.priority.high")}</option>
                    <option value="2">{move || tr("todo.priority.normal")}</option>
                    <option value="3">{move || tr("todo.priority.low")}</option>
                </Select>
            </div>

            <div class="flex gap-2 pt-2">
                <Button
                    variant=ButtonVariant::Primary
                    class="flex-1"
                    on_click=move |_| save_edit.with_value(|f| f())
                >{move || tr("common.save")}</Button>
                <Button
                    variant=ButtonVariant::Secondary
                    on_click=move |_| edit_open.set(false)
                >{move || tr("common.cancel")}</Button>
            </div>
        </Dialog>

        // Delete confirm modal
        <Dialog
            open=del_open
            on_close=move || { set_delete_id.set(None); del_open.set(false); }
            labelledby="todo-delete-title"
            class="max-w-sm"
        >
            <h2 id="todo-delete-title" class="font-display text-lg font-bold">{move || tr("todo.delete.title")}</h2>
            <p class="text-sm" style="color: var(--ink-70);">{move || tr("common.irreversible")}</p>
            <div class="flex gap-2">
                <Button
                    variant=ButtonVariant::Danger
                    class="flex-1 bg-brick text-white border-2 border-brick"
                    on_click=move |_| confirm_del.with_value(|f| f())
                >{move || tr("common.delete")}</Button>
                <Button
                    variant=ButtonVariant::Secondary
                    on_click=move |_| { set_delete_id.set(None); del_open.set(false); }
                >{move || tr("common.cancel")}</Button>
            </div>
        </Dialog>
    }
}
