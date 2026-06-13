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

- [LiteParse + Paddle Jobs 统一入库架构（2026-06-13）](/home/chuan/context-osv6/avrag-rs/docs/liteparse-paddle-ingestion-architecture-2026-06-13.md) — **当前实现真相源**：LiteParse hybrid 探针 + `router/page_routes` 页内分拣 + Paddle Jobs OCR；Excel-only Office；已知缺口见 §0.1（Brooks M3/M4/M5）。
- [P4 前 MinerU / shadow / 灰度迁移（历史归档）](/home/chuan/context-osv6/avrag-rs/docs/archive/p4-mineru-shadow-migration-historical.md) — 已删除的 `LITEPARSE_*` / `MINERU_*` 开关与跳过的 Shadow/灰度阶段。
- [入库路由讨论纪要（2026-06-10，过程记录）](/home/chuan/context-osv6/avrag-rs/docs/ingestion-routing-discussion-2026-06-10.md) — 讨论过程；**路由结论以上文为准**。
- [历史文档：视觉 PDF 入库与检索改造（2026-06-10）](/home/chuan/context-osv6/avrag-rs/docs/archive/visual-pdf-ingest-requirements-2026-06-10.md) — 已归档；MinerU 旁路/视觉默认方案仅代表 P4 前讨论。

## Brooks-Lint 当前报告

- [架构审计（2026-06-13 v6）](/home/chuan/context-osv6/avrag-rs/docs/brooks-architecture-audit-2026-06-13.md) — 当前架构健康分 87/100；生产依赖图无环，v5 主要 Warning（`app-core` Redis adapter、`app-chat` 千行文件、`share` axum 泄漏）已核销；剩余风险集中在结构性卫生（死的 `avrag-test-kit`、未用且制造 Cargo 环的 dev-dep、`NotebookStore` 双路径、文档漂移）。
- [PR 审查（2026-06-13 v6）](/home/chuan/context-osv6/avrag-rs/docs/brooks-pr-review-2026-06-13-v6.md) — 当前 PR 健康分 48/100；blocker 为 Git 变更集分裂与 smoke runner 清单解析失效。
- [测试质量审查（2026-06-13 round6）](/home/chuan/context-osv6/avrag-rs/docs/brooks-test-quality-review-2026-06-13.md)
- [技术债评估（2026-06-13 v6）](/home/chuan/context-osv6/avrag-rs/docs/brooks-tech-debt-assessment-2026-06-13.md)

旧版 Brooks 报告放在 `docs/archive/`；本轮已归档 [架构审计 v5](/home/chuan/context-osv6/avrag-rs/docs/archive/brooks-architecture-audit-2026-06-13-v5.md)；先前批次归档了 [PR 审查 v5](/home/chuan/context-osv6/avrag-rs/docs/archive/brooks-pr-review-2026-06-13-v5.md) 与 [架构审计 v4](/home/chuan/context-osv6/avrag-rs/docs/archive/brooks-architecture-audit-2026-06-13-v4.md)。

## 历史与外部资料

以下目录不是当前产品架构真相源：

- `/home/chuan/context-osv6/archive/`
- `/home/chuan/context-osv6/awesome-design-md/`
- `/home/chuan/context-osv6/node_modules/`
- `/home/chuan/context-osv6/.worktrees/`
- `/home/chuan/context-osv6/memory/` 日志笔记
- 生成的视觉回归或测试输出目录

这些目录仅作为参考保留，本次 2026-04-26 文档清理没有重写它们。
