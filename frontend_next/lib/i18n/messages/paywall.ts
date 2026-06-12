import type { UiMessageDescriptor } from "./types";

export const paywallMessages = {
  paywallTitle5h: {
    zh: "5h 用量已达上限",
    en: "5h limit reached",
  },
  paywallTitle7d: {
    zh: "7d 用量已达上限",
    en: "7d limit reached",
  },
  paywallSubtitle: {
    zh: "Free → Plus，解锁 10× 用量",
    en: "Free → Plus, unlock 10× usage",
  },
  paywallContinueFree: {
    zh: "继续 Free",
    en: "Continue Free",
  },
  paywallResetHint: {
    zh: "限额自动重置，请关注使用节奏",
    en: "Limits reset automatically — pace your usage",
  },
  paywallLoading: {
    zh: "加载中...",
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
