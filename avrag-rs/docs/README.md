# 文档索引

> 最后更新：2026-06-13。

## 当前架构

当前产品架构以此文档为准：

- [产品架构基准版（2026-05-12）](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-05-12-architecture-baseline.md)
- [历史文档：当前产品架构（2026-04-26，部分过时）](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md)

摘要：

- `Main Agent` 负责用户交互、记忆、workspace 指代消解、RAG tool planning 与最终回答。
- `RAG API` 是检索服务，不是面向用户的自主 agent。
- `RAG API` 可以运行有边界的模型辅助检索算子，例如三元组抽取、query entity extraction、relation/path rerank、chunk rerank。
- Postgres 是产品控制面。
- Milvus 是目标检索数据面，承载 BM25 sparse、text dense、multimodal dense 与 graph relation retrieval。

## 被覆盖的旧检索描述

仍提到 Qdrant、Tantivy 或 PostgreSQL BM25 作为目标架构的 Markdown 文档已经检索过。处理方式有两类：

- 在原文增加 2026-04-26 说明。
- 作为历史计划/报告保留，其旧检索栈描述仅代表当时实现状态。

除非文档明确说明是在描述当前兼容实现，否则不要把旧 Qdrant/Tantivy 方案视为当前目标。

## 入库 / Ingestion

- [LiteParse + Paddle Jobs 统一入库架构（2026-06-13）](/home/chuan/context-osv6/avrag-rs/docs/liteparse-paddle-ingestion-architecture-2026-06-13.md) — **当前实现真相源**：LiteParse hybrid 探针 + `router/page_routes` 页内分拣 + Paddle Jobs OCR；Office 全格式经 `office-parser-jvm`（doc/docx/xls/xlsx/ppt/pptx）；已知缺口见 §0.1（Brooks M3/M5）。
- [P4 前 MinerU / shadow / 灰度迁移（历史归档）](/home/chuan/context-osv6/avrag-rs/docs/archive/p4-mineru-shadow-migration-historical.md) — 已删除的 `LITEPARSE_*` / `MINERU_*` 开关与跳过的 Shadow/灰度阶段。
- [入库路由讨论纪要（2026-06-10，过程记录）](/home/chuan/context-osv6/avrag-rs/docs/ingestion-routing-discussion-2026-06-10.md) — 讨论过程；**路由结论以上文为准**。
- [历史文档：视觉 PDF 入库与检索改造（2026-06-10）](/home/chuan/context-osv6/avrag-rs/docs/archive/visual-pdf-ingest-requirements-2026-06-10.md) — 已归档；MinerU 旁路/视觉默认方案仅代表 P4 前讨论。

## E2E 与全功能测试

- [**全功能 E2E 测试指南（Agent 执行手册，2026-06-13）**](/home/chuan/context-osv6/avrag-rs/docs/full-functional-e2e-guide.md) — **Agent 单一真相源**：覆盖矩阵、真实文档解析 / 真实 LLM RAG / 真实 Chat / 真实 WebSearch、并行编排、发布门禁、补测 backlog。
- [**E2E 测试分析框架 TEAF（2026-06-13）**](/home/chuan/context-osv6/avrag-rs/docs/e2e-analysis-framework.md) — 基于测试的分层分析：覆盖 / 回归 / 归因 / 稳定性 / 质量五平面；与 [`e2e-test-registry.yaml`](/home/chuan/context-osv6/avrag-rs/docs/e2e-test-registry.yaml) 及 `e2e-analyzer` 对齐。
- [E2E 质量门禁](/home/chuan/context-osv6/avrag-rs/docs/e2e-gates.md) — 分层 pass/fail 语义与 ADR-0008。
- [Product E2E 计划（可执行版）](/home/chuan/context-osv6/avrag-rs/docs/product-e2e-plan.md) — 历史 P0–P14 设计与实施状态。

## Brooks-Lint 当前报告（M15，2026-06-13 post-v7）

- [合并修复计划 v7](/home/chuan/context-osv6/avrag-rs/docs/brooks-merged-fix-plan-2026-06-13-v7.md) — S0–S9 + M15 已完成。
- [架构审计（2026-06-13）](/home/chuan/context-osv6/avrag-rs/docs/brooks-architecture-audit-2026-06-13.md) — **99/100**；test-kit/dev-dep/NotebookStore 卫生项已核销。
- [PR 审查（2026-06-13 v7）](/home/chuan/context-osv6/avrag-rs/docs/brooks-pr-review-2026-06-13-v7.md) — **98/100**；v6 blocker 已闭环。
- [测试质量审查（2026-06-13）](/home/chuan/context-osv6/avrag-rs/docs/brooks-test-quality-review-2026-06-13.md) — **99/100**。
- [技术债评估（2026-06-13）](/home/chuan/context-osv6/avrag-rs/docs/brooks-tech-debt-assessment-2026-06-13.md) — **99/100**；S9 核销 desktop/share 跨报告矛盾。

旧版 Brooks 报告放在 `docs/archive/`（含 [PR v6](/home/chuan/context-osv6/avrag-rs/docs/archive/brooks-pr-review-2026-06-13-v6.md)、[架构 v5](/home/chuan/context-osv6/avrag-rs/docs/archive/brooks-architecture-audit-2026-06-13-v5.md) 等）。

## 历史与外部资料

以下目录不是当前产品架构真相源：

- `/home/chuan/context-osv6/archive/`
- `/home/chuan/context-osv6/awesome-design-md/`
- `/home/chuan/context-osv6/node_modules/`
- `/home/chuan/context-osv6/.worktrees/`
- `/home/chuan/context-osv6/memory/` 日志笔记
- 生成的视觉回归或测试输出目录

这些目录仅作为参考保留，本次 2026-04-26 文档清理没有重写它们。
