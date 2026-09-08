use crate::api_base::poc_api_base;
use crate::components::shell::ProductChrome;
use crate::components::ui::PageStatus;
use leptos::prelude::*;
use web_sdk::{
    BrowserRestClient, DailyUsage, UsageForecastResponse, UsageWindowResponse,
};

#[component]
pub fn UsagePage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let window = RwSignal::new(None::<UsageWindowResponse>);
    let history = RwSignal::new(Vec::<DailyUsage>::new());
    let forecast = RwSignal::new(None::<UsageForecastResponse>);
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
            error.set(Some("请先登录后查看用量。".to_string()));
            return;
        }
        loading.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            let window_res = client.get_usage_window().await;
            let history_res = client.get_usage_history(7).await;
            let forecast_res = client.get_usage_forecast().await;
            match (window_res, history_res, forecast_res) {
                (Ok(w), Ok(h), Ok(f)) => {
                    window.set(Some(w));
                    history.set(h.daily);
                    forecast.set(Some(f));
                    loading.set(false);
                }
                (Err(err), _, _) | (_, Err(err), _) | (_, _, Err(err)) => {
                    error.set(Some(format!("加载用量失败：{err}")));
                    loading.set(false);
                }
            }
        });
    });

    view! {
        <ProductChrome>
        <div class="settings-shell" data-testid="usage-page">
            <header class="settings-header">
                <div class="settings-header-left">
                    <a href="/settings" class="settings-back-link">"← 返回设置"</a>
                    <h1 class="settings-title">"用量总览"</h1>
                </div>
            </header>
            <PageStatus
                loading=Signal::derive(move || loading.get())
                error=Signal::derive(move || error.get())
                empty=Signal::derive(move || window.get().is_none())
                empty_text="暂无用量数据。"
            >
                <main class="settings-content">
                    <section class="settings-panel">
                        <h2>"当前套餐"</h2>
                        <p class="settings-panel-desc" data-testid="usage-plan">
                            {move || {
                                forecast
                                    .get()
                                    .map(|f| format!("当前套餐：{}", f.current_plan.to_uppercase()))
                                    .unwrap_or_default()
                            }}
                        </p>
                        <div class="settings-usage-cards">
                            <div class="settings-usage-card">
                                <span class="settings-usage-label">"近 5 小时 Token"</span>
                                <span class="settings-usage-value" data-testid="usage-5h">
                                    {move || {
                                        window
                                            .get()
                                            .map(|w| {
                                                format!(
                                                    "{:.0}",
                                                    w.rolling_5h.used_tokens_approx.unwrap_or(w.rolling_5h.used)
                                                )
                                            })
                                            .unwrap_or_else(|| "—".to_string())
                                    }}
                                </span>
                            </div>
                            <div class="settings-usage-card">
                                <span class="settings-usage-label">"近 7 天 Token"</span>
                                <span class="settings-usage-value" data-testid="usage-7d">
                                    {move || {
                                        window
                                            .get()
                                            .map(|w| {
                                                format!(
                                                    "{:.0}",
                                                    w.rolling_7d.used_tokens_approx.unwrap_or(w.rolling_7d.used)
                                                )
                                            })
                                            .unwrap_or_else(|| "—".to_string())
                                    }}
                                </span>
                            </div>
                        </div>
                    </section>
                    <section class="settings-panel">
                        <h2>"用量趋势"</h2>
                        {move || view! { <UsageTrendChart daily=history.get()/> }}
                    </section>
                </main>
            </PageStatus>
        </div>
        </ProductChrome>
    }
}

#[component]
fn UsageTrendChart(daily: Vec<DailyUsage>) -> impl IntoView {
    if daily.is_empty() {
        return view! {
            <p class="page-status-empty" data-testid="usage-trend-chart">"暂无趋势数据。"</p>
        }
        .into_any();
    }
    let max = daily.iter().map(|d| d.tokens).max().unwrap_or(1).max(1) as f64;
    let width = 600.0;
    let height = 180.0;
    let pad = (36.0, 16.0, 28.0, 16.0);
    let inner_w = width - pad.0 - pad.3;
    let inner_h = height - pad.1 - pad.2;
    let n = daily.len();
    let points: String = daily
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let x = pad.0
                + if n > 1 {
                    i as f64 * inner_w / (n - 1) as f64
                } else {
                    inner_w / 2.0
                };
            let y = pad.1 + inner_h - (d.tokens as f64 / max) * inner_h;
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    view! {
        <svg
            class="usage-trend-chart"
            viewBox="0 0 600 180"
            role="img"
            aria-label="用量趋势"
            data-testid="usage-trend-chart"
        >
            <polyline points=points class="usage-trend-line" fill="none"/>
        </svg>
    }
    .into_any()
}
