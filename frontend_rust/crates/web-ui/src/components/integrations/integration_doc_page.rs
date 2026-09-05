use crate::components::help::public_common::PublicSeoHead;
use crate::components::help::public_content::{integration_doc, INTEGRATIONS_SHARED};
use leptos::prelude::*;

/// `/integrations/{slug}` 承接文档页：只做需求承接 + 锚链，
/// 接入事实源是 /help/api-access/agents（api-access-for-agents.md）。
#[component]
pub fn IntegrationDocPage(slug: &'static str) -> impl IntoView {
    match integration_doc(slug) {
        Some(doc) => render_doc(doc).into_any(),
        None => view! {
            <main class="pub-shell" data-testid="integrations-unknown">
                <p class="pub-muted">"未找到该集成页。"</p>
                <a href="/integrations" class="pub-btn">"返回全部集成"</a>
            </main>
        }
        .into_any(),
    }
}

fn doc_seo_title(slug: &str) -> &'static str {
    match slug {
        "mcp" => "Context OS MCP 接入：把工作区知识库暴露给任意 MCP 客户端",
        "cursor" => "把 Context OS 知识库接进 Cursor（MCP）",
        "claude-desktop" => "把 Context OS 知识库接进 Claude Desktop（MCP）",
        _ => "Context OS 集成",
    }
}

fn doc_canonical(slug: &str) -> &'static str {
    match slug {
        "mcp" => "/integrations/mcp",
        "cursor" => "/integrations/cursor",
        "claude-desktop" => "/integrations/claude-desktop",
        _ => "/integrations",
    }
}

fn doc_seo_description(slug: &str) -> &'static str {
    match slug {
        "mcp" => "Context OS MCP 接入：统一 MCP HTTP 端点、工作区工具目录、鉴权边界与调用示例，任意 MCP 客户端从这里开始。",
        "cursor" => "把 Context OS 工作区知识库接进 Cursor：本地 stdio（context-os-mcp）与云端 MCP HTTP 两条路径的可复现步骤。",
        "claude-desktop" => "把 Context OS 工作区知识库接进 Claude Desktop：stdio 配置或远程 MCP 连接器均可，回答可溯源到库内文档。",
        _ => "Context OS 集成承接页。",
    }
}

fn render_doc(doc: &'static crate::components::help::public_content::IntegrationDoc) -> impl IntoView {
    let shared = &INTEGRATIONS_SHARED;
    let canonical = doc_canonical(doc.slug);
    let title = doc_seo_title(doc.slug);
    let description = doc_seo_description(doc.slug);

    view! {
        <PublicSeoHead title=title description=description canonical=canonical en_href=None/>
        <main class="pub-shell" data-testid="integrations-doc-page">
            <div class="pub-center">
                <header class="pub-header">
                    <p class="pub-subtitle">{doc.subtitle}</p>
                    <h1 class="pub-title">{doc.h1}</h1>
                    <p class="pub-muted">{shared.author_line}</p>
                    <div class="pub-btn-row">
                        <a href="/help/api-access/agents" class="pub-btn">{shared.button_agents}</a>
                        <a href="/desktop/buy" class="pub-btn">{shared.button_desktop}</a>
                        <a href="/pricing" class="pub-btn">{shared.button_pricing}</a>
                        <a href="/integrations" class="pub-btn">{shared.back_label}</a>
                    </div>
                </header>

                <article class="pub-card" data-testid="integrations-doc-body">
                    {doc.intro
                        .iter()
                        .map(|text| view! { <p class="pub-p">{*text}</p> })
                        .collect_view()}
                    {doc.sections
                        .iter()
                        .map(|section| {
                            view! {
                                <section>
                                    <h2 class="pub-h2">{section.h2}</h2>
                                    {section.paragraphs
                                        .iter()
                                        .map(|text| view! { <p class="pub-p">{*text}</p> })
                                        .collect_view()}
                                    {section.table.as_ref().map(|table| {
                                        view! {
                                            <div class="pub-table-wrap">
                                                <table class="pub-table">
                                                    <thead>
                                                        <tr>
                                                            {table.headers
                                                                .iter()
                                                                .map(|header| {
                                                                    view! { <th class="pub-th">{*header}</th> }
                                                                })
                                                                .collect_view()}
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {table.rows
                                                            .iter()
                                                            .map(|row| {
                                                                view! {
                                                                    <tr>
                                                                        {row.iter()
                                                                            .map(|cell| {
                                                                                view! { <td class="pub-td">{*cell}</td> }
                                                                            })
                                                                            .collect_view()}
                                                                    </tr>
                                                                }
                                                            })
                                                            .collect_view()}
                                                    </tbody>
                                                </table>
                                            </div>
                                        }
                                    })}
                                    {section.code.as_ref().map(|(label, text)| {
                                        view! {
                                            <div class="pub-code-block">
                                                <span class="pub-code-label">{*label}</span>
                                                <pre class="pub-code"><code>{*text}</code></pre>
                                            </div>
                                        }
                                    })}
                                    {(!section.bullets.is_empty()).then(|| {
                                        view! {
                                            <ul class="pub-list">
                                                {section.bullets
                                                    .iter()
                                                    .map(|item| view! { <li>{*item}</li> })
                                                    .collect_view()}
                                            </ul>
                                        }
                                    })}
                                </section>
                            }
                        })
                        .collect_view()}
                    <p class="pub-muted">
                        "接入协议的单一事实源："
                        <a href="/help/api-access/agents">"Agent API 文档"</a>
                        "；若页面与文档不一致，以文档为准。"
                    </p>
                </article>
            </div>
        </main>
    }
}

#[component]
pub fn IntegrationMcpPage() -> impl IntoView {
    view! { <IntegrationDocPage slug="mcp"/> }
}

#[component]
pub fn IntegrationCursorPage() -> impl IntoView {
    view! { <IntegrationDocPage slug="cursor"/> }
}

#[component]
pub fn IntegrationClaudeDesktopPage() -> impl IntoView {
    view! { <IntegrationDocPage slug="claude-desktop"/> }
}
