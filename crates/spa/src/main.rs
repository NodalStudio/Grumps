use leptos::prelude::*;

mod api;
mod app;
mod auth;
mod components;
mod demo;
mod i18n;
mod pages;

fn main() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
    leptos::mount::mount_to_body(app::App);
}
