use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use crate::auth::use_auth;
use crate::components::header::PageHeader;

#[component]
pub fn SettingsPage() -> impl IntoView {
    let auth = use_auth();
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    let (quiet_mode, set_quiet) = signal(false);
    let (recap, set_recap) = signal(true);
    let (auto_memory, set_auto_memory) = signal(true);
    let (proactive, set_proactive) = signal(false);
    let (persona, set_persona) = signal("grumps".to_string());
    let (ical_url, set_ical_url) = signal(String::new());
    let (refresh, set_refresh) = signal(0u32);

    let api = auth.api.clone();
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
                if let Some(token) = settings.ical_token {
                    set_ical_url.set(format!("https://grumps.app/api/w/{}/calendar/ical/{}", s, token));
                }
            }
        }
    });

    let api_save = auth.api.clone();
    let save_agent = move |_| {
        let api = api_save.clone();
        let s = slug();
        let persona_val = persona.get();
        let proactive_val = proactive.get();
        let auto_mem = auto_memory.get();
        leptos::task::spawn_local(async move {
            let _ = api.update_settings(&s, &serde_json::json!({
                "persona": persona_val,
                "proactive_mode": proactive_val,
                "auto_memory": auto_mem,
            })).await;
        });
    };

    let api_regen = auth.api.clone();
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
        <PageHeader title="Settings".into() subtitle="Workspace configuration".to_string() />
        <div class="flex-1 overflow-y-auto p-8">
            <div class="max-w-xl">
                // General
                <SettingsSection title="General">
                    <SettingRow label="Language" value="English" />
                    <SettingRow label="Timezone" value="Europe/Paris" />
                </SettingsSection>

                // Bot Behavior
                <SettingsSection title="Bot Behavior">
                    <Toggle label="Quiet mode" description="Minimal confirmations in chat" value=quiet_mode on_toggle=move || set_quiet.update(|v| *v = !*v) />
                    <Toggle label="Auto recap" description="Weekly summary sent to the group" value=recap on_toggle=move || set_recap.update(|v| *v = !*v) />
                </SettingsSection>

                // Agent IA
                <SettingsSection title="Agent IA">
                    <div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
                        <div>
                            <div class="font-medium text-sm">"Persona"</div>
                            <div class="text-xs" style="color: var(--ink-40);">"Agent personality"</div>
                        </div>
                        <select
                            class="border-2 border-ink rounded-sm px-3 py-1.5 text-sm bg-transparent outline-none"
                            on:change=move |ev| set_persona.set(event_target_value(&ev))
                            prop:value=persona
                        >
                            <option value="grumps">"Grumps (default)"</option>
                            <option value="assistant">"Neutral assistant"</option>
                            <option value="coach">"Coach"</option>
                        </select>
                    </div>
                    <Toggle
                        label="Proactive mode"
                        description="Agent can send messages without being asked (Pro+)"
                        value=proactive
                        on_toggle=move || set_proactive.update(|v| *v = !*v)
                    />
                    <Toggle
                        label="Auto memory"
                        description="Agent learns from conversations automatically"
                        value=auto_memory
                        on_toggle=move || set_auto_memory.update(|v| *v = !*v)
                    />
                    <div class="py-3" style="border-bottom: 1px solid var(--ink-08);">
                        <a href="#" class="text-sm font-semibold" style="color: var(--teal);">"Voir consentement"</a>
                    </div>
                    <div class="pt-3 pb-1">
                        <button
                            class="px-4 py-2 text-sm font-bold border-2 border-ink rounded-sm cursor-pointer"
                            style="background: var(--ink); color: var(--cream);"
                            on:click=save_agent
                        >"Save Agent Settings"</button>
                    </div>
                </SettingsSection>

                // Calendar
                <SettingsSection title="Calendrier">
                    <div class="py-3 flex flex-col gap-2" style="border-bottom: 1px solid var(--ink-08);">
                        <div class="font-medium text-sm">"iCal subscription URL"</div>
                        <div class="text-xs mb-2" style="color: var(--ink-40);">"Subscribe in any calendar app (Apple, Google, Outlook...)"</div>
                        <div class="flex items-center gap-2">
                            <input
                                type="text" readonly
                                class="flex-1 border-2 border-ink rounded-sm px-3 py-1.5 text-xs bg-transparent outline-none font-mono"
                                placeholder="Generate a token below"
                                prop:value=ical_url
                            />
                            <button
                                class="px-3 py-1.5 text-xs font-bold border-2 border-ink rounded-sm cursor-pointer flex-shrink-0"
                                on:click=copy_ical
                            >"Copy"</button>
                        </div>
                        <div class="flex gap-2 mt-1">
                            <button
                                class="px-3 py-1.5 text-xs font-bold border-2 border-ink rounded-sm cursor-pointer"
                                on:click=regen_ical
                            >"Regenerate"</button>
                            <button
                                class="px-3 py-1.5 text-xs font-semibold border rounded-sm cursor-pointer"
                                style="border-color: var(--brick); color: var(--brick);"
                                on:click=move |_| set_ical_url.set(String::new())
                            >"Revoke"</button>
                        </div>
                    </div>
                </SettingsSection>

                // Quotas
                <SettingsSection title="Quotas ce mois">
                    <QuotaRow label="Agent calls" used=42 limit=200 />
                    <QuotaRow label="Web searches" used=8 limit=50 />
                    <div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
                        <div class="font-medium text-sm">"Storage"</div>
                        <div class="font-semibold text-[13px]" style="color: var(--ink-70);">"12 MB / 100 MB"</div>
                    </div>
                </SettingsSection>

                // Plan
                <SettingsSection title="Plan & Usage">
                    <SettingRow label="Current plan" value="Free" />
                </SettingsSection>
            </div>
        </div>
    }
}

#[component]
fn SettingsSection(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="mb-8">
            <h3 class="font-display text-base font-bold mb-4 pb-2 border-b-2 border-ink">{title}</h3>
            {children()}
        </div>
    }
}

#[component]
fn SettingRow(label: &'static str, value: &'static str) -> impl IntoView {
    view! {
        <div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
            <div class="font-medium text-sm">{label}</div>
            <div class="font-semibold text-[13px]" style="color: var(--ink-70);">{value}</div>
        </div>
    }
}

#[component]
fn Toggle(
    label: &'static str,
    description: &'static str,
    value: ReadSignal<bool>,
    on_toggle: impl Fn() + 'static,
) -> impl IntoView {
    view! {
        <div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
            <div>
                <div class="font-medium text-sm">{label}</div>
                <div class="text-xs" style="color: var(--ink-40);">{description}</div>
            </div>
            <div
                class="w-10 h-6 border-2 border-ink rounded-full relative cursor-pointer transition-colors flex-shrink-0"
                style:background=move || if value.get() { "var(--teal)" } else { "var(--cream)" }
                on:click=move |_| on_toggle()
            >
                <div
                    class="absolute w-3.5 h-3.5 rounded-full top-[1px] transition-all"
                    style:background=move || if value.get() { "white" } else { "var(--ink)" }
                    style:left=move || if value.get() { "18px" } else { "2px" }
                ></div>
            </div>
        </div>
    }
}

#[component]
fn QuotaRow(label: &'static str, used: i64, limit: i64) -> impl IntoView {
    let pct = ((used as f64 / limit as f64) * 100.0) as u32;
    let bar_color = if pct > 80 { "var(--brick)" } else if pct > 60 { "var(--teal)" } else { "var(--ink-40)" };

    view! {
        <div class="py-3 flex flex-col gap-1" style="border-bottom: 1px solid var(--ink-08);">
            <div class="flex items-center justify-between">
                <div class="font-medium text-sm">{label}</div>
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
