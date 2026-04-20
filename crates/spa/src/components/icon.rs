use leptos::prelude::*;

/// Monochrome 2px-stroke SVG icon. Inherits colour via `currentColor`.
/// Sizing is done by the parent (Tailwind `size-4` / `size-5` / `size-6`).
///
/// Source drawings live in `crates/spa/assets/icons/<name>.svg`.
/// When adding an icon, paste its `<path>` content into the match below
/// and keep the viewBox at `0 0 24 24`, stroke-width `2`, square caps.
#[component]
pub fn Icon(
    name: &'static str,
    #[prop(optional, default = "")] class: &'static str,
) -> impl IntoView {
    let svg = match name {
        "overview" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <rect x="4" y="4" width="7" height="7"/>
                <rect x="13" y="4" width="7" height="7"/>
                <rect x="4" y="13" width="7" height="7"/>
                <rect x="13" y="13" width="7" height="7"/>
            </svg>
        }.into_any(),
        "todos" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <rect x="4" y="4" width="16" height="16"/>
                <path d="M8 12 L11 15 L16 9"/>
            </svg>
        }.into_any(),
        "notes" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M5 3 L15 3 L19 7 L19 21 L5 21 Z"/>
                <path d="M15 3 L15 7 L19 7"/>
                <path d="M8 11 H16 M8 15 H16 M8 19 H13"/>
            </svg>
        }.into_any(),
        "files" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M3 7 L3 19 L21 19 L21 9 L11 9 L9 7 Z"/>
            </svg>
        }.into_any(),
        "history" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M4 12 A8 8 0 1 1 8 18"/>
                <path d="M4 8 L4 14 L10 14"/>
                <path d="M12 8 L12 12 L15 14"/>
            </svg>
        }.into_any(),
        "calendar" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <rect x="3" y="5" width="18" height="16"/>
                <path d="M3 9 L21 9 M8 3 L8 7 M16 3 L16 7"/>
            </svg>
        }.into_any(),
        "memory" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M6 3 L18 3 L18 21 L12 16 L6 21 Z"/>
            </svg>
        }.into_any(),
        "scheduled" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <circle cx="12" cy="12" r="8"/>
                <path d="M12 7 L12 12 L16 14"/>
            </svg>
        }.into_any(),
        "settings" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <circle cx="12" cy="12" r="3"/>
                <path d="M12 2 L12 5 M12 19 L12 22 M22 12 L19 12 M5 12 L2 12 M19 5 L17 7 M7 17 L5 19 M19 19 L17 17 M7 7 L5 5"/>
            </svg>
        }.into_any(),
        "workspaces" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <rect x="3" y="3" width="18" height="18"/>
                <path d="M12 3 L12 21 M3 12 L21 12"/>
            </svg>
        }.into_any(),
        "globe" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <circle cx="12" cy="12" r="9"/>
                <path d="M3 12 L21 12"/>
                <ellipse cx="12" cy="12" rx="4" ry="9"/>
            </svg>
        }.into_any(),
        "message" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M3 5 L21 5 L21 17 L11 17 L7 21 L7 17 L3 17 Z"/>
            </svg>
        }.into_any(),
        "recap" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <rect x="5" y="5" width="14" height="16"/>
                <rect x="8" y="3" width="8" height="4"/>
                <path d="M9 11 H15 M9 15 H15 M9 19 H13"/>
            </svg>
        }.into_any(),
        "task-check" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <circle cx="12" cy="12" r="9"/>
                <path d="M7 12 L11 16 L17 9"/>
            </svg>
        }.into_any(),
        "webhook" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M4 13 L4 15 A2 2 0 0 0 6 17 L10 17 A2 2 0 0 0 12 15 L12 9 A2 2 0 0 1 14 7 L18 7 A2 2 0 0 1 20 9 L20 11"/>
            </svg>
        }.into_any(),
        "bolt" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M13 3 L5 14 L11 14 L9 21 L19 10 L13 10 Z"/>
            </svg>
        }.into_any(),
        "pin" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M8 4 L16 4 L14 10 L18 14 L6 14 L10 10 Z"/>
                <path d="M12 14 L12 21"/>
            </svg>
        }.into_any(),
        "chevron-down" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M5 9 L12 16 L19 9"/>
            </svg>
        }.into_any(),
        _ => view! {
            <svg viewBox="0 0 24 24"></svg>
        }.into_any(),
    };
    view! { <span class=class>{svg}</span> }
}
