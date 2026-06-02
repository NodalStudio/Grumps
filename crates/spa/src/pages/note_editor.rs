use crate::api::use_api;
use crate::components::header::PageHeader;
use crate::components::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::i18n::{tr, tr_p};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

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

    let (editing, set_editing) = signal(false);
    let (content, set_content) = signal(String::new());
    let (title, set_title) = signal(String::new());
    let (save_state, set_save_state) = signal(SaveState::Idle);

    let api_for_save = use_api();
    let save = StoredValue::new(move || {
        let api = api_for_save.clone();
        let s = slug();
        let id = note_id();
        let t = title.get();
        let c = content.get();
        set_save_state.set(SaveState::Saving);
        spawn_local(async move {
            match api.update_note(&s, &id, &t, &c).await {
                Ok(()) => set_save_state.set(SaveState::Saved),
                Err(_) => set_save_state.set(SaveState::Error),
            }
        });
    });

    view! {
        <Suspense fallback=|| view! { <div class="p-8" style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
            {move || note.get().map(|data| {
                if let Some(n) = data.clone() {
                    set_content.set(n.content.clone().unwrap_or_default());
                    set_title.set(n.title.clone().unwrap_or_default());
                    let header_title = n.title.clone()
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| tr("page.note_editor.untitled"));
                    let created_at = n.created_at.chars().take(10).collect::<String>();
                    view! {
                        <PageHeader
                            title=header_title
                            subtitle=tr_p("page.note_editor.created", &[("date", &created_at)])
                        >
                            <span class="text-xs mr-2" style="color: var(--ink-40);">
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
                                        <input
                                            class="w-full p-3 border-2 border-ink rounded-xs text-sm font-semibold outline-hidden"
                                            style="background: var(--cream);"
                                            placeholder=tr("common.title.placeholder")
                                            prop:value=title
                                            on:input=move |ev| set_title.set(event_target_value(&ev))
                                        />
                                        <textarea
                                            class="w-full min-h-[400px] p-4 border-2 border-ink rounded-xs text-sm font-mono outline-hidden resize-y"
                                            style="background: var(--cream); font-family: 'JetBrains Mono', monospace;"
                                            prop:value=content
                                            on:input=move |ev| set_content.set(event_target_value(&ev))
                                        ></textarea>
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="prose max-w-none p-6 border-2 border-ink rounded-xs" style="background: var(--cream-light);">
                                        <pre class="whitespace-pre-wrap text-sm">{content.get()}</pre>
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
