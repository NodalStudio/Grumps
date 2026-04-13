use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use crate::auth::use_auth;
use crate::components::header::PageHeader;

#[component]
pub fn NoteEditorPage() -> impl IntoView {
    let auth = use_auth();
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();
    let note_id = move || params.read().get("id").unwrap_or_default();

    let note = LocalResource::new(move || {
        let api = auth.api.clone();
        let s = slug();
        let id = note_id();
        async move { api.get_note(&s, &id).await.ok() }
    });

    let (editing, set_editing) = signal(false);
    let (content, set_content) = signal(String::new());
    let (_title, set_title) = signal(String::new());

    view! {
        <Suspense fallback=|| view! { <div class="p-8" style="color: var(--ink-40);">"Loading..."</div> }>
            {move || note.get().map(|data| {
                if let Some(n) = (*data).clone() {
                    set_content.set(n.content.clone().unwrap_or_default());
                    set_title.set(n.title.clone().unwrap_or_default());
                    view! {
                        <PageHeader title=n.title.clone().unwrap_or("(untitled)".into()) subtitle=format!("Created {}", n.created_at)>
                            <button
                                class="px-4 py-2 text-sm font-semibold border-2 border-ink rounded-sm cursor-pointer"
                                style="background: var(--ink); color: var(--cream);"
                                on:click=move |_| set_editing.update(|e| *e = !*e)
                            >
                                {move || if editing.get() { "Preview" } else { "Edit" }}
                            </button>
                        </PageHeader>
                        <div class="flex-1 overflow-y-auto p-8">
                            {move || if editing.get() {
                                view! {
                                    <textarea
                                        class="w-full min-h-[400px] p-4 border-2 border-ink rounded-sm text-sm font-mono outline-none resize-y"
                                        style="background: var(--cream); font-family: 'JetBrains Mono', monospace;"
                                        prop:value=content
                                        on:input=move |ev| set_content.set(event_target_value(&ev))
                                    ></textarea>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="prose max-w-none p-6 border-2 border-ink rounded-sm" style="background: var(--cream-light);">
                                        <pre class="whitespace-pre-wrap text-sm">{content.get()}</pre>
                                    </div>
                                }.into_any()
                            }}
                        </div>
                    }.into_any()
                } else {
                    view! { <div class="p-8 font-display text-lg">"Note not found."</div> }.into_any()
                }
            })}
        </Suspense>
    }
}
