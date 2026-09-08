use crate::api_base::poc_api_base;
use crate::i18n::use_i18n;
use leptos::prelude::*;
use leptos_router::hooks::use_query_map;
use web_sdk::{BillingOrderStatusResponse, BrowserRestClient};

#[component]
pub fn UpgradeSuccessPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let i18n = use_i18n();
    let query_map = use_query_map();
    let order_id = Signal::derive(move || query_map.with(|q| q.get("order_id").or_else(|| q.get("session_id"))));
    let order_status = RwSignal::new(None::<BillingOrderStatusResponse>);
    let loading = RwSignal::new(false);
    let error = RwSignal::new(false);
    let refresh = RwSignal::new(0_u64);
    let request_generation = RwSignal::new(0_u64);

    Effect::new(move |_| {
        let _ = refresh.get();
        let generation = request_generation.get_untracked() + 1;
        request_generation.set(generation);
        let tok = token.get();
        order_status.set(None);
        error.set(false);
        loading.set(false);
        let Some(oid) = order_id.get() else {
            return;
        };
        let tok = if tok.is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            tok
        };
        if tok.is_empty() {
            error.set(true);
            return;
        }
        loading.set(true);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            let result = client.get_order_status(&oid).await;
            if request_generation.get_untracked() != generation {
                return;
            }
            match result {
                Ok(resp) if resp.order_id == oid => order_status.set(Some(resp)),
                _ => error.set(true),
            }
            loading.set(false);
        });
    });

    let status_key = move || {
        if order_id.get().is_none() { "payment.missing" }
        else if loading.get() { "payment.checking" }
        else if error.get() { "payment.error" }
        else { match order_status.get().as_ref().map(|s| s.status.as_str()) {
            Some("paid") => "payment.paid",
            Some("pending") => "payment.pending",
            Some("failed" | "cancelled" | "canceled" | "expired") => "payment.failed",
            _ => "payment.unknown",
        }}
    };

    view! {
        <div class="auth-page-container" data-testid="upgrade-success-page">
            <main class="auth-card" aria-label=move || i18n.t("payment.title")>
                <header class="auth-header">
                    <h1 class="auth-title">{move || i18n.t("payment.title")}</h1>
                    <p class="auth-subtitle" role="status" data-testid="payment-status">{move || i18n.t(status_key())}</p>
                </header>
                <div class="success-order-details">
                    <Show when=move || order_id.get().is_some()>
                        <p class="success-order-id" data-testid="success-order-id">
                            {move || {
                                let id = order_id.get().unwrap_or_default();
                                i18n.tf("upgradeSuccess.order", &[("id", id.as_str())])
                            }}
                        </p>
                    </Show>
                    <div class="success-actions">
                        <Show when=move || order_id.get().is_some()>
                            <button type="button" data-testid="payment-refresh" disabled=move || loading.get()
                                on:click=move |_| refresh.update(|n| *n += 1)>{move || i18n.t("payment.refresh")}</button>
                        </Show>
                        <a href="/chat" class="auth-submit-btn" data-testid="back-to-chat-btn">
                            {move || i18n.t("upgradeSuccess.chatCta")}
                        </a>
                        <a href="/dashboard" class="dashboard-chat-link">
                            {move || i18n.t("upgradeSuccess.dashboardCta")}
                        </a>
                    </div>
                </div>
            </main>
        </div>
    }
}
