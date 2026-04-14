#[component]
pub fn ConfirmResetPage() -> impl IntoView {
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
    let reset_ticket = move || params.get().get("ticket").unwrap_or_default();

    let (new_password, set_new_password) = signal(String::new());
    let (confirm_password, set_confirm_password) = signal(String::new());
    let (error, set_error) = signal(String::new());
    let (loading, set_loading) = signal(false);

    let handle_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let locale_now = locale.get_untracked();
        let ticket = reset_ticket();
        let password_val = new_password.get();
        let confirm_val = confirm_password.get();

        if ticket.is_empty() {
            set_error.set(t(locale_now, MessageKey::InvalidResetSessionError).to_string());
            return;
        }

        if password_val.is_empty() {
            set_error.set(t(locale_now, MessageKey::PasswordRequiredError).to_string());
            return;
        }

        if password_val != confirm_val {
            set_error.set(t(locale_now, MessageKey::PasswordMismatchError).to_string());
            return;
        }

        if password_val.len() < 8 {
            set_error.set(t(locale_now, MessageKey::PasswordTooShortError).to_string());
            return;
        }

        set_loading.set(true);
        set_error.set(String::new());

        let client = api_client();
        let req = ConfirmResetPasswordRequest {
            reset_ticket: ticket,
            new_password: password_val,
        };
        let navigate_for_async = navigate.clone();

        spawn(async move {
            match client.confirm_reset_password(&req).await {
                Ok(_) => {
                    navigate_for_async("/login", NavigateOptions::default());
                }
                Err(e) => {
                    set_error.set(format!(
                        "{}: {}",
                        t(locale_now, MessageKey::ResetPasswordFailed),
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
                        {move || t(locale.get(), MessageKey::SetNewPasswordTitle)}
                    </h1>
                    <p class="app-page-subtitle">
                        {move || t(locale.get(), MessageKey::SetNewPasswordIntro)}
                    </p>
                </div>

                <form on:submit=handle_submit class="space-y-4">
                    <div>
                        <label class="app-form-label" for="new-password">
                            {move || t(locale.get(), MessageKey::NewPasswordLabel)}
                        </label>
                        <input
                            id="new-password"
                            type="password"
                            autocomplete="new-password"
                            class="app-input"
                            value=move || new_password.get()
                            on:input=move |ev| set_new_password.set(event_target_value(&ev))
                            required
                        />
                    </div>

                    <div>
                        <label class="app-form-label" for="confirm-password">
                            {move || t(locale.get(), MessageKey::ConfirmPasswordLabel)}
                        </label>
                        <input
                            id="confirm-password"
                            type="password"
                            autocomplete="new-password"
                            class="app-input"
                            value=move || confirm_password.get()
                            on:input=move |ev| set_confirm_password.set(event_target_value(&ev))
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
                                t(locale.get(), MessageKey::ResettingAction)
                            } else {
                                t(locale.get(), MessageKey::ResetPasswordAction)
                            }
                        }}
                    </button>
                </form>
            </div>
        </AuthFrame>
    }
    .into_any()
}
