#[component]
pub fn LoginPage() -> impl IntoView {
    let auth = use_auth_state();
    let navigate = use_navigate();
    let locale = use_ui_prefs_state().locale;

    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (error, set_error) = signal(String::new());
    let (loading, set_loading) = signal(false);
    let (show_password, set_show_password) = signal(false);

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
        <div class="app-auth-shell">
            <div class="mx-auto flex w-full max-w-[640px] flex-col items-center justify-center">
                <div class="mb-8 flex flex-col items-center text-center">
                    <div class="mb-5 inline-flex h-14 w-14 items-center justify-center rounded-2xl bg-foreground text-background shadow-[0_8px_20px_rgba(0,0,0,0.18)]">
                        <svg class="h-7 w-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M8 5v14M16 5v14M5 8h14M5 16h14"/>
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M8 8a3 3 0 013-3h2a3 3 0 013 3v0a3 3 0 01-3 3h-2a3 3 0 01-3-3z"/>
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M8 16a3 3 0 003 3h2a3 3 0 003-3v0a3 3 0 00-3-3h-2a3 3 0 00-3 3z"/>
                        </svg>
                    </div>

                    <h1 class="text-[46px] font-semibold leading-[1.08] tracking-[-0.02em] text-foreground">
                        {move || {
                            if locale.get() == crate::i18n::Locale::ZhCn {
                                "欢迎回来"
                            } else {
                                "Welcome back"
                            }
                        }}
                    </h1>
                    <p class="mt-2 text-[20px] leading-[1.35] text-muted-foreground">
                        {move || {
                            if locale.get() == crate::i18n::Locale::ZhCn {
                                "登录以继续探索您的知识库"
                            } else {
                                "Sign in to continue with your knowledge base"
                            }
                        }}
                    </p>
                </div>

                <div class="app-surface-card w-full p-7 md:p-8">
                    <form on:submit=handle_submit class="space-y-5">
                        <div>
                            <label class="app-form-label" for="login-email">
                                {move || t(locale.get(), MessageKey::EmailLabel)}
                            </label>
                            <div class="relative">
                                <span class="pointer-events-none absolute inset-y-0 left-4 flex items-center text-muted-foreground">
                                    <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16v12H4z"/>
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M22 7l-10 7L2 7"/>
                                    </svg>
                                </span>
                                <input
                                    id="login-email"
                                    type="email"
                                    autocomplete="email"
                                    class="app-input h-12 pl-12 pr-4"
                                    placeholder="name@example.com"
                                    value=move || email.get()
                                    on:input=move |ev| set_email.set(event_target_value(&ev))
                                    required
                                />
                            </div>
                        </div>

                        <div>
                            <div class="mb-2 flex items-center justify-between gap-3">
                                <label class="app-form-label mb-0" for="login-password">
                                    {move || t(locale.get(), MessageKey::PasswordLabel)}
                                </label>
                                <Show when=move || ui_capabilities().password_reset>
                                    <A href="/reset-password" attr:class="text-sm font-medium text-muted-foreground transition-colors hover:text-foreground">
                                        {move || t(locale.get(), MessageKey::ForgotPassword)}
                                    </A>
                                </Show>
                            </div>

                            <div class="relative">
                                <span class="pointer-events-none absolute inset-y-0 left-4 flex items-center text-muted-foreground">
                                    <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 11c1.657 0 3-1.343 3-3V6a3 3 0 10-6 0v2c0 1.657 1.343 3 3 3z"/>
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 21h14a2 2 0 002-2v-5a2 2 0 00-2-2H5a2 2 0 00-2 2v5a2 2 0 002 2z"/>
                                    </svg>
                                </span>
                                <input
                                    id="login-password"
                                    type=move || if show_password.get() { "text" } else { "password" }
                                    autocomplete="current-password"
                                    class="app-input h-12 pl-12 pr-12"
                                    placeholder="********"
                                    value=move || password.get()
                                    on:input=move |ev| set_password.set(event_target_value(&ev))
                                    required
                                />
                                <button
                                    type="button"
                                    class="absolute inset-y-0 right-3 my-auto inline-flex h-8 w-8 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                                    on:click=move |_| set_show_password.update(|v| *v = !*v)
                                >
                                    <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M1 12s4-7 11-7 11 7 11 7-4 7-11 7S1 12 1 12z"/>
                                        <circle cx="12" cy="12" r="3" stroke-width="2"/>
                                    </svg>
                                </button>
                            </div>
                        </div>

                        {move || {
                            (!error.get().is_empty()).then(|| {
                                view! { <NoticeBanner message=error.get() tone=NoticeTone::Danger /> }
                            })
                        }}

                        <button
                            type="submit"
                            class="mt-1 inline-flex h-12 w-full items-center justify-center gap-2 rounded-[14px] bg-foreground text-base font-semibold text-background transition-colors hover:bg-foreground/90 disabled:cursor-not-allowed disabled:opacity-50"
                            disabled=move || loading.get()
                        >
                            {move || {
                                if loading.get() {
                                    if locale.get() == crate::i18n::Locale::ZhCn {
                                        "登录中..."
                                    } else {
                                        "Signing in..."
                                    }
                                } else if locale.get() == crate::i18n::Locale::ZhCn {
                                    "继续登录"
                                } else {
                                    "Continue"
                                }
                            }}
                            <span aria-hidden="true">{"->"}</span>
                        </button>
                    </form>
                </div>

                <p class="mt-8 text-center text-[18px] text-muted-foreground">
                    <span>{move || t(locale.get(), MessageKey::NoAccountQuestion)}</span>
                    " "
                    <A href="/register" attr:class="font-semibold text-foreground transition-colors hover:opacity-80">
                        {move || t(locale.get(), MessageKey::SignUpAction)}
                    </A>
                </p>

                <p class="mt-11 text-center text-[16px] text-muted-foreground">
                    <span class="mr-1">{"* "}</span>
                    "context-os · Intelligent workspace"
                </p>
            </div>
        </div>
    }
}
