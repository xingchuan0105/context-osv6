import type { UiLocale } from "./config";

type UiMessageDescriptor = {
  zh: string;
  en: string;
};

export const UI_MESSAGES = {
  commonCancel: {
    zh: "取消",
    en: "Cancel",
  },
  commonSave: {
    zh: "保存",
    en: "Save",
  },
  adminNavAuditLogs: {
    zh: "审计日志",
    en: "Audit Logs",
  },
  adminNavBilling: {
    zh: "账单",
    en: "Billing",
  },
  adminNavDegradation: {
    zh: "降级",
    en: "Degradation",
  },
  adminNavFeatureFlags: {
    zh: "功能开关",
    en: "Feature Flags",
  },
  adminNavHealth: {
    zh: "健康",
    en: "Health",
  },
  adminNavLabel: {
    zh: "后台导航",
    en: "Admin navigation",
  },
  adminNavOrganizations: {
    zh: "组织",
    en: "Organizations",
  },
  adminNavRagHealth: {
    zh: "RAG 健康",
    en: "RAG Health",
  },
  adminNavUsage: {
    zh: "用量",
    en: "Usage",
  },
  adminNavUsers: {
    zh: "用户",
    en: "Users",
  },
  adminNavWorkers: {
    zh: "执行器",
    en: "Workers",
  },
  adminShellTitle: {
    zh: "后台管理",
    en: "Admin",
  },
  authConfirmPasswordLabel: {
    zh: "确认密码",
    en: "Confirm password",
  },
  authContinueToLogin: {
    zh: "去登录",
    en: "Go to sign in",
  },
  authCreateAccount: {
    zh: "创建账号",
    en: "Create account",
  },
  authCreatingAccount: {
    zh: "创建中...",
    en: "Creating...",
  },
  authEmailAndCodeRequired: {
    zh: "请输入邮箱和验证码。",
    en: "Enter your email and verification code.",
  },
  authEmailAndPasswordRequired: {
    zh: "请输入邮箱和密码。",
    en: "Enter your email and password.",
  },
  authEmailLabel: {
    zh: "邮箱",
    en: "Email",
  },
  authForgotPassword: {
    zh: "忘记密码",
    en: "Forgot password",
  },
  authHasAccount: {
    zh: "已有账号？",
    en: "Already have an account?",
  },
  authLoginFailed: {
    zh: "登录失败，请稍后再试。",
    en: "Sign-in failed. Try again later.",
  },
  authLoginSubmit: {
    zh: "继续登录",
    en: "Continue",
  },
  authLoginSubmitting: {
    zh: "登录中...",
    en: "Signing in...",
  },
  authLoginSubtitle: {
    zh: "登录以继续进入您的工作区",
    en: "Sign in to continue into your workspace",
  },
  authLoginTitle: {
    zh: "欢迎回来",
    en: "Welcome back",
  },
  authNameLabel: {
    zh: "姓名",
    en: "Name",
  },
  authNeedsAccount: {
    zh: "还没有账号？",
    en: "Need an account?",
  },
  authNewPasswordLabel: {
    zh: "新密码",
    en: "New password",
  },
  authNewPasswordRequired: {
    zh: "请输入新密码。",
    en: "Enter a new password.",
  },
  authOptional: {
    zh: "可选",
    en: "Optional",
  },
  authPasswordLabel: {
    zh: "密码",
    en: "Password",
  },
  authPasswordMinLengthHint: {
    zh: "至少 8 位",
    en: "At least 8 characters",
  },
  authPasswordMinLengthRequired: {
    zh: "密码至少需要 8 位。",
    en: "Password must be at least 8 characters.",
  },
  authPasswordMismatch: {
    zh: "两次输入的密码不一致。",
    en: "Passwords do not match.",
  },
  authRegisterFailed: {
    zh: "创建账号失败，请稍后再试。",
    en: "Account creation failed. Try again later.",
  },
  authRegisterSubtitle: {
    zh: "使用邮箱创建账号，开始使用 Context OS。",
    en: "Create your account with email to get started with Context OS.",
  },
  authResetBackToLogin: {
    zh: "返回登录",
    en: "Back to sign in",
  },
  authResetBackToPrevious: {
    zh: "返回上一步",
    en: "Back",
  },
  authResetBackToStart: {
    zh: "返回找回密码",
    en: "Back to reset",
  },
  authResetCodeHint: {
    zh: "6 位验证码",
    en: "6-digit code",
  },
  authResetCodeLabel: {
    zh: "验证码",
    en: "Verification code",
  },
  authResetConfirmFailed: {
    zh: "重置密码失败，请稍后再试。",
    en: "Password reset failed. Try again later.",
  },
  authResetConfirmSubmit: {
    zh: "完成重置",
    en: "Finish reset",
  },
  authResetConfirmSubmitting: {
    zh: "提交中...",
    en: "Submitting...",
  },
  authResetConfirmSubtitle: {
    zh: "输入新密码后即可完成重置。",
    en: "Set a new password to finish the reset flow.",
  },
  authResetConfirmTitle: {
    zh: "设置新密码",
    en: "Set a new password",
  },
  authResetConfirmUnavailable: {
    zh: "请先完成验证码验证。",
    en: "Complete code verification first.",
  },
  authResetEmailRequired: {
    zh: "请输入邮箱。",
    en: "Enter your email.",
  },
  authResetRequestSubtitle: {
    zh: "输入邮箱后，我们会发送验证码到你的邮箱。",
    en: "Enter your email and we'll send a verification code.",
  },
  authResetRequestTitle: {
    zh: "找回密码",
    en: "Reset password",
  },
  authResetSendFailed: {
    zh: "发送验证码失败，请稍后再试。",
    en: "Failed to send the verification code. Try again later.",
  },
  authResetSendSubmit: {
    zh: "发送验证码",
    en: "Send code",
  },
  authResetSendSubmitting: {
    zh: "发送中...",
    en: "Sending...",
  },
  authResetUnavailable: {
    zh: "密码重置暂不可用。",
    en: "Password reset is currently unavailable.",
  },
  authResetVerifyFailed: {
    zh: "验证验证码失败，请稍后再试。",
    en: "Verification failed. Try again later.",
  },
  authResetVerifySubmit: {
    zh: "继续",
    en: "Continue",
  },
  authResetVerifySubmitting: {
    zh: "验证中...",
    en: "Verifying...",
  },
  authResetVerifySubtitle: {
    zh: "输入邮箱和验证码，继续到设置新密码。",
    en: "Enter your email and verification code to continue.",
  },
  authResetVerifyTitle: {
    zh: "验证验证码",
    en: "Verify code",
  },
  authSignIn: {
    zh: "去登录",
    en: "Sign in",
  },
  authSignUp: {
    zh: "去注册",
    en: "Sign up",
  },
  authVerificationRequired: {
    zh: "请先完成验证码验证。",
    en: "Complete verification first.",
  },
  gateCheckingSession: {
    zh: "正在检查登录状态...",
    en: "Checking your session...",
  },
  gateInitializingAuth: {
    zh: "正在初始化认证状态...",
    en: "Initializing authentication...",
  },
  gateRedirectingDashboard: {
    zh: "正在跳转到工作台...",
    en: "Redirecting to the dashboard...",
  },
  gateRedirectingLogin: {
    zh: "正在跳转到登录页...",
    en: "Redirecting to sign in...",
  },
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
    zh: "工作区 API 主要面向资料上传、URL 导入和 RAG 查询，不开放聊天与全局搜索代理。",
    en: "Workspace APIs focus on uploads, URL imports, and RAG queries rather than chat or global search proxies.",
  },
  helpApiAccessTitle: {
    zh: "API 访问",
    en: "API Access",
  },
  helpApiAccessSubtitle: {
    zh: "面向手动接入和调试 API 的开发者。自动化代理请使用单独的 agent 文档。",
    en: "For developers wiring up and debugging the API by hand. Automated agents should use the separate agent docs.",
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
  homeEntering: {
    zh: "正在进入 Context OS...",
    en: "Entering Context OS...",
  },
  authErrorAccountNotRegistered: {
    zh: "此账号还未注册，请先注册。",
    en: "This account is not registered yet. Sign up first.",
  },
  authErrorEmailExists: {
    zh: "该邮箱已注册，请直接登录。",
    en: "This email is already registered. Sign in instead.",
  },
  authErrorInvalidCredentials: {
    zh: "邮箱或密码错误。",
    en: "Incorrect email or password.",
  },
  authErrorInvalidPassword: {
    zh: "密码错误。",
    en: "Incorrect password.",
  },
  authErrorInvalidResetTicket: {
    zh: "重置会话无效或已过期。",
    en: "The reset session is invalid or has expired.",
  },
  authErrorPasswordResetUnavailable: {
    zh: "当前环境未启用密码找回，请联系管理员。",
    en: "Password reset is unavailable in this environment. Contact an administrator.",
  },
  authErrorServiceUnavailable: {
    zh: "服务暂时不可用，请稍后再试。",
    en: "The service is temporarily unavailable. Try again later.",
  },
  dashboardActionDelete: {
    zh: "删除",
    en: "Delete",
  },
  dashboardActionFavorite: {
    zh: "收藏",
    en: "Favorite",
  },
  dashboardActionRename: {
    zh: "重命名",
    en: "Rename",
  },
  dashboardActionUnfavorite: {
    zh: "取消收藏",
    en: "Unfavorite",
  },
  dashboardBrandSubtitle: {
    zh: "工作区控制台",
    en: "Workspace Dashboard",
  },
  dashboardCardFavoriteBadge: {
    zh: "已收藏",
    en: "Favorited",
  },
  dashboardCardMemberBadge: {
    zh: "成员工作区",
    en: "Member workspace",
  },
  dashboardCardMineBadge: {
    zh: "我的工作区",
    en: "My workspace",
  },
  dashboardCloseSearch: {
    zh: "关闭搜索",
    en: "Close search",
  },
  dashboardConfirmDelete: {
    zh: "删除 {title}?",
    en: "Delete {title}?",
  },
  dashboardCreateAction: {
    zh: "创建工作区",
    en: "Create workspace",
  },
  dashboardCreateDialogLabel: {
    zh: "新建工作区",
    en: "Create workspace",
  },
  dashboardCreateFirst: {
    zh: "创建第一个工作区",
    en: "Create the first workspace",
  },
  dashboardCreateLoading: {
    zh: "创建中...",
    en: "Creating...",
  },
  dashboardCreateTitle: {
    zh: "新建工作区",
    en: "Create workspace",
  },
  dashboardEmptyAllTitle: {
    zh: "还没有工作区",
    en: "No workspaces yet",
  },
  dashboardEmptyBody: {
    zh: "先创建一个工作区，然后再进入工作区。",
    en: "Create a workspace first, then jump into the workspace shell.",
  },
  dashboardEmptyDescription: {
    zh: "暂无描述",
    en: "No description",
  },
  dashboardEmptyFavoritesTitle: {
    zh: "还没有收藏的工作区",
    en: "No favorited workspaces yet",
  },
  dashboardGenerateNameError: {
    zh: "无法生成工作区名称",
    en: "Unable to generate a workspace name.",
  },
  dashboardHeadingAll: {
    zh: "全部工作区",
    en: "All workspaces",
  },
  dashboardHeadingCount: {
    zh: "{count} 个工作区",
    en: "{count} workspaces",
  },
  dashboardHeadingFavorites: {
    zh: "我的收藏",
    en: "Favorites",
  },
  dashboardHeadingMine: {
    zh: "我的工作区",
    en: "My workspaces",
  },
  dashboardListLabel: {
    zh: "工作区列表",
    en: "Workspace list",
  },
  dashboardLoadError: {
    zh: "加载工作区失败",
    en: "Failed to load workspaces.",
  },
  dashboardLoading: {
    zh: "正在加载工作区...",
    en: "Loading workspaces...",
  },
  dashboardLoginRequired: {
    zh: "请先登录",
    en: "Please sign in first.",
  },
  dashboardPromptRename: {
    zh: "重命名 {title}",
    en: "Rename {title}",
  },
  dashboardRoleMember: {
    zh: "成员",
    en: "Member",
  },
  dashboardRoleOwner: {
    zh: "所有者",
    en: "Owner",
  },
  dashboardSearchDialogLabel: {
    zh: "快速打开工作区",
    en: "Quick open workspace",
  },
  dashboardCreatedAtColumn: {
    zh: "创建时间",
    en: "Created",
  },
  dashboardRoleColumn: {
    zh: "角色",
    en: "Role",
  },
  dashboardSettingsLink: {
    zh: "设置",
    en: "Settings",
  },
  dashboardProfileLink: {
    zh: "账号信息",
    en: "Account",
  },
  dashboardSearchEmptyIdle: {
    zh: "输入关键词搜索工作区",
    en: "Type to search workspaces",
  },
  dashboardSearchEmptyNoMatch: {
    zh: "没有匹配的工作区",
    en: "No matching workspaces",
  },
  dashboardSearchLabel: {
    zh: "搜索工作区",
    en: "Search workspaces",
  },
  dashboardSearchPlaceholder: {
    zh: "搜索工作区标题或描述",
    en: "Search workspace titles or descriptions",
  },
  dashboardSearchResultsLabel: {
    zh: "工作区搜索结果",
    en: "Workspace search results",
  },
  dashboardSearchSubtitle: {
    zh: "输入关键词，点击结果进入工作区",
    en: "Type a keyword and jump directly into the workspace.",
  },
  dashboardSearchTitle: {
    zh: "快速打开工作区",
    en: "Quick open workspace",
  },
  dashboardSortRecent: {
    zh: "创建时间",
    en: "Created",
  },
  dashboardSortTitle: {
    zh: "标题",
    en: "Title",
  },
  dashboardStatusFailed: {
    zh: "异常",
    en: "failed",
  },
  dashboardStatusProcessing: {
    zh: "处理中",
    en: "processing",
  },
  dashboardStatusReady: {
    zh: "就绪",
    en: "ready",
  },
  dashboardTabAll: {
    zh: "全部",
    en: "All",
  },
  dashboardTabFavorites: {
    zh: "我的收藏",
    en: "Favorites",
  },
  dashboardTabMine: {
    zh: "我的工作区",
    en: "My workspaces",
  },
  dashboardTabsLabel: {
    zh: "工作区标签",
    en: "Workspace tabs",
  },
  dashboardToolbarSearch: {
    zh: "搜索工作区",
    en: "Search workspaces",
  },
  dashboardViewCard: {
    zh: "卡片",
    en: "Cards",
  },
  dashboardViewGridLabel: {
    zh: "工作区卡片",
    en: "Workspace cards",
  },
  dashboardViewList: {
    zh: "列表",
    en: "List",
  },
  dashboardViewModeLabel: {
    zh: "工作区视图模式",
    en: "Workspace view mode",
  },
  dashboardWorkspaceNameField: {
    zh: "名称",
    en: "Name",
  },
  dashboardWorkspaceDescriptionField: {
    zh: "描述",
    en: "Description",
  },
  dashboardNewWorkspace: {
    zh: "新建工作区",
    en: "New workspace",
  },
  dashboardUntitledWorkspace: {
    zh: "未命名工作区",
    en: "Untitled workspace",
  },
  workspaceAnalyze: {
    zh: "分析",
    en: "Analyze",
  },
  workspaceDistribute: {
    zh: "传播",
    en: "Distribute",
  },
  workspaceAnonymousUser: {
    zh: "未登录",
    en: "Not signed in",
  },
  workspaceApi: {
    zh: "API",
    en: "API",
  },
  workspaceChatComposerHint: {
    zh: "Enter 发送，Shift+Enter 换行。",
    en: "Press Enter to send and Shift+Enter for a newline.",
  },
  workspaceChatActionAddToNote: {
    zh: "加入笔记",
    en: "Add to note",
  },
  workspaceChatActionCopy: {
    zh: "复制",
    en: "Copy",
  },
  workspaceChatActionEdit: {
    zh: "编辑",
    en: "Edit",
  },
  workspaceChatActionRegenerate: {
    zh: "重新生成",
    en: "Regenerate",
  },
  workspaceChatClearMode: {
    zh: "清除对话模式",
    en: "Clear chat mode",
  },
  workspaceChatComposerLabel: {
    zh: "工作区对话输入框",
    en: "Workspace chat composer",
  },
  workspaceChatComposerPlaceholder: {
    zh: "输入 / 选择模式，然后开始提问...",
    en: "Type / to choose a mode, then start typing...",
  },
  workspaceCitationsLabel: {
    zh: "引用",
    en: "Citations",
  },
  workspaceChatEyebrow: {
    zh: "工作区对话",
    en: "Workspace chat",
  },
  workspaceChatLoadError: {
    zh: "加载工作区对话记录失败。",
    en: "Failed to load workspace transcript.",
  },
  workspaceChatModeChat: {
    zh: "聊天模式",
    en: "chat",
  },
  workspaceChatModeLabel: {
    zh: "对话模式",
    en: "Chat mode",
  },
  workspaceChatModeRag: {
    zh: "知识库检索",
    en: "RAG",
  },
  workspaceChatModeSearch: {
    zh: "网络搜索",
    en: "web_search",
  },
  workspaceChatRegionLabel: {
    zh: "工作区对话",
    en: "Workspace chat",
  },
  workspaceChatSubtitle: {
    zh: "和当前工作区对话",
    en: "Chat with the workspace",
  },
  workspaceCreateAction: {
    zh: "创建工作区",
    en: "Create workspace",
  },
  workspaceCreateDialogLabel: {
    zh: "新建工作区",
    en: "Create workspace",
  },
  workspaceDegradeReasons: {
    zh: "降级原因：{reasons}",
    en: "Degrade reasons: {reasons}",
  },
  workspaceDescriptionField: {
    zh: "描述",
    en: "Description",
  },
  workspaceGuardIntervened: {
    zh: "Guardrail 已介入当前回答。",
    en: "Guardrails intervened in this answer.",
  },
  workspaceHistoryLabel: {
    zh: "工作区历史",
    en: "Workspace history",
  },
  workspaceHistorySearch: {
    zh: "搜索会话",
    en: "Search sessions",
  },
  workspaceSearchDialogLabel: {
    zh: "搜索会话",
    en: "Search sessions",
  },
  workspaceSearchTitle: {
    zh: "搜索会话",
    en: "Search sessions",
  },
  workspaceSearchSubtitle: {
    zh: "按关键词搜索会话标题、摘要和聊天正文。",
    en: "Search session titles, summaries, and chat content by keyword.",
  },
  workspaceSearchPlaceholder: {
    zh: "输入关键词搜索聊天正文",
    en: "Search chat content by keyword",
  },
  workspaceSearchEmptyIdle: {
    zh: "输入关键词后即可搜索会话和聊天正文。",
    en: "Type a keyword to search sessions and chat content.",
  },
  workspaceSearchEmptyNoMatch: {
    zh: "没有找到匹配的会话。",
    en: "No matching sessions found.",
  },
  workspaceSearchResultsLabel: {
    zh: "会话搜索结果",
    en: "Session search results",
  },
  workspaceSearchLoading: {
    zh: "正在加载会话内容…",
    en: "Loading session content...",
  },
  workspaceSearchLoadError: {
    zh: "部分会话内容加载失败，搜索结果可能不完整。",
    en: "Some session content could not be loaded, so results may be incomplete.",
  },
  workspaceHistorySubtitle: {
    zh: "查看会话、筛选结果和线程操作",
    en: "Sessions, filters, and thread actions",
  },
  workspaceHistoryTitle: {
    zh: "历史",
    en: "History",
  },
  workspaceLanguageChinese: {
    zh: "中文",
    en: "Chinese",
  },
  workspaceLanguageEnglish: {
    zh: "English",
    en: "English",
  },
  workspaceMenuLanguage: {
    zh: "语言",
    en: "Language",
  },
  workspaceMenuTheme: {
    zh: "主题",
    en: "Theme",
  },
  workspaceNameField: {
    zh: "工作区名称",
    en: "Workspace name",
  },
  workspaceNewThread: {
    zh: "新建会话",
    en: "New session",
  },
  workspaceNewWorkspace: {
    zh: "新建工作区",
    en: "New workspace",
  },
  workspaceNoMessages: {
    zh: "还没有消息。",
    en: "No messages yet.",
  },
  workspaceEmptyStateBody: {
    zh: "开始一个新对话，或围绕当前工作区资料提出问题。",
    en: "Start a new conversation or ask a question about this workspace.",
  },
  workspaceNoSessionsMatch: {
    zh: "暂无会话。",
    en: "No sessions yet.",
  },
  workspacePromptRenameSession: {
    zh: "重命名会话",
    en: "Rename session",
  },
  workspaceRecentSession: {
    zh: "最近",
    en: "Recent",
  },
  workspaceRightRailLabel: {
    zh: "工作区右侧栏",
    en: "Workspace right rail",
  },
  workspaceRenameSessionAction: {
    zh: "重命名",
    en: "Rename",
  },
  workspaceDeleteSessionAction: {
    zh: "删除",
    en: "Delete",
  },
  workspacePinSessionAction: {
    zh: "置顶",
    en: "Pin",
  },
  workspaceUnpinSessionAction: {
    zh: "取消置顶",
    en: "Unpin",
  },
  workspacePinnedSession: {
    zh: "已置顶",
    en: "Pinned",
  },
  workspaceLogout: {
    zh: "退出登录",
    en: "Log out",
  },
  workspaceOpenAccountMenu: {
    zh: "打开账号菜单",
    en: "Open account menu",
  },
  workspaceOpenSettingsMenu: {
    zh: "打开设置菜单",
    en: "Open settings menu",
  },
  workspaceSaveTitle: {
    zh: "保存标题",
    en: "Save workspace title",
  },
  workspaceSending: {
    zh: "发送中...",
    en: "Sending...",
  },
  workspaceSend: {
    zh: "发送",
    en: "Send",
  },
  workspaceSessionTitleField: {
    zh: "工作区标题",
    en: "Workspace title",
  },
  workspaceShare: {
    zh: "分享",
    en: "Share",
  },
  workspaceThemeDark: {
    zh: "深色",
    en: "Dark",
  },
  workspaceThemeLight: {
    zh: "浅色",
    en: "Light",
  },
  workspaceThemeSystem: {
    zh: "跟随系统",
    en: "System",
  },
  workspaceThreadTitleField: {
    zh: "会话标题",
    en: "Session title",
  },
  workspaceTitleEyebrow: {
    zh: "工作区",
    en: "Workspace",
  },
  workspaceToggleHistoryRail: {
    zh: "切换历史栏",
    en: "Toggle history rail",
  },
  workspaceToggleRightRail: {
    zh: "切换右侧栏",
    en: "Toggle right rail",
  },
  workspaceTranscriptLabel: {
    zh: "工作区记录",
    en: "Workspace transcript",
  },
  workspaceRenameSessionDialogLabel: {
    zh: "重命名会话",
    en: "Rename session",
  },
  workspaceRenameSessionSubmit: {
    zh: "保存会话",
    en: "Save session",
  },
  workspaceStreamError: {
    zh: "发送工作区对话失败。",
    en: "Failed to stream workspace chat.",
  },
  "workspaceShell.eyebrow": {
    zh: "工作区",
    en: "Workspace",
  },
  "workspaceShell.titleLabel": {
    zh: "工作区标题",
    en: "Workspace title",
  },
  "workspaceShell.titlePlaceholder": {
    zh: "输入工作区标题",
    en: "Enter a workspace title",
  },
  "workspaceShell.descriptionLabel": {
    zh: "工作区描述",
    en: "Workspace description",
  },
  "workspaceShell.descriptionPlaceholder": {
    zh: "补充当前工作区的目标或上下文",
    en: "Add a short goal or context for this workspace",
  },
  "workspaceShell.saveAction": {
    zh: "保存标题",
    en: "Save title",
  },
  "workspaceShell.analyzeAction": {
    zh: "分析",
    en: "Analyze",
  },
  "workspaceShell.shareAction": {
    zh: "分享",
    en: "Share",
  },
  "workspaceShell.apiAction": {
    zh: "API 访问",
    en: "API access",
  },
  "workspaceShell.newThreadAction": {
    zh: "新建会话",
    en: "New session",
  },
  "workspaceShell.newWorkspaceAction": {
    zh: "新建工作区",
    en: "New workspace",
  },
  "workspaceShell.createDialogTitle": {
    zh: "创建工作区",
    en: "Create workspace",
  },
  "workspaceShell.createDialogSubtitle": {
    zh: "先命名并补充描述，再进入新的工作区。",
    en: "Name the workspace and add a short description before entering it.",
  },
  "workspaceShell.historyRailToggle": {
    zh: "切换历史栏",
    en: "Toggle history rail",
  },
  "workspaceShell.rightRailToggle": {
    zh: "切换右侧栏",
    en: "Toggle right rail",
  },
  "workspaceShell.settingsMenuLabel": {
    zh: "工作区设置菜单",
    en: "Workspace settings menu",
  },
  "workspaceShell.accountMenuLabel": {
    zh: "账户菜单",
    en: "Account menu",
  },
  "workspaceShell.themeSectionTitle": {
    zh: "主题",
    en: "Theme",
  },
  "workspaceShell.languageSectionTitle": {
    zh: "语言",
    en: "Language",
  },
  "workspaceShell.emptyTitle": {
    zh: "还没有工作区内容",
    en: "No workspace content yet",
  },
  "workspaceShell.emptyBody": {
    zh: "先创建会话、上传资料，或从右侧栏开始整理上下文。",
    en: "Start with a thread, upload sources, or organize context from the right rail.",
  },
  "workspaceShell.loadError": {
    zh: "加载工作区失败。",
    en: "Failed to load the workspace.",
  },
  "workspaceRightRail.label": {
    zh: "工作区右侧栏",
    en: "Workspace right rail",
  },
  "workspaceRightRail.sourcesSectionTitle": {
    zh: "内容源",
    en: "Sources",
  },
  "workspaceRightRail.sourcesSectionSubtitle": {
    zh: "管理可检索资料、URL 导入和文档状态。",
    en: "Manage retrievable sources, URL imports, and document status.",
  },
  "workspaceRightRail.notesSectionTitle": {
    zh: "笔记",
    en: "Notes",
  },
  "workspaceRightRail.notesSectionSubtitle": {
    zh: "记录工作区结论、草稿和后续行动。",
    en: "Capture conclusions, drafts, and next actions for the workspace.",
  },
  "workspaceRightRail.viewerSectionTitle": {
    zh: "预览",
    en: "Preview",
  },
  "workspaceRightRail.viewerSectionSubtitle": {
    zh: "查看资料片段、引用内容和原始文本。",
    en: "Inspect source excerpts, citations, and raw content.",
  },
  "workspaceRightRail.sourceUrlLabel": {
    zh: "资料链接",
    en: "Source URLs",
  },
  "workspaceRightRail.sourceUrlPlaceholder": {
    zh: "每行输入一个 URL，支持直接粘贴多行链接",
    en: "Paste one URL per line",
  },
  "workspaceRightRail.addUrlAction": {
    zh: "添加链接",
    en: "Add URL",
  },
  "workspaceRightRail.refreshAction": {
    zh: "刷新",
    en: "Refresh",
  },
  "workspaceRightRail.reindexAction": {
    zh: "重建索引",
    en: "Reindex",
  },
  "workspaceRightRail.emptySourcesTitle": {
    zh: "还没有资料",
    en: "No sources yet",
  },
  "workspaceRightRail.emptySourcesBody": {
    zh: "上传文件或添加 URL 后，资料会出现在这里。",
    en: "Uploaded files and imported URLs will appear here.",
  },
  "workspaceRightRail.emptyNotesTitle": {
    zh: "还没有笔记",
    en: "No notes yet",
  },
  "workspaceRightRail.emptyNotesBody": {
    zh: "把关键结论、决策和行动项记录到这里。",
    en: "Capture key conclusions, decisions, and action items here.",
  },
  "workspaceRightRail.notesSavedBanner": {
    zh: "笔记已同步。",
    en: "Notes synced.",
  },
  "workspaceRightRail.notesSavingBanner": {
    zh: "正在同步笔记...",
    en: "Syncing notes...",
  },
  "workspaceRightRail.notesErrorBanner": {
    zh: "同步笔记失败。",
    en: "Failed to sync notes.",
  },
  "workspaceRightRail.viewerEmptyTitle": {
    zh: "选择资料以查看内容",
    en: "Select a source to preview",
  },
  "workspaceRightRail.viewerEmptyBody": {
    zh: "从资料列表或引用卡片中选中一个条目后，这里会显示预览。",
    en: "Choose a source from the list or a citation card to preview it here.",
  },
  "workspaceRightRail.viewerLoadMoreAction": {
    zh: "加载更多",
    en: "Load more",
  },
  "workspaceRightRail.sourcesError": {
    zh: "加载资料失败。",
    en: "Failed to load sources.",
  },
  "workspaceRightRail.notesError": {
    zh: "加载笔记失败。",
    en: "Failed to load notes.",
  },
  "workspaceRightRail.saveNoteError": {
    zh: "保存笔记失败。",
    en: "Failed to save note.",
  },
  "workspaceRightRail.promoteNoteError": {
    zh: "转换为来源失败。",
    en: "Failed to convert note to source.",
  },
  "workspaceRightRail.promoteNoteEmptyError": {
    zh: "笔记为空，无法转换为来源。",
    en: "Cannot convert an empty note into a source.",
  },
  "workspaceRightRail.viewerError": {
    zh: "加载资料预览失败。",
    en: "Failed to load source preview.",
  },
  "workspaceRightRail.loading": {
    zh: "加载中...",
    en: "Loading...",
  },
  "workspaceRightRail.updating": {
    zh: "更新中...",
    en: "Updating...",
  },
  "workspaceRightRail.totalCount": {
    zh: "共 {count} 项",
    en: "{count} total",
  },
  "workspaceRightRail.selectAllAction": {
    zh: "全选",
    en: "Select all",
  },
  "workspaceRightRail.clearSelectionAction": {
    zh: "清除选择",
    en: "Clear selection",
  },
  "workspaceRightRail.hidePreviewAction": {
    zh: "收起预览",
    en: "Hide preview",
  },
  "workspaceRightRail.sourceActionsLabel": {
    zh: "资料操作",
    en: "Source actions",
  },
  "workspaceRightRail.openSourceAction": {
    zh: "打开预览",
    en: "Open preview",
  },
  "workspaceRightRail.deleteSourceAction": {
    zh: "删除",
    en: "Delete",
  },
  "workspaceRightRail.sourcesListLabel": {
    zh: "资料列表",
    en: "Sources list",
  },
  "workspaceRightRail.notesListLabel": {
    zh: "笔记列表",
    en: "Notes list",
  },
  "workspaceRightRail.newNoteAction": {
    zh: "新建笔记",
    en: "New note",
  },
  "workspaceRightRail.saveNoteAction": {
    zh: "保存笔记",
    en: "Save note",
  },
  "workspaceRightRail.noteEditorToolbar": {
    zh: "笔记编辑工具栏",
    en: "Note editor toolbar",
  },
  "workspaceRightRail.newSourceAction": {
    zh: "添加内容源",
    en: "New Source",
  },
  "workspaceRightRail.addSourceTitle": {
    zh: "添加新资料",
    en: "Add New Source",
  },
  "workspaceRightRail.addSourceSubtitle": {
    zh: "选择文件、网页链接或粘贴内容来扩展当前工作区。",
    en: "Add a file, web link, or pasted content to this workspace.",
  },
  "workspaceRightRail.uploadFileTab": {
    zh: "上传文件",
    en: "Upload File",
  },
  "workspaceRightRail.webLinkTab": {
    zh: "网页链接",
    en: "Web Link",
  },
  "workspaceRightRail.pasteTextTab": {
    zh: "粘贴文本",
    en: "Paste Text",
  },
  "workspaceRightRail.uploadDropTitle": {
    zh: "拖拽文件到这里",
    en: "Drop files here",
  },
  "workspaceRightRail.uploadDropBody": {
    zh: "产品支持上传 {formats} 格式。",
    en: "Supported upload formats: {formats}.",
  },
  "workspaceRightRail.browseFilesAction": {
    zh: "浏览文件",
    en: "Browse Files",
  },
  "workspaceRightRail.addLinkAction": {
    zh: "添加链接",
    en: "Add Link",
  },
  "workspaceRightRail.pasteTitleLabel": {
    zh: "标题",
    en: "Title",
  },
  "workspaceRightRail.pasteContentLabel": {
    zh: "文本内容",
    en: "Text",
  },
  "workspaceRightRail.saveAsSourceAction": {
    zh: "保存为资料",
    en: "Save as Source",
  },
  "workspaceRightRail.untitledNote": {
    zh: "未命名笔记",
    en: "Untitled note",
  },
  "workspaceRightRail.emptyNotePreview": {
    zh: "还没有内容。",
    en: "No content yet.",
  },
  "workspaceRightRail.promotedNoteBadge": {
    zh: "已转换为来源",
    en: "Converted to source",
  },
  "workspaceRightRail.noteTitleLabel": {
    zh: "标题",
    en: "Title",
  },
  "workspaceRightRail.noteContentLabel": {
    zh: "内容",
    en: "Content",
  },
  "workspaceRightRail.idleState": {
    zh: "空闲",
    en: "Idle",
  },
  "workspaceRightRail.loadingSourcePreview": {
    zh: "正在加载资料预览...",
    en: "Loading source preview...",
  },
  "workspaceRightRail.viewerSectionLabel": {
    zh: "资料预览",
    en: "Source viewer",
  },
  "workspaceRightRail.closeViewerAction": {
    zh: "关闭",
    en: "Close",
  },
  "workspaceRightRail.citationFallbackTitle": {
    zh: "引用",
    en: "Citation",
  },
  "workspaceRightRail.viewerScore": {
    zh: "分数 {score}",
    en: "Score {score}",
  },
  "workspaceRightRail.viewerLocation": {
    zh: "第 {page} 页 · 游标 {cursor}",
    en: "Page {page} · Cursor {cursor}",
  },
  "workspaceRightRail.viewerPage": {
    zh: "第 {page} 页",
    en: "Page {page}",
  },
  "workspaceCitation.dialogLabel": {
    zh: "引用片段",
    en: "Citation chunk",
  },
  "workspaceCitation.chunkTitle": {
    zh: "Chunk 内容",
    en: "Chunk content",
  },
  "workspaceCitation.loading": {
    zh: "正在加载引用片段...",
    en: "Loading citation chunk...",
  },
  "workspaceCitation.empty": {
    zh: "当前引用没有可展示的文本内容。",
    en: "This citation does not include displayable text.",
  },
  "workspaceCitation.error": {
    zh: "加载引用片段失败。",
    en: "Failed to load citation chunk.",
  },
  "workspaceRightRail.selectNoteToEdit": {
    zh: "选择一条笔记后可在此直接编辑。",
    en: "Select a note to edit it in place.",
  },
  "workspaceRightRail.promoteNoteAction": {
    zh: "转换为来源",
    en: "Convert to source",
  },
  "workspaceRightRail.deleteNoteAction": {
    zh: "删除笔记",
    en: "Delete note",
  },
  "workspaceRightRail.sessionActionsLabel": {
    zh: "{title} 操作",
    en: "{title} actions",
  },
  "workspaceRightRail.resizePanelsLabel": {
    zh: "调整右侧面板大小",
    en: "Resize right rail panels",
  },
  "workspaceRightRail.sourceStatus.processing": {
    zh: "处理中",
    en: "processing",
  },
  "workspaceRightRail.sourceStatus.pending": {
    zh: "等待中",
    en: "pending",
  },
  "workspaceRightRail.sourceStatus.enqueueing": {
    zh: "入队中",
    en: "enqueueing",
  },
  "workspaceRightRail.sourceStatus.queued": {
    zh: "排队中",
    en: "queued",
  },
  "workspaceRightRail.sourceStatus.indexing": {
    zh: "索引中",
    en: "indexing",
  },
  "workspaceRightRail.sourceStatus.completed": {
    zh: "已完成",
    en: "completed",
  },
  "workspaceRightRail.sourceStatus.ready": {
    zh: "就绪",
    en: "ready",
  },
  "workspaceRightRail.sourceStatus.failed": {
    zh: "失败",
    en: "failed",
  },
  "workspaceRightRail.sourceStatus.error": {
    zh: "异常",
    en: "error",
  },
  "sharedPublic.backHomeAction": {
    zh: "返回首页",
    en: "Back home",
  },
  "sharedPublic.pageTitle": {
    zh: "共享工作区",
    en: "Shared workspace",
  },
  "sharedPublic.pageSubtitle": {
    zh: "通过共享链接浏览内容。登录后可提问。",
    en: "Browse shared content by link. Sign in to ask questions.",
  },
  "sharedPublic.readAccessLabel": {
    zh: "访问方式",
    en: "Read access",
  },
  "sharedPublic.readAccessValue": {
    zh: "任何人可查看",
    en: "Anyone with the link can view",
  },
  "sharedPublic.interactionAccessLabel": {
    zh: "互动权限",
    en: "Interaction",
  },
  "sharedPublic.interactionAccessValue": {
    zh: "登录后可提问",
    en: "Sign in to ask questions",
  },
  "sharedPublic.loading": {
    zh: "正在加载共享内容...",
    en: "Loading shared content...",
  },
  "sharedPublic.invalidLinkTitle": {
    zh: "共享链接不可用",
    en: "Share link unavailable",
  },
  "sharedPublic.invalidLinkBody": {
    zh: "这个共享链接无效、已撤销，或已经过期。",
    en: "This share link is invalid, revoked, or expired.",
  },
  "sharedPublic.permissionLabel": {
    zh: "权限",
    en: "Permission",
  },
  "sharedPublic.scopeLabel": {
    zh: "范围",
    en: "Scope",
  },
  "sharedPublic.expiresAtLabel": {
    zh: "过期时间",
    en: "Expires at",
  },
  "sharedPublic.downloadPolicyLabel": {
    zh: "下载策略",
    en: "Download policy",
  },
  "sharedPublic.downloadAllowed": {
    zh: "允许下载",
    en: "Downloads allowed",
  },
  "sharedPublic.downloadOnlineOnly": {
    zh: "仅在线查看",
    en: "View online only",
  },
  "sharedPublic.sourcesSectionTitle": {
    zh: "共享资料",
    en: "Shared sources",
  },
  "sharedPublic.sourcesSectionSubtitle": {
    zh: "当前公开可见的资料列表和状态。",
    en: "The list of publicly visible sources and their current status.",
  },
  "sharedPublic.sourcesEmptyTitle": {
    zh: "暂时没有可见资料",
    en: "No visible sources yet",
  },
  "sharedPublic.sourcesEmptyBody": {
    zh: "共享已开启，但当前没有可公开浏览的资料。",
    en: "Sharing is enabled, but there are no publicly visible sources yet.",
  },
  "sharedPublic.chatSectionTitle": {
    zh: "共享问答",
    en: "Shared Q&A",
  },
  "sharedPublic.chatSectionSubtitle": {
    zh: "查看内容无需登录，登录后才能发起问答。",
    en: "Viewing does not require sign-in. Asking questions does.",
  },
  "sharedPublic.questionLabel": {
    zh: "提问",
    en: "Question",
  },
  "sharedPublic.questionPlaceholder": {
    zh: "输入你的问题",
    en: "Ask a question",
  },
  "sharedPublic.submitAction": {
    zh: "开始提问",
    en: "Ask question",
  },
  "sharedPublic.submitting": {
    zh: "回答中...",
    en: "Answering...",
  },
  "sharedPublic.answerTitle": {
    zh: "回答",
    en: "Answer",
  },
  "sharedPublic.citationsTitle": {
    zh: "引用资料",
    en: "Citations",
  },
  "sharedPublic.degradedBanner": {
    zh: "回答经过降级处理。",
    en: "This answer was served in a degraded mode.",
  },
  "sharedPublic.inviteTitle": {
    zh: "工作区邀请",
    en: "Workspace invite",
  },
  "sharedPublic.inviteSubtitle": {
    zh: "登录后可接受或拒绝这条工作区邀请。",
    en: "Sign in to accept or decline this workspace invite.",
  },
  "sharedPublic.acceptInviteAction": {
    zh: "接受邀请",
    en: "Accept invite",
  },
  "sharedPublic.declineInviteAction": {
    zh: "拒绝邀请",
    en: "Decline invite",
  },
  "sharedPublic.signInToContinueAction": {
    zh: "登录后继续",
    en: "Sign in to continue",
  },
  "sharedPublic.signUpToContinueAction": {
    zh: "注册后继续",
    en: "Sign up to continue",
  },
  "sharedPublic.signInRequiredTitle": {
    zh: "登录后可提问",
    en: "Sign in to ask questions",
  },
  "sharedPublic.signInRequiredBody": {
    zh: "查看内容无需登录。为了控制 AI 成本，只有登录用户才能发起提问和互动。",
    en: "Viewing does not require sign-in. To control AI cost, only signed-in users can ask questions and interact.",
  },
  "sharedPublic.inviteAcceptedTitle": {
    zh: "已接受邀请",
    en: "Invite accepted",
  },
  "sharedPublic.inviteDeclinedTitle": {
    zh: "已拒绝邀请",
    en: "Invite declined",
  },
  "shareCenter.tabsLabel": {
    zh: "分享中心选项卡",
    en: "Share center tabs",
  },
  "shareCenter.tabs.settings": {
    zh: "设置",
    en: "Settings",
  },
  "shareCenter.tabs.analytics": {
    zh: "分析",
    en: "Analytics",
  },
  "shareCenter.tabs.accessLogs": {
    zh: "访问日志",
    en: "Access logs",
  },
  "shareCenter.pageTitle": {
    zh: "分享中心",
    en: "Share center",
  },
  "shareCenter.pageSubtitle": {
    zh: "管理公开访问、成员邀请和分享数据。",
    en: "Manage public access, member invites, and sharing analytics.",
  },
  "shareCenter.settingsSectionTitle": {
    zh: "分享设置",
    en: "Share settings",
  },
  "shareCenter.settingsSectionSubtitle": {
    zh: "控制访问级别、下载策略和链接有效期。",
    en: "Control access level, download policy, and link lifetime.",
  },
  "shareCenter.statusCardTitle": {
    zh: "当前状态",
    en: "Current status",
  },
  "shareCenter.statusEnabled": {
    zh: "已启用分享",
    en: "Sharing enabled",
  },
  "shareCenter.statusDisabled": {
    zh: "未启用分享",
    en: "Sharing disabled",
  },
  "shareCenter.accessLevelLabel": {
    zh: "访问级别",
    en: "Access level",
  },
  "shareCenter.accessLevel.private": {
    zh: "私有",
    en: "Private",
  },
  "shareCenter.accessLevel.link": {
    zh: "仅链接",
    en: "Link only",
  },
  "shareCenter.accessLevel.public": {
    zh: "公开",
    en: "Public",
  },
  "shareCenter.expiresAtLabel": {
    zh: "过期时间",
    en: "Expiration time",
  },
  "shareCenter.expiresAtPlaceholder": {
    zh: "留空表示不过期",
    en: "Leave empty for no expiration",
  },
  "shareCenter.allowDownloadLabel": {
    zh: "允许下载",
    en: "Allow downloads",
  },
  "shareCenter.allowDownloadHint": {
    zh: "启用后，访客可下载当前公开资料。",
    en: "When enabled, visitors can download the currently shared sources.",
  },
  "shareCenter.shareUrlLabel": {
    zh: "分享链接",
    en: "Share link",
  },
  "shareCenter.shareUrlPlaceholder": {
    zh: "生成链接后会显示在这里",
    en: "The share link will appear here after it is generated",
  },
  "shareCenter.copyLinkAction": {
    zh: "复制链接",
    en: "Copy link",
  },
  "shareCenter.generateLinkAction": {
    zh: "生成链接",
    en: "Generate link",
  },
  "shareCenter.disableShareAction": {
    zh: "关闭分享",
    en: "Disable sharing",
  },
  "shareCenter.saveAction": {
    zh: "保存更改",
    en: "Save changes",
  },
  "shareCenter.saving": {
    zh: "保存中...",
    en: "Saving...",
  },
  "shareCenter.inviteSectionTitle": {
    zh: "成员邀请",
    en: "Member invites",
  },
  "shareCenter.inviteSectionSubtitle": {
    zh: "邀请成员加入当前工作区并控制其访问角色。",
    en: "Invite members into this workspace and control their access role.",
  },
  "shareCenter.inviteEmailLabel": {
    zh: "邀请邮箱",
    en: "Invite email",
  },
  "shareCenter.inviteEmailPlaceholder": {
    zh: "name@example.com",
    en: "name@example.com",
  },
  "shareCenter.inviteRoleLabel": {
    zh: "邀请角色",
    en: "Invite role",
  },
  "shareCenter.inviteRole.viewer": {
    zh: "查看者",
    en: "Viewer",
  },
  "shareCenter.inviteRole.editor": {
    zh: "编辑者",
    en: "Editor",
  },
  "shareCenter.inviteAction": {
    zh: "发送邀请",
    en: "Send invite",
  },
  "shareCenter.membersSectionTitle": {
    zh: "成员列表",
    en: "Members",
  },
  "shareCenter.membersEmptyTitle": {
    zh: "还没有共享成员",
    en: "No shared members yet",
  },
  "shareCenter.membersEmptyBody": {
    zh: "发送邀请后，成员会显示在这里。",
    en: "Invited members will appear here.",
  },
  "shareCenter.analyticsSectionTitle": {
    zh: "分享分析",
    en: "Share analytics",
  },
  "shareCenter.analyticsSectionSubtitle": {
    zh: "查看总访问量、独立访客和按天访问趋势。",
    en: "Review total views, unique visitors, and the daily trend.",
  },
  "shareCenter.analytics.totalViews": {
    zh: "总访问量",
    en: "Total views",
  },
  "shareCenter.analytics.uniqueVisitors": {
    zh: "独立访客",
    en: "Unique visitors",
  },
  "shareCenter.analytics.dailyViews": {
    zh: "按天访问量",
    en: "Daily views",
  },
  "shareCenter.analyticsEmptyTitle": {
    zh: "暂时还没有分享访问数据",
    en: "No sharing analytics yet",
  },
  "shareCenter.analyticsEmptyBody": {
    zh: "当访客开始访问共享链接后，这里会显示趋势数据。",
    en: "Trends will appear here once visitors start using the share link.",
  },
  "shareCenter.accessLogsSectionTitle": {
    zh: "访问日志",
    en: "Access logs",
  },
  "shareCenter.accessLogsSectionSubtitle": {
    zh: "查看最近访客、访问时间和访问动作。",
    en: "Review recent visitors, access times, and actions.",
  },
  "shareCenter.accessLogsEmptyTitle": {
    zh: "暂时还没有访问日志",
    en: "No access logs yet",
  },
  "shareCenter.accessLogsEmptyBody": {
    zh: "公开访问发生后，这里会显示访问记录。",
    en: "Logs will appear here after public visits occur.",
  },
  "shareCenter.accessLogs.visitorIdColumn": {
    zh: "访客 ID",
    en: "Visitor ID",
  },
  "shareCenter.accessLogs.accessedAtColumn": {
    zh: "访问时间",
    en: "Accessed at",
  },
  "shareCenter.accessLogs.actionColumn": {
    zh: "动作",
    en: "Action",
  },
  "shareCenter.loadError": {
    zh: "加载分享中心失败。",
    en: "Failed to load the share center.",
  },
  "shareCenter.saveError": {
    zh: "保存分享设置失败。",
    en: "Failed to save share settings.",
  },
  "shareCenter.inviteError": {
    zh: "发送邀请失败。",
    en: "Failed to send invite.",
  },
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
  "admin.shellTitle": {
    zh: "后台管理",
    en: "Admin",
  },
  "admin.navLabel": {
    zh: "后台导航",
    en: "Admin navigation",
  },
  "admin.nav.organizations": {
    zh: "组织",
    en: "Organizations",
  },
  "admin.nav.users": {
    zh: "用户",
    en: "Users",
  },
  "admin.nav.usage": {
    zh: "用量",
    en: "Usage",
  },
  "admin.nav.billing": {
    zh: "账单",
    en: "Billing",
  },
  "admin.nav.health": {
    zh: "健康",
    en: "Health",
  },
  "admin.nav.ragHealth": {
    zh: "RAG 健康",
    en: "RAG Health",
  },
  "admin.nav.featureFlags": {
    zh: "功能开关",
    en: "Feature flags",
  },
  "admin.nav.workers": {
    zh: "执行器",
    en: "Workers",
  },
  "admin.nav.degradation": {
    zh: "降级",
    en: "Degradation",
  },
  "admin.nav.auditLogs": {
    zh: "审计日志",
    en: "Audit logs",
  },
  "admin.pageSubtitle": {
    zh: "查看组织、用量、健康状态和系统级运营数据。",
    en: "Review organizations, usage, health, and system-wide operational signals.",
  },
  "admin.searchLabel": {
    zh: "搜索",
    en: "Search",
  },
  "admin.searchPlaceholder": {
    zh: "按名称、邮箱或资源 ID 筛选",
    en: "Filter by name, email, or resource ID",
  },
  "admin.filter.statusLabel": {
    zh: "状态",
    en: "Status",
  },
  "admin.filter.roleLabel": {
    zh: "角色",
    en: "Role",
  },
  "admin.filter.periodLabel": {
    zh: "周期",
    en: "Period",
  },
  "admin.filter.windowLabel": {
    zh: "时间窗口",
    en: "Time window",
  },
  "admin.filter.pageSizeLabel": {
    zh: "每页条数",
    en: "Rows per page",
  },
  "admin.filter.sortLabel": {
    zh: "排序",
    en: "Sort",
  },
  "admin.refreshAction": {
    zh: "刷新",
    en: "Refresh",
  },
  "admin.exportAction": {
    zh: "导出",
    en: "Export",
  },
  "admin.detailsAction": {
    zh: "查看详情",
    en: "View details",
  },
  "admin.blockAction": {
    zh: "封禁",
    en: "Block",
  },
  "admin.unblockAction": {
    zh: "解除封禁",
    en: "Unblock",
  },
  "admin.emptyTitle": {
    zh: "没有匹配结果",
    en: "No matching results",
  },
  "admin.emptyBody": {
    zh: "调整筛选条件后再试一次。",
    en: "Adjust the current filters and try again.",
  },
  "admin.loadError": {
    zh: "加载后台数据失败。",
    en: "Failed to load admin data.",
  },
  "admin.table.organization": {
    zh: "组织",
    en: "Organization",
  },
  "admin.table.plan": {
    zh: "计划",
    en: "Plan",
  },
  "admin.table.status": {
    zh: "状态",
    en: "Status",
  },
  "admin.table.users": {
    zh: "用户数",
    en: "Users",
  },
  "admin.table.requests": {
    zh: "请求数",
    en: "Requests",
  },
  "admin.table.createdAt": {
    zh: "创建时间",
    en: "Created at",
  },
  "admin.table.lastActive": {
    zh: "最近活跃",
    en: "Last active",
  },
  "admin.metrics.totalOrganizations": {
    zh: "组织总数",
    en: "Total organizations",
  },
  "admin.metrics.totalUsers": {
    zh: "用户总数",
    en: "Total users",
  },
  "admin.metrics.totalRequests": {
    zh: "请求总数",
    en: "Total requests",
  },
  "admin.metrics.totalDocuments": {
    zh: "文档总数",
    en: "Total documents",
  },
  "admin.health.sectionTitle": {
    zh: "系统健康",
    en: "System health",
  },
  "admin.health.sectionSubtitle": {
    zh: "检查服务状态、退化信号和恢复建议。",
    en: "Check service status, degradation signals, and recovery hints.",
  },
  "admin.billing.sectionTitle": {
    zh: "账单概览",
    en: "Billing overview",
  },
  "admin.billing.sectionSubtitle": {
    zh: "查看计划分布、收款状态和账单风险。",
    en: "Review plan mix, collection status, and billing risks.",
  },
  "admin.featureFlags.sectionTitle": {
    zh: "功能开关",
    en: "Feature flags",
  },
  "admin.featureFlags.sectionSubtitle": {
    zh: "管理开关状态、变更请求和审核流。",
    en: "Manage flag state, change requests, and review flow.",
  },
  "admin.auditLogs.sectionTitle": {
    zh: "审计日志",
    en: "Audit logs",
  },
  "admin.auditLogs.sectionSubtitle": {
    zh: "按动作、资源和执行者追踪后台操作。",
    en: "Trace admin activity by action, resource, and actor.",
  },
  "admin.workers.sectionTitle": {
    zh: "执行器状态",
    en: "Worker status",
  },
  "admin.workers.sectionSubtitle": {
    zh: "查看执行队列、处理能力和异常节点。",
    en: "Review queue health, capacity, and failing workers.",
  },
  "admin.degradation.sectionTitle": {
    zh: "降级状态",
    en: "Degradation status",
  },
  "admin.degradation.sectionSubtitle": {
    zh: "查看当前降级策略、触发原因和影响范围。",
    en: "Review active degradation policies, triggers, and blast radius.",
  },
  "admin.status.active": {
    zh: "正常",
    en: "Active",
  },
  "admin.status.blocked": {
    zh: "已封禁",
    en: "Blocked",
  },
  "admin.status.healthy": {
    zh: "健康",
    en: "Healthy",
  },
  "admin.status.degraded": {
    zh: "降级中",
    en: "Degraded",
  },
  "admin.status.unhealthy": {
    zh: "异常",
    en: "Unhealthy",
  },
} satisfies Record<string, UiMessageDescriptor>;

export type UiMessageKey = keyof typeof UI_MESSAGES;

type UiMessageCatalog = {
  [key: string]: string | UiMessageCatalog;
};

function insertCatalogValue(catalog: UiMessageCatalog, key: string, value: string) {
  if (!key.includes(".")) {
    catalog[key] = value;
    return;
  }

  const segments = key.split(".");
  let cursor = catalog;

  for (const segment of segments.slice(0, -1)) {
    const current = cursor[segment];

    if (!current || typeof current === "string") {
      cursor[segment] = {};
    }

    cursor = cursor[segment] as UiMessageCatalog;
  }

  cursor[segments[segments.length - 1]!] = value;
}

function buildLocaleCatalog(locale: UiLocale): UiMessageCatalog {
  const catalog: UiMessageCatalog = {};

  for (const [key, descriptor] of Object.entries(UI_MESSAGES)) {
    insertCatalogValue(catalog, key, locale === "zh-CN" ? descriptor.zh : descriptor.en);
  }

  return catalog;
}

const MESSAGE_CATALOG_BY_LOCALE: Record<UiLocale, UiMessageCatalog> = {
  "zh-CN": buildLocaleCatalog("zh-CN"),
  en: buildLocaleCatalog("en"),
};

export function getMessageCatalog(locale: UiLocale) {
  return MESSAGE_CATALOG_BY_LOCALE[locale];
}

function interpolate(template: string, values?: Record<string, string | number>) {
  if (!values) {
    return template;
  }

  return template.replace(/\{(\w+)\}/g, (_match, key: string) => String(values[key] ?? ""));
}

export function formatUiMessage(
  locale: UiLocale,
  key: UiMessageKey,
  values?: Record<string, string | number>,
) {
  const descriptor = UI_MESSAGES[key];
  const template = locale === "zh-CN" ? descriptor.zh : descriptor.en;

  return interpolate(template, values);
}
