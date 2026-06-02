use leptos::prelude::*;
use tw_merge::tw_merge;

/// Styled wrapper around a native `<select>` (keeps native a11y + OS
/// keyboard). The caller provides `<option>` children and an `on_change`.
#[component]
pub fn Select(
    /// Current value, bound to the native select via `prop:value`.
    #[prop(into)]
    value: Signal<String>,
    /// Fired with the new value on change.
    on_change: impl Fn(String) + 'static,
    #[prop(into, optional)] aria_label: MaybeProp<String>,
    #[prop(into, optional, default = String::new())] class: String,
    children: Children,
) -> impl IntoView {
    let merged = tw_merge!(
        "appearance-none border-2 border-ink rounded-xs pl-3 pr-8 py-1.5 text-sm \
         bg-transparent cursor-pointer outline-hidden \
         focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-teal",
        class
    );
    view! {
        <div class="relative inline-flex items-center">
            <select
                class=merged
                aria-label=move || aria_label.get()
                prop:value=value
                on:change=move |ev| on_change(event_target_value(&ev))
            >
                {children()}
            </select>
            <span
                aria-hidden="true"
                class="pointer-events-none absolute right-2 text-ink select-none"
            >
                "▾"
            </span>
        </div>
    }
}
