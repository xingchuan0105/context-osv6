import type { UiMessageDescriptor } from "./types";

/**
 * Workspace API access surface + related developer-facing strings.
 */
export const apiAccessMessages = {
  "apiAccess.backWorkspace": {
    zh: "返回工作区",
    en: "Back to workspace",
  },
  "apiAccess.overline": {
    zh: "工作区 API",
    en: "Workspace API",
  },
  "apiAccess.title": {
    zh: "API 访问",
    en: "API access",
  },
  "apiAccess.subtitle": {
    zh: "为这个工作区创建 API 密钥，并查看面向开发者与 LLM agents 的接入说明。",
    en: "Create API keys for this workspace and view developer / agent setup docs.",
  },
  "apiAccess.workspaceIdNote": {
    zh: "在 API 路径中，这个工作区对应 workspace_id。",
    en: "In API paths this workspace is identified by workspace_id.",
  },
  "apiAccess.embeddedLead": {
    zh: "为这个工作区创建 API 密钥。下方提供人类说明与 agent 稳定文档入口。",
    en: "Create API keys for this workspace. Human docs and a stable agent doc are below.",
  },
  "apiAccess.createTitle": {
    zh: "创建 API 密钥",
    en: "Create API key",
  },
  "apiAccess.createSubtitle": {
    zh: "按工作区粒度创建密钥，控制索引能力与频率限制。",
    en: "Keys are scoped to this workspace for indexing capability and rate limits.",
  },
  "apiAccess.nameLabel": {
    zh: "密钥名称",
    en: "Key name",
  },
  "apiAccess.permissionsLabel": {
    zh: "权限",
    en: "Permissions",
  },
  "apiAccess.permIndex": {
    zh: "索引（index）",
    en: "Index",
  },
  "apiAccess.permQuery": {
    zh: "查询（query）",
    en: "Query",
  },
  "apiAccess.permNote": {
    zh: "API 密钥只支持资料管理与知识库查询；聊天与网络搜索默认不可用。",
    en: "API keys cover source management and knowledge-base query only; chat and web search are not available by default.",
  },
  "apiAccess.rateLimitLabel": {
    zh: "速率限制（RPM）",
    en: "Rate limit (RPM)",
  },
  "apiAccess.expiresLabel": {
    zh: "过期时间 RFC3339（可选）",
    en: "Expires at RFC3339 (optional)",
  },
  "apiAccess.createAction": {
    zh: "创建密钥",
    en: "Create key",
  },
  "apiAccess.creating": {
    zh: "创建中…",
    en: "Creating…",
  },
  "apiAccess.newKeyTitle": {
    zh: "新密钥",
    en: "New key",
  },
  "apiAccess.newKeyOnce": {
    zh: "明文只会返回这一次。",
    en: "Plaintext is shown only once.",
  },
  "apiAccess.listTitle": {
    zh: "已创建密钥",
    en: "Created keys",
  },
  "apiAccess.listSubtitle": {
    zh: "仅展示当前仍处于生效状态的工作区 API 密钥。",
    en: "Only active workspace API keys are listed.",
  },
  "apiAccess.loading": {
    zh: "加载中…",
    en: "Loading…",
  },
  "apiAccess.empty": {
    zh: "还没有 API 密钥。",
    en: "No API keys yet.",
  },
  "apiAccess.statusActive": {
    zh: "生效中",
    en: "Active",
  },
  "apiAccess.statusRevoked": {
    zh: "已撤销",
    en: "Revoked",
  },
  "apiAccess.permShortIndex": {
    zh: "索引",
    en: "Index",
  },
  "apiAccess.permShortQuery": {
    zh: "查询",
    en: "Query",
  },
  "apiAccess.metaExpires": {
    zh: "过期时间 {value}",
    en: "Expires {value}",
  },
  "apiAccess.never": {
    zh: "永不",
    en: "Never",
  },
  "apiAccess.metaLastUsed": {
    zh: "最近使用 {value}",
    en: "Last used {value}",
  },
  "apiAccess.neverUsed": {
    zh: "从未",
    en: "Never",
  },
  "apiAccess.revoke": {
    zh: "撤销",
    en: "Revoke",
  },
  "apiAccess.revoking": {
    zh: "撤销中…",
    en: "Revoking…",
  },
  "apiAccess.agentTitle": {
    zh: "给 Agent 用",
    en: "For agents",
  },
  "apiAccess.agentSubtitle": {
    zh: "把本工作区交给 Claude Code / Codex / Cursor：先创建密钥（上方），再复制下列字段与 MCP 配置。",
    en: "Hand this workspace to Claude Code / Codex / Cursor: create a key above, then copy fields and MCP config.",
  },
  "apiAccess.agentDesktopHint": {
    zh: "本机客户端默认 API 为 127.0.0.1:18080；请先确认客户端栈已启动。",
    en: "Local client default API is 127.0.0.1:18080; ensure the client stack is running.",
  },
  "apiAccess.agentCloudHint": {
    zh: "云端与本机使用同一 MCP 工具表，仅 base URL 不同。",
    en: "Cloud and local use the same MCP tool table; only the base URL differs.",
  },
  "apiAccess.copy": {
    zh: "复制",
    en: "Copy",
  },
  "apiAccess.copyConfig": {
    zh: "复制配置",
    en: "Copy config",
  },
  "apiAccess.copyToken": {
    zh: "复制 token",
    en: "Copy token",
  },
  "apiAccess.copied": {
    zh: "已复制：{label}",
    en: "Copied: {label}",
  },
  "apiAccess.copyFailed": {
    zh: "复制失败，请手动选择 {label}",
    en: "Copy failed; select {label} manually",
  },
  "apiAccess.mcpSnippetTitle": {
    zh: "stdio MCP 配置片段",
    en: "stdio MCP snippet",
  },
  "apiAccess.mcpSnippetHint": {
    zh: "粘贴到 Claude Code / Cursor 的 MCP 配置；将 command 换成本机 context-os-mcp 路径，密钥用上方一次性明文替换。",
    en: "Paste into Claude Code / Cursor MCP config; set command to your local context-os-mcp path and replace the key with the one-time plaintext above.",
  },
  "apiAccess.agentTokenTitle": {
    zh: "用户态 agent token（建库）",
    en: "User agent token (create workspace)",
  },
  "apiAccess.agentTokenHint": {
    zh: "短时用户 JWT（默认 120 分钟）。export 为 CONTEXT_OS_USER_TOKEN 后可用 context-os workspace create；工作区密钥仍不能建库。分享仍走 UI。",
    en: "Short-lived user JWT (default 120 min). Export as CONTEXT_OS_USER_TOKEN for context-os workspace create; workspace keys cannot create workspaces. Sharing stays in the UI.",
  },
  "apiAccess.mintToken": {
    zh: "签发 2h token",
    en: "Mint 2h token",
  },
  "apiAccess.minting": {
    zh: "签发中…",
    en: "Minting…",
  },
  "apiAccess.mintedWithExpiry": {
    zh: "已签发 agent token（约 {minutes} 分钟，至 {expires}）",
    en: "Minted agent token (~{minutes} min, until {expires})",
  },
  "apiAccess.minted": {
    zh: "已签发 agent token",
    en: "Minted agent token",
  },
  "apiAccess.agentProbeNote": {
    zh: "探活：context-os status。本机桌面可 context-os auth from-desktop --save 写入 ~/.config/context-os/user.token（CLI/MCP 自动加载）。脚本：context-os ingest / ask / share enable（需 user token）。工具参数 workspace_id 须与本页一致。",
    en: "Probe: context-os status. On desktop, context-os auth from-desktop --save writes ~/.config/context-os/user.token (auto-loaded by CLI/MCP). Scripts: context-os ingest / ask / share enable (user token). Tool args must use this page’s workspace_id.",
  },
  "apiAccess.docsTitle": {
    zh: "给 LLM Agents",
    en: "For LLM agents",
  },
  "apiAccess.docsSubtitle": {
    zh: "给要接入这个工作区的 agent 看的入口卡。先理解边界，再读取稳定文档。",
    en: "Onboarding card for agents connecting to this workspace. Read the boundary first, then the stable doc.",
  },
  "apiAccess.docsOrderTitle": {
    zh: "推荐顺序",
    en: "Suggested order",
  },
  "apiAccess.docsOrderBody": {
    zh: "如果 agent 要直连这个工作区，先读人类说明确认作用域，再读取稳定 agent 文档执行。",
    en: "If an agent connects directly, read the human guide for scope, then the stable agent doc to execute.",
  },
  "apiAccess.docsStep1Title": {
    zh: "先读说明页",
    en: "Read the guide first",
  },
  "apiAccess.docsStep1Body": {
    zh: "看清支持范围、认证方式，以及工作区与 workspace_id 的映射。",
    en: "Confirm scope, auth, and the workspace ↔ workspace_id mapping.",
  },
  "apiAccess.docsStep2Title": {
    zh: "再读稳定 agent 文档",
    en: "Then the stable agent doc",
  },
  "apiAccess.docsStep2Body": {
    zh: "这份链接适合 agent 直接抓取，内容更短，也更适合程序化读取。",
    en: "This link is short and stable for agents to fetch programmatically.",
  },
  "apiAccess.errMissingWorkspaceId": {
    zh: "工作区 ID 缺失，未发起 API 请求。请检查路由参数。",
    en: "Missing workspace id; no API call was made. Check the route.",
  },
  "apiAccess.errInvalidWorkspaceId": {
    zh: "工作区 ID 无效（{id} 不是 UUID），未发起 API 请求。",
    en: "Invalid workspace id ({id} is not a UUID); no API call was made.",
  },
  "apiAccess.errSession": {
    zh: "登录状态失效，请重新登录。",
    en: "Session expired; sign in again.",
  },
  "apiAccess.errSessionMint": {
    zh: "登录状态失效，请重新登录后再签发 agent token。",
    en: "Session expired; sign in again before minting an agent token.",
  },
  "apiAccess.errNameRequired": {
    zh: "请输入密钥名称。",
    en: "Enter a key name.",
  },
  "apiAccess.errRateLimit": {
    zh: "速率限制必须是正整数。",
    en: "Rate limit must be a positive integer.",
  },
  "apiAccess.errPermissions": {
    zh: "请至少选择一种权限。",
    en: "Select at least one permission.",
  },
  "apiAccess.errLoadKeys": {
    zh: "加载 API 密钥失败",
    en: "Failed to load API keys",
  },
  "apiAccess.errCreateKey": {
    zh: "创建 API 密钥失败",
    en: "Failed to create API key",
  },
  "apiAccess.errRevokeKey": {
    zh: "撤销 API 密钥失败",
    en: "Failed to revoke API key",
  },
  "apiAccess.errMintToken": {
    zh: "签发 agent token 失败",
    en: "Failed to mint agent token",
  },
  "apiAccess.errNoToken": {
    zh: "未返回 token",
    en: "No token returned",
  },
  "apiAccess.errWithDetail": {
    zh: "{fallback}：{detail}",
    en: "{fallback}: {detail}",
  },
  "settingsProvider.type.agentLlm": {
    zh: "主 Agent LLM",
    en: "Main agent LLM",
  },
  "settingsProvider.type.parseLlm": {
    zh: "文件解析 LLM",
    en: "File-parse LLM",
  },
  "settingsProvider.type.embedRerank": {
    zh: "向量嵌入 / 重排序",
    en: "Embedding / Rerank",
  },
  "settingsProvider.model.deepseek": {
    zh: "DeepSeek API · DeepSeek V4 Flash",
    en: "DeepSeek API · DeepSeek V4 Flash",
  },
  "settingsProvider.model.bailian": {
    zh: "百炼 API · Qwen3.7 Flash",
    en: "Bailian API · Qwen3.7 Flash",
  },
  "settingsProvider.model.siliconflow": {
    zh: "SiliconFlow · BAAI/bge-m3 · BAAI/bge-reranker-v2-m3",
    en: "SiliconFlow · BAAI/bge-m3 · BAAI/bge-reranker-v2-m3",
  },
} satisfies Record<string, UiMessageDescriptor>;
