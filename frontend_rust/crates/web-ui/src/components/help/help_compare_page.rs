use super::public_common::{PublicPageHeader, PublicSeoHead};
use super::public_content::{COMPARE, COMPARE_UPDATED_LINE};
use leptos::prelude::*;

/// `/help/compare` 公开中立选型对照（zh-CN）。
#[component]
pub fn HelpComparePage() -> impl IntoView {
    view! {
        <PublicSeoHead
            title="AI 知识库工具对比"
            description="个人 AI 知识库怎么选？Context OS 与笔记内置 AI、第二大脑应用、通用 RAG 套件的中立对比：适用场景、分享、外接 Agent，不编造竞品数据。"
            canonical="/help/compare"
            en_href=Some("/en/help/compare")
        />
        <main class="pub-shell" data-testid="help-compare-page">
            <div class="pub-center">
                <PublicPageHeader
                    subtitle=COMPARE.subtitle
                    title=COMPARE.h1
                    updated=COMPARE_UPDATED_LINE
                    buttons=view! {
                            <a href="/help/faq" class="pub-btn">{COMPARE.button_faq}</a>
                            <a href="/help/api-access/agents" class="pub-btn">{COMPARE.button_agents}</a>
                            <a href="/pricing" class="pub-btn">{COMPARE.button_pricing}</a>
                            <a href="/desktop" class="pub-btn">{COMPARE.button_desktop}</a>
                        }
                        .into_any()
                />
                <article class="pub-card" data-testid="help-compare-body">
                    <h2 class="pub-h2">{COMPARE.positioning_title}</h2>
                    <p class="pub-p">
                        {COMPARE.positioning
                            .iter()
                            .map(|item| {
                                view! {
                                    <span>
                                        <strong>{item.label}</strong>
                                        {"："}
                                        {item.text}
                                        {"  "}
                                    </span>
                                }
                            })
                            .collect_view()}
                    </p>

                    <h2 class="pub-h2">{COMPARE.table_title}</h2>
                    <div class="pub-table-wrap">
                        <table class="pub-table" data-testid="help-compare-table">
                            <thead>
                                <tr>
                                    {COMPARE.table_headers
                                        .iter()
                                        .map(|header| {
                                            view! { <th class="pub-th">{*header}</th> }
                                        })
                                        .collect_view()}
                                </tr>
                            </thead>
                            <tbody>
                                {COMPARE.rows
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

                    <h2 class="pub-h2">{COMPARE.fits_title}</h2>
                    <ul class="pub-list">
                        {COMPARE.fits.iter().map(|item| view! { <li>{*item}</li> }).collect_view()}
                    </ul>

                    <h2 class="pub-h2">{COMPARE.other_title}</h2>
                    <ul class="pub-list">
                        {COMPARE.other.iter().map(|item| view! { <li>{*item}</li> }).collect_view()}
                    </ul>

                    <h2 class="pub-h2">{COMPARE.no_claim_title}</h2>
                    <p class="pub-p">{COMPARE.no_claim}</p>

                    <h2 class="pub-h2">{COMPARE.next_title}</h2>
                    <ul class="pub-list">
                        <li>
                            {COMPARE.next_faq}
                            <a href="/help/faq">FAQ</a>
                        </li>
                        <li>
                            {COMPARE.next_agents}
                            <a href="/help/api-access/agents">Agent API</a>
                            {" · "}
                            <a href="/help/api-access">{COMPARE.next_api_access}</a>
                        </li>
                        <li>
                            {COMPARE.next_pricing}
                            <a href="/pricing">{COMPARE.link_pricing}</a>
                        </li>
                        <li>
                            {COMPARE.next_integrations}
                            <a href="/integrations">{COMPARE.next_integrations_label}</a>
                        </li>
                        <li>
                            {COMPARE.next_enter}
                            <a href="/chat">{COMPARE.next_enter_label}</a>
                            {" · "}
                            <a href="/register">{COMPARE.next_register}</a>
                        </li>
                    </ul>
                    <p class="pub-muted">
                        {COMPARE.evidence_text}
                        <a href="/pricing">{COMPARE.link_pricing}</a>
                        {" · "}
                        <a href="/help/api-access/agents">{COMPARE.link_agents}</a>
                        {" · "}
                        <a href="/help/faq">{COMPARE.link_faq}</a>
                        {"。"}
                    </p>
                </article>
            </div>
        </main>
    }
}
