use leptos::prelude::*;

#[component]
pub fn DesktopBuyPage() -> impl IntoView {
    view! {
        <div class="pricing-shell" data-testid="desktop-buy-page">
            <header class="pricing-header">
                <div class="pricing-header-left">
                    <a href="/desktop" class="settings-back-link">"← 返回客户端页面"</a>
                    <h1 class="pricing-title">"Context-OS 桌面客户端购买与授权"</h1>
                    <p class="pricing-subtitle">"独立桌面版本，原生流畅体验，支持本地或私有算力"</p>
                </div>
            </header>

            <main class="pricing-content">
                <section class="settings-panel">
                    <div class="desktop-buy-info">
                        <h2>"客户端免费开放使用"</h2>
                        <p class="settings-panel-desc">
                            "根据最新 ADR-0010 规范，Context-OS 桌面客户端已面向所有用户免费开放下载与连接，无需额外激活授权卡。云端高级模型服务可通过按量充值或订阅 Pro 会员解锁。"
                        </p>
                        <div class="desktop-buy-actions">
                            <a href="/pricing" class="auth-submit-btn" data-testid="goto-cloud-pricing">
                                "升级云端会员 / 充值 →"
                            </a>
                            <a href="/desktop" class="dashboard-chat-link">
                                "下载桌面安装包"
                            </a>
                        </div>
                    </div>
                </section>
            </main>
        </div>
    }
}
