# Worker Dev Runbook

## 范围

当前 worker 已接入 PostgreSQL 任务、解析、分块、embedding 与 Milvus indexing。本文固定：

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
- `MILVUS_URL`
  - 默认 `http://127.0.0.1:19530`

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

worker 写 Postgres 控制面和 Milvus retrieval data plane。

## 本地验证

```bash
cargo test --manifest-path crates/ingestion/Cargo.toml
cargo check -p avrag-worker
```

## 服务器部署提醒

### PDF 视觉入库（VisualRaster，2026-06-10 起默认）

低文字页 / 扫描页走 **PyMuPDF sidecar**，不再使用 MinerU PDF OCR。

1. 启动 pdf-visual-renderer（默认 `127.0.0.1:9091`）：
   - `PDF_RENDERER_BASE_URL=http://127.0.0.1:9091`
   - **端口冲突**：Milvus standalone 默认也占 `9091`（metrics）。`docker-compose.milvus.yml` 已把 Milvus 宿主机映射改为 `19091:9091`，gRPC 仍用 `19530`。若 9091 被占，`pdf-renderer-up.sh` 会提示并退出。
   - 启动：`./scripts/pdf-renderer-up.sh`
   - 停止：`./scripts/pdf-renderer-down.sh`
2. 可选调参：
   - `PDF_VISUAL_PAGES_PER_CHUNK=4` — 多页 fusion chunk 大小
   - `PDF_RENDERER_TIMEOUT_MS=60000`
   - `MM_EMBEDDING_IMAGE_TOKEN_ESTIMATE=896` — 多图 embed 限流估算
3. VLM 页摘要（INGESTION_LLM）与可选 triplet（`INGESTION_VLM_TRIPLET_ENABLED=1`）依赖 `INGESTION_LLM_*` 配置。
4. 本地 object store 无 presigned URL 时，worker 在 **spawn 任务内** 将页图读入并编码为 `data:image/...;base64,...`（避免主线程同时持有多 chunk 大图）；降级原因写入 `parse_run.outputs.multimodal_degrade_reasons`。
5. 单 chunk 多模态 embed 失败时 **跳过该 chunk 向量** 并记 degrade，不会导致整单 ingest 失败。

### 图片 MinerU（仅 `MineruImage` 路由）

独立图片文件（非 PDF 页 OCR）仍可能走 MinerU：

- `MINERU_BASE_URL=https://mineru.net/api/v4`
- `MINERU_API_KEY=<有效 key>`
- MinerU `v4` 只接受可访问的 `http(s)` 文件 URL；对象存储需对 MinerU 可达（presigned URL）。

### Office 解析

- `OFFICE_PARSER_BASE_URL=http://127.0.0.1:9090`
- `./scripts/office-parser-up.sh` / `./scripts/office-parser-down.sh`
