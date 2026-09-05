use leptos::prelude::*;

#[component]
pub fn UsagePage() -> impl IntoView {
    view! {
        <div class="settings-shell" data-testid="usage-page">
            <header class="settings-header">
                <div class="settings-header-left">
                    <a href="/settings" class="settings-back-link">"← 返回设置"</a>
                    <h1 class="settings-title">"用量总览"</h1>
                </div>
            </header>
            <main class="settings-content">
                <section class="settings-panel">
                    <h2>"Token 与模型调用用量"</h2>
                    <p class="settings-panel-desc">
                        "查看本周期的账户 Token 消耗与各模型使用明细。"
                    </p>
                    <div class="settings-usage-cards">
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"总对话 Token"</span>
                            <span class="settings-usage-value">"0"</span>
                        </div>
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">"向量与检索"</span>
                            <span class="settings-usage-value">"0"</span>
                        </div>
                    </div>
                </section>
            </main>
        </div>
    }
}
