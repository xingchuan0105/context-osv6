use crate::api_base::poc_api_base;
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_router::NavigateOptions;
use web_sdk::{auth_reset_confirm, auth_reset_send_code, auth_reset_verify_code};

#[component]
pub fn ResetPasswordRequestPage() -> impl IntoView {
    let navigate = use_navigate();
    let email = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let loading = RwSignal::new(false);

    let on_submit = {
        let navigate = navigate.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let email_val = email.get().trim().to_string();
            if email_val.is_empty() {
                error.set(Some("请输入注册邮箱".to_string()));
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
                match auth_reset_send_code(&base, &email_val).await {
                    Ok(_) => {
                        let encoded = web_sdk::encode_uri_component(&email_val);
                        navigate(
                            &format!("/reset-password/verify?email={encoded}"),
                            NavigateOptions::default(),
                        );
                    }
                    Err(err) => {
                        error.set(Some(format!("发送验证码失败：{err}")));
                    }
                }
                loading.set(false);
            });
        }
    };

    view! {
        <div class="auth-page-container">
            <main class="auth-card" aria-label="找回密码">
                <header class="auth-header">
                    <h1 class="auth-title">"找回密码"</h1>
                    <p class="auth-subtitle">"输入你的注册邮箱以接收重置验证码"</p>
                </header>
                <form class="auth-form" on:submit=on_submit>
                    <div class="auth-field">
                        <label for="reset-email">"邮箱"</label>
                        <input
                            id="reset-email"
                            type="email"
                            data-testid="reset-email"
                            placeholder="you@example.com"
                            prop:value=move || email.get()
                            on:input=move |ev| email.set(event_target_value(&ev))
                            required
                        />
                    </div>
                    {move || {
                        error.get().map(|msg| {
                            view! {
                                <p class="auth-error" role="alert" data-testid="reset-error">
                                    {msg}
                                </p>
                            }
                        })
                    }}
                    <button
                        type="submit"
                        class="auth-submit-btn"
                        data-testid="reset-request-submit"
                        disabled=move || loading.get()
                    >
                        {move || if loading.get() { "发送中…" } else { "获取验证码" }}
                    </button>
                </form>
                <footer class="auth-footer-links">
                    <a href="/login" class="auth-link">"返回登录"</a>
                </footer>
            </main>
        </div>
    }
}

#[component]
pub fn ResetPasswordVerifyPage() -> impl IntoView {
    let navigate = use_navigate();
    let query_map = use_query_map();
    let email = Signal::derive(move || query_map.with(|q| q.get("email").unwrap_or_default()));
    let code = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let loading = RwSignal::new(false);

    let on_submit = {
        let navigate = navigate.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let email_val = email.get();
            let code_val = code.get().trim().to_string();
            if code_val.is_empty() {
                error.set(Some("请输入 6 位验证码".to_string()));
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
                match auth_reset_verify_code(&base, &email_val, &code_val).await {
                    Ok(ticket) => {
                        let enc_email = web_sdk::encode_uri_component(&email_val);
                        let enc_ticket = web_sdk::encode_uri_component(&ticket);
                        navigate(
                            &format!("/reset-password/confirm?ticket={enc_ticket}&email={enc_email}"),
                            NavigateOptions::default(),
                        );
                    }
                    Err(err) => {
                        error.set(Some(format!("验证失败：{err}")));
                    }
                }
                loading.set(false);
            });
        }
    };

    view! {
        <div class="auth-page-container">
            <main class="auth-card" aria-label="输入验证码">
                <header class="auth-header">
                    <h1 class="auth-title">"输入重置验证码"</h1>
                    <p class="auth-subtitle">{move || format!("验证码已发送至 {}", email.get())}</p>
                </header>
                <form class="auth-form" on:submit=on_submit>
                    <div class="auth-field">
                        <label for="reset-code">"验证码"</label>
                        <input
                            id="reset-code"
                            type="text"
                            data-testid="reset-code"
                            placeholder="6 位验证码"
                            prop:value=move || code.get()
                            on:input=move |ev| code.set(event_target_value(&ev))
                            required
                        />
                    </div>
                    {move || {
                        error.get().map(|msg| {
                            view! {
                                <p class="auth-error" role="alert" data-testid="verify-error">
                                    {msg}
                                </p>
                            }
                        })
                    }}
                    <button
                        type="submit"
                        class="auth-submit-btn"
                        data-testid="reset-verify-submit"
                        disabled=move || loading.get()
                    >
                        {move || if loading.get() { "核验中…" } else { "核验验证码" }}
                    </button>
                </form>
                <footer class="auth-footer-links">
                    <a href="/reset-password" class="auth-link">"重新发送"</a>
                </footer>
            </main>
        </div>
    }
}

#[component]
pub fn ResetPasswordConfirmPage() -> impl IntoView {
    let navigate = use_navigate();
    let query_map = use_query_map();
    let ticket = Signal::derive(move || query_map.with(|q| q.get("ticket").unwrap_or_default()));
    let new_password = RwSignal::new(String::new());
    let confirm_password = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let success = RwSignal::new(false);
    let loading = RwSignal::new(false);

    let on_submit = {
        let navigate = navigate.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let pass_val = new_password.get();
            let confirm_val = confirm_password.get();
            let ticket_val = ticket.get();

            if pass_val.len() < 8 {
                error.set(Some("密码至少需 8 位".to_string()));
                return;
            }
            if pass_val != confirm_val {
                error.set(Some("两次输入的密码不一致".to_string()));
                return;
            }
            if ticket_val.is_empty() {
                error.set(Some("缺少重置凭据，请重新申请".to_string()));
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
                match auth_reset_confirm(&base, &ticket_val, &pass_val).await {
                    Ok(_) => {
                        success.set(true);
                        leptos::task::spawn_local(async move {
                            // 延时 1.5 秒后跳转到登录
                            sleep_ms(1500).await;
                            navigate("/login", NavigateOptions::default());
                        });
                    }
                    Err(err) => {
                        error.set(Some(format!("重置失败：{err}")));
                    }
                }
                loading.set(false);
            });
        }
    };

    view! {
        <div class="auth-page-container">
            <main class="auth-card" aria-label="设置新密码">
                <header class="auth-header">
                    <h1 class="auth-title">"设置新密码"</h1>
                    <p class="auth-subtitle">"请输入你的新登录密码"</p>
                </header>
                <form class="auth-form" on:submit=on_submit>
                    <div
                        class="auth-field"
                        style=move || if success.get() { "display: none;" } else { "" }
                    >
                        <label for="new-password">"新密码 (至少 8 位)"</label>
                        <input
                            id="new-password"
                            type="password"
                            data-testid="new-password"
                            placeholder="••••••••"
                            prop:value=move || new_password.get()
                            on:input=move |ev| new_password.set(event_target_value(&ev))
                            required
                        />
                    </div>
                    <div
                        class="auth-field"
                        style=move || if success.get() { "display: none;" } else { "" }
                    >
                        <label for="confirm-new-password">"确认新密码"</label>
                        <input
                            id="confirm-new-password"
                            type="password"
                            data-testid="confirm-new-password"
                            placeholder="••••••••"
                            prop:value=move || confirm_password.get()
                            on:input=move |ev| confirm_password.set(event_target_value(&ev))
                            required
                        />
                    </div>
                    {move || {
                        error.get().map(|msg| {
                            view! {
                                <p class="auth-error" role="alert" data-testid="confirm-error">
                                    {msg}
                                </p>
                            }
                        })
                    }}
                    <Show when=move || !success.get()>
                        <button
                            type="submit"
                            class="auth-submit-btn"
                            data-testid="reset-confirm-submit"
                            disabled=move || loading.get()
                        >
                            {move || if loading.get() { "提交中…" } else { "确认修改密码" }}
                        </button>
                    </Show>
                    <Show when=move || success.get()>
                        <div class="auth-success-message" role="status" data-testid="reset-success">
                            <p>"密码修改成功！正在返回登录页…"</p>
                        </div>
                    </Show>
                </form>
            </main>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
async fn sleep_ms(ms: i32) {
    let Some(window) = web_sys::window() else { return; };
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

#[cfg(not(target_arch = "wasm32"))]
async fn sleep_ms(_ms: i32) {}
