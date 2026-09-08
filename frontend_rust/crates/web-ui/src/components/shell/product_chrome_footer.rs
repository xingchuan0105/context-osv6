use crate::i18n::use_i18n;
use crate::routes::dest;
use leptos::prelude::*;

#[component]
pub fn ProductChromeFooter() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <footer class="product-chrome-footer" data-testid="product-chrome-footer">
            <nav
                class="product-chrome-footer-nav"
                aria-label=move || i18n.t("productChrome.footerNavLabel")
            >
                <a href="https://www.contextlm.top" rel="noopener noreferrer">
                    {move || i18n.t("productChrome.brandHomeShort")}
                </a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::DASHBOARD>{move || i18n.t("productChrome.productHome")}</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::HELP>{move || i18n.t("productChrome.help")}</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::PRICING>{move || i18n.t("productChrome.pricing")}</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::TOPUP>{move || i18n.t("productChrome.topup")}</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::DESKTOP data-testid="product-chrome-desktop">
                    {move || i18n.t("productChrome.client")}
                </a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::LEGAL>{move || i18n.t("productChrome.legalCenter")}</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::LEGAL_TERMS>{move || i18n.t("productChrome.terms")}</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::LEGAL_PRIVACY>{move || i18n.t("productChrome.privacy")}</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::LEGAL_LICENSES>{move || i18n.t("productChrome.licenses")}</a>
            </nav>
            <div>"© 2026 Context-OS"</div>
        </footer>
    }
}
