import type { UiMessageDescriptor } from "./types";

/**
 * Pricing page copy — ADR-0010 share-slot primary product.
 */
export const pricingMessages = {
  pricingTitle: {
    zh: "选择适合你的方案",
    en: "Choose your plan",
  },
  pricingSubtitle: {
    zh: "客户端免费 · 私有使用免费。付费解锁更多可分享知识库名额。",
    en: "Client free · private use free. Paid plans unlock more shareable workspaces.",
  },
  pricingMonthly: {
    zh: "月付",
    en: "Monthly",
  },
  pricingYearly: {
    zh: "年付",
    en: "Yearly",
  },
  pricingYearlyHint: {
    zh: "约 10 个月价",
    en: "~10 months price",
  },
  /** @deprecated alias — prefer pricingYearly */
  pricingYearlySoon: {
    zh: "年付",
    en: "Yearly",
  },
  pricingTierPlusBadge: {
    zh: "推荐",
    en: "Recommended",
  },
  pricingPlanFreeDescription: {
    zh: "客户端与私有知识库免费；可分享工作区 3 个。",
    en: "Client and private workspaces free; 3 shareable workspaces.",
  },
  pricingPlanPlusDescription: {
    zh: "可分享工作区 10 个，适合团队对外协作。",
    en: "10 shareable workspaces for team sharing.",
  },
  pricingPlanProDescription: {
    zh: "可分享工作区 100 个，适合规模化知识服务。",
    en: "100 shareable workspaces for scale.",
  },
  pricingShareSlots: {
    zh: "可分享工作区 {n} 个",
    en: "{n} shareable workspaces",
  },
  pricingWalletAddonTitle: {
    zh: "模型调用余额（可选）",
    en: "Model call balance (optional)",
  },
  pricingWalletAddonBody: {
    zh: "使用平台模型时从余额扣费；也可配置自定义 Provider，对话走你自己的额度。向量检索始终走平台并计入余额。",
    en: "Platform models bill from your balance. With a custom provider, chat uses your own quota. Embedding/retrieval always use the platform and bill the balance.",
  },
  pricingFaqToken: {
    zh: "主商品是什么？",
    en: "What am I buying?",
  },
  pricingFaqTokenAnswer: {
    zh: "主商品是可分享工作区名额（Free 3 / Plus 10 / Pro 100）。客户端与仅自己用的知识库免费。模型调用另用余额或自定义 Provider。",
    en: "Primary product is shareable workspace slots (Free 3 / Plus 10 / Pro 100). The client and private workspaces are free. Model calls use balance or a custom provider.",
  },
  pricingFaqReset: {
    zh: "访客提问谁付费？",
    en: "Who pays for visitor questions?",
  },
  pricingFaqResetAnswer: {
    zh: "分享场景由知识库 Owner 承担模型成本（Owner-pays）。请关注余额与自定义 Provider 配置。",
    en: "On shared workspaces the Owner pays model cost (Owner-pays). Watch balance and custom provider setup.",
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
    zh: "常见问题",
    en: "FAQ",
  },
  pricingUpgradeTo: {
    zh: "升级 {name}",
    en: "Upgrade to {name}",
  },
  pricingMonthlyInterval: {
    zh: "按月计费",
    en: "Billed monthly",
  },
  pricingYearlyInterval: {
    zh: "按年计费",
    en: "Billed yearly",
  },
  pricingDesktopCrossTitle: {
    zh: "本地客户端",
    en: "Desktop client",
  },
  pricingDesktopCrossBody: {
    zh: "Windows / 桌面客户端免费使用，无需买断许可。需要上云分享时再升级名额。",
    en: "Desktop clients are free — no license purchase. Upgrade only when you need more share slots.",
  },
  pricingDesktopCrossCta: {
    zh: "下载客户端",
    en: "Download client",
  },
  alipayQrTitle: {
    zh: "支付宝扫码支付",
    en: "Pay with Alipay",
  },
  alipayQrScanHint: {
    zh: "请打开支付宝 App 扫码支付",
    en: "Open the Alipay app and scan the QR code to pay",
  },
  alipayQrWaiting: {
    zh: "等待支付确认…",
    en: "Waiting for payment confirmation…",
  },
  alipayQrPaid: {
    zh: "支付成功",
    en: "Payment successful",
  },
  alipayQrCancel: {
    zh: "取消支付",
    en: "Cancel payment",
  },
  alipayQrTimeout: {
    zh: "支付超时，请重新发起支付",
    en: "Payment timed out. Please start a new payment.",
  },
} satisfies Record<string, UiMessageDescriptor>;
