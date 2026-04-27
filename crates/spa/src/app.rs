use leptos::prelude::*;
use leptos_router::components::{Router, Routes, Route, ParentRoute, Redirect};
use leptos_router::path;

use crate::pages;
use crate::auth::gate::AuthGate;
use crate::i18n::provide_locale;

#[component]
pub fn App() -> impl IntoView {
    provide_locale();
    if crate::demo::is_demo() {
        crate::auth::provide_auth();   // legacy AuthState shim for demo pages
        crate::demo::install_postmessage_nav();
    } else {
        // Non-demo pages that still rely on the legacy AuthState shim need
        // a context to be present even before AuthGate runs. Provide an
        // empty shim so use_auth() in those pages doesn't panic; once
        // AuthGate populates SessionContext, new pages should prefer that.
        crate::auth::provide_auth();
    }
    let base: String = crate::demo::router_base();
    view! {
        <Router base=base>
            <Routes fallback=|| view! { <div class="p-8 font-display text-2xl">{move || crate::i18n::tr("common.404")}</div> }>
                <Route path=path!("/login") view=pages::login::LoginPage />
                <Route path=path!("/admin/observability") view=pages::global_observability::GlobalObservabilityPage />

                // Protected routes wrapped in AuthGate.
                <Route path=path!("/") view=move || view! {
                    <AuthGate>
                        <Redirect path="/dashboard".to_string()/>
                    </AuthGate>
                } />
                <Route path=path!("/dashboard") view=move || view! {
                    <AuthGate>
                        <pages::dashboard::DashboardPage/>
                    </AuthGate>
                } />
                <Route path=path!("/settings") view=move || view! {
                    <AuthGate>
                        <pages::global_settings::GlobalSettingsPage/>
                    </AuthGate>
                } />
                <ParentRoute path=path!("/w/:slug") view=move || view! {
                    <AuthGate>
                        <pages::workspace::WorkspaceLayout/>
                    </AuthGate>
                }>
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
            </Routes>
        </Router>
    }
}
