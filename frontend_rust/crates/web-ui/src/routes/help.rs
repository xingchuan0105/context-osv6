//! Product help center page (wiki-style)

use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

use crate::i18n::choose;
use crate::state::ui_prefs::use_ui_prefs_state;

#[component]
pub fn HelpPage() -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    let location = use_location();
    let is_preview_route = Memo::new(move |_| location.pathname.get().starts_with("/preview/live"));
    let is_preview_for_dashboard = is_preview_route.clone();
    let dashboard_href = Memo::new(move |_| {
        if is_preview_for_dashboard.get() {
            "/preview/live/dashboard".to_string()
        } else {
            "/dashboard".to_string()
        }
    });
    let is_preview_for_settings_profile = is_preview_route.clone();
    let settings_profile_href = Memo::new(move |_| {
        if is_preview_for_settings_profile.get() {
            "/preview/live/settings?tab=profile".to_string()
        } else {
            "/settings?tab=profile".to_string()
        }
    });

    view! {
        <div class="app-page-shell">
            <div class="mx-auto max-w-5xl space-y-6">
                <div class="flex flex-wrap items-start justify-between gap-3 sm:gap-4">
                    <div class="app-page-heading mb-0">
                        <h1 class="app-page-title">
                            {move || choose(locale.get(), "帮助中心", "Help Center")}
                        </h1>
                        <p class="app-page-subtitle">
                            {move || choose(
                                locale.get(),
                                "以 Wiki 方式汇总 Context-OS 的核心功能、接口能力与常见排查路径。",
                                "A wiki-style reference for Context-OS core workflows, API capabilities, and troubleshooting."
                            )}
                        </p>
                    </div>
                    <div class="flex items-center gap-2">
                        <A href=move || dashboard_href.get() attr:class="app-button-secondary">
                            {move || choose(locale.get(), "返回 Dashboard", "Back to Dashboard")}
                        </A>
                        <A href=move || settings_profile_href.get() attr:class="app-button-secondary">
                            {move || choose(locale.get(), "账户设置", "Account Settings")}
                        </A>
                    </div>
                </div>

                <div class="app-surface-card space-y-4">
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {move || choose(locale.get(), "1. 账户与认证", "1. Account & Authentication")}
                    </h2>
                    <ul class="list-disc space-y-2 pl-5 text-sm text-muted-foreground">
                        <li>{move || choose(locale.get(), "支持注册、登录、重置密码与会话退出。", "Supports sign up, sign in, reset password, and session logout.")}</li>
                        <li>{move || choose(locale.get(), "用户资料页可查看邮箱、编辑姓名，并查看个人额度卡片。", "Profile page provides email, name editing, and personal usage quota card.")}</li>
                        <li>{move || choose(locale.get(), "若出现 401/403，请先确认 token 有效与登录状态。", "For 401/403 responses, verify token validity and sign-in state first.")}</li>
                    </ul>
                </div>

                <div class="app-surface-card space-y-4">
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {move || choose(locale.get(), "2. Workspace 与会话", "2. Workspace & Sessions")}
                    </h2>
                    <ul class="list-disc space-y-2 pl-5 text-sm text-muted-foreground">
                        <li>{move || choose(locale.get(), "每个 Notebook 对应一个 Workspace。左侧为会话历史，中间为对话，右侧为资料与笔记。", "Each notebook maps to one workspace: left thread history, center chat, right sources and notes.")}</li>
                        <li>{move || choose(locale.get(), "历史列表支持关键词过滤；点击会话可恢复该 session 消息。", "Thread history supports keyword filtering; selecting a thread restores that session messages.")}</li>
                        <li>{move || choose(locale.get(), "Workspace 顶栏支持快速分享弹层、快捷设置弹层、API 页面跳转、用户页面跳转。", "Workspace top bar supports share popover, settings popover, API page jump, and user page jump.")}</li>
                    </ul>
                </div>

                <div class="app-surface-card space-y-4">
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {move || choose(locale.get(), "3. 资料管理与 Doc Scope", "3. Sources & Doc Scope")}
                    </h2>
                    <ul class="list-disc space-y-2 pl-5 text-sm text-muted-foreground">
                        <li>{move || choose(locale.get(), "支持上传文件与添加 URL 资料源。", "Supports file upload and URL source ingestion.")}</li>
                        <li>{move || choose(locale.get(), "会话可按资料勾选形成文档范围（doc scope），影响回答上下文。", "Per-session source selection forms doc scope and controls retrieval context.")}</li>
                        <li>{move || choose(locale.get(), "资料状态异常时可执行重建索引；索引中状态会在右侧面板提示。", "You can trigger re-indexing for unhealthy sources; indexing status appears in the source pane.")}</li>
                    </ul>
                </div>

                <div class="app-surface-card space-y-4">
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {move || choose(locale.get(), "4. 分享与协作", "4. Sharing & Collaboration")}
                    </h2>
                    <ul class="list-disc space-y-2 pl-5 text-sm text-muted-foreground">
                        <li>{move || choose(locale.get(), "Share Center 支持分享开关、访问级别、成员邀请与日志追踪。", "Share Center supports access mode, permission scope, member invite/remove, and access logs.")}</li>
                        <li>{move || choose(locale.get(), "公开分享链接支持只读知识库页面与引用追踪。", "Public shared links support read-only knowledge base page and citation traces.")}</li>
                        <li>{move || choose(locale.get(), "Workspace 顶栏分享按钮默认走轻量弹层，不强制跳转新页面。", "Workspace Share button uses a lightweight popover by default, without forced route transition.")}</li>
                    </ul>
                </div>

                <div class="app-surface-card space-y-4">
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {move || choose(locale.get(), "5. API 接入", "5. API Access")}
                    </h2>
                    <ul class="list-disc space-y-2 pl-5 text-sm text-muted-foreground">
                        <li>{move || choose(locale.get(), "每个 Notebook 可独立创建 API Key，支持权限与频率控制。", "Each notebook can create scoped API keys with permission and rate controls.")}</li>
                        <li>{move || choose(locale.get(), "支持资料上传、URL 导入、RAG 查询等能力。", "Supports source upload, URL ingestion, and RAG query workflows.")}</li>
                        <li>{move || choose(locale.get(), "如需撤销访问，可在 API 页面执行密钥撤销。", "To revoke access, use key revocation in API Access page.")}</li>
                    </ul>
                </div>

                <div class="app-surface-card space-y-4">
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {move || choose(locale.get(), "6. 设置、主题、语言、额度", "6. Settings, Theme, Language & Quota")}
                    </h2>
                    <ul class="list-disc space-y-2 pl-5 text-sm text-muted-foreground">
                        <li>{move || choose(locale.get(), "设置页统一管理主题（浅色/深色/跟随系统）与语言（中文/英文）。", "Settings centralizes theme (light/dark/system) and language (Chinese/English).")}</li>
                        <li>{move || choose(locale.get(), "通知页支持偏好开关与静默时段。", "Notifications page manages delivery toggles and quiet-hour preferences.")}</li>
                        <li>{move || choose(locale.get(), "个人额度卡片展示 5 小时与 7 天窗口用量、阻断与恢复时间。", "Personal usage card shows 5-hour and 7-day windows, blocked state, and recovery timing.")}</li>
                    </ul>
                </div>

                <div class="app-surface-card space-y-4">
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {move || choose(locale.get(), "7. 常见问题排查", "7. Troubleshooting Quick Checks")}
                    </h2>
                    <ul class="list-disc space-y-2 pl-5 text-sm text-muted-foreground">
                        <li>{move || choose(locale.get(), "登录报错 503：优先检查后端与中间件健康状态，以及 .env 配置是否可解析。", "Login 503: first check backend/middleware health and valid .env parsing.")}</li>
                        <li>{move || choose(locale.get(), "界面语言混杂：到设置页重新选择语言，并确认缓存状态。", "Mixed language UI: reselect language in Settings and refresh cached state.")}</li>
                        <li>{move || choose(locale.get(), "页面样式未更新：确认当前分支前端构建是否成功，并清理浏览器缓存后重载。", "UI style not updated: confirm frontend build on current branch and hard-refresh browser cache.")}</li>
                    </ul>
                </div>
            </div>
        </div>
    }
}
