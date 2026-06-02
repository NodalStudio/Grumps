use leptos::prelude::*;

/// Form field wrapper: renders a `<label>` bound to the control via `id`,
/// plus optional help text. The caller renders the actual input as children
/// and sets its `id=<same id>` so the `<label for>` association is correct.
#[component]
pub fn Field(
    /// Localized label text.
    #[prop(into)]
    label: String,
    /// Stable id for the control; used as the `<label for>`.
    #[prop(into)]
    id: String,
    /// Optional localized help text shown under the control. When set, a
    /// `<p id="{id}-help">` is rendered; the caller's input should carry
    /// `aria-describedby=format!("{}-help", id)` so screen readers announce it.
    #[prop(into, optional)]
    help: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    let help_id = format!("{}-help", id);
    view! {
        <div class="flex flex-col gap-1.5 py-2">
            <label
                for=id
                class="text-[11px] font-bold uppercase tracking-wider"
                style="color: var(--ink-40);"
            >
                {label}
            </label>
            {children()}
            <Show when=move || help.get().is_some()>
                <p id=help_id.clone() class="text-xs" style="color: var(--ink-40);">
                    {move || help.get().unwrap_or_default()}
                </p>
            </Show>
        </div>
    }
}
