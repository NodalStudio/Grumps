use leptos::prelude::*;
use tw_merge::tw_merge;

/// Accessible checkbox — a real `<input type="checkbox">` styled to the
/// design system. Controlled by the caller's signal.
#[component]
pub fn Checkbox(
    #[prop(into)] checked: Signal<bool>,
    on_change: impl Fn() + 'static,
    /// Used as the input id (and the `for` of an adjacent label).
    #[prop(into)]
    id: String,
    #[prop(into, optional)] aria_label: MaybeProp<String>,
    #[prop(into, optional, default = String::new())] class: String,
) -> impl IntoView {
    let merged = tw_merge!(
        "size-4 border-2 border-ink rounded-xs cursor-pointer accent-teal \
         focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-teal",
        class
    );
    view! {
        <input
            type="checkbox"
            id=id
            class=merged
            aria-label=move || aria_label.get()
            prop:checked=move || checked.get()
            on:change=move |_| on_change()
        />
    }
}
