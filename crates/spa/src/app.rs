use leptos::prelude::*;
use leptos_router::components::{Router, Routes, Route, ParentRoute, Redirect};
use leptos_router::path;

use crate::pages;
use crate::auth::provide_auth;

#[component]
pub fn App() -> impl IntoView {
    provide_auth();
    view! {
        <Router>
            <Routes fallback=|| view! { <div class="p-8 font-display text-2xl">"404 — Not found."</div> }>
                <Route path=path!("/login") view=pages::login::LoginPage />
                <Route path=path!("/dashboard") view=pages::dashboard::DashboardPage />
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
                </ParentRoute>
                <Route path=path!("/") view=|| view! { <Redirect path="/login" /> } />
            </Routes>
        </Router>
    }
}
