use leptos::prelude::*;
use tw_merge::tw_merge;

/// Visual variant. Maps the existing ad-hoc button styles:
/// Primary = filled ink; Secondary = ink outline; Danger = brick outline;
/// Ghost = no border.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Danger,
    Ghost,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    #[default]
    Default,
    Sm,
}

impl ButtonVariant {
    fn classes(self) -> &'static str {
        match self {
            ButtonVariant::Primary => "bg-ink text-cream border-2 border-ink",
            ButtonVariant::Secondary => "bg-transparent text-ink border-2 border-ink",
            ButtonVariant::Danger => "bg-transparent text-brick border border-brick",
            ButtonVariant::Ghost => "bg-transparent text-ink border-2 border-transparent",
        }
    }
}

impl ButtonSize {
    fn classes(self, icon_only: bool) -> &'static str {
        match (self, icon_only) {
            (ButtonSize::Default, false) => "px-4 py-2 text-sm",
            (ButtonSize::Default, true) => "p-2",
            (ButtonSize::Sm, false) => "px-3 py-1.5 text-xs",
            (ButtonSize::Sm, true) => "p-1.5",
        }
    }
}

/// Accessible button. A real `<button>`; icon-only buttons MUST pass
/// `aria_label`. `disabled` is reactive.
#[component]
pub fn Button(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    /// Icon-only buttons get square padding and require `aria_label`.
    #[prop(optional)]
    icon_only: bool,
    /// Required for icon-only buttons; optional otherwise.
    #[prop(into, optional)]
    aria_label: MaybeProp<String>,
    #[prop(into, optional)] disabled: Signal<bool>,
    /// Fired on activation (click / Space / Enter via the native button).
    on_click: impl Fn() + 'static,
    #[prop(into, optional, default = String::new())] class: String,
    children: Children,
) -> impl IntoView {
    let merged = tw_merge!(
        "inline-flex items-center justify-center gap-1.5 font-bold rounded-xs \
         cursor-pointer transition-colors disabled:opacity-40 disabled:cursor-not-allowed \
         focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-teal",
        variant.classes(),
        size.classes(icon_only),
        class
    );
    view! {
        <button
            type="button"
            class=merged
            aria-label=move || aria_label.get()
            disabled=move || disabled.get()
            on:click=move |_| on_click()
        >
            {children()}
        </button>
    }
}
