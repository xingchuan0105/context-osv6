#[component]
pub fn ResetPasswordPage() -> impl IntoView {
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

    let (email, set_email) = signal(String::new());
    let (error, set_error) = signal(String::new());
    let (success, set_success) = signal(false);
    let (loading, set_loading) = signal(false);

    let handle_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let locale_now = locale.get_untracked();
        let email_val = email.get();

        if email_val.is_empty() {
            set_error.set(t(locale_now, MessageKey::EmailRequiredError).to_string());
            return;
        }

        set_loading.set(true);
        set_error.set(String::new());
        set_success.set(false);

        let client = api_client();
        let req = SendResetCodeRequest {
            email: email_val.clone(),
            lang: Some(locale_now.as_str().to_string()),
        };
        let navigate_for_async = navigate.clone();

        spawn(async move {
            match client.send_reset_code(&req).await {
                Ok(_) => {
                    set_success.set(true);
                    let encoded_email = urlencoding::encode(&email_val);
                    navigate_for_async(
                        &format!("/reset-password/verify?email={}", encoded_email),
                        NavigateOptions::default(),
                    );
                }
                Err(e) => {
                    set_error.set(format!(
                        "{}: {}",
                        t(locale_now, MessageKey::SendResetCodeFailed),
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
                        {move || t(locale.get(), MessageKey::ResetPasswordTitle)}
                    </h1>
                    <p class="app-page-subtitle">
                        {move || t(locale.get(), MessageKey::ResetPasswordIntro)}
                    </p>
                </div>

                <form on:submit=handle_submit class="space-y-4">
                    <div>
                        <label class="app-form-label" for="reset-email">
                            {move || t(locale.get(), MessageKey::EmailLabel)}
                        </label>
                        <input
                            id="reset-email"
                            type="email"
                            autocomplete="email"
                            class="app-input"
                            value=move || email.get()
                            on:input=move |ev| set_email.set(event_target_value(&ev))
                            required
                        />
                    </div>

                    {move || {
                        (!error.get().is_empty()).then(|| {
                            view! { <NoticeBanner message=error.get() tone=NoticeTone::Danger /> }
                        })
                    }}

                    {move || {
                        success.get().then(|| {
                            view! {
                                <NoticeBanner
                                    message=t(locale.get(), MessageKey::ResetCodeSentRedirecting).to_string()
                                    tone=NoticeTone::Success
                                />
                            }
                        })
                    }}

                    <button
                        type="submit"
                        class="app-button-primary w-full"
                        disabled=move || loading.get()
                    >
                        {move || {
                            if loading.get() {
                                t(locale.get(), MessageKey::SendingAction)
                            } else {
                                t(locale.get(), MessageKey::SendResetCodeAction)
                            }
                        }}
                    </button>
                </form>

                <div class="text-center">
                    <A href="/login" attr:class="app-link">
                        {move || t(locale.get(), MessageKey::BackToSignIn)}
                    </A>
                </div>
            </div>
        </AuthFrame>
    }
    .into_any()
}

// ----------------------------------------------------------------------------
// VerifyResetPage - Step 2: Verify the reset code
// ----------------------------------------------------------------------------
