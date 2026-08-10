import type { UiMessageKey } from "../i18n/messages";

/**
 * In-app canonical navigation catalog — single source of truth for
 * PRODUCT_IA §4 canonical destinations (docs/design/PRODUCT_IA.md).
 *
 * Every product destination linked from global chrome (primary nav, product
 * footer, Cmd/Ctrl+K palette, product guide) is declared here exactly once.
 * IA rule: update PRODUCT_IA.md before adding/moving an entry; an entry that
 * no surface renders is an orphan route in the making.
 *
 * Out of scope (own single sources): admin sub-nav (admin-shell.tsx),
 * settings tabs (settings-tabs.ts), cross-site discovery (lib/site-map.ts).
 */

export type AppNavId =
  | "dashboard"
  | "share-traffic"
  | "settings"
  | "providers"
  | "billing"
  | "pricing"
  | "topup"
  | "desktop"
  | "help"
  | "api-access"
  | "legal"
  | "legal-terms"
  | "legal-privacy"
  | "legal-licenses";

/** Command palette grouping (static commands only; palette adds dynamic groups). */
export type AppNavPaletteGroup = "nav" | "billing" | "help";

export type AppNavEntry = {
  id: AppNavId;
  /** Canonical href per PRODUCT_IA §4 (deep links may carry ?tab= / #topup). */
  href: string;
  labelKey: UiMessageKey;
  /** Present when the entry is a command-palette static command. */
  paletteGroup?: AppNavPaletteGroup;
  paletteKeywords?: string;
};

/**
 * Canonical order doubles as palette static-command order.
 * Footer / guide render their own explicit id lists (surface ordering differs).
 */
export const APP_NAV_ENTRIES: readonly AppNavEntry[] = [
  {
    id: "dashboard",
    href: "/dashboard",
    labelKey: "productChrome.productHome",
    paletteGroup: "nav",
    paletteKeywords: "dashboard workspaces home 工作台",
  },
  {
    id: "share-traffic",
    href: "/dashboard/analytics",
    labelKey: "commandPalette.item.shareTraffic",
    paletteGroup: "nav",
    paletteKeywords: "share analytics traffic views 分享 访问",
  },
  {
    id: "settings",
    href: "/settings",
    labelKey: "appPrimaryNav.settings",
    paletteGroup: "nav",
    paletteKeywords: "settings profile preferences 设置",
  },
  {
    id: "providers",
    href: "/settings?tab=providers",
    labelKey: "productGuide.graph.providers",
    paletteGroup: "nav",
    paletteKeywords: "providers byok llm key 模型 密钥",
  },
  {
    id: "billing",
    href: "/settings?tab=billing",
    labelKey: "commandPalette.item.billing",
    paletteGroup: "billing",
    paletteKeywords: "billing wallet balance 账单 余额",
  },
  {
    id: "pricing",
    href: "/pricing",
    labelKey: "productChrome.pricing",
    paletteGroup: "billing",
    paletteKeywords: "pricing plan membership upgrade 定价 会员 升级",
  },
  {
    id: "topup",
    href: "/pricing#topup",
    labelKey: "commandPalette.item.topup",
    paletteGroup: "billing",
    paletteKeywords: "topup recharge wallet 充值 余额",
  },
  {
    id: "desktop",
    href: "/desktop",
    labelKey: "productChrome.client",
    paletteGroup: "help",
    paletteKeywords: "desktop client download mcp 客户端 下载",
  },
  {
    id: "help",
    href: "/help",
    labelKey: "productChrome.help",
    paletteGroup: "help",
    paletteKeywords: "help guide docs 帮助 上手",
  },
  {
    id: "api-access",
    href: "/help/api-access",
    labelKey: "productGuide.graph.api",
    paletteGroup: "help",
    paletteKeywords: "api agent mcp cli access 接入",
  },
  {
    id: "legal",
    href: "/legal",
    labelKey: "productChrome.legalCenter",
  },
  {
    id: "legal-terms",
    href: "/legal/terms",
    labelKey: "productChrome.terms",
  },
  {
    id: "legal-privacy",
    href: "/legal/privacy",
    labelKey: "productChrome.privacy",
  },
  {
    id: "legal-licenses",
    href: "/legal/licenses",
    labelKey: "productChrome.licenses",
  },
];

export function appNavEntry(id: AppNavId): AppNavEntry {
  const entry = APP_NAV_ENTRIES.find((candidate) => candidate.id === id);
  if (!entry) {
    throw new Error(`Unknown app nav id: ${id}`);
  }
  return entry;
}

export function appNavHref(id: AppNavId): string {
  return appNavEntry(id).href;
}

/** Resolve an ordered surface id list (footer / guide graph) to entries. */
export function appNavEntriesByIds(ids: readonly AppNavId[]): AppNavEntry[] {
  return ids.map(appNavEntry);
}

/** Static command-palette entries, in canonical order. */
export function paletteNavEntries(): AppNavEntry[] {
  return APP_NAV_ENTRIES.filter((entry) => entry.paletteGroup !== undefined);
}
