import { describe, expect, it } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";

import robots from "@/app/robots";
import sitemap from "@/app/sitemap";
import { GET as llmsGet } from "@/app/llms.txt/route";
import EnLayout from "@/app/en/layout";
import { metadata as helpMetadata } from "@/app/(app)/help/page";
import { metadata as helpWriteMetadata } from "@/app/(app)/help/write/page";
import { softwareApplicationJsonLd } from "@/components/software-application-jsonld";
import { organizationJsonLd, websiteJsonLd } from "@/components/organization-jsonld";
import { metadata as homeMetadata } from "@/app/page";
import { metadata as desktopMetadata } from "@/app/(marketing)/desktop/page";
import { metadata as legalMetadata } from "@/app/(marketing)/legal/page";
import { metadata as licensesMetadata } from "@/app/(marketing)/legal/licenses/page";
import { metadata as licensesProjectMetadata } from "@/app/(marketing)/legal/licenses/project/page";
import { metadata as licensesThirdPartyMetadata } from "@/app/(marketing)/legal/licenses/third-party/page";
import { metadata as privacyMetadata } from "@/app/(marketing)/legal/privacy/page";
import { metadata as termsMetadata } from "@/app/(marketing)/legal/terms/page";
import { metadata as pricingMetadata } from "@/app/(marketing)/pricing/page";
import { metadata as apiAccessMetadata } from "@/app/(open)/help/api-access/page";
import { metadata as agentsMetadata } from "@/app/(open)/help/api-access/agents/page";
import { metadata as faqMetadata } from "@/app/(open)/help/faq/page";
import { metadata as compareMetadata } from "@/app/(open)/help/compare/page";
import { metadata as enFaqMetadata } from "@/app/en/help/faq/page";
import { metadata as enCompareMetadata } from "@/app/en/help/compare/page";
import { metadata as enHomeMetadata } from "@/app/en/page";
import { metadata as enPricingMetadata } from "@/app/en/pricing/page";
import { metadata as enDesktopMetadata } from "@/app/en/desktop/page";
import { metadata as enApiAccessMetadata } from "@/app/en/help/api-access/page";
import { metadata as enAgentsMetadata } from "@/app/en/help/api-access/agents/page";
import { metadata as enLegalMetadata } from "@/app/en/legal/page";
import { metadata as enTermsMetadata } from "@/app/en/legal/terms/page";
import { metadata as enPrivacyMetadata } from "@/app/en/legal/privacy/page";
import { metadata as enLicensesMetadata } from "@/app/en/legal/licenses/page";
import { metadata as enLicensesProjectMetadata } from "@/app/en/legal/licenses/project/page";
import { metadata as enLicensesThirdPartyMetadata } from "@/app/en/legal/licenses/third-party/page";
import { metadata as integrationsMetadata } from "@/app/(open)/integrations/page";
import { metadata as integrationsCursorMetadata } from "@/app/(open)/integrations/cursor/page";
import { metadata as integrationsClaudeDesktopMetadata } from "@/app/(open)/integrations/claude-desktop/page";
import { metadata as integrationsMcpMetadata } from "@/app/(open)/integrations/mcp/page";
import { IntegrationDocPage } from "@/components/integrations/integration-doc-page";
import { IntegrationIndexPage } from "@/components/integrations/integration-index-page";

type RobotsRule = {
  userAgent?: string | string[];
  allow?: string | string[];
  disallow?: string | string[];
};

function asArray<T>(value: T | T[] | undefined): T[] {
  if (value === undefined) return [];
  return Array.isArray(value) ? value : [value];
}

describe("robots.txt（GEO 方案 A4）", () => {
  it("allows public crawl and disallows private/utility routes", () => {
    const result = robots();
    const rules = asArray(result.rules) as RobotsRule[];
    const wildcard = rules.find((rule) => rule.userAgent === "*");

    expect(wildcard).toBeDefined();
    expect(asArray(wildcard?.allow)).toContain("/");

    const disallowed = asArray(wildcard?.disallow);
    for (const path of ["/dashboard", "/settings", "/admin", "/shared", "/api/", "/upgrade"]) {
      expect(disallowed).toContain(path);
    }

    expect(result.sitemap).toBe("https://app.contextlm.top/sitemap.xml");
  });

  it("explicitly allows major AI and China crawlers with the same private-path boundary", () => {
    const result = robots();
    const rules = asArray(result.rules) as RobotsRule[];

    for (const bot of [
      "GPTBot",
      "ClaudeBot",
      "PerplexityBot",
      "Baiduspider",
      "Bytespider",
      "Sogou",
      "YisouSpider",
      "360Spider",
    ]) {
      const rule = rules.find((entry) => entry.userAgent === bot);
      expect(rule, `missing rule for ${bot}`).toBeDefined();
      expect(asArray(rule?.allow)).toContain("/");
      expect(asArray(rule?.disallow)).toContain("/dashboard");
    }
  });
});

describe("sitemap.xml（GEO 方案 A4）", () => {
  it("lists public marketing/help routes only", () => {
    const urls = sitemap().map((entry) => entry.url);

    for (const path of [
      "/pricing",
      "/desktop",
      "/legal",
      "/help/api-access",
      "/help/api-access/agents",
      "/help/faq",
      "/help/compare",
      "/integrations",
      "/integrations/cursor",
      "/integrations/claude-desktop",
      "/integrations/mcp",
      "/en",
      "/en/pricing",
      "/en/desktop",
      "/en/help/faq",
      "/en/help/compare",
      "/en/help/api-access",
      "/en/help/api-access/agents",
      "/en/legal",
      "/en/legal/terms",
      "/en/legal/privacy",
      "/en/legal/licenses",
      "/en/legal/licenses/project",
      "/en/legal/licenses/third-party",
    ]) {
      expect(urls).toContain(`https://app.contextlm.top${path}`);
    }

    expect(urls.some((url) => url.includes("/dashboard"))).toBe(false);
    expect(urls.some((url) => url.includes("/shared"))).toBe(false);
    expect(urls.some((url) => url.includes("/settings"))).toBe(false);
  });
});

describe("llms.txt（GEO 方案 A4）", () => {
  it("serves an agent-readable index with key public URLs", async () => {
    const response = llmsGet();
    const body = await response.text();

    expect(response.headers.get("content-type")).toContain("text/plain");
    expect(body).toContain("https://app.contextlm.top/help/api-access/agents");
    expect(body).toContain("https://app.contextlm.top/pricing");
    expect(body).toContain("https://app.contextlm.top/help/api-access");
    expect(body).toContain("https://app.contextlm.top/help/faq");
    expect(body).toContain("https://app.contextlm.top/help/compare");
    expect(body).toContain("https://app.contextlm.top/en/help/faq");
    expect(body).toContain("https://app.contextlm.top/en/help/compare");
    expect(body).toContain("https://app.contextlm.top/en/pricing");
    expect(body).toContain("https://app.contextlm.top/en/desktop");
    expect(body).toContain("https://app.contextlm.top/en/help/api-access");
    expect(body).toContain("https://app.contextlm.top/en/help/api-access/agents");
    expect(body).toContain("https://app.contextlm.top/en/legal");
    expect(body).toContain("https://app.contextlm.top/en/legal/licenses");
    expect(body).toContain("https://app.contextlm.top/en/legal/terms");
    expect(body).toContain("https://app.contextlm.top/en/legal/privacy");
    expect(body).toContain("Context OS by ContextLM");
    expect(body).toContain("https://app.contextlm.top/integrations");
    expect(body).toContain("https://app.contextlm.top/integrations/cursor");
    expect(body).toContain("https://app.contextlm.top/integrations/claude-desktop");
    expect(body).toContain("https://app.contextlm.top/integrations/mcp");
  });
});

describe("公开页 canonical（GEO 方案 A1）", () => {
  const cases: Array<[string, { alternates?: { canonical?: unknown } | null } | undefined]> = [
    ["/", homeMetadata],
    ["/pricing", pricingMetadata],
    ["/desktop", desktopMetadata],
    ["/legal", legalMetadata],
    ["/legal/terms", termsMetadata],
    ["/legal/privacy", privacyMetadata],
    ["/legal/licenses", licensesMetadata],
    ["/legal/licenses/project", licensesProjectMetadata],
    ["/legal/licenses/third-party", licensesThirdPartyMetadata],
    ["/help/api-access", apiAccessMetadata],
    ["/help/api-access/agents", agentsMetadata],
    ["/help/faq", faqMetadata],
    ["/help/compare", compareMetadata],
    ["/en/help/faq", enFaqMetadata],
    ["/en/help/compare", enCompareMetadata],
    ["/en", enHomeMetadata],
    ["/en/pricing", enPricingMetadata],
    ["/en/desktop", enDesktopMetadata],
    ["/en/help/api-access", enApiAccessMetadata],
    ["/en/help/api-access/agents", enAgentsMetadata],
    ["/en/legal", enLegalMetadata],
    ["/en/legal/terms", enTermsMetadata],
    ["/en/legal/privacy", enPrivacyMetadata],
    ["/en/legal/licenses", enLicensesMetadata],
    ["/en/legal/licenses/project", enLicensesProjectMetadata],
    ["/en/legal/licenses/third-party", enLicensesThirdPartyMetadata],
    ["/integrations", integrationsMetadata],
    ["/integrations/cursor", integrationsCursorMetadata],
    ["/integrations/claude-desktop", integrationsClaudeDesktopMetadata],
    ["/integrations/mcp", integrationsMcpMetadata],
  ];

  it.each(cases)("%s declares a self-referencing canonical", (path, metadata) => {
    expect(metadata?.alternates?.canonical).toBe(path);
  });
});

describe("实体 JSON-LD（Phase E E1.1）", () => {
  it("inLanguage / 文案随 locale 参数化（此前 en 面误挂 zh-CN 稿）", () => {
    expect(softwareApplicationJsonLd().inLanguage).toBe("zh-CN");
    expect(softwareApplicationJsonLd("en").inLanguage).toBe("en");
    expect(websiteJsonLd().inLanguage).toBe("zh-CN");
    expect(websiteJsonLd("en").inLanguage).toBe("en");
  });

  it("app/en/layout mounts Organization + WebSite + SoftwareApplication as en", () => {
    const html = renderToStaticMarkup(createElement(EnLayout, null, createElement("p", null, "body")));
    const ldBlocks = html.split("application/ld+json").length - 1;
    expect(ldBlocks).toBe(3);
    expect(html).toContain('"inLanguage":"en"');
    expect(html).not.toContain("zh-CN");
  });

  it("organization entity stays locale-neutral with real sameAs", () => {
    const org = organizationJsonLd();
    expect(org.name).toBe("ContextLM");
    expect(org.sameAs).toContain("https://blog.contextlm.top/");
  });
});

describe("app 壳页索引边界（Phase E E1.2-B）", () => {
  it("/help and /help/write are noindex, follow（robots 前缀会误杀公开 /help/* 族）", () => {
    expect(helpMetadata?.robots).toEqual({ index: false, follow: true });
    expect(helpWriteMetadata?.robots).toEqual({ index: false, follow: true });
  });

  it("public /help/* pages are NOT disallowed in robots.txt", () => {
    const result = robots();
    const rules = asArray(result.rules) as RobotsRule[];
    const wildcard = rules.find((rule) => rule.userAgent === "*");
    const disallowed = asArray(wildcard?.disallow);
    expect(disallowed).not.toContain("/help");
    for (const path of ["/help/faq", "/help/compare", "/help/api-access"]) {
      expect(disallowed.some((entry) => path.startsWith(entry))).toBe(false);
    }
  });
});

describe("integrations 承接页（Phase E Slice 2）", () => {
  const slugs = ["cursor", "claude-desktop", "mcp"] as const;

  it("每个承接页恰好一个 H1，CTA 只链 canonical（agents 文档 / desktop / pricing）", () => {
    for (const slug of slugs) {
      const html = renderToStaticMarkup(createElement(IntegrationDocPage, { slug }));
      expect(html.match(/<h1[ >]/g)).toHaveLength(1);
      expect(html).toContain('href="/help/api-access/agents"');
      expect(html).toContain('href="/desktop"');
      expect(html).toContain('href="/pricing"');
      expect(html).toContain('href="/integrations"');
    }
  });

  it("索引页链到三个承接页与 agents 文档", () => {
    const html = renderToStaticMarkup(createElement(IntegrationIndexPage));
    expect(html.match(/<h1[ >]/g)).toHaveLength(1);
    for (const slug of slugs) {
      expect(html).toContain(`href="/integrations/${slug}"`);
    }
    expect(html).toContain('href="/help/api-access/agents"');
  });
});
