import type { UiMessageDescriptor } from "./types";

/** Protective hard-stop / upgrade prompts — not share-slot primary narrative. */
export const paywallMessages = {
  paywallTitle5h: {
    zh: "平台保护限速已触发",
    en: "Platform protective limit reached",
  },
  paywallTitle7d: {
    zh: "平台保护限速已触发",
    en: "Platform protective limit reached",
  },
  paywallSubtitle5h: {
    zh: "余额不足且未配置自定义 Provider 时，平台会暂时限速。充值余额、配置自定义 Provider，或升级分享名额即可恢复。",
    en: "Without balance or a custom provider, the platform may throttle. Top up balance, add a custom provider, or upgrade share slots.",
  },
  paywallSubtitle7d: {
    zh: "余额不足且未配置自定义 Provider 时，平台会暂时限速。充值余额、配置自定义 Provider，或升级分享名额即可恢复。",
    en: "Without balance or a custom provider, the platform may throttle. Top up balance, add a custom provider, or upgrade share slots.",
  },
  paywallContinueFree: {
    zh: "稍后再说",
    en: "Not now",
  },
  paywallViewPlans: {
    zh: "查看会员档位",
    en: "View membership plans",
  },
  paywallResetHint: {
    zh: "也可配置自定义 Provider，对话不再消耗平台余额。",
    en: "Or add a custom provider so chat does not spend platform balance",
  },
  paywallLoading: {
    zh: "加载中…",
    en: "Loading...",
  },
  paywallErrorLoad: {
    zh: "无法加载升级信息，请稍后重试。",
    en: "Unable to load upgrade details. Please try again later.",
  },
  paywallErrorBackDashboard: {
    zh: "返回工作台",
    en: "Back to dashboard",
  },
} satisfies Record<string, UiMessageDescriptor>;
