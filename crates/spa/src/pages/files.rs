use leptos::prelude::*;
use crate::components::header::PageHeader;

#[component]
pub fn FilesPage() -> impl IntoView {
    view! {
        <PageHeader title="Files".into() subtitle="Shared files".to_string() />
        <div class="flex-1 overflow-y-auto p-8">
            // Upload zone
            <div class="border-2 border-dashed rounded-sm p-10 text-center mb-6 cursor-pointer transition-colors hover:border-ink" style="border-color: var(--ink-15);">
                <div class="text-3xl mb-2">"↑"</div>
                <div class="font-semibold">"Drop files here"</div>
                <div class="text-xs mt-1" style="color: var(--ink-40);">"or click to browse · coming soon"</div>
            </div>
            <div class="text-center py-8">
                <p class="font-display text-lg font-bold">"No files yet."</p>
                <p class="text-sm mt-1" style="color: var(--ink-40);">"Store files from chat with @grumps store"</p>
            </div>
        </div>
    }
}
