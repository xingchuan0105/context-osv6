use super::public_common::PublicSeoHead;
use super::public_content::API_ACCESS;
use leptos::prelude::*;

/// `/help/api-access` 公开人类接入说明（zh-CN）。密钥创建/撤销仍在工作区分享中心。
#[component]
pub fn HelpApiAccessPage() -> impl IntoView {
    view! {
        <PublicSeoHead
            title="API 访问"
            description="Context OS API 接入说明：每个工作区单独管理密钥；自动化代理（Agent）请使用 agent 文档与 Agent Pack。"
            canonical="/help/api-access"
            en_href=Some("/en/help/api-access")
        />
        <main class="pub-shell pub-shell-wide" data-testid="help-api-access-page">
            <div class="pub-center">
                <header class="pub-header">
                    <div class="pub-header-row">
                        <div>
                            <h1 class="pub-title">{API_ACCESS.title}</h1>
                            <p class="pub-subtitle">{API_ACCESS.subtitle}</p>
                            <p class="pub-muted">
                                {crate::components::help::public_content::AUTHOR_LINE}
                            </p>
                            <p class="pub-muted">{API_ACCESS.updated}</p>
                            <p class="pub-muted">{API_ACCESS.evidence_line}</p>
                        </div>
                        <div class="pub-btn-row">
                            <a href="/help" class="pub-btn">{API_ACCESS.back_help}</a>
                            <a href="/help/api-access/agents" class="pub-btn">{API_ACCESS.agent_docs}</a>
                            <a href="/help/faq" class="pub-btn">{API_ACCESS.cta_faq}</a>
                            <a href="/help/compare" class="pub-btn">{API_ACCESS.cta_compare}</a>
                            <a href="/integrations" class="pub-btn">{API_ACCESS.cta_integrations}</a>
                        </div>
                    </div>
                </header>

                <section class="pub-card" data-testid="api-access-overview">
                    <h2 class="pub-h2">{API_ACCESS.overview_title}</h2>
                    <ul class="pub-list">
                        {API_ACCESS.overview_items
                            .iter()
                            .map(|item| view! { <li>{*item}</li> })
                            .collect_view()}
                    </ul>
                </section>

                <section class="pub-card" data-testid="api-access-automation">
                    <h2 class="pub-h2">{API_ACCESS.automation_title}</h2>
                    <p class="pub-p">{API_ACCESS.automation_body}</p>
                    <ol class="pub-list">
                        {API_ACCESS.automation_steps
                            .iter()
                            .map(|step| view! { <li>{*step}</li> })
                            .collect_view()}
                    </ol>
                    <div class="pub-btn-row">
                        <a href="/help/api-access/agents" class="pub-btn">{API_ACCESS.agent_docs}</a>
                    </div>
                </section>
            </div>
        </main>
    }
}
