import type { UiMessageDescriptor } from "./types";

export const homeMessages = {
  "home.seoTitle": {
    zh: "Context OS — 可分享的个人知识工作区",
    en: "Context OS — a shareable personal knowledge workspace",
  },
  "home.seoSubtitle": {
    zh: "把分散的文档变成可检索、可分享、可被 AI 引用的知识工作区：自己问答，也可以把库开放给访客或外接 Agent。",
    en: "Turn scattered documents into a searchable, shareable, AI-citable knowledge workspace — ask questions yourself, or open a workspace to guests and external agents.",
  },
  "home.seoBulletDocs": {
    zh: "文档入库与问答：上传文件或粘贴 URL 即成资料源；提问可按库限定检索范围，回答可溯源到具体文档。",
    en: "Docs in, answers out: upload files or paste URLs as sources; scope retrieval per workspace, with answers traceable to specific documents.",
  },
  "home.seoBulletAgents": {
    zh: "外接 Agent（MCP / API）：每个工作区可单独创建密钥，通过 MCP 或 HTTP API 把知识库接到 Cursor、Claude 等 Agent。",
    en: "External agents (MCP / API): mint per-workspace keys and connect the knowledge base to agents like Cursor or Claude over MCP or HTTP API.",
  },
  "home.seoBulletShare": {
    zh: "会员与分享名额：免费档即可建仓；升级会员获得更多可分享名额，访客免登录浏览公开库。",
    en: "Membership & share slots: start free; upgrade for more shareable slots so guests can browse public workspaces without signing in.",
  },
  "home.seoCtaEnter": {
    zh: "进入应用",
    en: "Open the app",
  },
  "home.seoCtaPricing": {
    zh: "查看定价",
    en: "View pricing",
  },
  "home.seoCtaAgents": {
    zh: "Agent 接入说明",
    en: "Agent access guide",
  },
} satisfies Record<string, UiMessageDescriptor>;
