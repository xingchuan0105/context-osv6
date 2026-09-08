use crate::api_base::poc_api_base;
use crate::components::shell::{MarketingChrome, ProductChromeFooter};
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
    let plans_failed = RwSignal::new(false);
    let interval = RwSignal::new("month".to_string());
    let agreed = RwSignal::new(false);
    let show_pay = RwSignal::new(false);

    Effect::new(move |_| {
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token)
        } else {
            Some(token.get_untracked())
        };

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), tok.clone());
            match client.get_billing_plans().await {
                Ok(resp) => {
                    plans.set(resp.plans);
                    plans_failed.set(false);
                }
                Err(_) => plans_failed.set(true),
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
        if !agreed.get_untracked() {
            error.set(Some("请先同意服务条款后再充值。".to_string()));
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
                    show_pay.set(true);
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
                    <h1 class="pricing-title">"套餐定价与会员权益"</h1>
                    <p class="pricing-subtitle">"选择适合您的生产力方案，支持随时充值或升降级"</p>
                </div>
            </header>

            <main class="pricing-content">
                <div class="pricing-interval" data-testid="interval-toggle">
                    <button
                        type="button"
                        class=move || if interval.get() == "month" { "topup-pack-btn is-active" } else { "topup-pack-btn" }
                        data-testid="interval-month"
                        on:click=move |_| interval.set("month".to_string())
                    >
                        "按月"
                    </button>
                    <button
                        type="button"
                        class=move || if interval.get() == "year" { "topup-pack-btn is-active" } else { "topup-pack-btn" }
                        data-testid="interval-year"
                        on:click=move |_| interval.set("year".to_string())
                    >
                        "按年"
                    </button>
                </div>
                <p class="settings-error" role="status" hidden=move || !plans_failed.get() data-testid="plans-fallback">
                    "套餐接口暂时不可用，以下为营销说明。"
                </p>
                <section class="pricing-plans-section">
                    <div class="pricing-plans-grid" data-testid="plans-grid">
                        {move || {
                            let live = plans.get();
                            let selected = interval.get();
                            let cards = if live.is_empty() {
                                vec![
                                    ("free".into(), "免费体验版".into(), "¥0".into(), "适合个人基础问答与轻量体验".into(), false),
                                    ("pro".into(), "Pro 专业版".into(), if selected == "year" { "¥999/年".into() } else { "¥99/月".into() }, "深度知识库构建、无限持久资料与高速 Agent".into(), true),
                                ]
                            } else {
                                live.into_iter()
                                    .filter(|plan| plan.interval == selected || plan.plan_id == "free")
                                    .map(|plan| {
                                        let popular = plan.plan_id == "pro";
                                        (plan.plan_id, plan.name, plan.price_label_cny, plan.description, popular)
                                    })
                                    .collect()
                            };
                            cards
                                .into_iter()
                                .map(|(id, name, price, desc, popular)| {
                                    let test_id = format!("plan-{id}");
                                    view! {
                                        <div
                                            class=if popular { "pricing-plan-card is-popular" } else { "pricing-plan-card" }
                                            data-testid=test_id
                                        >
                                            <Show when=move || popular>
                                                <span class="pricing-popular-badge">"推荐"</span>
                                            </Show>
                                            <h2 class="pricing-plan-name">{name}</h2>
                                            <div class="pricing-plan-price">{price}</div>
                                            <p class="pricing-plan-desc">{desc}</p>
                                        </div>
                                    }
                                })
                                .collect_view()
                        }}
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
                            {move || {
                                let packs = topup_packs.get();
                                let items = if packs.is_empty() {
                                    vec![
                                        ("topup_50".into(), 50_i64),
                                        ("topup_100".into(), 100),
                                        ("topup_200".into(), 200),
                                    ]
                                } else {
                                    packs
                                        .into_iter()
                                        .map(|pack| (pack.pack_id, pack.amount_yuan))
                                        .collect()
                                };
                                items
                                    .into_iter()
                                    .map(|(id, yuan)| {
                                        let id_click = id.clone();
                                        let test_id = format!("pack-{yuan}");
                                        view! {
                                            <button
                                                type="button"
                                                class=move || {
                                                    if selected_pack.get() == id {
                                                        "topup-pack-btn is-active"
                                                    } else {
                                                        "topup-pack-btn"
                                                    }
                                                }
                                                data-testid=test_id
                                                on:click=move |_| selected_pack.set(id_click.clone())
                                            >
                                                {format!("¥{yuan}")}
                                            </button>
                                        }
                                    })
                                    .collect_view()
                            }}
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

                    <label class="pricing-agree">
                        <input
                            type="checkbox"
                            data-testid="pricing-agree"
                            prop:checked=move || agreed.get()
                            on:change=move |ev| {
                                let checked = event_target_checked(&ev);
                                agreed.set(checked);
                            }
                        />
                        " 我已阅读并同意用户服务协议与充值说明"
                    </label>

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

                    <section class="pricing-faq" data-testid="pricing-faq">
                        <h2>"常见问题"</h2>
                        <details>
                            <summary>"余额用在哪里？"</summary>
                            <p>"平台模型推理、文档解析与网络搜索按量扣费。"</p>
                        </details>
                        <details>
                            <summary>"可以随时充值吗？"</summary>
                            <p>"可以。充值完成后余额立即到账。"</p>
                        </details>
                    </section>

                    <div class="app-dialog-backdrop" hidden=move || !show_pay.get()>
                        <div class="app-dialog" role="dialog" aria-label="扫码支付" data-testid="pay-qr-dialog">
                            <header class="app-dialog-header">
                                <h2>"扫码支付"</h2>
                                <button type="button" class="app-dialog-close" on:click=move |_| show_pay.set(false)>"关闭"</button>
                            </header>
                            {move || {
                                checkout_url.get().map(|url| {
                                    view! {
                                        <div class="topup-success-redirect" data-testid="checkout-redirect-box">
                                            <p data-testid="pay-qr">"请使用所选支付方式完成付款。"</p>
                                            <a href=url.clone() class="dashboard-chat-link" target="_blank" rel="noreferrer" data-testid="pay-link">
                                                "点击前往支付网关（测试模式）→"
                                            </a>
                                        </div>
                                    }
                                })
                            }}
                        </div>
                    </div>
                </section>
            </main>
        </div>
    }
}

#[component]
pub fn ZhPricingPage() -> impl IntoView {
    view! {
        <MarketingChrome locale="zh" active="pricing"/>
        <PricingPage/>
        <ProductChromeFooter/>
    }
}
