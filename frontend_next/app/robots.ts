import type { MetadataRoute } from "next";

import { getPublicSiteUrl } from "../lib/seo";

/** 桌面端静态导出（output: export）要求 metadata route 显式静态化。 */
export const dynamic = "force-static";

/**
 * 索引边界（GEO/SEO 方案 A4，docs/plans/2026-08-11-contextlm-geo-seo-optimization-plan.md）：
 * 公开面 = / 、/pricing、/desktop、/legal/*、/help/api-access*、auth 页；
 * 登录后与功能面（dashboard/settings/admin/shared/api…）统一 disallow。
 *
 * Crawler 策略：GEO 目标是「被搜索 / AI 引用」，公开内容对主流 bot 明示放行，
 * 私有路径同样 disallow。`User-agent: *` 已 Allow；下列名单为**显式**对齐
 * （欧美 AI + 国内搜索/字节系），避免被托管 robots 或默认策略误读成「未声明」。
 * DeepSeek / 豆包对话产品无稳定公开 UA 时，仍走 `*`（不禁止）。
 */
const PRIVATE_PATHS = [
  "/admin",
  "/api/",
  "/activate",
  "/dashboard",
  "/invite",
  "/reset-password",
  "/settings",
  "/shared",
  "/upgrade",
];

/** Explicit allow-list: same allow/disallow as `*`. Order is documentation only. */
const NAMED_CRAWLERS = [
  // Western AI / answer engines
  "GPTBot",
  "OAI-SearchBot",
  "ChatGPT-User",
  "ClaudeBot",
  "Claude-User",
  "PerplexityBot",
  "Google-Extended",
  // China search + major platform crawlers
  "Baiduspider",
  "Baiduspider-render",
  "Bytespider",
  "Sogou",
  "YisouSpider",
  "360Spider",
  "HaosouSpider",
];

export default function robots(): MetadataRoute.Robots {
  return {
    rules: [
      { userAgent: "*", allow: "/", disallow: PRIVATE_PATHS },
      ...NAMED_CRAWLERS.map((bot) => ({
        userAgent: bot,
        allow: "/",
        disallow: PRIVATE_PATHS,
      })),
    ],
    sitemap: `${getPublicSiteUrl()}/sitemap.xml`,
  };
}
