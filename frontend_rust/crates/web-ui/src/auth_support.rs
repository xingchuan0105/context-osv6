use anyhow::Error;
use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local;
use web_sdk::{ApiClient, ApiError};

use crate::api::api_base_url;
use crate::i18n::{Locale, choose};
use crate::platform::ui_capabilities;

const RESET_EMAIL_STORAGE_KEY: &str = "context_os.reset.email.v1";
const RESET_TICKET_STORAGE_KEY: &str = "context_os.reset.ticket.v1";

#[cfg(not(target_arch = "wasm32"))]
fn env_first(keys: &[&str], default: &str) -> String {
    keys.iter()
        .find_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| default.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn password_reset_enabled_on_server() -> bool {
    let email_provider = env_first(&["EMAIL_PROVIDER"], "smtp");
    let smtp_host = env_first(&["MAIL_HOST", "SMTP_HOST"], "smtp.163.com");
    let smtp_from = env_first(&["MAIL_FROM", "SMTP_FROM"], "");
    email_provider.eq_ignore_ascii_case("smtp")
        && !smtp_host.trim().is_empty()
        && !smtp_from.trim().is_empty()
}

#[cfg(target_arch = "wasm32")]
fn password_reset_enabled_on_server() -> bool {
    true
}

fn fallback_auth_error(locale: Locale) -> String {
    choose(
        locale,
        "服务暂时不可用，请稍后再试。",
        "Service is temporarily unavailable. Please try again later.",
    )
    .to_string()
}

pub(crate) fn describe_auth_error(locale: Locale, fallback: &str, error: &Error) -> String {
    let Some(api_error) = error.downcast_ref::<ApiError>() else {
        return fallback.to_string();
    };

    let message = match api_error.code() {
        Some("account_not_registered") | Some("email_not_registered") => choose(
            locale,
            "此账号还未注册，请先注册。",
            "This account is not registered yet. Please sign up first.",
        )
        .to_string(),
        Some("invalid_password") => choose(locale, "密码错误。", "Incorrect password.").to_string(),
        Some("invalid_credentials") => {
            choose(locale, "邮箱或密码错误。", "Incorrect email or password.").to_string()
        }
        Some("email_exists") => choose(
            locale,
            "该邮箱已注册，请直接登录。",
            "This email is already registered. Please sign in.",
        )
        .to_string(),
        Some("password_reset_unavailable") => choose(
            locale,
            "当前环境未启用密码找回，请联系管理员。",
            "Password reset is not available in this environment.",
        )
        .to_string(),
        Some("invalid_reset_ticket") => choose(
            locale,
            "重置会话无效或已过期。",
            "Reset session is invalid or expired.",
        )
        .to_string(),
        Some("service_unavailable") => fallback_auth_error(locale),
        Some("validation_error") => {
            let server_message = api_error.message().trim();
            if server_message.is_empty() {
                fallback.to_string()
            } else {
                server_message.to_string()
            }
        }
        _ => {
            let server_message = api_error.message().trim();
            if server_message.is_empty() {
                fallback.to_string()
            } else {
                server_message.to_string()
            }
        }
    };

    if message.is_empty() {
        fallback.to_string()
    } else {
        message
    }
}

pub(crate) fn use_password_reset_enabled() -> ReadSignal<bool> {
    let initial = ui_capabilities().password_reset && password_reset_enabled_on_server();
    let (enabled, _set_enabled) = signal(initial);

    #[cfg(target_arch = "wasm32")]
    {
        let set_enabled = _set_enabled;
        Effect::new(move |_| {
            spawn_local(async move {
                let next = match ApiClient::new(api_base_url())
                    .auth_runtime_capabilities()
                    .await
                {
                    Ok(response) => initial && response.password_reset_enabled,
                    Err(_) => initial,
                };
                set_enabled.set(next);
            });
        });
    }

    enabled
}

pub(crate) async fn logout_current_session(token: Option<String>) {
    if let Some(token) = token {
        let _ = ApiClient::new(api_base_url())
            .with_auth(token)
            .logout()
            .await;
    }
}

#[cfg(target_arch = "wasm32")]
fn session_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.session_storage().ok().flatten()
}

#[cfg(target_arch = "wasm32")]
fn read_storage_value(key: &str) -> Option<String> {
    session_storage()?.get_item(key).ok().flatten()
}

#[cfg(not(target_arch = "wasm32"))]
fn read_storage_value(_key: &str) -> Option<String> {
    None
}

#[cfg(target_arch = "wasm32")]
fn write_storage_value(key: &str, value: &str) {
    if let Some(storage) = session_storage() {
        let _ = storage.set_item(key, value);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_storage_value(_key: &str, _value: &str) {}

#[cfg(target_arch = "wasm32")]
fn remove_storage_value(key: &str) {
    if let Some(storage) = session_storage() {
        let _ = storage.remove_item(key);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn remove_storage_value(_key: &str) {}

pub(crate) fn store_reset_email(email: &str) {
    write_storage_value(RESET_EMAIL_STORAGE_KEY, email);
}

pub(crate) fn read_reset_email() -> Option<String> {
    read_storage_value(RESET_EMAIL_STORAGE_KEY)
}

pub(crate) fn store_reset_ticket(ticket: &str) {
    write_storage_value(RESET_TICKET_STORAGE_KEY, ticket);
}

pub(crate) fn read_reset_ticket() -> Option<String> {
    read_storage_value(RESET_TICKET_STORAGE_KEY)
}

pub(crate) fn clear_reset_email() {
    remove_storage_value(RESET_EMAIL_STORAGE_KEY);
}

pub(crate) fn clear_reset_ticket() {
    remove_storage_value(RESET_TICKET_STORAGE_KEY);
}

pub(crate) fn clear_reset_flow_state() {
    clear_reset_email();
    clear_reset_ticket();
}
