import type { MetadataRoute } from "next";

import { getPublicSiteUrl } from "../lib/seo";

/** 桌面端静态导出（output: export）要求 metadata route 显式静态化。 */
export const dynamic = "force-static";

/**
 * 索引边界（GEO/SEO 方案 A4，docs/plans/2026-08-11-contextlm-geo-seo-optimization-plan.md）：
 * 公开面 = / 、/pricing、/desktop、/legal/*、/help/api-access*、auth 页；
 * 登录后与功能面（dashboard/settings/admin/shared/api…）统一 disallow。
 *
 * AI crawler 策略：GEO 目标是「被 AI 引用」，公开内容对主流 AI bot 明示放行，
 * 私有路径同样 disallow。
 */
const PRIVATE_PATHS = [
  "/admin",
  "/api/",
  "/activate",
  "/dashboard",
  "/invite",
  "/reset-password",
  "/settings",
  "/setup",
  "/shared",
  "/upgrade",
];

const AI_CRAWLERS = [
  "GPTBot",
  "OAI-SearchBot",
  "ChatGPT-User",
  "ClaudeBot",
  "Claude-User",
  "PerplexityBot",
  "Google-Extended",
];

export default function robots(): MetadataRoute.Robots {
  return {
    rules: [
      { userAgent: "*", allow: "/", disallow: PRIVATE_PATHS },
      ...AI_CRAWLERS.map((bot) => ({
        userAgent: bot,
        allow: "/",
        disallow: PRIVATE_PATHS,
      })),
    ],
    sitemap: `${getPublicSiteUrl()}/sitemap.xml`,
  };
}
