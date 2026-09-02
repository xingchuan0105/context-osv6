# GEO/SEO 测量闭环 Runbook（D2 / D3）

**日期**: 2026-09-01 · **状态**: Active · **上游**: `docs/plans/2026-08-11-contextlm-geo-seo-optimization-plan.md` §6 Phase D
**范围**: 公开站点与文档面（`contextlm.top`、`app.contextlm.top` 公开路由）
**非目标**: 保证排名/AI 引用率；自动抓取平台数据（观测为人工离线记录）

---

## D2 — 月度 AI 引用抽检（人工，每月一次）

### 种子问题（3 个，固定不变，便于跨月对比）

| query_id | 意图 | 问题 |
|----------|------|------|
| q1 | brand | Context OS 是什么？ |
| q2 | category | 个人 AI 知识库怎么选？ |
| q3 | compare | Context OS 和 Notion AI 有什么区别？ |

### 平台与记录

- 平台：ChatGPT（openai）、Perplexity（perplexity）；匿名/无痕会话，不登录个人账号。
- 每问记录：`answer_text`（回答正文）、`citations`（引用 URI 与位置）、`observed_at`、`locale`、`session_policy`。
- 记录格式：`docs/plans/geo-seo-briefs/observation-bundle-YYYY-MM.json`（模板见同目录 `observation-bundle-template.json`）。

### 执行

```bash
cd /home/chuan/GEOHub
.venv/bin/geo-seo-hub measure \
  --input /home/chuan/context-osv6/docs/plans/geo-seo-briefs/observation-bundle-YYYY-MM.json \
  --output runs/contextlm-measure
```

验收：`measurement-report.json` 生成；记录「是否提到品牌 / 是否引用本站 URI / 引用位置」。

### 判读（不设排名 KPI）

- 品牌被提到 → 记录；未被提到 → 回看 q1 对应公开页（首页 / FAQ）的 answer-readiness。
- 引用本站 URI → 记录来源页；未引用 → 回看该页 canonical / 结构化数据 / 可见文本。
- 连续 2 个月无变化 → 回到 `diagnose` 单页复检，不盲目加内容。

---

## D3 — 每发一页复跑 diagnose（发布后）

### 单页 brief

`docs/plans/geo-seo-briefs/diagnose-contextlm.json` 的 `target_urls` 已含 5 个核心 URL。新增/改动公开页时，把该页 URL 加入 `target_urls`（或新建单页 brief），再跑：

```bash
cd /home/chuan/GEOHub
.venv/bin/geo-seo-hub diagnose \
  --input /home/chuan/context-osv6/docs/plans/geo-seo-briefs/diagnose-contextlm.json \
  --output runs/contextlm-YYYY-MM-DD
```

验收：对比 `report.md` 的页面就绪度与 warning 列表；归档 run_id 到主方案 §14 复测记录。

### 触发条件

- 新增公开 SSR 页（FAQ / comparison / landing / article）。
- 修改公开页的 H1 / 正文 / 结构化数据 / canonical。
- 纯样式或登录后路由改动**不触发**。

---

## 记录位置

- 观测 bundle：`docs/plans/geo-seo-briefs/observation-bundle-*.json`
- 诊断 run：`/home/chuan/GEOHub/runs/contextlm-*/`
- 主方案复测表：`docs/plans/2026-08-11-contextlm-geo-seo-optimization-plan.md` §14
