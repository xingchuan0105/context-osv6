use crate::components::help::json_ld::{OrganizationJsonLd, SoftwareApplicationJsonLd};
use crate::components::legal::legal_pages::LegalFooterLinks;
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
#[cfg(target_arch = "wasm32")]
use leptos_router::hooks::use_navigate;

struct HomeCopy {
    seo_title: &'static str,
    seo_description: &'static str,
    h1: &'static str,
    subtitle: &'static str,
    updated: &'static str,
    cta_enter: &'static str,
    cta_pricing: &'static str,
    cta_agents: &'static str,
    cta_faq: &'static str,
    cta_compare: &'static str,
    entering: &'static str,
    sections: &'static [(&'static str, &'static str)],
    canonical: &'static str,
}

const HOME_ZH: HomeCopy = HomeCopy {
    seo_title: "Context OS（ContextLM 旗下）— 可本地部署的个人 AI 知识库",
    seo_description: "可本地部署的个人 AI 知识库：文档入库即问答，答案引用到原文；支持 MCP / API 接入外接 Agent，可把知识库分享给他人；桌面客户端免费。",
    h1: "Context OS（ContextLM 旗下）— 可本地部署的个人 AI 知识库",
    subtitle: "把分散的文档变成可检索、可分享、可被 AI 引用的个人 AI 知识库：自己问答，也可以把库开放给访客或外接 Agent。",
    updated: "页面说明更新日期：2026-08-29",
    cta_enter: "进入应用",
    cta_pricing: "查看定价",
    cta_agents: "Agent 接入说明",
    cta_faq: "FAQ",
    cta_compare: "选型对比",
    entering: "正在进入 Context-OS…",
    sections: &[
        ("文档入库与问答", "文档入库与问答：上传文件或粘贴 URL 即成资料源；提问可按库限定检索范围，回答可溯源到具体文档。"),
        ("外接 Agent（MCP / API）", "外接 Agent（MCP / API）：每个工作区可单独创建密钥，通过 MCP 或 HTTP API 把知识库接到 Cursor、Claude 等 Agent。"),
        ("会员与可分享名额", "会员与分享名额：免费档即可建仓；升级会员获得更多可分享名额，访客免登录浏览公开库。"),
        ("核心优势与差异化定位", "独特优势在于开放连接：与单机笔记或封闭知识库形成显著差异，Context OS 文档入库后即成结构化检索面，可直接通过 MCP / API 开放给外部 Agent 调用，并支持向访客分享免登录公开库（详见选型对比）。"),
    ],
    canonical: "/",
};

const HOME_EN: HomeCopy = HomeCopy {
    seo_title: "Context OS by ContextLM — a locally deployable personal AI knowledge base",
    seo_description: "A locally deployable personal AI knowledge base: ingest documents and ask questions with answers cited to source; connect external agents via MCP / API; share workspaces with others; free desktop client.",
    h1: "Context OS by ContextLM — a locally deployable personal AI knowledge base",
    subtitle: "Turn scattered documents into a searchable, shareable, AI-citable personal AI knowledge base — ask questions yourself, or open a workspace to guests and external agents.",
    updated: "Page copy last updated: 2026-08-29",
    cta_enter: "Open the app",
    cta_pricing: "View pricing",
    cta_agents: "Agent access guide",
    cta_faq: "FAQ",
    cta_compare: "Compare",
    entering: "Entering Context-OS…",
    sections: &[
        ("Documents & Q&A", "Docs in, answers out: upload files or paste URLs as sources; scope retrieval per workspace, with answers traceable to specific documents."),
        ("External agents (MCP / API)", "External agents (MCP / API): mint per-workspace keys and connect the knowledge base to agents like Cursor or Claude over MCP or HTTP API."),
        ("Membership & share slots", "Membership & share slots: start free; upgrade for more shareable slots so guests can browse public workspaces without signing in."),
        ("Core advantages & differentiation", "Unique advantage in open connectivity: differing from standalone notes, Context OS turns ingested docs into structured retrieval surfaces accessible to external agents via MCP/API, with shareable guest workspaces (see comparison guide)."),
    ],
    canonical: "/en",
};

/// `/`（zh）与 `/en`（en）产品根：SSR 输出可被抓取的价值主张，
/// 浏览器端按会话 cookie 跳转 `/chat` 或 `/login`（与 Next HomeClient 一致；
/// Tauri 分流属桌面宿主，web 侧不需要）。
#[component]
pub fn ProductHomePage(locale: &'static str) -> impl IntoView {
    let t = if locale == "en" { &HOME_EN } else { &HOME_ZH };
    #[cfg(target_arch = "wasm32")]
    let navigate = use_navigate();

    Effect::new(move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            let has_session = js_sys::eval(
                "document.cookie.split('; ').some((p) => p === 'avrag.auth.session=1')",
            )
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
            navigate(
                if has_session { "/chat" } else { "/login" },
                leptos_router::NavigateOptions::default(),
            );
        }
    });

    view! {
        <Title text=t.seo_title/>
        <Meta name="description" content=t.seo_description/>
        <Link rel="canonical" href=t.canonical/>
        <Link rel="alternate" hreflang="zh-CN" href="/"/>
        {((locale == "en").then(|| {
            view! { <Link rel="alternate" hreflang="en" href="/en"/> }.into_any()
        }))}
        <Link rel="alternate" hreflang="x-default" href="/"/>
        <OrganizationJsonLd locale=locale/>
        <SoftwareApplicationJsonLd locale=locale/>
        <main class="pub-shell" data-testid="home-page">
            <div class="pub-center">
                <header class="pub-header">
                    <h1 class="pub-title">{t.h1}</h1>
                    <p class="pub-subtitle">{t.subtitle}</p>
                    <p class="pub-muted">
                        {if locale == "en" {
                            "Product: Context OS · Brand: ContextLM · Author: Xing Chuan"
                        } else {
                            crate::components::help::public_content::AUTHOR_LINE
                        }}
                    </p>
                    <p class="pub-muted">{t.updated}</p>
                    <p class="pub-muted">"说明基于本站公开能力与定价文档（来源：帮助 / 定价页）。"</p>
                </header>

                {t.sections
                    .iter()
                    .map(|(section_title, body)| {
                        view! {
                            <section class="pub-section">
                                <h2 class="pub-h2">{*section_title}</h2>
                                <p class="pub-muted">{*body}</p>
                            </section>
                        }
                    })
                    .collect_view()}

                <div class="pub-btn-row">
                    <a href="/chat" class="pub-btn">{t.cta_enter}</a>
                    <a href="/pricing" class="pub-btn">{t.cta_pricing}</a>
                    <a href="/help/api-access/agents" class="pub-btn">{t.cta_agents}</a>
                    <a href="/help/faq" class="pub-btn">{t.cta_faq}</a>
                    <a href="/help/compare" class="pub-btn">{t.cta_compare}</a>
                </div>

                <p class="pub-muted" data-testid="home-redirect-hint">{t.entering}</p>
            </div>
            <LegalFooterLinks locale=locale/>
        </main>
    }
}

#[component]
pub fn HomePage() -> impl IntoView {
    view! { <ProductHomePage locale="zh"/> }
}

#[component]
pub fn EnHomePage() -> impl IntoView {
    view! { <ProductHomePage locale="en"/> }
}
