import type { UiMessageDescriptor } from "./types";

export const chatMessages = {
  "chat.newConversation": {
    zh: "新对话",
    en: "New chat",
  },
  "chat.recent": {
    zh: "最近",
    en: "Recent",
  },
  "chat.workspaces": {
    zh: "工作区",
    en: "Workspaces",
  },
  "chat.allWorkspaces": {
    zh: "查看全部工作区",
    en: "View all workspaces",
  },
  "chat.personalContext": {
    zh: "本对话",
    en: "This chat",
  },
  "chat.workspaceContext": {
    zh: "工作区 · {name}",
    en: "Workspace · {name}",
  },
  "chat.untitled": {
    zh: "未命名对话",
    en: "Untitled chat",
  },
  "chat.noRecent": {
    zh: "还没有最近对话。",
    en: "No recent chats yet.",
  },
  "chat.loading": {
    zh: "正在加载对话…",
    en: "Loading chats…",
  },
  "chat.listLoadError": {
    zh: "最近对话加载失败；仍可开始新对话。",
    en: "Recent chats could not be loaded. You can still start a new chat.",
  },
  "chat.retry": {
    zh: "重试",
    en: "Retry",
  },
  "chat.sidebarLabel": {
    zh: "对话与工作区",
    en: "Chats and workspaces",
  },
  "chat.regionLabel": {
    zh: "对话",
    en: "Chat",
  },
  "chat.composerLabel": {
    zh: "对话输入框",
    en: "Chat composer",
  },
  "chat.heroSubtitle": {
    zh: "直接提问；需要最新信息时可开启网络搜索。",
    en: "Ask directly, or turn on web search when you need current information.",
  },
  "chat.loadError": {
    zh: "加载对话记录失败。",
    en: "Failed to load this chat.",
  },
  "chat.attachFile": {
    zh: "添加文件",
    en: "Attach file",
  },
  "chat.fileTrayLabel": {
    zh: "会话文件",
    en: "Session files",
  },
  "chat.fileStatus.uploading": {
    zh: "上传中",
    en: "Uploading",
  },
  "chat.fileStatus.parsing": {
    zh: "解析中",
    en: "Parsing",
  },
  "chat.fileStatus.ready": {
    zh: "就绪",
    en: "Ready",
  },
  "chat.fileStatus.failed": {
    zh: "解析失败",
    en: "Failed",
  },
  "chat.fileRetry": {
    zh: "重试解析",
    en: "Retry parse",
  },
  "chat.fileRemove": {
    zh: "移除",
    en: "Remove",
  },
  "chat.fileBlockedHint": {
    zh: "文件处理完成后才能发送消息；可移除未就绪的文件后继续。",
    en: "Sending is blocked until files finish processing; remove pending files to continue.",
  },
  "chat.citationTombstone": {
    zh: "来源已删除，仅保留引用信息。",
    en: "Source deleted. Citation facts retained only.",
  },
  "chat.citationScope.session": {
    zh: "本会话文件",
    en: "Session file",
  },
  "chat.citationScope.workspace": {
    zh: "工作区资料",
    en: "Workspace material",
  },
  "chat.fileUploadFailed": {
    zh: "文件上传失败，请重试。",
    en: "File upload failed. Please try again.",
  },
} satisfies Record<string, UiMessageDescriptor>;
