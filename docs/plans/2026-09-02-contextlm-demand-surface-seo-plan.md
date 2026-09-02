# Context LM — 需求承接面 SEO 方案（Phase E · Demand Surface）

**日期**: 2026-09-02 · **状态**: Plan（待执行批准）
**上游**: [`2026-08-11-contextlm-geo-seo-optimization-plan.md`](./2026-08-11-contextlm-geo-seo-optimization-plan.md)（Phase A–D 已关，GEOHub 代理分达标）、[`2026-09-01-contextlm-english-public-surface-plan.md`](./2026-09-01-contextlm-english-public-surface-plan.md)（英文面已上线）
**审查依据**: 外部文章框架「B2B / AI SaaS / B2C：自然流量究竟集中在哪些 URL」对本站的逐项代码审查（2026-09-02，结论见 §1；证据路径见 §7）
**范围**: `app.contextlm.top` 公开面（zh + en）+ `docs/design/PRODUCT_IA.md`；少量运维项
**非目标**: 搜索排名 / AI 引用率承诺；Ghost 博客内容运营；免费在线推理型工具页；程序化批量页面；付费投放

---

## 1. 审查结论（为什么开 Phase E）

对照文章框架逐项核实代码与线上数据（2026-09-02）：

| 文章框架维度 | 本站现状 | 判定 |
|---|---|---|
| 商业/信任页（漏斗底） | `/pricing`、`/desktop`、`/help/compare`、`/help/faq`、`/legal/*`、`/en/*` 全部 SSR + canonical + hreflang 双向 + JSON-LD + 更新日/作者/证据链；Phase A–D 已验证 | ✅ 强 |
| **流量承接页（TOFU/MOFU）** | **0**：无 `/blog`、`/tools`、`/templates`、`/integrations`、`/alternatives`、`/use-cases` | ❌ **缺位** |
| 转化路径 | faq/compare 内链 `/pricing`、`/help/api-access`；footer 全局带客户端入口 | ✅ 通（help 页 CTA 缺 `/desktop`） |
| AI SaaS 工程清单（文章第二层） | 无 JS 可读正文 ✅（A2/A3 实测）；登录墙边界正确 ✅；无程序化空白 URL ✅；`/shared/*` noindex ✅；无免费推理成本暴露 ✅ | ✅ |
| 衡量（文章第四节） | GSC 已接（D1）；**无目录分组四指标表**；D2 观测 bundle 就绪但 observations 空 | 🟡 |

核心结论：可索引 URL ~27 个，**100% 是漏斗底部**；非品牌搜索需求只有 `/help/compare` + `/help/faq` 两个承接点。按文章逻辑，自然流量天花板 = 品牌词 + 少量长尾；而品牌词 "Context OS" 正被同名项目稀释（上游 §19.2 英文预检 0/4 引用，仅命中 GitHub monorepo）。

### 1.1 对 180 问地图的更正（防误用）

`geo-discover` run `run-ae2f5a18bf60` 的 180 条问题 = **3 受众 × 3 场景 × 4 意图 × 5 种子题的结构化展开**，不是真实搜索量证据。种子题：

1. Context OS 是什么
2. MCP API 连接 AI agent 知识库
3. 个人知识库 RAG 工作区
4. 团队可分享文档问答
5. 第二大脑 AI 知识库

它是**覆盖地图**（哪些主题该有权威答案），不是需求排序。Phase E 选题需以 GSC 已有查询数据或外部关键词工具二次确认，不拿它当需求证明。

---

## 2. 框架映射（文章 AI SaaS 目录 → ContextLM 取舍）

ContextLM 判定：**AI SaaS（PLG：免费客户端 + 会员/BYOK）**，文章的 Tool-led 逻辑适配如下：

| 文章目录 | 取舍 | 理由 |
|---|---|---|
| `/tools/` `/templates/` `/prompts/` | **不做** | 产品本身即工具且在登录墙内；免费在线试运行 = 文章点名的推理成本项；避免无差异化程序化页 |
| `/alternatives/<竞品>` | **暂不做** | repo 硬约束：无编造竞品数据；类目级 `/help/compare` 已覆盖 |
| **`/integrations/<client>`** | **主攻** | 产品事实型（MCP / API 是真实已发布能力）；每页可做到「真实输入 / 输出 / 下一步」；宿主按 Phase C 既定决策 = app 公开 SSR |
| `/use-cases/<任务>` | **并入 integrations** | 本产品的用例与「接到哪个客户端」高度同构；不建第二套同任务页（IA：同任务不出现第三完成页） |
| blog 内容集群 | `blog.contextlm.top` 已活跃（2026-08 仍更新）；编辑型长文继续 Ghost；**产品事实型进 app**（Phase C 宿主决策不变） | — |

---

## 3. 切片

### Slice 0 — IA 前置（PRODUCT_IA.md；红线：IA before pages）

| ID | 任务 | 验收 |
|----|------|------|
| E0.1 | `PRODUCT_IA.md` §3.1 营销/发现树新增 `/integrations`（索引页，可与 `/help/api-access` 区分定位或合并，见 E2.2 备注）与 `/integrations/:client` 条目：公开 SSR、产品事实型、每页须真实可复现步骤否则不上 | PRODUCT_IA diff 可审 |
| E0.2 | §4 Canonical 增行：integrations 页 = 需求承接 deep entry，只允许链向 canonical（`/help/api-access/agents`、`/desktop`、`/pricing`），**不得**出现第二 checkout / 第二下载路径 | 同上 |
| E0.3 | nav 入口约定：footer 帮助组 + `/help` 索引 + faq/compare/api-access 互链；**不进主 nav、不进 nav-config**（Help ≠ primary nav） | `tests/navigation/nav-config.test.ts` 保持不变红 |

### Slice 1 — 工程与边界清障（先于内容，全部小刀）

| ID | 任务 | 验收 |
|----|------|------|
| E1.1 | **JSON-LD 覆盖缺口**：`SoftwareApplication` + `Organization` JSON-LD 目前只挂 `(marketing)`/`(open)` layout，`/`（zh 首页）与全部 `/en/*` 页不在两组 → 未挂载；且 `software-application-jsonld.tsx` 硬编码 `inLanguage: "zh-CN"`。改法：组件参数化 locale/url，zh 首页直挂，新增 `app/en/layout.tsx` 挂 en 版 | view-source 含 JSON-LD；`inLanguage` 随 locale；`tests/seo` 增断言 |
| E1.2 | **`/help` 定性**：现为 `(app)` 组 client 壳页，middleware 无鉴权重定向，不在 sitemap、robots 未 disallow → 匿名可索引的薄页。二选一：**A（推荐）** SSR 化为公开帮助索引（IA §3.1 本就定义 `/help`=帮助全文入口；链 faq/compare/api-access/desktop）→ 进 sitemap；B 保持 app 壳 → `robots.ts` PRIVATE_PATHS 加 `/help` | 二选一落地；robots/sitemap/测试三者一致 |
| E1.3 | **faq/compare CTA 补 `/desktop`**：`lib/content/faq.ts`、`lib/content/compare.ts`（zh/en 四处）+ `components/help/faq-page.tsx`、`compare-page.tsx` 各加一条客户端链。免费客户端是本产品最接近「先体验再注册」的激活入口，当前 CTA 链漏掉它 | zh/en 页面均见三链 CTA；tests 绿 |
| E1.4 | **www→apex 301**（marketing 仓 `~/context-os-landing` + nginx/CF；上游 §16.4 遗留）：`www.contextlm.top` 301 到唯一首选域 | `curl -I` 两域验证；运维单独授权 |
| E1.5 | **D2 英文观测录制**（人工）：`observation-bundle-2026-09-en.json` 的 observations 自 2026-09-01 起为空；按 runbook 在 ChatGPT/Perplexity 匿名会话录制 3 问 → `geo-seo-hub measure` 正式 run | measure run 归档 |
| — | 回归门：`pnpm test`（seo/navigation/style）；GEOHub diagnose 原 5 URL 复跑 | warning 仍 0 |

### Slice 2 — 首批 integrations 页（zh 先行，en 随后）

选题（从 §1.1 种子题的「接入外接 Agent」场景落 3 页，验证模型后再扩）：

| URL | 承接问句 | 真实能力锚点 |
|-----|----------|--------------|
| `/integrations/cursor` | 把个人知识库接进 Cursor（MCP） | MCP 端点 + 工作区密钥（agents 文档既有步骤） |
| `/integrations/claude-desktop` | Claude Desktop + 本地可溯源知识库 | 同上 |
| `/integrations/mcp` | MCP 知识库总入口（协议/端点/鉴权边界） | `/help/api-access/agents` 为单一事实源 |

每页硬模板（文章标准：对应输入方式、输出结果、下一步产品动作；**防模板化同文**）：

1. 真实可复现步骤：版本 + 命令/配置 + 边界如实（只读权限、密钥作用域、计费方式）
2. 证据链：锚链 `/help/api-access/agents` 对应小节 + 作者 + 更新日（authority/freshness 信号沿用 Phase B 模式）
3. CTA 三链：agents 文档 / `/desktop`（免费客户端）/ `/pricing`；**不复制接入说明正文**（单一事实源在 agents 文档）
4. 结构：H1=1、H2≥3、`<main>`/`<article>` landmark、HTML 列表或表格（Extractability 沿用 FAQ/compare 模式）

配套：`sitemap.ts` + `llms.txt` 收录；faq/compare/api-access 反向互链；`tests/seo` 增 canonical/sitemap 断言。

**验收（每页）**：GEOHub diagnose 单页复跑 warning=0、就绪度 ≥90；无 JS 可见正文 ≥500 字。

### Slice 3 — 品牌词防御（"Context OS" 同名稀释）

| ID | 任务 | 验收 |
|----|------|------|
| E3.1 | home（zh/en）正文与 `Organization` JSON-LD 强化全称实体「Context OS by ContextLM」+ `alternateName`；llms.txt 首行同步 | view-source / 测试断言 |
| E3.2 | （repo 外，建议项）GitHub monorepo README 顶部链 `https://app.contextlm.top`——英文品牌问当前唯一命中即该仓库 | — |
| E3.3 | 复跑英文预检 4 问（上游 §19.2），记录变化；连续 2 个月无改善 → 回 diagnose 单页复检 | 记录归档（不设排名 KPI） |

### Slice 4 — 衡量闭环（文章第四节落地）

| ID | 任务 | 验收 |
|----|------|------|
| E4.1 | **目录分组四指标表**（文章诊断法，本站分组极简）：GSC 导出 landing page → 按 `/`、`/pricing`、`/desktop`、`/help/*`、`/legal/*`、`/en/*`、`/integrations/*` 分组，对比自然搜索点击 / 非品牌曝光 / 转化 / 激活 | 基线表 1 份归档 + 月度 |
| E4.2 | 激活事件定义（后端已有数据，不引重分析栈）：注册 / 客户端下载 / 首建 workspace / 首次分享开启 / 首次付费；落地页 → 事件按来源组合归因 | 事件口径文档化 |
| E4.3 | D2 月度例行（中英各 3 问），延续 observation bundle 协议 | 月度记录 |

---

## 4. 成功指标（不夸大）

| 指标 | 现状（2026-09-02） | Phase E 目标 |
|------|--------------------|--------------|
| 承接型可索引 URL | 2（faq/compare） | ≥5（+3 integrations；`/help` 定性后另计） |
| GEOHub diagnose warning | 0 | 维持 0 |
| help 族内链网 | 缺 `/desktop` CTA | faq/compare/api-access/integrations 全互联 |
| 文章式目录四指标表 | 无 | 基线 1 份 + 月度 |
| 英文品牌问引用 | 0/4 | 记录趋势（不设 KPI） |
| 免费流量推理成本 | 0 | 保持 0（integrations 页不含在线试运行） |

## 5. 风险与边界

| 风险 | 缓解 |
|------|------|
| 模板化同文页（文章点名的 AI SaaS 最常见死法） | 每页真实步骤 + 边界如实 + diagnose 复测；写不出真实可复现步骤的选题不上 |
| 出现第二 checkout / 下载路径 | E0.2 canonical 约束；nav-config 不动 |
| 180 问地图被误当搜索量证据 | §1.1 更正；选题需 GSC/外部工具佐证 |
| integrations 页与 agents 文档内容漂移 | 接入步骤单一事实源在 agents 文档，integrations 页只做需求承接 + 锚链 |
| 工程量膨胀 | Slice 1 均为小刀；Slice 2 主体是内容件，模板一次成型；纯前端 + `pnpm test` 级验证，无重构建 |

## 6. Owner 与依赖

- 前端（本仓）：E0、E1.1–E1.3、Slice 2 页面与测试
- 内容：Slice 2 三页文案（zh 先行 → en 人工翻译，沿英文面方案 §3 硬约束）
- 运维：E1.4（marketing 仓 + CF/nginx）
- 人工：E1.5、E3.3、E4.1、E4.3（GSC 导出 / AI 平台录制）

## 7. 附录：审查证据（2026-09-02）

- URL 面：`frontend_next/app/sitemap.ts`（27 URL）；`frontend_next/app/robots.ts`（PRIVATE_PATHS 无 `/help`）；`frontend_next/app/llms.txt/route.ts`
- 渲染/结构化数据：`frontend_next/app/page.tsx` + `home-client.tsx`（SSR 摘要）；`frontend_next/app/en/page.tsx`（hreflang）；`(marketing)/layout.tsx`、`(open)/layout.tsx`（JSON-LD 挂载点，`/` 与 `/en/*` 未覆盖）；`components/software-application-jsonld.tsx`（`inLanguage` 硬编码 zh-CN）
- 内链：`components/help/faq-page.tsx`、`compare-page.tsx`（CTA 链 pricing/api-access，无 desktop）；`lib/content/faq.ts`、`compare.ts`
- 边界：`middleware.ts` + `lib/middleware-routing.ts`（`/help` 无鉴权重定向）；`tests/seo/public-seo.test.ts`（现行断言范围）
- 需求图：`/home/chuan/GEOHub/runs/contextlm/run-ae2f5a18bf60/query-map.json`（结构化展开，§1.1）
- 相关文档：`docs/design/PRODUCT_IA.md` §3.1/§4 · `docs/engineering/MULTI_SITE_IA_INTEGRATION_PLAN_2026-07-14.md` · `docs/plans/2026-08-12-gsc-onboarding-checklist.md`

---

## 8. 执行记录

**2026-09-02（Slice 0 + Slice 1 代码侧）**：

- **E0 ✅**：`PRODUCT_IA.md` §3.1 增 `/integrations`·`/integrations/:client`（标记计划中，发布前不进 sitemap）、`/help` 定性注释；§4 增「按 client 接入知识库」canonical 行；E0.3 入口约定（不进主 nav / nav-config）已写入。
- **E1.1 ✅**：`components/software-application-jsonld.tsx` / `organization-jsonld.tsx` 参数化 locale（导出纯函数 `softwareApplicationJsonLd` / `organizationJsonLd` / `websiteJsonLd`）；zh 首页 `app/page.tsx` 直挂；新增 `app/en/layout.tsx` 挂 en 版（`inLanguage: "en"`，description / 作者名 / offer 文案随 locale 切换）。
- **E1.2 ✅ 采用 B，机制改为 per-page noindex**：读 `/help` 全文确认其为应用内帮助中心（返回工作台 / 账户 / 排障主题）——A（公开索引）属内容新建而非清障，移入 Slice 2 再议；且 robots `Disallow: /help` 前缀会误杀公开的 `/help/faq|compare|api-access` 族。实现：`/help`、`/help/write` 拆 server 壳（`robots: { index: false, follow: true }`）+ client 正文（`help-center-client.tsx` / `help-write-client.tsx`）；`robots.ts` / `sitemap.ts` 不动，测试断言公开 help 族不受 disallow 影响。
- **E1.3 ✅**：`lib/content/faq.ts` / `compare.ts`（type + zh/en）与 `components/help/faq-page.tsx` / `compare-page.tsx` 各增 `buttonDesktop`（免费客户端 / Free desktop client）按钮 → `/desktop`。
- **E1.4 ⬜** 运维项（marketing 仓 + CF/nginx，需单独授权）；**E1.5 ⬜** 人工录制 D2。
- **验收**：`pnpm vitest run tests/seo tests/help tests/navigation tests/style` 47/47 通过（含新增 JSON-LD locale / en 布局挂载 / noindex / 公开 help 族不受 robots 误伤断言）；`pnpm typecheck` 通过；`code-review-graph update` 已跑。未提交：工作树混有英文面在途改动，提交由用户统一处理。

**2026-09-02（Slice 2 代码侧 + E3.1）**：

- **Slice 2 ✅（zh 先行，en 待人工翻译）**：`lib/content/integrations.ts` 内容单一数据源（索引 + `cursor` / `claude-desktop` / `mcp` 三页）；`components/integrations/`（doc / index / shared）+ `app/(open)/integrations/*` 4 路由（`force-static` + canonical + zh-CN/x-default hreflang）。**内容事实全部取自 `public/docs/api-access-for-agents.md`**（端点 `/api/v1/mcp`、Agent Pack 字段、workspace.* 工具表、stdio 包装器与 env、错误码），未发明新声明；客户端侧只写两条已锚定路径（本地 stdio 配置 = 文档原例；云端 remote = 文档端点 + Bearer 头），UI 菜单名处注明「以客户端版本为准」。sitemap +4 路径；llms.txt +3 条 + Notes 日期。
- **E0.3 落地方式更正**：footer 帮助组实际由 `nav-config.ts` 驱动（IA §4 单一数据源 + nav 测试看守），footer 项即 nav-config 项。本批入口 = **help 族互链**（faq 证据链 +1、compare 下一步行 +1、agents 页与 api-access 页按钮各 +1 Integrations）+ sitemap + llms.txt。app 壳页均已 noindex，footer 入口 SEO 价值≈0，nav-config footer 项待 integrations 有真实流量数据后再议。
- **E3.1 ✅**：`OrganizationJsonLd` 加 `alternateName: "Context OS"`；`home.seoTitle` zh 改「Context OS（ContextLM 旗下）— …」、en 改「Context OS by ContextLM — …」；zh/en 首页 metadata title 同步；llms.txt 首行改全称。`home-summary` 测试的品牌断言随之 getByText→getAllByText（H1 现也含品牌名）。
- **E3.2 ✅**：README 顶部改「**Context OS by ContextLM** — workspace-centric knowledge base · RAG · agents」，产品行补 `[集成](https://app.contextlm.top/integrations)`（原已有应用/客户端链接，实际缺口是品牌全称关联与集成页入口）。
- **E3.3 ✅（第 1 个月数据点，2026-09-02，web-search 代理口径，非 ChatGPT/Perplexity 正式 D2）**：

  | query | 本站召回 | 观察 |
  |-------|---------|------|
  | What is Context OS? | ❌ | 同名稀释加剧：Elixir Data「governed AI agent OS」概念文居首；另有 GitHub ContextOS、GTM context-os quickstart、Trace iOS app、LinkedIn 概念文 |
  | How to choose a personal AI knowledge base? | ❌ | 通用清单站（toolfinder、博客园评测、Reddit r/selfhosted） |
  | Context OS vs Notion AI? | ❌ | 搜索代理把「Context OS」自行改写成类别词（Glean context 层），品牌歧义被类别化 |
  | Context OS contextlm.top MCP knowledge base | ✅ 部分 | 带 `contextlm` 限定后 **contextlm.top 与 app.contextlm.top 双域召回**（较 09-01 预检「仅 GitHub 命中」改善）；MCP 细节召回仍以 modelcontextprotocol.io / 第三方 GitHub 为主，本站 MCP 内容未直接出现 |

  判读：①全限定问已能召回双域，E3.1 全称实体方向正确；②无限定品牌问稀释加剧 → 英文面后续按 §19.2 加差异化定位（本地部署 / MCP / 可分享名额）；③`/integrations/mcp` 正对 MCP 查询缺口，部署 + 索引爬升后下月复测；④注意另一组同名：arXiv 论文 ContextLM、ContextLM.ai（TTS）。连续 2 个月无限定问改善 → 回 diagnose + 定位强化。
- **Slice 4 预备 ✅**：GSC 目录分组四指标模板落 `docs/plans/geo-seo-briefs/gsc-directory-baseline-template.md`（7 目录组 × 4 指标 + 激活事件口径）；部署后 diagnose brief 落 `docs/plans/geo-seo-briefs/diagnose-contextlm-integrations.json`（4 新 URL，验收线 warning=0 / 就绪度 ≥90 / 无 JS ≥500 字）。
- **剩余（人工/运维）**：E1.4 www→apex 301（需单独授权）；E1.5 D2 正式录制；Slice 4 首份 GSC 基线（需 GSC 导出）；en 版 integrations 三页人工翻译；部署后跑 diagnose brief。
- **验收**：53/53（seo/help/navigation/style；新增 integrations 单 H1 + CTA canonical 断言、sitemap/llms/canonical 4 页断言）；`pnpm typecheck` 通过；`code-review-graph update` 已跑。
- **部署后待办**：写 `diagnose-contextlm-integrations.json` brief（4 新 URL），GEOHub 单页复跑（验收线：warning=0、就绪度 ≥90、无 JS 正文 ≥500 字）。

**2026-09-02（部署 + 复测关闭 Slice 2 验收门）**：

- **部署**：`scripts/deploy-frontend.sh` 两跑（初次 + evidence 修复后重发），rev `92f38626+dirty`，冒烟 local_fe / pub_desktop / pub_login 全 200。4 个 integrations 路由在构建中均为静态预渲染（○）。
- **线上抽查**：4 新 URL 均 200 + 单 H1 + 自指 canonical + `/desktop` CTA；zh 首页新标题（ContextLM 旗下）+ JSON-LD、`/en` 新标题 + `"inLanguage":"en"`、`/help` `noindex, follow`、sitemap 含 4 条 integrations——E1.1 / E1.2 / E3.1 / Slice 2 全部线上生效。
- **GEOHub diagnose**（`diagnose-contextlm-integrations.json`）：
  - 第一跑 `runs/contextlm-integrations-2026-09-02/run-929dc14815eb`：**1 warning**——`/integrations` 索引页无 evidence 语言信号（启发式找 来源/方法 类词），就绪度 87，未过验收线。
  - 修复：索引页加证据方法行（「来源与方法：步骤、端点与错误码均锚定 Agent API 文档并对照公开定价页核对」）+ 第三段「从哪页开始」导览。
  - 第二跑 `runs/contextlm-integrations-2026-09-02-fix/run-84ce434051c3`：**completed，warning_count = 0**；站点六维全 100（Authority / Discoverability / Evidence / Extractability / Freshness / Structure）。

  | URL | 就绪度 | 无 JS 可见文本 |
  |-----|-------:|---------------:|
  | `/integrations` | **100**（87→100） | ~756 字 |
  | `/integrations/cursor` | **100** | ~1532 字 |
  | `/integrations/claude-desktop` | **100** | ~1553 字 |
  | `/integrations/mcp` | **100** | ~2008 字 |

  Slice 2 验收门（warning=0 / ≥90 / ≥500 字）**全部关闭**。
- **待 GSC**：sitemap 已含新路径（`/sitemap.xml` force-static，随部署更新）；索引爬升期约 2–4 周，下月按 `gsc-directory-baseline-template.md` 填首份基线 + E3.3 第 2 个月预检。
