use leptos::prelude::*;
use tw_merge::tw_merge;

/// Accessible, **controlled** switch styled to the Grumps design system.
/// The caller owns the state: `checked` is read reactively and `on_change`
/// fires on activation (click / Space / Enter). Vertical centering is done
/// by `items-center` (no hand-positioned knob).
#[component]
pub fn Switch(
    /// Current on/off state, owned by the parent.
    #[prop(into)] checked: Signal<bool>,
    /// Fired when the user toggles the control.
    on_change: impl Fn() + 'static,
    /// Accessible label (already localized by the caller).
    #[prop(into, optional, default = String::new())] aria_label: String,
    /// Extra classes merged onto the track.
    #[prop(into, optional, default = String::new())] class: String,
) -> impl IntoView {
    let state = move || if checked.get() { "checked" } else { "unchecked" };
    let track = tw_merge!(
        "relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center \
         rounded-full border-2 border-ink px-[3px] transition-colors \
         focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-teal \
         data-[state=checked]:bg-teal data-[state=unchecked]:bg-cream",
        class
    );
    view! {
        <button
            type="button"
            role="switch"
            aria-checked=move || checked.get().to_string()
            aria-label=aria_label
            data-state=state
            class=track
            on:click=move |_| on_change()
        >
            <span
                data-state=state
                class="pointer-events-none block size-4 rounded-full transition-transform \
                       data-[state=checked]:translate-x-[18px] \
                       data-[state=checked]:bg-cream data-[state=unchecked]:bg-ink"
            ></span>
        </button>
    }
}
