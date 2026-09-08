use crate::i18n::{UiLocale, lookup};
use crate::routes::dest;
use leptos::prelude::*;
use leptos_router::hooks::use_location;

#[component]
pub fn MarketingChrome(
    #[prop(default = "zh")] locale: &'static str,
    #[prop(default = "none")] active: &'static str,
) -> impl IntoView {
    let location = use_location();
    let en = locale == "en";
    let loc = if en { UiLocale::En } else { UiLocale::ZhCn };
    let t = move |key: &'static str| lookup(loc, key);
    let lang_zh = Signal::derive(move || {
        let path = location.pathname.get();
        if path == "/en" {
            "/".to_string()
        } else if let Some(rest) = path.strip_prefix("/en/") {
            format!("/{rest}")
        } else {
            path
        }
    });
    let lang_en = Signal::derive(move || {
        let path = location.pathname.get();
        if path == "/" || path.is_empty() {
            "/en".to_string()
        } else if path.starts_with("/en/") || path == "/en" {
            path
        } else {
            format!("/en{path}")
        }
    });

    view! {
        <header class="mkt-chrome" data-testid="marketing-chrome">
            <div class="mkt-chrome-inner">
                <a class="app-top-bar-brand" href=dest::CHAT data-testid="mkt-brand-lockup">
                    <img
                        class="app-top-bar-mark"
                        src="/brand/context-os-mark.svg"
                        alt=""
                        width="28"
                        height="28"
                    />
                    <span class="app-top-bar-wordmark">"Context-OS"</span>
                </a>
                <nav class="mkt-chrome-nav" aria-label=t("marketingChrome.navAria")>
                    <a
                        href=if en { "/en/pricing" } else { dest::PRICING }
                        class=if active == "pricing" { "mkt-nav-link is-active" } else { "mkt-nav-link" }
                        data-testid="mkt-nav-pricing"
                    >
                        {t("productChrome.pricing")}
                    </a>
                    <a
                        href=if en { "/en/desktop" } else { dest::DESKTOP }
                        class=if active == "desktop" { "mkt-nav-link is-active" } else { "mkt-nav-link" }
                        data-testid="mkt-nav-desktop"
                    >
                        {t("productChrome.client")}
                    </a>
                    <a
                        href=if en { "/en/legal" } else { dest::LEGAL }
                        class=if active == "legal" { "mkt-nav-link is-active" } else { "mkt-nav-link" }
                        data-testid="mkt-nav-legal"
                    >
                        {t("marketingChrome.legal")}
                    </a>
                    <span class="mkt-lang" role="group" aria-label="Language">
                        <a
                            href=move || lang_zh.get()
                            class=if !en { "mkt-lang-btn is-active" } else { "mkt-lang-btn" }
                            data-testid="mkt-lang-zh-CN"
                        >
                            {t("workspaceLanguageChinese")}
                        </a>
                        <a
                            href=move || lang_en.get()
                            class=if en { "mkt-lang-btn is-active" } else { "mkt-lang-btn" }
                            data-testid="mkt-lang-en"
                        >
                            "EN"
                        </a>
                    </span>
                    <a
                        class="mkt-nav-enter"
                        href=dest::CHAT
                        data-testid="mkt-nav-enter-app"
                    >
                        {t("marketingChrome.enterApp")}
                    </a>
                </nav>
            </div>
        </header>
    }
}
