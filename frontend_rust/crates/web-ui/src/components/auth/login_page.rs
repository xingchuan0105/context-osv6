use crate::api_base::poc_api_base;
use crate::i18n::{tf_now, t_now, use_i18n};
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_router::NavigateOptions;
use web_sdk::{auth_login, write_browser_auth, AuthUser, PersistedAuth};

#[component]
pub fn LoginPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let i18n = use_i18n();
    let navigate = use_navigate();
    let query_map = use_query_map();

    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let loading = RwSignal::new(false);

    let next_path = Signal::derive(move || {
        query_map.with(|q| q.get("next").filter(|p| !p.trim().is_empty()))
    });

    let nav_for_effect = navigate.clone();
    // 已经登录的用户直接导向聊天
    Effect::new(move |_| {
        if !token.get().is_empty() {
            let target = next_path.get().unwrap_or_else(|| "/chat".to_string());
            nav_for_effect(&target, NavigateOptions::default());
        }
    });

    let on_submit = {
        let navigate = navigate.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let email_val = email.get().trim().to_string();
            let pass_val = password.get();
            if email_val.is_empty() || pass_val.is_empty() {
                error.set(Some(t_now("auth.needEmailPassword")));
                return;
            }
            if loading.get() {
                return;
            }

            loading.set(true);
            error.set(None);

            let navigate = navigate.clone();
            let target = next_path.get().unwrap_or_else(|| "/chat".to_string());
            leptos::task::spawn_local(async move {
                let base = poc_api_base();
                match auth_login(&base, &email_val, &pass_val).await {
                    Ok(payload) => {
                        let auth = PersistedAuth {
                            token: payload.token.clone(),
                            user: AuthUser {
                                id: payload.user.id,
                                email: payload.user.email,
                                full_name: payload.user.full_name,
                                bio: payload.user.bio,
                                contact_url: payload.user.contact_url,
                                avatar_url: payload.user.avatar_url,
                                banner_url: payload.user.banner_url,
                                public_profile_enabled: Some(payload.user.public_profile_enabled),
                            },
                        };
                        write_browser_auth(&auth);
                        token.set(payload.token);
                        navigate(&target, NavigateOptions::default());
                    }
                    Err(err) => {
                        error.set(Some(tf_now("auth.loginFailedDetail", &[("error", &err.to_string())])));
                    }
                }
                loading.set(false);
            });
        }
    };

    view! {
        <div class="auth-page-container">
            <main class="auth-card" aria-label=move || i18n.t("auth.loginAria")>
                <header class="auth-header">
                    <h1 class="auth-title">{move || i18n.t("auth.loginHeading")}</h1>
                    <p class="auth-subtitle">{move || i18n.t("auth.loginLead")}</p>
                </header>
                <form class="auth-form" on:submit=on_submit>
                    <div class="auth-field">
                        <label for="login-email">{move || i18n.t("authEmailLabel")}</label>
                        <input
                            id="login-email"
                            type="email"
                            data-testid="login-email"
                            placeholder="you@example.com"
                            prop:value=move || email.get()
                            on:input=move |ev| email.set(event_target_value(&ev))
                            required
                        />
                    </div>
                    <div class="auth-field">
                        <label for="login-password">{move || i18n.t("authPasswordLabel")}</label>
                        <input
                            id="login-password"
                            type="password"
                            data-testid="login-password"
                            placeholder="••••••••"
                            prop:value=move || password.get()
                            on:input=move |ev| password.set(event_target_value(&ev))
                            required
                        />
                    </div>
                    {move || {
                        error.get().map(|msg| {
                            view! {
                                <p class="auth-error" role="alert" data-testid="login-error">
                                    {msg}
                                </p>
                            }
                        })
                    }}
                    <button
                        type="submit"
                        class="auth-submit-btn"
                        data-testid="login-submit"
                        disabled=move || loading.get()
                    >
                        {move || if loading.get() { i18n.t("authLoginSubmitting") } else { i18n.t("auth.loginAria") }}
                    </button>
                </form>
                <footer class="auth-footer-links">
                    <a href="/register" class="auth-link" data-testid="goto-register">{move || i18n.t("auth.noAccountCta")}</a>
                    <span class="auth-divider">"·"</span>
                    <a href="/reset-password" class="auth-link" data-testid="goto-reset">{move || i18n.t("authForgotPassword")}</a>
                </footer>
            </main>
        </div>
    }
}
