# Worker Dev Runbook

## 范围

当前 worker 处于 Wave 0 skeleton 阶段，目标不是完成真实 ingestion，而是先固定：

- 文档 ingestion / reindex 任务契约
- worker poll / heartbeat 生命周期
- 文档状态机约束
- 最小 audit / state sink 接口

## 当前组成

- `bins/worker`
  - 启动 worker runtime
  - 读取 heartbeat / poll 间隔
  - 运行 `WorkerRuntime`
- `crates/ingestion`
  - `IngestionTask`
  - `IngestionTaskPayload`
  - `DocumentStateMachine`
  - `TaskSource`
  - `AuditSink`
  - `StateSink`
  - `WorkerRuntime`

## 环境变量

- `AVRAG_WORKER_HEARTBEAT_SECS`
  - 默认 `30`
- `AVRAG_WORKER_POLL_SECS`
  - 默认 `5`

## 运行

```bash
cargo run -p avrag-worker
```

## 任务契约

支持两类任务：

1. `ingest_document`
   - 用于新上传文件的解析、分块、索引写入
2. `reindex_document`
   - 用于手动或系统触发的重建流程

最小公共字段：

- `task_id`
- `kind`
- `org_id`
- `notebook_id`
- `document_id`
- `requested_by`
- `idempotency_key`
- `enqueued_at`
- `payload`

## 文档状态机

当前允许的核心状态迁移：

- `pending -> enqueueing`
- `pending -> queued`
- `enqueueing -> queued`
- `queued -> processing`
- `processing -> completed`
- `processing -> failed`
- `failed -> queued`
- `completed -> queued`

这覆盖了新文档摄取和 reindex 两条主路径。

## 与主线集成点

后续主线需要接入以下实现：

- `TaskSource`
  - 从 PostgreSQL / Redis / 持久队列读取任务
- `AuditSink`
  - 写入 `audit_log`
- `StateSink`
  - 更新 `documents.status`
- 真实执行节点
  - parser
  - chunker
  - summary builder
  - embedding producer
  - sparse / dense index writer

当前 PostgreSQL 模式下已接入：

- `TaskSource`
  - 从 `ingestion_tasks` 认领任务
- `AuditSink`
  - 写入 `audit_log`
- `StateSink`
  - 更新 `documents.status`
- 真实最小执行节点
  - 从 `AVRAG_OBJECT_ROOT` 读取对象文件
  - 写 summary chunk
  - 写 body chunks

仍未接入：

- parser
- advanced chunker
- embedding producer
- sparse / dense index writer

## 本地验证

```bash
cargo test --manifest-path crates/ingestion/Cargo.toml
cargo check -p avrag-worker
```
