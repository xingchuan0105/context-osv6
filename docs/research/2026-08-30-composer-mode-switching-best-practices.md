# Research — Composer「检索模式」交互设计业界实践

**日期**: 2026-08-30
**问题**: AI 助手/知识库产品中,聊天输入框的「知识库检索 / 网络搜索」能力切换怎么做防呆(默认值、状态可见性、上下文绑定、多能力共存 UI、首问引导)?
**状态**: 供 Context-OS workspace composer 防呆再设计参考
**采信等级**: ✅=官方文档/博客原文证实;⚠️=二手来源(维基/科技媒体);❌=本网络环境不可达,未查到(如实标注,不做推断)

**取证说明**: ChatGPT(help.openai.com)、Google(support.google.com / notebooklm.google.com)、Perplexity(support.perplexity.ai)、xAI(help.x.ai)在本环境不可达(403/连接超时),ChatGPT 部分经由官方博客页与官方 Help「Projects」长文抓回;Perplexity 条目主要依赖二手。

---

## 1. ChatGPT(OpenAI)

- ✅ **自动检索 + 可手动强制**:发布文原文——「ChatGPT will choose to search the web based on what you ask, or you can manually choose to search by clicking the web search icon.」(openai.com/index/introducing-chatgpt-search/,2024-10-31 发布;页内更新:2024-12-16 全登录用户、2025-02-05 免注册全量)
- ✅ **来源呈现**:「Click the Sources button below the response to open a sidebar with the references.」——回答下方 Sources 按钮 + 右侧栏引用。
- ✅ **Projects = 上下文绑定的容器**:官方 help「Projects in ChatGPT」——上传 reference files 后「ChatGPT can use what you add to provide more informed answers」,无需每轮选文件、无模式开关;新会话直接继承项目文件。设计上以「容器(项目)绑定上下文」替代「每条消息选模式」。
- ⚠️ 演进解读(维基/媒体综合):搜索从「手动 icon 触发」起步,上市当天文案即写明自动判断,后续仅扩面(全量→免注册)。方向是**减少用户需要做的模式决定**。

## 2. Claude(Anthropic)

- ✅ **先全局开一次,之后按题自动**:「Toggle on web search in your profile settings … When applicable, Claude will search the web to inform its response」;2025-05-27 起全球全 plan。(claude.com/blog/web-search,原 anthropic.com/news/web-search)
- ✅ **开关在 composer,不在 profile**(现行 help):「Click on the "+" button in the lower left corner of the chat window」,勾选「Web search」后「When you ask about topics that benefit from current information, Claude invokes a search tool」;可口头强制或口头禁用。(support.claude.com/en/articles/10684626-enable-and-use-web-search)
- ✅ **回答一律带引用**:「Every response includes citations, so you can easily verify sources yourself.」
- ✅ **Projects 知识库完全自动、零开关**:上传到 project knowledge 的文档自动用于该 project 内对话;RAG 版本「automatically activates … No setup or configuration is required」。(support.claude.com/articles/9517075、11473015)
- **要点**:Claude 把「开关」降级为一次性的会话级偏好(勾一次持续生效),检索决策交给模型;文档检索则根本不给用户开关。

## 3. Perplexity

- ⚠️ 模式选择器(Fast / Auto / Reasoning / Deep Research / Labs)在 composer 一级位置,默认 **Auto**,由系统按问题难度路由(知乎/中文指南与产品介绍一致;官方 support 站不可达,未见官方原文,保留待核)。Auto 的存在本身 = 业界承认「让用户逐题选模式」是负担。

## 4. NotebookLM(Google)

- ❌ 官方 FAQ(support.google.com / notebooklm.google.com)本环境不可达。
- ⚠️ 公认形态:封闭语料——只答上传来源、答案带指向原文的行内引用;不提供「脱离来源的纯聊天模式」。作为「默认正确」的极限案例(空间即语义),其教程/介绍文章均确认此行为,但本轮未拿到官方原句,采信等级 ⚠️。

## 5. Notion AI / Enterprise Search

- ✅ **检索是唯一路径,不是模式**:「To use Enterprise Search, open the Home tab … enter your question」;默认覆盖 workspace + 连接应用 + web:「By default, Enterprise Search looks at all of the sources available」。
- ✅ **来源可以做减法而非加法**:「toggle Web search off」「Toggle Apps and integrations off」;也能 @-mention 或「Add context」收窄。「it'll always cite its sources so you can go back to the source.」(notion.com/help/enterprise-search,Business/Enterprise)
- 关键差异:Notion 默认「全开」,用户的操作是**排除**不想查的,而不是**想起**要开。

## 6. Glean(企业检索助手)

- ✅ **默认 All Knowledge,query 期做减法**(官方 user guide「Info access」):「By default, Glean uses All Knowledge mode and automatically selects the most relevant sources for each question. You don't need to choose a knowledge source manually.」
- ✅ **chat box 内两个 toggle——与我们的 chips 同构,但语义相反**:「Search the web」与「Use company sources」两个开关默认都**开**;「When both toggles are off, Glean relies only on the LLM's pre-trained knowledge. Responses generated this way don't include citations because no sources are retrieved.」
- ✅ 权限内可见性 + 深链引用:「Permission-aware: Answers use only content you already have permission to see.」;deep-linked citations 悬停显示原文高亮与页码。(docs.glean.com/user-guide/assistant/...)
- Glean 是与我们最接近的对照:**同样的双 toggle,默认值取「双开」,并把「双关=无引用裸 LLM」明确标注为降级态**。

## 7. Cursor(上下文锚定 vs 自动检索的配合)

- ✅ **@-mention 是「我知道目标」的加速器,不知道则交给 agent**:`@`「to attach specific context … helps Agent focus on the right files」;「If you're not sure which files matter, skip it — Agent finds relevant files through its own search.」(cursor.com/help/customization/context)
- ✅ 自动检索按需触发:grep「runs automatically; no configuration needed」;Explore subagent「uses … automatically when it decides a task benefits from broad search」。(cursor.com/docs/agent/tools/search)
- 模式:显式锚定(用户点名对象)与自动检索(系统兜底)不是二选一,而是**两层**:上层手选、下层自动。

## 8. Grok / Raycast

- ❌ help.x.ai、grok.com、docs.raycast.com 本环境不可达,未查到,不臆断。

---

## 可迁移设计原则(每条附实例)

1. **默认值应匹配容器的核心任务,且随上下文自动调整**——项目/工作区里有文档,提问默认应该对文档做检索。实例:ChatGPT Projects(文件入项目即自动可答)、Claude Projects(「will use」自动),反例即本产品默认纯 chat。
2. **开关语义优先做「减法」**:让用户关掉不想要的来源,而不是想起并打开想要的。实例:Notion(默认全量可 toggle off)、Glean(双 toggle 默认双开)。
3. **「检索中/引了什么」必须可见**:自动检索的产品全部配系统化引用展示(Sources 侧栏 / inline citations / deep-linked 引用)。实例:ChatGPT Sources、Claude「Every response includes citations」、Glean deep-linked citations、Notion「always cite its sources」。
4. **多能力共存用多选 toggle,不用单选模式**:Glean 的双 toggle 与我们的 chips 同构,是知识库+联网场景的主流形态;单选 segmented 见于 Perplexity 那种「互斥的速度/深度档位」,与「两个独立证据来源」语义不同。
5. **有可检索内容时,「纯 LLM 回答」是应被标注的降级态**:Glean 明说双关 off = 无引用;ChatGPT/Claude 的纯知识回答不标引用即自然显形。我们产品问题恰是降级态(纯 chat)对用户完全隐形。
6. **首问前给「情境默认」,Ask-once**:空状态文案只起辅助作用;真正防住误用的是「上传即选中并点亮检索」与「引导式点击」(选中来源是明确动作,满足显式同意)。实例:各 Projects 类产品上传即生效;Cursor「不确定就交给 agent 自己找」。
7. **逐题强制/禁用用自然语言兜底已验证可行**(Claude「在 prompt 里说 Search the web / 不要用 web search」);对应我们后端 Lead 判定 base_tools/none 的现状——检索意图可以留给 Lead,但前提是检索工具已挂载。

## 对本产品的直接启示

- 「初始自带 RAG / chat 要点击」的两极方案都偏离业界:业界不做「惩罚式门槛」,也不停在「默认全关」;主流是**容器即语义**(有文档的容器默认检索这些文档)+ **减法开关** + **降级态显形**。
- 我们已有但写错通道的「上传/选中文档 → 自动 rag」正是业界默认做法,应修复复活;两个 chips 保留多选语义,但默认值与状态可见性按上述原则重排。