use leptos::prelude::*;
use leptos_router::components::{Router, Routes, Route, ParentRoute, Redirect};
use leptos_router::path;

use crate::pages;

#[component]
pub fn App() -> impl IntoView {
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
                </ParentRoute>
                <Route path=path!("/") view=|| view! { <Redirect path="/login" /> } />
            </Routes>
        </Router>
    }
}
