use leptos::prelude::*;

/// 应用内帮助中心（Help ≠ primary nav：不进主导航，仅从弱入口进入）。
/// 仅链接已挂载的规范路由，不制造孤儿目的地。
#[component]
pub fn HelpPage() -> impl IntoView {
    view! {
        <div class="help-shell" data-testid="help-page">
            <header class="help-header">
                <a href="/chat" class="settings-back-link">"← 返回对话"</a>
                <h1 class="help-title">"帮助中心"</h1>
            </header>
            <main class="help-content">
                <section class="help-card" data-testid="help-card-write">
                    <h2 class="help-card-title">"长文与提示词编写"</h2>
                    <p class="help-card-desc">
                        "撰写长文的分段策略、上下文投喂方式，以及让检索与回答更稳定的提示词建议。"
                    </p>
                    <a href="/help/write" class="admin-panel-btn">"阅读指引 →"</a>
                </section>
                <section class="help-card" data-testid="help-card-api-access">
                    <h2 class="help-card-title">"API 接入"</h2>
                    <p class="help-card-desc">
                        "为工作区创建密钥，把知识库接进 Cursor / Claude 等外接 Agent（MCP）；Agent 可读文档与集成承接页互链。"
                    </p>
                    <a href="/help/api-access" class="admin-panel-btn">"查看接入说明 →"</a>
                </section>
                <section class="help-card" data-testid="help-card-providers">
                    <h2 class="help-card-title">"模型与自备密钥 (BYOK)"</h2>
                    <p class="help-card-desc">
                        "为对话、Agent 主模型、文档解析与向量检索配置你自己的 API Key；密钥由系统安全保存，可随时撤销。"
                    </p>
                    <a href="/settings?tab=providers" class="admin-panel-btn">"前往设置 →"</a>
                </section>
                <section class="help-card" data-testid="help-card-pricing">
                    <h2 class="help-card-title">"套餐与钱包充值"</h2>
                    <p class="help-card-desc">
                        "对比各套餐的能力差异与配额，使用钱包余额完成充值与订阅支付。"
                    </p>
                    <a href="/pricing" class="admin-panel-btn">"查看套餐 →"</a>
                </section>
                <section class="help-card" data-testid="help-card-desktop">
                    <h2 class="help-card-title">"桌面客户端"</h2>
                    <p class="help-card-desc">
                        "下载并登录桌面客户端，获得本地文件托管与系统级快捷入口。"
                    </p>
                    <a href="/desktop/buy" class="admin-panel-btn">"获取客户端 →"</a>
                </section>
                <section class="help-card" data-testid="help-card-dashboard">
                    <h2 class="help-card-title">"工作区与知识库"</h2>
                    <p class="help-card-desc">
                        "创建工作区、上传持久资料与笔记，并通过分享中心生成公开只读链接。"
                    </p>
                    <a href="/dashboard" class="admin-panel-btn">"打开工作台 →"</a>
                </section>
            </main>
        </div>
    }
}
