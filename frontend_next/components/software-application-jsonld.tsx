/**
 * GEO：公开面共用的 SoftwareApplication 实体标注。
 * 挂载点：(marketing)/(open) layout、zh 首页（app/page.tsx）、en 面（app/en/layout.tsx）。
 * locale 决定 inLanguage 与文案（Phase E E1.1：此前 en 面误挂 zh-CN 稿）。
 */
export type JsonLdLocale = "zh" | "en";

const COPY: Record<
  JsonLdLocale,
  { inLanguage: string; description: string; authorName: string; offerNote: string }
> = {
  zh: {
    inLanguage: "zh-CN",
    description:
      "Agentic 知识工作台：以私有知识库为中心，Agent 调度多路检索，支持知识库只读对话式分享与 BYOK 计费。ContextLM 旗下产品。",
    authorName: "邢川",
    offerNote: "免费注册；BYOK 按量自理或平台充值",
  },
  en: {
    inLanguage: "en",
    description:
      "Agentic knowledge workbench: a private knowledge base at the center, agents orchestrating multi-route retrieval, read-only conversational library sharing, and BYOK billing. A ContextLM product.",
    authorName: "Xing Chuan",
    offerNote: "Free to register; pay-as-you-go BYOK or wallet top-up",
  },
};

export function softwareApplicationJsonLd(locale: JsonLdLocale = "zh") {
  const copy = COPY[locale];
  return {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    name: "Context OS",
    applicationCategory: "BusinessApplication",
    operatingSystem: "Web, Windows",
    url: "https://app.contextlm.top/",
    inLanguage: copy.inLanguage,
    description: copy.description,
    publisher: {
      "@type": "Organization",
      name: "ContextLM",
      url: "https://contextlm.top/",
    },
    author: {
      "@type": "Person",
      name: copy.authorName,
      url: "https://contextlm.top/#studio",
    },
    offers: {
      "@type": "Offer",
      price: "0",
      priceCurrency: "CNY",
      description: copy.offerNote,
    },
  };
}

export function SoftwareApplicationJsonLd({ locale = "zh" }: { locale?: JsonLdLocale }) {
  return (
    <script
      type="application/ld+json"
      dangerouslySetInnerHTML={{ __html: JSON.stringify(softwareApplicationJsonLd(locale)) }}
    />
  );
}
