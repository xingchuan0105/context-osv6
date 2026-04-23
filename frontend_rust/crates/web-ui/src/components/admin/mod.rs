//! Admin components

use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use leptos_router::components::A;
use web_sdk::ApiClient;
use web_sdk::dtos::{AdminUsageResponse, HealthResponse, OrgRow, UserRow};

use crate::api::api_base_url;
use crate::components::common::ErrorBanner;
use crate::i18n::{Locale, choose};
use crate::state::auth::use_auth_state;
use crate::state::ui_prefs::use_ui_prefs_state;

/// Format date helper - extracts YYYY-MM-DD from ISO string
fn format_date(iso_string: &str) -> String {
    if iso_string.len() >= 10 {
        iso_string[..10].to_string()
    } else {
        iso_string.to_string()
    }
}

/// Helper for number formatting
fn format_number(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn plan_label(locale: Locale, plan: &str) -> String {
    match plan {
        "" | "N/A" | "n/a" | "unknown" => choose(locale, "未配置", "Unset").to_string(),
        "free" => choose(locale, "免费版", "Free").to_string(),
        "starter" => choose(locale, "入门版", "Starter").to_string(),
        "pro" => choose(locale, "专业版", "Pro").to_string(),
        "team" => choose(locale, "团队版", "Team").to_string(),
        "enterprise" => choose(locale, "企业版", "Enterprise").to_string(),
        _ => plan.to_string(),
    }
}

fn user_role_label(locale: Locale, role: &str) -> String {
    match role {
        "owner" => choose(locale, "所有者", "Owner").to_string(),
        "admin" => choose(locale, "管理员", "Admin").to_string(),
        "member" => choose(locale, "成员", "Member").to_string(),
        "viewer" => choose(locale, "查看者", "Viewer").to_string(),
        "editor" => choose(locale, "编辑者", "Editor").to_string(),
        _ => role.to_string(),
    }
}

fn org_status_label(locale: Locale, blocked: bool) -> &'static str {
    if blocked {
        choose(locale, "已封禁", "Blocked")
    } else {
        choose(locale, "正常", "Active")
    }
}

fn health_status_label(locale: Locale, status: &str) -> String {
    match status {
        "ok" | "healthy" | "ready" => choose(locale, "健康", "Healthy").to_string(),
        "degraded" => choose(locale, "降级中", "Degraded").to_string(),
        "error" | "failed" | "unhealthy" => choose(locale, "异常", "Unhealthy").to_string(),
        _ => status.to_string(),
    }
}

fn metric_tone_classes(tone: &str) -> (&'static str, &'static str) {
    match tone {
        "success" => ("bg-emerald-500", "text-emerald-700"),
        "warning" => ("bg-amber-500", "text-amber-700"),
        "danger" => ("bg-rose-500", "text-rose-700"),
        _ => ("bg-sky-500", "text-slate-900"),
    }
}

#[component]
pub fn AdminMetricCard(
    #[prop(into)] label: Signal<String>,
    #[prop(into)] value: Signal<String>,
    tone: &'static str,
    #[prop(optional, into)] detail: Option<Signal<String>>,
) -> impl IntoView {
    let (dot_class, value_class) = metric_tone_classes(tone);

    view! {
        <div class="rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
            <div class="flex items-center gap-2 text-xs font-medium text-slate-500">
                <span class={format!("h-2.5 w-2.5 rounded-full {}", dot_class)}></span>
                <span>{move || label.get()}</span>
            </div>
            <div class={format!("mt-3 text-2xl font-semibold {}", value_class)}>{move || value.get()}</div>
            {move || {
                detail
                    .as_ref()
                    .map(|text| text.get())
                    .filter(|text| !text.is_empty())
                    .map(|text| view! { <p class="mt-2 text-xs text-slate-500">{text}</p> })
            }}
        </div>
    }
}

// ----------------------------------------------------------------------------
// OrgListTable component - displays organizations in a table
// ----------------------------------------------------------------------------

#[component]
pub fn OrgListTable(
    #[prop(into)] orgs: Signal<Vec<OrgRow>>,
    set_orgs: WriteSignal<Vec<OrgRow>>,
) -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let (error, set_error) = signal(String::new());
    let (busy_org_id, set_busy_org_id) = signal(String::new());

    let handle_block = move |org_id: String, blocked: bool| {
        let Some(token) = auth.token.get() else {
            set_error
                .set(choose(locale.get_untracked(), "尚未登录", "Not authenticated").to_string());
            return;
        };
        set_error.set(String::new());
        set_busy_org_id.set(org_id.clone());
        let client = ApiClient::new(api_base_url()).with_auth(token);
        let org_id_for_update = org_id.clone();
        let current_locale = locale.get_untracked();
        spawn(async move {
            match client.block_org(&org_id, blocked).await {
                Ok(_) => {
                    set_orgs.update(|items| {
                        if let Some(row) =
                            items.iter_mut().find(|item| item.id == org_id_for_update)
                        {
                            row.blocked = blocked;
                        }
                    });
                }
                Err(error) => {
                    set_error.set(format!(
                        "{}: {}",
                        choose(
                            current_locale,
                            "更新组织状态失败",
                            "Failed to update organization status"
                        ),
                        error
                    ));
                }
            }
            set_busy_org_id.set(String::new());
        });
    };

    view! {
        <div class="space-y-4">
            <Show when=move || !error.get().is_empty()>
                <ErrorBanner message={error.get()} />
            </Show>
            <div class="overflow-x-auto">
                <table class="min-w-full divide-y divide-gray-200">
                    <thead class="bg-gray-50">
                        <tr>
                            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                {move || choose(locale.get(), "名称", "Name")}
                            </th>
                            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                {move || choose(locale.get(), "方案", "Plan")}
                            </th>
                            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                {move || choose(locale.get(), "用户数", "Users")}
                            </th>
                            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                {move || choose(locale.get(), "知识库数", "Notebooks")}
                            </th>
                            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                {move || choose(locale.get(), "查询数", "Queries")}
                            </th>
                            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                {move || choose(locale.get(), "状态", "Status")}
                            </th>
                            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                                {move || choose(locale.get(), "操作", "Actions")}
                            </th>
                        </tr>
                    </thead>
                    <tbody class="bg-white divide-y divide-gray-200">
                        {orgs.get().into_iter().map(|org| {
                            let org_id = org.id.clone();
                            let org_id_display = org.id.chars().take(8).collect::<String>();
                            let is_blocked = org.blocked;
                            let row_href = format!("/admin/organizations/{}", org.id);
                            let disabled_org_id = org_id.clone();
                            let click_org_id = org_id.clone();
                            view! {
                                <tr class="hover:bg-gray-50">
                                    <td class="px-6 py-4 whitespace-nowrap">
                                        <A href={row_href} attr:class="text-sm font-medium text-gray-900 hover:text-blue-600">
                                            {org.name.clone()}
                                        </A>
                                        <div class="text-xs text-gray-500">
                                            {move || format!("{}: {}...", choose(locale.get(), "ID", "ID"), org_id_display)}
                                        </div>
                                    </td>
                                    <td class="px-6 py-4 whitespace-nowrap">
                                        <div class="text-sm text-gray-900">{plan_label(locale.get(), &org.plan)}</div>
                                    </td>
                                    <td class="px-6 py-4 whitespace-nowrap">
                                        <div class="text-sm text-gray-900">{org.user_count}</div>
                                    </td>
                                    <td class="px-6 py-4 whitespace-nowrap">
                                        <div class="text-sm text-gray-900">{org.notebook_count}</div>
                                    </td>
                                    <td class="px-6 py-4 whitespace-nowrap">
                                        <div class="text-sm text-gray-900">{org.query_count}</div>
                                    </td>
                                    <td class="px-6 py-4 whitespace-nowrap">
                                        {if is_blocked {
                                            view! { <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-800">{org_status_label(locale.get(), true)}</span> }
                                        } else {
                                            view! { <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">{org_status_label(locale.get(), false)}</span> }
                                        }}
                                    </td>
                                    <td class="px-6 py-4 whitespace-nowrap text-sm">
                                        <button
                                            class="text-red-600 hover:text-red-900 disabled:opacity-50"
                                            disabled=move || busy_org_id.get() == disabled_org_id
                                            on:click=move |_| {
                                                handle_block(click_org_id.clone(), !is_blocked);
                                            }
                                        >
                                            {move || {
                                                if is_blocked {
                                                    choose(locale.get(), "解除封禁", "Unblock")
                                                } else {
                                                    choose(locale.get(), "封禁", "Block")
                                                }
                                            }}
                                        </button>
                                    </td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
        </div>
    }
}

// ----------------------------------------------------------------------------
// OrgDetailPanel component - shows org details
// ----------------------------------------------------------------------------

#[component]
pub fn OrgDetailPanel(org: OrgRow) -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let (org_state, set_org_state) = signal(org);
    let (action_loading, set_action_loading) = signal(false);
    let (action_error, set_action_error) = signal(String::new());

    let handle_block = move |_| {
        let Some(token) = auth.token.get() else {
            set_action_error
                .set(choose(locale.get_untracked(), "尚未登录", "Not authenticated").to_string());
            return;
        };
        let current_org = org_state.get_untracked();
        let org_id = current_org.id.clone();
        let next_blocked = !current_org.blocked;
        let current_locale = locale.get_untracked();
        set_action_loading.set(true);
        set_action_error.set(String::new());
        let client = ApiClient::new(api_base_url()).with_auth(token);
        spawn(async move {
            match client.block_org(&org_id, next_blocked).await {
                Ok(_) => set_org_state.update(|org| org.blocked = next_blocked),
                Err(error) => {
                    set_action_error.set(format!(
                        "{}: {}",
                        choose(
                            current_locale,
                            "更新组织状态失败",
                            "Failed to update organization status"
                        ),
                        error
                    ));
                }
            }
            set_action_loading.set(false);
        });
    };

    view! {
        <div class="space-y-6">
            <Show when=move || !action_error.get().is_empty()>
                <ErrorBanner message={action_error.get()} />
            </Show>
            {/* Org info card */}
            <div class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
                <div class="flex items-center justify-between mb-4">
                    <h3 class="text-lg font-semibold text-gray-900">
                        {move || choose(locale.get(), "组织详情", "Organization Details")}
                    </h3>
                    <Show
                        when=move || !org_state.get().blocked
                        fallback=move || view! {
                            <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-800">
                                {move || org_status_label(locale.get(), true)}
                            </span>
                        }
                    >
                        <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">
                            {move || org_status_label(locale.get(), false)}
                        </span>
                    </Show>
                </div>

                <dl class="space-y-3">
                    <div class="flex items-center justify-between">
                        <dt class="text-sm font-medium text-gray-500">{move || choose(locale.get(), "名称", "Name")}</dt>
                        <dd class="text-sm text-gray-900">{move || org_state.get().name.clone()}</dd>
                    </div>
                    <div class="flex items-center justify-between">
                        <dt class="text-sm font-medium text-gray-500">{move || choose(locale.get(), "方案", "Plan")}</dt>
                        <dd class="text-sm text-gray-900">{move || plan_label(locale.get(), &org_state.get().plan)}</dd>
                    </div>
                    <div class="flex items-center justify-between">
                        <dt class="text-sm font-medium text-gray-500">{move || choose(locale.get(), "用户数", "User Count")}</dt>
                        <dd class="text-sm text-gray-900">{move || org_state.get().user_count}</dd>
                    </div>
                    <div class="flex items-center justify-between">
                        <dt class="text-sm font-medium text-gray-500">{move || choose(locale.get(), "知识库数", "Notebook Count")}</dt>
                        <dd class="text-sm text-gray-900">{move || org_state.get().notebook_count}</dd>
                    </div>
                    <div class="flex items-center justify-between">
                        <dt class="text-sm font-medium text-gray-500">{move || choose(locale.get(), "创建时间", "Created At")}</dt>
                        <dd class="text-sm text-gray-900">{move || format_date(&org_state.get().created_at)}</dd>
                    </div>
                </dl>

                <div class="mt-6 pt-4 border-t border-gray-200">
                    <button
                        class="px-4 py-2 bg-red-600 text-white text-sm font-medium rounded hover:bg-red-700 disabled:opacity-50"
                        disabled=move || action_loading.get()
                        on:click=handle_block
                    >
                        {move || {
                            if action_loading.get() {
                                choose(locale.get(), "处理中...", "Processing...")
                            } else if org_state.get().blocked {
                                choose(locale.get(), "解除封禁组织", "Unblock Organization")
                            } else {
                                choose(locale.get(), "封禁组织", "Block Organization")
                            }
                        }}
                    </button>
                </div>
            </div>

            {/* Usage summary */}
            <div class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
                <h3 class="text-lg font-semibold text-gray-900 mb-4">
                    {move || choose(locale.get(), "使用概览", "Usage Summary")}
                </h3>
                <div class="grid grid-cols-3 gap-4 text-center">
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "用户", "Users").to_string())
                        value=Signal::derive(move || org_state.get().user_count.to_string())
                        tone="primary"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "知识库", "Notebooks").to_string())
                        value=Signal::derive(move || org_state.get().notebook_count.to_string())
                        tone="success"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "查询", "Queries").to_string())
                        value=Signal::derive(move || org_state.get().query_count.to_string())
                        tone="warning"
                    />
                </div>
            </div>
        </div>
    }
}

// ----------------------------------------------------------------------------
// UserListTable component - displays users in a table
// ----------------------------------------------------------------------------

#[component]
pub fn UserListTable(#[prop(into)] users: Signal<Vec<UserRow>>) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;

    view! {
        <div class="overflow-x-auto">
            <table class="min-w-full divide-y divide-gray-200">
                <thead class="bg-gray-50">
                    <tr>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                            {move || choose(locale.get(), "邮箱", "Email")}
                        </th>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                            {move || choose(locale.get(), "姓名", "Name")}
                        </th>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                            {move || choose(locale.get(), "组织 ID", "Org ID")}
                        </th>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                            {move || choose(locale.get(), "角色", "Role")}
                        </th>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                            {move || choose(locale.get(), "创建时间", "Created")}
                        </th>
                        <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                            {move || choose(locale.get(), "最近活跃", "Last Active")}
                        </th>
                    </tr>
                </thead>
                <tbody class="bg-white divide-y divide-gray-200">
                    {users.get().into_iter().map(|user| {
                        let org_id_display = user.org_id[..8].to_string();
                        view! {
                            <tr class="hover:bg-gray-50">
                                <td class="px-6 py-4 whitespace-nowrap">
                                    <div class="text-sm font-medium text-gray-900">{user.email.clone()}</div>
                                </td>
                                <td class="px-6 py-4 whitespace-nowrap">
                                    <div class="text-sm text-gray-900">{user.full_name.clone()}</div>
                                </td>
                                <td class="px-6 py-4 whitespace-nowrap">
                                    <div class="text-xs text-gray-500">{org_id_display}...</div>
                                </td>
                                <td class="px-6 py-4 whitespace-nowrap">
                                    <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-800">
                                        {user_role_label(locale.get(), &user.role)}
                                    </span>
                                </td>
                                <td class="px-6 py-4 whitespace-nowrap">
                                    <div class="text-sm text-gray-500">{format_date(&user.created_at)}</div>
                                </td>
                                <td class="px-6 py-4 whitespace-nowrap">
                                    <div class="text-sm text-gray-500">
                                        {user.last_active_at.as_ref().map(|d| format_date(d)).unwrap_or_else(|| choose(locale.get(), "从未", "Never").to_string())}
                                    </div>
                                </td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

// ----------------------------------------------------------------------------
// UsageChart component - shows platform-wide usage
// ----------------------------------------------------------------------------

#[component]
pub fn UsageChart(usage: Option<AdminUsageResponse>) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    // Clone usage for display closures
    let usage_display = usage.clone();
    let usage_requests = usage.clone();
    let usage_tokens = usage.clone();
    let usage_documents = usage.clone();
    let has_usage = usage.is_some();

    view! {
        <div class="space-y-6">
            {/* Summary cards */}
            <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                <AdminMetricCard
                    label=Signal::derive(move || choose(locale.get(), "总请求数", "Total Requests").to_string())
                    value=Signal::derive(move || usage_requests.as_ref().map(|u| format_number(u.total_requests)).unwrap_or_else(|| choose(locale.get(), "暂无", "N/A").to_string()))
                    tone="primary"
                />
                <AdminMetricCard
                    label=Signal::derive(move || choose(locale.get(), "总令牌数", "Total Tokens").to_string())
                    value=Signal::derive(move || usage_tokens.as_ref().map(|u| format_number(u.total_tokens)).unwrap_or_else(|| choose(locale.get(), "暂无", "N/A").to_string()))
                    tone="success"
                />
                <AdminMetricCard
                    label=Signal::derive(move || choose(locale.get(), "总文档数", "Total Documents").to_string())
                    value=Signal::derive(move || usage_documents.as_ref().map(|u| format_number(u.total_documents)).unwrap_or_else(|| choose(locale.get(), "暂无", "N/A").to_string()))
                    tone="warning"
                />
            </div>

            {/* Usage details */}
            <Show
                when=move || has_usage
                fallback=move || view! {
                    <div class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
                        <div class="text-center text-gray-500">{move || choose(locale.get(), "正在加载使用数据...", "Loading usage data...")}</div>
                    </div>
                }
            >
                <div class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
                    <h3 class="text-lg font-semibold text-gray-900 mb-4">
                        {move || choose(locale.get(), "平台统计", "Platform Statistics")}
                    </h3>
                    <dl class="space-y-3">
                        <div class="flex items-center justify-between">
                            <dt class="text-sm font-medium text-gray-500">{move || choose(locale.get(), "总请求数", "Total Requests")}</dt>
                            <dd class="text-sm font-medium text-gray-900">
                                {usage_display.as_ref().map(|u| u.total_requests.to_string()).unwrap_or_default()}
                            </dd>
                        </div>
                        <div class="flex items-center justify-between">
                            <dt class="text-sm font-medium text-gray-500">{move || choose(locale.get(), "已处理令牌总数", "Total Tokens Processed")}</dt>
                            <dd class="text-sm font-medium text-gray-900">
                                {usage_display.as_ref().map(|u| u.total_tokens.to_string()).unwrap_or_default()}
                            </dd>
                        </div>
                        <div class="flex items-center justify-between">
                            <dt class="text-sm font-medium text-gray-500">{move || choose(locale.get(), "已索引文档总数", "Total Documents Indexed")}</dt>
                            <dd class="text-sm font-medium text-gray-900">
                                {usage_display.as_ref().map(|u| u.total_documents.to_string()).unwrap_or_default()}
                            </dd>
                        </div>
                    </dl>
                </div>
            </Show>
        </div>
    }
}

// ----------------------------------------------------------------------------
// HealthStatus component - shows system health
// ----------------------------------------------------------------------------

#[component]
pub fn HealthStatus(health: Option<HealthResponse>) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    // Clone for each closure to avoid move issues
    let health_status = health.clone();
    let health_service = health.clone();
    let health_version = health.clone();
    let health_status_display = health.clone();

    let has_health = health.is_some();

    let status_class = health
        .as_ref()
        .map(|h| {
            if h.status == "ok" || h.status == "healthy" || h.status == "ready" {
                "bg-green-100 text-green-800"
            } else {
                "bg-red-100 text-red-800"
            }
        })
        .unwrap_or("bg-gray-100 text-gray-800")
        .to_string();

    let is_healthy = health
        .as_ref()
        .map(|h| h.status == "ok" || h.status == "healthy" || h.status == "ready")
        .unwrap_or(false);

    view! {
        <div class="space-y-6">
            {/* Status indicator */}
            <div class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
                <div class="flex items-center justify-between">
                    <div>
                        <h3 class="text-lg font-semibold text-gray-900">
                            {move || choose(locale.get(), "系统状态", "System Status")}
                        </h3>
                        <Show
                            when=move || has_health
                            fallback=move || view! {
                                <p class="text-gray-500 mt-1">{move || choose(locale.get(), "加载中...", "Loading...")}</p>
                            }
                        >
                            <p class="text-gray-500 mt-1">
                                {health_status_display.as_ref().map(|h| health_status_label(locale.get(), &h.status)).unwrap_or_default()}
                            </p>
                        </Show>
                    </div>
                    <div class="flex items-center gap-3">
                        <Show
                            when=move || has_health
                            fallback=move || view! {
                                <div class="w-4 h-4 bg-gray-400 rounded-full"></div>
                            }
                        >
                            <Show
                                when=move || is_healthy
                                fallback=move || view! {
                                    <div class="w-4 h-4 bg-red-500 rounded-full"></div>
                                }
                            >
                                <div class="w-4 h-4 bg-green-500 rounded-full"></div>
                            </Show>
                        </Show>
                    </div>
                </div>
            </div>

            {/* Health details */}
            <Show
                when=move || has_health
                fallback=move || view! {
                    <div class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
                        <div class="text-center text-gray-500">{move || choose(locale.get(), "正在加载健康数据...", "Loading health data...")}</div>
                    </div>
                }
            >
                <div class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
                    <h3 class="text-lg font-semibold text-gray-900 mb-4">
                        {move || choose(locale.get(), "健康详情", "Health Details")}
                    </h3>
                    <dl class="space-y-3">
                        <div class="flex items-center justify-between">
                            <dt class="text-sm font-medium text-gray-500">{move || choose(locale.get(), "状态", "Status")}</dt>
                            <dd>
                                <span class={format!("inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium {}", status_class)}>
                                    {health_status.as_ref().map(|h| health_status_label(locale.get(), &h.status)).unwrap_or_default()}
                                </span>
                            </dd>
                        </div>
                        <div class="flex items-center justify-between">
                            <dt class="text-sm font-medium text-gray-500">{move || choose(locale.get(), "服务", "Service")}</dt>
                            <dd class="text-sm text-gray-900">
                                {health_service.as_ref().map(|h| h.service.clone()).unwrap_or_default()}
                            </dd>
                        </div>
                        <div class="flex items-center justify-between">
                            <dt class="text-sm font-medium text-gray-500">{move || choose(locale.get(), "版本", "Version")}</dt>
                            <dd class="text-sm text-gray-900">
                                {health_version.as_ref().map(|h| h.version.clone()).unwrap_or_default()}
                            </dd>
                        </div>
                    </dl>
                </div>
            </Show>
        </div>
    }
}
