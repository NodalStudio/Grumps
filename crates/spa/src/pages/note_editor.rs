use crate::api::use_api;
use crate::components::header::PageHeader;
use crate::components::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::components::ui::field::Field;
use crate::i18n::{tr, tr_p};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::{use_navigate, use_params_map};

#[derive(Copy, Clone, PartialEq)]
enum SaveState {
    Idle,
    Saving,
    Saved,
    Error,
}

#[component]
pub fn NoteEditorPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();
    let note_id = move || params.read().get("id").unwrap_or_default();

    let api_for_load = use_api();
    let note = LocalResource::new(move || {
        let api = api_for_load.clone();
        let s = slug();
        let id = note_id();
        async move { api.get_note(&s, &id).await.ok() }
    });

    let api_for_links = use_api();
    let links = LocalResource::new(move || {
        let api = api_for_links.clone();
        let s = slug();
        let id = note_id();
        async move { api.get_note_links(&s, &id).await.ok() }
    });

    let api_for_titles = use_api();
    let all_notes = LocalResource::new(move || {
        let api = api_for_titles.clone();
        let s = slug();
        async move { api.get_notes(&s).await.unwrap_or_default() }
    });

    // Held for the unresolved-wikilink "create note on click" action below.
    // `StoredValue` (Copy) so the doubly-nested reactive closures around the
    // preview view can each grab a fresh clone without moving the original
    // out of an enclosing `move` closure (which would make it FnOnce).
    let api_for_wikilink_create = StoredValue::new(use_api());
    let navigate = StoredValue::new_local(use_navigate());

    let (editing, set_editing) = signal(false);
    let (content, set_content) = signal(String::new());
    let (title, set_title) = signal(String::new());
    let (save_state, set_save_state) = signal(SaveState::Idle);
    // Current [[partial query, or None when the picker is closed.
    let (link_query, set_link_query) = signal(None::<String>);

    let api_for_save = use_api();
    let save = StoredValue::new(move || {
        let api = api_for_save.clone();
        let s = slug();
        let id = note_id();
        let req = crate::api::UpdateNoteRequest {
            title: Some(title.get()),
            content: content.get(),
        };
        set_save_state.set(SaveState::Saving);
        spawn_local(async move {
            match api.update_note(&s, &id, req).await {
                Ok(()) => set_save_state.set(SaveState::Saved),
                Err(_) => set_save_state.set(SaveState::Error),
            }
        });
    });

    view! {
        <Suspense fallback=|| view! { <div class="p-8" style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
            {move || note.get().map(|data| {
                if let Some(n) = data.clone() {
                    // tr() resolves demo/seed keys; real note text passes through.
                    set_content.set(n.content.clone().map(|c| tr(&c)).unwrap_or_default());
                    set_title.set(n.title.clone().map(|t| tr(&t)).unwrap_or_default());
                    let raw_title = n.title.clone();
                    let created_at = n.created_at.chars().take(10).collect::<String>();
                    view! {
                        <PageHeader
                            title=move || raw_title.clone()
                                .filter(|s| !s.is_empty())
                                .map(|s| tr(&s))
                                .unwrap_or_else(|| tr("page.note_editor.untitled"))
                            subtitle=Signal::derive(move || tr_p("page.note_editor.created", &[("date", &created_at)]))
                        >
                            <span class="text-xs me-2" style="color: var(--ink-40);">
                                {move || match save_state.get() {
                                    SaveState::Idle => String::new(),
                                    SaveState::Saving => tr("page.note_editor.saving"),
                                    SaveState::Saved => tr("page.note_editor.saved"),
                                    SaveState::Error => tr("page.note_editor.save_error"),
                                }}
                            </span>
                            <Button
                                variant=ButtonVariant::Secondary
                                size=ButtonSize::Sm
                                class="text-sm bg-cream-light"
                                on_click=move |_| set_editing.update(|e| *e = !*e)
                            >
                                {move || if editing.get() { tr("page.note_editor.preview") } else { tr("page.note_editor.edit") }}
                            </Button>
                            <Button
                                variant=ButtonVariant::Primary
                                class="py-1.5"
                                disabled=Signal::derive(move || save_state.get() == SaveState::Saving)
                                on_click=move |_| save.with_value(|f| f())
                            >
                                {move || tr("page.note_editor.save")}
                            </Button>
                        </PageHeader>
                        <div class="flex-1 overflow-y-auto p-8">
                            {move || if editing.get() {
                                view! {
                                    <div class="flex flex-col gap-3">
                                        <Field label=tr("common.title.label") id="note-title">
                                            <input
                                                id="note-title"
                                                class="w-full p-3 border-2 border-ink rounded-xs text-sm font-semibold outline-hidden"
                                                style="background: var(--cream);"
                                                placeholder=tr("common.title.placeholder")
                                                prop:value=title
                                                on:input=move |ev| set_title.set(event_target_value(&ev))
                                            />
                                        </Field>
                                        <Field label=tr("page.note_editor.content_label") id="note-content">
                                            <textarea
                                                id="note-content"
                                                class="w-full min-h-[400px] p-4 border-2 border-ink rounded-xs text-sm font-mono outline-hidden resize-y"
                                                style="background: var(--cream); font-family: 'JetBrains Mono', monospace;"
                                                prop:value=content
                                                on:input=move |ev| {
                                                    let v = event_target_value(&ev);
                                                    set_content.set(v.clone());
                                                    // Open the picker when the caret sits in an unclosed [[…
                                                    match v.rsplit_once("[[") {
                                                        Some((_, tail)) if !tail.contains("]]") && !tail.contains('\n') => {
                                                            set_link_query.set(Some(tail.to_string()));
                                                        }
                                                        _ => set_link_query.set(None),
                                                    }
                                                }
                                            ></textarea>
                                        </Field>
                                        {move || {
                                            match link_query.get() {
                                                None => ().into_any(),
                                                Some(q) => {
                                                    let qn = grumps_core::wikilink::normalize_title(&q);
                                                    let matches: Vec<_> = all_notes.get().unwrap_or_default()
                                                        .into_iter()
                                                        .filter_map(|n| n.title.clone())
                                                        .filter(|t| grumps_core::wikilink::normalize_title(t).contains(&qn))
                                                        .take(8)
                                                        .collect();
                                                    if matches.is_empty() {
                                                        view! { <div class="text-sm text-ink/50 px-2 py-1">{tr("page.note_editor.link_picker_empty")}</div> }.into_any()
                                                    } else {
                                                        view! {
                                                            <ul class="border-2 border-ink rounded-xs mt-1 bg-cream-light max-h-48 overflow-y-auto">
                                                                {matches.into_iter().map(|title_text| {
                                                                    let title_for_click = title_text.clone();
                                                                    view! {
                                                                        <li>
                                                                            <button
                                                                                type="button"
                                                                                class="w-full text-left px-2 py-1 text-sm hover:bg-cream"
                                                                                on:click=move |_| {
                                                                                    set_content.update(|c| {
                                                                                        if let Some(idx) = c.rfind("[[") {
                                                                                            c.truncate(idx);
                                                                                            c.push_str(&format!("[[{}]]", title_for_click));
                                                                                        }
                                                                                    });
                                                                                    set_link_query.set(None);
                                                                                }
                                                                            >{title_text}</button>
                                                                        </li>
                                                                    }
                                                                }).collect::<Vec<_>>()}
                                                            </ul>
                                                        }.into_any()
                                                    }
                                                }
                                            }
                                        }}
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div>
                                        <div class="prose max-w-none p-6 border-2 border-ink rounded-xs" style="background: var(--cream-light);">
                                            {move || {
                                                let slug_v = slug();
                                                let resolver: std::collections::HashMap<String, String> =
                                                    links.get().flatten().map(|l| {
                                                        l.outgoing.into_iter()
                                                            .filter_map(|o| o.id.map(|id| (o.target_norm, id)))
                                                            .collect()
                                                    }).unwrap_or_default();
                                                let api = api_for_wikilink_create.get_value();
                                                let nav = navigate.get_value();
                                                let slug_for_create = slug_v.clone();
                                                let on_create_unresolved = move |target: String| {
                                                    let api = api.clone();
                                                    let nav = nav.clone();
                                                    let s = slug_for_create.clone();
                                                    spawn_local(async move {
                                                        // Seed with an H1 so the note has
                                                        // sensible starting content AND passes
                                                        // the worker's `content` min=1 validation
                                                        // (`de_trim` trims to "# {target}").
                                                        let req = crate::api::CreateNoteRequest {
                                                            content: format!("# {}\n", target),
                                                            title: Some(target),
                                                        };
                                                        if let Ok(new_note) = api.create_note(&s, req).await {
                                                            let path = format!("{}/w/{}/notes/{}", crate::demo::router_base(), s, new_note.id);
                                                            nav(&path, Default::default());
                                                        }
                                                    });
                                                };
                                                crate::components::wikilink::render_note_content(content.get(), resolver, slug_v, on_create_unresolved)
                                            }}
                                        </div>
                                        {move || {
                                            let bl = links.get().flatten().map(|l| l.backlinks).unwrap_or_default();
                                            if bl.is_empty() {
                                                ().into_any()
                                            } else {
                                                let slug_v = slug();
                                                view! {
                                                    <div class="mt-6">
                                                        <h3 class="font-display text-sm mb-2">{move || tr("page.note_editor.backlinks_heading")}</h3>
                                                        <ul class="flex flex-col gap-1">
                                                            <For
                                                                each=move || bl.clone()
                                                                key=|r| r.id.clone()
                                                                children=move |r| {
                                                                    let href = format!("{}/w/{}/notes/{}", crate::demo::router_base(), slug_v, r.id);
                                                                    let label = r.title.clone().unwrap_or_else(|| tr("page.note_editor.untitled"));
                                                                    view! { <li><a href=href class="text-sm underline">{label}</a></li> }
                                                                }
                                                            />
                                                        </ul>
                                                    </div>
                                                }.into_any()
                                            }
                                        }}
                                    </div>
                                }.into_any()
                            }}
                        </div>
                    }.into_any()
                } else {
                    view! { <div class="p-8 font-display text-lg">{move || tr("page.note_editor.not_found")}</div> }.into_any()
                }
            })}
        </Suspense>
    }
}
