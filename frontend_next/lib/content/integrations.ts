/**
 * /integrations/* 内容单一数据源（Phase E Slice 2）。
 * 承接页只做需求承接 + 锚链：接入步骤、端点、工具、错误码的单一事实源是
 * `public/docs/api-access-for-agents.md`（/help/api-access/agents）。
 * 每个事实（端点、字段、工具名、错误码）都取自该文档，不在本文件发明新声明。
 * en 版待人工翻译后并入（模式同 faq.ts / compare.ts）。
 */

export type IntegrationSection = {
  h2: string;
  paragraphs?: string[];
  bullets?: string[];
  code?: { label: string; text: string };
  table?: { headers: string[]; rows: string[][] };
};

export type IntegrationSlug = "cursor" | "claude-desktop" | "mcp";

export type IntegrationDoc = {
  slug: IntegrationSlug;
  cardTitle: string;
  cardDesc: string;
  h1: string;
  subtitle: string;
  intro: string[];
  sections: IntegrationSection[];
};

export const integrationsUpdated = "2026-09-02";

export const integrationsShared = {
  indexSubtitle: "集成承接",
  indexH1: "把 Context OS 知识库接进你的 AI 工具",
  indexIntro: [
    "Context OS 为每个工作区提供独立的 MCP HTTP 端点与工作区密钥：一个密钥只作用于一个工作区，权限只有 index（入库）与 query（检索问答）两种。下面按客户端给出可复现的接入步骤。",
    "接入协议（端点、工具目录、错误码）的单一事实源是 Agent API 文档；本文只做需求承接与步骤引导，若页面与文档不一致，以文档为准。",
    "不确定从哪页开始：用 Cursor 写代码选 Cursor 页；用 Claude 桌面端选 Claude Desktop 页；其他 MCP 客户端或想直接看协议细节，从 MCP 总入口进入。",
  ],
  indexEvidenceLine:
    "本页为集成承接导航；声明的来源与方法：步骤、端点与错误码均锚定 Agent API 文档（/help/api-access/agents）并对照公开定价页逐条核对。",
  authorPrefix: "产品：Context OS · 品牌：ContextLM · 作者：",
  authorName: "邢川",
  updatedLabel: "页面说明更新日期：",
  buttonAgents: "Agent API 文档",
  buttonDesktop: "免费客户端",
  buttonPricing: "定价",
  evidenceTitle: "证据与进一步阅读",
  evidenceItems: [
    { label: "Agent API 文档（接入事实源）", href: "/help/api-access/agents" },
    { label: "人类接入说明", href: "/help/api-access" },
    { label: "产品 FAQ", href: "/help/faq" },
    { label: "选型对比", href: "/help/compare" },
    { label: "定价与充值", href: "/pricing" },
    { label: "免费桌面客户端", href: "/desktop" },
  ] as Array<{ label: string; href: string }>,
  backLabel: "全部集成",
};

export const integrationDocs: Record<IntegrationSlug, IntegrationDoc> = {
  mcp: {
    slug: "mcp",
    cardTitle: "MCP 接入总入口",
    cardDesc: "统一 MCP 端点、工作区工具目录、鉴权与边界——任何 MCP 客户端都从这里开始。",
    h1: "Context OS MCP 接入：把工作区知识库暴露给任意 MCP 客户端",
    subtitle: "集成承接 · MCP 总入口",
    intro: [
      "Context OS 为每个工作区提供统一的 MCP HTTP 端点：POST {api_base}/api/v1/mcp，走 JSON-RPC 2.0（initialize / tools/list / tools/call）。工作区密钥按 workspace 隔离，权限只有 index（入库）与 query（检索问答）。",
      "云端部署的 api_base 是 https://app.contextlm.top；本地桌面客户端运行期间，同一套 MCP / REST 接口面监听在 http://127.0.0.1:18080，工具与权限一致。",
    ],
    sections: [
      {
        h2: "连接参数（来自 Agent Pack）",
        paragraphs: [
          "在产品 UI 的工作区 Share → API Access 面板创建密钥后，点「Copy full agent pack」即得以下字段；把它们填进任意 MCP 客户端即可连通。",
        ],
        table: {
          headers: ["字段", "含义"],
          rows: [
            ["workspace_id", "目标工作区 UUID；每次 tools/call 都必须携带"],
            ["api_base", "API 的 HTTPS 源（云端或本地 http://127.0.0.1:18080）"],
            ["mcp_http", "{api_base}/api/v1/mcp"],
            ["api_key", "工作区密钥；以 Authorization: Bearer <api_key> 头发送"],
            ["docs_agent", "/help/api-access/agents（协议事实源）"],
          ],
        },
      },
      {
        h2: "工作区工具目录",
        paragraphs: ["以下工具对工作区密钥可用（权限列即所需 key 权限）："],
        table: {
          headers: ["工具", "权限", "用途"],
          rows: [
            ["workspace.rag_query", "query", "对库做检索问答（RAG）"],
            ["workspace.search_query", "query", "联网检索（原生工具）"],
            ["workspace.list_sources", "query", "列出库内资料源"],
            ["workspace.create_upload / complete_upload", "index", "文件上传入库（返回 upload_url 后 HTTP PUT）"],
            ["workspace.add_url_source", "index", "把 URL 加入资料源"],
            ["workspace.document_status", "index 或 query", "轮询解析状态直到 completed"],
          ],
        },
      },
      {
        h2: "调用示例",
        paragraphs: ["检索问答一次 tools/call 的报文形状如下（注意 arguments 必须带 workspace_id）："],
        code: {
          label: "POST /api/v1/mcp",
          text: '{\n  "jsonrpc": "2.0",\n  "id": "1",\n  "method": "tools/call",\n  "params": {\n    "name": "workspace.rag_query",\n    "arguments": {\n      "workspace_id": "<workspace_id>",\n      "query": "总结库里已入库的文档"\n    }\n  }\n}',
        },
      },
      {
        h2: "本地桌面：stdio 包装器",
        paragraphs: [
          "桌面客户端运行时提供 context-os-mcp（stdio → HTTP 网关转发，工具与权限与 HTTP 面一致），适合 Cursor、Claude Desktop 等 stdio MCP 客户端。相关环境变量：CONTEXT_OS_API_KEY（工作区密钥）、CONTEXT_OS_API_BASE（默认 http://127.0.0.1:18080）、CONTEXT_OS_WORKSPACE_ID。探测命令：context-os-mcp --check 或 context-os status。",
        ],
      },
      {
        h2: "边界与限制",
        bullets: [
          "密钥限定单工作区：跨工作区使用会返回 workspace_scope_mismatch。",
          "分享管理类工具（share_create_link、share_update_settings 等）需要用户会话；工作区密钥调用会收到 api_key_forbidden。",
          "建工作区在产品 UI 完成；agent 凭据不做账号级操作（账号类工具对 key 返回 workspace_key_cannot_call_org_tools）。",
          "旧路径 POST /mcp/workspaces/{workspace_id} 已弃用，应迁移到 /api/v1/mcp。",
        ],
      },
    ],
  },
  cursor: {
    slug: "cursor",
    cardTitle: "Cursor",
    cardDesc: "让 Cursor 里的 Agent 检索你的工作区知识库：本地 stdio 或云端 MCP HTTP 两条路径。",
    h1: "把 Context OS 知识库接进 Cursor（MCP）",
    subtitle: "集成承接 · Cursor",
    intro: [
      "目标：让 Cursor 里的 Agent 能对指定 Context OS 工作区做检索问答（workspace.rag_query / workspace.search_query），答案溯源到库内文档。",
      "两条连接路径：本地 stdio（桌面客户端运行时，配置即贴即用）与云端 MCP HTTP（无需常驻客户端）。两条路径的工具与权限一致。",
    ],
    sections: [
      {
        h2: "第一步：准备一个工作区密钥",
        paragraphs: [
          "免费注册并登录 app.contextlm.top，创建一个工作区并上传文件或粘贴 URL 完成入库。在工作区打开 Share → API Access，创建 API 密钥（默认含 index 与 query 权限），点「Copy full agent pack」。Pack 里有 workspace_id、api_base、mcp_http 和 api_key 四个关键值。",
        ],
      },
      {
        h2: "方式 A：本地 stdio（已在用桌面客户端时推荐）",
        paragraphs: [
          "桌面客户端运行期间在本机 18080 端口提供同一套 MCP 接口；用官方 stdio 包装器 context-os-mcp 接入即可。在 Cursor 的 MCP 设置（Settings → MCP & Tools → 新建 server，或直接编辑项目/全局 mcp.json）中添加：",
        ],
        code: {
          label: "mcp.json",
          text: '{\n  "mcpServers": {\n    "context-os": {\n      "command": "/path/to/context-os-mcp",\n      "env": {\n        "CONTEXT_OS_API_BASE": "http://127.0.0.1:18080",\n        "CONTEXT_OS_API_KEY": "<workspace_api_key>"\n      }\n    }\n  }\n}',
        },
      },
      {
        h2: "方式 B：云端 MCP HTTP（无需桌面客户端）",
        paragraphs: [
          "在 Cursor 的 MCP 设置里添加 remote server：URL 填 https://app.contextlm.top/api/v1/mcp，请求头加 Authorization: Bearer <api_key>（值来自 Agent Pack）。",
        ],
      },
      {
        h2: "让 Agent 每次调用带上 workspace_id",
        paragraphs: [
          "每个 tools/call 的 arguments 都需要 workspace_id（Agent Pack 里有）。把 Pack 内容粘贴给 Cursor，或把 workspace_id 写进项目规则 / 对话上下文，Agent 即可在调用时自动携带。",
        ],
      },
      {
        h2: "验证",
        paragraphs: [
          "让 Cursor 调用 workspace.list_sources 列出资料源；再问一个只有库内文档才能回答的问题，检查引用是否指向你的文档。若出现 api_key_forbidden 或 workspace_scope_mismatch，对照 Agent API 文档的常见错误表排查。",
        ],
      },
      {
        h2: "边界与限制",
        bullets: [
          "密钥只有单工作区的 index / query 权限；分享与密钥管理等用户会话工具对 key 不可用。",
          "免费档即可完成本页全流程；会员档位影响的是可分享名额，不影响 API 接入。",
        ],
      },
    ],
  },
  "claude-desktop": {
    slug: "claude-desktop",
    cardTitle: "Claude Desktop",
    cardDesc: "让 Claude 桌面端经 MCP 查询你的工作区：stdio 配置或远程连接器均可。",
    h1: "把 Context OS 知识库接进 Claude Desktop（MCP）",
    subtitle: "集成承接 · Claude Desktop",
    intro: [
      "目标：让 Claude 桌面端通过 MCP 查询指定 Context OS 工作区，回答基于库内文档并可溯源。",
      "两条连接路径：本地 stdio（claude_desktop_config.json 的 mcpServers）与远程 MCP 连接器（填 URL 与 Bearer 请求头）。两条路径最终都指向同一套工作区工具。",
    ],
    sections: [
      {
        h2: "第一步：准备一个工作区密钥",
        paragraphs: [
          "免费注册并登录 app.contextlm.top，创建工作区并入库资料。在工作区 Share → API Access 创建密钥（默认含 index 与 query），点「Copy full agent pack」拿到 workspace_id、api_base、mcp_http 与 api_key。",
        ],
      },
      {
        h2: "方式 A：本地 stdio（桌面客户端在运行时）",
        paragraphs: [
          "编辑 Claude Desktop 的配置文件 claude_desktop_config.json（Settings → Developer → Edit Config；菜单名以你的客户端版本为准），加入以下 mcpServers 配置（命令路径换成你本机的 context-os-mcp）：",
        ],
        code: {
          label: "claude_desktop_config.json",
          text: '{\n  "mcpServers": {\n    "context-os": {\n      "command": "/path/to/context-os-mcp",\n      "env": {\n        "CONTEXT_OS_API_BASE": "http://127.0.0.1:18080",\n        "CONTEXT_OS_API_KEY": "<workspace_api_key>"\n      }\n    }\n  }\n}',
        },
      },
      {
        h2: "方式 B：远程 MCP 连接器（无需本地客户端）",
        paragraphs: [
          "Claude Desktop / Web 的连接器（Custom Connectors）支持添加远程 MCP 服务器：URL 填 https://app.contextlm.top/api/v1/mcp，请求头加 Authorization: Bearer <api_key>。连接器入口的名称与位置随客户端版本略有差异，以你当前版本的设置为准。",
        ],
      },
      {
        h2: "让 Agent 每次调用带上 workspace_id",
        paragraphs: [
          "工具调用的 arguments 必须带 workspace_id。把 Agent Pack 内容粘贴进对话，或写入 CLAUDE.md / 项目记忆，Claude 即可在调用 workspace.rag_query 等工具时自动携带。",
        ],
      },
      {
        h2: "验证",
        paragraphs: [
          "先问「这个工作区里有哪些资料？」（应触发 workspace.list_sources）；再问一个只有库内文档才能回答的问题，核对回答是否引用你的文档。错误码含义见 Agent API 文档常见错误表。",
        ],
      },
      {
        h2: "边界与限制",
        bullets: [
          "方式 A 依赖桌面客户端处于运行状态（stdio 包装器转发到本机 18080 接口面）；不常驻客户端时用方式 B。",
          "密钥只有单工作区 index / query 权限；分享管理类调用需要用户会话，key 会收到 api_key_forbidden。",
        ],
      },
    ],
  },
};
