import type { UiLocale } from "./i18n/config";
import { formatUiMessage, UI_MESSAGES } from "./i18n/messages";

type UiMessageDescriptor = {
  zh: string;
  en: string;
};

const SURFACE_UI_MESSAGES = {
  commonUnlimited: {
    zh: "不限",
    en: "Unlimited",
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
    zh: "个人用量",
    en: "Personal usage",
  },
  "settings.usage.sectionSubtitle": {
    zh: "查看当前账号的配额窗口和分项用量。",
    en: "Review quota windows and current usage breakdown.",
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
    zh: "正在加载账单信息...",
    en: "Loading billing information...",
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
    zh: "部分账单信息加载失败：{items}",
    en: "Some billing data failed to load: {items}",
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
  "shareCenter.pageTitle": {
    zh: "分享与追踪",
    en: "Share & track",
  },
  "shareCenter.pageSubtitle": {
    zh: "发布当前工作区的知识组合，并追踪它的传播情况。",
    en: "Publish this workspace as a knowledge package and track how it spreads.",
  },
  "shareCenter.controlBarTitle": {
    zh: "分享控制",
    en: "Share controls",
  },
  "shareCenter.controlBarSubtitle": {
    zh: "用一个开关管理分享状态，并维护当前入口链接和有效期。",
    en: "Use one control to manage share state and maintain the current link and validity window.",
  },
  "shareCenter.controlBarNoLink": {
    zh: "当前还没有可用的分享链接",
    en: "No active share link yet",
  },
  "shareCenter.settingsSectionTitle": {
    zh: "分享设置",
    en: "Share settings",
  },
  "shareCenter.settingsSectionSubtitle": {
    zh: "控制分享入口、互动规则、下载权限和链接有效期。",
    en: "Control the share link, interaction rules, download policy, and expiry.",
  },
  "shareCenter.readAccessLabel": {
    zh: "访问方式",
    en: "Read access",
  },
  "shareCenter.readAccessValue": {
    zh: "任何人可查看",
    en: "Anyone with the link can view",
  },
  "shareCenter.interactionAccessLabel": {
    zh: "互动权限",
    en: "Interaction",
  },
  "shareCenter.interactionAccessValue": {
    zh: "登录后可提问",
    en: "Sign in to ask questions",
  },
  "shareCenter.interactionRuleHint": {
    zh: "任何拿到分享链接的人都可以查看这份知识内容。为了控制 AI 成本，登录后才能发起提问和互动。",
    en: "Anyone with the share link can view this knowledge. To control AI cost, users must sign in before asking questions and interacting.",
  },
  "shareCenter.shareSwitchLabel": {
    zh: "分享开关",
    en: "Share switch",
  },
  "shareCenter.validityLabel": {
    zh: "有效期",
    en: "Validity",
  },
  "shareCenter.validityHint": {
    zh: "修改有效期后，点击“更新分享”生成新链接即可生效。",
    en: 'Change the validity window, then click "Update share" to generate a new link.',
  },
  "shareCenter.validityOption7d": {
    zh: "7 天",
    en: "7 days",
  },
  "shareCenter.validityOption30d": {
    zh: "30 天",
    en: "30 days",
  },
  "shareCenter.validityOption90d": {
    zh: "90 天",
    en: "90 days",
  },
  "shareCenter.validityOptionNever": {
    zh: "长期有效",
    en: "No expiry",
  },
  "shareCenter.inviteSectionTitle": {
    zh: "成员与权限",
    en: "Members & permissions",
  },
  "shareCenter.inviteSectionSubtitle": {
    zh: "邀请成员参与当前 Workspace，并管理他们的访问权限。",
    en: "Invite collaborators to this workspace and manage their access.",
  },
  "shareCenter.backToWorkspace": {
    zh: "返回 Workspace",
    en: "Back to workspace",
  },
  "shareCenter.loading": {
    zh: "正在加载分享数据...",
    en: "Loading share data...",
  },
  "shareCenter.analyticsLoading": {
    zh: "正在加载分享分析...",
    en: "Loading share analytics...",
  },
  "shareCenter.accessLogsLoading": {
    zh: "正在加载访问日志...",
    en: "Loading access logs...",
  },
  "shareCenter.settingsLoadError": {
    zh: "加载分享设置失败。",
    en: "Failed to load share settings.",
  },
  "shareCenter.membersLoadError": {
    zh: "加载成员列表失败。",
    en: "Failed to load members.",
  },
  "shareCenter.analyticsLoadError": {
    zh: "加载访问数据失败。",
    en: "Failed to load analytics.",
  },
  "shareCenter.accessLogsLoadError": {
    zh: "加载最近活动失败。",
    en: "Failed to load recent activity.",
  },
  "shareCenter.copyLinkAction": {
    zh: "复制链接",
    en: "Copy link",
  },
  "shareCenter.openShareAction": {
    zh: "打开分享页",
    en: "Open share page",
  },
  "shareCenter.updateShareAction": {
    zh: "更新分享",
    en: "Update share",
  },
  "shareCenter.copyLinkSuccess": {
    zh: "分享链接已复制。",
    en: "Share link copied.",
  },
  "shareCenter.copyLinkError": {
    zh: "复制分享链接失败。",
    en: "Failed to copy share link.",
  },
  "shareCenter.shareLinkUnavailable": {
    zh: "当前没有可用的分享链接。",
    en: "There is no active share link right now.",
  },
  "shareCenter.updateShareSuccess": {
    zh: "分享链接已更新。",
    en: "Share link updated.",
  },
  "shareCenter.downloadPolicyTitle": {
    zh: "下载策略",
    en: "Download policy",
  },
  "shareCenter.downloadAllowed": {
    zh: "允许下载",
    en: "Downloads allowed",
  },
  "shareCenter.downloadRestricted": {
    zh: "仅在线查看",
    en: "Online only",
  },
  "shareCenter.notSet": {
    zh: "未设置",
    en: "Not set",
  },
  "shareCenter.expiresAtHint": {
    zh: "仅在生成新链接时生效，例如 2026-04-30T18:00:00Z。",
    en: "Applied only when generating a new share link, for example 2026-04-30T18:00:00Z.",
  },
  "shareCenter.expiresAtReadOnlyHint": {
    zh: "当前链接的过期时间需在撤销后重新生成链接时调整。",
    en: "To change this expiry, revoke the current link and generate a new one.",
  },
  "shareCenter.loginRequired": {
    zh: "请先登录。",
    en: "Please sign in first.",
  },
  "shareCenter.inviteSending": {
    zh: "邀请中...",
    en: "Sending...",
  },
  "shareCenter.inviteEmailRequired": {
    zh: "请输入成员邮箱。",
    en: "Enter an email address.",
  },
  "shareCenter.inviteEmailInvalid": {
    zh: "请输入有效的邮箱地址。",
    en: "Enter a valid email address.",
  },
  "shareCenter.memberRole.viewer": {
    zh: "查看者",
    en: "Viewer",
  },
  "shareCenter.memberRole.editor": {
    zh: "编辑者",
    en: "Editor",
  },
  "shareCenter.memberRole.owner": {
    zh: "所有者",
    en: "Owner",
  },
  "shareCenter.memberStatus.pending": {
    zh: "待接受",
    en: "Pending",
  },
  "shareCenter.memberStatus.accepted": {
    zh: "已接受",
    en: "Accepted",
  },
  "shareCenter.memberStatus.revoked": {
    zh: "已撤销",
    en: "Revoked",
  },
  "shareCenter.memberInvitedAt": {
    zh: "邀请时间：{value}",
    en: "Invited at: {value}",
  },
  "shareCenter.removeAction": {
    zh: "移除",
    en: "Remove",
  },
  "shareCenter.confirmRemoveAction": {
    zh: "确认移除",
    en: "Confirm remove",
  },
  "shareCenter.removePendingHint": {
    zh: "确认从当前 Workspace 中移除 {name} 吗？",
    en: "Remove {name} from this workspace?",
  },
  "shareCenter.removeError": {
    zh: "移除成员失败。",
    en: "Failed to remove member.",
  },
  "shareCenter.metricUnavailable": {
    zh: "--",
    en: "--",
  },
  "shareCenter.statusInactive": {
    zh: "未开启",
    en: "Inactive",
  },
  "shareCenter.statusActive": {
    zh: "分享中",
    en: "Live",
  },
  "shareCenter.statusExpired": {
    zh: "已过期",
    en: "Expired",
  },
  "shareCenter.overviewSectionTitle": {
    zh: "传播概览",
    en: "Distribution overview",
  },
  "shareCenter.overviewSectionSubtitle": {
    zh: "用当前已经采集到的数据，判断这份知识是否开始传播、是否仍在传播。",
    en: "Use current telemetry to check whether this knowledge package has started spreading and whether it is still active.",
  },
  "shareCenter.overviewCurrentStatus": {
    zh: "当前状态",
    en: "Current status",
  },
  "shareCenter.overviewTotalViews": {
    zh: "总访问次数",
    en: "Total views",
  },
  "shareCenter.overviewRecentViews": {
    zh: "近 7 天访问",
    en: "Views in last 7 days",
  },
  "shareCenter.overviewActiveDays": {
    zh: "近 30 天活跃天数",
    en: "Active days in last 30 days",
  },
  "shareCenter.overviewLastAccess": {
    zh: "最近一次访问",
    en: "Last access",
  },
  "shareCenter.trendSectionTitle": {
    zh: "访问趋势",
    en: "Access trend",
  },
  "shareCenter.trendSectionSubtitle": {
    zh: "查看这份知识最近的传播节奏。",
    en: "Review the recent rhythm of distribution.",
  },
  "shareCenter.trendRange7": {
    zh: "7 天",
    en: "7 days",
  },
  "shareCenter.trendRange30": {
    zh: "30 天",
    en: "30 days",
  },
  "shareCenter.trendEmptyTitle": {
    zh: "还没有访问数据",
    en: "No access data yet",
  },
  "shareCenter.trendEmptyBody": {
    zh: "开启分享后，这里会显示最近的访问趋势。",
    en: "Once sharing is enabled and people start visiting, the recent trend will appear here.",
  },
  "shareCenter.activitySectionTitle": {
    zh: "最近活动",
    en: "Recent activity",
  },
  "shareCenter.activitySectionSubtitle": {
    zh: "查看最近发生的访问行为。",
    en: "See the most recent access activity.",
  },
  "shareCenter.activityActionLabel": {
    zh: "动作",
    en: "Action",
  },
  "shareCenter.activityTimeLabel": {
    zh: "时间",
    en: "Time",
  },
  "shareCenter.activityEmptyTitle": {
    zh: "还没有最近活动",
    en: "No recent activity",
  },
  "shareCenter.activityEmptyBody": {
    zh: "一旦有人访问分享页，这里会出现最新记录。",
    en: "New visits will show up here once people access the shared page.",
  },
  "shareCenter.diagnosticsSectionTitle": {
    zh: "传播诊断",
    en: "Distribution diagnostics",
  },
  "shareCenter.diagnosticsSectionSubtitle": {
    zh: "把原始访问数据翻译成下一步动作建议。",
    en: "Translate raw access data into next actions.",
  },
  "shareCenter.diagnosticsLoading": {
    zh: "正在生成传播诊断...",
    en: "Building diagnostics...",
  },
  "shareCenter.diagnosticsDisabledTitle": {
    zh: "传播尚未开始",
    en: "Distribution has not started",
  },
  "shareCenter.diagnosticsDisabledBody": {
    zh: "开启分享后，这里会开始追踪这份知识的传播情况。",
    en: "Enable sharing to start tracking distribution for this knowledge package.",
  },
  "shareCenter.diagnosticsNotStartedTitle": {
    zh: "未启动传播",
    en: "Distribution not started",
  },
  "shareCenter.diagnosticsNotStartedBody": {
    zh: "这份知识已经可以被访问，但还没有产生访问。你可能还没有分发出去，或者入口不够明确。",
    en: "This knowledge package is available, but nobody has accessed it yet. You may need to distribute it or make the entry point clearer.",
  },
  "shareCenter.diagnosticsStalledTitle": {
    zh: "传播停滞",
    en: "Distribution stalled",
  },
  "shareCenter.diagnosticsStalledBody": {
    zh: "这份知识曾被访问，但近 7 天没有继续传播。可以考虑重新分发，或优化分享说明。",
    en: "This knowledge package was accessed before, but there have been no visits in the last 7 days. Consider redistributing it or clarifying the share description.",
  },
  "shareCenter.diagnosticsSpikeTitle": {
    zh: "短时爆发",
    en: "Short-lived spike",
  },
  "shareCenter.diagnosticsSpikeBody": {
    zh: "访问集中在少数几天内，传播更像一次性曝光，而不是持续使用。",
    en: "Visits are concentrated in only a few days, which looks more like a one-off spike than sustained usage.",
  },
  "shareCenter.diagnosticsExpiringTitle": {
    zh: "即将过期",
    en: "Expiring soon",
  },
  "shareCenter.diagnosticsExpiringBody": {
    zh: "当前分享将在 3 天内过期。如果你希望继续传播，需要更新分享设置。",
    en: "The current share will expire within 3 days. Update the share settings if you want distribution to continue.",
  },
  "shareCenter.diagnosticsExpiredTitle": {
    zh: "分享已过期",
    en: "Share expired",
  },
  "shareCenter.diagnosticsExpiredBody": {
    zh: "链接已经失效。历史访问仍然保留，但要继续传播需要重新生成或更新分享。",
    en: "The link has expired. Historical activity is still available, but you need to regenerate or update sharing to continue distribution.",
  },
  "shareCenter.diagnosticsHealthyTitle": {
    zh: "传播仍在继续",
    en: "Distribution is ongoing",
  },
  "shareCenter.diagnosticsHealthyBody": {
    zh: "最近仍有访问，当前这份知识处于持续传播状态。",
    en: "Recent visits are still coming in, which suggests this knowledge package is still actively spreading.",
  },
} satisfies Record<string, UiMessageDescriptor>;

type SurfaceUiMessageKey = keyof typeof SURFACE_UI_MESSAGES;
type CoreUiMessageKey = keyof typeof UI_MESSAGES;

export type SettingsShareUiMessageKey = SurfaceUiMessageKey | CoreUiMessageKey;

function interpolate(template: string, values?: Record<string, string | number>) {
  if (!values) {
    return template;
  }

  return template.replace(/\{(\w+)\}/g, (_match, key: string) => String(values[key] ?? ""));
}

export function formatSettingsShareMessage(
  locale: UiLocale,
  key: SettingsShareUiMessageKey,
  values?: Record<string, string | number>,
) {
  if (Object.prototype.hasOwnProperty.call(SURFACE_UI_MESSAGES, key)) {
    const descriptor = SURFACE_UI_MESSAGES[key as SurfaceUiMessageKey];
    const template = locale === "zh-CN" ? descriptor.zh : descriptor.en;
    return interpolate(template, values);
  }

  return formatUiMessage(locale, key as CoreUiMessageKey, values);
}
