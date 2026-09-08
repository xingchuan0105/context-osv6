use crate::api_base::poc_api_base;
use crate::i18n::{tf_now, t_now, use_i18n};
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_router::NavigateOptions;
use web_sdk::{auth_reset_confirm, auth_reset_send_code, auth_reset_verify_code};

#[component]
pub fn ResetPasswordRequestPage() -> impl IntoView {
    let i18n = use_i18n();
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
                error.set(Some(t_now("auth.resetNeedEmail")));
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
                        error.set(Some(tf_now("auth.resetSendFailedDetail", &[("error", &err.to_string())])));
                    }
                }
                loading.set(false);
            });
        }
    };

    view! {
        <div class="auth-page-container">
            <main class="auth-card" aria-label=move || i18n.t("authResetRequestTitle")>
                <header class="auth-header">
                    <h1 class="auth-title">{move || i18n.t("authResetRequestTitle")}</h1>
                    <p class="auth-subtitle">{move || i18n.t("auth.resetRequestLead")}</p>
                </header>
                <form class="auth-form" on:submit=on_submit>
                    <div class="auth-field">
                        <label for="reset-email">{move || i18n.t("authEmailLabel")}</label>
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
                        {move || if loading.get() { i18n.t("authResetSendSubmitting") } else { i18n.t("auth.resetGetCode") }}
                    </button>
                </form>
                <footer class="auth-footer-links">
                    <a href="/login" class="auth-link">{move || i18n.t("authResetBackToLogin")}</a>
                </footer>
            </main>
        </div>
    }
}

#[component]
pub fn ResetPasswordVerifyPage() -> impl IntoView {
    let i18n = use_i18n();
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
                error.set(Some(t_now("auth.resetNeedSix")));
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
                        error.set(Some(tf_now("auth.resetVerifyFailedDetail", &[("error", &err.to_string())])));
                    }
                }
                loading.set(false);
            });
        }
    };

    view! {
        <div class="auth-page-container">
            <main class="auth-card" aria-label=move || i18n.t("auth.resetVerifyAria")>
                <header class="auth-header">
                    <h1 class="auth-title">{move || i18n.t("auth.resetVerifyHeading")}</h1>
                    <p class="auth-subtitle">{move || {
                        let addr = email.get();
                        i18n.tf("auth.resetCodeSentTo", &[("email", addr.as_str())])
                    }}</p>
                </header>
                <form class="auth-form" on:submit=on_submit>
                    <div class="auth-field">
                        <label for="reset-code">{move || i18n.t("authResetCodeLabel")}</label>
                        <input
                            id="reset-code"
                            type="text"
                            data-testid="reset-code"
                            placeholder=move || i18n.t("authResetCodeHint")
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
                        {move || if loading.get() { i18n.t("auth.resetVerifying") } else { i18n.t("auth.resetVerifySubmit") }}
                    </button>
                </form>
                <footer class="auth-footer-links">
                    <a href="/reset-password" class="auth-link">{move || i18n.t("auth.resetResend")}</a>
                </footer>
            </main>
        </div>
    }
}

#[component]
pub fn ResetPasswordConfirmPage() -> impl IntoView {
    let i18n = use_i18n();
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
                error.set(Some(t_now("auth.resetNeedPassword")));
                return;
            }
            if pass_val != confirm_val {
                error.set(Some(t_now("authPasswordMismatch")));
                return;
            }
            if ticket_val.is_empty() {
                error.set(Some(t_now("auth.resetMissingTicket")));
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
                        error.set(Some(tf_now("auth.resetFailedDetail", &[("error", &err.to_string())])));
                    }
                }
                loading.set(false);
            });
        }
    };

    view! {
        <div class="auth-page-container">
            <main class="auth-card" aria-label=move || i18n.t("auth.resetNewPasswordAria")>
                <header class="auth-header">
                    <h1 class="auth-title">{move || i18n.t("authResetConfirmTitle")}</h1>
                    <p class="auth-subtitle">{move || i18n.t("auth.resetNewPasswordLead")}</p>
                </header>
                <form class="auth-form" on:submit=on_submit>
                    <Show when=move || !success.get()>
                        <div class="auth-field">
                            <label for="new-password">{move || i18n.t("auth.resetNewPasswordField")}</label>
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
                    </Show>
                    <Show when=move || !success.get()>
                        <div class="auth-field">
                            <label for="confirm-new-password">{move || i18n.t("auth.resetConfirmNew")}</label>
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
                    </Show>
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
                            {move || if loading.get() { i18n.t("authResetConfirmSubmitting") } else { i18n.t("auth.resetFinish") }}
                        </button>
                    </Show>
                    <Show when=move || success.get()>
                        <div class="auth-success-message" role="status" data-testid="reset-success">
                            <p>{move || i18n.t("auth.resetSuccess")}</p>
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
