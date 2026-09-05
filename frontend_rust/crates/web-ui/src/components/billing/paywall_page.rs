use leptos::prelude::*;

#[component]
pub fn PaywallPage() -> impl IntoView {
    view! {
        <div class="auth-page-container" data-testid="paywall-page">
            <main class="auth-card" aria-label="会员升级提示">
                <header class="auth-header">
                    <h1 class="auth-title">"需升级会员以继续使用"</h1>
                    <p class="auth-subtitle">"当前功能需要 Pro 专业版或可用钱包余额"</p>
                </header>
                <div class="paywall-body">
                    <p class="paywall-desc">
                        "您可能已超出免费体验额度，或正在访问受限的持久知识库高级能力。升级后可立享高速专属通道。"
                    </p>
                    <div class="paywall-actions">
                        <a href="/pricing" class="auth-submit-btn" data-testid="goto-pricing-btn">
                            "查看定价与升级套餐 →"
                        </a>
                        <a href="/pricing#topup" class="dashboard-chat-link" data-testid="goto-topup-btn">
                            "按需充值钱包余额"
                        </a>
                    </div>
                </div>
            </main>
        </div>
    }
}
