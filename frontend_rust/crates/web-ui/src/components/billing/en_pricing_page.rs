use super::pricing_page::PricingPage;
use crate::components::help::json_ld::{OrganizationJsonLd, SoftwareApplicationJsonLd};
use crate::components::legal::legal_pages::LegalFooterLinks;
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};

/// `/en/pricing`：英文定价页。套餐数据来自后端契约（与 zh 同一 API）；
/// UI 文案暂复用 zh 组件（偏差已记录于 E4.4 任务文档，en 文案随后续切片补齐）。
#[component]
pub fn EnPricingPage() -> impl IntoView {
    view! {
        <Title text="Pricing"/>
        <Meta name="description" content="Context OS membership tiers and on-page top-up: start free, upgrade to unlock more share slots."/>
        <Link rel="canonical" href="/en/pricing"/>
        <Link rel="alternate" hreflang="zh-CN" href="/pricing"/>
        <Link rel="alternate" hreflang="en" href="/en/pricing"/>
        <Link rel="alternate" hreflang="x-default" href="/pricing"/>
        <OrganizationJsonLd locale="en"/>
        <SoftwareApplicationJsonLd locale="en"/>
        <PricingPage/>
        <LegalFooterLinks locale="en"/>
    }
}
