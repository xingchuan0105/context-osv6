use crate::components::help::public_content::{INTEGRATION_DOCS, INTEGRATIONS_SHARED, INTEGRATIONS_UPDATED};
use leptos::prelude::*;

/// `/integrations` 索引：三张承接卡 + 互链（help 族入口，非主 nav）。
#[component]
pub fn IntegrationIndexPage() -> impl IntoView {
    let shared = &INTEGRATIONS_SHARED;
    let updated = format!("{}{}", shared.updated_label, INTEGRATIONS_UPDATED);

    view! {
        <leptos_meta::Title text="把 Context OS 知识库接进 Cursor / Claude（MCP 集成）"/>
        <leptos_meta::Meta
            name="description"
            content="Context OS 集成承接：按客户端给出把工作区知识库接进 Cursor、Claude Desktop 与任意 MCP 客户端的可复现步骤；接入协议事实源见 Agent API 文档。"
        />
        <leptos_meta::Link rel="canonical" href="/integrations"/>
        <leptos_meta::Link rel="alternate" hreflang="zh-CN" href="/integrations"/>
        <leptos_meta::Link rel="alternate" hreflang="x-default" href="/integrations"/>
        <main class="pub-shell" data-testid="integrations-page">
            <div class="pub-center">
                <header class="pub-header">
                    <p class="pub-subtitle">{shared.index_subtitle}</p>
                    <h1 class="pub-title">{shared.index_h1}</h1>
                    <p class="pub-muted">{shared.author_line}</p>
                    <p class="pub-muted">{updated}</p>
                    <div class="pub-btn-row">
                        <a href="/help/api-access/agents" class="pub-btn">{shared.button_agents}</a>
                        <a href="/desktop/buy" class="pub-btn">{shared.button_desktop}</a>
                        <a href="/pricing" class="pub-btn">{shared.button_pricing}</a>
                    </div>
                </header>

                <div class="pub-card-grid">
                    {INTEGRATION_DOCS
                        .iter()
                        .map(|doc| {
                            let href = format!("/integrations/{}", doc.slug);
                            let test_id = format!("integrations-card-{}", doc.slug);
                            view! {
                                <a href=href class="pub-card pub-card-link" data-testid=test_id>
                                    <strong>{doc.card_title}</strong>
                                    <span class="pub-card-desc">{doc.card_desc}</span>
                                </a>
                            }
                        })
                        .collect_view()}
                </div>

                <article class="pub-card" data-testid="integrations-index-body">
                    {shared.index_intro
                        .iter()
                        .map(|text| {
                            view! { <p class="pub-p">{*text}</p> }
                        })
                        .collect_view()}
                    <p class="pub-muted">{shared.index_evidence_line}</p>

                    <h2 class="pub-h2">{shared.evidence_title}</h2>
                    <ul class="pub-list">
                        {shared.evidence_items
                            .iter()
                            .map(|(label, href)| {
                                view! {
                                    <li><a href=*href>{*label}</a></li>
                                }
                            })
                            .collect_view()}
                    </ul>
                </article>
            </div>
        </main>
    }
}
