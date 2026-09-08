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

    Effect::new(move |_| {
        let Some(oid) = order_id.get() else {
            return;
        };
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
            if let Ok(resp) = client.get_order_status(&oid).await {
                order_status.set(Some(resp));
            }
        });
    });

    view! {
        <div class="auth-page-container" data-testid="upgrade-success-page">
            <main class="auth-card" aria-label=move || i18n.t("upgradeSuccess.title")>
                <header class="auth-header">
                    <h1 class="auth-title">{move || i18n.t("upgradeSuccess.title")}</h1>
                    <p class="auth-subtitle">{move || i18n.t("upgradeSuccess.subtitle")}</p>
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
