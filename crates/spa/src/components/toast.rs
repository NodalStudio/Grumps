use leptos::prelude::*;

#[component]
pub fn ToastContainer() -> impl IntoView {
    // TODO: reactive toast queue
    view! { <div class="fixed bottom-6 right-6 z-50 flex flex-col gap-2"></div> }
}
