# ADR 0001: M1 + M2 先以可运行骨架落地

> Historical ADR. Qdrant references describe the early M1/M2 scope and are superseded as target architecture by [2026-04-26 Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

## 背景

PRD_RUST 覆盖的是完整平台重构，而当前 `context-osv6` 只有前端代码。若直接从真实 PostgreSQL/Qdrant/Rust RAG runtime 全量起步，交付路径会过长，且前端无法尽早联调。

## 决策

第一批仅落 `M1 + M2`，并采用两层策略：

1. 先建立可编译、可启动、可观测的 Rust workspace
2. 用内存态占位服务托住 notebook/document/chat 主链路，对前端暴露稳定协议

## 结果

- 前端可以尽早对齐 `/api/v1/*` 与 SSE 事件
- 后续可把 `app` 内的内存占位逐步替换为 `storage-pg` / `storage-qdrant` / `cache-redis`
- 真实检索未接入前，`degrade_trace` 必须明确标注占位运行时

## 不在本 ADR 范围内

- 真实多租户 RLS
- 真实 Qdrant Dense 召回
- 真实 BM25 / rerank
- Stripe / share / admin
