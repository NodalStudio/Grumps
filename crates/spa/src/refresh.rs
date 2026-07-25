//! Global "refetch on focus" trigger for workspace pages.
//!
//! Chat-made changes (a todo done from Telegram, a note edited via `@grumps
//! edit`, …) never appear in an already-open SPA tab because every page's
//! `LocalResource` only reruns when its own local signals change. This
//! module provides one shared counter, bumped by [`install_focus_refresh`]
//! on `window focus` / `document visibilitychange` (and by the manual
//! refresh button in [`crate::components::header::PageHeader`]); pages that
//! want to stay live read it inside their resource closure alongside their
//! own dependencies, e.g. `let _ = crate::refresh::use_refresh().get();`.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Clone, Copy)]
pub struct RefreshSignal(pub RwSignal<u32>);

/// Install the shared counter into context. Call once, from
/// `WorkspaceLayout`.
pub fn provide_refresh() -> RwSignal<u32> {
    let sig = RwSignal::new(0u32);
    provide_context(RefreshSignal(sig));
    sig
}

/// Current refresh counter, or a fresh (never-bumped) one if called outside
/// the workspace layout — matches the fallback style of `use_timezone()`.
pub fn use_refresh() -> RwSignal<u32> {
    use_context::<RefreshSignal>()
        .map(|s| s.0)
        .unwrap_or_else(|| RwSignal::new(0))
}

/// Attach `window` `focus` and `document` `visibilitychange` listeners that
/// bump `sig` — so pages depending on it refetch whenever the tab regains
/// attention. Leaked intentionally: the listeners live for the lifetime of
/// the workspace layout, which itself lives for the lifetime of the tab.
pub fn install_focus_refresh(sig: RwSignal<u32>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };

    let bump = move || sig.update(|n| *n = n.wrapping_add(1));

    let focus_cb = Closure::<dyn FnMut()>::new(bump);
    let _ = window.add_event_listener_with_callback("focus", focus_cb.as_ref().unchecked_ref());
    focus_cb.forget();

    let doc_for_visibility = document.clone();
    let visibility_cb = Closure::<dyn FnMut()>::new(move || {
        if doc_for_visibility.visibility_state() == web_sys::VisibilityState::Visible {
            bump();
        }
    });
    let _ = document.add_event_listener_with_callback(
        "visibilitychange",
        visibility_cb.as_ref().unchecked_ref(),
    );
    visibility_cb.forget();
}
