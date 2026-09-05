use crate::api_base::poc_api_base;
use contracts::share::{AccessLogEntry, ShareAnalyticsResponse, ShareSettings};
use leptos::prelude::*;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;
use web_sdk::BrowserRestClient;

#[derive(Params, PartialEq, Clone, Debug)]
struct ShareParams {
    workspace_id: Option<String>,
}

#[component]
pub fn WorkspaceSharePage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let params = use_params::<ShareParams>();
    let workspace_id = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.workspace_id.clone())
            .unwrap_or_default()
    });

    let share_settings = RwSignal::new(None::<ShareSettings>);
    let current_token = RwSignal::new(None::<String>);
    let error = RwSignal::new(None::<String>);
    let copied = RwSignal::new(false);
    let busy = RwSignal::new(false);

    Effect::new(move |_| {
        let wid = workspace_id.get();
        if wid.is_empty() {
            return;
        }
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if tok.is_empty() {
            return;
        }

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            if let Ok(settings) = client.get_share_settings(&wid).await {
                current_token.set(Some(settings.share_token.clone()));
                share_settings.set(Some(settings));
            }
        });
    });

    let on_create_share = move |_| {
        let wid = workspace_id.get_untracked();
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if wid.is_empty() || tok.is_empty() || busy.get() {
            return;
        }

        busy.set(true);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            match client.create_share(&wid).await {
                Ok(resp) => {
                    current_token.set(Some(resp.share_token));
                }
                Err(err) => {
                    error.set(Some(format!("创建分享失败：{err}")));
                }
            }
            busy.set(false);
        });
    };

    let on_revoke_share = move |_| {
        let wid = workspace_id.get_untracked();
        let Some(stok) = current_token.get_untracked() else {
            return;
        };
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if wid.is_empty() || tok.is_empty() || busy.get() {
            return;
        }

        busy.set(true);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            if client.revoke_share(&wid, &stok).await.is_ok() {
                current_token.set(None);
                share_settings.set(None);
            }
            busy.set(false);
        });
    };

    let on_copy_link = move |_| {
        let Some(tok) = current_token.get_untracked() else {
            return;
        };
        let _link = format!("/shared/kb/{tok}");
        #[cfg(target_arch = "wasm32")]
        if let Some(w) = web_sys::window() {
            let _ = w.navigator().clipboard().write_text(&_link);
        }
        copied.set(true);
    };

    view! {
        <div class="settings-shell" data-testid="workspace-share-page">
            <header class="settings-header">
                <div class="settings-header-left">
                    <a href=move || format!("/dashboard/{}", workspace_id.get()) class="settings-back-link">
                        "← 返回工作台"
                    </a>
                    <h1 class="settings-title">"工作区公开分享中心"</h1>
                </div>
            </header>

            <nav class="settings-nav" aria-label="分享导航">
                <a href=move || format!("/dashboard/{}/share", workspace_id.get()) class="settings-nav-item is-active">
                    "分享链接设置"
                </a>
                <a href=move || format!("/dashboard/{}/share/access-logs", workspace_id.get()) class="settings-nav-item">
                    "访问审计日志"
                </a>
                <a href=move || format!("/dashboard/{}/share/analytics", workspace_id.get()) class="settings-nav-item">
                    "互动与流量分析"
                </a>
            </nav>

            <main class="settings-content">
                <section class="settings-panel" data-testid="share-panel">
                    <h2>"只读公开分享"</h2>
                    <p class="settings-panel-desc">
                        "生成公开链接后，外部访客可以通过只读 Token 访问该知识库并提问，无法修改资料或查看其他工作区。"
                    </p>
                    {move || {
                        if let Some(tok) = current_token.get() {
                            view! {
                                <div class="share-active-box" data-testid="share-active-box">
                                    <div class="share-link-row">
                                        <span class="share-link-text">{format!("/shared/kb/{tok}")}</span>
                                        <button
                                            type="button"
                                            class="dashboard-btn-confirm"
                                            data-testid="copy-share-btn"
                                            on:click=on_copy_link
                                        >
                                            {move || if copied.get() { "已复制！" } else { "复制链接" }}
                                        </button>
                                    </div>
                                    <button
                                        type="button"
                                        class="settings-btn-revoke"
                                        data-testid="revoke-share-btn"
                                        disabled=move || busy.get()
                                        on:click=on_revoke_share
                                    >
                                        "关闭公开分享"
                                    </button>
                                </div>
                            }
                            .into_any()
                        } else {
                            view! {
                                <div class="share-inactive-box">
                                    <button
                                        type="button"
                                        class="dashboard-create-btn"
                                        data-testid="create-share-btn"
                                        disabled=move || busy.get()
                                        on:click=on_create_share
                                    >
                                        "生成公开分享链接"
                                    </button>
                                </div>
                            }
                            .into_any()
                        }
                    }}
                </section>
            </main>
        </div>
    }
}

#[component]
pub fn WorkspaceShareLogsPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let params = use_params::<ShareParams>();
    let workspace_id = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.workspace_id.clone())
            .unwrap_or_default()
    });

    let logs = RwSignal::new(Vec::<AccessLogEntry>::new());

    Effect::new(move |_| {
        let wid = workspace_id.get();
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if wid.is_empty() || tok.is_empty() {
            return;
        }

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            if let Ok(resp) = client.get_share_access_logs(&wid).await {
                logs.set(resp.logs);
            }
        });
    });

    view! {
        <div class="settings-shell" data-testid="share-logs-page">
            <header class="settings-header">
                <div class="settings-header-left">
                    <a href=move || format!("/dashboard/{}/share", workspace_id.get()) class="settings-back-link">
                        "← 返回分享中心"
                    </a>
                    <h1 class="settings-title">"访问审计日志"</h1>
                </div>
            </header>
            <main class="settings-content">
                <section class="settings-panel">
                    <ul class="share-log-list">
                        <For
                            each=move || logs.get()
                            key=|log| log.id.clone()
                            children=move |log| {
                                view! {
                                    <li class="share-log-item">
                                        <span>{log.accessed_at}</span>
                                        <span>{log.action}</span>
                                        <span>{format!("访客: {}", log.visitor_id)}</span>
                                    </li>
                                }
                            }
                        />
                    </ul>
                </section>
            </main>
        </div>
    }
}

#[component]
pub fn WorkspaceShareAnalyticsPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let params = use_params::<ShareParams>();
    let workspace_id = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.workspace_id.clone())
            .unwrap_or_default()
    });

    let analytics = RwSignal::new(None::<ShareAnalyticsResponse>);

    Effect::new(move |_| {
        let wid = workspace_id.get();
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if wid.is_empty() || tok.is_empty() {
            return;
        }

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            if let Ok(resp) = client.get_share_analytics(&wid).await {
                analytics.set(Some(resp));
            }
        });
    });

    view! {
        <div class="settings-shell" data-testid="share-analytics-page">
            <header class="settings-header">
                <div class="settings-header-left">
                    <a href=move || format!("/dashboard/{}/share", workspace_id.get()) class="settings-back-link">
                        "← 返回分享中心"
                    </a>
                    <h1 class="settings-title">"分享互动分析"</h1>
                </div>
            </header>
            <main class="settings-content">
                <section class="settings-panel">
                    <div class="settings-usage-cards">
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"总浏览量 (Views)"</span>
                            <span class="settings-usage-value">
                                {move || analytics.get().map(|a| a.total_views).unwrap_or(0).to_string()}
                            </span>
                        </div>
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"独立访客 (UV)"</span>
                            <span class="settings-usage-value">
                                {move || analytics.get().map(|a| a.total_unique_visitors).unwrap_or(0).to_string()}
                            </span>
                        </div>
                    </div>
                </section>
            </main>
        </div>
    }
}
