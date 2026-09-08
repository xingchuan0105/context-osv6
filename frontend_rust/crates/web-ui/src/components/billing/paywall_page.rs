use crate::i18n::use_i18n;
use leptos::prelude::*;

#[component]
pub fn PaywallPage() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="auth-page-container" data-testid="paywall-page">
            <main class="auth-card" aria-label=move || i18n.t("paywall.title")>
                <header class="auth-header">
                    <h1 class="auth-title">{move || i18n.t("paywall.title")}</h1>
                    <p class="auth-subtitle">{move || i18n.t("paywall.subtitle")}</p>
                </header>
                <div class="paywall-body">
                    <p class="paywall-desc">
                        {move || i18n.t("paywall.body")}
                    </p>
                    <div class="paywall-actions">
                        <a href="/pricing" class="auth-submit-btn" data-testid="goto-pricing-btn">
                            {move || i18n.t("paywall.pricingCta")}
                        </a>
                        <a href="/pricing#topup" class="dashboard-chat-link" data-testid="goto-topup-btn">
                            {move || i18n.t("paywall.topupCta")}
                        </a>
                    </div>
                </div>
            </main>
        </div>
    }
}
