import type { UiMessageDescriptor } from "./types";

export const commandPaletteMessages = {
  "commandPalette.title": {
    zh: "快速跳转",
    en: "Quick jump",
  },
  "commandPalette.placeholder": {
    zh: "搜索会话、工作区、文档与页面…",
    en: "Search sessions, workspaces, docs, and pages…",
  },
  "commandPalette.empty": {
    zh: "没有匹配项",
    en: "No matches",
  },
  "commandPalette.hint": {
    zh: "↑↓ 选择 · Enter 打开 · Esc 关闭",
    en: "↑↓ select · Enter open · Esc close",
  },
  "commandPalette.group.sessions": {
    zh: "会话",
    en: "Sessions",
  },
  "commandPalette.group.workspaces": {
    zh: "工作区",
    en: "Workspaces",
  },
  "commandPalette.group.sources": {
    zh: "文档",
    en: "Documents",
  },
  "commandPalette.group.nav": {
    zh: "导航",
    en: "Navigate",
  },
  "commandPalette.group.billing": {
    zh: "会员与余额",
    en: "Plan & wallet",
  },
  "commandPalette.group.help": {
    zh: "帮助与客户端",
    en: "Help & client",
  },
  "commandPalette.workspaceRecent": {
    zh: "最近 · {title}",
    en: "Recent · {title}",
  },
  "commandPalette.sessionLabel": {
    zh: "会话 · {title}",
    en: "Session · {title}",
  },
  "commandPalette.sessionUntitled": {
    zh: "未命名会话",
    en: "Untitled session",
  },
  "commandPalette.sourceLabel": {
    zh: "文档 · {name}",
    en: "Doc · {name}",
  },
  "commandPalette.sourceLabelWithWs": {
    zh: "文档 · {name}（{workspace}）",
    en: "Doc · {name} ({workspace})",
  },
  "commandPalette.loadingWorkspaces": {
    zh: "正在加载工作区…",
    en: "Loading workspaces…",
  },
  "commandPalette.loadingSearch": {
    zh: "正在搜索…",
    en: "Searching…",
  },
  "commandPalette.item.shareTraffic": {
    zh: "分享访问（汇总）",
    en: "Share traffic (all)",
  },
  "commandPalette.item.billing": {
    zh: "设置 · 账单",
    en: "Settings · billing",
  },
  "commandPalette.item.topup": {
    zh: "充值余额",
    en: "Top up wallet",
  },
} satisfies Record<string, UiMessageDescriptor>;
