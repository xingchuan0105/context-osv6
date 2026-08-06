import type { UiMessageDescriptor } from "./types";

/** Usage / plan entry copy — consumption reference, not quota wall. */
export const usageMessages = {
  "planEntry.upgrade": {
    zh: "升级",
    en: "Upgrade",
  },
  "planEntry.viewSubscription": {
    zh: "会员状态",
    en: "Membership",
  },
  currentPlan: {
    zh: "当前方案",
    en: "Current plan",
  },
  usageTitle: {
    zh: "消费明细",
    en: "Usage details",
  },
  usageWindow5h: {
    zh: "近 5 小时",
    en: "Last 5 hours",
  },
  usageWindow7d: {
    zh: "近 7 天",
    en: "Last 7 days",
  },
  usageEstimatedReset: {
    zh: "预计 {time} 后缓解",
    en: "Eases in {time}",
  },
  usageSoftLimitWarning: {
    zh: "接近平台保护限速，建议检查余额或自定义 Provider",
    en: "Approaching protective throttle — check balance or custom provider",
  },
  usageTrendTitle: {
    zh: "近 7 日消费趋势",
    en: "Last 7-day usage trend",
  },
  toastUpgradeCta: {
    zh: "升级方案或充值余额 →",
    en: "Upgrade plan or top up balance →",
  },
  toastClose: {
    zh: "关闭",
    en: "Close",
  },
  toastResetsIn: {
    zh: "还有 {time}",
    en: "{time} left",
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
    zh: "消费数据加载失败，请稍后重试。",
    en: "Failed to load usage data. Please try again later.",
  },
  usageErrorBackDashboard: {
    zh: "返回工作台",
    en: "Back to dashboard",
  },
  usageCurrentPlanLabel: {
    zh: "当前方案:",
    en: "Current plan:",
  },
  usageFreeUpgradeHint: {
    zh: "→ 升级 Plus/Pro 增加可分享工作区名额",
    en: "→ Upgrade Plus/Pro for more shareable workspaces",
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
    zh: "参考乘数 M={m}（平台模型计费折算）；有自定义 Provider 时对话可不走平台余额",
    en: "Reference multiplier M={m} for platform model billing; custom provider chat can skip platform balance",
  },
  usageForecastDetail: {
    zh: "预计 30 天用量 {projected}（参考）",
    en: "Projected 30-day usage {projected} (reference)",
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
