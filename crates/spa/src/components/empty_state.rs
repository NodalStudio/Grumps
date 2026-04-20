use leptos::prelude::*;
use crate::components::Icon;

#[component]
pub fn EmptyState(
    title: String,
    message: String,
    #[prop(optional, default = "")] icon: &'static str,
    #[prop(optional)] cta: Option<(String, String)>,
) -> impl IntoView {
    let has_icon = !icon.is_empty();
    view! {
        <div class="text-center py-16 max-w-sm mx-auto">
            {has_icon.then(|| view! {
                <div class="inline-flex items-center justify-center text-muted mb-4">
                    <Icon name=icon class="size-6"/>
                </div>
            })}
            <h3 class="text-display">{title}</h3>
            <p class="text-body-sm mt-2 text-muted">{message}</p>
            {cta.map(|(label, href)| view! {
                <a href=href
                   class="inline-flex items-center gap-2 mt-6 px-4 py-2.5 text-meta border-2 border-strong rounded-sm bg-surface-raised text-primary hover:bg-hover-tint transition-colors">
                    {label}
                </a>
            })}
        </div>
    }
}
