//! Share components - Analytics, Access Logs, and Settings panels

use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use std::sync::Arc;
use web_sdk::dtos::{AccessLogEntry, MemberRow, ShareAnalyticsResponse, ShareSettings};

use crate::i18n::{Locale, choose};
use crate::state::ui_prefs::use_ui_prefs_state;

fn access_level_label(locale: Locale, access_level: &str) -> String {
    match access_level {
        "private" => choose(locale, "私有", "Private").to_string(),
        "link" => choose(locale, "仅链接", "Link Only").to_string(),
        "public" => choose(locale, "公开", "Public").to_string(),
        _ => access_level.to_string(),
    }
}

fn member_role_label(locale: Locale, role: &str) -> String {
    match role {
        "viewer" => choose(locale, "查看者", "viewer").to_string(),
        "editor" => choose(locale, "编辑者", "editor").to_string(),
        _ => role.to_string(),
    }
}

fn member_status_label(locale: Locale, status: &str) -> String {
    match status {
        "pending" => choose(locale, "待接受", "pending").to_string(),
        "accepted" => choose(locale, "已接受", "accepted").to_string(),
        "revoked" => choose(locale, "已撤销", "revoked").to_string(),
        _ => status.to_string(),
    }
}

/// ShareAnalytics component - displays share analytics
#[component]
pub fn ShareAnalytics(analytics: ShareAnalyticsResponse) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    let max_views = analytics.views_by_day.values().max().unwrap_or(&1).max(&1);

    view! {
        <div class="app-surface-card">
            <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                {move || choose(locale.get(), "分析", "Analytics")}
            </h3>

            <div class="grid grid-cols-2 gap-4 mb-6">
                <div class="app-metric-card">
                    <div class="text-2xl font-bold text-card-foreground">{analytics.total_views}</div>
                    <div class="text-sm text-muted-foreground">{move || choose(locale.get(), "总访问量", "Total Views")}</div>
                </div>
                <div class="app-metric-card">
                    <div class="text-2xl font-bold text-card-foreground">{analytics.total_unique_visitors}</div>
                    <div class="text-sm text-muted-foreground">{move || choose(locale.get(), "独立访客", "Unique Visitors")}</div>
                </div>
            </div>

            <div class="space-y-2">
                <h4 class="text-sm font-medium text-foreground">
                    {move || choose(locale.get(), "按天访问量", "Views by Day")}
                </h4>
                <div class="flex items-end gap-1 h-32">
                    {analytics.views_by_day.iter().map(|(day, views)| {
                        let height_pct = (*views as f64 / *max_views as f64 * 100.0).max(5.0) as u32;
                        view! {
                            <div class="flex-1 flex flex-col items-center gap-1">
                                <div
                                    class="w-full rounded-t bg-primary"
                                    style={format!("height: {}px", height_pct)}
                                    title={format!("{}: {} views", day, views)}
                                ></div>
                                <div class="w-full truncate text-center text-xs text-muted-foreground">
                                    {day.chars().skip(5).take(2).collect::<String>()}
                                </div>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </div>
        </div>
    }
}

/// ShareAccessLogs component - displays access logs
#[component]
pub fn ShareAccessLogs(logs: Vec<AccessLogEntry>) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    let is_empty = logs.is_empty();

    view! {
        <div class="app-surface-card">
            <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                {move || choose(locale.get(), "访问日志", "Access Logs")}
            </h3>

            <Show when=move || is_empty>
                <div class="app-empty-state">
                    {move || choose(locale.get(), "暂时没有访问日志", "No access logs yet")}
                </div>
            </Show>

            <Show when=move || !is_empty>
                <div class="app-table-shell overflow-x-auto">
                    <table class="w-full text-sm">
                        <thead>
                            <tr class="border-b border-border bg-muted/30">
                                <th class="px-3 py-2 text-left font-medium text-muted-foreground">{move || choose(locale.get(), "访客 ID", "Visitor ID")}</th>
                                <th class="px-3 py-2 text-left font-medium text-muted-foreground">{move || choose(locale.get(), "访问时间", "Accessed At")}</th>
                                <th class="px-3 py-2 text-left font-medium text-muted-foreground">{move || choose(locale.get(), "动作", "Action")}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {logs.iter().map(|log| {
                                view! {
                                    <tr class="border-b border-border/60 hover:bg-muted/30">
                                        <td class="py-2 px-3 font-mono text-xs">{log.visitor_id.clone()}</td>
                                        <td class="px-3 py-2 text-muted-foreground">{log.accessed_at.clone()}</td>
                                        <td class="py-2 px-3">
                                            <span class="app-status-badge bg-muted text-muted-foreground">
                                                {log.action.clone()}
                                            </span>
                                        </td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>
                </div>
            </Show>
        </div>
    }
}

/// ShareSettingsPanel component - manage sharing settings
#[component]
pub fn ShareSettingsPanel(
    settings: ShareSettings,
    on_settings_updated: Arc<dyn Fn(ShareSettings) + Send + Sync>,
    on_enable_toggle: Arc<dyn Fn(ShareSettings) + Send + Sync>,
) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    let initial_token = settings.share_token.clone();
    let initial_level = settings.access_level.clone();
    let initial_expires_at = settings.expires_at.clone().unwrap_or_default();
    let initial_download = settings.allow_download;

    let (share_token, set_share_token) = signal(initial_token);
    let (access_level, set_access_level) = signal(initial_level);
    let (expires_at, set_expires_at) = signal(initial_expires_at);
    let (allow_download, set_allow_download) = signal(initial_download);
    let (saving, _set_saving) = signal(false);

    // Update local state when props change (for external updates)
    spawn(async move {
        set_share_token.set(settings.share_token.clone());
        set_access_level.set(settings.access_level.clone());
        set_expires_at.set(settings.expires_at.clone().unwrap_or_default());
        set_allow_download.set(settings.allow_download);
    });

    let on_settings_updated_clone = on_settings_updated.clone();
    let handle_save = move |_| {
        let new_settings = ShareSettings {
            share_token: share_token.get(),
            access_level: access_level.get(),
            expires_at: (!expires_at.get().trim().is_empty()).then(|| expires_at.get()),
            allow_download: allow_download.get(),
        };
        on_settings_updated_clone(new_settings);
    };

    let on_enable_toggle_clone = on_enable_toggle.clone();
    let handle_enable_toggle = move |_| {
        on_enable_toggle_clone(ShareSettings {
            share_token: share_token.get(),
            access_level: access_level.get(),
            expires_at: (!expires_at.get().trim().is_empty()).then(|| expires_at.get()),
            allow_download: allow_download.get(),
        });
    };

    view! {
        <div class="app-surface-card">
            <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                {move || choose(locale.get(), "分享设置", "Share Settings")}
            </h3>

            <div class="mb-4">
                <label class="app-form-label">
                    {move || choose(locale.get(), "分享令牌", "Share Token")}
                </label>
                <div class="flex gap-2">
                    <input
                        type="text"
                        readonly
                        class="app-input flex-1 bg-muted/40 font-mono text-sm text-muted-foreground"
                        value=share_token.get()
                    />
                </div>
                <p class="mt-1 text-xs text-muted-foreground">
                    {move || choose(locale.get(), "复制这个令牌以分享当前知识库", "Copy this token to share your notebook")}
                </p>
            </div>

            <div class="mb-4">
                <label class="app-form-label">
                    {move || choose(locale.get(), "访问级别", "Access Level")}
                </label>
                <select
                    class="app-input"
                    on:change=move |ev| set_access_level.set(event_target_value(&ev))
                >
                    <option value="private" selected={access_level.get() == "private"}>{move || access_level_label(locale.get(), "private")}</option>
                    <option value="link" selected={access_level.get() == "link"}>{move || access_level_label(locale.get(), "link")}</option>
                    <option value="public" selected={access_level.get() == "public"}>{move || access_level_label(locale.get(), "public")}</option>
                </select>
                <p class="mt-1 text-xs text-muted-foreground">
                    {move || match access_level.get().as_str() {
                        "private" => choose(locale.get(), "仅自己可访问", "Only you can access"),
                        "link" => choose(locale.get(), "持有链接的任何人可查看", "Anyone with the link can view"),
                        "public" => choose(locale.get(), "对所有人公开可见", "Discoverable and accessible to anyone"),
                        _ => "",
                    }}
                </p>
            </div>

            <div class="mb-4">
                <label class="app-form-label">
                    {move || choose(locale.get(), "链接过期时间", "Link Expiration")}
                </label>
                <input
                    type="text"
                    class="app-input"
                    placeholder={move || choose(locale.get(), "RFC3339，可选", "RFC3339, optional")}
                    value=move || expires_at.get()
                    on:input=move |ev| set_expires_at.set(event_target_value(&ev))
                />
                <p class="mt-1 text-xs text-muted-foreground">
                    {move || choose(locale.get(), "示例：2026-03-31T18:00:00Z", "Example: 2026-03-31T18:00:00Z")}
                </p>
            </div>

            <label class="app-toggle-row mb-4">
                <div>
                    <div class="font-medium text-foreground">
                        {move || choose(locale.get(), "允许下载原始资料", "Allow source downloads")}
                    </div>
                    <div class="mt-1 text-xs text-muted-foreground">
                        {move || choose(locale.get(), "公开页会据此显示下载能力。", "The public share page will expose downloads based on this switch.")}
                    </div>
                </div>
                <input
                    type="checkbox"
                    checked=move || allow_download.get()
                    on:change=move |ev| set_allow_download.set(event_target_checked(&ev))
                />
            </label>

            <div class="flex gap-3 pt-2">
                <button
                    class="app-button-secondary"
                    on:click=handle_enable_toggle
                >
                    {move || if share_token.get().is_empty() {
                        choose(locale.get(), "生成链接", "Generate Link")
                    } else {
                        choose(locale.get(), "关闭分享", "Disable Sharing")
                    }}
                </button>
                <button
                    class="app-button-primary"
                    disabled=saving.get()
                    on:click=handle_save
                >
                    {move || if saving.get() {
                        choose(locale.get(), "保存中...", "Saving...")
                    } else {
                        choose(locale.get(), "保存更改", "Save Changes")
                    }}
                </button>
            </div>
        </div>
    }
}

#[component]
pub fn MembersPanel(
    members: Vec<MemberRow>,
    invite_email: ReadSignal<String>,
    set_invite_email: WriteSignal<String>,
    invite_role: ReadSignal<String>,
    set_invite_role: WriteSignal<String>,
    inviting: ReadSignal<bool>,
    on_invite: Arc<dyn Fn() + Send + Sync>,
    set_remove_member_id: WriteSignal<Option<String>>,
) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    let members_for_view = members.clone();
    let members_for_empty = members.clone();
    let members_for_non_empty = members.clone();
    view! {
        <div class="app-surface-card">
            <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                {move || choose(locale.get(), "成员", "Members")}
            </h3>

            <div class="grid gap-3 md:grid-cols-[1fr_160px_auto] mb-4">
                <input
                    type="email"
                    class="app-input"
                    placeholder={move || choose(locale.get(), "member@example.com", "member@example.com")}
                    value=move || invite_email.get()
                    on:input=move |ev| set_invite_email.set(event_target_value(&ev))
                />
                <select
                    class="app-input"
                    on:change=move |ev| set_invite_role.set(event_target_value(&ev))
                >
                    <option value="viewer" selected={move || invite_role.get() == "viewer"}>{move || member_role_label(locale.get(), "viewer")}</option>
                    <option value="editor" selected={move || invite_role.get() == "editor"}>{move || member_role_label(locale.get(), "editor")}</option>
                </select>
                <button
                    class="app-button-primary"
                    disabled=move || inviting.get()
                    on:click=move |_| on_invite()
                >
                    {move || if inviting.get() {
                        choose(locale.get(), "邀请中...", "Inviting...")
                    } else {
                        choose(locale.get(), "邀请", "Invite")
                    }}
                </button>
            </div>

            <Show when=move || members_for_empty.is_empty()>
                <div class="app-empty-state">{move || choose(locale.get(), "暂时没有成员", "No members yet")}</div>
            </Show>

            <Show when=move || !members_for_non_empty.is_empty()>
                <div class="space-y-2">
                    {members_for_view.clone().into_iter().map(|member| {
                        let member_id = StoredValue::new(member.member_id.clone());
                        let label = if !member.email.is_empty() {
                            member.email.clone()
                        } else {
                            member.user_id.clone()
                        };
                        view! {
                            <div class="flex items-center justify-between rounded-xl border border-border bg-card px-3 py-2">
                                <div class="min-w-0">
                                    <div class="truncate text-sm font-medium text-card-foreground">{label}</div>
                                    <div class="text-xs text-muted-foreground">
                                        {member_role_label(locale.get(), &member.role)}
                                        {" · "}
                                        {member_status_label(locale.get(), &member.status)}
                                    </div>
                                </div>
                                <button
                                    class="app-button-danger px-3 py-1.5 text-xs"
                                    on:click=move |_| set_remove_member_id.set(Some(member_id.get_value()))
                                >
                                    {move || choose(locale.get(), "移除", "Remove")}
                                </button>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </Show>
        </div>
    }
}
