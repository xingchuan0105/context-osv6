import type { UiMessageDescriptor } from "./types";

export const helpMessages = {
  helpAccountSettings: {
    zh: "账户设置",
    en: "Account settings",
  },
  helpBackDashboard: {
    zh: "返回 Dashboard",
    en: "Back to dashboard",
  },
  helpItemAccount1: {
    zh: "支持注册、登录、重置密码与退出登录。",
    en: "Registration, sign-in, password reset, and sign-out are all supported.",
  },
  helpItemAccount2: {
    zh: "登录态失效时，受保护路由会自动回到登录页。",
    en: "Protected routes return to sign-in automatically when the session expires.",
  },
  helpItemAccount3: {
    zh: "如果遇到 401 或 403，先确认当前 token 是否仍然有效。",
    en: "If you hit a 401 or 403, confirm the current token is still valid first.",
  },
  helpItemApi1: {
    zh: "每个工作区可以单独创建和撤销 API 密钥。",
    en: "Each workspace can create and revoke its own API keys.",
  },
  helpItemApi2: {
    zh: "当前 API Access 页面提供权限、速率限制和一次性明文 key 展示。",
    en: "API Access shows scopes, rate limits, and one-time plaintext key reveal.",
  },
  helpItemApi3: {
    zh: "工作区 API 密钥只用于该工作区的资料上传、URL 导入和 RAG 查询；先在 UI 里创建工作区，再在此页创建密钥。",
    en: "Workspace API keys only cover uploads, URL imports, and RAG for that workspace. Create the workspace in the UI first, then mint a key here.",
  },
  helpApiAccessTitle: {
    zh: "API 访问",
    en: "API Access",
  },
  helpApiAccessSubtitle: {
    zh: "面向个人用户的 API 接入说明。每个工作区单独管理密钥；自动化代理请使用 agent 文档。",
    en: "API access for personal users. Each workspace has its own keys; automated agents should use the agent docs.",
  },
  helpApiAccessOverviewTitle: {
    zh: "你会在这里找到",
    en: "What this page covers",
  },
  helpApiAccessAutomationTitle: {
    zh: "需要自动化时",
    en: "For automation",
  },
  helpApiAccessAutomationBody: {
    zh: "如果脚本、代理或批处理系统需要调用 API，请切换到 agent 文档，那里包含面向机器的接入说明。",
    en: "If a script, agent, or batch system needs to call the API, switch to the agent docs for machine-oriented setup.",
  },
  helpApiAccessBackHelp: {
    zh: "返回帮助中心",
    en: "Back to help",
  },
  helpItemApiHumanDocs: {
    zh: "打开人类开发者 API 文档",
    en: "Open the human developer API docs",
  },
  helpItemApiAgentDocs: {
    zh: "打开 Agent API 文档",
    en: "Open the agent API docs",
  },
  helpItemCollab1: {
    zh: "Share Center 支持访问级别、分享链接、成员邀请和访问日志。",
    en: "Share Center covers access levels, share links, member invites, and access logs.",
  },
  helpItemCollab2: {
    zh: "公开分享链接会落到只读的 shared workspace 页面。",
    en: "Public share links open a read-only shared workspace page.",
  },
  helpItemCollab3: {
    zh: "邀请页支持未登录用户先登录或注册，再继续接受邀请。",
    en: "Invite flows let signed-out users sign in or register before accepting access.",
  },
  helpItemDocs1: {
    zh: "支持上传文件和添加 URL 资料源。",
    en: "You can upload files and add URL-based sources.",
  },
  helpItemDocs2: {
    zh: "会话可以按资料勾选形成 doc scope，直接影响 RAG 检索上下文。",
    en: "Sessions can scope retrieval to selected documents, directly affecting RAG context.",
  },
  helpItemDocs3: {
    zh: "资料状态异常时可以执行重建索引，并在右侧面板观察状态变化。",
    en: "If source state looks wrong, reindex it and watch status changes in the right rail.",
  },
  helpItemTroubleshooting1: {
    zh: "分享页没有数据时，先确认该工作区是否已经启用分享链接。",
    en: "If share pages are empty, confirm the workspace has sharing enabled first.",
  },
  helpItemTroubleshooting2: {
    zh: "API 调用失败时，先确认密钥仍处于生效状态、没有过期，且命中了正确的 workspace 路径。",
    en: "If API calls fail, check the key is still active, unexpired, and targeting the correct workspace path.",
  },
  helpItemTroubleshooting3: {
    zh: "界面文案或状态不一致时，优先检查当前路由是否仍停留在兼容跳转链路上。",
    en: "If UI copy or state looks inconsistent, verify you are not still on a compatibility redirect path.",
  },
  helpItemWorkspace1: {
    zh: "每个工作区包含左侧历史、中间对话区，以及右侧资料和笔记面板。",
    en: "Each workspace contains history on the left, chat in the middle, and sources plus notes on the right.",
  },
  helpItemWorkspace2: {
    zh: "历史列表支持关键词过滤；点击已有线程会恢复对应会话消息。",
    en: "The history list supports filtering, and opening a thread restores its messages.",
  },
  helpItemWorkspace3: {
    zh: "顶栏现在提供合并后的传播入口、API Access 和新建工作区快捷入口。",
    en: "The top bar now provides the merged propagation entry, API Access, and New Workspace shortcuts.",
  },
  helpSectionAccountTitle: {
    zh: "1. 账户与认证",
    en: "1. Accounts & authentication",
  },
  helpSectionApiTitle: {
    zh: "5. API 接入",
    en: "5. API access",
  },
  helpSectionCollabTitle: {
    zh: "4. 分享与协作",
    en: "4. Sharing & collaboration",
  },
  helpSectionDocsTitle: {
    zh: "3. 资料管理与 Doc Scope",
    en: "3. Source management & doc scope",
  },
  helpSectionTroubleshootingTitle: {
    zh: "6. 常见排查",
    en: "6. Common troubleshooting",
  },
  helpSectionWorkspaceTitle: {
    zh: "2. 工作区与会话",
    en: "2. Workspaces & sessions",
  },
  helpSubtitle: {
    zh: "按 Wiki 方式整理 Context-OS 的核心工作流、API 能力边界，以及出现问题时的优先排查路径。",
    en: "A wiki-style guide to Context-OS core workflows, API boundaries, and the first troubleshooting path to check.",
  },
  helpTitle: {
    zh: "帮助中心",
    en: "Help center",
  },
} satisfies Record<string, UiMessageDescriptor>;
