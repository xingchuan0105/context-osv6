use super::public_common::{PublicPageHeader, PublicSeoHead};
use super::public_content::{author_line, compare_copy, compare_updated_line};
use leptos::prelude::*;

fn compare_seo(locale: &str) -> (&'static str, &'static str, &'static str) {
    if locale == "en" {
        (
            "AI knowledge base comparison",
            "How to choose a personal AI knowledge base? A neutral comparison of Context OS vs notes AI, second-brain apps, and general RAG stacks: use cases, sharing, external agents — no fabricated competitor data.",
            "/en/help/compare",
        )
    } else {
        (
            "AI 知识库工具对比",
            "个人 AI 知识库怎么选？Context OS 与笔记内置 AI、第二大脑应用、通用 RAG 套件的中立对比：适用场景、分享、外接 Agent，不编造竞品数据。",
            "/help/compare",
        )
    }
}

#[component]
pub fn HelpComparePageView(locale: &'static str) -> impl IntoView {
    let c = compare_copy(locale);
    let (seo_title, seo_description, canonical) = compare_seo(locale);

    view! {
        <PublicSeoHead
    locale=locale
            title=seo_title
            description=seo_description
            canonical=canonical
            zh_href="/help/compare"
            en_href=Some("/en/help/compare")
        />
        <main class="pub-shell" data-testid=if locale == "en" { "help-compare-page-en" } else { "help-compare-page" }>
            <div class="pub-center">
                <PublicPageHeader
                    subtitle=c.subtitle
                    title=c.h1
                    updated=compare_updated_line(locale)
                    author=author_line(locale)
                    buttons=view! {
                            <a href="/help/faq" class="pub-btn">{c.button_faq}</a>
                            <a href="/help/api-access/agents" class="pub-btn">{c.button_agents}</a>
                            <a href="/pricing" class="pub-btn">{c.button_pricing}</a>
                            <a href="/desktop" class="pub-btn">{c.button_desktop}</a>
                        }
                        .into_any()
                />
                <article class="pub-card" data-testid="help-compare-body">
                    <h2 class="pub-h2">{c.positioning_title}</h2>
                    <p class="pub-p">
                        {c.positioning
                            .iter()
                            .map(|item| {
                                let colon = if locale == "en" { ": " } else { "：" };
                                view! {
                                    <span>
                                        <strong>{item.label}</strong>
                                        {colon}
                                        {item.text}
                                        {"  "}
                                    </span>
                                }
                            })
                            .collect_view()}
                    </p>

                    <h2 class="pub-h2">{c.table_title}</h2>
                    <div class="pub-table-wrap">
                        <table class="pub-table" data-testid="help-compare-table">
                            <thead>
                                <tr>
                                    {c.table_headers
                                        .iter()
                                        .map(|header| {
                                            view! { <th class="pub-th">{*header}</th> }
                                        })
                                        .collect_view()}
                                </tr>
                            </thead>
                            <tbody>
                                {c.rows
                                    .iter()
                                    .map(|row| {
                                        view! {
                                            <tr>
                                                <td class="pub-td">{row.dim}</td>
                                                <td class="pub-td">{row.cos}</td>
                                                <td class="pub-td">{row.notes}</td>
                                                <td class="pub-td">{row.rag}</td>
                                            </tr>
                                        }
                                    })
                                    .collect_view()}
                            </tbody>
                        </table>
                    </div>

                    <h2 class="pub-h2">{c.fits_title}</h2>
                    <ul class="pub-list">
                        {c.fits.iter().map(|item| view! { <li>{*item}</li> }).collect_view()}
                    </ul>

                    <h2 class="pub-h2">{c.other_title}</h2>
                    <ul class="pub-list">
                        {c.other.iter().map(|item| view! { <li>{*item}</li> }).collect_view()}
                    </ul>

                    <h2 class="pub-h2">{c.no_claim_title}</h2>
                    <p class="pub-p">{c.no_claim}</p>

                    <h2 class="pub-h2">{c.next_title}</h2>
                    <ul class="pub-list">
                        <li>
                            {c.next_faq}
                            <a href="/help/faq">FAQ</a>
                        </li>
                        <li>
                            {c.next_agents}
                            <a href="/help/api-access/agents">Agent API</a>
                            {" · "}
                            <a href="/help/api-access">{c.next_api_access}</a>
                        </li>
                        <li>
                            {c.next_pricing}
                            <a href="/pricing">{c.link_pricing}</a>
                        </li>
                        <li>
                            {c.next_integrations}
                            <a href="/integrations">{c.next_integrations_label}</a>
                        </li>
                        <li>
                            {c.next_enter}
                            <a href="/chat">{c.next_enter_label}</a>
                            {" · "}
                            <a href="/register">{c.next_register}</a>
                        </li>
                    </ul>
                    <p class="pub-muted">
                        {c.evidence_text}
                        <a href="/pricing">{c.link_pricing}</a>
                        {" · "}
                        <a href="/help/api-access/agents">{c.link_agents}</a>
                        {" · "}
                        <a href="/help/faq">{c.link_faq}</a>
                        {"。"}
                    </p>
                </article>
            </div>
        </main>
    }
}

#[component]
pub fn HelpComparePage() -> impl IntoView {
    view! { <HelpComparePageView locale="zh"/> }
}

#[component]
pub fn EnHelpComparePage() -> impl IntoView {
    view! { <HelpComparePageView locale="en"/> }
}
