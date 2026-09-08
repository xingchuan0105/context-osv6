use crate::i18n::use_i18n;
use leptos::prelude::*;

#[component]
pub fn DesktopBuyPage() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="pricing-shell" data-testid="desktop-buy-page">
            <header class="pricing-header">
                <div class="pricing-header-left">
                    <a href="/desktop" class="settings-back-link">{move || i18n.t("desktopBuy.back")}</a>
                    <h1 class="pricing-title">{move || i18n.t("desktopBuy.title")}</h1>
                    <p class="pricing-subtitle">{move || i18n.t("desktopBuy.subtitle")}</p>
                </div>
            </header>

            <main class="pricing-content">
                <section class="settings-panel">
                    <div class="desktop-buy-info">
                        <h2>{move || i18n.t("desktopBuy.freeTitle")}</h2>
                        <p class="settings-panel-desc">
                            {move || i18n.t("desktopBuy.freeBody")}
                        </p>
                        <div class="desktop-buy-actions">
                            <a href="/pricing" class="auth-submit-btn" data-testid="goto-cloud-pricing">
                                {move || i18n.t("desktopBuy.upgradeCta")}
                            </a>
                            <a href="/desktop" class="dashboard-chat-link">
                                {move || i18n.t("desktopBuy.downloadCta")}
                            </a>
                        </div>
                    </div>
                </section>
            </main>
        </div>
    }
}
