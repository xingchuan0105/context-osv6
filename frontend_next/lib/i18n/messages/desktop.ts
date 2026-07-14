import type { UiMessageDescriptor } from "./types";

export const desktopMessages = {
  "desktop.downloadWindows": {
    zh: "下载 Windows 版",
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
    zh: "当前为便携版 EXE：下载后直接运行即可（建议固定安装目录）。完整安装包（NSIS）将在后续发版提供。",
    en: "Portable EXE: run after download. A full NSIS installer will ship in a later release.",
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
    zh: "下载 Windows 安装包（或便携 EXE）",
    en: "Download the Windows package",
  },
  "desktop.installStep2": {
    zh: "运行安装程序或直接启动 EXE（需 Windows 10+ 与 WebView2）",
    en: "Run the installer or EXE (Windows 10+ with WebView2)",
  },
  "desktop.installStep3": {
    zh: "购买授权后，在客户端粘贴密钥，或点击「在 AVRag Desktop 中激活」",
    en: "After purchase, paste the license key or use Activate in Desktop",
  },
  "desktop.smartScreenHint": {
    zh: "若 SmartScreen 提示未知应用，选择「仍要运行」（正式签名上线前属正常现象）。",
    en: "If SmartScreen warns about an unknown app, choose Run anyway (until code signing ships).",
  },
  "desktop.needClientHint": {
    zh: "尚未安装客户端？请先下载再激活。",
    en: "Don't have the app yet? Download it before activating.",
  },
  "desktop.backToHub": {
    zh: "返回官网",
    en: "Brand site",
  },
  "desktop.openSaaS": {
    zh: "打开云端应用",
    en: "Open cloud app",
  },
} satisfies Record<string, UiMessageDescriptor>;
