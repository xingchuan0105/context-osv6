use super::pricing_page::PricingPage;
use crate::components::help::json_ld::{OrganizationJsonLd, SoftwareApplicationJsonLd};
use crate::components::shell::PublicSiteHeader;
use crate::components::legal::legal_pages::LegalFooterLinks;
use crate::i18n::UiLocale;
use leptos::prelude::*;
use leptos_meta::{Html, Link, Meta, Title};

/// `/en/pricing`：英文定价页。套餐数据来自后端契约（与 zh 同一 API）。
#[component]
pub fn EnPricingPage() -> impl IntoView {
    view! {
        <Html {..} lang="en"/>
        <Title text="Pricing"/>
        <Meta name="description" content="Context OS membership tiers and on-page top-up: start free, upgrade to unlock more share slots."/>
        <Link rel="canonical" href="/en/pricing"/>
        <Link rel="alternate" hreflang="zh-CN" href="/pricing"/>
        <Link rel="alternate" hreflang="en" href="/en/pricing"/>
        <Link rel="alternate" hreflang="x-default" href="/pricing"/>
        <OrganizationJsonLd locale="en"/>
        <SoftwareApplicationJsonLd locale="en"/>
        <PublicSiteHeader locale="en" active="pricing"/>
        <PricingPage locale_override=UiLocale::En/>
        <LegalFooterLinks locale="en"/>
    }
}
