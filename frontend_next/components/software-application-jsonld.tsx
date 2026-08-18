/**
 * GEO：公开营销/帮助页共用的 SoftwareApplication 实体标注。
 * 仅在公开路由组（(marketing)、(open)）的 layout 中挂载。
 */
export function SoftwareApplicationJsonLd() {
  const jsonLd = {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    name: "Context OS",
    applicationCategory: "BusinessApplication",
    operatingSystem: "Web, Windows",
    url: "https://app.contextlm.top/",
    inLanguage: "zh-CN",
    description:
      "Agentic 知识工作台：以私有知识库为中心，Agent 调度多路检索，支持知识库只读对话式分享与 BYOK 计费。ContextLM 旗下产品。",
    publisher: {
      "@type": "Organization",
      name: "ContextLM",
      url: "https://contextlm.top/",
    },
    offers: {
      "@type": "Offer",
      price: "0",
      priceCurrency: "CNY",
      description: "免费注册；BYOK 按量自理或平台充值",
    },
  };

  return (
    <script
      type="application/ld+json"
      dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
    />
  );
}
