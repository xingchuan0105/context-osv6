use super::json_ld::FaqPageJsonLd;
use super::public_common::{PublicPageHeader, PublicSeoHead};
use super::public_content::{author_line, faq_copy, faq_updated_line};
use leptos::prelude::*;

fn faq_seo(locale: &str) -> (&'static str, &'static str, &'static str) {
    if locale == "en" {
        (
            "Context OS FAQ: MCP access, BYOK, share slots & pricing",
            "Context OS product FAQ: MCP / agent access, workspace key boundaries, share slots, BYOK, membership and wallet pricing.",
            "/en/help/faq",
        )
    } else {
        (
            "Context OS 常见问题：MCP 接入、BYOK、分享名额与定价",
            "Context OS 产品 FAQ：MCP / Agent 接入、工作区密钥边界、可分享名额、BYOK、会员与余额定价。",
            "/help/faq",
        )
    }
}

#[component]
pub fn HelpFaqPageView(locale: &'static str) -> impl IntoView {
    let c = faq_copy(locale);
    let (seo_title, seo_description, canonical) = faq_seo(locale);

    view! {
        <PublicSeoHead
    locale=locale
            title=seo_title
            description=seo_description
            canonical=canonical
            zh_href="/help/faq"
            en_href=Some("/en/help/faq")
        />
        <FaqPageJsonLd
            items=c.items
                .iter()
                .map(|item| (item.question.to_string(), item.answer.to_string()))
                .collect()
            locale=if locale == "en" { "en" } else { "zh-CN" }
        />
        <main class="pub-shell" data-testid=if locale == "en" { "help-faq-page-en" } else { "help-faq-page" }>
            <div class="pub-center">
                <PublicPageHeader
                    subtitle=c.subtitle
                    title=c.h1
                    updated=faq_updated_line(locale)
                    author=author_line(locale)
                    buttons=view! {
                            <a href="/help/compare" class="pub-btn">{c.button_compare}</a>
                            <a href="/help/api-access" class="pub-btn">{c.button_api_access}</a>
                            <a href="/pricing" class="pub-btn">{c.button_pricing}</a>
                            <a href="/desktop" class="pub-btn">{c.button_desktop}</a>
                        }
                        .into_any()
                />
                <article class="pub-card" data-testid="help-faq-body">
                    {c.items
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
                    <h2 class="pub-h2">{c.evidence_title}</h2>
                    <ul class="pub-list">
                        <li><a href="/pricing">{c.evidence_pricing}</a></li>
                        <li><a href="/help/api-access">{c.evidence_api_access}</a></li>
                        <li><a href="/help/api-access/agents">{c.evidence_agents}</a></li>
                        <li><a href="/help/compare">{c.evidence_compare}</a></li>
                        <li><a href="/integrations">{c.evidence_integrations}</a></li>
                    </ul>
                    <p class="pub-muted">
                        {c.evidence_prefix}
                        <a href="/pricing">{c.link_pricing}</a>
                        {" · "}
                        <a href="/help/api-access">{c.link_api_access}</a>
                        {" · "}
                        <a href="/help/api-access/agents">{c.link_agents}</a>
                        {c.evidence_suffix}
                    </p>
                </article>
            </div>
        </main>
    }
}

#[component]
pub fn HelpFaqPage() -> impl IntoView {
    view! { <HelpFaqPageView locale="zh"/> }
}

#[component]
pub fn EnHelpFaqPage() -> impl IntoView {
    view! { <HelpFaqPageView locale="en"/> }
}
