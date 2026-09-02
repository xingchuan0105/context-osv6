/**
 * /help/compare 内容单一数据源（zh-CN / en）。
 * 页面组件从这里读取，避免两套文案漂移。英文为人工翻译（HCU：不机翻）。
 */
export type CompareLocale = "zh-CN" | "en";

export type CompareRow = { dim: string; cos: string; notes: string; rag: string };

export type CompareCopy = {
  subtitle: string;
  h1: string;
  authorPrefix: string;
  authorName: string;
  updatedLabel: string;
  evidenceText: string;
  linkPricing: string;
  linkAgents: string;
  linkFaq: string;
  buttonFaq: string;
  buttonAgents: string;
  buttonPricing: string;
  buttonDesktop: string;
  positioningTitle: string;
  positioning: Array<{ label: string; text: string }>;
  tableTitle: string;
  tableHeaders: { dim: string; cos: string; notes: string; rag: string };
  rows: CompareRow[];
  fitsTitle: string;
  fits: string[];
  otherTitle: string;
  other: string[];
  noClaimTitle: string;
  noClaim: string;
  nextTitle: string;
  nextFaq: string;
  nextAgents: string;
  nextApiAccess: string;
  nextPricing: string;
  nextIntegrations: string;
  nextIntegrationsLabel: string;
  nextEnter: string;
  nextEnterLabel: string;
  nextRegister: string;
};

export const compareContent: Record<CompareLocale, CompareCopy> = {
  "zh-CN": {
    subtitle: "中立选型对照",
    h1: "AI 知识库工具对比：Context OS 与其它知识方案怎么选",
    authorPrefix: "产品：Context OS · 品牌：ContextLM · 作者：",
    authorName: "邢川",
    updatedLabel: "页面说明更新日期：",
    evidenceText:
      "下表描述产品形态差异，不引用未核验的竞品流量、排名或价格数字（方法：对照本站公开定价与 API 文档，竞品侧仅写常见产品类别特征）。来源：",
    linkPricing: "定价",
    linkAgents: "Agent 文档",
    linkFaq: "FAQ",
    buttonFaq: "常见问题",
    buttonAgents: "Agent 接入",
    buttonPricing: "定价",
    buttonDesktop: "免费客户端",
    positioningTitle: "一句话定位",
    positioning: [
      {
        label: "Context OS",
        text: "把个人/小团队知识收成「可检索、可分享、可被外接 Agent 调用」的工作区。",
      },
      {
        label: "笔记内置 AI / 第二大脑类应用",
        text: "写作与整理体验优先，AI 服务文档工作流。",
      },
      {
        label: "通用 RAG 套件 / 自建栈",
        text: "最大灵活度，工程与运维成本也最高。",
      },
    ],
    tableTitle: "能力对照（类别级，非单品跑分）",
    tableHeaders: {
      dim: "维度",
      cos: "Context OS",
      notes: "笔记 AI / 第二大脑类",
      rag: "通用 RAG / 自建",
    },
    rows: [
      {
        dim: "核心对象",
        cos: "工作区（Workspace）为产品真相；来源 / 笔记 / 对话挂在库上",
        notes: "页面或笔记本为中心；AI 多为文档内嵌能力",
        rag: "管道 / 索引 / 应用代码为中心",
      },
      {
        dim: "入库与问答",
        cos: "上传文件或 URL 成资料源；按库检索，回答可溯源到文档",
        notes: "写作与整理强；跨库检索深度因产品而异",
        rag: "可自建任意检索栈；需自运维与调参",
      },
      {
        dim: "对外分享",
        cos: "库级分享；访客问答由所有者付费（Owner-pays）；会员控制可分享名额",
        notes: "常见为页面/空间协作；知识库「访客问答 + 所有者计费」模型不一定等同",
        rag: "需自建鉴权、配额与计费",
      },
      {
        dim: "外接 Agent",
        cos: "一等能力：工作区密钥 + MCP HTTP + Agent Pack 一次复制接入",
        notes: "部分产品提供 API/插件；MCP 工作区密钥路径并非默认主叙事",
        rag: "完全可定制；集成成本由团队承担",
      },
      {
        dim: "模型与费用",
        cos: "会员（分享名额）与余额（平台模型/检索）分离；支持 BYOK",
        notes: "多为套餐或席位；模型计费因厂商而异",
        rag: "基础设施 + 模型账单自理",
      },
      {
        dim: "客户端",
        cos: "桌面客户端按产品叙事免费；本机私有与本机 Agent 场景见客户端页",
        notes: "通常与笔记编辑体验绑定",
        rag: "自选部署形态",
      },
    ],
    fitsTitle: "更适合选 Context OS 的情况",
    fits: [
      "知识要以「工作区」为单位隔离，并可能对访客开放问答。",
      "希望 Cursor / Claude 等外接 Agent 通过 MCP 读同一库，而不是只在笔记 UI 内聊。",
      "接受「会员管分享名额、余额管模型」的拆分，而不是只要一个写笔记席位。",
      "需要公开可复制的 Agent Pack 与工作区密钥边界（见 Agent 文档）。",
    ],
    otherTitle: "可能更适合其它方案的情况",
    other: [
      "日常主战场是长文写作、块编辑与团队 wiki，且 AI 仅作写作辅助。",
      "必须深度定制检索、重排、权限与多租户计费，并有工程团队维护。",
      "不需要对外分享或 Agent 接入，只想在单一笔记应用内完结。",
    ],
    noClaimTitle: "不做什么声明",
    noClaim:
      "本页不声称 Context OS「全面优于」任一具体竞品，也不给出未核验的市场份额、延迟或准确率数字。竞品能力以各厂商当前文档为准；若你评估具体产品，请以其官方说明为证据。",
    nextTitle: "下一步",
    nextFaq: "产品事实问答：",
    nextAgents: "外接 Agent：",
    nextApiAccess: "人类接入说明",
    nextPricing: "名额与余额：",
    nextIntegrations: "把知识库接进 Cursor / Claude 等工具：",
    nextIntegrationsLabel: "集成承接",
    nextEnter: "进入产品：",
    nextEnterLabel: "应用入口",
    nextRegister: "注册",
  },
  en: {
    subtitle: "Neutral comparison",
    h1: "AI knowledge base comparison: Context OS vs other knowledge approaches",
    authorPrefix: "Product: Context OS · Brand: ContextLM · Author: ",
    authorName: "Xing Chuan",
    updatedLabel: "Page copy last updated: ",
    evidenceText:
      "This table describes product-shape differences and cites no unverified competitor traffic, ranking, or price figures (method: cross-check our public pricing and API docs; competitor columns describe common product-category traits only). Sources: ",
    linkPricing: "Pricing",
    linkAgents: "Agent docs",
    linkFaq: "FAQ",
    buttonFaq: "FAQ",
    buttonAgents: "Agent access",
    buttonPricing: "Pricing",
    buttonDesktop: "Free desktop client",
    positioningTitle: "One-line positioning",
    positioning: [
      {
        label: "Context OS",
        text: "Turns personal / small-team knowledge into workspaces that are searchable, shareable, and callable by external agents.",
      },
      {
        label: "Notes AI / second-brain apps",
        text: "Writing and organizing first; AI serves the document workflow.",
      },
      {
        label: "General RAG / self-built stacks",
        text: "Maximum flexibility, with the highest engineering and ops cost.",
      },
    ],
    tableTitle: "Capability comparison (category-level, not per-product scores)",
    tableHeaders: {
      dim: "Dimension",
      cos: "Context OS",
      notes: "Notes AI / second brain",
      rag: "General RAG / self-built",
    },
    rows: [
      {
        dim: "Core object",
        cos: "Workspace is the product's source of truth; sources / notes / chats hang off the library",
        notes: "Page or notebook centered; AI is mostly an in-document capability",
        rag: "Pipelines / indexes / app code centered",
      },
      {
        dim: "Ingest & Q&A",
        cos: "Upload files or URLs as sources; per-library retrieval with answers traceable to documents",
        notes: "Strong writing and organizing; cross-library retrieval depth varies by product",
        rag: "Build any retrieval stack; you own ops and tuning",
      },
      {
        dim: "External sharing",
        cos: "Library-level sharing; guest Q&A billed to the owner (owner-pays); membership controls share slots",
        notes: "Usually page/space collaboration; the guest-Q&A + owner-billing model may not be equivalent",
        rag: "Build your own auth, quotas, and billing",
      },
      {
        dim: "External agents",
        cos: "First-class: workspace key + MCP HTTP + Agent Pack, copy once to connect",
        notes: "Some products offer APIs/plugins; a workspace-key MCP path is not their default story",
        rag: "Fully customizable; integration cost is on your team",
      },
      {
        dim: "Models & cost",
        cos: "Membership (share slots) is separate from wallet (platform models/retrieval); BYOK supported",
        notes: "Mostly plans or seats; model billing varies by vendor",
        rag: "Infrastructure + model bills are yours",
      },
      {
        dim: "Client",
        cos: "Desktop client is free per the product narrative; see the client page for local-private and local-agent scenarios",
        notes: "Usually tied to the notes editing experience",
        rag: "Choose your own deployment",
      },
    ],
    fitsTitle: "When Context OS fits better",
    fits: [
      "Knowledge must be isolated per workspace and may be opened to guest Q&A.",
      "You want external agents like Cursor / Claude to read the same library over MCP, not just chat inside a notes UI.",
      "You accept the split of membership for share slots and wallet for models, rather than a single notes seat.",
      "You need a publicly copyable Agent Pack and workspace-key boundaries (see Agent docs).",
    ],
    otherTitle: "When other options may fit better",
    other: [
      "Your main work is long-form writing, block editing, and team wikis, with AI only as a writing aid.",
      "You must deeply customize retrieval, reranking, permissions, and multi-tenant billing, and have an engineering team to maintain it.",
      "You don't need external sharing or agent access and want everything inside a single notes app.",
    ],
    noClaimTitle: "What we don't claim",
    noClaim:
      "This page does not claim Context OS is 'better overall' than any specific competitor, and gives no unverified market-share, latency, or accuracy figures. Competitor capabilities follow each vendor's current documentation; if you evaluate a specific product, use its official docs as evidence.",
    nextTitle: "Next steps",
    nextFaq: "Product facts: ",
    nextAgents: "External agents: ",
    nextApiAccess: "Human access guide",
    nextPricing: "Slots & wallet: ",
    nextIntegrations: "Connect the library to Cursor / Claude and other tools: ",
    nextIntegrationsLabel: "Integrations",
    nextEnter: "Enter the product: ",
    nextEnterLabel: "Enter app",
    nextRegister: "Sign up",
  },
};
