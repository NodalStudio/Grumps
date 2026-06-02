//! Reactive toast notifications. A single [`ToastHandle`] is provided into
//! Leptos context at the app root (see [`provide_toasts`]); any component can
//! pull it with [`use_toasts`] and fire a toast. The viewport
//! ([`ToastViewport`]) is mounted once at the root and renders the live list.
//!
//! The primitive is **i18n-agnostic**: callers pass already-localized text
//! (the `tr(...)` result), so the queue holds plain `String`s.

use crate::components::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::i18n::tr;
use leptos::prelude::*;

/// Auto-dismiss delay (ms) after which a toast removes itself.
const AUTO_DISMISS_MS: u32 = 4000;

#[derive(Clone, Copy, PartialEq)]
pub enum ToastKind {
    Success,
    Error,
    // Part of the primitive's public API; no consumer fires a neutral toast yet.
    #[allow(dead_code)]
    Info,
}

#[derive(Clone)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    /// Already-localized message text (caller passes the `tr(...)` result).
    pub message: String,
}

/// Cheap-`Copy` handle to the toast queue. Holds the live `Vec<Toast>` and a
/// monotonic id counter; cloning it just copies the two signals.
#[derive(Clone, Copy)]
pub struct ToastHandle {
    toasts: RwSignal<Vec<Toast>>,
    next_id: RwSignal<u64>,
}

impl ToastHandle {
    /// Push a toast with a fresh id and schedule its auto-dismiss.
    pub fn show(&self, kind: ToastKind, message: String) {
        let mut id = 0u64;
        self.next_id.update(|n| {
            id = *n;
            *n += 1;
        });
        self.toasts
            .update(|list| list.push(Toast { id, kind, message }));

        // Fire-and-forget auto-dismiss. `dismiss` is idempotent, so a timer
        // firing after a manual dismiss is a harmless no-op.
        let handle = *self;
        gloo_timers::callback::Timeout::new(AUTO_DISMISS_MS, move || {
            handle.dismiss(id);
        })
        .forget();
    }

    pub fn success(&self, message: String) {
        self.show(ToastKind::Success, message)
    }

    pub fn error(&self, message: String) {
        self.show(ToastKind::Error, message)
    }

    fn dismiss(&self, id: u64) {
        self.toasts.update(|list| list.retain(|t| t.id != id));
    }
}

/// Create the toast queue and provide the handle into Leptos context.
/// Call exactly once, in `App`.
pub fn provide_toasts() -> ToastHandle {
    let handle = ToastHandle {
        toasts: RwSignal::new(Vec::new()),
        next_id: RwSignal::new(0),
    };
    provide_context(handle);
    handle
}

/// Pull the toast handle from context. Panics only if `provide_toasts` was not
/// called at the root (a programmer error, surfaced loudly in dev).
pub fn use_toasts() -> ToastHandle {
    use_context::<ToastHandle>().expect("ToastHandle provided at root")
}

/// Per-kind inline style: background, text colour, and the offset-block shadow.
fn kind_style(kind: ToastKind) -> &'static str {
    match kind {
        ToastKind::Success => {
            "background: var(--teal); color: var(--cream); border-color: var(--ink); \
             box-shadow: 4px 4px 0 var(--ink);"
        }
        ToastKind::Error => {
            "background: var(--brick); color: var(--cream); border-color: var(--ink); \
             box-shadow: 4px 4px 0 var(--ink);"
        }
        ToastKind::Info => {
            "background: var(--cream); color: var(--ink); border-color: var(--ink); \
             box-shadow: 4px 4px 0 var(--ink);"
        }
    }
}

/// Bottom-end anchored toast stack. Mount once at the app root.
#[component]
pub fn ToastViewport() -> impl IntoView {
    let handle = use_toasts();
    let toasts = handle.toasts;
    view! {
        <div class="fixed bottom-6 end-6 z-50 flex flex-col gap-2">
            <For each=move || toasts.get() key=|t| t.id let:toast>
                {
                    let id = toast.id;
                    view! {
                        <div
                            role="status"
                            aria-live="polite"
                            aria-atomic="true"
                            class="flex items-center gap-3 max-w-sm border-2 rounded-xs px-4 py-3 \
                                   text-sm font-medium"
                            style=kind_style(toast.kind)
                        >
                            <span class="flex-1">{toast.message.clone()}</span>
                            <Button
                                variant=ButtonVariant::Ghost
                                size=ButtonSize::Sm
                                icon_only=true
                                aria_label=tr("toast.dismiss")
                                class="shrink-0 text-current"
                                on_click=move |_| handle.dismiss(id)
                            >
                                "\u{00D7}"
                            </Button>
                        </div>
                    }
                }
            </For>
        </div>
    }
}
