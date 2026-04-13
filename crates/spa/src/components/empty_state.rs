use leptos::prelude::*;

#[component]
pub fn EmptyState(title: String, message: String) -> impl IntoView {
    view! {
        <div class="text-center py-16 max-w-sm mx-auto">
            <h3 class="font-display text-lg font-bold">{title}</h3>
            <p class="text-sm mt-2" style="color: var(--ink-40);">{message}</p>
        </div>
    }
}
