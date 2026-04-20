use leptos::prelude::*;
use crate::auth::use_auth;
use crate::i18n::tr;

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
        <div class="min-h-screen flex items-center justify-center" style="background: var(--cream);">
            <div class="w-96 border-2 border-ink rounded-sm" style="background: var(--cream-light);">
                // Header
                <div class="px-8 pt-8 pb-6 text-center border-b-2 border-ink">
                    <h1 class="font-display text-3xl font-extrabold uppercase tracking-tight">
                        "GRUMPS"<span class="text-brick">"."</span>
                    </h1>
                    <p class="text-xs uppercase tracking-widest mt-1" style="color: var(--ink-40);">
                        {move || if step.get() == 1 { tr("brand.tagline") } else { tr("auth.login.check_messaging") }}
                    </p>
                </div>

                // Body
                <div class="px-8 py-7">
                    // Error display
                    {move || error.get().map(|e| view! {
                        <div class="mb-4 p-3 text-sm border-2 border-brick text-brick rounded-sm">{e}</div>
                    })}

                    // Step 1: Phone
                    <div style:display=move || if step.get() == 1 { "block" } else { "none" }>
                        <label class="block text-xs font-semibold uppercase tracking-wider mb-2" style="color: var(--ink-70);">
                            {move || tr("auth.login.phone_label")}
                        </label>
                        <input
                            type="tel"
                            class="w-full text-base p-3 border-2 border-ink rounded-sm outline-none focus:border-brick"
                            style="background: var(--cream); font-family: var(--font-body);"
                            prop:placeholder=move || tr("auth.login.phone_placeholder")
                            on:input=move |ev| set_phone.set(event_target_value(&ev))
                            prop:value=phone
                        />
                        <button
                            class="w-full mt-4 p-3 text-sm font-bold uppercase tracking-wider border-2 border-ink rounded-sm cursor-pointer transition-colors"
                            style="background: var(--ink); color: var(--cream); font-family: var(--font-body);"
                            on:click=send_otp
                            disabled=loading
                        >
                            {move || if loading.get() { tr("auth.login.sending") } else { tr("auth.login.send_code") }}
                        </button>
                        <p class="text-center text-xs mt-4" style="color: var(--ink-40); line-height: 1.5;">
                            {move || tr("auth.login.note_send")}
                        </p>
                    </div>

                    // Step 2: OTP
                    <div style:display=move || if step.get() == 2 { "block" } else { "none" }>
                        <label class="block text-xs font-semibold uppercase tracking-wider mb-2" style="color: var(--ink-70);">
                            {move || tr("auth.login.code_label")}
                        </label>
                        <input
                            type="text"
                            class="w-full text-center text-2xl font-display font-bold p-3 border-2 border-ink rounded-sm outline-none focus:border-brick tracking-widest"
                            style="background: var(--cream);"
                            maxlength="6"
                            placeholder="000000"
                            on:input=move |ev| set_code.set(event_target_value(&ev))
                            prop:value=code
                        />
                        <button
                            class="w-full mt-4 p-3 text-sm font-bold uppercase tracking-wider border-2 border-ink rounded-sm cursor-pointer transition-colors"
                            style="background: var(--ink); color: var(--cream); font-family: var(--font-body);"
                            on:click=verify
                            disabled=loading
                        >
                            {move || if loading.get() { tr("auth.login.verifying") } else { tr("auth.login.verify") }}
                        </button>
                        <p class="text-center text-xs mt-4" style="color: var(--ink-40);">
                            {move || tr("auth.login.didnt_get_it")}
                            <a href="#" class="font-semibold" style="color: var(--brick);"
                                on:click=move |_| set_step.set(1)>
                                {move || tr("auth.login.try_again")}
                            </a>
                        </p>
                    </div>
                </div>
            </div>
        </div>
    }
}
