use crate::api_base::poc_api_base;
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_router::NavigateOptions;
use web_sdk::{auth_login, write_browser_auth, AuthUser, PersistedAuth};

#[component]
pub fn LoginPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
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
                error.set(Some("请输入邮箱和密码".to_string()));
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
                        error.set(Some(format!("登录失败：{err}")));
                    }
                }
                loading.set(false);
            });
        }
    };

    view! {
        <div class="auth-page-container">
            <main class="auth-card" aria-label="登录">
                <header class="auth-header">
                    <h1 class="auth-title">"登录 Context-OS"</h1>
                    <p class="auth-subtitle">"输入凭据以继续使用"</p>
                </header>
                <form class="auth-form" on:submit=on_submit>
                    <div class="auth-field">
                        <label for="login-email">"邮箱"</label>
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
                        <label for="login-password">"密码"</label>
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
                        {move || if loading.get() { "登录中…" } else { "登录" }}
                    </button>
                </form>
                <footer class="auth-footer-links">
                    <a href="/register" class="auth-link" data-testid="goto-register">"没有账号？立即注册"</a>
                    <span class="auth-divider">"·"</span>
                    <a href="/reset-password" class="auth-link" data-testid="goto-reset">"忘记密码？"</a>
                </footer>
            </main>
        </div>
    }
}
