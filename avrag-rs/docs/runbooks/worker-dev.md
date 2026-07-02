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
- `AVRAG_WORKER_QUEUE_GROUP`
  - 默认 `default`
  - 仅 claim 对应 `ingestion_tasks.queue_group` 的任务；用于隔离不同 worker 池
- `AVRAG_INGESTION_QUEUE_GROUP`
  - 默认 `default`
  - API/入队侧写入 `ingestion_tasks.queue_group`，应与目标 worker 组对齐
- `AVRAG_WORKER_SKIP_STORAGE_PROBE`
  - 默认 `false`
  - `true` 时跳过 worker 启动阶段对象存储探针（仅建议诊断时临时使用）
- `MILVUS_URL`
  - 默认 `http://127.0.0.1:19530`

## 运行

```bash
cargo run -p avrag-worker
```

启动时建议先确认：

1. **对象存储 probe**
   - 若走 S3，worker 会做 `.worker-probe` `HEAD`；若失败会直接退出。
   - 若走本地目录，worker 会写入/读回/删除探针文件。
2. **健康探针**
   - `AVRAG_WORKER_HEALTH_PORT=0` 时会自动选端口并写入 `AVRAG_WORKER_HEALTH_PORT_FILE`。
   - 本地可直接 `curl http://127.0.0.1:<port>/health` 验证存活。

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

## Dead-letter 监控

建议定期检查 dead-letter 积压，按队列组拆分观察：

```sql
select queue_group, count(*) as dead_letter_count
from ingestion_tasks
where status = 'dead_letter'
group by queue_group
order by dead_letter_count desc;
```

常见排障顺序：

1. 核对 `queue_group` 是否匹配（入队组与 worker 组一致）。
2. 查看 `last_error` / `last_failed_at` 判断是否为可重试外部依赖故障。
3. 修复后按任务维度重投递（避免直接全表重置）。

## 本地验证

```bash
cargo test --manifest-path crates/ingestion/Cargo.toml
cargo check -p avrag-worker
```

## 服务器部署提醒

### PDF 入库（LiteParse + Paddle Jobs，P4 后默认）

PDF 与 Office→PDF 文档走 **LiteParse 主链**：hybrid 探针（`probe_pdf_hybrid`）→ `router/page_routes` 页内分拣 → Worker `execute_pdf_parse`。

| 页型 | 处理方式 |
|------|----------|
| A/B（有字） | LiteParse 抽字；B 类附加 Figure → MM |
| C/D（表/扫描） | Paddle AI Studio **Jobs** API（`PADDLE_OCR_*`） |
| E（兜底） | `pdf-renderer` sidecar 整页 VisualRaster |

1. **Paddle Jobs（C/D 类 PDF 页 + 独立图片）**
   - `PADDLE_OCR_BASE_URL` — 默认 `https://paddleocr.aistudio-app.com/api/v2/ocr`
   - `PADDLE_OCR_API_TOKEN` — AI Studio Token（**禁止入库/日志**）
   - `PADDLE_OCR_MODEL` — 如 `PaddleOCR-VL-1.6`
2. **E 类 VisualRaster sidecar**（仅 OCR 失败 / Job 预算耗尽时）
   - `PDF_RENDERER_BASE_URL=http://127.0.0.1:9091`
   - **端口冲突**：Milvus standalone 默认也占 `9091`（metrics）。`docker-compose.milvus.yml` 已把 Milvus 宿主机映射改为 `19091:9091`，gRPC 仍用 `19530`。若 9091 被占，`pdf-renderer-up.sh` 会提示并退出。
   - 启动：`./scripts/pdf-renderer-up.sh` / 停止：`./scripts/pdf-renderer-down.sh`
3. 可选调参：`PDF_VISUAL_PAGES_PER_CHUNK=4`、`PDF_RENDERER_TIMEOUT_MS=60000`、`MM_EMBEDDING_IMAGE_TOKEN_ESTIMATE=896`
4. VLM 页摘要（INGESTION_LLM）与可选 triplet（`INGESTION_VLM_TRIPLET_ENABLED=1`）依赖 `INGESTION_LLM_*` 配置。

> **已删除：** MinerU PDF OCR、`LITEPARSE_ENABLED` / shadow / 灰度开关。历史见 `docs/archive/p4-mineru-shadow-migration-historical.md`。架构详情见 `docs/liteparse-paddle-ingestion-architecture-2026-06-13.md`。

### 独立图片 Paddle OCR（`ParseRoute::PaddleOcrImage`）

独立图片（`png` / `jpg` / `webp` 等）走 Paddle Jobs，1 文件 = 1 Job：

- `PADDLE_OCR_BASE_URL` — 默认 `https://paddleocr.aistudio-app.com/api/v2/ocr`
- `PADDLE_OCR_API_TOKEN` — AI Studio Token（**禁止入库/日志**）
- `PADDLE_OCR_MODEL` — 如 `PaddleOCR-VL-1.6`

产出：`DocumentType::Image`，`pdf_route_mode=paddle_image`，文本块 + Figure 块（含 MM 索引）。

### Office 解析（仅 Excel）

- `OFFICE_PARSER_BASE_URL=http://127.0.0.1:9090`
- `./scripts/office-parser-up.sh` / `./scripts/office-parser-down.sh`
