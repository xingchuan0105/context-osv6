/**
 * /help/faq 内容单一数据源（zh-CN / en）。
 * 页面组件与 FaqPageJsonLd 都从这里读取，避免两套文案漂移。
 * 英文为人工翻译（HCU：不机翻）；产品名 Context OS / 品牌 ContextLM 不译。
 */
export type FaqLocale = "zh-CN" | "en";

export type FaqItem = { question: string; answer: string };

export type FaqCopy = {
  subtitle: string;
  h1: string;
  authorPrefix: string;
  authorName: string;
  updatedLabel: string;
  evidencePrefix: string;
  evidenceSuffix: string;
  linkPricing: string;
  linkApiAccess: string;
  linkAgents: string;
  buttonCompare: string;
  buttonApiAccess: string;
  buttonPricing: string;
  buttonDesktop: string;
  evidenceTitle: string;
  evidencePricing: string;
  evidenceApiAccess: string;
  evidenceAgents: string;
  evidenceCompare: string;
  evidenceIntegrations: string;
  items: FaqItem[];
};

export const faqContent: Record<FaqLocale, FaqCopy> = {
  "zh-CN": {
    subtitle: "产品事实 FAQ",
    h1: "Context OS 常见问题",
    authorPrefix: "产品：Context OS · 品牌：ContextLM · 作者：",
    authorName: "邢川",
    updatedLabel: "页面说明更新日期：",
    evidencePrefix: "说明基于本站公开能力与定价文档（来源：",
    evidenceSuffix: "）。",
    linkPricing: "定价",
    linkApiAccess: "API 接入",
    linkAgents: "Agent 文档",
    buttonCompare: "选型对比",
    buttonApiAccess: "API 接入",
    buttonPricing: "定价",
    buttonDesktop: "免费客户端",
    evidenceTitle: "证据与进一步阅读",
    evidencePricing: "定价与充值",
    evidenceApiAccess: "API 访问（人类）",
    evidenceAgents: "Agent API 文档",
    evidenceCompare: "Context OS 选型对比",
    evidenceIntegrations: "集成承接（Cursor / Claude / MCP）",
    items: [
      {
        question: "Context OS 是什么？",
        answer:
          "Context OS 是可本地部署的个人 AI 知识库：把文档入库后可按库检索问答，也可把库开放给访客或外接 Agent（MCP / API）。产品名 Context OS，品牌为 ContextLM。",
      },
      {
        question: "AI 知识库是什么？",
        answer:
          "AI 知识库指把文档、网页、笔记等资料集中入库后，让模型「先查库、再回答」的系统：提问时先从库中找出最相关的片段，模型基于这些片段作答，因此答案可标注来源、限定在库内事实，而不是凭训练记忆泛泛而谈。Context OS 属于这一类产品：文档入库即成可检索的知识库，支持追问、溯源与外接 Agent 调用。",
      },
      {
        question: "RAG 知识库是什么、怎么搭建？",
        answer:
          "RAG（Retrieval-Augmented Generation，检索增强生成）是 AI 知识库的底层方法：检索层负责从文档中找出与问题相关的片段，生成层让模型基于这些片段回答，从而减少凭空编造。用 Context OS 搭建不需要自己写检索代码：上传文件或粘贴 URL 即完成入库，解析、索引与检索编排由系统自动完成；桌面客户端可本机私有使用。把库接入 Cursor、Claude 等外接 Agent 的做法见 Agent API 文档。",
      },
      {
        question: "MCP 工具是什么？",
        answer:
          "MCP（Model Context Protocol）是连接 AI 应用与外部数据源、工具的开放协议：MCP 服务器把知识库等资源以统一接口暴露给 Claude、Cursor 等 Agent 调用。Context OS 为每个工作区提供 MCP HTTP 接入——创建工作区密钥、复制 Agent Pack 即可让外接 Agent 检索该库，具体见下一条。",
      },
      {
        question: "如何用 MCP / 外接 Agent 接入？",
        answer:
          "在分享中心为工作区创建 API 密钥后，复制「完整接入包（Agent Pack）」粘贴到 Cursor / Claude 等客户端即可。包内含 workspace_id、api_base、mcp_http 与密钥用法。步骤与工具边界见 Agent API 文档；人类可读摘要见 API 访问。",
      },
      {
        question: "工作区密钥能做什么、不能做什么？",
        answer:
          "工作区 API 密钥按库作用域，面向资料管理与知识库查询（索引 / 查询类权限由创建时勾选）。聊天与网络搜索等能力不走该密钥默认路径。建库、分享治理等用户态操作需要用户登录或单独签发的 agent token，不能用工作区密钥代替。详见 Agent 文档中的 Authentication / Scope 章节。",
      },
      {
        question: "会员档位与可分享名额是什么关系？",
        answer:
          "会员主商品是「可同时开启分享的工作区数量」：Free 3 / Plus 10 / Pro 100。客户端与仅自己使用的私有工作区始终免费。升级只增加分享名额，不自动等于模型调用额度。",
      },
      {
        question: "余额充值与会员有何区别？",
        answer:
          "两者独立。余额用于平台模型调用、向量检索，以及分享页上由所有者承担的访客问答（Owner-pays）。可以只充值不升级，也可以两者都要。配置自定义 Provider（BYOK）后，最终回答走你自己的模型额度，从而减少平台对话扣费。Qwen3.7 Flash 作为快速模型，同时用于文档索引和检索子代理，并从余额扣费。",
      },
      {
        question: "什么是 BYOK？",
        answer:
          "BYOK（Bring Your Own Key）即在设置中配置自己的模型 Provider。最终回答走自有额度。Qwen3.7 Flash 作为快速模型，同时用于文档索引和检索子代理。入口：设置 · 模型 Provider（需登录）；说明见定价页相关提示。",
      },
      {
        question: "分享页访客问答谁付费？",
        answer:
          "分享开启后，访客在公开页上的问答成本由工作区所有者承担（Owner-pays），从所有者余额或 BYOK 策略中结算，而不是向访客单独收费。具体以定价页与账单说明为准。",
      },
      {
        question: "桌面客户端收费吗？",
        answer:
          "桌面客户端按产品叙事为免费使用；本机私有与本机 Agent 场景见客户端页。本机库对外分享需先发布到云端（向量导入、不重灌库），可分享名额仍受会员档位约束。",
      },
      {
        question: "和笔记 AI / 通用 RAG 怎么选？",
        answer:
          "若你需要「按工作区隔离的知识库 + 可分享 + 外接 Agent（MCP）」，优先看 Context OS。若你主要在单一笔记产品内写文档并使用其内置 AI，可能笔记套件更合适。中立对照表见选型对比。",
      },
    ],
  },
  en: {
    subtitle: "Product facts FAQ",
    h1: "Context OS FAQ",
    authorPrefix: "Product: Context OS · Brand: ContextLM · Author: ",
    authorName: "Xing Chuan",
    updatedLabel: "Page copy last updated: ",
    evidencePrefix: "Claims summarize public product capabilities (source: ",
    evidenceSuffix: ").",
    linkPricing: "Pricing",
    linkApiAccess: "API access",
    linkAgents: "Agent docs",
    buttonCompare: "Compare",
    buttonApiAccess: "API access",
    buttonPricing: "Pricing",
    buttonDesktop: "Free desktop client",
    evidenceTitle: "Evidence & further reading",
    evidencePricing: "Pricing & top-up",
    evidenceApiAccess: "API access (human)",
    evidenceAgents: "Agent API docs",
    evidenceCompare: "Context OS comparison",
    evidenceIntegrations: "Integrations (Cursor / Claude / MCP)",
    items: [
      {
        question: "What is Context OS?",
        answer:
          "Context OS is a locally deployable personal AI knowledge base: ingest documents, then query and chat against each library, or open a library to guests and external agents (MCP / API). The product is Context OS; the brand is ContextLM.",
      },
      {
        question: "What is an AI knowledge base?",
        answer:
          "An AI knowledge base ingests documents, web pages, notes, and other material, then has the model search first and answer second: each question first retrieves the most relevant passages from the library, and the model answers from those passages. Answers can therefore cite sources and stay grounded in library facts instead of relying on training memory. Context OS is this kind of product: documents become a searchable knowledge base on ingest, with follow-up questions, source tracing, and external agent access.",
      },
      {
        question: "What is a RAG knowledge base, and how do you build one?",
        answer:
          "RAG (Retrieval-Augmented Generation) is the method underneath AI knowledge bases: a retrieval layer finds passages relevant to the question, and a generation layer has the model answer from those passages, reducing fabrication. With Context OS you don't write retrieval code: upload files or paste URLs to ingest, and parsing, indexing, and retrieval orchestration happen automatically; the desktop client can run fully private on your machine. See the Agent API docs for connecting a library to Cursor, Claude, and other external agents.",
      },
      {
        question: "What is MCP?",
        answer:
          "MCP (Model Context Protocol) is an open protocol connecting AI applications to external data sources and tools: an MCP server exposes resources such as a knowledge base through a uniform interface for agents like Claude and Cursor. Context OS provides MCP HTTP access per workspace — create a workspace key and copy the Agent Pack to let an external agent query that library (see the next item).",
      },
      {
        question: "How do I connect via MCP / an external agent?",
        answer:
          "Create an API key for the workspace in the share center, then copy the full Agent Pack into Cursor, Claude, or another client. The pack contains workspace_id, api_base, mcp_http, and key usage. See the Agent API docs for steps and tool boundaries, or API access for a human-readable summary.",
      },
      {
        question: "What can a workspace key do — and not do?",
        answer:
          "A workspace API key is scoped per library, for material management and knowledge-base queries (indexing / query permissions are selected at creation). Chat and web search do not go through the key by default. User-level actions such as creating libraries and managing sharing require a signed-in user or a separately issued agent token — a workspace key cannot substitute. See Authentication / Scope in the Agent docs.",
      },
      {
        question: "How do membership tiers relate to share slots?",
        answer:
          "The membership product is the number of workspaces you can share at once: Free 3 / Plus 10 / Pro 100. The client and private, self-only workspaces are always free. Upgrading adds share slots; it does not automatically add model usage quota.",
      },
      {
        question: "What's the difference between wallet top-up and membership?",
        answer:
          "They are independent. The wallet pays for platform model calls, vector retrieval, and guest Q&A on shared pages billed to the owner (owner-pays). You can top up without upgrading, or do both. With a custom provider (BYOK), final answers use your own model quota, reducing platform chat charges. Qwen3.7 Flash is the fast model used for document indexing and the retrieval sub-agent, billed from the wallet.",
      },
      {
        question: "What is BYOK?",
        answer:
          "BYOK (Bring Your Own Key) means configuring your own model provider in settings; final answers then use your own quota. Qwen3.7 Flash is the fast model used for document indexing and the retrieval sub-agent. Entry: Settings · Model provider (sign-in required); see the pricing page for details.",
      },
      {
        question: "Who pays for guest Q&A on a shared page?",
        answer:
          "Once sharing is on, guest Q&A on the public page is billed to the workspace owner (owner-pays), settled from the owner's wallet or BYOK policy — guests are not charged separately. See the pricing page and billing notes for specifics.",
      },
      {
        question: "Is the desktop client free?",
        answer:
          "The desktop client is free per the product narrative; see the client page for local-private and local-agent scenarios. Sharing a local library externally requires publishing it to the cloud first (vector import, no re-ingest), and share slots still follow your membership tier.",
      },
      {
        question: "How do I choose between note AI / general RAG and Context OS?",
        answer:
          "If you need workspace-isolated knowledge bases plus sharing plus external agent access (MCP), look at Context OS first. If you mostly write inside a single notes product and use its built-in AI, a notes suite may fit better. See the comparison page for a neutral table.",
      },
    ],
  },
};
