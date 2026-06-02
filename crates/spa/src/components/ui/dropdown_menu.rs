//! Accessible **controlled** dropdown menu primitive.
//!
//! The caller owns visibility via the `open` signal; this component only reads
//! it (toggling it from the trigger click and clearing it on Escape / activate /
//! click-outside). The caller authors each menu item inside `children` with
//! `role="menuitem"`; this component manages roving `tabindex`, arrow-key
//! navigation, type-agnostic activation (it dispatches a synthetic `click` so
//! the caller's `on:click` / anchor navigation fires), Escape-to-close with
//! focus restoration, and an outside-pointer dismissal.
//!
//! ## web-sys discipline (mirrors [`super::dialog`])
//! The reactive view mounts the panel via a `move || open.get().then(...)`
//! closure rather than `<Show>` (whose children are `Send + Sync`-bound and so
//! reject the CSR `!Send` `NodeRef`s). A single local `Effect` tracks `open`,
//! attaches the document `keydown` + `pointerdown` listeners and seeds focus on
//! the open transition, and parks its teardown in a `StoredValue`. The teardown
//! runs before the next effect execution and on `on_cleanup`, so listeners never
//! duplicate or leak across open→close→open cycles. No `.forget()`.

use leptos::prelude::*;
use tw_merge::tw_merge;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, KeyboardEvent};

/// Inline-axis alignment of the panel relative to the trigger. Logical, so it
/// flips correctly under RTL — never emit physical `left-`/`right-`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MenuAlign {
    /// Panel's leading edge aligns with the trigger's leading edge (`start-0`).
    Start,
    /// Panel's trailing edge aligns with the trigger's trailing edge (`end-0`).
    End,
}

/// Listener parked for one open cycle: the document keydown (Escape + arrows +
/// activation) and the document pointerdown (click-outside dismissal), each with
/// the `Closure` keeping its JS shim alive. Dropping the closures frees them.
struct Teardown {
    document: web_sys::Document,
    keydown: Closure<dyn FnMut(KeyboardEvent)>,
    pointerdown: Closure<dyn FnMut(web_sys::Event)>,
    /// Element focused when the menu opened, restored on Escape/close.
    saved_focus: Option<HtmlElement>,
}

impl Teardown {
    fn run_listeners_only(&self) {
        let _ = self
            .document
            .remove_event_listener_with_callback("keydown", self.keydown.as_ref().unchecked_ref());
        let _ = self.document.remove_event_listener_with_callback(
            "pointerdown",
            self.pointerdown.as_ref().unchecked_ref(),
        );
    }

    /// Full teardown on close/dispose: detach listeners and restore focus to the
    /// saved trigger element if it is still connected.
    fn run(self) {
        let saved = self.saved_focus.clone();
        self.run_listeners_only();
        if let Some(el) = saved {
            if el.is_connected() {
                let _ = el.focus();
            }
        }
    }
}

#[component]
pub fn DropdownMenu(
    /// Caller-owned visibility. The menu reads this; it toggles on trigger
    /// click and clears on dismissal/activation.
    open: RwSignal<bool>,
    /// Inner content of the trigger `<button>` (this component wraps it).
    #[prop(into)]
    trigger: ViewFn,
    /// Classes for the trigger `<button>`.
    #[prop(into, optional)]
    trigger_class: String,
    /// Inline style passthrough for the trigger (switchers use CSS-var bg).
    #[prop(into, optional)]
    trigger_style: String,
    /// Classes for the panel.
    #[prop(into, optional)]
    menu_class: String,
    /// Inline style passthrough for the panel.
    #[prop(into, optional)]
    menu_style: String,
    /// Accessible name for the trigger button (localized by the caller).
    #[prop(into, optional)]
    aria_label: MaybeProp<String>,
    /// Inline-axis alignment of the panel. Logical (RTL-safe).
    #[prop(optional, default = MenuAlign::End)]
    align: MenuAlign,
    /// Menu items, authored by the caller — each with `role="menuitem"`.
    children: ChildrenFn,
) -> impl IntoView {
    let panel_ref: NodeRef<leptos::html::Div> = NodeRef::new();

    // Live teardown for the current open cycle. Held in a `StoredValue` (not the
    // effect's prev-value) so `on_cleanup` can run it if the component is
    // disposed while still open, otherwise the document listeners would leak.
    let teardown: StoredValue<Option<Teardown>, LocalStorage> = StoredValue::new_local(None);

    {
        Effect::new(move |_| {
            // Take the previous open-cycle's teardown out of the store.
            let prev = teardown.try_update_value(Option::take).flatten();

            // Tracked: `open`, plus `panel_ref.get()` below (so the effect
            // re-runs once the panel mounts in the same open cycle).
            let is_open = open.get();

            // Resolve the previous teardown against the new state:
            // - close transition (prev present, now closed) → FULL teardown
            //   (restore focus) and stop.
            // - same-cycle re-run (prev present, still open) → detach old
            //   listeners only; carry the saved focus forward so we don't
            //   recapture it from inside the now-open menu.
            // - genuine open transition (prev absent) → capture fresh below.
            let salvaged_focus = match prev {
                Some(prev_td) => {
                    if !is_open {
                        prev_td.run();
                        return;
                    }
                    let saved = prev_td.saved_focus.clone();
                    prev_td.run_listeners_only();
                    Some(saved)
                }
                None => None,
            };

            // The panel only mounts when open; track its ref so this effect
            // re-runs once it exists.
            let panel = panel_ref.get();
            if !is_open {
                return;
            }
            let panel = match panel {
                Some(p) => p,
                None => return,
            };

            let document = match window().document() {
                Some(d) => d,
                None => return,
            };

            // Trigger to restore focus to: capture fresh on the open transition,
            // reuse on a same-cycle re-run.
            let saved_focus = match salvaged_focus {
                Some(f) => f,
                None => document
                    .active_element()
                    .and_then(|e| e.dyn_into::<HtmlElement>().ok()),
            };

            // Seed focus: the item carrying `data-current="true"`, else the
            // first item. Set roving tabindex accordingly.
            let panel_el: Element = panel.clone().unchecked_into();
            let items = menu_items(&panel_el);
            if !items.is_empty() {
                let start = items
                    .iter()
                    .position(|el| el.get_attribute("data-current").as_deref() == Some("true"))
                    .unwrap_or(0);
                set_roving(&items, start);
                let _ = items[start].focus();
            }

            // Document keydown: arrow nav, Home/End, Escape, activation.
            let keydown = Closure::<dyn FnMut(KeyboardEvent)>::new(move |ev: KeyboardEvent| {
                let panel = match panel_ref.get_untracked() {
                    Some(p) => p,
                    None => return,
                };
                let panel_el: Element = panel.unchecked_into();
                let items = menu_items(&panel_el);
                if items.is_empty() {
                    if ev.key() == "Escape" {
                        ev.prevent_default();
                        open.set(false);
                    }
                    return;
                }
                let cur = active_index(&items);
                match ev.key().as_str() {
                    "ArrowDown" => {
                        ev.prevent_default();
                        let next = cur.map(|i| (i + 1) % items.len()).unwrap_or(0);
                        set_roving(&items, next);
                        let _ = items[next].focus();
                    }
                    "ArrowUp" => {
                        ev.prevent_default();
                        let next = cur
                            .map(|i| (i + items.len() - 1) % items.len())
                            .unwrap_or(items.len() - 1);
                        set_roving(&items, next);
                        let _ = items[next].focus();
                    }
                    "Home" => {
                        ev.prevent_default();
                        set_roving(&items, 0);
                        let _ = items[0].focus();
                    }
                    "End" => {
                        ev.prevent_default();
                        let last = items.len() - 1;
                        set_roving(&items, last);
                        let _ = items[last].focus();
                    }
                    "Escape" => {
                        ev.prevent_default();
                        open.set(false);
                    }
                    "Enter" | " " | "Spacebar" => {
                        ev.prevent_default();
                        if let Some(i) = cur {
                            items[i].click();
                        }
                        open.set(false);
                    }
                    "Tab" => {
                        // Standard menu behavior: Tab closes and lets focus move
                        // on naturally (no preventDefault).
                        open.set(false);
                    }
                    _ => {}
                }
            });
            let _ = document
                .add_event_listener_with_callback("keydown", keydown.as_ref().unchecked_ref());

            // Document pointerdown: close when the press lands outside the panel.
            // (The trigger's own click toggles via `stop_propagation`, so a
            // press on the trigger does not reach this and re-toggle.)
            let panel_node: web_sys::Node = panel.clone().unchecked_into();
            let pointerdown =
                Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
                    if let Some(target) = ev.target() {
                        let target_node: web_sys::Node = target.unchecked_into();
                        if !panel_node.contains(Some(&target_node)) {
                            open.set(false);
                        }
                    }
                });
            let _ = document.add_event_listener_with_callback(
                "pointerdown",
                pointerdown.as_ref().unchecked_ref(),
            );

            teardown.set_value(Some(Teardown {
                document,
                keydown,
                pointerdown,
                saved_focus,
            }));
        });

        // Disposal safety net: run the live teardown if the component is torn
        // down while still open, so the document listeners don't leak.
        on_cleanup(move || {
            if let Some(td) = teardown.try_update_value(Option::take).flatten() {
                td.run();
            }
        });
    }

    let align_cls = match align {
        MenuAlign::Start => "start-0",
        MenuAlign::End => "end-0",
    };
    let panel_class = tw_merge!("absolute z-50", align_cls, menu_class);

    view! {
        <div class="relative inline-flex">
            <button
                type="button"
                aria-haspopup="menu"
                aria-expanded=move || open.get().to_string()
                aria-label=move || aria_label.get()
                class=trigger_class
                style=trigger_style
                on:click=move |ev| {
                    ev.stop_propagation();
                    open.update(|v| *v = !*v);
                }
            >
                {trigger.run()}
            </button>
            {move || {
                if !open.get() {
                    return None;
                }
                let panel_class = panel_class.clone();
                let menu_style = menu_style.clone();
                let inner = children();
                Some(
                    view! {
                        <div
                            node_ref=panel_ref
                            role="menu"
                            aria-label=move || aria_label.get()
                            class=panel_class
                            style=menu_style
                        >
                            {inner}
                        </div>
                    },
                )
            }}
        </div>
    }
}

/// All `[role="menuitem"]` descendants of `panel`, in document order.
fn menu_items(panel: &Element) -> Vec<HtmlElement> {
    let mut out = Vec::new();
    if let Ok(list) = panel.query_selector_all("[role=\"menuitem\"]") {
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

/// Index of the currently focused item, if focus is on one of `items`.
fn active_index(items: &[HtmlElement]) -> Option<usize> {
    let active = window()
        .document()
        .and_then(|d| d.active_element())
        .and_then(|e| e.dyn_into::<HtmlElement>().ok())?;
    items
        .iter()
        .position(|el| el.is_same_node(Some(active.unchecked_ref())))
}

/// Roving tabindex: `0` on the focused item, `-1` on all others.
fn set_roving(items: &[HtmlElement], focused: usize) {
    for (i, el) in items.iter().enumerate() {
        let _ = el.set_attribute("tabindex", if i == focused { "0" } else { "-1" });
    }
}
