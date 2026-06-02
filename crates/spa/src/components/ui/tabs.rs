//! Accessible **controlled** tabs / segmented-control primitive.
//!
//! The caller owns the selected value via an `RwSignal<String>`; this component
//! reads it, renders one `<button role="tab">` per [`TabItem`], and writes the
//! value on click or keyboard navigation. It implements the WAI-ARIA tabs
//! pattern with **automatic activation**: arrow keys move selection (not just
//! focus) and Home/End jump to the ends, all wrapping.
//!
//! Unlike [`super::dropdown_menu`] / [`super::dialog`], the tab row is always
//! mounted (it is not a popup), so the `keydown` handler is attached directly
//! with Leptos `on:keydown` on the tablist — Leptos removes it on unmount, so
//! there are no document listeners, `Closure` teardowns, or `on_cleanup` here.
//! The only `web_sys` use is moving DOM focus to the target tab after arrow
//! navigation; every step is guarded so a missing node degrades gracefully
//! rather than panicking.

use crate::i18n::tr;
use leptos::prelude::*;
use tw_merge::tw_merge;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, KeyboardEvent};

/// One tab: its selection `value` (stored in the caller's signal) and the i18n
/// `label_key` resolved reactively so the label tracks mid-session locale
/// switches.
#[derive(Clone)]
pub struct TabItem {
    pub value: String,
    pub label_key: String,
}

#[component]
pub fn Tabs(
    /// Selected value, owned by the caller. Read reactively; written on
    /// click / arrow / Home / End.
    value: RwSignal<String>,
    /// Tabs in DOM order.
    tabs: Vec<TabItem>,
    /// Accessible name for the tablist (localized by the caller). Omitted when
    /// empty.
    #[prop(into, optional)]
    aria_label: MaybeProp<String>,
    /// Extra classes merged onto the tablist container.
    #[prop(into, optional, default = String::new())]
    class: String,
    /// Extra classes merged onto each tab button (sizing / border tweaks).
    #[prop(into, optional, default = String::new())]
    tab_class: String,
) -> impl IntoView {
    let list_ref: NodeRef<leptos::html::Div> = NodeRef::new();

    // Ordered values, used to map a keyboard step to the value to select.
    let values: Vec<String> = tabs.iter().map(|t| t.value.clone()).collect();

    let container_class = tw_merge!("flex gap-2 items-center flex-wrap", class);

    // Focus the tab button at `idx` within the tablist (guarded; no-op if the
    // node tree isn't available yet).
    let focus_idx = move |idx: usize| {
        let Some(list) = list_ref.get_untracked() else {
            return;
        };
        if let Ok(nodes) = list.query_selector_all("[role=\"tab\"]") {
            if let Some(node) = nodes.get(idx as u32) {
                if let Ok(btn) = node.dyn_into::<HtmlElement>() {
                    let _ = btn.focus();
                }
            }
        }
    };

    let on_keydown = {
        let values = values.clone();
        move |ev: KeyboardEvent| {
            if values.is_empty() {
                return;
            }
            let len = values.len();
            let cur = values
                .iter()
                .position(|v| v == &value.get_untracked())
                .unwrap_or(0);
            let next = match ev.key().as_str() {
                "ArrowRight" | "ArrowDown" => (cur + 1) % len,
                "ArrowLeft" | "ArrowUp" => (cur + len - 1) % len,
                "Home" => 0,
                "End" => len - 1,
                _ => return,
            };
            ev.prevent_default();
            value.set(values[next].clone());
            focus_idx(next);
        }
    };

    view! {
        <div
            node_ref=list_ref
            role="tablist"
            aria-label=move || aria_label.get()
            class=container_class
            on:keydown=on_keydown
        >
            {tabs
                .into_iter()
                .map(|item| {
                    let val = item.value.clone();
                    let label_key = item.label_key.clone();
                    let v_sel = val.clone();
                    let v_bg = val.clone();
                    let v_text = val.clone();
                    let v_border = val.clone();
                    let v_tab = val.clone();
                    let v_click = val.clone();
                    let btn_class = tw_merge!(
                        "px-3 py-1 text-xs font-medium border rounded-xs cursor-pointer transition-colors \
                         focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-teal",
                        tab_class.clone()
                    );
                    view! {
                        <button
                            type="button"
                            role="tab"
                            aria-selected=move || (value.get() == v_sel).to_string()
                            tabindex=move || if value.get() == v_tab { "0" } else { "-1" }
                            class=btn_class
                            class:bg-ink=move || value.get() == v_bg
                            class:text-cream=move || value.get() == v_text
                            style:border-color=move || {
                                if value.get() == v_border { "var(--ink)" } else { "var(--ink-15)" }
                            }
                            on:click=move |_| value.set(v_click.clone())
                        >
                            {move || tr(&label_key)}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}
