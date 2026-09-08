use crate::api_base::poc_api_base;
use crate::components::shell::ProductChrome;
use crate::components::ui::PageStatus;
use contracts::share::ShareAnalyticsResponse;
use contracts::workspaces::Workspace;
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params};
use leptos_router::params::Params;
use leptos_router::NavigateOptions;
use web_sdk::BrowserRestClient;

#[derive(Params, PartialEq, Clone, Debug)]
struct AnalyzeParams {
    workspace_id: Option<String>,
}

#[component]
pub fn WorkspaceAnalyzePage() -> impl IntoView {
    let params = use_params::<AnalyzeParams>();
    let navigate = use_navigate();
    let target = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.workspace_id.clone())
            .map(|id| format!("/dashboard/{id}/share"))
    });

    Effect::new(move |_| {
        if let Some(href) = target.get() {
            navigate(
                &href,
                NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            );
        }
    });

    view! {
        <ProductChrome>
            <p class="dashboard-loading" data-testid="workspace-analyze-redirect">
                "正在前往分享中心…"
            </p>
        </ProductChrome>
    }
}

#[component]
pub fn GlobalAnalyticsPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let rows = RwSignal::new(Vec::<(Workspace, Option<ShareAnalyticsResponse>)>::new());
    let loading = RwSignal::new(true);
    let error = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let tok = if token.get().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get()
        };
        if tok.is_empty() {
            loading.set(false);
            error.set(Some("请先登录后查看分享统计。".to_string()));
            return;
        }
        loading.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            match client.list_workspaces().await {
                Ok(list) => {
                    let mut collected = Vec::new();
                    for workspace in list.workspaces {
                        let analytics = client.get_share_analytics(&workspace.id).await.ok();
                        collected.push((workspace, analytics));
                    }
                    rows.set(collected);
                    loading.set(false);
                }
                Err(err) => {
                    error.set(Some(format!("加载分享统计失败：{err}")));
                    loading.set(false);
                }
            }
        });
    });

    let totals = Signal::derive(move || {
        let mut views = 0_i64;
        let mut visitors = 0_i64;
        let mut by_day = std::collections::BTreeMap::<String, i64>::new();
        for (_ws, analytics) in rows.get() {
            if let Some(a) = analytics {
                views += a.total_views;
                visitors += a.total_unique_visitors;
                for (day, n) in a.views_by_day {
                    *by_day.entry(day).or_default() += n;
                }
            }
        }
        (views, visitors, by_day)
    });

    view! {
        <ProductChrome>
        <div class="dashboard-shell" data-testid="global-analytics-page">
            <header class="dashboard-header">
                <div class="dashboard-header-left">
                    <a href="/dashboard" class="dashboard-chat-link">"← 全部工作区"</a>
                    <h1 class="dashboard-title">"全局分享与访问分析"</h1>
                </div>
            </header>
            <PageStatus
                loading=Signal::derive(move || loading.get())
                error=Signal::derive(move || error.get())
                empty=Signal::derive(move || rows.get().is_empty())
                empty_text="还没有可统计的工作区。"
            >
                <section class="dashboard-panel">
                    <h2>"分享页面访问与互动统计"</h2>
                    <div class="settings-usage-cards">
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"总公开浏览量"</span>
                            <span class="settings-usage-value" data-testid="analytics-views">
                                {move || totals.get().0.to_string()}
                            </span>
                        </div>
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"独立访客数"</span>
                            <span class="settings-usage-value" data-testid="analytics-visitors">
                                {move || totals.get().1.to_string()}
                            </span>
                        </div>
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"已开启分享的工作区"</span>
                            <span class="settings-usage-value">
                                {move || {
                                    rows.get()
                                        .iter()
                                        .filter(|(_, analytics)| analytics.is_some())
                                        .count()
                                        .to_string()
                                }}
                            </span>
                        </div>
                    </div>
                    {move || {
                        let days = totals.get().2;
                        view! { <ShareViewsChart days=days/> }
                    }}
                </section>
            </PageStatus>
        </div>
        </ProductChrome>
    }
}

#[component]
fn ShareViewsChart(days: std::collections::BTreeMap<String, i64>) -> impl IntoView {
    let series: Vec<(String, i64)> = days.into_iter().collect();
    if series.is_empty() {
        return view! {
            <p class="page-status-empty" data-testid="share-views-bar-chart">"暂无按日访问数据。"</p>
        }
        .into_any();
    }
    let max = series.iter().map(|(_, n)| *n).max().unwrap_or(1).max(1);
    let width = 640.0;
    let height = 180.0;
    let pad_l = 36.0;
    let pad_b = 24.0;
    let inner_w = width - pad_l - 12.0;
    let inner_h = height - pad_b - 12.0;
    let n = series.len() as f64;
    let slot = inner_w / n;
    let bars: Vec<_> = series
        .iter()
        .enumerate()
        .map(|(i, (day, views))| {
            let h = (*views as f64 / max as f64) * inner_h;
            let x = pad_l + i as f64 * slot + slot * 0.2;
            let y = 12.0 + inner_h - h;
            (day.clone(), x, y, slot * 0.6, h, *views)
        })
        .collect();
    view! {
        <svg
            class="share-views-chart"
            viewBox="0 0 640 180"
            role="img"
            aria-label="访问量柱状图"
            data-testid="share-views-bar-chart"
        >
            {bars
                .into_iter()
                .map(|(day, x, y, w, h, views)| {
                    view! {
                        <g>
                            <rect x=x.to_string() y=y.to_string() width=w.to_string() height=h.to_string() class="share-views-bar"/>
                            <title>{format!("{day}: {views}")}</title>
                        </g>
                    }
                })
                .collect_view()}
        </svg>
    }
    .into_any()
}
