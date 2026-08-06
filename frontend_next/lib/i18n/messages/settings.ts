import type { UiMessageDescriptor } from "./types";

export const settingsMessages = {
  "settings.tabsLabel": {
    zh: "设置选项",
    en: "Settings tabs",
  },
  "settings.tabs.profile": {
    zh: "个人资料",
    en: "Profile",
  },
  "settings.tabs.billing": {
    zh: "会员状态",
    en: "Membership",
  },
  "settings.tabs.appearance": {
    zh: "偏好",
    en: "Preferences",
  },
  "settings.tabs.preferences": {
    zh: "偏好",
    en: "Preferences",
  },
  "settings.tabs.notifications": {
    zh: "通知",
    en: "Notifications",
  },
  "settings.tabs.security": {
    zh: "安全",
    en: "Security",
  },
  "settings.pageTitle": {
    zh: "设置",
    en: "Settings",
  },
  "settings.pageSubtitle": {
    zh: "管理会员状态、资料、外观、通知和安全设置。",
    en: "Manage membership, profile, appearance, notification, and security settings.",
  },
  "settings.profile.sectionTitle": {
    zh: "个人资料",
    en: "Profile",
  },
  "settings.profile.sectionSubtitle": {
    zh: "展示在分享页的 Owner 名片：显示名、简介、头像、封面与联系链接。",
    en: "Shown on shared pages as your Owner card: name, bio, avatar, banner, and contact link.",
  },
  "settings.profile.emailLabel": {
    zh: "邮箱",
    en: "Email",
  },
  "settings.profile.nameLabel": {
    zh: "显示名称",
    en: "Display name",
  },
  "settings.profile.namePlaceholder": {
    zh: "输入你的显示名称",
    en: "Enter your display name",
  },
  "settings.profile.bioLabel": {
    zh: "简介",
    en: "Bio",
  },
  "settings.profile.bioPlaceholder": {
    zh: "一句话介绍你自己（可选）",
    en: "A short intro about you (optional)",
  },
  "settings.profile.contactLabel": {
    zh: "联系链接",
    en: "Contact link",
  },
  "settings.profile.contactPlaceholder": {
    zh: "https://…",
    en: "https://…",
  },
  "settings.profile.avatarLabel": {
    zh: "头像",
    en: "Avatar",
  },
  "settings.profile.bannerLabel": {
    zh: "封面图",
    en: "Banner",
  },
  "settings.profile.uploadAction": {
    zh: "上传",
    en: "Upload",
  },
  "settings.profile.removeMediaAction": {
    zh: "移除",
    en: "Remove",
  },
  "settings.profile.mediaHint": {
    zh: "支持 JPEG / PNG / WebP / GIF；头像 ≤2MB，封面 ≤5MB。",
    en: "JPEG / PNG / WebP / GIF; avatar ≤2MB, banner ≤5MB.",
  },
  "settings.profile.invalidContactUrl": {
    zh: "联系链接须为 http(s) URL。",
    en: "Contact link must be an http(s) URL.",
  },
  "settings.profile.bioTooLong": {
    zh: "简介不能超过 500 个字符。",
    en: "Bio must be at most 500 characters.",
  },
  "settings.profile.saveAction": {
    zh: "保存资料",
    en: "Save profile",
  },
  "settings.billing.sectionTitle": {
    zh: "会员状态",
    en: "Membership",
  },
  "settings.billing.sectionSubtitle": {
    zh: "档位、分享名额、到期时间与余额一览。更换方案请前往定价页。",
    en: "Plan tier, share slots, renewal, and balance. Open pricing to change plan.",
  },
  "settings.billing.currentPlanLabel": {
    zh: "当前档位",
    en: "Current tier",
  },
  "settings.billing.shareQuotaLabel": {
    zh: "分享名额",
    en: "Share slots",
  },
  "settings.billing.shareQuotaValue": {
    zh: "{used} / {max}",
    en: "{used} / {max}",
  },
  "settings.billing.managePlanAction": {
    zh: "升级 / 管理方案",
    en: "Upgrade / manage plan",
  },
  "settings.billing.changePlanAction": {
    zh: "更换方案",
    en: "Change plan",
  },
  "settings.billing.usageDetailsLink": {
    zh: "消费明细",
    en: "Usage details",
  },
  "settings.billing.providerSummaryNone": {
    zh: "未配置自定义 Provider",
    en: "No custom provider",
  },
  "settings.billing.providerSummaryActive": {
    zh: "已配置 · {provider}",
    en: "Configured · {provider}",
  },
  "settings.billing.portalUnavailable": {
    zh: "自助支付门户暂未开通。请稍后重试，或联系支持。",
    en: "Self-service payment portal is not available. Try again later, or contact support.",
  },
  "settings.billing.portalFallbackHint": {
    zh: "请打开定价页完成 Creem/支付宝 支付。",
    en: "Open pricing for Creem/Alipay checkout.",
  },
  "settings.billing.planPickerHint": {
    zh: "选择方案后将进入定价/结账页完成变更。",
    en: "Selecting a plan opens the pricing/checkout page to complete the change.",
  },
  "settings.billing.openPricingPage": {
    zh: "打开定价页",
    en: "Open pricing",
  },
  "settings.billing.currentPlanBadge": {
    zh: "当前",
    en: "Current",
  },
  "settings.billing.portalAction": {
    zh: "打开支付门户",
    en: "Open payment portal",
  },
  "settings.billing.referralTitle": {
    zh: "邀请码",
    en: "Referral",
  },
  "settings.billing.referralSubtitle": {
    zh: "好友注册填写你的邀请码，双方各得 ¥5 赠送金。",
    en: "Friends who register with your code earn ¥5 gift credit each.",
  },
  "settings.billing.referralCodeLabel": {
    zh: "我的邀请码",
    en: "Your code",
  },
  "settings.billing.referralRewardedLabel": {
    zh: "已成功邀请",
    en: "Rewarded invites",
  },
  "settings.billing.referralCopy": {
    zh: "复制",
    en: "Copy",
  },
  "settings.billing.referralCopied": {
    zh: "已复制",
    en: "Copied",
  },
  "settings.billing.byokTitle": {
    zh: "自定义 Provider",
    en: "Custom provider",
  },
  "settings.billing.byokSubtitle": {
    zh: "密钥加密存储。自定义 Provider 只用于对话模型；向量与检索始终走平台，并从余额扣费。",
    en: "Keys are encrypted at rest. A custom provider covers chat models only; embedding and retrieval always use the platform and bill your balance.",
  },
  "settings.billing.byokProvider": {
    zh: "Provider",
    en: "Provider",
  },
  "settings.billing.byokBaseUrl": {
    zh: "Base URL（可选）",
    en: "Base URL (optional)",
  },
  "settings.billing.byokModelHint": {
    zh: "模型提示（可选）",
    en: "Model hint (optional)",
  },
  "settings.billing.byokApiKey": {
    zh: "API Key",
    en: "API Key",
  },
  "settings.billing.byokSave": {
    zh: "保存密钥",
    en: "Save key",
  },
  "settings.billing.byokRevoke": {
    zh: "吊销",
    en: "Revoke",
  },
  "settings.billing.byokRevoked": {
    zh: "已吊销",
    en: "Revoked",
  },
  "settings.billing.payerFundsRequired": {
    zh: "余额不足，且未配置可用的自定义 Provider。请充值或添加 Provider 后再试。",
    en: "Balance is empty and no custom provider is configured. Top up or add a provider.",
  },
  "settings.billing.payerFundsTopUpHint": {
    zh: "前往设置 → 会员状态充值，或配置自定义 Provider。",
    en: "Open Settings → Membership to top up, or configure a custom provider.",
  },
  "settings.appearance.sectionTitle": {
    zh: "外观",
    en: "Appearance",
  },
  "settings.appearance.sectionSubtitle": {
    zh: "控制工作台和后台页面的明暗观感。",
    en: "Control how the workspace and admin surfaces look.",
  },
  "settings.appearance.localeSubtitle": {
    zh: "选择界面显示语言。",
    en: "Choose the interface display language.",
  },
  "settings.appearance.themeLabel": {
    zh: "主题",
    en: "Theme",
  },
  "settings.appearance.localeLabel": {
    zh: "界面语言",
    en: "Interface language",
  },
  "settings.appearance.theme.system": {
    zh: "跟随系统",
    en: "System",
  },
  "settings.appearance.theme.light": {
    zh: "浅色",
    en: "Light",
  },
  "settings.appearance.theme.dark": {
    zh: "深色",
    en: "Dark",
  },
  "settings.notifications.sectionTitle": {
    zh: "通知",
    en: "Notifications",
  },
  "settings.notifications.sectionSubtitle": {
    zh: "管理消息偏好、摘要频率和免打扰时段。",
    en: "Manage message preferences, digest cadence, and quiet hours.",
  },
  "settings.notifications.emailUpdatesLabel": {
    zh: "邮件更新",
    en: "Email updates",
  },
  "settings.notifications.weeklyDigestLabel": {
    zh: "每周摘要",
    en: "Weekly digest",
  },
  "settings.notifications.quietHoursStartLabel": {
    zh: "免打扰开始时间",
    en: "Quiet hours start",
  },
  "settings.notifications.quietHoursEndLabel": {
    zh: "免打扰结束时间",
    en: "Quiet hours end",
  },
  "settings.notifications.saveAction": {
    zh: "保存通知设置",
    en: "Save notification settings",
  },
  "settings.notifications.emptyTitle": {
    zh: "还没有通知",
    en: "No notifications yet",
  },
  "settings.notifications.emptyBody": {
    zh: "新的系统通知和账户提醒会显示在这里。",
    en: "New system notices and account alerts will appear here.",
  },
  "settings.security.sectionTitle": {
    zh: "安全",
    en: "Security",
  },
  "settings.security.sectionSubtitle": {
    zh: "更新密码并检查账户访问安全。",
    en: "Update your password and review account access security.",
  },
  "settings.security.currentPasswordLabel": {
    zh: "当前密码",
    en: "Current password",
  },
  "settings.security.newPasswordLabel": {
    zh: "新密码",
    en: "New password",
  },
  "settings.security.changePasswordAction": {
    zh: "修改密码",
    en: "Change password",
  },
  "settings.security.signOutOtherSessionsAction": {
    zh: "退出其他会话",
    en: "Sign out other sessions",
  },
  "settings.loadError": {
    zh: "加载设置失败。",
    en: "Failed to load settings.",
  },
  "settings.saveSuccess": {
    zh: "设置已保存。",
    en: "Settings saved.",
  },
  "settings.saveError": {
    zh: "保存设置失败。",
    en: "Failed to save settings.",
  },
  "settings.profile.notAuthenticated": {
    zh: "请先登录。",
    en: "Please sign in first.",
  },
  "settings.profile.nameTooLong": {
    zh: "姓名长度不能超过 120 个字符。",
    en: "Name must be 120 characters or fewer.",
  },
  "settings.usage.sectionTitle": {
    zh: "平台保护限速（参考）",
    en: "Platform protective limits (reference)",
  },
  "settings.usage.sectionSubtitle": {
    zh: "仅在无余额且未配置自定义 Provider 时可能触发。有余额或自定义 Provider 后，对话不再受此限制主导。",
    en: "May apply only when balance is empty and no custom provider is set. With balance or a custom provider, chat is not driven by these windows.",
  },
  "settings.usage.loading": {
    zh: "正在加载用量...",
    en: "Loading usage...",
  },
  "settings.usage.empty": {
    zh: "暂无用量信息。",
    en: "No usage information.",
  },
  "settings.usage.scopeLabel": {
    zh: "配额作用域",
    en: "Quota scope",
  },
  "settings.usage.policyLabel": {
    zh: "当前策略",
    en: "Policy",
  },
  "settings.usage.policyEnabled": {
    zh: "已启用",
    en: "Enabled",
  },
  "settings.usage.policyDisabled": {
    zh: "未启用",
    en: "Disabled",
  },
  "settings.usage.estimated": {
    zh: "当前用量包含估算值。",
    en: "Current usage includes estimated values.",
  },
  "settings.usage.window5h": {
    zh: "5 小时窗口",
    en: "5-hour window",
  },
  "settings.usage.window7d": {
    zh: "7 天窗口",
    en: "7-day window",
  },
  "settings.usage.remaining": {
    zh: "剩余",
    en: "Remaining",
  },
  "settings.usage.nextRelief": {
    zh: "下次释放",
    en: "Next relief",
  },
  "settings.usage.blocked": {
    zh: "当前已触发限流",
    en: "Currently blocked",
  },
  "settings.usage.breakdownTitle": {
    zh: "分项消耗",
    en: "Breakdown",
  },
  "settings.usage.notSet": {
    zh: "未设置",
    en: "Not set",
  },
  "settings.usage.quotaScopePlanDefault": {
    zh: "默认方案 · {planId}",
    en: "Plan default - {planId}",
  },
  "settings.usage.quotaScopeUserOverride": {
    zh: "用户覆盖策略",
    en: "User override",
  },
  "settings.metric.embedding_tokens": {
    zh: "嵌入令牌",
    en: "Embedding tokens",
  },
  "settings.metric.llm_input_tokens": {
    zh: "模型输入令牌",
    en: "LLM input tokens",
  },
  "settings.metric.llm_output_tokens": {
    zh: "模型输出令牌",
    en: "LLM output tokens",
  },
  "settings.metric.pages_processed": {
    zh: "处理页数",
    en: "Pages processed",
  },
  "settings.metric.storage_bytes": {
    zh: "存储空间",
    en: "Storage",
  },
  "settings.billing.statusLabel": {
    zh: "状态",
    en: "Status",
  },
  "settings.billing.renewsOnLabel": {
    zh: "续费日期",
    en: "Renews on",
  },
  "settings.billing.notActive": {
    zh: "未开通",
    en: "Not active",
  },
  "settings.billing.loading": {
    zh: "正在加载会员信息...",
    en: "Loading membership information...",
  },
  "settings.billing.loadingUsage": {
    zh: "正在加载用量...",
    en: "Loading usage...",
  },
  "settings.billing.noUsageData": {
    zh: "暂无用量数据。",
    en: "No usage data.",
  },
  "settings.billing.loadingPlans": {
    zh: "正在加载方案...",
    en: "Loading plans...",
  },
  "settings.billing.noPlans": {
    zh: "暂无可用方案。",
    en: "No plans available.",
  },
  "settings.billing.loadingPortal": {
    zh: "加载中...",
    en: "Loading...",
  },
  "settings.billing.portalEmpty": {
    zh: "订阅入口返回为空。",
    en: "Billing portal URL is empty.",
  },
  "settings.billing.usageTitle": {
    zh: "用量",
    en: "Usage",
  },
  "settings.billing.availablePlansTitle": {
    zh: "可用方案",
    en: "Available plans",
  },
  "settings.billing.tokensLabel": {
    zh: "令牌",
    en: "Tokens",
  },
  "settings.billing.documentsLabel": {
    zh: "文档",
    en: "Documents",
  },
  "settings.billing.failedData": {
    zh: "部分会员信息加载失败：{items}",
    en: "Some membership data failed to load: {items}",
  },
  "settings.billing.failedItem.subscription": {
    zh: "订阅",
    en: "subscription",
  },
  "settings.billing.failedItem.usage": {
    zh: "用量",
    en: "usage",
  },
  "settings.billing.failedItem.plans": {
    zh: "方案",
    en: "plans",
  },
  "settings.billing.status.active": {
    zh: "有效",
    en: "Active",
  },
  "settings.billing.status.past_due": {
    zh: "逾期",
    en: "Past due",
  },
  "settings.billing.status.canceled": {
    zh: "已取消",
    en: "Canceled",
  },
  "settings.billing.walletTitle": {
    zh: "余额",
    en: "Balance",
  },
  "settings.billing.walletSubtitle": {
    zh: "用于平台模型调用与向量检索。充值会计入累计充值（邀请额度每 ¥50 增加 1 次）。",
    en: "Pays platform model calls and embedding/retrieval. Paid top-ups count toward lifetime total (+1 invite per ¥50).",
  },
  "settings.billing.walletBalanceLabel": {
    zh: "可用余额",
    en: "Available balance",
  },
  "settings.billing.walletLifetimePaidLabel": {
    zh: "累计充值",
    en: "Lifetime paid top-ups",
  },
  "settings.billing.walletTopupTitle": {
    zh: "充值",
    en: "Top up",
  },
  "settings.billing.walletTopupAction": {
    zh: "充值 {label}",
    en: "Top up {label}",
  },
  "settings.billing.walletTopupLoading": {
    zh: "正在创建结账…",
    en: "Starting checkout…",
  },
  "settings.billing.walletTopupFailed": {
    zh: "无法开始充值：{message}",
    en: "Could not start top-up: {message}",
  },
  "settings.billing.walletLoading": {
    zh: "正在加载余额…",
    en: "Loading balance…",
  },
  "settings.billing.failedItem.wallet": {
    zh: "余额",
    en: "balance",
  },
  "settings.appearance.themeDescription.system": {
    zh: "自动匹配设备当前主题。",
    en: "Follow the operating system preference.",
  },
  "settings.appearance.themeDescription.light": {
    zh: "适合白天和高亮环境。",
    en: "Keep surfaces bright and high-contrast.",
  },
  "settings.appearance.themeDescription.dark": {
    zh: "降低夜间眩光，强调工作区层次。",
    en: "Reduce glare and emphasize layered work surfaces.",
  },
  "settings.appearance.localeDescription.zh-CN": {
    zh: "面向当前主要用户群的默认语言。",
    en: "The default language for the current primary audience.",
  },
  "settings.appearance.localeDescription.en": {
    zh: "适合跨团队协作或对外演示。",
    en: "Useful for cross-team collaboration and external walkthroughs.",
  },
  "settings.appearance.currentTheme": {
    zh: "当前主题",
    en: "Current theme",
  },
  "settings.appearance.currentLanguage": {
    zh: "当前语言",
    en: "Current language",
  },
  "settings.notifications.productUpdatesLabel": {
    zh: "产品动态",
    en: "Product updates",
  },
  "settings.notifications.securityAlertsLabel": {
    zh: "安全提醒",
    en: "Security alerts",
  },
  "settings.notifications.historyTitle": {
    zh: "通知历史",
    en: "Notification history",
  },
  "settings.notifications.bellLabel": {
    zh: "通知",
    en: "Notifications",
  },
  "settings.notifications.listLabel": {
    zh: "通知列表",
    en: "Notification list",
  },
  "settings.notifications.unreadCount": {
    zh: "{count} 条未读",
    en: "{count} unread",
  },
  "settings.notifications.loading": {
    zh: "正在加载通知...",
    en: "Loading notifications...",
  },
  "settings.notifications.markRead": {
    zh: "标记已读",
    en: "Mark as read",
  },
  "settings.notifications.processing": {
    zh: "处理中...",
    en: "Processing...",
  },
  "settings.notifications.read": {
    zh: "已读",
    en: "Read",
  },
  "settings.notifications.quietHoursPlaceholderStart": {
    zh: "例如 22:00",
    en: "For example 22:00",
  },
  "settings.notifications.quietHoursPlaceholderEnd": {
    zh: "例如 08:00",
    en: "For example 08:00",
  },
  "settings.notifications.invalidTime": {
    zh: "请使用 24 小时制 HH:MM。",
    en: "Use HH:MM in 24-hour time.",
  },
  "settings.notifications.event.product_update": {
    zh: "产品动态",
    en: "Product update",
  },
  "settings.notifications.event.security_alert": {
    zh: "安全提醒",
    en: "Security alert",
  },
  "settings.notifications.event.weekly_digest": {
    zh: "每周摘要",
    en: "Weekly digest",
  },
  "settings.security.notAuthenticated": {
    zh: "尚未登录。",
    en: "Not authenticated.",
  },
  "settings.security.missingPassword": {
    zh: "请输入当前密码和新密码。",
    en: "Enter the current password and the new password.",
  },
  "settings.security.updating": {
    zh: "更新中...",
    en: "Updating...",
  },
  "settings.security.failed": {
    zh: "修改密码失败，请稍后再试。",
    en: "Failed to change the password. Try again later.",
  },
  "settings.security.resetPasswordAction": {
    zh: "重置密码",
    en: "Reset password",
  },
  "settings.security.currentSessionTitle": {
    zh: "当前会话",
    en: "Current session",
  },
  "settings.security.signedInAs": {
    zh: "当前登录账号",
    en: "Signed in as",
  },
  "settings.security.unknownAccount": {
    zh: "未知账号",
    en: "Unknown account",
  },
} satisfies Record<string, UiMessageDescriptor>;
