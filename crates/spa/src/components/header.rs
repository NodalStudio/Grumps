use leptos::prelude::*;
use leptos::IntoView;

/// Page header with reactive title — accepts anything that implements
/// `IntoView` for the title (String, &str, closure returning String, etc.).
/// This lets pages pass a `move || expensive_lookup()` closure that re-runs
/// reactively when its dependencies change (e.g. when SessionContext is
/// populated by AuthGate after the initial render).
#[component]
pub fn PageHeader<T>(
    title: T,
    #[prop(optional, into)] subtitle: Option<String>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView
where
    T: IntoView + 'static,
{
    view! {
        <div class="px-8 h-24 pb-5 border-b-2 border-ink flex items-end justify-between gap-4" style="background: var(--cream-light);">
            <div>
                <h2 class="font-display text-2xl font-extrabold tracking-tight">{title}</h2>
                {subtitle.map(|s| view! { <p class="text-[13px] mt-0.5" style="color: var(--ink-40);">{s}</p> })}
            </div>
            {children.map(|c| view! { <div class="flex gap-2 items-center">{c()}</div> })}
        </div>
    }
}
