use crate::api::use_api;
use crate::components::header::PageHeader;
use crate::i18n::tr;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[component]
pub fn NotesPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let notes = LocalResource::new(move || {
        let api = use_api();
        let s = slug();
        async move { api.get_notes(&s).await.unwrap_or_default() }
    });

    view! {
        <PageHeader title=tr("page.notes.title") subtitle=tr("page.notes.subtitle")>
            <a href=format!("/w/{}/notes/new", slug()) class="px-4 py-2 text-sm font-semibold border-2 border-ink rounded-xs cursor-pointer" style="background: var(--ink); color: var(--cream);">"+ "{move || tr("note.action.new")}</a>
        </PageHeader>
        <div class="flex-1 overflow-y-auto p-8">
            <Suspense fallback=|| view! { <div style="color: var(--ink-40);">{move || tr("common.loading")}</div> }>
                {move || notes.get().map(|data| {
                    let items: Vec<_> = data.clone();
                    if items.is_empty() {
                        return view! {
                            <div class="text-center py-16">
                                <p class="font-display text-lg font-bold">{move || tr("note.empty.title")}</p>
                                <p class="text-sm mt-1" style="color: var(--ink-40);">{move || tr("note.empty.hint")}</p>
                            </div>
                        }.into_any();
                    }
                    view! {
                        <div class="grid gap-3.5" style="grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));">
                            <For
                                each=move || items.clone()
                                key=|n| n.id.clone()
                                children=move |note| {
                                    let href = format!("/w/{}/notes/{}", slug(), note.id);
                                    let pinned = note.pinned.unwrap_or(0) == 1;
                                    view! {
                                        <a href=href class="block p-5 border-2 rounded-xs cursor-pointer transition-transform hover:-translate-y-0.5"
                                            style:border-color=move || if pinned { "var(--ochre)" } else { "var(--ink)" }
                                            style="background: var(--cream-light);">
                                            {if pinned { Some(view! { <div class="text-[11px] font-bold uppercase tracking-wider mb-2" style="color: var(--ochre);">{move || tr("note.pinned")}</div> }) } else { None }}
                                            <h3 class="font-display text-base font-bold mb-2">{
                                                let title = note.title.clone().unwrap_or_default();
                                                move || if title.is_empty() { tr("note.untitled") } else { tr(&title) }
                                            }</h3>
                                            <p class="text-[13px] line-clamp-3" style="color: var(--ink-70);">{let c = note.content.clone().unwrap_or_default(); move || tr(&c)}</p>
                                            <div class="flex items-center justify-between mt-3 pt-3 text-[11px] border-t" style="color: var(--ink-40); border-color: var(--ink-15);">
                                                <span>{let t = note.created_at.clone(); move || crate::datetime::format_instant(&t, &crate::datetime::use_timezone(), crate::i18n::use_locale().code())}</span>
                                                <span class="font-semibold uppercase tracking-wider px-1.5 py-0.5 border rounded-xs" style="border-color: var(--ink-15);">{note.source.clone()}</span>
                                            </div>
                                        </a>
                                    }
                                }
                            />
                        </div>
                    }.into_any()
                })}
            </Suspense>
        </div>
    }
}
