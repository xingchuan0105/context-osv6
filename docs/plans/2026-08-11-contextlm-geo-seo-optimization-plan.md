# Context LM / Context OS — GEO & SEO 优化方案

**日期**: 2026-08-11  
**状态**: Active plan（基于 GEOHub 0.3.1 实测；按 PR 切片落地。2026-08-11 复审决策已并入：A2 选定 SSR 摘要、Phase C 内容宿主已定、A4 实为从零创建）  
**范围**: 公开站点与文档面（`contextlm.top`、`app.contextlm.top` 的公开路由）  
**非目标**: 保证搜索排名/AI 引用率；自动改 CMS；GSC/Analytics 账号操作  

---

## 1. 背景与工具

| 项 | 说明 |
|----|------|
| 工具 | [GEOHub](https://github.com/yaojingang/GEOHub) v0.3.1（本地：`/home/chuan/GEOHub`） |
| GEO | Generative Engine Optimization：提升在 AI 回答中的**可引用 / 可抽取**就绪度 |
| SEO | 传统技术与内容规划（本轮 SEO skill 为 **advisory / 只读计划**） |
| 分析产物 | `/home/chuan/GEOHub/runs/contextlm/` |

### 1.1 本轮运行 ID

| 能力 | Run ID | 主要内容 |
|------|--------|----------|
| `geo-diagnose` | `run-b664673877e5` | 5 URL 抓取 + 诊断报告 `report.md` |
| `seo` | `run-cd08db440bd7` | 技术审计模式行动计划 `seo-plan.json` |
| `geo-discover` | `run-ae2f5a18bf60` | 180 条问题地图 + 机会表 |

复跑：

```bash
cd /home/chuan/GEOHub
.venv/bin/geo-seo-hub diagnose \
  --input /tmp/geohub-briefs/diagnose-contextlm.json \
  --output runs/contextlm
```

Brief 模板见同次会话生成的 `/tmp/geohub-briefs/*.json`；建议迁入仓库 `docs/plans/geo-seo-briefs/` 以便复现（可选后续 PR）。

---

## 2. 审计范围与抓取结果

| # | URL | 抓取 | 可见文本量（约） | Answer-readiness 启发式 |
|---|-----|------|------------------|-------------------------|
| 1 | https://contextlm.top/ | observed | 1489 字 | **91/100** |
| 2 | https://app.contextlm.top/ | observed | **64 字** | **26/100** |
| 3 | https://app.contextlm.top/pricing | observed | 中等 | **54/100** |
| 4 | https://app.contextlm.top/help/api-access | observed | **15 字**（「加载中…」） | **28/100** |
| 5 | https://app.contextlm.top/help/api-access/agents | observed | **~9k 字** | **69/100** |

说明：GEOHub diagnose **不执行 JS**。与多数搜索爬虫 / AI 抓取器一致。CSR 壳页会被判定为「几乎无正文」。

---

## 3. 站点级 GEO 分数（诊断）

| 维度 | 分数 | 解读 |
|------|-----:|------|
| Discoverability | **75** | 标题/描述等多页具备；canonical 普遍缺失 |
| Extractability | **56** | app 壳页与受登录门控页拖后腿 |
| Structure | **50** | 多页 H1/H2/main 结构不足 |
| Authority | **52** | 组织/作者/责任方信号弱 |
| Evidence | **52** | 声明缺少可核验来源/方法表述 |
| Freshness | **36** | 公开页缺少明确更新时间（最弱） |

**漏斗状态（GEOHub funnel）**：

- candidate-eligibility：**observed**（仅限已供给 URL）  
- citation-selection：**proxy**（就绪度代理，非真实引用概率）  
- answer-absorption：**not-observed**（未提交 ChatGPT/Perplexity 等平台观测）

---

## 4. 问题诊断（根因）

### 4.1 结构性根因（P0）

1. **App 壳页 SSR 空洞**  
   - `app.contextlm.top/` 首屏 HTML 几乎只有「正在进入…」+ 页脚法律链。  
   - 爬虫/GEO 看不到产品价值主张 → 就绪度 26。  

2. **人类帮助页被鉴权/壳挡住**  
   - `/help/api-access` 无登录时仅「加载中…」。  
   - 同路径族下的 `/help/api-access/agents` 已做成公开 SSR，对比鲜明（就绪度 69）。  

3. **全站公开页缺 canonical**  
   - 审计 5 URL 均 **warning: canonical missing**。  

### 4.2 内容与权威（P1）

4. **定价页**有内容但缺 authority / freshness / evidence 信号。  
5. **营销站** `contextlm.top` 相对最好（就绪度 91），应作为对外叙事主锚，与 app 域名关系需在 HTML 中写清（同一产品、分工：营销 vs 应用）。  

### 4.3 SEO 规划缺口（P2，缺外部证据）

`seo` skill 标明 missing：

- rendered page / full crawl evidence  
- indexation outcome（需 Search Console）  

在未接 GSC 前，**不编造**索引量、排名、流量结论。

---

## 5. 优化方案（可执行）

### 5.1 原则

| 原则 | 做法 |
|------|------|
| SSR 优先 | 凡希望被 AI/搜索引用的文案，必须出现在**无 JS** 的 HTML 中 |
| 公开 vs 应用 | 营销/文档公开；应用壳不承担 SEO 主叙事 |
| 证据边界 | 能力/定价声明旁链到可核验文档、版本或更新日志 |
| 小步可验 | 每改一页，用 GEOHub diagnose 对该 URL 复跑，对比就绪度 |

### 5.2 工作流（推荐）

```text
Discover（问题地图）
    → 选定 3～5 个对外问题
    → 内容页 SSR 落地（comparison / FAQ / landing）
    → Diagnose 单页复检
    → （可选）人工在 AI 平台抽检引用 → geo-measure
```

SEO 技术轨并行：

```text
robots / sitemap / canonical / 索引边界
    → GSC 验证
    → 再跑 seo skill 补「有证据」计划
```

---

## 6. 分阶段落地（PR / 任务切片）

### Phase A — 可抓取基线（1～2 周，工程向）

| ID | 任务 | 验收 |
|----|------|------|
| A1 | 全站公开页补 `rel=canonical`（marketing + app 公开路由） | 源码/响应头可见 canonical；diagnose 对应 warning 消失 |
| A2 | `app.contextlm.top/`：SSR 插入简短产品说明 + 单一 H1（**决策 2026-08-11**：采用 SSR 摘要；**放弃 302**——根页面是桌面端（Tauri）/web 共用入口，302 会动桌面端冷启动与登录漏斗，代价不对等） | 可见文本 ≫ 64；H1=1；就绪度明显上升 |
| A3 | `/help/api-access` 公开摘要 SSR（登录后仍进工作区密钥 UI） | 未登录可见「如何接入 / Agent Pack」；非仅「加载中…」 |
| A4 | **创建** `robots.txt` + `sitemap.xml`（仓库与 nginx 层均无现存文件，非「盘点」）+ `llms.txt`；文档化索引边界 | 三文件均 200；sitemap 含定价/帮助/营销公开页；robots 明示私有路径与 AI crawler 策略 |

**A2 推荐文案骨架（SSR，中英可 i18n）**：

- H1：Context OS — 可分享的个人知识工作区  
- 3 条 bullet：文档入库与问答 · 外接 Agent（MCP）· 会员与分享名额  
- CTA：进入应用 / 查看定价 / Agent 接入说明  

### Phase B — 权威与定价（1 周）

| ID | 任务 | 验收 |
|----|------|------|
| B0 | 前置：确认生产构建 `NEXT_PUBLIC_PRICING_REVAMP_ENABLED=1` 常开（flag=0 时 `/pricing` SSR 渲染为空并客户端跳 `/dashboard`——canonical 钱路由被特性门控清空，见 `frontend_next/lib/billing/featureFlag.ts`） | 部署清单记录 flag 状态 |
| B1 | 定价页：更新时间、组织/产品身份、档位对比表（HTML table 或清晰列表） | freshness/authority/evidence 信号改善 |
| B2 | 帮助中心入口页：公开目录 + 绝对链到 agents / 人类 API 说明 | 与 PRODUCT_IA 一致，无断链 |
| B3 | 页脚/关于：统一品牌 Context LM / Context OS 关系一句话 | 跨站实体一致 |

### Phase C — GEO 内容资产（2～4 周，内容向）

来自 `geo-discover`（180 问）的高优资产类型：

| 优先级 | 资产类型 | 建议选题 |
|--------|----------|----------|
| 85 | **comparison** | Context OS vs Notion AI / 其它第二大脑 / 通用 RAG 套件（中立对比表 + 证据） |
| 80 | **faq** | MCP 接入、workspace key 边界、分享名额、BYOK、定价 |
| 75 | **landing** | 「给外接 Agent 的知识库」落地页（链到 `/help/api-access/agents` + Pack） |
| 70 | **article** | 「什么是可分享工作区」「个人知识库如何给 Cursor/Claude 用」 |

**内容宿主决策（2026-08-11）**：编辑型内容（comparison / article）发 `blog.contextlm.top`（Ghost，MULTI_SITE 既定「内容 / SEO」面）；产品事实型内容（FAQ、「给外接 Agent 的知识库」landing）发可 SSR 的 marketing/help。新增公开 surface 前先更新 `docs/design/PRODUCT_IA.md`（IA before pages）。

**内容硬约束**（与 GEOHub content skill 一致）：

- 每条关键声明可追溯到来源（文档 URL / 产品版本 / 截图策略另议）  
- 禁止无证据编造竞品数据或流量承诺  
- 优先发在 **可 SSR 的 marketing/help**，再在 app 内链  

### Phase D — 测量闭环（持续）

| ID | 任务 | 验收 |
|----|------|------|
| D1 | 接 Google Search Console +（可选）Bing | 有索引覆盖与查询报告 |
| D2 | 月度人工抽检：3 个种子问在 ChatGPT/Perplexity 是否提到品牌 | 记录 JSON → `geo-measure` |
| D3 | 每发布一页对比文，复跑 `diagnose --input` 单页 brief | 就绪度与 warning 列表归档 |

---

## 7. 按页面的改造清单

### 7.1 https://contextlm.top/（保持为对外主叙事）

> 仓库：`~/context-os-landing`（独立静态 export 站，不在本 monorepo；见 MULTI_SITE 文档）。本节任务需在该仓落地，本仓改动不涉及。

- [x] 相对完整可见文案（已较强）  
- [ ] 补 canonical  
- [ ] 强化「与 app.contextlm.top 分工」一句  
- [ ] 更新时间 / 品牌实体  

### 7.2 https://app.contextlm.top/（应用入口）

- [x] SSR 产品摘要 + H1（2026-08-11 A2：`app/page.tsx` server 壳 + `home-client.tsx`，跳转逻辑不变）  
- [x] 避免把唯一价值主张只放在客户端渲染  
- [x] canonical；登录墙路由明确索引边界（robots disallow：/dashboard /settings /shared 等）  

### 7.3 https://app.contextlm.top/pricing

- [x] canonical（2026-08-11 A1）  
- [ ] 可见档位对比 + 更新日  
- [ ] 链到法律页与帮助  

### 7.4 https://app.contextlm.top/help/api-access

- [x] **公开摘要**（无登录可读）：Agent Pack 流程、权限边界、链到 agents 文档（2026-08-11 A3：迁至 `(open)` 组，无 App top bar）  
- [x] 完整密钥 UI 仍可登录后展示（密钥管理本就在 `/dashboard/:id/share#api`，未公开）  
- [x] H1/H2 结构  

### 7.5 https://app.contextlm.top/help/api-access/agents

- [x] 公开可读（2026-08-11 Agent Pack 工作已落地）  
- [x] 补 canonical（2026-08-11 A1）  
- [ ] 可选 Last-Updated（留 Phase B freshness）  
- [ ] 与营销「Agent 接入」落地页互链  

---

## 8. SEO 技术清单（与 GEO 并行）

| 项 | 动作 | 证据来源 |
|----|------|----------|
| robots.txt | 确认公开页允许；app 私有路由策略 | HTTP 200 正文 |
| sitemap | 含 marketing、pricing、help 公开页 | URL 列表与 lastmod |
| canonical | 每公开页唯一 | HTML link |
| 状态码 | 无软 404；帮助旧链 307/301 正确 | curl / GSC |
| host 规范化 | apex `contextlm.top` ↔ `www.contextlm.top` 301 到唯一首选域（`site-map.ts` 默认 `www`，审计用 apex；per-page canonical 生效前提） | curl -I 两域 |
| AI crawler | robots.txt 明示允许 GPTBot / ClaudeBot / PerplexityBot 等（GEO 目标 = 被引用）；私有路径统一 disallow。**注意（2026-08-12 实测）**：Cloudflare 托管 robots 块（AI Crawl Control）在文件头部注入 GPTBot / ClaudeBot / Google-Extended 等 `Disallow: /`，需在 CF 控制台关闭或放行，否则应用层规则被抢先生效 | robots.txt 正文 |
| llms.txt | app route 提供 llms.txt 索引（`app/llms.txt/route.ts`；public/ 在 standalone 部署不对外服务） | /llms.txt 200 |
| 索引边界 | 登录后 dashboard **不**进 sitemap | 代码审查 |
| 国际化 | hreflang 或明确单语默认（若中英并存） | 页面策略文档 |
| Search Console | 提交 sitemap、监控覆盖 | GSC |

`seo` skill 建议阶段：`scope` → `access-and-discovery` → `fetch-render-and-indexability`。  
**Write authorized: false** — 本方案默认只读；改线上需单独授权与回滚边界。

---

## 9. 成功指标（可观测、不夸大）

| 指标 | 基线（2026-08-11） | 目标（约 4～6 周） |
|------|-------------------|-------------------|
| app 首页可见文本 | ~64 | ≥ 300 且 H1=1（摘要定位；主叙事在 hub，不在 app 壳堆长文） |
| help/api-access 未登录可见文本 | ~15 | ≥ 500 + 稳定外链 |
| 公开页 canonical | 0/5 | 5/5 |
| diagnose Freshness | 36 | ≥ 55 |
| diagnose Extractability | 56 | ≥ 70 |
| 对外对比/FAQ 页 | 0 | ≥ 2 篇 SSR |
| GSC 索引覆盖 | 未知 | 已配置并有数据 |
| AI 引用抽检 | 未测 | 月度 3 问有记录 |

不设「排名第几」「引用率百分之几」类无测量基线的 KPI。

---

## 10. 风险与边界

| 风险 | 缓解 |
|------|------|
| App 全站 CSR 导致误判 | 以 SSR 正文为准；关键页用 diagnose 复测 |
| GEOHub Experimental | 结论作工程 backlog，不作商业承诺 |
| AGPL 工具链 | 分析输出可自用；嵌入商业产品需看法务 |
| 密钥/私有工作区内容 | **永不**把私有知识库正文当公开 SEO 素材 |
| 过度优化导致产品文案失真 | 以 PRODUCT_IA 与真实能力为准 |
| 根页面是桌面端（Tauri）/web 共用入口 | A2 改动须桌面构建（`BUILD_TARGET=desktop`）回归冷启动跳转 |
| `/pricing` 被 `NEXT_PUBLIC_PRICING_REVAMP_ENABLED` 门控，flag=0 时 SSR 为空并跳走 | Phase B 前置确认生产 flag 常开；中期评估移除门控 |
| Cloudflare 托管 robots.txt（AI Crawl Control）注入 GPTBot / ClaudeBot / Google-Extended 等 `Disallow: /`，与应用层 GEO 放行策略冲突 | 在 CF 控制台调整 AI Crawl Control（放行或关闭托管块）；每次部署后复核生产 robots.txt 首尾块 |

---

## 11. 建议 Owner 与依赖

| 角色 | 职责 |
|------|------|
| 前端 | A1–A3、canonical、公开 help 摘要 SSR（app 侧本仓 `frontend_next`；marketing 侧 `~/context-os-landing`） |
| 内容/增长 | Phase C 对比文与 FAQ |
| 运维 | robots/sitemap、GSC、证书与 CDN 缓存 |
| 产品 | 索引边界（哪些 app 路由公开）、品牌一句话 |

依赖：已存在 Agent Pack 与 `/help/api-access/agents` 公开页 — **C 期内容应优先链到此路径**。

---

## 12. 附录

### 12.1 相关仓库文档

- Agent Pack 设计：`docs/design/2026-08-11-api-access-agent-pack-design.md`  
- 产品 IA：`docs/design/PRODUCT_IA.md`  
- 多站点：`docs/engineering/MULTI_SITE_IA_INTEGRATION_PLAN_2026-07-14.md`  

### 12.2 GEOHub 本地命令速查

```bash
cd /home/chuan/GEOHub
.venv/bin/geo-seo-hub route --text "…"
.venv/bin/geo-seo-hub diagnose --input <brief.json> --output runs/contextlm
.venv/bin/geo-seo-hub seo      --input <brief.json> --output runs/contextlm
.venv/bin/geo-seo-hub discover --input <brief.json> --output runs/contextlm
```

### 12.3 原始报告路径

- 诊断 Markdown：`/home/chuan/GEOHub/runs/contextlm/run-b664673877e5/report.md`  
- SEO：`…/run-cd08db440bd7/report.md`  
- 机会图：`…/run-ae2f5a18bf60/opportunity-map.json`  
- 问题图：`…/run-ae2f5a18bf60/query-map.json`  

---

## 13. 下一步（默认推荐）

**执行记录（2026-08-11）**：Phase A 工程侧已在 `frontend_next` 落地——A1 全公开页 canonical（首页 / pricing / desktop / legal 族 / help/api-access / agents）、A2 首页 SSR 摘要（决策：SSR，放弃 302）、A3 `/help/api-access` 迁至 `(open)` 组公开、A4 新建 `app/robots.ts` / `app/sitemap.ts` / llms.txt（AI crawler 明示放行）。验收测试：`frontend_next/tests/seo/`。marketing 侧（§7.1）归属 `~/context-os-landing`，本轮未动。

**跟进（2026-08-12）**：已部署 rev `978de4c1+dirty`，生产实测 `/` 339 字 + canonical、`/help/api-access` 644 字 + canonical、`/pricing` 797 字 + canonical、robots/sitemap 200。llms.txt 改走 `app/llms.txt/route.ts`（standalone 不服务 public/）。发现 Cloudflare 托管 robots 块注入 AI bot `Disallow: /`（见 §8 / §10），待 CF 控制台处理。

1. **工程**：~~A1 + A3 + A2~~ 已落地（见执行记录）；下次部署后用 GEOHub diagnose 复跑 5 URL 并归档 run_id。  
2. **内容**：一篇「MCP/Agent 接入」落地 + 一篇中立对比大纲（先中文）。  
3. **运维**：GSC 接入后把本文件 §9 表补上真实基线。  

文档状态随 PR 关闭更新：完成项在 §7 打勾，并追加复跑 run_id。
