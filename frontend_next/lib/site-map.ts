/**
 * Cross-site discovery map (single source of truth for public URLs).
 * Satellite sites (landing/why) should mirror these hrefs when editing nav.
 *
 * @see docs/engineering/MULTI_SITE_IA_INTEGRATION_PLAN_2026-07-14.md
 */

export type SiteLocale = "zh" | "en";

export type SiteLinkId =
  | "hub"
  | "app_login"
  | "app_dashboard"
  | "desktop"
  | "desktop_buy"
  | "pricing"
  | "help"
  | "blog"
  | "why"
  | "canju"
  | "elo";

type SiteLinkDef = {
  id: SiteLinkId;
  /** Absolute when cross-origin; path-only when same app host. */
  href: string;
  label: { zh: string; en: string };
  /** Where this link must appear for "published" surfaces. */
  discovery: Array<"family_nav" | "hub_cta" | "app_footer" | "help" | "pricing" | "marketing_chrome">;
};

function trimSlash(url: string): string {
  return url.replace(/\/+$/, "");
}

/** Brand / marketing hub (landing). */
export function getHubOrigin(): string {
  if (typeof process !== "undefined" && process.env.NEXT_PUBLIC_BRAND_HOME_URL?.trim()) {
    return trimSlash(process.env.NEXT_PUBLIC_BRAND_HOME_URL.trim());
  }
  return "https://www.contextlm.top";
}

/** Public app origin (SaaS + marketing paths + releases). */
export function getAppPublicOrigin(): string {
  if (typeof process !== "undefined" && process.env.NEXT_PUBLIC_APP_ORIGIN?.trim()) {
    return trimSlash(process.env.NEXT_PUBLIC_APP_ORIGIN.trim());
  }
  return "https://app.contextlm.top";
}

export function appAbsoluteUrl(path: string): string {
  const p = path.startsWith("/") ? path : `/${path}`;
  return `${getAppPublicOrigin()}${p}`;
}

/** Same-origin path on app host (for Next Link). */
export const APP_PATHS = {
  login: "/login",
  register: "/register",
  dashboard: "/dashboard",
  desktop: "/desktop",
  desktopBuy: "/desktop/buy",
  pricing: "/pricing",
  help: "/help",
  legal: "/legal",
} as const;

export const EXTERNAL = {
  hub: () => getHubOrigin(),
  blog: "https://blog.contextlm.top",
  why: "https://whyimright.contextlm.top",
  canju: "https://canju.contextlm.top",
  elo: "https://elo.contextlm.top",
  appLogin: () => appAbsoluteUrl(`${APP_PATHS.login}?next=${encodeURIComponent(APP_PATHS.dashboard)}`),
  appDesktop: () => appAbsoluteUrl(APP_PATHS.desktop),
  appDesktopBuy: () => appAbsoluteUrl(APP_PATHS.desktopBuy),
  appPricing: () => appAbsoluteUrl(APP_PATHS.pricing),
} as const;

/** Canonical catalog for audits / satellite sync. */
export const SITE_LINKS: SiteLinkDef[] = [
  {
    id: "hub",
    href: getHubOrigin(),
    label: { zh: "官网", en: "Home" },
    discovery: ["family_nav", "marketing_chrome"],
  },
  {
    id: "app_login",
    href: APP_PATHS.login,
    label: { zh: "应用", en: "App" },
    discovery: ["family_nav", "hub_cta", "marketing_chrome"],
  },
  {
    id: "desktop",
    href: APP_PATHS.desktop,
    label: { zh: "客户端", en: "Client" },
    discovery: ["family_nav", "hub_cta", "app_footer", "help", "pricing", "marketing_chrome"],
  },
  {
    id: "desktop_buy",
    href: APP_PATHS.desktopBuy,
    label: { zh: "历史客户端授权", en: "Legacy client license" },
    /** Not in primary discovery (PRODUCT_IA P1-4); client is free → /desktop. */
    discovery: [],
  },
  {
    id: "pricing",
    href: APP_PATHS.pricing,
    label: { zh: "定价", en: "Pricing" },
    discovery: ["app_footer", "marketing_chrome"],
  },
  {
    id: "help",
    href: APP_PATHS.help,
    label: { zh: "帮助", en: "Help" },
    discovery: ["app_footer"],
  },
  {
    id: "blog",
    href: EXTERNAL.blog,
    label: { zh: "博客", en: "Blog" },
    discovery: ["family_nav"],
  },
  {
    id: "why",
    href: EXTERNAL.why,
    label: { zh: "Why I Am Right", en: "Why I Am Right" },
    discovery: ["family_nav"],
  },
  {
    id: "canju",
    href: EXTERNAL.canju,
    label: { zh: "象棋", en: "Xiangqi" },
    discovery: ["family_nav"],
  },
  {
    id: "elo",
    href: EXTERNAL.elo,
    label: { zh: "ELO-everything", en: "ELO-everything" },
    discovery: ["family_nav"],
  },
];

export function siteLinkLabel(id: SiteLinkId, locale: SiteLocale = "zh"): string {
  const hit = SITE_LINKS.find((l) => l.id === id);
  return hit ? hit.label[locale] : id;
}

/** Family nav items for hub / tools chrome (absolute URLs). */
export function familyNavLinks(locale: SiteLocale = "zh"): Array<{ id: string; label: string; href: string }> {
  return [
    { id: "app", label: locale === "zh" ? "应用" : "App", href: EXTERNAL.appLogin() },
    { id: "desktop", label: locale === "zh" ? "客户端" : "Client", href: EXTERNAL.appDesktop() },
    { id: "blog", label: locale === "zh" ? "博客" : "Blog", href: EXTERNAL.blog },
    { id: "why", label: "Why I Am Right", href: EXTERNAL.why },
    { id: "canju", label: locale === "zh" ? "象棋" : "Xiangqi", href: EXTERNAL.canju },
    { id: "elo", label: "ELO", href: EXTERNAL.elo },
  ];
}
