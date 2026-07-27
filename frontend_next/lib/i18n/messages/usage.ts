import type { UiMessageDescriptor } from "./types";

/** Plus vs Free rolling limits (see lib/billing/planLimits.ts): 5h 6×, 7d 10×. */
export const usageMessages = {
  "planEntry.upgrade": {
    zh: "升级",
    en: "Upgrade",
  },
  "planEntry.viewSubscription": {
    zh: "查看订阅与用量",
    en: "View subscription & usage",
  },
  currentPlan: {
    zh: "当前套餐",
    en: "Current plan",
  },
  usageTitle: {
    zh: "用量与套餐",
    en: "Usage & Plan",
  },
  usageWindow5h: {
    zh: "5 小时窗口",
    en: "5-hour window",
  },
  usageWindow7d: {
    zh: "7 天窗口",
    en: "7-day window",
  },
  usageEstimatedReset: {
    zh: "预计 {time} 后重置",
    en: "Resets in {time}",
  },
  usageSoftLimitWarning: {
    zh: "已超过软上限，建议控制节奏",
    en: "Soft limit reached, consider slowing down",
  },
  usageTrendTitle: {
    zh: "近 7 日用量趋势",
    en: "Last 7-day trend",
  },
  toastUpgradeCta: {
    zh: "升级 Plus 解锁 {multiplier}× 用量 →",
    en: "Upgrade to Plus for {multiplier}× usage →",
  },
  toastClose: {
    zh: "关闭",
    en: "Close",
  },
  toastResetsIn: {
    zh: "还有 {time} 重置",
    en: "Resets in {time}",
  },
  toastUsageAt: {
    zh: "{window} 用量已用 {pct}%",
    en: "{window} usage at {pct}%",
  },
  usageLoading: {
    zh: "加载中...",
    en: "Loading...",
  },
  usageErrorLoad: {
    zh: "用量数据加载失败，请稍后重试。",
    en: "Failed to load usage data. Please try again later.",
  },
  usageErrorBackDashboard: {
    zh: "返回工作台",
    en: "Back to dashboard",
  },
  usageCurrentPlanLabel: {
    zh: "当前套餐:",
    en: "Current plan:",
  },
  usageFreeUpgradeHint: {
    zh: "→ Free 升级 Plus：5h 6× / 7d 10× 用量",
    en: "→ Upgrade Free to Plus: 6× (5h) / 10× (7d) usage",
  },
  usageUnlimited: {
    zh: "无限制",
    en: "Unlimited",
  },
  usageApproxTokensLabel: {
    zh: "约 {used} / {limit} tokens",
    en: "≈ {used} / {limit} tokens",
  },
  usageMarginNote: {
    zh: "方案乘数 M={m}；按输入（含缓存命中优惠）与输出折算后计入额度，缓存命中更省",
    en: "Plan multiplier M={m}; usage is folded from input (cache-hit discounted) and output — cache hits cost less",
  },
  usageForecastDetail: {
    zh: "预计 30 天用量 {projected} / 7d 限额 {limit}",
    en: "Projected 30-day usage {projected} / 7d limit {limit}",
  },
  usageTrendEmpty: {
    zh: "暂无用量数据",
    en: "No usage data yet",
  },
  usageTrendAriaLabel: {
    zh: "近 N 日用量趋势",
    en: "Recent usage trend",
  },
} satisfies Record<string, UiMessageDescriptor>;
