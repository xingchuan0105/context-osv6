use crate::api_base::poc_api_base;
use crate::components::shell::{MarketingChrome, ProductChromeFooter};
use crate::components::ui::AppDialog;
use crate::i18n::{UiLocale, interpolate, lookup, use_i18n};
use leptos::prelude::*;
use web_sdk::{
    BillingPlan, BrowserRestClient, CheckoutRequest, TopupPack, WalletBalanceResponse,
};

#[component]
pub fn PricingPage(#[prop(optional)] locale_override: Option<UiLocale>) -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let i18n = use_i18n();
    let loc = move || locale_override.unwrap_or_else(|| i18n.locale.get());
    let t = move |key: &'static str| lookup(loc(), key);
    let plans = RwSignal::new(Vec::<BillingPlan>::new());
    let wallet = RwSignal::new(None::<WalletBalanceResponse>);
    let topup_packs = RwSignal::new(Vec::<TopupPack>::new());
    let selected_pack = RwSignal::new(String::new());
    let selected_provider = RwSignal::new("alipay".to_string());
    let checkout_url = RwSignal::new(None::<String>);
    let error = RwSignal::new(None::<String>);
    let loading = RwSignal::new(false);
    let refresh = RwSignal::new(0_u64);
    let catalog_loading = RwSignal::new(true);
    let plans_failed = RwSignal::new(false);
    let interval = RwSignal::new("month".to_string());
    let agreed = RwSignal::new(false);
    let show_pay = RwSignal::new(false);

    Effect::new(move |_| {
        let generation = refresh.get();
        catalog_loading.set(true);
        error.set(None);
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token)
        } else {
            Some(token.get_untracked())
        };

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), tok.clone());
            match client.get_billing_plans().await {
                Ok(resp) => {
                    plans.set(resp.plans.into_iter().map(|mut plan| { plan.current |= resp.current_plan_id == plan.plan_id; plan }).collect());
                    plans_failed.set(false);
                }
                Err(_) => { plans.set(Vec::new()); plans_failed.set(true); },
            }
            if tok.is_some() {
                match client.get_wallet_balance().await {
                    Ok(w) => wallet.set(Some(w)),
                    Err(err) => { wallet.set(None); error.set(Some(err.to_string())); }
                }
                match client.list_topup_packs().await {
                    Ok(packs) => { selected_pack.set(packs.first().map(|p| p.pack_id.clone()).unwrap_or_default()); topup_packs.set(packs); },
                    Err(err) => { topup_packs.set(Vec::new()); error.set(Some(err.to_string())); }
                }
            }
            if refresh.try_get_untracked() == Some(generation) { catalog_loading.set(false); }
        });
    });

    let checkout = move |plan_id: Option<String>| {
        if loading.get_untracked() { return; }
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if tok.is_empty() {
            error.set(Some(t("pricing.loginRequired")));
            return;
        }
        if !agreed.get_untracked() {
            error.set(Some(t("pricing.agreeRequired")));
            return;
        }

        loading.set(true);
        error.set(None);
        let pack_id = selected_pack.get();
        let provider = selected_provider.get();
        let checkout_locale = loc();

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            let membership = plan_id.is_some();
            let req = CheckoutRequest {
                plan_id,
                provider: Some(provider),
                kind: Some(if membership { "subscription" } else { "wallet_topup" }.into()),
                topup_pack_id: if membership { None } else { Some(pack_id) },
            };
            match client.create_checkout_session(&req).await {
                Ok(resp) => {
                    checkout_url.set(Some(resp.url));
                    show_pay.set(true);
                }
                Err(err) => {
                    let msg = err.to_string();
                    error.set(Some(interpolate(
                        &lookup(checkout_locale, "pricing.checkoutFailed"),
                        &[("error", msg.as_str())],
                    )));
                }
            }
            loading.set(false);
        });
    };

    view! {
        <div class="pricing-shell" data-testid="pricing-page">
            <header class="pricing-header">
                <div class="pricing-header-left">
                    <h1 class="pricing-title">{move || t("pricingTitle")}</h1>
                    <p class="pricing-subtitle">{move || t("pricingSubtitle")}</p>
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
                        {move || t("pricingMonthly")}
                    </button>
                    <button
                        type="button"
                        class=move || if interval.get() == "year" { "topup-pack-btn is-active" } else { "topup-pack-btn" }
                        data-testid="interval-year"
                        on:click=move |_| interval.set("year".to_string())
                    >
                        {move || t("pricingYearly")}
                    </button>
                </div>
                <p class="settings-error" role="status" hidden=move || !plans_failed.get() data-testid="plans-fallback">
                    {move || t("pricing.catalogFailed")}
                </p>
                <p role="status" hidden=move || !catalog_loading.get()>{move || t("common.loading")}</p>
                <button type="button" data-testid="pricing-retry" disabled=move || catalog_loading.get() on:click=move |_| refresh.update(|n| *n += 1)>{move || t("common.retry")}</button>
                    <div class="topup-provider-selector">
                        <label class="topup-label">{move || t("pricingPayMethodLabel")}</label>
                        <div class="topup-providers-row">
                            <button
                                type="button"
                                class=move || if selected_provider.get() == "alipay" { "topup-provider-btn is-active" } else { "topup-provider-btn" }
                                data-testid="provider-alipay"
                                on:click=move |_| selected_provider.set("alipay".to_string())
                            >
                                {move || t("pricing.alipayButton")}
                            </button>
                            <button
                                type="button"
                                class=move || if selected_provider.get() == "creem" { "topup-provider-btn is-active" } else { "topup-provider-btn" }
                                data-testid="provider-creem"
                                on:click=move |_| selected_provider.set("creem".to_string())
                            >
                                {move || t("pricing.creemButton")}
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
                        {move || t("pricing.agreeLabel")}
                    </label>


                <section class="pricing-plans-section">
                    <div class="pricing-plans-grid" data-testid="plans-grid">
                        {move || {
                            let live = plans.get();
                            let selected = interval.get();
                            let cards: Vec<_> = live.into_iter()
                                .filter(|plan| plan.interval == selected || plan.plan_id == "free")
                                .map(|plan| (plan.plan_id, plan.name, plan.price_label_cny, plan.description, plan.current))
                                .collect();
                            cards
                                .into_iter()
                                .map(|(id, name, price, desc, current)| {
                                    let free = id == "free";
                                    let purchase_id = id.clone();
                                    let test_id = format!("plan-{id}");
                                    view! {
                                        <div
                                            class="pricing-plan-card"
                                            data-testid=test_id
                                        >
                                            <h2 class="pricing-plan-name">{name}</h2>
                                            <div class="pricing-plan-price">{price}</div>
                                            <p class="pricing-plan-desc">{desc}</p>
                                            <button type="button" data-testid="subscribe-plan" disabled=move || current || free || catalog_loading.get() aria-disabled=move || loading.get() on:click=move |_| checkout(Some(purchase_id.clone()))>{move || t(if current { "pricing.currentPlan" } else if free { "pricing.freePlan" } else if loading.get() { "common.saving" } else { "pricing.subscribe" })}</button>
                                        </div>
                                    }
                                })
                                .collect_view()
                        }}
                    </div>
                </section>

                <section id="topup" class="settings-panel pricing-topup-section" data-testid="topup-panel">
                    <header class="settings-panel-header">
                        <h2 class="settings-panel-title">{move || t("pricingTopupTitle")}</h2>
                        <p class="settings-panel-desc">
                            {move || t("pricingTopupBody")}
                        </p>
                    </header>

                    <div class="settings-usage-cards">
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">{move || t("pricingWalletBalance").replace("{balance}", "")}</span>
                            <span class="settings-usage-value" data-testid="wallet-balance">
                                {move || {
                                    wallet
                                        .get()
                                        .map(|w| format!("¥{:.2}", (w.balance_fen as f64) / 100.0))
                                        .unwrap_or_else(|| t("common.unknown"))
                                }}
                            </span>
                        </div>
                    </div>

                    <div class="topup-pack-selector">
                        <label class="topup-label">{move || t("pricingTopupPacksLabel")}</label>
                        <div class="topup-packs-row">
                            {move || {
                                let packs = topup_packs.get();
                                let items: Vec<_> = packs.into_iter().map(|pack| (pack.pack_id, pack.amount_yuan)).collect();
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

                    <div class="topup-action-row">
                        <button
                            type="button"
                            class="auth-submit-btn"
                            data-testid="btn-start-topup"
                            disabled=move || catalog_loading.get() || selected_pack.get().is_empty()
                            aria-disabled=move || loading.get()
                            on:click=move |_| checkout(None)
                        >
                            {move || if loading.get() { t("pricingTopupLoading") } else { t("pricing.topupNow") }}
                        </button>
                    </div>

                    <section class="pricing-faq" data-testid="pricing-faq">
                        <h2>{move || t("pricingFaqTitle")}</h2>
                        <details>
                            <summary>{move || t("pricing.faqBalance")}</summary>
                            <p>{move || t("pricing.faqBalanceAnswer")}</p>
                        </details>
                        <details>
                            <summary>{move || t("pricing.faqAnytime")}</summary>
                            <p>{move || t("pricing.faqAnytimeAnswer")}</p>
                        </details>
                    </section>

                    <AppDialog open=Signal::derive(move || show_pay.get()) title_key="pricing.payDialog" test_id="pay-qr-dialog" on_close=Callback::new(move |_| show_pay.set(false))>
                            {move || {
                                checkout_url.get().map(|url| {
                                    view! {
                                        <div class="topup-success-redirect" data-testid="checkout-redirect-box">
                                            <p data-testid="pay-qr">{t("pricing.payHint")}</p>
                                            <a href=url.clone() class="dashboard-chat-link" target="_blank" rel="noreferrer" data-testid="pay-link">
                                                {t("pricing.payLink")}
                                            </a>
                                        </div>
                                    }
                                })
                            }}
                    </AppDialog>
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
