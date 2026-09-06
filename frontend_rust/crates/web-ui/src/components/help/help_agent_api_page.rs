use super::public_common::PublicSeoHead;
use super::public_content::{AGENT_DOC_MD, AGENT_DOC_UPDATED};
use leptos::prelude::*;
use web_sdk::render_assistant_markdown;

/// `/help/api-access/agents`（zh 路由）与 `/en/help/api-access/agents`（en 路由）：
/// 正文为英文（面向 Agent），两语言路由共用；canonical/hreflang 按 locale 切换。
#[component]
pub fn HelpAgentApiPageView(locale: &'static str) -> impl IntoView {
    // 与 Next 一致：剥掉文档级 `#` 标题，避免双 H1（页面已有一个 H1）。
    let body_md = AGENT_DOC_MD
        .lines()
        .skip_while(|line| line.trim_start().starts_with('#') || line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let rendered = render_assistant_markdown(&body_md);
    let (seo_title, seo_description, canonical) = if locale == "en" {
        (
            "Agent API access docs",
            "Agent-facing Context OS API access guide: MCP / HTTP calls, workspace keys, and permission boundaries.",
            "/en/help/api-access/agents",
        )
    } else {
        (
            "Agent API 接入文档",
            "面向 Agent 的 Context OS API 接入说明：MCP / HTTP 调用、工作区密钥与权限边界。",
            "/help/api-access/agents",
        )
    };

    view! {
        <PublicSeoHead
    locale=locale
            title=seo_title
            description=seo_description
            canonical=canonical
            zh_href="/help/api-access/agents"
            en_href=Some("/en/help/api-access/agents")
        />
        <main class="pub-shell" data-testid=if locale == "en" { "help-agent-api-page-en" } else { "help-agent-api-page" }>
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

#[component]
pub fn HelpAgentApiPage() -> impl IntoView {
    view! { <HelpAgentApiPageView locale="zh"/> }
}

#[component]
pub fn EnHelpAgentApiPage() -> impl IntoView {
    view! { <HelpAgentApiPageView locale="en"/> }
}
