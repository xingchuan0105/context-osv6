# GEOHub 复检（2026-08-15）— 新发现缺口修复方案

**日期**: 2026-08-15
**状态**: 待执行
**父方案**: [2026-08-11-contextlm-geo-seo-optimization-plan.md](./2026-08-11-contextlm-geo-seo-optimization-plan.md)（Phase A/B/C/D1 已完成）
**范围**: `frontend_next`（app.contextlm.top 公开面）；marketing 侧归属 `~/context-os-landing`（本轮不含）

## 0. 本轮诊断方法

用 GEOHub v0.5.0（`geo-seo-hub`，本机 `/tmp/GEOHub`）做了 brand 级 + 3 个 page 级诊断。
站点在 Cloudflare 后拦截默认 UA，改用浏览器 UA 抓取快照喂给 GEOHub 离线解析（`provided` 来源）。

| 诊断 | scope | 运行目录 |
|------|-------|----------|
| Context OS / ContextLM 品牌（5 页） | brand | `/tmp/GEOHub/runs/brand/run-5139a4ecffde` |
| 首页 /（site） | site | `/tmp/GEOHub/runs/run-f8d180d4b4c1` |
| 定价 /pricing | page | `/tmp/GEOHub/runs/pages/run-df0f86cc73b6` |
| 客户端 /desktop | page | `/tmp/GEOHub/runs/pages/run-dd32f6beac0a` |
| 帮助 /help | page | `/tmp/GEOHub/runs/pages/run-65865a90c8fa` |

**结论**：父方案 Phase A/B/C 已把多数公开页做到高就绪（/pricing 98、/help/faq 97、/help/compare 100）。
本轮新增**不在父方案完成清单内**的缺口 4 项（见下）。

---

## 1. 缺口 G1（高）— `/help` 落地是空壳客户端页

**现状**：`/help` 由 `app/(app)/help/page.tsx` 提供（登录壳内的帮助索引），未登录时只渲染
「加载中…」（快照可见文本 15–40 字符），无 canonical、无 H1/H2、Structure=0/100、就绪度 28/100。
而真实公开帮助在 `(open)` 组：`/help/faq`（97）、`/help/compare`（100）、`/help/api-access`（97）。
`/help` 也未进 sitemap（`app/sitemap.ts` PUBLIC_PATHS 不含裸 `/help`）。

**修复**：让 `/help` 落在公开帮助面，避免空壳被爬虫/AI 抓取。

| 选项 | 做法 | 取舍 |
|------|------|------|
| G1a（推荐） | `app/(app)/help/page.tsx` 改为对未登录访问 `redirect("/help/faq")`（307/308） | 最小改动；把公开帮助锚定到已就绪的 faq；不动登录壳逻辑 |
| G1b | 新建 `app/(open)/help/page.tsx` 公开 SSR 索引（目录 + 绝对链到 faq/compare/api-access） | 提供独立 landing，但多一个需维护的公开面 |

**验收**：`curl -I /help` 返回 3xx 指向 `/help/faq`；GEOHub diagnose 对 `/help` 不再产出近空正文。

---

## 2. 缺口 G2（高）— 全站无 JSON-LD 结构化数据

**现状**：`grep -r "application/ld+json" app` 无结果。diagnose 的 `structured-data-validity`=0/100（fail，warning）。
这是父方案未覆盖的新缺口（父方案只做了 canonical/robots/sitemap/SSR 正文）。

**修复**（在 SSR 端注入，符合「SSR 优先」原则）：

| 位置 | 注入 Schema |
|------|-------------|
| `app/layout.tsx`（根布局，server 组件） | `Organization` + `WebSite`（品牌实体 ContextLM / 产品 Context OS） |
| `app/(open)/help/faq/page.tsx` | `FAQPage`（mainEntity 映射现有 Q&A） |
| `app/(marketing)/desktop/page.tsx` | `SoftwareApplication`（/desktop 下载产品） |
| `app/(marketing)/pricing/page.tsx` | `OfferCatalog` 或结构化档位（可选，配合可见表格） |

注入方式：Next.js server 组件内 `<script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(schema) }}`。
品牌实体与 `site-map.ts`（Context OS / ContextLM）保持一致，避免实体不一致。

**验收**：`grep -rl "application/ld+json" app` 命中；diagnose `structured-data-validity` 转 pass。

---

## 3. 缺口 G3（中）— 客户端页 `/desktop` 缺 authority / freshness

**现状**：`app/(marketing)/desktop/page.tsx` 只导出 metadata + 渲染 `<DesktopPageClient />`；
客户端组件正文无作者/组织、无更新日期 → Authority=20/100、Freshness=20/100（父方案 Phase B 覆盖了
首页/pricing/help，未覆盖 /desktop）。

**修复**：仿照父方案已有的 `home.seoPublisher` / `home.seoEvidence` 模式（§15.4），在
`desktop/page.tsx` 用 **server 组件**补一行可见的权威 + 新鲜度信息：

- 作者 / 组织：`主理人：邢川 · 品牌 ContextLM`（命中 GEOHub authority 关键词 `作者|主理人|about`）
- 更新日期：`页面更新：YYYY-MM-DD`（命中 freshness 关键词）
- 来源链：`来源：/help/api-access · /pricing · /help/faq`（命中 evidence 关键词）

同时给 metadata 加 `dateModified`。不要写进 `"use client"` 的客户端组件（爬虫看不到）。

**验收**：diagnose 对 `/desktop` 的 Authority / Freshness ≥ 70。

---

## 4. 缺口 G4（中）— 品牌差异化事实缺失

**现状**：brand 诊断显示 Brand Fact Coverage 83/100，唯一 **input_gap** 是「differentiation（差异化）」——
首页/定价/帮助等供给源里没有明确的、可引用的差异化声明（Priority 90）。

**修复**：在首页 `home-client.tsx`（H1 摘要区）加一句**有依据**的差异化定位，并链到已有的中立对比页
`/help/compare`（Phase C 已交付）。遵守父方案硬约束「禁止无证据编造竞品数据」——差异化只写产品自身
定位（能力组合），不做竞品数值对比。

示例（定位句，非竞品数据）：
> 与通用笔记/知识库不同，Context OS 把文档入库后即可按库检索问答，并可将库**开放给访客或外接
> Agent（MCP/API）**——同一份知识同时服务人、访客与 AI（见对比 /help/compare）。

**验收**：brand 诊断 differentiation 从 input_gap 转为 covered；GEOHub brand 复检无 warning。

---

## 5. 父方案遗留/可选（不在本轮工程范围）

| 项 | 状态 |
|----|------|
| D2 月度 AI 引用抽检（3 个种子问 → `geo-measure`） | 进行中（运营流程） |
| marketing www→apex 301 合并 | 可选（归属 `~/context-os-landing`） |
| 抬 Extractability（更多 `<article>`/列表/表格 landmark） | 可选（父方案 §15.5，非 P0） |

---

## 6. 建议执行顺序

G1（/help 空壳，最快见效）→ G2（JSON-LD，富结果/结构化）→ G3（/desktop 权威）→ G4（品牌差异化文案）。

每完成一项，对对应 URL 用 GEOHub diagnose 单页复跑归档（沿用父方案 D3 惯例）。

> 解释边界：以上均基于快照的有界启发式，不代表实时 AI 平台召回/排名/引用份额；不采集平台数据。
