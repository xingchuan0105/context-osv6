import type { MetadataRoute } from "next";

import { getPublicSiteUrl } from "../lib/seo";

/** 桌面端静态导出（output: export）要求 metadata route 显式静态化；lastModified 即构建时间。 */
export const dynamic = "force-static";

/**
 * 公开面 sitemap（GEO/SEO 方案 A4）。登录后路由（/dashboard、/settings…）、
 * 功能页（/login、/invite…）与半私密分享页（/shared/*）不进入 sitemap。
 */
const PUBLIC_PATHS = [
  "",
  "/pricing",
  "/desktop",
  "/legal",
  "/legal/terms",
  "/legal/privacy",
  "/legal/licenses",
  "/legal/licenses/project",
  "/legal/licenses/third-party",
  "/help/api-access",
  "/help/api-access/agents",
  "/help/faq",
  "/help/compare",
  // 集成承接面（Phase E Slice 2，zh 先行；en 页发布后再补 /en/integrations/*）
  "/integrations",
  "/integrations/cursor",
  "/integrations/claude-desktop",
  "/integrations/mcp",
  // 英文公开面（2026-09-01 方案）：与 zh 页一一对应
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
];

export default function sitemap(): MetadataRoute.Sitemap {
  const base = getPublicSiteUrl();
  const lastModified = new Date();
  return PUBLIC_PATHS.map((path) => ({
    url: `${base}${path}`,
    lastModified,
  }));
}
