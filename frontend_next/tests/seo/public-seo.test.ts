import { describe, expect, it } from "vitest";

import robots from "@/app/robots";
import sitemap from "@/app/sitemap";
import { GET as llmsGet } from "@/app/llms.txt/route";
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
  ];

  it.each(cases)("%s declares a self-referencing canonical", (path, metadata) => {
    expect(metadata?.alternates?.canonical).toBe(path);
  });
});
