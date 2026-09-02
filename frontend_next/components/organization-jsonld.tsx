/**
 * GEO：Organization + WebSite 实体标注（EEAT 权威信号）。
 * 挂载点：(marketing)/(open) layout、zh 首页（app/page.tsx）、en 面（app/en/layout.tsx）。
 * sameAs 仅列真实存在且由 ContextLM 运营的站点（见 lib/site-map.ts）。
 */
import type { JsonLdLocale } from "./software-application-jsonld";

export function organizationJsonLd() {
  return {
    "@context": "https://schema.org",
    "@type": "Organization",
    name: "ContextLM",
    alternateName: "Context OS",
    url: "https://contextlm.top/",
    sameAs: [
      "https://github.com/xingchuan0105",
      "https://x.com/cHuaNXiNg105",
      "https://blog.contextlm.top/",
      "https://app.contextlm.top/",
    ],
  };
}

export function websiteJsonLd(locale: JsonLdLocale = "zh") {
  return {
    "@context": "https://schema.org",
    "@type": "WebSite",
    name: "Context OS",
    url: "https://app.contextlm.top/",
    inLanguage: locale === "zh" ? "zh-CN" : "en",
    publisher: {
      "@type": "Organization",
      name: "ContextLM",
      url: "https://contextlm.top/",
    },
  };
}

export function OrganizationJsonLd({ locale = "zh" }: { locale?: JsonLdLocale }) {
  return (
    <>
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(organizationJsonLd()) }}
      />
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(websiteJsonLd(locale)) }}
      />
    </>
  );
}
