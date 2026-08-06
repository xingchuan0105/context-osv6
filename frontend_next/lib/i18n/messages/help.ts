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
  helpBackHelp: {
    zh: "返回帮助中心",
    en: "Back to help",
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
    zh: "脚本与 coding agent 请用工作区 API 密钥调用 MCP。本机客户端可用 context-os-mcp（stdio）转发到 127.0.0.1:18080；也可用 HTTP POST /api/v1/mcp。配置片段在工作区 API Access「给 Agent 用」。",
    en: "Scripts and coding agents should use a workspace API key with MCP. On the desktop client, context-os-mcp (stdio) forwards to 127.0.0.1:18080; HTTP POST /api/v1/mcp also works. Copy snippets from workspace API Access → For agents.",
  },
  helpApiAccessAutomationStep1: {
    zh: "在产品 UI 创建 Workspace，打开该工作区的 API Access，创建带 index/query 的密钥。",
    en: "Create a workspace in the UI, open API Access on that workspace, and mint a key with index/query.",
  },
  helpApiAccessAutomationStep2: {
    zh: "本机客户端：构建或 stage context-os（含 context-os-mcp），设置 CONTEXT_OS_API_KEY 后运行 context-os status。",
    en: "Desktop: build or stage context-os (includes context-os-mcp), set CONTEXT_OS_API_KEY, then run context-os status.",
  },
  helpApiAccessAutomationStep3: {
    zh: "Agent 用 stdio MCP（command = context-os-mcp）；脚本可用 context-os ingest/ask；工具参数带上 workspace_id。",
    en: "Agents use stdio MCP (command = context-os-mcp); scripts can use context-os ingest/ask; always pass workspace_id.",
  },
  helpApiAccessAutomationStep4: {
    zh: "分享、成员与密钥管理仍只在用户会话 UI 中完成。",
    en: "Share, members, and key management stay on the signed-in user UI only.",
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
    zh: "顶栏现在提供分享、API Access 和新建工作区快捷入口，账户相关设置收进右侧账户菜单。",
    en: "The top bar now provides Share, API Access, and New Workspace shortcuts; account settings live in the account menu on the right.",
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
  helpSectionWriteTitle: {
    zh: "Write 模式",
    en: "Write mode",
  },
  helpItemWrite1: {
    zh: "输入主题后自动调研、生成大纲并分段撰写长文。",
    en: "Enter a topic to auto-research, outline, and draft a long-form article in sections.",
  },
  helpItemWrite2: {
    zh: "支持统计指纹精修，降低 AI 生成痕迹。",
    en: "Includes statistical-fingerprint refinement to reduce detectable AI patterns.",
  },
  helpItemWrite3: {
    zh: "单路调研失败时会自动降级为单路模式。",
    en: "Falls back to single-path research when one research path fails.",
  },
  helpItemWriteDocs: {
    zh: "查看 Write 模式文档",
    en: "Read Write mode documentation",
  },
  helpSubtitle: {
    zh: "按 Wiki 方式整理 Context-OS 的核心工作流、API 能力边界，以及出现问题时的优先排查路径。",
    en: "A wiki-style guide to Context-OS core workflows, API boundaries, and the first troubleshooting path to check.",
  },
  helpTitle: {
    zh: "帮助中心",
    en: "Help center",
  },
  helpSectionDesktopTitle: {
    zh: "7. 客户端",
    en: "7. Client",
  },
  helpItemDesktop1: {
    zh: "Windows 客户端完全免费下载使用；数据可留在本机，支持 MCP / CLI 供桌面 Agent 调用。",
    en: "The Windows client is free. Data can stay local; MCP / CLI work with desktop agents.",
  },
  helpItemDesktop2: {
    zh: "安装后配置 LLM Key 即可本地索引与问答；SmartScreen 提示时选择仍要运行。需要上云分享时再升级云端名额。",
    en: "Add an LLM key after install for local index and Q&A; use Run anyway if SmartScreen warns. Upgrade cloud share slots only when publishing online.",
  },
  helpItemDesktopDownload: {
    zh: "下载 / 了解客户端（免费）",
    en: "Download / learn about the client (free)",
  },
  helpItemDesktopBuy: {
    zh: "历史授权记录（非主路径）",
    en: "Legacy license records (not primary)",
  },
  helpWritePageSubtitle: {
    zh: "根据主题自动撰写长文，内置调研、大纲、分段写作与统计指纹精修。",
    en: "Automatically writes long-form articles from a topic, with built-in research, outlining, sectioned drafting, and statistical-fingerprint refinement.",
  },
  helpWriteUsageTitle: {
    zh: "用量预期",
    en: "Usage expectations",
  },
  helpWriteMetricColumn: {
    zh: "指标",
    en: "Metric",
  },
  helpWriteRangeColumn: {
    zh: "典型范围",
    en: "Typical range",
  },
  helpWriteLlmCalls: {
    zh: "LLM 调用",
    en: "LLM calls",
  },
  helpWritePerArticle: {
    zh: "篇",
    en: "article",
  },
  helpWriteTokenFull: {
    zh: "Token（全文）",
    en: "Token (full)",
  },
  helpWriteWallClock: {
    zh: "用时",
    en: "Time taken",
  },
  helpWriteDegradeTitle: {
    zh: "降级说明",
    en: "Degradation",
  },
  helpWriteDegradeBody: {
    zh: "当质量校验未全部通过时，文章仍会交付（软结束），并附带校验警告。单路调研失败时自动降级为单路。",
    en: "When quality checks are not fully satisfied, the article is still delivered (soft exit) with a validation warning. If one research path fails, it falls back to single-path.",
  },
} satisfies Record<string, UiMessageDescriptor>;
