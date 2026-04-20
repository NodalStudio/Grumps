use leptos::prelude::*;
use web_sys::window;

/// Three-state theme toggle: Light / Dark / Auto.
/// Persists to `localStorage.theme`; "auto" removes the key and follows
/// `prefers-color-scheme`. Writes `data-theme="dark"` on `<html>` when dark.
#[component]
pub fn ThemeToggle() -> impl IntoView {
    let (mode, set_mode) = signal(current_mode());

    Effect::new(move |_| {
        let m = mode.get();
        apply_mode(&m);
    });

    view! {
        <div class="inline-flex border-grumps border-strong rounded-sm overflow-hidden"
             role="group"
             aria-label="Theme">
            <button
                type="button"
                class=move || btn_class(&mode.get(), "light")
                on:click=move |_| set_mode.set("light".to_string())>
                "Light"
            </button>
            <button
                type="button"
                class=move || btn_class(&mode.get(), "auto")
                on:click=move |_| set_mode.set("auto".to_string())>
                "Auto"
            </button>
            <button
                type="button"
                class=move || btn_class(&mode.get(), "dark")
                on:click=move |_| set_mode.set("dark".to_string())>
                "Dark"
            </button>
        </div>
    }
}

fn btn_class(current: &str, mine: &str) -> String {
    let active = current == mine;
    let base = "px-2.5 py-1 text-meta cursor-pointer transition-colors";
    if active {
        format!("{base} bg-primary text-surface")
    } else {
        format!("{base} text-secondary hover:bg-hover-tint")
    }
}

fn current_mode() -> String {
    let Some(win) = window() else { return "auto".into(); };
    let Ok(Some(storage)) = win.local_storage() else { return "auto".into(); };
    match storage.get_item("theme").ok().flatten() {
        Some(v) if v == "light" || v == "dark" || v == "auto" => v,
        _ => "auto".into(),
    }
}

fn apply_mode(mode: &str) {
    let Some(win) = window() else { return; };
    let Some(doc) = win.document() else { return; };
    let Some(root) = doc.document_element() else { return; };
    let dark = match mode {
        "dark" => true,
        "light" => false,
        _ => win
            .match_media("(prefers-color-scheme: dark)")
            .ok()
            .flatten()
            .map(|mql| mql.matches())
            .unwrap_or(false),
    };
    if dark {
        let _ = root.set_attribute("data-theme", "dark");
    } else {
        let _ = root.remove_attribute("data-theme");
    }
    if let Ok(Some(storage)) = win.local_storage() {
        if mode == "auto" {
            let _ = storage.remove_item("theme");
        } else {
            let _ = storage.set_item("theme", mode);
        }
    }
}
