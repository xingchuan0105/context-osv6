import type { UiMessageDescriptor } from "./types";

export const settingsMessages = {
  "settings.tabsLabel": {
    zh: "设置选项",
    en: "Settings tabs",
  },
  "settings.tabs.profile": {
    zh: "资料",
    en: "Profile",
  },
  "settings.tabs.billing": {
    zh: "账单",
    en: "Billing",
  },
  "settings.tabs.appearance": {
    zh: "外观",
    en: "Appearance",
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
    zh: "管理账单、资料、外观、通知和安全设置。",
    en: "Manage billing, profile, appearance, notification, and security settings.",
  },
  "settings.profile.sectionTitle": {
    zh: "个人资料",
    en: "Profile",
  },
  "settings.profile.sectionSubtitle": {
    zh: "更新账户展示信息和默认身份标识。",
    en: "Update your account details and default identity.",
  },
  "settings.profile.emailLabel": {
    zh: "邮箱",
    en: "Email",
  },
  "settings.profile.nameLabel": {
    zh: "姓名",
    en: "Name",
  },
  "settings.profile.namePlaceholder": {
    zh: "输入你的显示名称",
    en: "Enter your display name",
  },
  "settings.profile.saveAction": {
    zh: "保存资料",
    en: "Save profile",
  },
  "settings.billing.sectionTitle": {
    zh: "账单与计划",
    en: "Billing and plan",
  },
  "settings.billing.sectionSubtitle": {
    zh: "查看当前订阅、用量和账单入口。",
    en: "Review your current subscription, usage, and billing entry points.",
  },
  "settings.billing.currentPlanLabel": {
    zh: "当前计划",
    en: "Current plan",
  },
  "settings.billing.managePlanAction": {
    zh: "管理计划",
    en: "Manage plan",
  },
  "settings.billing.portalAction": {
    zh: "打开账单门户",
    en: "Open billing portal",
  },
  "settings.appearance.sectionTitle": {
    zh: "外观",
    en: "Appearance",
  },
  "settings.appearance.sectionSubtitle": {
    zh: "控制工作台和后台页面的明暗观感。",
    en: "Control how the workspace and admin surfaces look.",
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
} satisfies Record<string, UiMessageDescriptor>;
