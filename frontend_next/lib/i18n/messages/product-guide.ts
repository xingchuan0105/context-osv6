import type { UiMessageDescriptor } from "./types";

/**
 * Dashboard product map / first-run guide (wiki + Obsidian-style topic graph).
 */
export const productGuideMessages = {
  "productGuide.open": {
    zh: "上手",
    en: "Guide",
  },
  "productGuide.openHint": {
    zh: "产品说明与模块关系（非主导航）",
    en: "How the product fits together (not primary nav)",
  },
  "productGuide.openFull": {
    zh: "打开总览",
    en: "Open overview",
  },
  "productGuide.title": {
    zh: "上手 · 产品说明",
    en: "Guide · product map",
  },
  "productGuide.subtitle": {
    zh: "像知识图谱一样串联各模块：LLM、工作区、分享、客户端与计费。点左侧主题或下方链接即可跳转。",
    en: "Modules linked like a knowledge graph: LLM, workspaces, share, client, billing. Use the left topics or linked entries.",
  },
  "productGuide.fullHelp": {
    zh: "打开完整帮助页 →",
    en: "Full help page →",
  },
  "productGuide.nav.overview": {
    zh: "总览",
    en: "Overview",
  },
  "productGuide.nav.llm": {
    zh: "两种 LLM",
    en: "Two LLM paths",
  },
  "productGuide.nav.workspace": {
    zh: "工作区",
    en: "Workspaces",
  },
  "productGuide.nav.share": {
    zh: "分享",
    en: "Share",
  },
  "productGuide.nav.client": {
    zh: "客户端",
    en: "Client",
  },
  "productGuide.nav.billing": {
    zh: "会员与充值",
    en: "Plan & top-up",
  },
  "productGuide.nav.settings": {
    zh: "设置与数据",
    en: "Settings & data",
  },
  "productGuide.nav.graph": {
    zh: "相关入口",
    en: "Related links",
  },
  "productGuide.overview.body": {
    zh: "Context-OS 以工作区为中心：上传文档 → 索引 → 问答；需要对外时再开启分享。本地客户端可把同一能力接到桌面 Agent（MCP / CLI）。",
    en: "Workspaces are the hub: upload → index → Q&A; enable share when you publish. The desktop client exposes the same stack to agents via MCP / CLI.",
  },
  "productGuide.overview.step1": {
    zh: "在工作台新建或打开一个工作区",
    en: "Create or open a workspace on the dashboard",
  },
  "productGuide.overview.step2": {
    zh: "配置 LLM（BYOK 或平台额度）后上传文档",
    en: "Configure LLM (BYOK or platform) then upload docs",
  },
  "productGuide.overview.step3": {
    zh: "在对话里提问；需要对外时打开分享与访客问答",
    en: "Chat in the workspace; turn on share for public Q&A",
  },
  "productGuide.llm.title": {
    zh: "两种 LLM 方式",
    en: "Two ways to use LLMs",
  },
  "productGuide.llm.byokTitle": {
    zh: "BYOK · 自定义 Provider",
    en: "BYOK · custom provider",
  },
  "productGuide.llm.byokBody": {
    zh: "在设置 → 模型服务商中填入你自己的 API Key（如 DeepSeek / 百炼 / SiliconFlow）。对话走你自己的额度；适合已有账号的用户。",
    en: "Add your own API keys under Settings → Providers. Chat uses your quota — best if you already have keys.",
  },
  "productGuide.llm.platformTitle": {
    zh: "平台代购 · 余额扣费",
    en: "Platform models · wallet",
  },
  "productGuide.llm.platformBody": {
    zh: "使用平台模型时从余额扣费。访客在分享页提问时默认由工作区所有者承担。可在定价页直接充值。",
    en: "Platform models debit your wallet. Guest Q&A on shared pages is billed to the workspace owner. Top up on the pricing page.",
  },
  "productGuide.llm.linkProviders": {
    zh: "配置模型服务商",
    en: "Configure providers",
  },
  "productGuide.llm.linkBilling": {
    zh: "定价页充值",
    en: "Top up on pricing",
  },
  "productGuide.workspace.title": {
    zh: "工作区与文档",
    en: "Workspaces & documents",
  },
  "productGuide.workspace.body": {
    zh: "每个工作区是一套独立知识库：来源、笔记、对话与分享状态互不串线。卡片上的「来源数」表示已接入文档量。",
    en: "Each workspace is an isolated knowledge base: sources, notes, chat, and share state do not cross. The source count on a card is how many docs are attached.",
  },
  "productGuide.workspace.link": {
    zh: "回到工作台",
    en: "Back to dashboard",
  },
  "productGuide.share.title": {
    zh: "分享 · 设置 · 场景",
    en: "Share · settings · use cases",
  },
  "productGuide.share.body": {
    zh: "在工作区开启分享后，访客可浏览或提问。档位限制的是「可同时开启分享的工作区数量」（Free 3 / Plus 10 / Pro 100），不是工作区总数。数据管理、访客与趋势见工作区内分享中心与数据分析。",
    en: "Enable share so guests can browse or ask. Plans limit how many workspaces can be shared at once (Free 3 / Plus 10 / Pro 100), not total workspaces. Manage data, visitors, and trends in Share center and Analytics.",
  },
  "productGuide.share.useCases": {
    zh: "常见场景：对外知识库、团队说明页、产品 FAQ、研究笔记公开副本。",
    en: "Common uses: public KB, team handbooks, product FAQ, public research notes.",
  },
  "productGuide.share.linkAnalytics": {
    zh: "分享数据分析",
    en: "Share analytics",
  },
  "productGuide.client.title": {
    zh: "本地客户端",
    en: "Desktop client",
  },
  "productGuide.client.body": {
    zh: "客户端完全免费：数据默认可留在本机，支持 MCP / CLI，可被 Claude Code、Codex 等桌面 Agent 调用。需要上云分享时再使用云端名额。",
    en: "The client is free: data can stay local, with MCP / CLI for desktop agents (Claude Code, Codex, …). Use cloud share slots only when you publish online.",
  },
  "productGuide.client.link": {
    zh: "打开客户端介绍页",
    en: "Open client page",
  },
  "productGuide.billing.title": {
    zh: "会员是干什么的 · 充值是干什么的",
    en: "What membership vs top-up does",
  },
  "productGuide.billing.memberTitle": {
    zh: "会员（Free / Plus / Pro）",
    en: "Membership (Free / Plus / Pro)",
  },
  "productGuide.billing.memberBody": {
    zh: "主商品是可分享工作区名额。客户端与仅自己使用的工作区不收费。升级会员 = 能同时对外分享更多库。",
    en: "Primary product is shareable workspace slots. Client and private workspaces stay free. Upgrading means more concurrent public shares.",
  },
  "productGuide.billing.topupTitle": {
    zh: "充值（模型调用余额）",
    en: "Top-up (model wallet)",
  },
  "productGuide.billing.topupBody": {
    zh: "余额用于平台模型调用与向量检索等计量能力。与会员档位独立：在定价页选套餐包即可充值，也可以只升级不充值。",
    en: "Wallet pays platform model calls and metered retrieval. Independent of plan — top up with packs on the pricing page, or upgrade only.",
  },
  "productGuide.billing.linkPricing": {
    zh: "查看定价与充值说明",
    en: "Pricing & top-up",
  },
  "productGuide.settings.title": {
    zh: "设置与数据",
    en: "Settings & data",
  },
  "productGuide.settings.body": {
    zh: "账户、偏好、账单、安全与通知集中在设置。模型服务商、用量与邀请奖励也在这里管理。",
    en: "Account, preferences, billing, security, and notifications live in Settings — plus providers, usage, and referrals.",
  },
  "productGuide.settings.link": {
    zh: "打开设置",
    en: "Open settings",
  },
  "productGuide.graph.title": {
    zh: "相关入口（图谱）",
    en: "Related entries (graph)",
  },
  "productGuide.graph.hint": {
    zh: "模块之间互相引用；从任一入口继续深入。",
    en: "Modules cross-link; continue from any entry.",
  },
  "productGuide.graph.help": {
    zh: "产品帮助",
    en: "Product help",
  },
  "productGuide.graph.api": {
    zh: "API / Agent 接入",
    en: "API / agent access",
  },
  "productGuide.graph.pricing": {
    zh: "定价",
    en: "Pricing",
  },
  "productGuide.graph.desktop": {
    zh: "客户端",
    en: "Client",
  },
  "productGuide.graph.analytics": {
    zh: "数据分析",
    en: "Analytics",
  },
  "productGuide.graph.providers": {
    zh: "模型服务商",
    en: "Providers",
  },
  "productGuide.graph.billing": {
    zh: "充值",
    en: "Top-up",
  },
} satisfies Record<string, UiMessageDescriptor>;
