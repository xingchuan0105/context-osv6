#[component]
pub fn RegisterPage() -> impl IntoView {
    let auth = use_auth_state();
    let navigate = use_navigate();
    let location = use_location();
    let location_for_submit = location.clone();
    let locale = use_ui_prefs_state().locale;

    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (confirm_password, set_confirm_password) = signal(String::new());
    let (full_name, set_full_name) = signal(String::new());
    let (error, set_error) = signal(String::new());
    let (loading, set_loading) = signal(false);

    let handle_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let locale_now = locale.get_untracked();
        let email_val = email.get();
        let password_val = password.get();
        let confirm_password_val = confirm_password.get();
        let full_name_val = full_name.get();

        if email_val.is_empty() || password_val.is_empty() {
            set_error.set(t(locale_now, MessageKey::EmailAndPasswordRequiredError).to_string());
            return;
        }

        if password_val.len() < 8 {
            set_error.set(t(locale_now, MessageKey::PasswordTooShortError).to_string());
            return;
        }

        if password_val != confirm_password_val {
            set_error.set(t(locale_now, MessageKey::PasswordMismatchError).to_string());
            return;
        }

        set_loading.set(true);
        set_error.set(String::new());

        let client = api_client();
        let req = RegisterRequest {
            email: email_val.clone(),
            password: password_val.clone(),
            full_name: if full_name_val.is_empty() {
                None
            } else {
                Some(full_name_val)
            },
        };
        let auth_for_async = auth.clone();
        let navigate_for_async = navigate.clone();
        let dashboard_path =
            scoped_auth_path(&location_for_submit.pathname.get_untracked(), "/dashboard");

        spawn(async move {
            match client.register(&req).await {
                Ok(resp) => {
                    if resp.success {
                        if let Some(data) = resp.data {
                            auth_for_async.set_auth(data.token, data.user);
                            navigate_for_async(&dashboard_path, NavigateOptions::default());
                        } else {
                            set_error.set(resp.error.unwrap_or_else(|| {
                                t(locale_now, MessageKey::RegistrationFailed).to_string()
                            }));
                        }
                    } else {
                        set_error.set(resp.error.unwrap_or_else(|| {
                            t(locale_now, MessageKey::RegistrationFailed).to_string()
                        }));
                    }
                }
                Err(e) => {
                    set_error.set(describe_auth_error(
                        locale_now,
                        &t(locale_now, MessageKey::RegistrationFailed).to_string(),
                        &e,
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
                        {move || t(locale.get(), MessageKey::CreateAccountTitle)}
                    </h1>
                </div>

                <form on:submit=handle_submit class="space-y-4">
                    <div>
                        <label class="app-form-label" for="reg-name">
                            {move || t(locale.get(), MessageKey::FullNameLabel)}
                        </label>
                        <input
                            id="reg-name"
                            type="text"
                            autocomplete="name"
                            class="app-input"
                            value=move || full_name.get()
                            on:input=move |ev| set_full_name.set(event_target_value(&ev))
                        />
                    </div>

                    <div>
                        <label class="app-form-label" for="reg-email">
                            {move || t(locale.get(), MessageKey::EmailLabel)}
                        </label>
                        <input
                            id="reg-email"
                            type="email"
                            autocomplete="email"
                            class="app-input"
                            value=move || email.get()
                            on:input=move |ev| set_email.set(event_target_value(&ev))
                            required
                        />
                    </div>

                    <div>
                        <label class="app-form-label" for="reg-password">
                            {move || t(locale.get(), MessageKey::PasswordLabel)}
                        </label>
                        <input
                            id="reg-password"
                            type="password"
                            autocomplete="new-password"
                            class="app-input"
                            value=move || password.get()
                            on:input=move |ev| set_password.set(event_target_value(&ev))
                            required
                        />
                        <p class="mt-2 text-xs text-muted-foreground">
                            {move || t(locale.get(), MessageKey::PasswordTooShortError)}
                        </p>
                    </div>

                    <div>
                        <label class="app-form-label" for="reg-password-confirm">
                            {move || t(locale.get(), MessageKey::ConfirmPasswordLabel)}
                        </label>
                        <input
                            id="reg-password-confirm"
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
                                t(locale.get(), MessageKey::CreateAccountPending)
                            } else {
                                t(locale.get(), MessageKey::CreateAccountAction)
                            }
                        }}
                    </button>
                </form>

                <p class="text-center text-sm text-muted-foreground">
                    <span>{move || t(locale.get(), MessageKey::AlreadyHaveAccount)}</span>
                    " "
                    <A
                        href=move || scoped_auth_path(&location.pathname.get(), "/login")
                        attr:class="app-link"
                    >
                        {move || t(locale.get(), MessageKey::SignInAction)}
                    </A>
                </p>
            </div>
        </AuthFrame>
    }
}

// ----------------------------------------------------------------------------
// ResetPasswordPage - Step 1: Send reset code
// ----------------------------------------------------------------------------
