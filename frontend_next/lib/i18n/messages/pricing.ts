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
    zh: "客户端免费 · 私有使用免费。会员解锁可分享名额；平台模型调用另用余额充值。",
    en: "Client free · private use free. Membership unlocks share slots; platform models use wallet top-up.",
  },
  pricingMembershipTitle: {
    zh: "会员档位 · 可分享工作区",
    en: "Membership · shareable workspaces",
  },
  pricingMembershipLead: {
    zh: "选择档位后在本页完成支付。主商品是可同时开启分享的工作区数量。",
    en: "Pick a tier and check out here. You are buying concurrent shareable workspace slots.",
  },
  pricingTopupTitle: {
    zh: "充值 · 模型调用余额",
    en: "Top-up · model wallet",
  },
  pricingTopupBody: {
    zh: "余额与会员独立。使用平台模型、向量检索，以及分享页访客问答（Owner-pays）时从余额扣费。已配置自定义 Provider 时，对话可走你自己的额度。",
    en: "Wallet is independent of plan. Platform models, retrieval, and shared guest Q&A (Owner-pays) debit the wallet. With a custom provider, chat can use your own quota.",
  },
  pricingTopupPoint1: {
    zh: "充值入口在「设置 → 账单」，支持套餐包快捷购买",
    en: "Top up under Settings → Billing with wallet packs",
  },
  pricingTopupPoint2: {
    zh: "不想用平台模型：可改配 BYOK，减少对话扣费",
    en: "Prefer BYOK: configure providers to reduce chat billing",
  },
  pricingTopupPoint3: {
    zh: "只升级会员不充值：分享名额增加，但平台模型仍需余额或 BYOK",
    en: "Upgrade-only adds share slots; platform models still need wallet or BYOK",
  },
  pricingTopupCta: {
    zh: "去账单充值",
    en: "Open billing & top up",
  },
  pricingTopupByokCta: {
    zh: "配置自定义 Provider",
    en: "Configure providers",
  },
  pricingFaqTopup: {
    zh: "充值和会员有什么区别？",
    en: "Top-up vs membership?",
  },
  pricingFaqTopupAnswer: {
    zh: "会员控制可分享工作区名额；充值是模型调用余额。两者可单独购买。本页上方升级档位，下方说明充值并跳转到账单完成充值。",
    en: "Membership controls share slots; top-up is the model wallet. Buy either independently. Upgrade tiers above; use the top-up section to open billing.",
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
  pricingTierPlusBadge: {
    zh: "推荐",
    en: "Recommended",
  },
  pricingPlanFreeDescription: {
    zh: "客户端与私有工作区免费；可分享工作区 3 个。",
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
  pricingFaqToken: {
    zh: "主商品是什么？",
    en: "What am I buying?",
  },
  pricingFaqTokenAnswer: {
    zh: "主商品是可分享工作区名额（Free 3 / Plus 10 / Pro 100）。客户端与仅自己使用的工作区免费。",
    en: "Primary product is shareable workspace slots (Free 3 / Plus 10 / Pro 100). The client and private workspaces are free.",
  },
  pricingFaqReset: {
    zh: "访客提问谁付费？",
    en: "Who pays for visitor questions?",
  },
  pricingFaqResetAnswer: {
    zh: "分享问答的成本由工作区所有者承担（Owner-pays）。可在设置中配置自定义 Provider。",
    en: "On shared workspaces, the workspace owner pays model costs (Owner-pays). Configure a custom provider in Settings if needed.",
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
  pricingViewDetails: {
    zh: "查看 {name} 详情",
    en: "View {name} details",
  },
  pricingMonthlyInterval: {
    zh: "按月计费",
    en: "Billed monthly",
  },
  pricingYearlyInterval: {
    zh: "按年计费",
    en: "Billed yearly",
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
