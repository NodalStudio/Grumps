use crate::components::header::PageHeader;
use crate::i18n::tr;
use leptos::prelude::*;

#[component]
pub fn FilesPage() -> impl IntoView {
    view! {
        <PageHeader title=tr("page.files.title") subtitle=tr("page.files.subtitle") />
        <div class="flex-1 overflow-y-auto p-8">
            // Upload zone
            <div class="border-2 border-dashed rounded-sm p-10 text-center mb-6 cursor-pointer transition-colors hover:border-ink" style="border-color: var(--ink-15);">
                <div class="text-3xl mb-2">"↑"</div>
                <div class="font-semibold">{move || tr("files.drop")}</div>
                <div class="text-xs mt-1" style="color: var(--ink-40);">{move || tr("files.drop.hint")}</div>
            </div>
            <div class="text-center py-8">
                <p class="font-display text-lg font-bold">{move || tr("files.empty.title")}</p>
                <p class="text-sm mt-1" style="color: var(--ink-40);">{move || tr("files.empty.hint")}</p>
            </div>
        </div>
    }
}
