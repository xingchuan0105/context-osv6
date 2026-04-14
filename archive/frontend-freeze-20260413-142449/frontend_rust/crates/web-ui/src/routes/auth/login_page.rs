#[component]
pub fn LoginPage() -> impl IntoView {
    let auth = use_auth_state();
    let navigate = use_navigate();
    let locale = use_ui_prefs_state().locale;

    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (error, set_error) = signal(String::new());
    let (loading, set_loading) = signal(false);

    let handle_submit =
        move |ev: SubmitEvent| {
            ev.prevent_default();
            let locale_now = locale.get_untracked();
            let email_val = email.get();
            let password_val = password.get();

            if email_val.is_empty() || password_val.is_empty() {
                set_error.set(t(locale_now, MessageKey::EmailAndPasswordRequiredError).to_string());
                return;
            }

            set_loading.set(true);
            set_error.set(String::new());

            let client = api_client();
            let req = LoginRequest {
                email: email_val.clone(),
                password: password_val.clone(),
            };
            let auth_for_async = auth.clone();
            let navigate_for_async = navigate.clone();

            spawn(async move {
                match client.login(&req).await {
                    Ok(resp) => {
                        if resp.success {
                            if let Some(data) = resp.data {
                                auth_for_async.set_auth(data.token, data.user);
                                navigate_for_async("/dashboard", NavigateOptions::default());
                            } else {
                                set_error.set(resp.error.unwrap_or_else(|| {
                                    t(locale_now, MessageKey::LoginFailed).to_string()
                                }));
                            }
                        } else {
                            set_error.set(resp.error.unwrap_or_else(|| {
                                t(locale_now, MessageKey::LoginFailed).to_string()
                            }));
                        }
                    }
                    Err(e) => {
                        set_error.set(format!("{}: {}", t(locale_now, MessageKey::LoginFailed), e));
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
                        {move || t(locale.get(), MessageKey::SignInTitle)}
                    </h1>
                </div>

                <form on:submit=handle_submit class="space-y-4">
                    <div>
                        <label class="app-form-label" for="login-email">
                            {move || t(locale.get(), MessageKey::EmailLabel)}
                        </label>
                        <input
                            id="login-email"
                            type="email"
                            autocomplete="email"
                            class="app-input"
                            value=move || email.get()
                            on:input=move |ev| set_email.set(event_target_value(&ev))
                            required
                        />
                    </div>

                    <div>
                        <label class="app-form-label" for="login-password">
                            {move || t(locale.get(), MessageKey::PasswordLabel)}
                        </label>
                        <input
                            id="login-password"
                            type="password"
                            autocomplete="current-password"
                            class="app-input"
                            value=move || password.get()
                            on:input=move |ev| set_password.set(event_target_value(&ev))
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
                                t(locale.get(), MessageKey::SignInPending)
                            } else {
                                t(locale.get(), MessageKey::SignInAction)
                            }
                        }}
                    </button>
                </form>

                <div class="space-y-3 text-center">
                    <Show when=move || ui_capabilities().password_reset>
                        <A href="/reset-password" attr:class="app-link">
                            {move || t(locale.get(), MessageKey::ForgotPassword)}
                        </A>
                    </Show>

                    <p class="text-sm text-muted-foreground">
                        <span>{move || t(locale.get(), MessageKey::NoAccountQuestion)}</span>
                        " "
                        <A href="/register" attr:class="app-link">
                            {move || t(locale.get(), MessageKey::SignUpAction)}
                        </A>
                    </p>
                </div>
            </div>
        </AuthFrame>
    }
}

// ----------------------------------------------------------------------------
// RegisterPage
// ----------------------------------------------------------------------------
