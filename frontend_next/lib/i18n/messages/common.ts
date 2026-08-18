import type { UiMessageDescriptor } from "./types";

export const commonMessages = {
  "appModal.close": {
    zh: "关闭",
    en: "Close",
  },
  "upgradeModal.title": {
    zh: "会员与充值",
    en: "Membership & top-up",
  },
  "upgradeModal.subtitle": {
    zh: "本页是产品说明。正式支付与充值请进入定价页或账单页完成。",
    en: "This dialog explains the product. Complete payment or top-up on the pricing or billing page.",
  },
  "upgradeModal.openFullPage": {
    zh: "打开定价详情页 →",
    en: "Open full pricing page →",
  },
  "upgradeModal.memberTitle": {
    zh: "会员档位是干什么的？",
    en: "What is membership for?",
  },
  "upgradeModal.memberBody": {
    zh: "会员解锁可分享工作区名额（Free 3 / Plus 10 / Pro 100）。客户端与仅自己使用的工作区始终免费。",
    en: "Membership unlocks shareable workspace slots (Free 3 / Plus 10 / Pro 100). Client and private workspaces stay free.",
  },
  "upgradeModal.topupTitle": {
    zh: "充值是干什么的？",
    en: "What is top-up for?",
  },
  "upgradeModal.topupBody": {
    zh: "余额支付平台模型调用与向量检索。与会员独立：可以只充值不升级，也可以两者都要。",
    en: "Wallet pays platform model calls and retrieval. Independent of plan — top up only, upgrade only, or both.",
  },
  "upgradeModal.topupStrip": {
    zh: "需要立刻支付？请到定价页选档或直接充值余额。",
    en: "Ready to pay? Open pricing for tiers or wallet top-up packs.",
  },
  "upgradeModal.pricingCta": {
    zh: "定价与支付详情",
    en: "Pricing & checkout",
  },
  "upgradeModal.topupCta": {
    zh: "去定价页充值",
    en: "Top up on pricing",
  },
  "settingsQuickModal.openFullPage": {
    zh: "打开设置页 →",
    en: "Open settings page →",
  },
  "settingsQuickModal.moreSettings": {
    zh: "更多设置",
    en: "More settings",
  },
  "settingsQuickModal.securityLink": {
    zh: "安全",
    en: "Security",
  },
  "settingsQuickModal.notificationsLink": {
    zh: "通知",
    en: "Notifications",
  },
  commonCancel: {
    zh: "取消",
    en: "Cancel",
  },
  commonUnlimited: {
    zh: "不限",
    en: "Unlimited",
  },
  "productChrome.footerNavLabel": {
    zh: "产品与法律链接",
    en: "Product and legal links",
  },
  "productChrome.brandHome": {
    zh: "品牌官网",
    en: "Brand site",
  },
  "productChrome.productHome": {
    zh: "工作台",
    en: "Dashboard",
  },
  "appPrimaryNav.settings": {
    zh: "设置",
    en: "Settings",
  },
  "productChrome.help": {
    zh: "产品帮助",
    en: "Help",
  },
  "productChrome.pricing": {
    zh: "定价",
    en: "Pricing",
  },
  "productChrome.client": {
    zh: "客户端",
    en: "Client",
  },
  "productChrome.legalCenter": {
    zh: "法律中心",
    en: "Legal center",
  },
  "marketingChrome.navLabel": {
    zh: "产品导航",
    en: "Product navigation",
  },
  "marketingChrome.login": {
    zh: "登录",
    en: "Log in",
  },
  "marketingChrome.enterApp": {
    zh: "进入应用",
    en: "Open app",
  },
  "productName.client": {
    zh: "Context-OS",
    en: "Context-OS",
  },
  "productName.short": {
    zh: "Context-OS",
    en: "Context-OS",
  },
  "productChrome.terms": {
    zh: "用户协议",
    en: "Terms",
  },
  "productChrome.privacy": {
    zh: "隐私政策",
    en: "Privacy",
  },
  "productChrome.licenses": {
    zh: "开源声明",
    en: "Open source",
  },
  "accountMenu.allSettings": {
    zh: "所有设置",
    en: "All settings",
  },
  "accountMenu.adminConsole": {
    zh: "管理后台",
    en: "Admin console",
  },
  "accountMenu.help": {
    zh: "帮助",
    en: "Help",
  },
  /** User-card CTA in account menu → open usage/membership quick surface. */
  "accountMenu.manageMembership": {
    zh: "会员管理",
    en: "Membership",
  },
  "accountMenu.upgradeMembership": {
    zh: "升级会员",
    en: "Upgrade plan",
  },

  "settingsTabBar.searchLabel": {
    zh: "搜索设置",
    en: "Search settings",
  },
  "settingsTabBar.searchPlaceholder": {
    zh: "搜索设置…",
    en: "Search settings…",
  },

  commonAnalytics: {
    zh: "数据分析",
    en: "Analytics",
  },

  // ADR-0010 referral fab / modal (dashboard + workspace)
  "referral.fabLabel": {
    zh: "邀请好友赚赠送金",
    en: "Invite friends for gift credit",
  },
  "referral.fabText": {
    zh: "邀请有礼",
    en: "Invite & earn",
  },
  "referral.modalTitle": {
    zh: "邀请好友 · 双方各得 ¥5",
    en: "Invite friends · ¥5 each",
  },
  "referral.hero": {
    zh: "把知识库带给同事与朋友：他们注册你得奖励，你也帮他们开局。",
    en: "Share Context-OS with colleagues and friends — you both earn gift credit when they join.",
  },
  "referral.bulletBoth": {
    zh: "好友用你的邀请码完成注册并通过验证：你与好友各得 ¥5 赠送金（入余额）。",
    en: "When a friend registers with your code and verifies: you both get ¥5 gift credit in the wallet.",
  },
  "referral.bulletStack": {
    zh: "好友仍享注册礼 ¥20，与邀请奖励叠加（被邀请人合计最高 ¥25 赠送面值）。",
    en: "New users still get the ¥20 signup grant; referral stacks on top (invitee up to ¥25 gift face value).",
  },
  "referral.bulletQuota": {
    zh: "邀请次数：基础 5 次；每累计充值 ¥50 现金再 +1 次（仅实付充值计入）。",
    en: "Invite quota: 5 base + 1 per ¥50 lifetime paid top-up (cash only).",
  },
  "referral.bulletWalletOnly": {
    zh: "奖励只加钱包余额，不增加可分享工作区名额；与工作区协作邀请无关。",
    en: "Rewards top up wallet balance only — not share slots. Separate from workspace member invites.",
  },
  "referral.shareLinkLabel": {
    zh: "邀请链接",
    en: "Invite link",
  },
  "referral.copyLink": {
    zh: "复制链接",
    en: "Copy link",
  },
  "referral.progress": {
    zh: "已成功 {rewarded} / 额度 {quota} · 剩余 {remaining}",
    en: "{rewarded} rewarded / quota {quota} · {remaining} left",
  },
  "referral.finePrint": {
    zh: "自邀与异常设备将被拒绝且不计奖。参数见产品商业模式（ADR-0010）。",
    en: "Self-invite and abuse are rejected without reward. Rules: product ADR-0010.",
  },
  "referral.loading": {
    zh: "加载邀请信息…",
    en: "Loading referral…",
  },
  "referral.loadError": {
    zh: "暂时无法加载邀请码，请稍后再试。",
    en: "Could not load your referral code. Try again later.",
  },
} satisfies Record<string, UiMessageDescriptor>;
