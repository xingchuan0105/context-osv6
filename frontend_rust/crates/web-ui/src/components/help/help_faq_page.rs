use super::json_ld::FaqPageJsonLd;
use super::public_common::{PublicPageHeader, PublicSeoHead};
use super::public_content::{FAQ, FAQ_UPDATED_LINE};
use leptos::prelude::*;

/// `/help/faq` 公开产品 FAQ（zh-CN）。canonical hreflang 结构对齐 Next metadata。
#[component]
pub fn HelpFaqPage() -> impl IntoView {
    view! {
        <PublicSeoHead
            title="Context OS 常见问题：MCP 接入、BYOK、分享名额与定价"
            description="Context OS 产品 FAQ：MCP / Agent 接入、工作区密钥边界、可分享名额、BYOK、会员与余额定价。"
            canonical="/help/faq"
            en_href=Some("/en/help/faq")
        />
        <FaqPageJsonLd
            items=FAQ.items
                .iter()
                .map(|item| (item.question.to_string(), item.answer.to_string()))
                .collect()
            locale="zh-CN"
        />
        <main class="pub-shell" data-testid="help-faq-page">
            <div class="pub-center">
                <PublicPageHeader
                    subtitle=FAQ.subtitle
                    title=FAQ.h1
                    updated=FAQ_UPDATED_LINE
                    buttons=view! {
                            <a href="/help/compare" class="pub-btn">{FAQ.button_compare}</a>
                            <a href="/help/api-access" class="pub-btn">{FAQ.button_api_access}</a>
                            <a href="/pricing" class="pub-btn">{FAQ.button_pricing}</a>
                            <a href="/desktop" class="pub-btn">{FAQ.button_desktop}</a>
                        }
                        .into_any()
                />
                <article class="pub-card" data-testid="help-faq-body">
                    {FAQ.items
                        .iter()
                        .map(|item| {
                            view! {
                                <section>
                                    <h2 class="pub-h2">{item.question}</h2>
                                    <p class="pub-p">{item.answer}</p>
                                </section>
                            }
                        })
                        .collect_view()}
                    <h2 class="pub-h2">{FAQ.evidence_title}</h2>
                    <ul class="pub-list">
                        <li><a href="/pricing">{FAQ.evidence_pricing}</a></li>
                        <li><a href="/help/api-access">{FAQ.evidence_api_access}</a></li>
                        <li><a href="/help/api-access/agents">{FAQ.evidence_agents}</a></li>
                        <li><a href="/help/compare">{FAQ.evidence_compare}</a></li>
                        <li><a href="/integrations">{FAQ.evidence_integrations}</a></li>
                    </ul>
                    <p class="pub-muted">
                        {FAQ.evidence_prefix}
                        <a href="/pricing">{FAQ.link_pricing}</a>
                        {" · "}
                        <a href="/help/api-access">{FAQ.link_api_access}</a>
                        {" · "}
                        <a href="/help/api-access/agents">{FAQ.link_agents}</a>
                        {FAQ.evidence_suffix}
                    </p>
                </article>
            </div>
        </main>
    }
}
