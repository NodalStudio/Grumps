use leptos::prelude::*;
use leptos_router::components::{Router, Routes, Route, ParentRoute, Redirect};
use leptos_router::path;

use crate::pages;
use crate::auth::provide_auth;
use crate::i18n::provide_locale;

#[component]
pub fn App() -> impl IntoView {
    provide_locale();
    provide_auth();
    if crate::demo::is_demo() {
        crate::demo::install_postmessage_nav();
    }
    // When the SPA runs under /demo/ (landing iframe), tell the router
    // to strip that prefix before matching routes — otherwise every URL
    // falls through to the 404 catch-all.
    let base: &'static str = if crate::demo::is_demo() { "/demo" } else { "" };
    view! {
        <Router base=base>
            <Routes fallback=|| view! { <div class="p-8 font-display text-2xl">"404 — Not found."</div> }>
                <Route path=path!("/login") view=pages::login::LoginPage />
                <Route path=path!("/dashboard") view=pages::dashboard::DashboardPage />
                <Route path=path!("/admin/observability") view=pages::global_observability::GlobalObservabilityPage />
                <ParentRoute path=path!("/w/:slug") view=pages::workspace::WorkspaceLayout>
                    <Route path=path!("/") view=pages::overview::OverviewPage />
                    <Route path=path!("/todos") view=pages::todos::TodosPage />
                    <Route path=path!("/notes") view=pages::notes::NotesPage />
                    <Route path=path!("/notes/:id") view=pages::note_editor::NoteEditorPage />
                    <Route path=path!("/files") view=pages::files::FilesPage />
                    <Route path=path!("/history") view=pages::history::HistoryPage />
                    <Route path=path!("/settings") view=pages::settings::SettingsPage />
                    <Route path=path!("/memory") view=pages::memory::MemoryPage />
                    <Route path=path!("/scheduled") view=pages::scheduled::ScheduledActionsPage />
                    <Route path=path!("/calendar") view=pages::calendar::CalendarPage />
                    <Route path=path!("/admin/observability") view=pages::observability::ObservabilityPage />
                </ParentRoute>
                <Route path=path!("/") view=|| {
                    if crate::demo::is_demo() {
                        view! { <Redirect path=format!("/w/{}", crate::demo::DEMO_SLUG) /> }.into_any()
                    } else {
                        view! { <Redirect path="/login".to_string() /> }.into_any()
                    }
                } />
            </Routes>
        </Router>
    }
}
