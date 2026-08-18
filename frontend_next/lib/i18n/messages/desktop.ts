import type { UiMessageDescriptor } from "./types";

export const desktopMessages = {
  "desktop.productTitle": {
    zh: "本地客户端 · 完全免费",
    en: "Desktop client · free",
  },
  "desktop.productSubtitle": {
    zh: "数据本地私有，开箱即用官方模型或配置自备 Key，可被 Claude / Codex 等桌面 Agent 以 MCP / CLI 调用。完全免费，无需买断许可。",
    en: "Private by default, official models out of the box or BYOK, callable by Claude / Codex via MCP / CLI. 100% free with no license fee.",
  },
  "desktop.feature1": {
    zh: "数据本地私有：文档与索引默认保存在本机，核心检索无需数据上云",
    en: "Private by default: docs and indexes stay on this machine",
  },
  "desktop.feature2": {
    zh: "开箱即用 + BYOK：内置官方托管模型免配置体验，同时支持自备主流大模型 Key",
    en: "Out of the box + BYOK: official models ready to use, or bring your own LLM keys",
  },
  "desktop.feature3": {
    zh: "桌面 Agent 友好：提供标准 MCP / CLI 接口，无缝接入 Claude Code、Codex 等本地编程助手",
    en: "Desktop-agent ready: standard MCP & CLI for Claude Code, Codex, and local agents",
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
    zh: "启动后登录账号即可直接问答（开箱自带官方模型），亦可按需切换自备 Key 或连接桌面 Agent (MCP / CLI)",
    en: "Sign in to chat immediately with official models, or switch to your own key and connect agents (MCP / CLI)",
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
    zh: "确认退出登录？退出后将暂停官方模型中继服务，您的本地知识库与历史记录将完整保留。",
    en: "Sign out? Official relay models will be paused; your local workspaces and history remain intact.",
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
    zh: "未登录账号。登录后默认使用官方托管模型（按量计费）；亦可随时配置自备 Key。",
    en: "Not signed in. After sign-in, official models are metered by usage; you can also configure your own keys anytime.",
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
    zh: "官方托管模型（按量计费）",
    en: "Official models (metered)",
  },
  "desktop.drawer.modelByok": {
    zh: "自备模型密钥 (BYOK)",
    en: "Bring your own key (BYOK)",
  },
  "desktop.drawer.modelOfficialHint": {
    zh: "通过平台安全中继调用模型，按实际 Token 用量从账户余额扣除，无需自备 Key。",
    en: "Relayed through the cloud and debited by usage — no own key needed.",
  },
  "desktop.drawer.modelByokHint": {
    zh: "优先用您的 Key 写最终回答，不扣平台对话费；翻资料仍走平台更快的模型。未配置时登录即可使用官方托管模型。",
    en: "Your key writes the final answer with no chat wallet debit; retrieval still uses the faster platform model. Otherwise sign in for official models.",
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
    zh: "管理模型服务商 (BYOK) →",
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
    zh: "本地运行环境诊断信息（PostgreSQL 向量库、Redis 缓存与核心引擎状态）。",
    en: "Local runtime diagnostics (PostgreSQL vector DB, Redis cache, and core engine status).",
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
    zh: "欢迎使用 Context-OS",
    en: "Welcome to Context-OS",
  },
  "desktop.cloudLoginSubtitle": {
    zh: "登录账号以启用官方模型开箱即用。文档与索引全程保留在您的本地设备。",
    en: "Sign in to access official models out of the box. Your docs and indexes stay local.",
  },
  "desktop.cloudLoginEmail": {
    zh: "账号邮箱",
    en: "Account Email",
  },
  "desktop.cloudLoginPassword": {
    zh: "密码",
    en: "Password",
  },
  "desktop.cloudLoginForgotPassword": {
    zh: "忘记密码？",
    en: "Forgot password?",
  },
  "desktop.cloudLoginSubmit": {
    zh: "登录并进入工作区",
    en: "Sign in & continue",
  },
  "desktop.cloudLoginSubmitting": {
    zh: "正在登录并初始化…",
    en: "Signing in & initializing…",
  },
  "desktop.cloudLoginFailed": {
    zh: "登录失败，请检查网络或账号密码后重试",
    en: "Sign-in failed; check your network or credentials and retry",
  },
  "desktop.cloudLoginNoAccount": {
    zh: "还没有账号？",
    en: "Don't have an account?",
  },
  "desktop.cloudLoginRegister": {
    zh: "立即注册 →",
    en: "Sign up →",
  },
  "desktop.cloudLoginByokHint": {
    zh: "💡 提示：登录后可随时在「设置 → 模型服务商」切换为您自备的 API Key (BYOK)。",
    en: "💡 Tip: You can switch to Bring-Your-Own-Key (BYOK) anytime under Settings → Providers.",
  },
  "desktop.cloudLoginPrivacyHint": {
    zh: "🔒 数据隐私承诺：您的知识库文档、向量索引与聊天记录仅保存在本机。",
    en: "🔒 Privacy First: Your local docs, vector indexes, and chat history never leave your machine.",
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
