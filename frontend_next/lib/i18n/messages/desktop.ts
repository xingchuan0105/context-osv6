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
  "desktop.activateRedirect": {
    zh: "客户端免费，无需激活。正在前往下载页…",
    en: "The client is free — no activation. Redirecting to the download page…",
  },
  "desktop.buyTitle": {
    zh: "客户端免费",
    en: "Client is free",
  },
  "desktop.buySubtitle": {
    zh: "客户端本身免费下载使用。云端分享名额与钱包见定价页。",
    en: "The client is free to download. Cloud share slots and wallet are on Pricing.",
  },
  "desktop.buyFreeBanner": {
    zh: "客户端买断已退役。请免费下载安装包；上云经营请看分享名额定价。",
    en: "Desktop buyout is retired. Download the free installer; cloud share slots are on Pricing.",
  },
  "desktop.buyFreeCta": {
    zh: "免费下载客户端",
    en: "Free client download",
  },
  "desktop.buyPricingCta": {
    zh: "云端定价（分享名额）",
    en: "Cloud pricing (share slots)",
  },
  "desktop.buyLegacyTitle": {
    zh: "历史授权档位（可选）",
    en: "Legacy license tiers (optional)",
  },
  "desktop.drawer.title": {
    zh: "客户端设置",
    en: "Client settings",
  },
  "desktop.drawer.close": {
    zh: "关闭",
    en: "Close",
  },
  "desktop.drawer.railLabel": {
    zh: "客户端设置分区",
    en: "Client settings sections",
  },
  "desktop.drawer.account": {
    zh: "账户",
    en: "Account",
  },
  "desktop.drawer.models": {
    zh: "模型",
    en: "Models",
  },
  "desktop.drawer.data": {
    zh: "数据",
    en: "Data",
  },
  "desktop.drawer.about": {
    zh: "关于",
    en: "About",
  },
  "desktop.drawer.diagnostics": {
    zh: "诊断",
    en: "Diagnostics",
  },
  "desktop.drawer.accountCloud": {
    zh: "云账户",
    en: "Cloud account",
  },
  "desktop.drawer.balance": {
    zh: "余额",
    en: "Balance",
  },
  "desktop.drawer.topup": {
    zh: "充值",
    en: "Top up",
  },
  "desktop.drawer.logout": {
    zh: "退出云登录",
    en: "Sign out of cloud",
  },
  "desktop.drawer.logoutConfirm": {
    zh: "确认退出？本机将停止使用官方模型（走余额）。",
    en: "Sign out? This machine will stop using official models (wallet-metered).",
  },
  "desktop.drawer.logoutConfirmAction": {
    zh: "确认退出",
    en: "Confirm sign-out",
  },
  "desktop.drawer.logoutWorking": {
    zh: "正在退出…",
    en: "Signing out…",
  },
  "desktop.drawer.notLoggedIn": {
    zh: "未登录云账户。登录后默认使用官方模型（走余额），按钱包余额计量扣费。",
    en: "Not signed in. After sign-in, official models are metered against your wallet balance by default.",
  },
  "desktop.drawer.login": {
    zh: "登录",
    en: "Sign in",
  },
  "desktop.drawer.modelSource": {
    zh: "当前来源",
    en: "Current source",
  },
  "desktop.drawer.modelOfficial": {
    zh: "官方模型（走余额）",
    en: "Official models (wallet-metered)",
  },
  "desktop.drawer.modelByok": {
    zh: "自定义 Provider（自备 Key）",
    en: "Custom provider (own key)",
  },
  "desktop.drawer.modelOfficialHint": {
    zh: "模型调用经云端转发，按用量从钱包余额扣费；无需配置自己的 Key。",
    en: "Calls are relayed through the cloud and debit your wallet by usage — no own key needed.",
  },
  "desktop.drawer.modelByokHint": {
    zh: "已配置的自备 Key 优先生效，调用不扣云端钱包余额；未配置时登录云账户即可使用官方模型（走余额）。",
    en: "A configured own key takes priority and does not debit the cloud wallet; without one, sign in to use official models (wallet-metered).",
  },
  "desktop.drawer.modelChat": {
    zh: "对话模型",
    en: "Chat model",
  },
  "desktop.drawer.modelEmbedding": {
    zh: "向量模型",
    en: "Embedding model",
  },
  "desktop.drawer.modelRerank": {
    zh: "重排模型",
    en: "Rerank model",
  },
  "desktop.drawer.modelManage": {
    zh: "管理 Provider（自备 Key）→",
    en: "Manage providers (BYOK) →",
  },
  "desktop.drawer.dataDir": {
    zh: "数据目录",
    en: "Data directory",
  },
  "desktop.drawer.logsDir": {
    zh: "日志目录",
    en: "Logs directory",
  },
  "desktop.drawer.open": {
    zh: "打开",
    en: "Open",
  },
  "desktop.drawer.version": {
    zh: "版本",
    en: "Version",
  },
  "desktop.drawer.clientPage": {
    zh: "客户端页",
    en: "Client page",
  },
  "desktop.drawer.pricingPage": {
    zh: "定价",
    en: "Pricing",
  },
  "desktop.drawer.aboutFree": {
    zh: "客户端免费；云端分享名额与钱包见定价页。",
    en: "The client is free; cloud share slots and wallet are on Pricing.",
  },
  "desktop.drawer.diagnosticsHint": {
    zh: "只读诊断信息，用于排查本机运行问题。",
    en: "Read-only diagnostics for troubleshooting the local runtime.",
  },
  "desktop.drawer.stackStatus": {
    zh: "本机数据栈",
    en: "Local data stack",
  },
  "desktop.drawer.envFile": {
    zh: "运行时 env 文件",
    en: "Runtime env file",
  },
  "desktop.startingClient": {
    zh: "正在启动客户端…",
    en: "Starting client…",
  },
  "desktop.cloudLoginTitle": {
    zh: "登录云账户",
    en: "Sign in to your cloud account",
  },
  "desktop.cloudLoginSubtitle": {
    zh: "登录后默认使用官方模型（走余额），按钱包余额计量扣费。",
    en: "After sign-in, official models are metered against your wallet balance by default.",
  },
  "desktop.cloudLoginEmail": {
    zh: "云账户邮箱",
    en: "Cloud account email",
  },
  "desktop.cloudLoginPassword": {
    zh: "密码",
    en: "Password",
  },
  "desktop.cloudLoginSubmit": {
    zh: "登录并启用官方模型",
    en: "Sign in and enable official models",
  },
  "desktop.cloudLoginSubmitting": {
    zh: "正在登录并准备官方模型…",
    en: "Signing in and preparing official models…",
  },
  "desktop.cloudLoginFailed": {
    zh: "登录失败，请检查网络后重试",
    en: "Sign-in failed; check your network and retry",
  },
  "desktop.cloudLoginNoAccount": {
    zh: "没有账户？",
    en: "No account yet?",
  },
  "desktop.cloudLoginRegister": {
    zh: "去注册 →",
    en: "Sign up →",
  },
  "desktop.cloudLoginByokHint": {
    zh: "想用自己的 Key？登录后可在 设置 → Provider 切换自定义 Provider（自备 Key）。",
    en: "Prefer your own key? After sign-in, switch under Settings → Providers (BYOK).",
  },
  "desktop.cloudLoginChecking": {
    zh: "正在检查云登录状态…",
    en: "Checking cloud sign-in…",
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
