use crate::api_base::poc_api_base;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use web_sdk::{auth_register, write_browser_auth, AuthUser, PersistedAuth};

#[component]
pub fn RegisterPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let navigate = use_navigate();

    let full_name = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let confirm_password = RwSignal::new(String::new());
    let consent = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let loading = RwSignal::new(false);

    let nav_for_effect = navigate.clone();
    Effect::new(move |_| {
        if !token.get().is_empty() {
            nav_for_effect("/chat", NavigateOptions::default());
        }
    });

    let on_submit = {
        let navigate = navigate.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let email_val = email.get().trim().to_string();
            let pass_val = password.get();
            let confirm_val = confirm_password.get();
            let name_val = full_name.get().trim().to_string();

            if email_val.is_empty() || pass_val.is_empty() {
                error.set(Some("请输入邮箱与密码".to_string()));
                return;
            }
            if pass_val.len() < 8 {
                error.set(Some("密码至少需 8 位".to_string()));
                return;
            }
            if pass_val != confirm_val {
                error.set(Some("两次输入的密码不一致".to_string()));
                return;
            }
            if !consent.get() {
                error.set(Some("请阅读并同意用户协议与隐私政策".to_string()));
                return;
            }
            if loading.get() {
                return;
            }

            loading.set(true);
            error.set(None);

            let navigate = navigate.clone();
            leptos::task::spawn_local(async move {
                let base = poc_api_base();
                let name_opt = (!name_val.is_empty()).then_some(name_val.as_str());
                match auth_register(&base, &email_val, &pass_val, name_opt).await {
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
                        navigate("/chat", NavigateOptions::default());
                    }
                    Err(err) => {
                        error.set(Some(format!("注册失败：{err}")));
                    }
                }
                loading.set(false);
            });
        }
    };

    view! {
        <div class="auth-page-container">
            <main class="auth-card" aria-label="注册">
                <header class="auth-header">
                    <h1 class="auth-title">"注册 Context-OS"</h1>
                    <p class="auth-subtitle">"创建新账号以开始使用"</p>
                </header>
                <form class="auth-form" on:submit=on_submit>
                    <div class="auth-field">
                        <label for="reg-name">"姓名 / 昵称"</label>
                        <input
                            id="reg-name"
                            type="text"
                            data-testid="register-name"
                            placeholder="可选"
                            prop:value=move || full_name.get()
                            on:input=move |ev| full_name.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="auth-field">
                        <label for="reg-email">"邮箱"</label>
                        <input
                            id="reg-email"
                            type="email"
                            data-testid="register-email"
                            placeholder="you@example.com"
                            prop:value=move || email.get()
                            on:input=move |ev| email.set(event_target_value(&ev))
                            required
                        />
                    </div>
                    <div class="auth-field">
                        <label for="reg-password">"密码 (至少 8 位)"</label>
                        <input
                            id="reg-password"
                            type="password"
                            data-testid="register-password"
                            placeholder="••••••••"
                            prop:value=move || password.get()
                            on:input=move |ev| password.set(event_target_value(&ev))
                            required
                        />
                    </div>
                    <div class="auth-field">
                        <label for="reg-confirm">"确认密码"</label>
                        <input
                            id="reg-confirm"
                            type="password"
                            data-testid="register-confirm-password"
                            placeholder="••••••••"
                            prop:value=move || confirm_password.get()
                            on:input=move |ev| confirm_password.set(event_target_value(&ev))
                            required
                        />
                    </div>
                    <div class="auth-field-checkbox">
                        <label>
                            <input
                                type="checkbox"
                                data-testid="consent-checkbox"
                                prop:checked=move || consent.get()
                                on:change=move |ev| consent.set(event_target_checked(&ev))
                            />
                            " 我已阅读并同意 "
                            <a href="/legal/terms" target="_blank" rel="noreferrer">"用户协议"</a>
                            " 与 "
                            <a href="/legal/privacy" target="_blank" rel="noreferrer">"隐私政策"</a>
                        </label>
                    </div>
                    {move || {
                        error.get().map(|msg| {
                            view! {
                                <p class="auth-error" role="alert" data-testid="register-error">
                                    {msg}
                                </p>
                            }
                        })
                    }}
                    <button
                        type="submit"
                        class="auth-submit-btn"
                        data-testid="register-submit"
                        disabled=move || loading.get()
                    >
                        {move || if loading.get() { "注册中…" } else { "注册账号" }}
                    </button>
                </form>
                <footer class="auth-footer-links">
                    <a href="/login" class="auth-link" data-testid="goto-login">"已有账号？立即登录"</a>
                </footer>
            </main>
        </div>
    }
}
