//! Notebook API access page

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use leptos_router::components::A;
use leptos_router::hooks::{use_location, use_params_map};
use web_sdk::ApiClient;
use web_sdk::dtos::{ApiKeyRow, CreateApiKeyRequest};

use crate::api::api_base_url;
use crate::components::common::ErrorBanner;
use crate::i18n::{Locale, choose};
use crate::load::run_once_after_hydration;
use crate::state::auth::use_auth_state;
use crate::state::ui_prefs::use_ui_prefs_state;

fn api_permission_label(locale: Locale, permission: &str) -> String {
    match permission {
        "index" => choose(locale, "索引", "Index").to_string(),
        "admin" => choose(locale, "管理员", "Admin").to_string(),
        _ => permission.to_string(),
    }
}

fn api_key_status_label(locale: Locale, is_active: bool) -> &'static str {
    if is_active {
        choose(locale, "生效中", "active")
    } else {
        choose(locale, "已撤销", "revoked")
    }
}

#[component]
pub fn ApiAccessPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let params = use_params_map();
    let notebook_id = move || params.get().get("notebook_id").unwrap_or_default();
    let location = use_location();
    let is_preview_route = Memo::new(move |_| location.pathname.get().starts_with("/preview/live"));
    let workspace_href = Memo::new(move |_| {
        let nid = notebook_id();
        if nid.is_empty() {
            if is_preview_route.get() {
                "/preview/live/dashboard".to_string()
            } else {
                "/dashboard".to_string()
            }
        } else if is_preview_route.get() {
            format!("/preview/live/workspace/{nid}")
        } else {
            format!("/dashboard/{nid}")
        }
    });

    let (keys, set_keys) = signal(Vec::<ApiKeyRow>::new());
    let (loaded_key, set_loaded_key) = signal(String::new());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (name, set_name) = signal(String::new());
    let (permission, set_permission) = signal("index".to_string());
    let (rate_limit, set_rate_limit) = signal("60".to_string());
    let (expires_at, set_expires_at) = signal(String::new());
    let (plain_key, set_plain_key) = signal(String::new());

    let fetch_keys = move || {
        let nid = notebook_id();
        let Some(token) = auth.token.get() else {
            return;
        };
        if nid.is_empty() {
            return;
        }
        set_loading.set(true);
        set_error.set(String::new());
        let client = ApiClient::new(api_base_url()).with_auth(token);
        let current_locale = locale.get_untracked();
        spawn(async move {
            match client.list_api_keys(&nid).await {
                Ok(resp) => set_keys.set(resp.api_keys),
                Err(error) => set_error.set(format!(
                    "{}: {}",
                    choose(
                        current_locale,
                        "加载 API 密钥失败",
                        "Failed to load API keys"
                    ),
                    error
                )),
            }
            set_loading.set(false);
        });
    };

    let auth_for_load = auth.clone();
    let fetch_keys_on_mount = fetch_keys.clone();
    run_once_after_hydration(
        move || {
            auth_for_load
                .token
                .get()
                .map(|value| format!("{}:{}", value, notebook_id()))
                .unwrap_or_default()
        },
        loaded_key,
        set_loaded_key,
        move || fetch_keys_on_mount(),
    );

    let handle_create = move |ev: SubmitEvent| {
        ev.prevent_default();
        let nid = notebook_id();
        let current_locale = locale.get_untracked();
        let Some(token) = auth.token.get() else {
            set_error.set(choose(current_locale, "尚未登录", "Not authenticated").to_string());
            return;
        };
        if nid.is_empty() || name.get().trim().is_empty() {
            set_error
                .set(choose(current_locale, "请输入密钥名称", "Key name is required").to_string());
            return;
        }
        let client = ApiClient::new(api_base_url()).with_auth(token);
        let req = CreateApiKeyRequest {
            name: name.get(),
            permissions: vec![permission.get()],
            rate_limit_rpm: rate_limit.get().parse::<u32>().ok(),
            expires_at: (!expires_at.get().trim().is_empty()).then(|| expires_at.get()),
        };
        spawn(async move {
            match client.create_api_key(&nid, &req).await {
                Ok(resp) => {
                    set_plain_key.set(resp.plaintext_key.clone());
                    set_keys.update(|items: &mut Vec<ApiKeyRow>| items.insert(0, resp.api_key));
                    set_name.set(String::new());
                    set_expires_at.set(String::new());
                }
                Err(error) => {
                    set_error.set(format!(
                        "{}: {}",
                        choose(current_locale, "创建密钥失败", "Failed to create key"),
                        error
                    ));
                }
            }
        });
    };

    view! {
        <div class="app-page-shell">
            <div class="mx-auto max-w-5xl space-y-6">
                <div class="flex items-start justify-between gap-4">
                    <div class="app-page-heading mb-0">
                        <h1 class="app-page-title">
                            {move || choose(locale.get(), "API 访问", "API Access")}
                        </h1>
                        <p class="app-page-subtitle">
                            {move || {
                                choose(
                                    locale.get(),
                                    "管理这个 Workspace 的 API 密钥，用于工作区资料管理和 RAG 查询。聊天与全局搜索暂不对外开放。",
                                    "Manage workspace API keys for source management and RAG queries. Chat and search are not exposed via API.",
                                )
                            }}
                        </p>
                    </div>
                    <A href=move || workspace_href.get() attr:class="app-link">
                        {move || choose(locale.get(), "返回工作台", "Back to Workspace")}
                    </A>
                </div>

                <Show when=move || !error.get().is_empty()>
                    <ErrorBanner message={error.get()} />
                </Show>

                <div class="grid gap-6 lg:grid-cols-[1fr_1fr]">
                    <div class="app-surface-card">
                        <h2 class="mb-4 text-lg font-semibold text-card-foreground">
                            {move || choose(locale.get(), "创建 API 密钥", "Create API Key")}
                        </h2>
                        <form on:submit=handle_create class="space-y-4">
                            <input
                                type="text"
                                class="app-input"
                                placeholder={move || choose(locale.get(), "密钥名称", "Key name")}
                                value=move || name.get()
                                on:input=move |ev| set_name.set(event_target_value(&ev))
                            />
                            <select class="app-input" on:change=move |ev| set_permission.set(event_target_value(&ev))>
                                <option value="index" selected={move || permission.get() == "index"}>
                                    {move || api_permission_label(locale.get(), "index")}
                                </option>
                                <option value="admin" selected={move || permission.get() == "admin"}>
                                    {move || api_permission_label(locale.get(), "admin")}
                                </option>
                            </select>
                            <p class="text-xs text-muted-foreground">
                                {move || {
                                    choose(
                                        locale.get(),
                                        "API 密钥仅支持资料管理与 RAG 查询，聊天和搜索代理默认不可用。",
                                        "Chat and search agents are intentionally unavailable via API keys.",
                                    )
                                }}
                            </p>
                            <input
                                type="number"
                                class="app-input"
                                placeholder={move || choose(locale.get(), "速率限制（RPM）", "Rate limit RPM")}
                                value=move || rate_limit.get()
                                on:input=move |ev| set_rate_limit.set(event_target_value(&ev))
                            />
                            <input
                                type="text"
                                class="app-input"
                                placeholder={move || choose(locale.get(), "过期时间 RFC3339（可选）", "Expires at RFC3339 (optional)")}
                                value=move || expires_at.get()
                                on:input=move |ev| set_expires_at.set(event_target_value(&ev))
                            />
                            <p class="text-xs text-muted-foreground">
                                {move || choose(locale.get(), "示例：2026-03-31T18:00:00Z", "Example: 2026-03-31T18:00:00Z")}
                            </p>
                            <button type="submit" class="app-button-primary">
                                {move || choose(locale.get(), "创建密钥", "Create Key")}
                            </button>
                        </form>

                        <Show when=move || !plain_key.get().is_empty()>
                            <div class="mt-4 rounded-xl border border-green-200 bg-green-50 p-3">
                                <div class="text-sm font-medium text-green-800">
                                    {move || choose(locale.get(), "新密钥", "New Key")}
                                </div>
                                <pre class="mt-2 whitespace-pre-wrap break-all text-xs text-green-900">{plain_key.get()}</pre>
                            </div>
                        </Show>
                    </div>

                    <div class="app-surface-card">
                        <h2 class="mb-4 text-lg font-semibold text-card-foreground">
                            {move || choose(locale.get(), "接入示例", "Integration Snippets")}
                        </h2>
                        <div class="space-y-4">
                            <div class="rounded-xl border border-amber-200 bg-amber-50 p-3 text-sm text-amber-900">
                                {move || {
                                    choose(
                                        locale.get(),
                                        "Workspace API 密钥支持工作区资料管理和 RAG 查询；聊天与搜索代理不通过 API 暴露。",
                                        "Workspace API keys support source management and RAG queries. Chat and search agents are disabled for API access.",
                                    )
                                }}
                            </div>
                            <div>
                                <div class="mb-1 text-sm font-medium text-card-foreground">
                                    {move || choose(locale.get(), "RAG 查询", "RAG Query")}
                                </div>
                                <pre class="app-code-block">{format!("curl -X POST {}/api/v1/notebooks/{}/query -H 'Authorization: Bearer <KEY>' -H 'Content-Type: application/json' -d '{{\"query\":\"What is Rust?\"}}'", api_base_url(), notebook_id())}</pre>
                            </div>
                            <div>
                                <div class="mb-1 text-sm font-medium text-card-foreground">
                                    {move || choose(locale.get(), "创建上传任务", "Create Upload")}
                                </div>
                                <pre class="app-code-block">{format!("curl -X POST {}/api/v1/notebooks/{}/documents -H 'Authorization: Bearer <KEY>' -H 'Content-Type: application/json' -d '{{\"filename\":\"notes.pdf\",\"file_size\":12345,\"mime_type\":\"application/pdf\"}}'", api_base_url(), notebook_id())}</pre>
                            </div>
                            <div>
                                <div class="mb-1 text-sm font-medium text-card-foreground">
                                    {move || choose(locale.get(), "添加 URL 资料", "Add URL Source")}
                                </div>
                                <pre class="app-code-block">{format!("curl -X POST {}/api/v1/notebooks/{}/sources/url -H 'Authorization: Bearer <KEY>' -H 'Content-Type: application/json' -d '{{\"url\":\"https://example.com/reference\"}}'", api_base_url(), notebook_id())}</pre>
                            </div>
                        </div>
                    </div>
                </div>

                <div class="app-surface-card">
                    <div class="mb-4 flex items-center justify-between">
                        <h2 class="text-lg font-semibold text-card-foreground">
                            {move || choose(locale.get(), "已创建密钥", "Active Keys")}
                        </h2>
                        <Show when=move || loading.get()>
                            <span class="text-sm text-muted-foreground">
                                {move || choose(locale.get(), "加载中...", "Loading...")}
                            </span>
                        </Show>
                    </div>
                    <Show when=move || keys.get().is_empty() && !loading.get()>
                        <div class="app-empty-state">
                            {move || choose(locale.get(), "还没有 API 密钥", "No API keys yet")}
                        </div>
                    </Show>
                    <div class="space-y-3">
                        {keys.get().into_iter().map(|key| {
                            let key_id = key.id.clone();
                            let auth = auth.clone();
                            let nid = notebook_id();
                            let key_name = key.name.clone();
                            let key_prefix = key.key_prefix.clone();
                            let permissions = key.permissions.clone();
                            let rate_limit_rpm = key.rate_limit_rpm;
                            let expires_at = key.expires_at.clone();
                            let last_used_at = key.last_used_at.clone();
                            let is_active = key.is_active;
                            view! {
                                <div class="flex items-center justify-between gap-4 rounded-xl border border-border bg-card px-4 py-3">
                                    <div class="min-w-0">
                                        <div class="text-sm font-medium text-card-foreground">{key_name}</div>
                                        <div class="text-xs text-muted-foreground">
                                            {key_prefix}
                                            {" · "}
                                            {move || {
                                                permissions
                                                    .iter()
                                                    .map(|permission| api_permission_label(locale.get(), permission))
                                                    .collect::<Vec<_>>()
                                                    .join(" / ")
                                            }}
                                            {" · "}
                                            {move || format!("{} RPM", rate_limit_rpm)}
                                        </div>
                                        <div class="mt-1 text-xs text-muted-foreground">
                                            {move || {
                                                format!(
                                                    "{} · {} {} · {} {}",
                                                    api_key_status_label(locale.get(), is_active),
                                                    choose(locale.get(), "过期时间", "Expires"),
                                                    expires_at.clone().unwrap_or_else(|| choose(locale.get(), "永不", "never").to_string()),
                                                    choose(locale.get(), "最近使用", "Last used"),
                                                    last_used_at.clone().unwrap_or_else(|| choose(locale.get(), "从未", "never").to_string()),
                                                )
                                            }}
                                        </div>
                                    </div>
                                    <button
                                        class="app-button-danger px-3 py-1.5 text-xs"
                                        on:click=move |_| {
                                            if let Some(token) = auth.token.get() {
                                                let current_locale = locale.get_untracked();
                                                let client = ApiClient::new(api_base_url()).with_auth(token);
                                                let key_id = key_id.clone();
                                                let nid = nid.clone();
                                                spawn(async move {
                                                    match client.delete_api_key(&nid, &key_id).await {
                                                        Ok(_) => fetch_keys(),
                                                        Err(error) => set_error.set(format!(
                                                            "{}: {}",
                                                            choose(current_locale, "撤销密钥失败", "Failed to revoke key"),
                                                            error
                                                        )),
                                                    }
                                                });
                                            }
                                        }
                                    >
                                        {move || choose(locale.get(), "撤销", "Revoke")}
                                    </button>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                </div>
            </div>
        </div>
    }
}
