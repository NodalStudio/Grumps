use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, KeyboardEvent};

/// Where the panel sits. `Center` is a classic centered modal; `End` is a
/// full-height slide-over drawer anchored to the inline-end edge (right in LTR,
/// left in RTL). Both share the same focus-trap / dismissal machinery.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogSide {
    #[default]
    Center,
    End,
}

/// Backdrop click listener: its `EventTarget` plus the `Closure` keeping the
/// JS shim alive, parked in [`Teardown`] so it can be detached on close.
type BackdropListener = (web_sys::EventTarget, Closure<dyn FnMut(web_sys::Event)>);

/// Accessible, **controlled** modal dialog styled to the Grumps design system.
///
/// The caller owns visibility via the `open` signal. The dialog only *reads*
/// `open`; it never writes it. When the user requests dismissal (Escape, or a
/// backdrop click when `close_on_backdrop` is true) the dialog calls
/// `on_close`. The caller's `on_close` handler is responsible for flipping
/// `open` to `false` (and for any other side effects). Buttons inside the body
/// that should close the dialog should likewise call back into the caller.
///
/// Accessibility: renders `role="dialog"` + `aria-modal="true"`, wires
/// `aria-labelledby`/`aria-describedby` from props (the caller supplies a title
/// element carrying the matching id), traps Tab focus within the panel, moves
/// focus into the dialog on open, restores focus to the trigger on close, and
/// locks body scroll while open.
///
/// ## How the side effects are wired
/// The reactive view is split in two:
/// - A reactive `move ||` closure (not `<Show>`, whose children are
///   `Send + Sync`-bound and so reject the CSR `NodeRef`s) tracks `open` and
///   mounts the backdrop + panel. The panel carries a `node_ref` so the effect
///   can query its focusables; the backdrop carries one for its click handler.
/// - A single `Effect` (which runs on the local CSR runtime and may capture
///   non-`Send` `Closure`/`StoredValue` state) tracks `open`. On the open
///   transition it saves the trigger, attaches the document keydown listener
///   (Escape + focus trap) and a backdrop click listener, locks body scroll,
///   and moves focus into the panel. The teardown is parked in a `StoredValue`
///   and run on the next close transition or by `on_cleanup` on dispose —
///   removing listeners, restoring scroll, and restoring focus, with no
///   duplicate handlers or leaked listeners across open→close→open cycles.
#[component]
pub fn Dialog(
    /// Caller-owned visibility. The dialog reads this and signals dismissal
    /// intent via `on_close`; the caller flips it to `false`.
    open: RwSignal<bool>,
    /// Fired when the dialog requests close (Escape, or backdrop click when
    /// enabled). The caller must set `open` to `false`.
    on_close: impl Fn() + 'static,
    /// Id of the element labelling the dialog → `aria-labelledby`.
    #[prop(into, optional)]
    labelledby: MaybeProp<String>,
    /// Id of the element describing the dialog → `aria-describedby`.
    #[prop(into, optional)]
    describedby: MaybeProp<String>,
    /// Whether clicking the backdrop requests close.
    #[prop(optional, default = true)]
    close_on_backdrop: bool,
    /// Panel placement: centered modal (default) or inline-end slide-over.
    #[prop(optional)]
    side: DialogSide,
    /// Extra classes merged onto the panel.
    #[prop(into, optional, default = String::new())]
    class: String,
    /// Dialog contents: caller supplies the title element (with the id
    /// referenced by `labelledby`), body, and footer/buttons. `ChildrenFn` so
    /// the subtree can be re-rendered across open→close→open cycles.
    children: ChildrenFn,
) -> impl IntoView {
    // The caller's `on_close` is `!Send`. Park it in a thread-local
    // `StoredValue`; the (`!Send`) local effect reads it. The Show children
    // closure must be `Send + Sync` in Leptos 0.8 CSR, so it must NOT capture
    // this handle — all dismissal wiring therefore lives in the effect.
    let on_close = StoredValue::new_local(on_close);

    // Node refs: `panel_ref` is queried for focusables; `backdrop_ref` gets a
    // click listener (backdrop click → dismiss) attached in the effect.
    let panel_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let backdrop_ref: NodeRef<leptos::html::Div> = NodeRef::new();

    // Live teardown for the current open cycle. Held in a `StoredValue` (not
    // the effect's prev-value) so `on_cleanup` can run it if the component is
    // disposed while still open — otherwise the document listener + scroll-lock
    // would leak. `!Send`, so `new_local`.
    let teardown: StoredValue<Option<Teardown>, LocalStorage> = StoredValue::new_local(None);

    // --- side effects (listeners, scroll-lock, focus) tied to `open` --------
    // This effect captures non-`Send` state, which is fine: effects run on the
    // local CSR runtime. The stored teardown is run before the next execution
    // and on dispose, so listeners never duplicate or leak.
    {
        Effect::new(move |_| {
            // Take the previous open-cycle's teardown out of the store.
            let prev = teardown.try_update_value(Option::take).flatten();

            // Tracked reads: `open` plus the two node refs (`.get()` further
            // down). Node refs resolve only after the panel/backdrop render, so
            // the effect re-runs within the *same* open cycle once they mount.
            let is_open = open.get();

            // Resolve the previous teardown against the new state:
            // - close transition (`prev` present, now closed) → FULL teardown
            //   (restore scroll + focus) and stop.
            // - same-cycle re-run (`prev` present, still open) → detach old
            //   listeners only; carry the trigger + prior-overflow forward so
            //   we don't recapture focus from inside the now-open dialog.
            // - genuine open transition (`prev` absent) → capture fresh below.
            let reentry = match prev {
                Some(prev_td) => {
                    if !is_open {
                        prev_td.run();
                        return;
                    }
                    let salvaged = (prev_td.prior_overflow.clone(), prev_td.saved_focus.clone());
                    prev_td.run_listeners_only();
                    Some(salvaged)
                }
                None => None,
            };

            if !is_open {
                return;
            }

            let document = match window().document() {
                Some(d) => d,
                None => return,
            };

            // Trigger + prior overflow: capture fresh on the open transition,
            // reuse on re-entry.
            let (prior_overflow, saved_focus) = match reentry {
                Some((overflow, focus)) => (overflow, focus),
                None => {
                    let saved_focus: Option<HtmlElement> = document
                        .active_element()
                        .and_then(|e| e.dyn_into::<HtmlElement>().ok());
                    // `Some("")` is the normal "no inline overflow set" case;
                    // `set_property("overflow", "")` clears it correctly on
                    // restore, so don't "fix" this to `None`.
                    let prior_overflow = document
                        .body()
                        .map(|b| b.style().get_property_value("overflow").unwrap_or_default());
                    (prior_overflow, saved_focus)
                }
            };
            if let Some(body) = document.body() {
                let _ = body.style().set_property("overflow", "hidden");
            }

            // Document keydown: Escape closes, Tab/Shift+Tab trap focus.
            let keydown =
                Closure::<dyn FnMut(KeyboardEvent)>::new(move |ev: KeyboardEvent| {
                    match ev.key().as_str() {
                        "Escape" => {
                            ev.prevent_default();
                            on_close.with_value(|f| f());
                        }
                        "Tab" => {
                            if let Some(panel) = panel_ref.get_untracked() {
                                trap_tab(&panel, &ev);
                            }
                        }
                        _ => {}
                    }
                });
            let _ = document
                .add_event_listener_with_callback("keydown", keydown.as_ref().unchecked_ref());

            // Backdrop click → dismiss (only when the click lands on the
            // backdrop itself, not a descendant of the panel). Attached to the
            // backdrop element so panel clicks never reach it. Tracking
            // `backdrop_ref.get()` re-runs this effect once the node mounts.
            let backdrop_listener = if close_on_backdrop {
                backdrop_ref.get().map(|backdrop| {
                    let backdrop_el: web_sys::EventTarget = backdrop.clone().into();
                    let backdrop_node: web_sys::Node = backdrop.clone().into();
                    let click =
                        Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
                            if let Some(target) = ev.target() {
                                let target_node: web_sys::Node = target.unchecked_into();
                                if backdrop_node.is_same_node(Some(&target_node)) {
                                    on_close.with_value(|f| f());
                                }
                            }
                        });
                    let _ = backdrop_el
                        .add_event_listener_with_callback("click", click.as_ref().unchecked_ref());
                    (backdrop_el, click)
                })
            } else {
                None
            };

            // Move focus into the dialog: first focusable child, else the panel.
            // Read the panel ref UNTRACKED — the `backdrop_ref.get()` tracked
            // read above already guarantees this effect re-runs once the DOM is
            // ready (panel and backdrop resolve in the same render flush). If we
            // tracked `panel_ref` too, a separate panel-mount tick could trigger
            // a second effect run in the same open cycle, re-registering the
            // keydown closure and calling focus() twice.
            if let Some(panel) = panel_ref.get_untracked() {
                let panel_el: &Element = panel.unchecked_ref();
                match first_focusable(panel_el) {
                    Some(el) => {
                        let _ = el.focus();
                    }
                    None => {
                        if let Some(p) = panel.dyn_ref::<HtmlElement>() {
                            let _ = p.focus();
                        }
                    }
                }
            }

            teardown.set_value(Some(Teardown {
                document,
                keydown,
                backdrop_listener,
                prior_overflow,
                saved_focus,
            }));
        });

        // Disposal safety net: if the component is torn down while still open
        // (e.g. parent route unmounts without flipping `open`), run the live
        // teardown so the document listener and scroll-lock don't leak.
        on_cleanup(move || {
            if let Some(td) = teardown.try_update_value(Option::take).flatten() {
                td.run();
            }
        });
    }

    let panel_base = match side {
        DialogSide::Center => {
            "w-full max-w-lg mx-4 border-2 border-ink rounded-xs p-6 \
             flex flex-col gap-4 max-h-[90vh] overflow-y-auto"
        }
        DialogSide::End => {
            "drawer-panel-end h-full w-full max-w-md border-s-2 border-ink p-6 \
             flex flex-col gap-5 overflow-y-auto"
        }
    };
    let panel_class = tw_merge::tw_merge!(panel_base, class);
    let backdrop_class = match side {
        DialogSide::Center => "fixed inset-0 z-50 flex items-center justify-center",
        DialogSide::End => "fixed inset-0 z-50 flex justify-end",
    };
    let panel_style = match side {
        DialogSide::Center => "background: var(--cream); box-shadow: 6px 6px 0 #1A1A1A;",
        DialogSide::End => "background: var(--cream); box-shadow: -8px 0 24px rgba(26,26,26,0.12);",
    };

    // Conditional render via a reactive closure rather than `<Show>`: the
    // panel/backdrop carry `NodeRef`s (CSR `LocalStorage`, `!Send`), and
    // `<Show>`'s children are `Send + Sync`-bound, so a plain `move ||`
    // returning `Option<_>` is the idiomatic CSR fit here. It tracks `open`
    // and mounts/unmounts the subtree on toggle; `children` is a `ChildrenFn`
    // so it re-renders cleanly on each reopen.
    view! {
        {move || {
            if !open.get() {
                return None;
            }
            // `labelledby` / `describedby` are `MaybeProp` (Copy) — captured by
            // copy into the attr closures below. `panel_class` is a `String`,
            // cloned per render since this closure re-runs on each open.
            let panel_class = panel_class.clone();
            let inner = children();
            Some(view! {
                // Backdrop. Click dismissal is wired in the effect via a
                // listener on this node (target-identity checked), so no
                // `on:click` here and no `!Send` capture leaks out.
                <div
                    node_ref=backdrop_ref
                    class=backdrop_class
                    style="background: rgba(26,26,26,0.5);"
                >
                    <div
                        node_ref=panel_ref
                        role="dialog"
                        aria-modal="true"
                        tabindex="-1"
                        aria-labelledby=move || labelledby.get()
                        aria-describedby=move || describedby.get()
                        class=panel_class
                        style=panel_style
                    >
                        {inner}
                    </div>
                </div>
            })
        }}
    }
}

/// Teardown state captured for one open cycle. Dropping/running it removes the
/// keydown listener, restores body scroll, and restores trigger focus.
struct Teardown {
    document: web_sys::Document,
    keydown: Closure<dyn FnMut(KeyboardEvent)>,
    backdrop_listener: Option<BackdropListener>,
    prior_overflow: Option<String>,
    saved_focus: Option<HtmlElement>,
}

impl Teardown {
    /// Detach the DOM listeners only (used on a same-cycle effect re-run,
    /// where scroll-lock and trigger focus must persist). The `Closure`s drop
    /// when `self` drops, freeing the JS shims.
    fn run_listeners_only(self) {
        let _ = self
            .document
            .remove_event_listener_with_callback("keydown", self.keydown.as_ref().unchecked_ref());
        if let Some((target, click)) = self.backdrop_listener.as_ref() {
            let _ =
                target.remove_event_listener_with_callback("click", click.as_ref().unchecked_ref());
        }
    }

    /// Full teardown on close/dispose: detach listeners, restore body scroll,
    /// and restore focus to the saved trigger element if still connected.
    fn run(self) {
        if let (Some(body), Some(prior)) = (self.document.body(), self.prior_overflow.as_ref()) {
            let _ = body.style().set_property("overflow", prior);
        }
        let saved_focus = self.saved_focus.clone();
        self.run_listeners_only();
        if let Some(el) = saved_focus {
            if el.is_connected() {
                let _ = el.focus();
            }
        }
    }
}

/// Selector matching the focusable descendants used for the focus trap.
const FOCUSABLE: &str = "a[href], button:not([disabled]), input:not([disabled]), \
     select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex=\"-1\"])";

/// First focusable descendant of `panel`, if any.
fn first_focusable(panel: &Element) -> Option<HtmlElement> {
    let list = panel.query_selector_all(FOCUSABLE).ok()?;
    let node = list.item(0)?;
    node.dyn_into::<HtmlElement>().ok()
}

/// Collect all focusable descendants of `panel` as `HtmlElement`s.
fn focusables(panel: &Element) -> Vec<HtmlElement> {
    let mut out = Vec::new();
    if let Ok(list) = panel.query_selector_all(FOCUSABLE) {
        let len = list.length();
        for i in 0..len {
            if let Some(node) = list.item(i) {
                if let Ok(el) = node.dyn_into::<HtmlElement>() {
                    out.push(el);
                }
            }
        }
    }
    out
}

/// Implement the Tab / Shift+Tab focus wrap within `panel`. Called from the
/// document keydown handler for the `"Tab"` key.
fn trap_tab(panel: &HtmlElement, ev: &KeyboardEvent) {
    let panel_el: &Element = panel.unchecked_ref();
    let items = focusables(panel_el);

    // No focusable children: keep focus pinned to the panel itself.
    if items.is_empty() {
        ev.prevent_default();
        let _ = panel.focus();
        return;
    }

    let first = &items[0];
    let last = &items[items.len() - 1];

    let active = window()
        .document()
        .and_then(|d| d.active_element())
        .and_then(|e| e.dyn_into::<HtmlElement>().ok());

    let active_is = |target: &HtmlElement| -> bool {
        active
            .as_ref()
            .map(|a| a.is_same_node(Some(target.unchecked_ref())))
            .unwrap_or(false)
    };

    // If focus is currently outside the panel, pull it to the first item.
    let focus_inside = active
        .as_ref()
        .map(|a| panel_el.contains(Some(a.unchecked_ref())))
        .unwrap_or(false);
    if !focus_inside {
        ev.prevent_default();
        let _ = first.focus();
        return;
    }

    if ev.shift_key() {
        // Shift+Tab on the first element wraps to the last.
        if active_is(first) {
            ev.prevent_default();
            let _ = last.focus();
        }
    } else {
        // Tab on the last element wraps to the first.
        if active_is(last) {
            ev.prevent_default();
            let _ = first.focus();
        }
    }
}
