# 文档索引

> 最后更新：2026-04-26。

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

## 历史与外部资料

以下目录不是当前产品架构真相源：

- `/home/chuan/context-osv6/archive/`
- `/home/chuan/context-osv6/awesome-design-md/`
- `/home/chuan/context-osv6/node_modules/`
- `/home/chuan/context-osv6/.worktrees/`
- `/home/chuan/context-osv6/memory/` 日志笔记
- 生成的视觉回归或测试输出目录

这些目录仅作为参考保留，本次 2026-04-26 文档清理没有重写它们。
