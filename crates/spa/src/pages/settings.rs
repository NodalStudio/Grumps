use crate::api::use_api;
use crate::components::header::PageHeader;
use crate::components::switch::Switch;
use crate::i18n::tr;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[component]
pub fn SettingsPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let (quiet_mode, set_quiet) = signal(false);
    let (recap, set_recap) = signal(true);
    let (auto_memory, set_auto_memory) = signal(true);
    let (proactive, set_proactive) = signal(false);
    let (persona, set_persona) = signal("grumps".to_string());
    let (ical_url, set_ical_url) = signal(String::new());
    let (workspace_locale, set_workspace_locale) = signal("en".to_string());
    let (refresh, set_refresh) = signal(0u32);

    let api = use_api();
    let _settings = LocalResource::new(move || {
        let api = api.clone();
        let s = slug();
        let _ = refresh.get();
        async move {
            if let Ok(settings) = api.get_settings(&s).await {
                set_quiet.set(settings.quiet_mode.unwrap_or(false));
                set_recap.set(settings.auto_recap.unwrap_or(true));
                set_auto_memory.set(settings.auto_memory.unwrap_or(true));
                set_proactive.set(settings.proactive_mode.unwrap_or(false));
                set_persona.set(settings.persona.unwrap_or("grumps".to_string()));
                set_workspace_locale.set(settings.language.unwrap_or("en".to_string()));
                if let Some(token) = settings.ical_token {
                    set_ical_url.set(format!(
                        "https://grumps.app/api/w/{}/calendar/ical/{}",
                        s, token
                    ));
                }
            }
        }
    });

    let api_save = use_api();
    let save_agent = move |_| {
        let api = api_save.clone();
        let s = slug();
        let persona_val = persona.get();
        let proactive_val = proactive.get();
        let auto_mem = auto_memory.get();
        leptos::task::spawn_local(async move {
            let _ = api
                .update_settings(
                    &s,
                    &serde_json::json!({
                        "persona": persona_val,
                        "proactive_mode": proactive_val,
                        "auto_memory": auto_mem,
                    }),
                )
                .await;
        });
    };

    let api_locale = use_api();
    let api_regen = use_api();
    let regen_ical = move |_| {
        let api = api_regen.clone();
        let s = slug();
        leptos::task::spawn_local(async move {
            if let Ok(resp) = api.regenerate_ical_token(&s).await {
                set_ical_url.set(resp.url);
            }
        });
    };

    let copy_ical = move |_| {
        let url = ical_url.get();
        leptos::task::spawn_local(async move {
            if let Some(window) = web_sys::window() {
                let clipboard = window.navigator().clipboard();
                let _ = wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&url)).await;
            }
        });
    };

    view! {
        <PageHeader title=tr("page.settings.title") subtitle=tr("page.settings.subtitle") />
        <div class="flex-1 overflow-y-auto p-8">
            <div class="max-w-xl">
                // General
                <SettingsSection title_key="settings.section.general">
                    <div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
                        <div class="font-medium text-sm">{move || tr("settings.row.language")}</div>
                        <select
                            class="border-2 border-ink rounded-xs px-3 py-1.5 text-sm bg-transparent outline-hidden"
                            on:change=move |ev| {
                                let new_locale = event_target_value(&ev);
                                let slug_str = slug();
                                let api = api_locale.clone();
                                leptos::task::spawn_local(async move {
                                    if api.update_workspace_locale(&slug_str, &new_locale).await.is_ok() {
                                        set_workspace_locale.set(new_locale);
                                    }
                                });
                            }
                            prop:value=workspace_locale
                        >
                            {grumps_i18n::Locale::ALL.iter().map(|loc| {
                                let code = loc.code();
                                let native = loc.native_name();
                                view! {
                                    <option value=code>{native}</option>
                                }
                            }).collect_view()}
                        </select>
                    </div>
                    <SettingRow label_key="settings.row.timezone" value=crate::datetime::use_timezone() />
                </SettingsSection>

                // Bot Behavior
                <SettingsSection title_key="settings.section.bot_behavior">
                    <Toggle label_key="settings.toggle.quiet_mode" desc_key="settings.toggle.quiet_mode.desc" value=quiet_mode on_toggle=move || set_quiet.update(|v| *v = !*v) />
                    <Toggle label_key="settings.toggle.auto_recap" desc_key="settings.toggle.auto_recap.desc" value=recap on_toggle=move || set_recap.update(|v| *v = !*v) />
                </SettingsSection>

                // AI agent
                <SettingsSection title_key="settings.section.agent">
                    <div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
                        <div>
                            <div class="font-medium text-sm">{move || tr("settings.row.persona")}</div>
                            <div class="text-xs" style="color: var(--ink-40);">{move || tr("settings.row.persona.desc")}</div>
                        </div>
                        <select
                            class="border-2 border-ink rounded-xs px-3 py-1.5 text-sm bg-transparent outline-hidden"
                            on:change=move |ev| set_persona.set(event_target_value(&ev))
                            prop:value=persona
                        >
                            <option value="grumps">{move || tr("settings.persona.grumps")}</option>
                            <option value="assistant">{move || tr("settings.persona.assistant")}</option>
                            <option value="coach">{move || tr("settings.persona.coach")}</option>
                        </select>
                    </div>
                    <Toggle
                        label_key="settings.toggle.proactive"
                        desc_key="settings.toggle.proactive.desc"
                        value=proactive
                        on_toggle=move || set_proactive.update(|v| *v = !*v)
                    />
                    <Toggle
                        label_key="settings.toggle.auto_memory"
                        desc_key="settings.toggle.auto_memory.desc"
                        value=auto_memory
                        on_toggle=move || set_auto_memory.update(|v| *v = !*v)
                    />
                    <div class="py-3" style="border-bottom: 1px solid var(--ink-08);">
                        <a href="#" class="text-sm font-semibold" style="color: var(--teal);">{move || tr("settings.consent_link")}</a>
                    </div>
                    <div class="pt-3 pb-1">
                        <button
                            class="px-4 py-2 text-sm font-bold border-2 border-ink rounded-xs cursor-pointer"
                            style="background: var(--ink); color: var(--cream);"
                            on:click=save_agent
                        >{move || tr("settings.save_agent")}</button>
                    </div>
                </SettingsSection>

                // Calendar
                <SettingsSection title_key="settings.section.calendar">
                    <div class="py-3 flex flex-col gap-2" style="border-bottom: 1px solid var(--ink-08);">
                        <div class="font-medium text-sm">{move || tr("settings.ical.label")}</div>
                        <div class="text-xs mb-2" style="color: var(--ink-40);">{move || tr("settings.ical.desc")}</div>
                        <div class="flex items-center gap-2">
                            <input
                                type="text" readonly
                                class="flex-1 border-2 border-ink rounded-xs px-3 py-1.5 text-xs bg-transparent outline-hidden font-mono"
                                placeholder=tr("settings.ical.placeholder")
                                prop:value=ical_url
                            />
                            <button
                                class="px-3 py-1.5 text-xs font-bold border-2 border-ink rounded-xs cursor-pointer shrink-0"
                                on:click=copy_ical
                            >{move || tr("settings.ical.copy")}</button>
                        </div>
                        <div class="flex gap-2 mt-1">
                            <button
                                class="px-3 py-1.5 text-xs font-bold border-2 border-ink rounded-xs cursor-pointer"
                                on:click=regen_ical
                            >{move || tr("settings.ical.regenerate")}</button>
                            <button
                                class="px-3 py-1.5 text-xs font-semibold border rounded-xs cursor-pointer"
                                style="border-color: var(--brick); color: var(--brick);"
                                on:click=move |_| set_ical_url.set(String::new())
                            >{move || tr("settings.ical.revoke")}</button>
                        </div>
                    </div>
                </SettingsSection>

                // Quotas
                <SettingsSection title_key="settings.section.quotas">
                    <QuotaRow label_key="settings.quota.agent_calls" used=42 limit=200 />
                    <QuotaRow label_key="settings.quota.web_searches" used=8 limit=50 />
                    <div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
                        <div class="font-medium text-sm">{move || tr("settings.quota.storage")}</div>
                        <div class="font-semibold text-[13px]" style="color: var(--ink-70);">"12 MB / 100 MB"</div>
                    </div>
                </SettingsSection>

                // Plan
                <SettingsSection title_key="settings.section.plan">
                    <SettingRow label_key="settings.row.current_plan" value=tr("settings.plan.free") />
                </SettingsSection>
            </div>
        </div>
    }
}

#[component]
fn SettingsSection(title_key: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="mb-8">
            <h3 class="font-display text-base font-bold mb-4 pb-2 border-b-2 border-ink">{move || tr(title_key)}</h3>
            {children()}
        </div>
    }
}

#[component]
fn SettingRow(label_key: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
            <div class="font-medium text-sm">{move || tr(label_key)}</div>
            <div class="font-semibold text-[13px]" style="color: var(--ink-70);">{value}</div>
        </div>
    }
}

#[component]
fn Toggle(
    label_key: &'static str,
    desc_key: &'static str,
    value: ReadSignal<bool>,
    on_toggle: impl Fn() + 'static,
) -> impl IntoView {
    view! {
        <div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
            <div>
                <div class="font-medium text-sm">{move || tr(label_key)}</div>
                <div class="text-xs" style="color: var(--ink-40);">{move || tr(desc_key)}</div>
            </div>
            <Switch checked=value on_change=on_toggle aria_label=Signal::derive(move || tr(label_key)) />
        </div>
    }
}

#[component]
fn QuotaRow(label_key: &'static str, used: i64, limit: i64) -> impl IntoView {
    let pct = ((used as f64 / limit as f64) * 100.0) as u32;
    let bar_color = if pct > 80 {
        "var(--brick)"
    } else if pct > 60 {
        "var(--teal)"
    } else {
        "var(--ink-40)"
    };

    view! {
        <div class="py-3 flex flex-col gap-1" style="border-bottom: 1px solid var(--ink-08);">
            <div class="flex items-center justify-between">
                <div class="font-medium text-sm">{move || tr(label_key)}</div>
                <div class="font-semibold text-[13px]" style="color: var(--ink-70);">{format!("{} / {}", used, limit)}</div>
            </div>
            <div class="w-full h-2 rounded-full" style="background: var(--ink-08);">
                <div
                    class="h-full rounded-full transition-all"
                    style:width=format!("{}%", pct)
                    style:background=bar_color
                ></div>
            </div>
        </div>
    }
}
