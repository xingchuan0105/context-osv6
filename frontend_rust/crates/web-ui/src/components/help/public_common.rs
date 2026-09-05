use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};

/// 公开页 SEO 头（title / description / canonical / hreflang）。
/// `en_href` 为 `/en/*` 对应路由；集成族暂无 en 页（与 Next metadata 一致）。
#[component]
pub fn PublicSeoHead(
    title: &'static str,
    description: &'static str,
    canonical: &'static str,
    en_href: Option<&'static str>,
) -> impl IntoView {
    view! {
        <Title text=title/>
        <Meta name="description" content=description/>
        <Link rel="canonical" href=canonical/>
        <Link rel="alternate" hreflang="zh-CN" href=canonical/>
        {en_href.map(|en| {
            view! { <Link rel="alternate" hreflang="en" href=en/> }
        })}
        <Link rel="alternate" hreflang="x-default" href=canonical/>
    }
}

/// 公开页共用页眉（副标题 / H1 / 署名 / 更新日 / CTA 行），口径与 Next faq/compare/integrations 一致。
#[component]
pub fn PublicPageHeader(
    subtitle: &'static str,
    title: &'static str,
    updated: &'static str,
    buttons: AnyView,
) -> impl IntoView {
    view! {
        <header class="pub-header">
            <p class="pub-subtitle">{subtitle}</p>
            <h1 class="pub-title">{title}</h1>
            <p class="pub-muted">{crate::components::help::public_content::AUTHOR_LINE}</p>
            <p class="pub-muted">{updated}</p>
            <div class="pub-btn-row">{buttons}</div>
        </header>
    }
}
