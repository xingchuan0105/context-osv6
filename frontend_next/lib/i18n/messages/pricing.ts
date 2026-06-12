import type { UiMessageDescriptor } from "./types";

export const pricingMessages = {
  pricingTitle: {
    zh: "选择适合你的方案",
    en: "Choose your plan",
  },
  pricingMonthly: {
    zh: "月付",
    en: "Monthly",
  },
  pricingYearlySoon: {
    zh: "年付暂未开放",
    en: "Yearly coming soon",
  },
  pricingTierFreeName: {
    zh: "Free",
    en: "Free",
  },
  pricingTierPlusName: {
    zh: "Plus",
    en: "Plus",
  },
  pricingTierProName: {
    zh: "Pro",
    en: "Pro",
  },
  pricingTierPlusBadge: {
    zh: "推荐",
    en: "Recommended",
  },
  pricingTierPlusTagline: {
    zh: "深度研究首选",
    en: "Best for deep research",
  },
  pricingTierProTagline: {
    zh: "重度无忧",
    en: "For power users",
  },
  pricingFaqToken: {
    zh: "token 用量怎么算？",
    en: "How is token usage counted?",
  },
  pricingFaqTokenAnswer: {
    zh: "输入 + 输出按 DeepSeek 公开计费标准累计",
    en: "Input + output per DeepSeek public pricing",
  },
  pricingFaqReset: {
    zh: "限额会重置吗？",
    en: "Do limits reset?",
  },
  pricingFaqResetAnswer: {
    zh: "5h 滚动窗口 + 7d 滚动窗口，最旧消耗点过后自动释放",
    en: "5h rolling + 7d rolling windows",
  },
  pricingFaqUpgrade: {
    zh: "升级后立即生效吗？",
    en: "Does upgrade take effect immediately?",
  },
  pricingFaqUpgradeAnswer: {
    zh: "支付成功后立即生效。降级在当前计费周期结束时生效。",
    en: "Effective immediately after payment. Downgrade at end of billing cycle.",
  },
  pricingFaqTitle: {
    zh: "❓ 常见问题",
    en: "❓ FAQ",
  },
  pricingUpgradeTo: {
    zh: "升级 {name}",
    en: "Upgrade to {name}",
  },
  pricingMonthlyInterval: {
    zh: "月付",
    en: "Monthly billing",
  },
} satisfies Record<string, UiMessageDescriptor>;
