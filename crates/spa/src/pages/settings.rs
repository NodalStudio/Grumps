use leptos::prelude::*;
use crate::components::header::PageHeader;

#[component]
pub fn SettingsPage() -> impl IntoView {
    let (quiet_mode, set_quiet) = signal(false);
    let (recap, set_recap) = signal(true);

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
                    <div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
                        <div>
                            <div class="font-medium text-sm">"Quiet mode"</div>
                            <div class="text-xs" style="color: var(--ink-40);">"Minimal confirmations in chat"</div>
                        </div>
                        <div
                            class="w-10 h-6 border-2 border-ink rounded-full relative cursor-pointer transition-colors"
                            style:background=move || if quiet_mode.get() { "var(--teal)" } else { "var(--cream)" }
                            on:click=move |_| set_quiet.update(|v| *v = !*v)
                        >
                            <div
                                class="absolute w-3.5 h-3.5 rounded-full top-[1px] transition-all"
                                style:background=move || if quiet_mode.get() { "white" } else { "var(--ink)" }
                                style:left=move || if quiet_mode.get() { "18px" } else { "2px" }
                            ></div>
                        </div>
                    </div>
                    <div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
                        <div>
                            <div class="font-medium text-sm">"Auto recap"</div>
                            <div class="text-xs" style="color: var(--ink-40);">"Weekly summary sent to the group"</div>
                        </div>
                        <div
                            class="w-10 h-6 border-2 border-ink rounded-full relative cursor-pointer transition-colors"
                            style:background=move || if recap.get() { "var(--teal)" } else { "var(--cream)" }
                            on:click=move |_| set_recap.update(|v| *v = !*v)
                        >
                            <div
                                class="absolute w-3.5 h-3.5 rounded-full top-[1px] transition-all"
                                style:background=move || if recap.get() { "white" } else { "var(--ink)" }
                                style:left=move || if recap.get() { "18px" } else { "2px" }
                            ></div>
                        </div>
                    </div>
                </SettingsSection>

                // Plan
                <SettingsSection title="Plan & Usage">
                    <SettingRow label="Current plan" value="Free" />
                    <SettingRow label="Storage used" value="0 MB / 100 MB" />
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
