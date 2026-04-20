use leptos::prelude::*;

#[component]
pub fn PageHeader(
    title: String,
    #[prop(optional)] subtitle: Option<String>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="px-8 pt-6 pb-5 border-b-2 border-ink flex items-end justify-between gap-4" style="background: var(--cream-light);">
            <div>
                <h2 class="font-display text-2xl font-extrabold tracking-tight">{title}</h2>
                {subtitle.map(|s| view! { <p class="text-body-sm mt-0.5" style="color: var(--ink-40);">{s}</p> })}
            </div>
            {children.map(|c| view! { <div class="flex gap-2 items-center">{c()}</div> })}
        </div>
    }
}
