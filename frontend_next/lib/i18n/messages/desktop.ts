import type { UiMessageDescriptor } from "./types";

export const desktopMessages = {
  "desktop.productTitle": {
    zh: "本地客户端 · 完全免费",
    en: "Desktop client · free",
  },
  "desktop.productSubtitle": {
    zh: "数据私有、可被 Claude / Codex 等桌面 Agent 以 MCP / CLI 调用。下载即用，无需买断许可。",
    en: "Private data, callable by Claude / Codex and other desktop agents via MCP / CLI. Free to download — no license fee.",
  },
  "desktop.feature1": {
    zh: "数据私有：文档与索引默认留在本机，不上云也能工作",
    en: "Private by default: docs and index stay on this machine",
  },
  "desktop.feature2": {
    zh: "MCP / CLI：本机 API 可被 coding agent 调用（建库、解析、问答）",
    en: "MCP / CLI: local API for coding agents (workspaces, ingest, Q&A)",
  },
  "desktop.feature3": {
    zh: "桌面 Agent 友好：Claude Code、Codex 等可接本机知识库",
    en: "Desktop-agent ready: Claude Code, Codex, and more can use your local KB",
  },
  "desktop.feature4": {
    zh: "完全免费使用客户端；需要上云分享时再升级云端名额",
    en: "Client is free; upgrade cloud share slots only when you publish online",
  },
  "desktop.feature5": {
    zh: "自带 LLM Key（BYOK），本地栈一键启动",
    en: "Bring your own LLM key; one-click local stack",
  },
  "desktop.feature6": {
    zh: "可选与云端工作区互通，分享仍走云端档位规则",
    en: "Optional cloud workspace sync; sharing still follows cloud plan limits",
  },
  "desktop.downloadWindows": {
    zh: "下载 Windows 客户端",
    en: "Download for Windows",
  },
  "desktop.downloadLoading": {
    zh: "正在获取下载信息…",
    en: "Checking download…",
  },
  "desktop.downloadUnavailable": {
    zh: "安装包暂未发布",
    en: "Installer not published yet",
  },
  "desktop.downloadUnavailableHint": {
    zh: "请稍后再试，或联系支持获取安装包。",
    en: "Please try again later or contact support.",
  },
  "desktop.downloadMeta": {
    zh: "v{version} · {size}",
    en: "v{version} · {size}",
  },
  "desktop.versionLabel": {
    zh: "版本",
    en: "Version",
  },
  "desktop.sha256Label": {
    zh: "SHA256",
    en: "SHA256",
  },
  "desktop.portableHint": {
    zh: "当前为便携版 EXE：下载后直接运行即可（建议固定安装目录）。",
    en: "Portable EXE: run after download (keep a stable folder).",
  },
  "desktop.signedHint": {
    zh: "安装包已 Authenticode 签名。若仍见 SmartScreen，选择「仍要运行」（自签/新发布者需积累信誉）。",
    en: "Installer is Authenticode-signed. SmartScreen may still warn for new/self-signed publishers — use Run anyway if needed.",
  },
  "desktop.unsignedHint": {
    zh: "安装包尚未使用商业代码签名证书；SmartScreen 可能提示未知发布者。",
    en: "Installer is not yet signed with a commercial code-signing certificate; SmartScreen may warn.",
  },
  "desktop.buyCta": {
    zh: "云端定价（分享名额）",
    en: "Cloud pricing (share slots)",
  },
  "desktop.learnMore": {
    zh: "Agent 接入说明",
    en: "Agent access guide",
  },
  "desktop.benefitsTitle": {
    zh: "为什么下载客户端？",
    en: "Why download the client?",
  },
  "desktop.ctaTitle": {
    zh: "下载与相关入口",
    en: "Download & related",
  },
  "desktop.installTitle": {
    zh: "安装步骤",
    en: "Install steps",
  },
  "desktop.installStep1": {
    zh: "下载 Windows 客户端安装包",
    en: "Download the Windows client installer",
  },
  "desktop.installStep2": {
    zh: "运行安装程序（需 Windows 10+ 与 WebView2）",
    en: "Run the installer (Windows 10+ with WebView2)",
  },
  "desktop.installStep3": {
    zh: "启动后配置 LLM Key，即可本地索引与问答；需要 MCP 时指向本机 API",
    en: "Add your LLM key, then index and chat locally; point MCP clients at the local API when needed",
  },
  "desktop.smartScreenHint": {
    zh: "若 SmartScreen 提示未知应用，选择「仍要运行」（正式签名信誉积累前属正常现象）。",
    en: "If SmartScreen warns about an unknown app, choose Run anyway (normal until publisher reputation builds).",
  },
  "desktop.needClientHint": {
    zh: "尚未安装客户端？请先下载再激活。",
    en: "Don't have the client yet? Download it before activating.",
  },
  "desktop.backToHub": {
    zh: "返回官网",
    en: "Back to brand site",
  },
  "desktop.openSaaS": {
    zh: "打开云端应用",
    en: "Open cloud app",
  },
  "desktop.activateInClient": {
    zh: "在 Context-OS 中激活",
    en: "Activate in Context-OS",
  },
  "desktop.buyTitle": {
    zh: "Context-OS",
    en: "Context-OS",
  },
  "desktop.buySubtitle": {
    zh: "本地 AI 知识助手",
    en: "Local AI knowledge assistant",
  },
  "desktop.welcomeTitle": {
    zh: "欢迎使用 Context-OS",
    en: "Welcome to Context-OS",
  },
  "desktop.licensesSubtitle": {
    zh: "管理 Context-OS 授权与已激活设备",
    en: "Manage Context-OS licenses and activated devices",
  },
} satisfies Record<string, UiMessageDescriptor>;
