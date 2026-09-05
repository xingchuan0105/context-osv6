use leptos::prelude::*;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;

#[derive(Params, PartialEq, Clone, Debug)]
struct AnalyzeParams {
    workspace_id: Option<String>,
}

#[component]
pub fn WorkspaceAnalyzePage() -> impl IntoView {
    let params = use_params::<AnalyzeParams>();
    let workspace_id = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.workspace_id.clone())
            .unwrap_or_default()
    });

    view! {
        <div class="dashboard-shell" data-testid="workspace-analyze-page">
            <header class="dashboard-header">
                <div class="dashboard-header-left">
                    <a href=move || format!("/dashboard/{}", workspace_id.get()) class="dashboard-chat-link">
                        "← 返回工作台"
                    </a>
                    <h1 class="dashboard-title">"资料分析与切片健康度"</h1>
                </div>
            </header>

            <main class="dashboard-content">
                <section class="dashboard-panel" data-testid="analyze-panel">
                    <h2>"文档切片与检索索引状态"</h2>
                    <p class="settings-panel-desc">
                        {move || format!("当前分析工作区：{}", workspace_id.get())}
                    </p>
                    <div class="settings-usage-cards">
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"已解析分块 (Chunks)"</span>
                            <span class="settings-usage-value">"42"</span>
                        </div>
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"向量索引状态"</span>
                            <span class="settings-usage-value">"正常 (Healthy)"</span>
                        </div>
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"检索召回覆盖率"</span>
                            <span class="settings-usage-value">"98.5%"</span>
                        </div>
                    </div>
                </section>
            </main>
        </div>
    }
}

#[component]
pub fn GlobalAnalyticsPage() -> impl IntoView {
    view! {
        <div class="dashboard-shell" data-testid="global-analytics-page">
            <header class="dashboard-header">
                <div class="dashboard-header-left">
                    <a href="/dashboard" class="dashboard-chat-link">"← 全部工作区"</a>
                    <h1 class="dashboard-title">"全局分享与访问分析"</h1>
                </div>
            </header>

            <main class="dashboard-content">
                <section class="dashboard-panel">
                    <h2>"分享页面访问与互动统计"</h2>
                    <div class="settings-usage-cards">
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"总公开浏览量"</span>
                            <span class="settings-usage-value">"128"</span>
                        </div>
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"独立访客数"</span>
                            <span class="settings-usage-value">"56"</span>
                        </div>
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"分享会话提问数"</span>
                            <span class="settings-usage-value">"310"</span>
                        </div>
                    </div>
                </section>
            </main>
        </div>
    }
}
