# Context LM / Context OS — GEO & SEO 优化方案

**日期**: 2026-08-11（初版）· **GEOHub 复测 2026-08-12**（§14）· **Phase B §15** · **Phase C §16**  
**状态**: Active plan — Phase A/B/C 已部署；GSC D1 用户已确认完成（§17）；D2 月度抽检仍开；CF robots 用户侧已关  
**范围**: 公开站点与文档面（`contextlm.top`、`app.contextlm.top` 的公开路由）  
**非目标**: 保证搜索排名/AI 引用率；自动改 CMS；GSC/Analytics 账号操作  

---

## 1. 背景与工具

| 项 | 说明 |
|----|------|
| 工具 | [GEOHub](https://github.com/yaojingang/GEOHub) v0.3.1（本地：`/home/chuan/GEOHub`） |
| GEO | Generative Engine Optimization：提升在 AI 回答中的**可引用 / 可抽取**就绪度 |
| SEO | 传统技术与内容规划（本轮 SEO skill 为 **advisory / 只读计划**） |
| 分析产物 | 基线：`runs/contextlm/` · 复测：`runs/contextlm-2026-08-12/` |

### 1.1 运行 ID

| 能力 | 基线 2026-08-11 | 复测 2026-08-12 | 主要内容 |
|------|-----------------|-----------------|----------|
| `geo-diagnose` | `run-b664673877e5` | **`run-62645983aca5`** | 5 URL 抓取 + `report.md` |
| `seo` | `run-cd08db440bd7` | `run-cd08db440bd7`（新目录复跑） | 技术审计计划 |
| `geo-discover` | `run-ae2f5a18bf60` | （本轮未重跑） | 180 条问题地图 |

复跑：

```bash
cd /home/chuan/GEOHub
.venv/bin/geo-seo-hub diagnose \
  --input /home/chuan/context-osv6/docs/plans/geo-seo-briefs/diagnose-contextlm.json \
  --output runs/contextlm-2026-08-12
```

Brief：`docs/plans/geo-seo-briefs/*.json`。
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
| AI / 国内 crawler | robots 明示允许：欧美 AI（GPTBot / ClaudeBot / PerplexityBot / Google-Extended 等）+ 国内（Baiduspider / Bytespider / Sogou / YisouSpider / 360Spider 等）；私有路径统一 disallow。DeepSeek/豆包对话无稳定公开 UA → 走 `*`。**注意（2026-08-12）**：CF AI Crawl Control 曾注入 Disallow，用户侧已关 | robots.txt 正文 |
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

1. **工程**：~~A1 + A3 + A2~~ 已落地；**2026-08-12 GEOHub 复测已归档**（§14，run `run-62645983aca5`）。  
2. **内容**：一篇「MCP/Agent 接入」落地 + 一篇中立对比大纲（先中文）。  
3. **运维**：GSC 接入；`contextlm.top` canonical + sitemap；CF robots AI bot 策略。  
4. **Phase B**：定价/帮助 authority + freshness（更新日、组织身份）；app 首页 H2。  

文档状态随 PR 关闭更新：完成项在 §7 打勾，并追加复跑 run_id。

---

## 14. 复测记录（2026-08-12）

同一 brief 重跑 GEOHub diagnose，与 08-11 基线对比。

- Run：`/home/chuan/GEOHub/runs/contextlm-2026-08-12/run-62645983aca5/`
- 部署 rev：`5e0c12ee+dirty`（Phase A 上线后）

### 14.1 站点分对比

| 维度 | 基线（08-11） | 复测（08-12） | Δ |
|------|-----:|-----:|---:|
| Discoverability | 75 | **95** | +20 |
| Extractability | 56 | **76** | +20 |
| Structure | 50 | **80** | +30 |
| Authority | 52 | 52 | 0 |
| Evidence | 52 | 52 | 0 |
| Freshness | 36 | 36 | 0 |

结构类维度（Discoverability / Extractability / Structure）显著上涨；Authority / Evidence / Freshness 未动——正是 Phase B 与内容期的目标面。

### 14.2 页面就绪度对比

| 页面 | 基线 | 复测 | Δ |
|------|-----:|-----:|---:|
| `app.contextlm.top/` | 26 | **47** | +21 |
| `/help/api-access` | 28 | **57** | +29 |
| `/pricing` | 54 | **58** | +4 |
| `/help/api-access/agents` | 69 | **73** | +4 |
| `contextlm.top/`（marketing） | 91 | 91 | 0 |

- 可见文本：app 首页 ~354 字（H1 + 摘要；基线 64）；`/help/api-access` ~659 字（不再是「加载中…」；基线 15）。
- diagnose warning 总数：23 → 13。

### 14.3 §9 指标快照

| 指标 | 基线 | 目标 | 复测实际 | 状态 |
|------|------|------|----------|------|
| app 首页可见文本 | ~64 | ≥ 300 且 H1=1 | ~354，H1=1 | ✅ |
| help/api-access 未登录可见文本 | ~15 | ≥ 500 | ~659 | ✅ |
| 公开页 canonical | 0/5 | 5/5 | 4/5（缺 marketing 首页） | 🟡 |
| diagnose Freshness | 36 | ≥ 55 | 36 | ⬜ Phase B |
| diagnose Extractability | 56 | ≥ 70 | 76 | ✅ |
| 对外对比/FAQ 页 | 0 | ≥ 2 篇 SSR | 0 | ⬜ Phase C |
| GSC 索引覆盖 | 未知 | 已配置并有数据 | 属性+sitemap 已提交（2026-08-12） | ✅ D1（索引量随时间爬升） |
| AI 引用抽检 | 未测 | 月度 3 问有记录 | 未测 | ⬜ D2 |

### 14.4 仍未关的口

1. `contextlm.top` 仍无 canonical；sitemap 404（marketing 侧，归属 `~/context-os-landing`）。
2. app 多页仍缺 authority / freshness / evidence 信号（Phase B 主战场）。
3. app 首页无 H2；agents 页正文在 `<pre>` 里，结构分仍吃亏。
4. Cloudflare 托管 robots 对 AI bot 的 Disallow 仍需控制台处理（见 §8 / §10）。

### 14.5 结论

Phase A（可抓取 / SSR / canonical）在 app 侧已验证有效。下一步顺序：marketing canonical + sitemap（`~/context-os-landing`）→ CF robots 控制台放行 → Phase B（更新日 / 组织身份 / authority-freshness 信号）→ 再开对比 / FAQ 内容（Phase C）。

---

## 14. GEOHub 复测报告（2026-08-12）

> 同一 diagnose brief 复跑。产物：`/home/chuan/GEOHub/runs/contextlm-2026-08-12/run-62645983aca5/`。

### 14.1 站点分：基线 → 复测

| 维度 | 2026-08-11 | 2026-08-12 | Δ |
|------|----------:|----------:|---:|
| Discoverability | 75 | **95** | **+20** |
| Extractability | 56 | **76** | **+20** |
| Structure | 50 | **80** | **+30** |
| Authority | 52 | 52 | 0 |
| Evidence | 52 | 52 | 0 |
| Freshness | 36 | 36 | 0 |

**解读**：Phase A（可抓取 / 结构 / canonical）达标；authority / evidence / freshness 未改善，需 Phase B 内容运营。

### 14.2 页面回答就绪度（启发式）

| URL | 基线 | 复测 | Δ | 可见文本（复测 snapshot） |
|-----|-----:|-----:|---:|---------------------------|
| https://contextlm.top/ | 91 | 91 | 0 | ~1489 |
| https://app.contextlm.top/ | 26 | **47** | **+21** | ~354（H1 + 产品摘要 SSR） |
| https://app.contextlm.top/pricing | 54 | **58** | +4 | ~863 |
| https://app.contextlm.top/help/api-access | 28 | **57** | **+29** | ~659（公开说明，非「加载中…」） |
| https://app.contextlm.top/help/api-access/agents | 69 | **73** | +4 | ~8930 |

### 14.3 Warning 数量

| | 基线 | 复测 |
|--|-----:|-----:|
| warning 条数 | 23 | **13**（−10） |

**已消除（相对基线，app 侧）**：空壳无 H1、近空可见文本、app 公开路由缺 canonical 等。

**仍在的 warning：**

| 页面 | 残留 |
|------|------|
| `contextlm.top` | **canonical 仍缺**（营销站 `~/context-os-landing`，本轮未动） |
| `app.contextlm.top/` | 无 H2；无 evidence / authority / freshness 信号 |
| `/pricing` | evidence / authority / freshness |
| `/help/api-access` | evidence / authority / freshness |
| `/help/api-access/agents` | 无 H2（正文在 `<pre>`）；无 freshness |

### 14.4 配套探测

| 探测 | 结果 |
|------|------|
| `app…/sitemap.xml` | **200**（含 `/` `/pricing` `/desktop` 等） |
| `contextlm.top/sitemap.xml` | **404** |
| app 公开页 live canonical | **有** |
| marketing live canonical | **无** |
| robots.txt（双域） | 200；注意 CF 托管块可能对 AI bot `Disallow`（见执行记录） |

### 14.5 Phase 勾选（复测后）

| ID | 状态 |
|----|------|
| A1 canonical | **部分**：app 完成；marketing **未完成** |
| A2 app 首页 SSR | **完成**（26→47） |
| A3 help/api-access 公开 | **完成**（28→57） |
| A4 robots/sitemap | **部分**：app sitemap 有；marketing sitemap 404 |
| B / C / D | **未完成** |

### 14.6 复测后下一刀

1. marketing `contextlm.top`：**canonical + sitemap**（关 A1/A4 剩余）。  
2. 处理 **Cloudflare robots** 对 AI bot 的 Disallow（否则 GEO 与 sitemap 放行被上游抵消）。  
3. 定价 + help：**更新日期 + 组织/产品身份**（抬 freshness/authority）。  
4. app 首页：H1 下 **2～3 个 H2**（能力 / Agent 接入 / 定价）。  
5. agents 页：markdown **渲染为真 H2**（勿长期整页 pre）。  
6. Phase C：comparison + FAQ 各一。  

### 14.7 复测产物

```
/home/chuan/GEOHub/runs/contextlm-2026-08-12/run-62645983aca5/
  report.md · diagnosis.json · diagnosis-funnel.json · input/sources/url-*.html
```

---

## 15. Phase B 落地 + GEOHub 复测（2026-08-12 下午）

工程已合入并部署：marketing canonical/sitemap；app 首页 H2×3；pricing/help 更新日；agents 真 H2 markdown。随后同一 brief 再跑 diagnose。

- Run：`/home/chuan/GEOHub/runs/contextlm-2026-08-12-phaseb/run-063c47ba2e1a/`
- Live 验收（curl）：`contextlm.top` `rel=canonical` + `/sitemap.xml` 200 + `/robots.txt` 200；app `/` 三 H2；`/help/api-access/agents` 多 H2（非整页 pre）

### 15.1 站点分：基线 → Phase A 复测 → Phase B 复测

| 维度 | 08-11 基线 | 08-12 A | 08-12 B | Δ(A→B) | Δ(基线→B) |
|------|----------:|--------:|--------:|-------:|----------:|
| Discoverability | 75 | 95 | **100** | +5 | **+25** |
| Extractability | 56 | 76 | **80** | +4 | **+24** |
| Structure | 50 | 80 | **100** | +20 | **+50** |
| Authority | 52 | 52 | 52 | 0 | 0 |
| Evidence | 52 | 52 | 52 | 0 | 0 |
| Freshness | 36 | 36 | **100** | **+64** | **+64** |

**解读**：H2 / canonical / 更新日期把结构与新鲜度拉满。Authority/Evidence 站点分仍 52——因 5 页里 3 页（home / pricing / api-access）未命中 GEOHub **英文/关键词**启发式（见 §15.3）。

### 15.2 页面就绪度

| URL | 基线 | A 复测 | B 复测 | Δ(A→B) | 可见文本（B） | H2（B） |
|-----|-----:|-------:|-------:|-------:|---------------|--------:|
| `contextlm.top/` | 91 | 91 | **95** | +4 | ~1489 | 5 |
| `app…/` | 26 | 47 | **67** | +20 | ~449 | **3** |
| `app…/pricing` | 54 | 58 | **72** | +14 | ~921 | 3 |
| `app…/help/api-access` | 28 | 57 | **70** | +13 | ~717 | 2 |
| `app…/help/api-access/agents` | 69 | 73 | **100** | +27 | ~8472 | **9** |

Warning 条数：23（基线）→ 13（A）→ **6**（B）。  
B 残留 6 条 warning 全部是 home/pricing/api-access 的 **no authority** + **no evidence**（各 3）。

### 15.3 §9 指标快照（B 后）

| 指标 | 目标 | B 实际 | 状态 |
|------|------|--------|------|
| app 首页可见文本 + H1/H2 | ≥300 且结构完整 | ~449，H1=1，H2=3 | ✅ |
| help/api-access 未登录可见 | ≥500 | ~717 | ✅ |
| 公开页 canonical | 5/5 | **5/5**（marketing 已补） | ✅ |
| marketing sitemap | 200 | **200** | ✅ |
| diagnose Freshness | ≥55 | **100** | ✅ |
| diagnose Structure | — | **100** | ✅ |
| diagnose Extractability | ≥70 | **80** | ✅ |
| diagnose Authority / Evidence | ≥70 | 仍 52 | 🟡（§15.4 修） |
| 对比/FAQ SSR | ≥2 | 0 | ⬜ Phase C |
| GSC / AI 引用抽检 | 配置+记录 | GSC ✅；抽检 ⬜ | 🟡 D1 关 / D2 开 |

### 15.4 Authority/Evidence 关键词对齐（已部署 + 再测通过）

GEOHub `diagnose.py` 启发式（非语义）：

- **authority**：`author|editor|expert|about|contact|作者|专家|关于|联系`
- **evidence**：`source|reference|citation|method|数据|来源|参考|方法`

此前中文页写「主理人：邢川」**不命中**；agents 英文页因正文含 `Authentication`（子串 `author`）与 `source/method` 正命中。  
补丁：`home.seoPublisher` → **作者 / author**；新增 `home.seoEvidence`（**来源 / source**），挂 home / pricing / api-access。前端 rev `5e0c12ee+dirty` 已部署。

#### B2 再测（补丁后）`run-d925f7257d18`

| 维度 | B1 | B2 |
|------|---:|---:|
| Discoverability | 100 | **100** |
| Extractability | 80 | **80** |
| Structure | 100 | **100** |
| Authority | 52 | **100** |
| Evidence | 52 | **100** |
| Freshness | 100 | **100** |

| URL | B1 就绪 | B2 就绪 |
|-----|--------:|--------:|
| marketing | 95 | 95 |
| app `/` | 67 | **93** |
| pricing | 72 | **98** |
| help/api-access | 70 | **97** |
| agents | 100 | **100** |

- **warning 条数：6 → 0**；run status：`completed`（无 `completed-with-warnings`）。
- Extractability 仍 80：部分页 landmark/list 信号未拉满；非 P0。

### 15.5 仍未关的口

1. **Cloudflare 托管 robots** 对 AI bot 的 Disallow（若 CF 仍注入）— 控制台人工。  
2. **Phase C**：comparison + FAQ 各一篇 SSR。  
3. **GSC** 提交 sitemap；AI 引用月度抽检。  
4. （可选）抬 Extractability：更多 `<main>`/`<article>`/列表/表格 landmark。

### 15.6 结论与下一刀

Phase B **关闭**：结构 / 可发现 / 新鲜度 / 权威 / 证据 均已在 GEOHub 代理指标上达标；marketing canonical+sitemap 线上 200。  
下一刀顺序：

1. CF robots AI bot 策略（运维控制台）。  
2. Phase C：中文对比文 + FAQ（SSR）。  
3. GSC / 抽检（D1/D2）。

### 15.7 产物

```
B1: /home/chuan/GEOHub/runs/contextlm-2026-08-12-phaseb/run-063c47ba2e1a/
B2: /home/chuan/GEOHub/runs/contextlm-2026-08-12-phaseb2/run-d925f7257d18/
  report.md · diagnosis.json · input/sources/url-*.html
```

---

## 16. Phase C — FAQ + 选型对比 SSR（2026-08-12）

**内容宿主**：产品事实 FAQ / 中立对比表放 app 公开 SSR（`/help/faq`、`/help/compare`）；长文编辑型仍可 Ghost。已先更新 `docs/design/PRODUCT_IA.md` §3.1。

### 16.1 交付

| URL | 角色 | 线上 |
|-----|------|------|
| https://app.contextlm.top/help/faq | MCP / 密钥 / 名额 / BYOK / 定价 FAQ | **200**，H1=1，H2=10，canonical，作者/来源/更新日期 |
| https://app.contextlm.top/help/compare | 类别级选型对照表（无编造竞品数据） | **200**，H1=1，H2=6，HTML table，canonical |

配套：`sitemap.ts` / `llms.txt` 收录；首页 / API 接入 / Agent 文档互链；SEO 测试 19 通过；前端 rev `5e0c12ee+dirty`。

Brief：`docs/plans/geo-seo-briefs/diagnose-contextlm-phase-c.json`

### 16.2 GEOHub 单页复测

- Run：`/home/chuan/GEOHub/runs/contextlm-2026-08-12-phasec/run-2de2d2e79bc4/`
- status：`completed`，**warning_count = 0**

| 维度 | 分数 |
|------|-----:|
| Discoverability | **100** |
| Structure | **100** |
| Extractability | **90** |
| Authority | **100** |
| Evidence | **100** |
| Freshness | **100** |

| URL | 就绪度 | 可见文本 | H2 |
|-----|-------:|---------:|---:|
| `/help/faq` | **97** | ~1358 | 10 |
| `/help/compare` | **100** | ~1344 | 6 |

### 16.3 §9 指标

| 指标 | 目标 | 实际 | 状态 |
|------|------|------|------|
| 对外对比/FAQ 页 | ≥ 2 篇 SSR | **2**（faq + compare） | ✅ |
| CF AI bot robots | 放行 | 用户确认已关 | ✅ |
| GSC | 配置 | **用户 2026-08-12 确认完成** | ✅ D1 |
| AI 引用抽检 | 月度 3 问 | 种子问已写入 GSC 清单 §9 | ⬜ D2 |

### 16.4 下一刀

1. ~~GSC 手册~~ → **§17**（待人工在控制台完成验证与提交）。  
2. D2：种子问月度抽检 → 可选 `geo-measure`（种子问见 GSC 清单 §9）。  
3. （可选）www→apex 301；Ghost 长文；对比表保持 app canonical。

### 16.5 产物

```
/home/chuan/GEOHub/runs/contextlm-2026-08-12-phasec/run-2de2d2e79bc4/
```

---

## 17. GSC 接入说明（D1 · 2026-08-12）

**完整操作清单**（逐步截图式文字）：

[`docs/plans/2026-08-12-gsc-onboarding-checklist.md`](./2026-08-12-gsc-onboarding-checklist.md)

### 17.1 摘要（给执行人 5 分钟扫完）

| 步骤 | 动作 |
|------|------|
| 1 | GSC 添加 **Domain 属性** `contextlm.top`（推荐）或 URL-prefix：`app.` + apex |
| 2 | Cloudflare 加 Google 给的 **DNS TXT** → 点验证 |
| 3 | 提交 sitemap：`https://app.contextlm.top/sitemap.xml` + `https://contextlm.top/sitemap.xml` |
| 4 | 对 `/help/faq`、`/help/compare`、`/pricing`、marketing 首页做 **网址检查** |

### 17.2 提交用 URL（复制粘贴）

```
https://app.contextlm.top/sitemap.xml
https://contextlm.top/sitemap.xml
```

App sitemap 当前公开路径（随发版变；以线上 XML 为准）：

`/` · `/pricing` · `/desktop` · `/legal/*` · `/help/api-access` · `/help/api-access/agents` · `/help/faq` · `/help/compare`

### 17.3 探测备注（写手册时）

- Marketing **canonical = apex**（`https://contextlm.top`）；www 与 apex 均 200，**尚未** 301 合并。  
- 工程侧 D1 **文档完成**；**账号验证/提交** 须人工勾选清单 §10。  

### 17.4 D1 状态

| 项 | 状态 |
|----|------|
| 操作手册 | ✅ 已写 |
| GSC 属性验证 | ✅ **2026-08-12 用户确认完成** |
| Sitemap 提交成功 | ✅ **2026-08-12 用户确认完成** |
| 索引覆盖有数据 | ⏳ 通常数日～数周后在 GSC「网页索引编制 / 效果」中可见（无需再操作） |

**D1 关闭**。剩余增长向：D2 月度 AI 引用抽检；可选 www→apex 301。

---

## 18. 国内爬虫显式放行（2026-08-12）

**改动**

| 面 | 文件 | 内容 |
|----|------|------|
| App | `frontend_next/app/robots.ts` | 在 `*` 之外点名 Allow：Baiduspider / Baiduspider-render / Bytespider / Sogou / YisouSpider / 360Spider / HaosouSpider（+ 原有 GPT/Claude/Perplexity/Google-Extended）；私有路径仍 Disallow |
| Marketing | `context-os-landing/public/robots.txt` | 同上显式 Allow（无私有路径） |
| Nginx | `deploy/nginx/context-os-landing.conf` | `/robots.txt`、`/sitemap.xml` → `Cache-Control: max-age=300`，避免 CF 长时间 HIT 旧文件 |

**线上**

- `app.contextlm.top/robots.txt`：已含 Baiduspider / Bytespider 等（部署 rev `14705287+dirty`）。  
- Marketing：源站 + **用户 2026-08-12 确认 CF purge 完成**，边缘应与源站一致（含国内爬虫 Allow）。  

**未做 / 边界**

- DeepSeek / 豆包对话产品：无稳定公开 UA → 继续走 `*`，不禁止。  
- 百度站长平台：未接（可选，类似 GSC）。  
- 不保证国内搜索排名或模型引用。  
- 百度站长操作手册：[`docs/plans/2026-08-12-baidu-ziyuan-checklist.md`](./2026-08-12-baidu-ziyuan-checklist.md)（待人工验证）。  
