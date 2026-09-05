use crate::api_base::poc_api_base;
use leptos::prelude::*;
use web_sdk::{
    BillingPlan, BrowserRestClient, CheckoutRequest, TopupPack, WalletBalanceResponse,
};

#[component]
pub fn PricingPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let plans = RwSignal::new(Vec::<BillingPlan>::new());
    let wallet = RwSignal::new(None::<WalletBalanceResponse>);
    let topup_packs = RwSignal::new(Vec::<TopupPack>::new());
    let selected_pack = RwSignal::new("topup_50".to_string());
    let selected_provider = RwSignal::new("alipay".to_string());
    let checkout_url = RwSignal::new(None::<String>);
    let error = RwSignal::new(None::<String>);
    let loading = RwSignal::new(false);

    Effect::new(move |_| {
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token)
        } else {
            Some(token.get_untracked())
        };

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), tok.clone());
            if let Ok(resp) = client.get_billing_plans().await {
                plans.set(resp.plans);
            }
            if tok.is_some() {
                if let Ok(w) = client.get_wallet_balance().await {
                    wallet.set(Some(w));
                }
                if let Ok(packs) = client.list_topup_packs().await {
                    topup_packs.set(packs);
                }
            }
        });
    });

    let on_topup_checkout = move |_| {
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if tok.is_empty() {
            error.set(Some("请先登录后进行充值".to_string()));
            return;
        }

        loading.set(true);
        error.set(None);
        let pack_id = selected_pack.get();
        let provider = selected_provider.get();

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            let req = CheckoutRequest {
                plan_id: None,
                provider: Some(provider),
                kind: Some("wallet_topup".to_string()),
                topup_pack_id: Some(pack_id),
            };
            match client.create_checkout_session(&req).await {
                Ok(resp) => {
                    checkout_url.set(Some(resp.url));
                }
                Err(err) => {
                    error.set(Some(format!("创建结账会话失败：{err}")));
                }
            }
            loading.set(false);
        });
    };

    view! {
        <div class="pricing-shell" data-testid="pricing-page">
            <header class="pricing-header">
                <div class="pricing-header-left">
                    <a href="/chat" class="settings-back-link">"← 返回对话"</a>
                    <h1 class="pricing-title">"套餐定价与会员权益"</h1>
                    <p class="pricing-subtitle">"选择适合您的生产力方案，支持随时充值或升降级"</p>
                </div>
            </header>

            <main class="pricing-content">
                <section class="pricing-plans-section">
                    <div class="pricing-plans-grid" data-testid="plans-grid">
                        <div class="pricing-plan-card" data-testid="plan-free">
                            <h2 class="pricing-plan-name">"免费体验版"</h2>
                            <div class="pricing-plan-price">"¥0"</div>
                            <p class="pricing-plan-desc">"适合个人基础问答与轻量体验"</p>
                            <ul class="pricing-plan-features">
                                <li>"包含 Quick Chat 日常对话"</li>
                                <li>"单会话最多 3 个临时文件"</li>
                                <li>"基础问答速度"</li>
                            </ul>
                        </div>

                        <div class="pricing-plan-card is-popular" data-testid="plan-pro">
                            <span class="pricing-popular-badge">"推荐"</span>
                            <h2 class="pricing-plan-name">"Pro 专业版"</h2>
                            <div class="pricing-plan-price">"¥99" <span class="pricing-plan-period">"/月"</span></div>
                            <p class="pricing-plan-desc">"深度知识库构建、无限持久资料与高速 Agent"</p>
                            <ul class="pricing-plan-features">
                                <li>"无限量工作区持久资料库"</li>
                                <li>"专属 Agent 快速通道"</li>
                                <li>"深度 Markdown 笔记与切片分析"</li>
                                <li>"自备 API Key (BYOK) 零平台加价"</li>
                            </ul>
                        </div>
                    </div>
                </section>

                <section id="topup" class="settings-panel pricing-topup-section" data-testid="topup-panel">
                    <header class="settings-panel-header">
                        <h2 class="settings-panel-title">"钱包余额与按需充值"</h2>
                        <p class="settings-panel-desc">
                            "账户余额用于平台模型推理、OCR 解析与网络搜索按量扣费。支持随时充值。"
                        </p>
                    </header>

                    <div class="settings-usage-cards">
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"当前可用余额"</span>
                            <span class="settings-usage-value" data-testid="wallet-balance">
                                {move || {
                                    wallet
                                        .get()
                                        .map(|w| format!("¥{:.2}", (w.balance_fen as f64) / 100.0))
                                        .unwrap_or_else(|| "¥0.00".to_string())
                                }}
                            </span>
                        </div>
                    </div>

                    <div class="topup-pack-selector">
                        <label class="topup-label">"选择充值金额："</label>
                        <div class="topup-packs-row">
                            <button
                                type="button"
                                class=move || if selected_pack.get() == "topup_50" { "topup-pack-btn is-active" } else { "topup-pack-btn" }
                                data-testid="pack-50"
                                on:click=move |_| selected_pack.set("topup_50".to_string())
                            >
                                "¥50"
                            </button>
                            <button
                                type="button"
                                class=move || if selected_pack.get() == "topup_100" { "topup-pack-btn is-active" } else { "topup-pack-btn" }
                                data-testid="pack-100"
                                on:click=move |_| selected_pack.set("topup_100".to_string())
                            >
                                "¥100"
                            </button>
                            <button
                                type="button"
                                class=move || if selected_pack.get() == "topup_200" { "topup-pack-btn is-active" } else { "topup-pack-btn" }
                                data-testid="pack-200"
                                on:click=move |_| selected_pack.set("topup_200".to_string())
                            >
                                "¥200"
                            </button>
                        </div>
                    </div>

                    <div class="topup-provider-selector">
                        <label class="topup-label">"支付方式："</label>
                        <div class="topup-providers-row">
                            <button
                                type="button"
                                class=move || if selected_provider.get() == "alipay" { "topup-provider-btn is-active" } else { "topup-provider-btn" }
                                data-testid="provider-alipay"
                                on:click=move |_| selected_provider.set("alipay".to_string())
                            >
                                "支付宝 (Alipay 沙箱/测试)"
                            </button>
                            <button
                                type="button"
                                class=move || if selected_provider.get() == "creem" { "topup-provider-btn is-active" } else { "topup-provider-btn" }
                                data-testid="provider-creem"
                                on:click=move |_| selected_provider.set("creem".to_string())
                            >
                                "Creem (国际卡/测试)"
                            </button>
                        </div>
                    </div>

                    {move || {
                        error.get().map(|msg| {
                            view! {
                                <p class="settings-error" role="alert" data-testid="topup-error">
                                    {msg}
                                </p>
                            }
                        })
                    }}

                    <div class="topup-action-row">
                        <button
                            type="button"
                            class="auth-submit-btn"
                            data-testid="btn-start-topup"
                            disabled=move || loading.get()
                            on:click=on_topup_checkout
                        >
                            {move || if loading.get() { "创建中…" } else { "立即充值" }}
                        </button>
                    </div>

                    {move || {
                        checkout_url.get().map(|url| {
                            view! {
                                <div class="topup-success-redirect" data-testid="checkout-redirect-box">
                                    <p>"结账链接已创建："</p>
                                    <a href=url class="dashboard-chat-link" target="_blank" rel="noreferrer" data-testid="pay-link">
                                        "点击前往支付网关（测试模式）→"
                                    </a>
                                </div>
                            }
                        })
                    }}
                </section>
            </main>
        </div>
    }
}
