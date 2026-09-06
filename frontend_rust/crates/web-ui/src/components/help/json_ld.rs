use leptos::prelude::*;
use serde_json::json;

/// GEO 结构化数据（对齐 Next organization-jsonld / software-application-jsonld / faq-page-jsonld）。
/// script 正文用 inner_html 注入原始 JSON，避免文本转义破坏 JSON-LD。

fn script(json: String) -> AnyView {
    view! { <script type="application/ld+json" inner_html=json></script> }.into_any()
}

/// Organization + WebSite 实体标注。locale: "zh" | "en"。
#[component]
pub fn OrganizationJsonLd(locale: &'static str) -> impl IntoView {
    let language = if locale == "en" { "en" } else { "zh-CN" };
    let org = json!({
        "@context": "https://schema.org",
        "@type": "Organization",
        "name": "ContextLM",
        "alternateName": "Context OS",
        "url": "https://contextlm.top/",
        "sameAs": [
            "https://github.com/xingchuan0105",
            "https://x.com/cHuaNXiNg105",
            "https://blog.contextlm.top/",
            "https://app.contextlm.top/",
        ],
    });
    let website = json!({
        "@context": "https://schema.org",
        "@type": "WebSite",
        "name": "Context OS",
        "url": "https://app.contextlm.top/",
        "inLanguage": language,
        "publisher": {
            "@type": "Organization",
            "name": "ContextLM",
            "url": "https://contextlm.top/",
        },
    });
    view! {
        {script(org.to_string())}
        {script(website.to_string())}
    }
}

/// SoftwareApplication 实体标注。locale: "zh" | "en"。
#[component]
pub fn SoftwareApplicationJsonLd(locale: &'static str) -> impl IntoView {
    let (in_language, description, author_name, offer_note) = if locale == "en" {
        (
            "en",
            "Agentic knowledge workbench: a private knowledge base at the center, agents orchestrating multi-route retrieval, read-only conversational library sharing, and BYOK billing. A ContextLM product.",
            "Xing Chuan",
            "Free to register; pay-as-you-go BYOK or wallet top-up",
        )
    } else {
        (
            "zh-CN",
            "Agentic 知识工作台：以私有知识库为中心，Agent 调度多路检索，支持知识库只读对话式分享与 BYOK 计费。ContextLM 旗下产品。",
            "邢川",
            "免费注册；BYOK 按量自理或平台充值",
        )
    };
    let payload = json!({
        "@context": "https://schema.org",
        "@type": "SoftwareApplication",
        "name": "Context OS",
        "applicationCategory": "BusinessApplication",
        "operatingSystem": "Web, Windows",
        "url": "https://app.contextlm.top/",
        "inLanguage": in_language,
        "description": description,
        "publisher": {
            "@type": "Organization",
            "name": "ContextLM",
            "url": "https://contextlm.top/",
        },
        "author": {
            "@type": "Person",
            "name": author_name,
            "url": "https://contextlm.top/#studio",
        },
        "offers": {
            "@type": "Offer",
            "price": "0",
            "priceCurrency": "CNY",
            "description": offer_note,
        },
    });
    script(payload.to_string())
}

/// FAQPage 结构化数据（与页面可见问答同源）。
#[component]
pub fn FaqPageJsonLd(items: Vec<(String, String)>, locale: &'static str) -> impl IntoView {
    let main_entity: Vec<_> = items
        .into_iter()
        .map(|(question, answer)| {
            json!({
                "@type": "Question",
                "name": question,
                "acceptedAnswer": {
                    "@type": "Answer",
                    "text": answer,
                },
            })
        })
        .collect();
    let payload = json!({
        "@context": "https://schema.org",
        "@type": "FAQPage",
        "inLanguage": locale,
        "mainEntity": main_entity,
    });
    script(payload.to_string())
}
