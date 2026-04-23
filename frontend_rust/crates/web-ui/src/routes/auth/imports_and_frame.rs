// Auth pages - Login, Register, and Password Reset flows

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use leptos_router::NavigateOptions;
use leptos_router::components::A;
use leptos_router::hooks::{use_location, use_navigate};
use web_sdk::ApiClient;
use web_sdk::dtos::{
    ConfirmResetPasswordRequest, LoginRequest, RegisterRequest, SendResetCodeRequest,
    VerifyResetCodeRequest,
};

use crate::api::api_base_url;
use crate::auth_support::{
    clear_reset_flow_state, describe_auth_error, read_reset_email, read_reset_ticket,
    store_reset_email, store_reset_ticket, use_password_reset_enabled,
};
use crate::components::{
    ContextOsMark, LocaleToggle, NoticeBanner, NoticeTone, SectionCard, UnavailableFeatureCard,
};
use crate::i18n::{MessageKey, t};
use crate::state::auth::use_auth_state;
use crate::state::ui_prefs::use_ui_prefs_state;

/// Helper to create an unauthenticated API client.
fn api_client() -> ApiClient {
    ApiClient::new(api_base_url())
}

fn scoped_auth_path(current_path: &str, route: &str) -> String {
    if current_path.starts_with("/preview/live") {
        format!("/preview/live{route}")
    } else {
        route.to_string()
    }
}

#[component]
fn AuthFrame(children: Children) -> impl IntoView {
    view! {
        <div class="app-auth-shell">
            <div class="w-full max-w-md space-y-4">
                <div class="flex justify-end">
                    <LocaleToggle />
                </div>
                <SectionCard>{children()}</SectionCard>
            </div>
        </div>
    }
}

// ----------------------------------------------------------------------------
// LoginPage
// ----------------------------------------------------------------------------
