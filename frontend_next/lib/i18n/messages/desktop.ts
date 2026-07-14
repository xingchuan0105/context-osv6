import type { UiMessageDescriptor } from "./types";

export const desktopMessages = {
  "desktop.productTitle": {
    zh: "Context-OS 客户端",
    en: "Context-OS Client",
  },
  "desktop.productSubtitle": {
    zh: "本地 AI 知识助手。自带 LLM API Key，离线优先，数据留在本机。",
    en: "Local AI knowledge assistant. Bring your own LLM key; data stays on this machine.",
  },
  "desktop.feature1": {
    zh: "16+ LLM 服务商预设，含智谱 Coding Plan 一键配置",
    en: "16+ LLM provider presets, including Zhipu Coding Plan one-click setup",
  },
  "desktop.feature2": {
    zh: "本地文档索引与 RAG 检索，支持 PDF / Markdown",
    en: "Local document index and RAG for PDF / Markdown",
  },
  "desktop.feature3": {
    zh: "买断制授权，v1.x 终身免费升级",
    en: "One-time license with free upgrades through v1.x",
  },
  "desktop.feature4": {
    zh: "与云端工作区数据互通（可选同步）",
    en: "Optional sync with cloud workspaces",
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
    zh: "购买授权",
    en: "Buy license",
  },
  "desktop.learnMore": {
    zh: "了解更多",
    en: "Learn more",
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
    zh: "购买授权后，在客户端粘贴密钥，或点击「在 Context-OS 客户端中激活」",
    en: "After purchase, paste the license key in the client or open Activate in Context-OS Client",
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
    en: "Brand site",
  },
  "desktop.openSaaS": {
    zh: "打开云端应用",
    en: "Open cloud app",
  },
  "desktop.activateInClient": {
    zh: "在 Context-OS 客户端中激活",
    en: "Activate in Context-OS Client",
  },
  "desktop.buyTitle": {
    zh: "Context-OS 客户端",
    en: "Context-OS Client",
  },
  "desktop.buySubtitle": {
    zh: "本地 AI 知识助手",
    en: "Local AI knowledge assistant",
  },
  "desktop.welcomeTitle": {
    zh: "欢迎使用 Context-OS 客户端",
    en: "Welcome to Context-OS Client",
  },
  "desktop.licensesSubtitle": {
    zh: "管理 Context-OS 客户端授权与已激活设备",
    en: "Manage Context-OS Client licenses and activated devices",
  },
} satisfies Record<string, UiMessageDescriptor>;
