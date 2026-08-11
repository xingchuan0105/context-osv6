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
];

export default function sitemap(): MetadataRoute.Sitemap {
  const base = getPublicSiteUrl();
  const lastModified = new Date();
  return PUBLIC_PATHS.map((path) => ({
    url: `${base}${path}`,
    lastModified,
  }));
}
