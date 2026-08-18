import type { UiMessageDescriptor } from "./types";

/**
 * Workspace API access surface + related developer-facing strings.
 */
export const apiAccessMessages = {
  "apiAccess.title": {
    zh: "API 访问",
    en: "API access",
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
    zh: "接入本工作区（给 Agent）",
    en: "Connect this workspace (for agents)",
  },
  "apiAccess.agentSubtitle": {
    zh: "创建密钥后，一次复制完整接入包，粘贴给 Cursor / Claude / 外接 Agent 即可连接。",
    en: "After creating a key, copy the full agent pack once and paste it into Cursor / Claude / an external agent.",
  },
  "apiAccess.agentDesktopHint": {
    zh: "本机客户端默认 API 为 127.0.0.1:18080；请先确认客户端栈已启动。",
    en: "Local client default API is 127.0.0.1:18080; ensure the client stack is running.",
  },
  "apiAccess.agentCloudHint": {
    zh: "云端与本机使用同一 MCP 工具表，仅 base URL 不同。",
    en: "Cloud and local use the same MCP tool table; only the base URL differs.",
  },
  "apiAccess.copyPack": {
    zh: "复制完整接入包",
    en: "Copy full agent pack",
  },
  "apiAccess.copyPackDisabled": {
    zh: "先创建密钥",
    en: "Create a key first",
  },
  "apiAccess.packReady": {
    zh: "已含本次密钥，可直接交给 Agent。",
    en: "Includes this session’s key — ready to hand to an agent.",
  },
  "apiAccess.packNeedKey": {
    zh: "请先在上方创建工作区密钥，主按钮才会启用。",
    en: "Create a workspace key above to enable the primary copy action.",
  },
  "apiAccess.packCopied": {
    zh: "已复制完整接入包，可粘贴给 Agent",
    en: "Full agent pack copied — paste into your agent",
  },
  "apiAccess.packPreviewTitle": {
    zh: "接入包预览",
    en: "Pack preview",
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
  "apiAccess.advancedTitle": {
    zh: "高级",
    en: "Advanced",
  },
  "apiAccess.advancedHint": {
    zh: "单字段复制、用户态 token、探测说明。日常接入用上方完整包即可。",
    en: "Per-field copy, user agent token, probes. Day-to-day setup uses the full pack above.",
  },
  "apiAccess.mcpSnippetTitle": {
    zh: "stdio MCP 配置片段",
    en: "stdio MCP snippet",
  },
  "apiAccess.mcpSnippetHint": {
    zh: "已含于完整接入包；需要单独复制时用此按钮。",
    en: "Already included in the full pack; use this to copy JSON alone.",
  },
  "apiAccess.agentTokenTitle": {
    zh: "用户态 agent token（建库 / 分享工具）",
    en: "User agent token (create workspace / share tools)",
  },
  "apiAccess.agentTokenHint": {
    zh: "短时用户 JWT（默认 120 分钟）。工作区密钥不能建库或管分享；仅高级自动化需要。",
    en: "Short-lived user JWT (default 120 min). Workspace keys cannot create workspaces or manage share; advanced only.",
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
    zh: "探活：context-os status。本机可 context-os auth from-desktop --save。工具参数 workspace_id 须与本页一致。",
    en: "Probe: context-os status. Desktop: context-os auth from-desktop --save. Tool args must use this page’s workspace_id.",
  },
  "apiAccess.docsTitle": {
    zh: "说明链接",
    en: "Documentation",
  },
  "apiAccess.docsSubtitle": {
    zh: "连接优先；文档作补强。链接均为绝对地址。",
    en: "Connect first; docs are secondary. Links are absolute.",
  },
  "apiAccess.docsHumanLabel": {
    zh: "给人类",
    en: "For humans",
  },
  "apiAccess.docsAgentLabel": {
    zh: "给 Agent",
    en: "For agents",
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
    zh: "文档索引 / 检索子代理",
    en: "Indexing / retrieval worker",
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
