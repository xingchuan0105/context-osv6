use super::public_common::PublicSeoHead;
use super::public_content::{AGENT_DOC_MD, AGENT_DOC_UPDATED};
use leptos::prelude::*;
use web_sdk::render_assistant_markdown;

/// `/help/api-access/agents` Agent 可读接入文档（英文正文，zh/en 路由共用）。
/// 文档单一事实源：`assets/docs/api-access-for-agents.md`（编译期内嵌）。
#[component]
pub fn HelpAgentApiPage() -> impl IntoView {
    // 与 Next 一致：剥掉文档级 `#` 标题，避免双 H1（页面已有一个 H1）。
    let body_md = AGENT_DOC_MD
        .lines()
        .skip_while(|line| line.trim_start().starts_with('#') || line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let rendered = render_assistant_markdown(&body_md);

    view! {
        <PublicSeoHead
            title="Agent API 接入文档"
            description="面向 Agent 的 Context OS API 接入说明：MCP / HTTP 调用、工作区密钥与权限边界。"
            canonical="/help/api-access/agents"
            en_href=Some("/en/help/api-access/agents")
        />
        <main class="pub-shell" data-testid="help-agent-api-page">
            <div class="pub-center">
                <header class="pub-header">
                    <p class="pub-subtitle">"Agent-readable API access"</p>
                    <h1 class="pub-title">"Context OS Agent API"</h1>
                    <p class="pub-muted">"Product: Context OS · Brand: ContextLM · Operator: Xing Chuan"</p>
                    <p class="pub-muted">{format!("Page copy last updated: {AGENT_DOC_UPDATED}")}</p>
                    <div class="pub-btn-row">
                        <a href="/help/api-access" class="pub-btn">"Human guide"</a>
                        <a href="/help/faq" class="pub-btn">"FAQ"</a>
                        <a href="/help/compare" class="pub-btn">"Compare"</a>
                        <a href="/integrations" class="pub-btn">"Integrations"</a>
                        <a href="/help" class="pub-btn">"Help"</a>
                    </div>
                </header>
                <article
                    class="pub-card pub-doc-body"
                    data-testid="agent-api-doc-body"
                    inner_html=rendered
                ></article>
            </div>
        </main>
    }
}
