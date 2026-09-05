/// 公开页内容单一数据源（对齐 `frontend_next/lib/content/*` 与 i18n zh 文案）。
/// E4.1 仅服务 zh-CN 路由；en 内容随 E4.4 `/en/*` 切片并入。
/// 不在本文件发明新声明：接入事实源是 `assets/docs/api-access-for-agents.md`。

pub const AGENT_DOC_MD: &str = include_str!("../../../../../assets/docs/api-access-for-agents.md");

pub const FAQ_UPDATED_LINE: &str = "页面说明更新日期：2026-08-29";
pub const COMPARE_UPDATED_LINE: &str = "页面说明更新日期：2026-08-29";
pub const INTEGRATIONS_UPDATED: &str = "2026-09-02";
pub const API_ACCESS_UPDATED: &str = "2026-08-12";
pub const AGENT_DOC_UPDATED: &str = "2026-08-12";

/// 品牌署名行（i18n `home.seoPublisher`）。hub 作者外链依赖营销站 origin，E4.1 先渲染纯文本。
pub const AUTHOR_LINE: &str = "产品：Context OS · 品牌：ContextLM · 作者：邢川";

// ---------- /help/faq ----------

pub struct FaqItem {
    pub question: &'static str,
    pub answer: &'static str,
}

pub struct FaqCopy {
    pub subtitle: &'static str,
    pub h1: &'static str,
    pub updated_label: &'static str,
    pub evidence_prefix: &'static str,
    pub evidence_suffix: &'static str,
    pub link_pricing: &'static str,
    pub link_api_access: &'static str,
    pub link_agents: &'static str,
    pub button_compare: &'static str,
    pub button_api_access: &'static str,
    pub button_pricing: &'static str,
    pub button_desktop: &'static str,
    pub evidence_title: &'static str,
    pub evidence_pricing: &'static str,
    pub evidence_api_access: &'static str,
    pub evidence_agents: &'static str,
    pub evidence_compare: &'static str,
    pub evidence_integrations: &'static str,
    pub items: &'static [FaqItem],
}

pub const FAQ: FaqCopy = FaqCopy {
    subtitle: "产品事实 FAQ",
    h1: "Context OS 常见问题",
    updated_label: "页面说明更新日期：",
    evidence_prefix: "说明基于本站公开能力与定价文档（来源：",
    evidence_suffix: "）。",
    link_pricing: "定价",
    link_api_access: "API 接入",
    link_agents: "Agent 文档",
    button_compare: "选型对比",
    button_api_access: "API 接入",
    button_pricing: "定价",
    button_desktop: "免费客户端",
    evidence_title: "证据与进一步阅读",
    evidence_pricing: "定价与充值",
    evidence_api_access: "API 访问（人类）",
    evidence_agents: "Agent API 文档",
    evidence_compare: "Context OS 选型对比",
    evidence_integrations: "集成承接（Cursor / Claude / MCP）",
    items: &[
        FaqItem {
            question: "Context OS 是什么？",
            answer: "Context OS 是可本地部署的个人 AI 知识库：把文档入库后可按库检索问答，也可把库开放给访客或外接 Agent（MCP / API）。产品名 Context OS，品牌为 ContextLM。",
        },
        FaqItem {
            question: "AI 知识库是什么？",
            answer: "AI 知识库指把文档、网页、笔记等资料集中入库后，让模型「先查库、再回答」的系统：提问时先从库中找出最相关的片段，模型基于这些片段作答，因此答案可标注来源、限定在库内事实，而不是凭训练记忆泛泛而谈。Context OS 属于这一类产品：文档入库即成可检索的知识库，支持追问、溯源与外接 Agent 调用。",
        },
        FaqItem {
            question: "RAG 知识库是什么、怎么搭建？",
            answer: "RAG（Retrieval-Augmented Generation，检索增强生成）是 AI 知识库的底层方法：检索层负责从文档中找出与问题相关的片段，生成层让模型基于这些片段回答，从而减少凭空编造。用 Context OS 搭建不需要自己写检索代码：上传文件或粘贴 URL 即完成入库，解析、索引与检索编排由系统自动完成；桌面客户端可本机私有使用。把库接入 Cursor、Claude 等外接 Agent 的做法见 Agent API 文档。",
        },
        FaqItem {
            question: "MCP 工具是什么？",
            answer: "MCP（Model Context Protocol）是连接 AI 应用与外部数据源、工具的开放协议：MCP 服务器把知识库等资源以统一接口暴露给 Claude、Cursor 等 Agent 调用。Context OS 为每个工作区提供 MCP HTTP 接入——创建工作区密钥、复制 Agent Pack 即可让外接 Agent 检索该库，具体见下一条。",
        },
        FaqItem {
            question: "如何用 MCP / 外接 Agent 接入？",
            answer: "在分享中心为工作区创建 API 密钥后，复制「完整接入包（Agent Pack）」粘贴到 Cursor / Claude 等客户端即可。包内含 workspace_id、api_base、mcp_http 与密钥用法。步骤与工具边界见 Agent API 文档；人类可读摘要见 API 访问。",
        },
        FaqItem {
            question: "工作区密钥能做什么、不能做什么？",
            answer: "工作区 API 密钥按库作用域，面向资料管理与知识库查询（索引 / 查询类权限由创建时勾选）。聊天与网络搜索等能力不走该密钥默认路径。建库、分享治理等用户态操作需要用户登录或单独签发的 agent token，不能用工作区密钥代替。详见 Agent 文档中的 Authentication / Scope 章节。",
        },
        FaqItem {
            question: "会员档位与可分享名额是什么关系？",
            answer: "会员主商品是「可同时开启分享的工作区数量」：Free 3 / Plus 10 / Pro 100。客户端与仅自己使用的私有工作区始终免费。升级只增加分享名额，不自动等于模型调用额度。",
        },
        FaqItem {
            question: "余额充值与会员有何区别？",
            answer: "两者独立。余额用于平台模型调用、向量检索，以及分享页上由所有者承担的访客问答（Owner-pays）。可以只充值不升级，也可以两者都要。配置自定义 Provider（BYOK）后，最终回答走你自己的模型额度，从而减少平台对话扣费。Qwen3.7 Flash 作为快速模型，同时用于文档索引和检索子代理，并从余额扣费。",
        },
        FaqItem {
            question: "什么是 BYOK？",
            answer: "BYOK（Bring Your Own Key）即在设置中配置自己的模型 Provider。最终回答走自有额度。Qwen3.7 Flash 作为快速模型，同时用于文档索引和检索子代理。入口：设置 · 模型 Provider（需登录）；说明见定价页相关提示。",
        },
        FaqItem {
            question: "分享页访客问答谁付费？",
            answer: "分享开启后，访客在公开页上的问答成本由工作区所有者承担（Owner-pays），从所有者余额或 BYOK 策略中结算，而不是向访客单独收费。具体以定价页与账单说明为准。",
        },
        FaqItem {
            question: "桌面客户端收费吗？",
            answer: "桌面客户端按产品叙事为免费使用；本机私有与本机 Agent 场景见客户端页。本机库对外分享需先发布到云端（向量导入、不重灌库），可分享名额仍受会员档位约束。",
        },
        FaqItem {
            question: "和笔记 AI / 通用 RAG 怎么选？",
            answer: "若你需要「按工作区隔离的知识库 + 可分享 + 外接 Agent（MCP）」，优先看 Context OS。若你主要在单一笔记产品内写文档并使用其内置 AI，可能笔记套件更合适。中立对照表见选型对比。",
        },
    ],
};

// ---------- /help/compare ----------

pub struct CompareRow {
    pub dim: &'static str,
    pub cos: &'static str,
    pub notes: &'static str,
    pub rag: &'static str,
}

pub struct ComparePositioning {
    pub label: &'static str,
    pub text: &'static str,
}

pub struct CompareCopy {
    pub subtitle: &'static str,
    pub h1: &'static str,
    pub updated_label: &'static str,
    pub evidence_text: &'static str,
    pub link_pricing: &'static str,
    pub link_agents: &'static str,
    pub link_faq: &'static str,
    pub button_faq: &'static str,
    pub button_agents: &'static str,
    pub button_pricing: &'static str,
    pub button_desktop: &'static str,
    pub positioning_title: &'static str,
    pub positioning: &'static [ComparePositioning],
    pub table_title: &'static str,
    pub table_headers: &'static [&'static str],
    pub rows: &'static [CompareRow],
    pub fits_title: &'static str,
    pub fits: &'static [&'static str],
    pub other_title: &'static str,
    pub other: &'static [&'static str],
    pub no_claim_title: &'static str,
    pub no_claim: &'static str,
    pub next_title: &'static str,
    pub next_faq: &'static str,
    pub next_agents: &'static str,
    pub next_api_access: &'static str,
    pub next_pricing: &'static str,
    pub next_integrations: &'static str,
    pub next_integrations_label: &'static str,
    pub next_enter: &'static str,
    pub next_enter_label: &'static str,
    pub next_register: &'static str,
}

pub const COMPARE: CompareCopy = CompareCopy {
    subtitle: "中立选型对照",
    h1: "AI 知识库工具对比：Context OS 与其它知识方案怎么选",
    updated_label: "页面说明更新日期：",
    evidence_text: "下表描述产品形态差异，不引用未核验的竞品流量、排名或价格数字（方法：对照本站公开定价与 API 文档，竞品侧仅写常见产品类别特征）。来源：",
    link_pricing: "定价",
    link_agents: "Agent 文档",
    link_faq: "FAQ",
    button_faq: "常见问题",
    button_agents: "Agent 接入",
    button_pricing: "定价",
    button_desktop: "免费客户端",
    positioning_title: "一句话定位",
    positioning: &[
        ComparePositioning {
            label: "Context OS",
            text: "把个人/小团队知识收成「可检索、可分享、可被外接 Agent 调用」的工作区。",
        },
        ComparePositioning {
            label: "笔记内置 AI / 第二大脑类应用",
            text: "写作与整理体验优先，AI 服务文档工作流。",
        },
        ComparePositioning {
            label: "通用 RAG 套件 / 自建栈",
            text: "最大灵活度，工程与运维成本也最高。",
        },
    ],
    table_title: "能力对照（类别级，非单品跑分）",
    table_headers: &["维度", "Context OS", "笔记 AI / 第二大脑类", "通用 RAG / 自建"],
    rows: &[
        CompareRow {
            dim: "核心对象",
            cos: "工作区（Workspace）为产品真相；来源 / 笔记 / 对话挂在库上",
            notes: "页面或笔记本为中心；AI 多为文档内嵌能力",
            rag: "管道 / 索引 / 应用代码为中心",
        },
        CompareRow {
            dim: "入库与问答",
            cos: "上传文件或 URL 成资料源；按库检索，回答可溯源到文档",
            notes: "写作与整理强；跨库检索深度因产品而异",
            rag: "可自建任意检索栈；需自运维与调参",
        },
        CompareRow {
            dim: "对外分享",
            cos: "库级分享；访客问答由所有者付费（Owner-pays）；会员控制可分享名额",
            notes: "常见为页面/空间协作；知识库「访客问答 + 所有者计费」模型不一定等同",
            rag: "需自建鉴权、配额与计费",
        },
        CompareRow {
            dim: "外接 Agent",
            cos: "一等能力：工作区密钥 + MCP HTTP + Agent Pack 一次复制接入",
            notes: "部分产品提供 API/插件；MCP 工作区密钥路径并非默认主叙事",
            rag: "完全可定制；集成成本由团队承担",
        },
        CompareRow {
            dim: "模型与费用",
            cos: "会员（分享名额）与余额（平台模型/检索）分离；支持 BYOK",
            notes: "多为套餐或席位；模型计费因厂商而异",
            rag: "基础设施 + 模型账单自理",
        },
        CompareRow {
            dim: "客户端",
            cos: "桌面客户端按产品叙事免费；本机私有与本机 Agent 场景见客户端页",
            notes: "通常与笔记编辑体验绑定",
            rag: "自选部署形态",
        },
    ],
    fits_title: "更适合选 Context OS 的情况",
    fits: &[
        "知识要以「工作区」为单位隔离，并可能对访客开放问答。",
        "希望 Cursor / Claude 等外接 Agent 通过 MCP 读同一库，而不是只在笔记 UI 内聊。",
        "接受「会员管分享名额、余额管模型」的拆分，而不是只要一个写笔记席位。",
        "需要公开可复制的 Agent Pack 与工作区密钥边界（见 Agent 文档）。",
    ],
    other_title: "可能更适合其它方案的情况",
    other: &[
        "日常主战场是长文写作、块编辑与团队 wiki，且 AI 仅作写作辅助。",
        "必须深度定制检索、重排、权限与多租户计费，并有工程团队维护。",
        "不需要对外分享或 Agent 接入，只想在单一笔记应用内完结。",
    ],
    no_claim_title: "不做什么声明",
    no_claim: "本页不声称 Context OS「全面优于」任一具体竞品，也不给出未核验的市场份额、延迟或准确率数字。竞品能力以各厂商当前文档为准；若你评估具体产品，请以其官方说明为证据。",
    next_title: "下一步",
    next_faq: "产品事实问答：",
    next_agents: "外接 Agent：",
    next_api_access: "人类接入说明",
    next_pricing: "名额与余额：",
    next_integrations: "把知识库接进 Cursor / Claude 等工具：",
    next_integrations_label: "集成承接",
    next_enter: "进入产品：",
    next_enter_label: "应用入口",
    next_register: "注册",
};

// ---------- /help/api-access ----------

pub struct ApiAccessCopy {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub updated: &'static str,
    pub evidence_line: &'static str,
    pub back_help: &'static str,
    pub agent_docs: &'static str,
    pub cta_faq: &'static str,
    pub cta_compare: &'static str,
    pub cta_integrations: &'static str,
    pub overview_title: &'static str,
    pub overview_items: &'static [&'static str],
    pub automation_title: &'static str,
    pub automation_body: &'static str,
    pub automation_steps: &'static [&'static str],
}

pub const API_ACCESS: ApiAccessCopy = ApiAccessCopy {
    title: "API 访问",
    subtitle: "面向个人用户的 API 接入说明。每个工作区单独管理密钥；自动化代理请使用 agent 文档。",
    updated: "页面说明更新日期：2026-08-12",
    evidence_line: "说明基于本站公开能力与定价文档（来源：帮助 / 定价页）。",
    back_help: "返回帮助中心",
    agent_docs: "打开 Agent API 文档",
    cta_faq: "FAQ",
    cta_compare: "选型对比",
    cta_integrations: "集成承接",
    overview_title: "你会在这里找到",
    overview_items: &[
        "每个工作区可以单独创建和撤销 API 密钥。",
        "当前 API Access 页面提供权限、速率限制和一次性明文 key 展示。",
        "工作区 API 密钥只用于该工作区的资料上传、URL 导入和知识库查询；先在应用里创建工作区，再在此页创建密钥。",
    ],
    automation_title: "需要自动化时",
    automation_body: "脚本与 coding agent 请用工作区 API 密钥调用 MCP。本机客户端可用 context-os-mcp（stdio）转发到 127.0.0.1:18080；也可用 HTTP POST /api/v1/mcp。配置片段在工作区 API Access「给 Agent 用」。",
    automation_steps: &[
        "在产品 UI 创建 Workspace，打开该工作区的 API Access，创建带 index/query 的密钥。",
        "本机客户端：构建或 stage context-os（含 context-os-mcp），设置 CONTEXT_OS_API_KEY 后运行 context-os status。",
        "Agent 用 stdio MCP（command = context-os-mcp）；脚本可用 context-os ingest/ask；工具参数带上 workspace_id。",
        "分享、成员与密钥管理仍只在用户会话 UI 中完成。",
    ],
};

// ---------- /integrations ----------

pub struct IntegrationTable {
    pub headers: &'static [&'static str],
    pub rows: &'static [&'static [&'static str]],
}

pub struct IntegrationSection {
    pub h2: &'static str,
    pub paragraphs: &'static [&'static str],
    pub bullets: &'static [&'static str],
    pub code: Option<(&'static str, &'static str)>,
    pub table: Option<IntegrationTable>,
}

pub struct IntegrationDoc {
    pub slug: &'static str,
    pub card_title: &'static str,
    pub card_desc: &'static str,
    pub h1: &'static str,
    pub subtitle: &'static str,
    pub intro: &'static [&'static str],
    pub sections: &'static [IntegrationSection],
}

pub struct IntegrationsShared {
    pub index_subtitle: &'static str,
    pub index_h1: &'static str,
    pub index_intro: &'static [&'static str],
    pub index_evidence_line: &'static str,
    pub updated_label: &'static str,
    pub author_line: &'static str,
    pub button_agents: &'static str,
    pub button_desktop: &'static str,
    pub button_pricing: &'static str,
    pub evidence_title: &'static str,
    pub evidence_items: &'static [(&'static str, &'static str)],
    pub back_label: &'static str,
}

pub const INTEGRATIONS_SHARED: IntegrationsShared = IntegrationsShared {
    index_subtitle: "集成承接",
    index_h1: "把 Context OS 知识库接进你的 AI 工具",
    index_intro: &[
        "Context OS 为每个工作区提供独立的 MCP HTTP 端点与工作区密钥：一个密钥只作用于一个工作区，权限只有 index（入库）与 query（检索问答）两种。下面按客户端给出可复现的接入步骤。",
        "接入协议（端点、工具目录、错误码）的单一事实源是 Agent API 文档；本文只做需求承接与步骤引导，若页面与文档不一致，以文档为准。",
        "不确定从哪页开始：用 Cursor 写代码选 Cursor 页；用 Claude 桌面端选 Claude Desktop 页；其他 MCP 客户端或想直接看协议细节，从 MCP 总入口进入。",
    ],
    index_evidence_line: "本页为集成承接导航；声明的来源与方法：步骤、端点与错误码均锚定 Agent API 文档（/help/api-access/agents）并对照公开定价页逐条核对。",
    updated_label: "页面说明更新日期：",
    author_line: AUTHOR_LINE,
    button_agents: "Agent API 文档",
    button_desktop: "免费客户端",
    button_pricing: "定价",
    evidence_title: "证据与进一步阅读",
    evidence_items: &[
        ("Agent API 文档（接入事实源）", "/help/api-access/agents"),
        ("人类接入说明", "/help/api-access"),
        ("产品 FAQ", "/help/faq"),
        ("选型对比", "/help/compare"),
        ("定价与充值", "/pricing"),
        ("免费桌面客户端", "/desktop/buy"),
    ],
    back_label: "全部集成",
};

pub const INTEGRATION_DOCS: &[IntegrationDoc] = &[
    IntegrationDoc {
        slug: "mcp",
        card_title: "MCP 接入总入口",
        card_desc: "统一 MCP 端点、工作区工具目录、鉴权与边界——任何 MCP 客户端都从这里开始。",
        h1: "Context OS MCP 接入：把工作区知识库暴露给任意 MCP 客户端",
        subtitle: "集成承接 · MCP 总入口",
        intro: &[
            "Context OS 为每个工作区提供统一的 MCP HTTP 端点：POST {api_base}/api/v1/mcp，走 JSON-RPC 2.0（initialize / tools/list / tools/call）。工作区密钥按 workspace 隔离，权限只有 index（入库）与 query（检索问答）。",
            "云端部署的 api_base 是 https://app.contextlm.top；本地桌面客户端运行期间，同一套 MCP / REST 接口面监听在 http://127.0.0.1:18080，工具与权限一致。",
        ],
        sections: &[
            IntegrationSection {
                h2: "连接参数（来自 Agent Pack）",
                paragraphs: &[
                    "在产品 UI 的工作区 Share → API Access 面板创建密钥后，点「Copy full agent pack」即得以下字段；把它们填进任意 MCP 客户端即可连通。",
                ],
                bullets: &[],
                code: None,
                table: Some(IntegrationTable {
                    headers: &["字段", "含义"],
                    rows: &[
                        &["workspace_id", "目标工作区 UUID；每次 tools/call 都必须携带"],
                        &["api_base", "API 的 HTTPS 源（云端或本地 http://127.0.0.1:18080）"],
                        &["mcp_http", "{api_base}/api/v1/mcp"],
                        &["api_key", "工作区密钥；以 Authorization: Bearer <api_key> 头发送"],
                        &["docs_agent", "/help/api-access/agents（协议事实源）"],
                    ],
                }),
            },
            IntegrationSection {
                h2: "工作区工具目录",
                paragraphs: &["以下工具对工作区密钥可用（权限列即所需 key 权限）："],
                bullets: &[],
                code: None,
                table: Some(IntegrationTable {
                    headers: &["工具", "权限", "用途"],
                    rows: &[
                        &["workspace.rag_query", "query", "对库做检索问答（RAG）"],
                        &["workspace.search_query", "query", "联网检索（原生工具）"],
                        &["workspace.list_sources", "query", "列出库内资料源"],
                        &["workspace.create_upload / complete_upload", "index", "文件上传入库（返回 upload_url 后 HTTP PUT）"],
                        &["workspace.add_url_source", "index", "把 URL 加入资料源"],
                        &["workspace.document_status", "index 或 query", "轮询解析状态直到 completed"],
                    ],
                }),
            },
            IntegrationSection {
                h2: "调用示例",
                paragraphs: &["检索问答一次 tools/call 的报文形状如下（注意 arguments 必须带 workspace_id）："],
                bullets: &[],
                code: Some((
                    "POST /api/v1/mcp",
                    "{\n  \"jsonrpc\": \"2.0\",\n  \"id\": \"1\",\n  \"method\": \"tools/call\",\n  \"params\": {\n    \"name\": \"workspace.rag_query\",\n    \"arguments\": {\n      \"workspace_id\": \"<workspace_id>\",\n      \"query\": \"总结库里已入库的文档\"\n    }\n  }\n}",
                )),
                table: None,
            },
            IntegrationSection {
                h2: "本地桌面：stdio 包装器",
                paragraphs: &[
                    "桌面客户端运行时提供 context-os-mcp（stdio → HTTP 网关转发，工具与权限与 HTTP 面一致），适合 Cursor、Claude Desktop 等 stdio MCP 客户端。相关环境变量：CONTEXT_OS_API_KEY（工作区密钥）、CONTEXT_OS_API_BASE（默认 http://127.0.0.1:18080）、CONTEXT_OS_WORKSPACE_ID。探测命令：context-os-mcp --check 或 context-os status。",
                ],
                bullets: &[],
                code: None,
                table: None,
            },
            IntegrationSection {
                h2: "边界与限制",
                paragraphs: &[],
                bullets: &[
                    "密钥限定单工作区：跨工作区使用会返回 workspace_scope_mismatch。",
                    "分享管理类工具（share_create_link、share_update_settings 等）需要用户会话；工作区密钥调用会收到 api_key_forbidden。",
                    "建工作区在产品 UI 完成；agent 凭据不做账号级操作（账号类工具对 key 返回 workspace_key_cannot_call_org_tools）。",
                    "旧路径 POST /mcp/workspaces/{workspace_id} 已弃用，应迁移到 /api/v1/mcp。",
                ],
                code: None,
                table: None,
            },
        ],
    },
    IntegrationDoc {
        slug: "cursor",
        card_title: "Cursor",
        card_desc: "让 Cursor 里的 Agent 检索你的工作区知识库：本地 stdio 或云端 MCP HTTP 两条路径。",
        h1: "把 Context OS 知识库接进 Cursor（MCP）",
        subtitle: "集成承接 · Cursor",
        intro: &[
            "目标：让 Cursor 里的 Agent 能对指定 Context OS 工作区做检索问答（workspace.rag_query / workspace.search_query），答案溯源到库内文档。",
            "两条连接路径：本地 stdio（桌面客户端运行时，配置即贴即用）与云端 MCP HTTP（无需常驻客户端）。两条路径的工具与权限一致。",
        ],
        sections: &[
            IntegrationSection {
                h2: "第一步：准备一个工作区密钥",
                paragraphs: &[
                    "免费注册并登录 app.contextlm.top，创建一个工作区并上传文件或粘贴 URL 完成入库。在工作区打开 Share → API Access，创建 API 密钥（默认含 index 与 query 权限），点「Copy full agent pack」。Pack 里有 workspace_id、api_base、mcp_http 和 api_key 四个关键值。",
                ],
                bullets: &[],
                code: None,
                table: None,
            },
            IntegrationSection {
                h2: "方式 A：本地 stdio（已在用桌面客户端时推荐）",
                paragraphs: &[
                    "桌面客户端运行期间在本机 18080 端口提供同一套 MCP 接口；用官方 stdio 包装器 context-os-mcp 接入即可。在 Cursor 的 MCP 设置（Settings → MCP & Tools → 新建 server，或直接编辑项目/全局 mcp.json）中添加：",
                ],
                bullets: &[],
                code: Some((
                    "mcp.json",
                    "{\n  \"mcpServers\": {\n    \"context-os\": {\n      \"command\": \"/path/to/context-os-mcp\",\n      \"env\": {\n        \"CONTEXT_OS_API_BASE\": \"http://127.0.0.1:18080\",\n        \"CONTEXT_OS_API_KEY\": \"<workspace_api_key>\"\n      }\n    }\n  }\n}",
                )),
                table: None,
            },
            IntegrationSection {
                h2: "方式 B：云端 MCP HTTP（无需桌面客户端）",
                paragraphs: &[
                    "在 Cursor 的 MCP 设置里添加 remote server：URL 填 https://app.contextlm.top/api/v1/mcp，请求头加 Authorization: Bearer <api_key>（值来自 Agent Pack）。",
                ],
                bullets: &[],
                code: None,
                table: None,
            },
            IntegrationSection {
                h2: "让 Agent 每次调用带上 workspace_id",
                paragraphs: &[
                    "每个 tools/call 的 arguments 都需要 workspace_id（Agent Pack 里有）。把 Pack 内容粘贴给 Cursor，或把 workspace_id 写进项目规则 / 对话上下文，Agent 即可在调用时自动携带。",
                ],
                bullets: &[],
                code: None,
                table: None,
            },
            IntegrationSection {
                h2: "验证",
                paragraphs: &[
                    "让 Cursor 调用 workspace.list_sources 列出资料源；再问一个只有库内文档才能回答的问题，检查引用是否指向你的文档。若出现 api_key_forbidden 或 workspace_scope_mismatch，对照 Agent API 文档的常见错误表排查。",
                ],
                bullets: &[],
                code: None,
                table: None,
            },
            IntegrationSection {
                h2: "边界与限制",
                paragraphs: &[],
                bullets: &[
                    "密钥只有单工作区的 index / query 权限；分享与密钥管理等用户会话工具对 key 不可用。",
                    "免费档即可完成本页全流程；会员档位影响的是可分享名额，不影响 API 接入。",
                ],
                code: None,
                table: None,
            },
        ],
    },
    IntegrationDoc {
        slug: "claude-desktop",
        card_title: "Claude Desktop",
        card_desc: "让 Claude 桌面端经 MCP 查询你的工作区：stdio 配置或远程连接器均可。",
        h1: "把 Context OS 知识库接进 Claude Desktop（MCP）",
        subtitle: "集成承接 · Claude Desktop",
        intro: &[
            "目标：让 Claude 桌面端通过 MCP 查询指定 Context OS 工作区，回答基于库内文档并可溯源。",
            "两条连接路径：本地 stdio（claude_desktop_config.json 的 mcpServers）与远程 MCP 连接器（填 URL 与 Bearer 请求头）。两条路径最终都指向同一套工作区工具。",
        ],
        sections: &[
            IntegrationSection {
                h2: "第一步：准备一个工作区密钥",
                paragraphs: &[
                    "免费注册并登录 app.contextlm.top，创建工作区并入库资料。在工作区 Share → API Access 创建密钥（默认含 index 与 query），点「Copy full agent pack」拿到 workspace_id、api_base、mcp_http 与 api_key。",
                ],
                bullets: &[],
                code: None,
                table: None,
            },
            IntegrationSection {
                h2: "方式 A：本地 stdio（桌面客户端在运行时）",
                paragraphs: &[
                    "编辑 Claude Desktop 的配置文件 claude_desktop_config.json（Settings → Developer → Edit Config；菜单名以你的客户端版本为准），加入以下 mcpServers 配置（命令路径换成你本机的 context-os-mcp）：",
                ],
                bullets: &[],
                code: Some((
                    "claude_desktop_config.json",
                    "{\n  \"mcpServers\": {\n    \"context-os\": {\n      \"command\": \"/path/to/context-os-mcp\",\n      \"env\": {\n        \"CONTEXT_OS_API_BASE\": \"http://127.0.0.1:18080\",\n        \"CONTEXT_OS_API_KEY\": \"<workspace_api_key>\"\n      }\n    }\n  }\n}",
                )),
                table: None,
            },
            IntegrationSection {
                h2: "方式 B：远程 MCP 连接器（无需本地客户端）",
                paragraphs: &[
                    "Claude Desktop / Web 的连接器（Custom Connectors）支持添加远程 MCP 服务器：URL 填 https://app.contextlm.top/api/v1/mcp，请求头加 Authorization: Bearer <api_key>。连接器入口的名称与位置随客户端版本略有差异，以你当前版本的设置为准。",
                ],
                bullets: &[],
                code: None,
                table: None,
            },
            IntegrationSection {
                h2: "让 Agent 每次调用带上 workspace_id",
                paragraphs: &[
                    "工具调用的 arguments 必须带 workspace_id。把 Agent Pack 内容粘贴进对话，或写入 CLAUDE.md / 项目记忆，Claude 即可在调用 workspace.rag_query 等工具时自动携带。",
                ],
                bullets: &[],
                code: None,
                table: None,
            },
            IntegrationSection {
                h2: "验证",
                paragraphs: &[
                    "先问「这个工作区里有哪些资料？」（应触发 workspace.list_sources）；再问一个只有库内文档才能回答的问题，核对回答是否引用你的文档。错误码含义见 Agent API 文档常见错误表。",
                ],
                bullets: &[],
                code: None,
                table: None,
            },
            IntegrationSection {
                h2: "边界与限制",
                paragraphs: &[],
                bullets: &[
                    "方式 A 依赖桌面客户端处于运行状态（stdio 包装器转发到本机 18080 接口面）；不常驻客户端时用方式 B。",
                    "密钥只有单工作区 index / query 权限；分享管理类调用需要用户会话，key 会收到 api_key_forbidden。",
                ],
                code: None,
                table: None,
            },
        ],
    },
];

pub fn integration_doc(slug: &str) -> Option<&'static IntegrationDoc> {
    INTEGRATION_DOCS.iter().find(|doc| doc.slug == slug)
}
