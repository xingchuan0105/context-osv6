use super::public_common::PublicSeoHead;
use super::public_content::{api_access_copy, author_line};
use leptos::prelude::*;

fn api_access_seo(locale: &str) -> (&'static str, &'static str, &'static str) {
    if locale == "en" {
        (
            "API access",
            "Context OS API access guide: each workspace manages its own keys; automated agents should use the agent docs and Agent Pack.",
            "/en/help/api-access",
        )
    } else {
        (
            "API 访问",
            "Context OS API 接入说明：每个工作区单独管理密钥；自动化代理（Agent）请使用 agent 文档与 Agent Pack。",
            "/help/api-access",
        )
    }
}

/// `/help/api-access`（zh）与 `/en/help/api-access`（en）公开人类接入说明。
#[component]
pub fn HelpApiAccessPageView(locale: &'static str) -> impl IntoView {
    let c = api_access_copy(locale);
    let (seo_title, seo_description, canonical) = api_access_seo(locale);

    view! {
        <PublicSeoHead
    locale=locale
            title=seo_title
            description=seo_description
            canonical=canonical
            zh_href="/help/api-access"
            en_href=Some("/en/help/api-access")
        />
        <main class="pub-shell pub-shell-wide" data-testid=if locale == "en" { "help-api-access-page-en" } else { "help-api-access-page" }>
            <div class="pub-center">
                <header class="pub-header">
                    <div class="pub-header-row">
                        <div>
                            <h1 class="pub-title">{c.title}</h1>
                            <p class="pub-subtitle">{c.subtitle}</p>
                            <p class="pub-muted">{author_line(locale)}</p>
                            <p class="pub-muted">{c.updated}</p>
                            <p class="pub-muted">{c.evidence_line}</p>
                        </div>
                        <div class="pub-btn-row">
                            <a href="/help" class="pub-btn">{c.back_help}</a>
                            <a href="/help/api-access/agents" class="pub-btn">{c.agent_docs}</a>
                            <a href="/help/faq" class="pub-btn">{c.cta_faq}</a>
                            <a href="/help/compare" class="pub-btn">{c.cta_compare}</a>
                            <a href="/integrations" class="pub-btn">{c.cta_integrations}</a>
                        </div>
                    </div>
                </header>

                <section class="pub-card" data-testid="api-access-overview">
                    <h2 class="pub-h2">{c.overview_title}</h2>
                    <ul class="pub-list">
                        {c.overview_items
                            .iter()
                            .map(|item| view! { <li>{*item}</li> })
                            .collect_view()}
                    </ul>
                </section>

                <section class="pub-card" data-testid="api-access-automation">
                    <h2 class="pub-h2">{c.automation_title}</h2>
                    <p class="pub-p">{c.automation_body}</p>
                    <ol class="pub-list">
                        {c.automation_steps
                            .iter()
                            .map(|step| view! { <li>{*step}</li> })
                            .collect_view()}
                    </ol>
                    <div class="pub-btn-row">
                        <a href="/help/api-access/agents" class="pub-btn">{c.agent_docs}</a>
                    </div>
                </section>
            </div>
        </main>
    }
}

#[component]
pub fn HelpApiAccessPage() -> impl IntoView {
    view! { <HelpApiAccessPageView locale="zh"/> }
}

#[component]
pub fn EnHelpApiAccessPage() -> impl IntoView {
    view! { <HelpApiAccessPageView locale="en"/> }
}
