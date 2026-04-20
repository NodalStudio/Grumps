//! Lang switcher dropdown. Renders the current locale's native name and,
//! on click, a list of all 14 supported locales. Persists the choice via
//! `set_locale` (which writes to localStorage and updates `<html lang>`).

use grumps_i18n::Locale;
use leptos::prelude::*;

use crate::i18n::{set_locale, use_locale};

#[component]
pub fn LangSwitcher() -> impl IntoView {
    let open = RwSignal::new(false);
    let current = move || use_locale();

    view! {
        <div class="relative inline-flex">
            <button
                class="flex items-center gap-1.5 text-[12px] font-semibold uppercase tracking-wider px-2.5 py-1.5 border border-ink/15 hover:border-ink rounded-[3px] cursor-pointer"
                style="background: var(--cream-light); color: var(--ink-70);"
                on:click=move |ev| { ev.stop_propagation(); open.update(|v| *v = !*v); }
            >
                <span>{move || current().native_name()}</span>
                <span class="text-[9px]">"\u{25BE}"</span>
            </button>
            <Show when=move || open.get() fallback=|| ()>
                <div
                    class="absolute right-0 bottom-full mb-1.5 z-50 min-w-[200px] max-h-[360px] overflow-y-auto p-1.5 border-2 border-ink rounded-[3px]"
                    style="background: var(--cream); box-shadow: 4px 4px 0 var(--ink-15);"
                >
                    {Locale::ALL.iter().map(|loc| {
                        let l = *loc;
                        let is_current = move || current() == l;
                        view! {
                            <button
                                class="w-full flex items-center justify-between gap-3 px-2.5 py-1.5 text-[13px] rounded-[2px] hover:bg-black/[0.04]"
                                style:font-weight=move || if is_current() { "700" } else { "500" }
                                style:color=move || if is_current() { "var(--ink)" } else { "var(--ink-70)" }
                                on:click=move |ev| { ev.stop_propagation(); set_locale(l); open.set(false); }
                            >
                                <span>{l.native_name()}</span>
                                <span class="font-display text-[10px] font-bold tracking-wider uppercase" style="color: var(--ink-40);">{l.code()}</span>
                            </button>
                        }
                    }).collect_view()}
                </div>
            </Show>
        </div>
    }
}
