#[component]
pub fn VerifyResetPage() -> impl IntoView {
    let locale = use_ui_prefs_state().locale;

    if !ui_capabilities().password_reset {
        return view! {
            <AuthFrame>
                <UnavailableFeatureCard
                    title={t(locale.get_untracked(), MessageKey::ResetPasswordTitle).to_string()}
                    description={t(locale.get_untracked(), MessageKey::ResetPasswordIntro).to_string()}
                />
            </AuthFrame>
        }
        .into_any();
    }

    let navigate = use_navigate();

    let params = use_query_map();
    let email_from_url = move || params.get().get("email").unwrap_or_default();

    let (email, set_email) = signal(String::new());
    let (code, set_code) = signal(String::new());
    let (error, set_error) = signal(String::new());
    let (loading, set_loading) = signal(false);

    Effect::new(move |_| {
        let url_email = email_from_url();
        if !url_email.is_empty() {
            set_email.set(url_email);
        }
    });

    let handle_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let locale_now = locale.get_untracked();
        let email_val = email.get();
        let code_val = code.get();

        if email_val.is_empty() || code_val.is_empty() {
            set_error.set(t(locale_now, MessageKey::EmailAndCodeRequiredError).to_string());
            return;
        }

        set_loading.set(true);
        set_error.set(String::new());

        let client = api_client();
        let req = VerifyResetCodeRequest {
            email: email_val.clone(),
            code: code_val.clone(),
        };
        let navigate_for_async = navigate.clone();

        spawn(async move {
            match client.verify_reset_code(&req).await {
                Ok(resp) => {
                    if resp.success {
                        if let Some(data) = resp.data {
                            if let Some(ticket) = data.reset_ticket {
                                let encoded_ticket = urlencoding::encode(&ticket);
                                navigate_for_async(
                                    &format!("/reset-password/confirm?ticket={}", encoded_ticket),
                                    NavigateOptions::default(),
                                );
                            } else {
                                set_error
                                    .set(t(locale_now, MessageKey::NoResetTicketError).to_string());
                            }
                        } else {
                            set_error.set(resp.error.unwrap_or_else(|| {
                                t(locale_now, MessageKey::VerificationFailed).to_string()
                            }));
                        }
                    } else {
                        set_error.set(resp.error.unwrap_or_else(|| {
                            t(locale_now, MessageKey::VerificationFailed).to_string()
                        }));
                    }
                }
                Err(e) => {
                    set_error.set(format!(
                        "{}: {}",
                        t(locale_now, MessageKey::VerificationFailed),
                        e
                    ));
                }
            }
            set_loading.set(false);
        });
    };

    view! {
        <AuthFrame>
            <div class="space-y-6">
                <div class="space-y-2 text-center">
                    <h1 class="app-page-title">
                        {move || t(locale.get(), MessageKey::VerifyResetCodeTitle)}
                    </h1>
                    <p class="app-page-subtitle">
                        {move || t(locale.get(), MessageKey::VerifyResetCodeIntro)}
                    </p>
                </div>

                <form on:submit=handle_submit class="space-y-4">
                    <div>
                        <label class="app-form-label" for="verify-email">
                            {move || t(locale.get(), MessageKey::EmailLabel)}
                        </label>
                        <input
                            id="verify-email"
                            type="email"
                            autocomplete="email"
                            class="app-input"
                            value=move || email.get()
                            on:input=move |ev| set_email.set(event_target_value(&ev))
                            required
                        />
                    </div>

                    <div>
                        <label class="app-form-label" for="verify-code">
                            {move || t(locale.get(), MessageKey::ResetCodeLabel)}
                        </label>
                        <input
                            id="verify-code"
                            type="text"
                            autocomplete="one-time-code"
                            class="app-input"
                            value=move || code.get()
                            on:input=move |ev| set_code.set(event_target_value(&ev))
                            required
                        />
                    </div>

                    {move || {
                        (!error.get().is_empty()).then(|| {
                            view! { <NoticeBanner message=error.get() tone=NoticeTone::Danger /> }
                        })
                    }}

                    <button
                        type="submit"
                        class="app-button-primary w-full"
                        disabled=move || loading.get()
                    >
                        {move || {
                            if loading.get() {
                                t(locale.get(), MessageKey::VerifyingAction)
                            } else {
                                t(locale.get(), MessageKey::VerifyCodeAction)
                            }
                        }}
                    </button>
                </form>

                <div class="text-center">
                    <A href="/reset-password" attr:class="app-link">
                        {move || t(locale.get(), MessageKey::RequestAnotherCode)}
                    </A>
                </div>
            </div>
        </AuthFrame>
    }
    .into_any()
}

// ----------------------------------------------------------------------------
// ConfirmResetPage - Step 3: Set new password
// ----------------------------------------------------------------------------
