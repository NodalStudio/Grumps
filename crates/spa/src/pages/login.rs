use leptos::prelude::*;
use crate::auth::use_auth;

#[component]
pub fn LoginPage() -> impl IntoView {
    let auth = use_auth();
    let (step, set_step) = signal(1u8);
    let (phone, set_phone) = signal(String::new());
    let (code, set_code) = signal(String::new());
    let (error, set_error) = signal(None::<String>);
    let (loading, set_loading) = signal(false);

    let auth2 = auth.clone();
    let send_otp = move |_| {
        let auth = auth2.clone();
        let phone_val = phone.get();
        set_loading.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            match auth.api.send_otp(&phone_val).await {
                Ok(_) => set_step.set(2),
                Err(e) => set_error.set(Some(e)),
            }
            set_loading.set(false);
        });
    };

    let verify = move |_| {
        let auth = auth.clone();
        let phone_val = phone.get();
        let code_val = code.get();
        set_loading.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            match auth.api.verify_otp(&phone_val, &code_val).await {
                Ok(resp) => {
                    auth.login(resp);
                    let window = web_sys::window().unwrap();
                    let _ = window.location().set_href("/dashboard");
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_loading.set(false);
        });
    };

    view! {
        <div class="min-h-screen flex items-center justify-center bg-surface">
            <div class="w-96 border-2 border-strong rounded-sm bg-surface-raised">
                // Header
                <div class="px-8 pt-8 pb-6 text-center border-b-2 border-strong">
                    <h1 class="font-display text-3xl font-extrabold uppercase tracking-tight">
                        "GRUMPS"<span class="text-accent">"."</span>
                    </h1>
                    <p class="text-xs uppercase tracking-widest mt-1 text-muted">
                        {move || if step.get() == 1 { "Gets it done. No small talk." } else { "Check your WhatsApp" }}
                    </p>
                </div>

                // Body
                <div class="px-8 py-7">
                    // Error display
                    {move || error.get().map(|e| view! {
                        <div class="mb-4 p-3 text-sm border-2 border-accent text-accent rounded-sm">{e}</div>
                    })}

                    // Step 1: Phone
                    <div style:display=move || if step.get() == 1 { "block" } else { "none" }>
                        <label class="block text-xs font-semibold uppercase tracking-wider mb-2 text-secondary">
                            "Your WhatsApp number"
                        </label>
                        <input
                            type="tel"
                            class="w-full text-base p-3 border-2 border-strong rounded-sm outline-none focus:border-accent bg-surface"
                            style="font-family: var(--font-body);"
                            placeholder="+33 6 12 34 56 78"
                            on:input=move |ev| set_phone.set(event_target_value(&ev))
                            prop:value=phone
                        />
                        <button
                            class="w-full mt-4 p-3 text-sm font-bold uppercase tracking-wider border-2 border-strong rounded-sm cursor-pointer transition-colors bg-primary text-surface"
                            style="font-family: var(--font-body);"
                            on:click=send_otp
                            disabled=loading
                        >
                            {move || if loading.get() { "Sending..." } else { "Send code" }}
                        </button>
                        <p class="text-center text-xs mt-4 text-muted" style="line-height: 1.5;">
                            "We\u{2019}ll send a 6-digit code to verify you\u{2019}re in the group."
                        </p>
                    </div>

                    // Step 2: OTP
                    <div style:display=move || if step.get() == 2 { "block" } else { "none" }>
                        <label class="block text-xs font-semibold uppercase tracking-wider mb-2 text-secondary">
                            "Enter the 6-digit code"
                        </label>
                        <input
                            type="text"
                            class="w-full text-center text-2xl font-display font-bold p-3 border-2 border-strong rounded-sm outline-none focus:border-accent tracking-widest bg-surface"
                            maxlength="6"
                            placeholder="000000"
                            on:input=move |ev| set_code.set(event_target_value(&ev))
                            prop:value=code
                        />
                        <button
                            class="w-full mt-4 p-3 text-sm font-bold uppercase tracking-wider border-2 border-strong rounded-sm cursor-pointer transition-colors bg-primary text-surface"
                            style="font-family: var(--font-body);"
                            on:click=verify
                            disabled=loading
                        >
                            {move || if loading.get() { "Verifying..." } else { "Verify" }}
                        </button>
                        <p class="text-center text-xs mt-4 text-muted">
                            "Didn\u{2019}t get it? "
                            <a href="#" class="font-semibold text-accent"
                                on:click=move |_| set_step.set(1)>
                                "Try again"
                            </a>
                        </p>
                    </div>
                </div>
            </div>
        </div>
    }
}
