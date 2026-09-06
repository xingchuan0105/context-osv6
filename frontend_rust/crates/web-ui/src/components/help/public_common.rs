use super::json_ld::{OrganizationJsonLd, SoftwareApplicationJsonLd};
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};

/// 公开页 SEO 头（title / description / canonical / hreflang + (open) 布局的 JSON-LD）。
/// `zh_href` 是 zh-CN alternate（也作为 x-default，对齐 Next：x-default 恒指 zh 路径）；
/// `en_href` 为 `/en/*` 对应路由，无 en 页时传 None（集成族）。
#[component]
pub fn PublicSeoHead(
    locale: &'static str,
    title: &'static str,
    description: &'static str,
    canonical: &'static str,
    zh_href: &'static str,
    en_href: Option<&'static str>,
) -> impl IntoView {
    view! {
        <Title text=title/>
        <Meta name="description" content=description/>
        <Link rel="canonical" href=canonical/>
        <Link rel="alternate" hreflang="zh-CN" href=zh_href/>
        {en_href.map(|en| {
            view! { <Link rel="alternate" hreflang="en" href=en/> }
        })}
        <Link rel="alternate" hreflang="x-default" href=zh_href/>
        <OrganizationJsonLd locale=locale/>
        <SoftwareApplicationJsonLd locale=locale/>
    }
}

/// 公开页共用页眉（副标题 / H1 / 署名 / 更新日 / CTA 行），口径与 Next faq/compare/integrations 一致。
#[component]
pub fn PublicPageHeader(
    subtitle: &'static str,
    title: &'static str,
    updated: &'static str,
    author: &'static str,
    buttons: AnyView,
) -> impl IntoView {
    view! {
        <header class="pub-header">
            <p class="pub-subtitle">{subtitle}</p>
            <h1 class="pub-title">{title}</h1>
            <p class="pub-muted">{author}</p>
            <p class="pub-muted">{updated}</p>
            <div class="pub-btn-row">{buttons}</div>
        </header>
    }
}
