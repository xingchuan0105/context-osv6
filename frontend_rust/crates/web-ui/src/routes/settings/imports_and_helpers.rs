// Settings page - billing, profile, security and notifications

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use leptos_router::NavigateOptions;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;
use web_sdk::ApiClient;
use web_sdk::dtos::{
    ChangePasswordRequest, NotificationPreferences, NotificationRow, UserPreferences,
};

use crate::api::api_base_url;
use crate::components::billing::BillingPanel;
use crate::components::common::{EmptyMessage, ErrorBanner, LoadingMessage};
use crate::components::{UnavailableFeatureCard, UsageLimitCard};
use crate::i18n::{Locale, choose};
use crate::load::run_once_after_hydration;
use crate::platform::ui_capabilities;
use crate::state::auth::use_auth_state;
use crate::state::ui_prefs::{Theme, use_ui_prefs_state};

#[derive(Clone, Copy, PartialEq)]
enum SettingsTab {
    Billing,
    Profile,
    Appearance,
    Security,
    Notifications,
}

#[derive(Clone)]
struct CurrentDeviceInfo {
    label: String,
    timezone: String,
}

#[cfg(target_arch = "wasm32")]
fn current_device_info(locale: Locale) -> CurrentDeviceInfo {
    let label = web_sys::window()
        .and_then(|window| window.navigator().user_agent().ok())
        .map(|ua| {
            if ua.contains("Mac OS X") {
                choose(
                    locale,
                    "当前设备：macOS 浏览器",
                    "Current device: macOS browser",
                )
            } else if ua.contains("Windows") {
                choose(
                    locale,
                    "当前设备：Windows 浏览器",
                    "Current device: Windows browser",
                )
            } else if ua.contains("Linux") {
                choose(
                    locale,
                    "当前设备：Linux 浏览器",
                    "Current device: Linux browser",
                )
            } else if ua.contains("Android") {
                choose(
                    locale,
                    "当前设备：Android 浏览器",
                    "Current device: Android browser",
                )
            } else if ua.contains("iPhone") || ua.contains("iPad") {
                choose(
                    locale,
                    "当前设备：iOS 浏览器",
                    "Current device: iOS browser",
                )
            } else {
                choose(
                    locale,
                    "当前设备：浏览器会话",
                    "Current device: browser session",
                )
            }
            .to_string()
        })
        .unwrap_or_else(|| {
            choose(
                locale,
                "当前设备：浏览器会话",
                "Current device: browser session",
            )
            .to_string()
        });
    let timezone = js_sys::Reflect::get(
        &js_sys::Intl::DateTimeFormat::new(&js_sys::Array::new(), &js_sys::Object::new())
            .resolved_options(),
        &wasm_bindgen::JsValue::from_str("timeZone"),
    )
    .ok()
    .and_then(|value| value.as_string())
    .unwrap_or_else(|| choose(locale, "未知时区", "Unknown timezone").to_string());
    CurrentDeviceInfo { label, timezone }
}

#[cfg(not(target_arch = "wasm32"))]
fn current_device_info(locale: Locale) -> CurrentDeviceInfo {
    CurrentDeviceInfo {
        label: choose(
            locale,
            "当前设备：服务器渲染会话",
            "Current device: server-rendered session",
        )
        .to_string(),
        timezone: "UTC".to_string(),
    }
}
