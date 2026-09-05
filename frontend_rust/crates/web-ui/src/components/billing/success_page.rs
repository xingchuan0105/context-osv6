use crate::api_base::poc_api_base;
use leptos::prelude::*;
use leptos_router::hooks::use_query_map;
use web_sdk::{BillingOrderStatusResponse, BrowserRestClient};

#[component]
pub fn UpgradeSuccessPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
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
            <main class="auth-card" aria-label="支付成功">
                <header class="auth-header">
                    <h1 class="auth-title">"支付成功！"</h1>
                    <p class="auth-subtitle">"感谢您对 Context-OS 的支持，您的会员权益或充值金额已到账。"</p>
                </header>
                <div class="success-order-details">
                    <Show when=move || order_id.get().is_some()>
                        <p class="success-order-id" data-testid="success-order-id">
                            {move || format!("订单号：{}", order_id.get().unwrap_or_default())}
                        </p>
                    </Show>
                    <div class="success-actions">
                        <a href="/chat" class="auth-submit-btn" data-testid="back-to-chat-btn">
                            "开始使用对话"
                        </a>
                        <a href="/dashboard" class="dashboard-chat-link">
                            "进入工作台"
                        </a>
                    </div>
                </div>
            </main>
        </div>
    }
}
