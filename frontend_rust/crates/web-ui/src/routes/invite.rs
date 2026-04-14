//! Invite page - Accept or decline notebook invitations

use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use web_sdk::ApiClient;

use crate::api::api_base_url;
use crate::i18n::choose;
use crate::load::run_once_after_hydration;
use crate::state::auth::use_auth_state;
use crate::state::ui_prefs::use_ui_prefs_state;

// ----------------------------------------------------------------------------
// InvitePage - Accept or decline a notebook invitation
// ----------------------------------------------------------------------------

#[component]
pub fn InvitePage() -> impl IntoView {
    // Get notebook_id and member_id from route params
    let params = use_params_map();
    let notebook_id = move || params.get().get("notebook_id").unwrap_or_default();
    let member_id = move || params.get().get("member_id").unwrap_or_default();

    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;

    // State
    let (loading, set_loading) = signal(true);
    let (action_loading, set_action_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (success, set_success) = signal(false);
    let (notebook_name, set_notebook_name) = signal(String::new());
    let (invite_status, set_invite_status) = signal(String::new());
    let (loaded_key, set_loaded_key) = signal(String::new());

    let auth_for_load = auth.clone();
    run_once_after_hydration(
        move || {
            auth_for_load
                .token
                .get()
                .map(|value| format!("{}:{}:{}", value, notebook_id(), member_id()))
                .unwrap_or_default()
        },
        loaded_key,
        set_loaded_key,
        move || {
            spawn(async move {
                let nid = notebook_id();
                let mid = member_id();
                let current_locale = locale.get_untracked();

                if nid.is_empty() || mid.is_empty() {
                    set_error.set(
                        choose(current_locale, "邀请链接无效", "Invalid invitation link")
                            .to_string(),
                    );
                    set_loading.set(false);
                    return;
                }

                let Some(token) = auth.token.get() else {
                    set_loading.set(false);
                    return;
                };

                let client = ApiClient::new(api_base_url()).with_auth(token);

                match client.get_notebook(&nid).await {
                    Ok(resp) => {
                        set_notebook_name.set(resp.notebook.title.clone());
                    }
                    Err(_) => {
                        set_notebook_name.set(
                            choose(current_locale, "未知知识库", "Unknown notebook").to_string(),
                        );
                    }
                }

                set_invite_status.set("pending".to_string());
                set_loading.set(false);
            });
        },
    );

    let handle_accept = move |_| {
        let nid = notebook_id();
        let mid = member_id();
        let current_locale = locale.get_untracked();

        if nid.is_empty() || mid.is_empty() {
            set_error.set(choose(current_locale, "邀请信息无效", "Invalid invitation").to_string());
            return;
        }

        let token = auth.token.get();
        if token.is_none() {
            set_error.set(
                choose(
                    current_locale,
                    "请先登录后再接受邀请",
                    "Please log in to accept the invitation",
                )
                .to_string(),
            );
            return;
        }

        set_action_loading.set(true);
        set_error.set(String::new());

        let client = ApiClient::new(api_base_url()).with_auth(token.unwrap());

        spawn(async move {
            match client.accept_invite(&nid, &mid).await {
                Ok(_) => {
                    set_success.set(true);
                    set_invite_status.set("accepted".to_string());
                }
                Err(e) => {
                    set_error.set(format!(
                        "{}: {}",
                        choose(
                            current_locale,
                            "接受邀请失败",
                            "Failed to accept invitation"
                        ),
                        e
                    ));
                }
            }
            set_action_loading.set(false);
        });
    };

    let handle_decline = move |_| {
        let nid = notebook_id();
        let mid = member_id();
        let current_locale = locale.get_untracked();

        if nid.is_empty() || mid.is_empty() {
            set_error.set(choose(current_locale, "邀请信息无效", "Invalid invitation").to_string());
            return;
        }

        let token = auth.token.get();
        if token.is_none() {
            set_error.set(
                choose(
                    current_locale,
                    "请先登录后再拒绝邀请",
                    "Please log in to decline the invitation",
                )
                .to_string(),
            );
            return;
        }

        set_action_loading.set(true);
        set_error.set(String::new());

        let client = ApiClient::new(api_base_url()).with_auth(token.unwrap());

        spawn(async move {
            match client.decline_invite(&nid, &mid).await {
                Ok(_) => {
                    set_success.set(true);
                    set_invite_status.set("declined".to_string());
                }
                Err(e) => {
                    set_error.set(format!(
                        "{}: {}",
                        choose(
                            current_locale,
                            "拒绝邀请失败",
                            "Failed to decline invitation"
                        ),
                        e
                    ));
                }
            }
            set_action_loading.set(false);
        });
    };

    // Helper for showing accepted state
    let show_accepted =
        move || !loading.get() && success.get() && invite_status.get() == "accepted";

    // Helper for showing declined state
    let show_declined =
        move || !loading.get() && success.get() && invite_status.get() == "declined";

    // Helper for showing pending state
    let show_pending = move || !loading.get() && error.get().is_empty() && !success.get();

    view! {
        <div class="min-h-screen flex items-center justify-center bg-gray-50">
            <div class="bg-white rounded-lg shadow-md w-full max-w-md p-8">
                {/* Loading */}
                <Show when=move || loading.get()>
                    <div class="text-center">
                        <div class="text-gray-500">
                            {move || choose(locale.get(), "正在加载邀请...", "Loading invitation...")}
                        </div>
                    </div>
                </Show>

                {/* Error */}
                <Show when=move || !loading.get() && !error.get().is_empty()>
                    <div class="text-center">
                        <div class="text-red-600 mb-4">{error.get()}</div>
                        <A href="/" attr:class="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 inline-block">
                            {move || choose(locale.get(), "返回首页", "Go to Home")}
                        </A>
                    </div>
                </Show>

                {/* Success - Accepted */}
                <Show when=show_accepted>
                    <div class="text-center">
                        <div class="mb-4">
                            <svg class="w-16 h-16 mx-auto text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/>
                            </svg>
                        </div>
                        <h2 class="text-xl font-semibold text-gray-900 mb-2">
                            {move || choose(locale.get(), "已接受邀请", "Invitation Accepted!")}
                        </h2>
                        <p class="text-gray-600 mb-6">
                            {move || choose(locale.get(), "你现在可以访问：", "You now have access to ")}
                            {notebook_name.get()}
                        </p>
                        <A href="/dashboard" attr:class="block w-full px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 text-center">
                            {move || choose(locale.get(), "前往控制台", "Go to Dashboard")}
                        </A>
                    </div>
                </Show>

                {/* Success - Declined */}
                <Show when=show_declined>
                    <div class="text-center">
                        <div class="mb-4">
                            <svg class="w-16 h-16 mx-auto text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
                            </svg>
                        </div>
                        <h2 class="text-xl font-semibold text-gray-900 mb-2">
                            {move || choose(locale.get(), "已拒绝邀请", "Invitation Declined")}
                        </h2>
                        <p class="text-gray-600 mb-6">
                            {move || choose(locale.get(), "你已拒绝加入：", "You have declined the invitation to ")}
                            {notebook_name.get()}
                        </p>
                        <A href="/" attr:class="block w-full px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 text-center">
                            {move || choose(locale.get(), "返回首页", "Go to Home")}
                        </A>
                    </div>
                </Show>

                {/* Pending invitation */}
                <Show when=show_pending>
                    <div class="text-center">
                        <div class="mb-6">
                            <svg class="w-16 h-16 mx-auto text-blue-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/>
                            </svg>
                        </div>
                        <h2 class="text-xl font-semibold text-gray-900 mb-2">
                            {move || choose(locale.get(), "你收到了一条协作邀请", "You've been invited!")}
                        </h2>
                        <p class="text-gray-600 mb-6">
                            {move || choose(locale.get(), "有人邀请你协作这个知识库：", "You have been invited to collaborate on:")}
                        </p>
                        <div class="bg-gray-50 rounded-lg p-4 mb-6">
                            <p class="font-medium text-gray-900">{notebook_name.get()}</p>
                        </div>
                        <p class="text-sm text-gray-500 mb-6">
                            {move || choose(locale.get(), "知识库 ID：", "Notebook ID: ")}
                            {notebook_id()}
                        </p>

                        <div class="flex gap-3">
                            <button
                                class="flex-1 px-4 py-2 border border-gray-300 rounded hover:bg-gray-50 text-gray-700"
                                disabled=action_loading.get()
                                on:click=handle_decline
                            >
                                {move || choose(locale.get(), "拒绝", "Decline")}
                            </button>
                            <button
                                class="flex-1 px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
                                disabled=action_loading.get()
                                on:click=handle_accept
                            >
                                {move || {
                                    if action_loading.get() {
                                        choose(locale.get(), "处理中...", "Processing...")
                                    } else {
                                        choose(locale.get(), "接受邀请", "Accept")
                                    }
                                }}
                            </button>
                        </div>
                    </div>
                </Show>
            </div>
        </div>
    }
}
