use crate::routes::dest;
use leptos::prelude::*;

#[component]
pub fn ProductChromeFooter() -> impl IntoView {
    view! {
        <footer class="product-chrome-footer" data-testid="product-chrome-footer">
            <nav class="product-chrome-footer-nav" aria-label="产品页脚">
                <a href="https://www.contextlm.top" rel="noopener noreferrer">"品牌首页"</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::DASHBOARD>"工作台"</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::HELP>"帮助"</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::PRICING>"定价"</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::TOPUP>"充值"</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::DESKTOP data-testid="product-chrome-desktop">"客户端"</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::LEGAL>"法律中心"</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::LEGAL_TERMS>"用户协议"</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::LEGAL_PRIVACY>"隐私政策"</a>
                <span aria-hidden="true">"·"</span>
                <a href=dest::LEGAL_LICENSES>"开源声明"</a>
            </nav>
            <div>"© 2026 Context-OS"</div>
        </footer>
    }
}
