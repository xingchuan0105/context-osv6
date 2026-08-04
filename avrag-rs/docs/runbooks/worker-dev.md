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
# 推荐：product-dev-up 拉起 minio/api/worker/next，worker 日志 tee 到 .dev-logs/worker.log
bash scripts/product-dev-up.sh   # 仓库根

# 或单独（worker 依赖 PATH 上的解析器：lit CLI（PDF）、office-direct-extract（Office）、markitdown（文本/代码））：
cd avrag-rs
mkdir -p .dev-logs
RUST_LOG=info,avrag_worker=info cargo run -p avrag-worker 2>&1 | tee -a .dev-logs/worker.log
```

| 依赖 | 说明 |
|------|------|
| `lit` CLI（`LITEPARSE_BIN`/`LITEPARSE_TIMEOUT_MS`/`LITEPARSE_SCANNED_MIN_CHARS`） | PDF 解析（`lit parse --format markdown --no-ocr`）；缺失/超时以「liteparse parse failed」显式报错 |
| `office-direct-extract`（`OFFICE_DIRECT_BIN`/`OFFICE_DIRECT_TIMEOUT_MS`） | Office 直读（docx/xlsx/pptx），doc/ppt/xls 先经 soffice 转 OOXML；docx 经 `pandoc -t gfm`（标准 GFM 表格，strip 图片死引用）；缺失以「office-direct parse failed」显式报错 |
| LibreOffice `soffice`（`OFFICE_SOFFICE_BIN`/`OFFICE_SOFFICE_TIMEOUT_MS`/`OFFICE_SOFFICE_MAX_CONCURRENT`） | 仅 doc/ppt/xls 旧二进制转 OOXML 时调用；需 writer/calc/impress 三组件齐全 |
| `markitdown` CLI（`MARKITDOWN_BIN`/`MARKITDOWN_TIMEOUT_MS`） | 文本/代码类兜底（txt/md/rst/csv/tsv/json/toml/yaml/html/代码）；缺失时摄入以「markitdown 子进程启动失败」显式报错 |
| `.dev-logs/worker.log` | `product-dev-up` 默认 tee；避免只挂 pts 丢日志 |
| ~~`PDF_RENDERER_BASE_URL`~~ | 已退役（2026-08-02），office parser(:9090)/PDF renderer(:9091) 不再被调用 |

**解析器安装（本地 worker 三组件需在 PATH）：**

- `lit` CLI（PDF）—— 见 `plans/2026-08-02-parser-pipeline-direct-readers.md` §5.7（VPS 预置项）。
- `office-direct-extract` —— `pip install -e ./scripts/office-direct`（worker venv；自带 openpyxl/python-pptx）。docx 路径还需系统级 `pandoc`（`pandoc --version`，GFM writer），缺失时 docx 解析以「office-direct parse failed」显式报错。脚本自身 `#!/usr/bin/env python3` + `chmod +x`，`OFFICE_DIRECT_BIN` 也支持直接指向 .py 路径。
- LibreOffice —— 系统安装并确保 `soffice --version` 可用，需 writer/calc/impress 三组件齐全（仅 doc/ppt/xls 旧二进制用到）。

启动时建议先确认：

1. **对象存储 probe**
   - 若走 S3，worker 会做 `.worker-probe` `HEAD`；若失败会直接退出。
   - 若走本地目录，worker 会写入/读回/删除探针文件。
2. **健康探针**
   - `AVRAG_WORKER_HEALTH_PORT=0` 时会自动选端口并写入 `AVRAG_WORKER_HEALTH_PORT_FILE`。
   - 本地可直接 `curl http://127.0.0.1:<port>/health` 验证存活。
3. **解析器 CLI**
   - `command -v lit && command -v office-direct-extract && command -v markitdown`（worker PATH 必须可见；docx 还需 `command -v pandoc`；旧二进制 Office 还需 `command -v soffice`）。

## 任务契约

支持两类任务：

1. `ingest_document`
   - 用于新上传文件的解析、分块、索引写入
2. `reindex_document`
   - 用于手动或系统触发的重建流程

最小公共字段：

- `task_id`
- `kind`
- `owner_user_id`
- `workspace_id`
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

### 文档入库（按格式分工，2026-08-02 起）

PDF 走 **liteparse**（`lit parse --format markdown --no-ocr`）；Office 类（docx/xlsx/pptx/doc/ppt/xls）走 **office-direct-extract**（docx/xlsx/pptx 直读，旧二进制 doc/ppt/xls 经 soffice 无损转 OOXML 后直读）；txt/md/html/csv/代码走 **markitdown**。各子进程均产出 markdown → IR/切块/索引；表格类文档另经 struct-query 表格阶段（per-doc duckdb + 证据 chunk）。原 LiteParse 主链（hybrid 探针/页路由/VisualRaster 兜底）已退役删除。设计见 `plans/2026-08-02-parser-pipeline-direct-readers.md`。

| 文档形态 | 处理方式 |
|------|----------|
| PDF（数字版） | liteparse（`lit parse --format markdown --no-ocr`）子进程 → markdown → IR |
| PDF（扫描件，liteparse 提取近空） | 整本转 Paddle Jobs OCR（`paddle_ocr_pdf`，1 文件 1 Job，`pdf_route_mode=paddle_ocr_pdf`） |
| Office（docx/xlsx/pptx/doc/ppt/xls） | office-direct-extract 直读（docx 经 `pandoc -t gfm` 产标准 GFM 表格并 strip 图片语法；xlsx/pptx 直读；旧二进制 doc/ppt/xls 经 soffice 转 OOXML）→ markdown → IR |
| 文本/代码（txt/md/csv/json/toml/yaml/html/代码） | markitdown 子进程 → markdown → IR |
| 独立图片（png/jpg/webp） | Paddle AI Studio **Jobs** API（`PADDLE_OCR_*`，现役唯一图片路径） |

### Embedding / Rerank 供应商（2026-08-03 起：SiliconFlow）

> **2026-08-04 产品默认**：查询侧 **关闭** `MM_EMBEDDING_*` / `MM_RERANK_*`（字多图少成本）。插图走 VLM 描述 → text 索引；扫描 Paddle。权威：`docs/engineering/2026-08-04-mm-off-vlm-figure-text-plan.md`。


| 槽位 | 模型 | 约束 |
|---|---|---|
| `EMBEDDING_*`（text） | `Pro/BAAI/bge-m3`（1024d） | **禁 dimensions 参数（400）→ `EMBEDDING_DIMENSIONS` 必须留空**（代码空串→None→不发字段）；base_url `https://api.siliconflow.cn/v1` |
| `RERANK_*`（text） | `Pro/BAAI/bge-reranker-v2-m3` | 只收裸字符串 documents；**`RERANK_API_STYLE` 留空**（不走 dashscope_vl_rerank） |
| `MM_EMBEDDING_*`（multimodal） | `Qwen/Qwen3-VL-Embedding-8B` | OpenAI 形状 `/embeddings`，`input` 混合对象数组 `[{text},{image}]` + `dimensions`（`MM_EMBEDDING_API_STYLE=openai_vl_embedding`） |
| `MM_RERANK_*`（multimodal） | `Qwen/Qwen3-VL-Reranker-8B` | OpenAI 形状 `/rerank`，裸字符串 `query` + `documents` 对象数组（`MM_RERANK_API_STYLE=openai_vl_rerank`） |

所有槽位 key 复用 `SILICONFLOW_API_KEY`。向量维度全部 1024（与 Milvus collection schema 兼容；换模型后**必须重灌**，新旧向量不能混排）。

1. **Paddle Jobs（仅独立图片）**
   - `PADDLE_OCR_BASE_URL` — 默认 `https://paddleocr.aistudio-app.com/api/v2/ocr`
   - `PADDLE_OCR_API_TOKEN` — AI Studio Token（**禁止入库/日志**）
   - `PADDLE_OCR_MODEL` — 如 `PaddleOCR-VL-1.6`
2. **E 类 VisualRaster sidecar** —— 已退役（2026-08-02 直读切换）：`PDF_RENDERER_BASE_URL`、`PDF_VISUAL_PAGES_PER_CHUNK`、`PDF_RENDERER_TIMEOUT_MS` 均不再生效；`pdf-renderer-up.sh/down.sh` 已删除；扫描件 PDF 现由 liteparse 提取近空检测后整本转 Paddle OCR（见上表），不再按零块校验拒收。
3. 可选调参：`MM_EMBEDDING_IMAGE_TOKEN_ESTIMATE=896`
4. **摄取 LLM 会话（profile/summary/triplet 同一 DashScope 会话链）**：profile、summary、triplet 三件套由 `INGESTION_LLM_*` 驱动的**同一会话续接**（Responses API `previous_response_id` 链 + `x-dashscope-session-cache` 会话缓存，`INGESTION_LLM_API_STYLE=dashscope_responses`）。`INGESTION_VLM_TRIPLET_ENABLED=1` 的可选 VLM triplet 走无状态 `INGESTION_LLM` 直连。**数据驻留：会话缓存由 DashScope 侧保存文档全文，最长保留 7 天**（`previous_response_id` 有效期），按文档合规要求需明确知悉。

> **已删除：** MinerU PDF OCR、`LITEPARSE_ENABLED` / shadow / 灰度开关。历史见 `docs/archive/p4-mineru-shadow-migration-historical.md`。liteparse/office parser 退役见 `docs/plans/2026-07-31-struct-query-w2-s4-window2-handoff.md`。

### 独立图片 Paddle OCR（`ParseRoute::PaddleOcrImage`）

独立图片（`png` / `jpg` / `webp` 等）走 Paddle Jobs，1 文件 = 1 Job：

- `PADDLE_OCR_BASE_URL` — 默认 `https://paddleocr.aistudio-app.com/api/v2/ocr`
- `PADDLE_OCR_API_TOKEN` — AI Studio Token（**禁止入库/日志**）
- `PADDLE_OCR_MODEL` — 如 `PaddleOCR-VL-1.6`

产出：`DocumentType::Image`，`pdf_route_mode=paddle_image`，文本块 + Figure 块（含 MM 索引）。

### Office 解析

docx/xlsx/pptx/doc/ppt/xls 走 worker 子进程 `office-direct-extract` 直读（docx 经 `pandoc -t gfm`；xlsx/pptx 用 openpyxl/python-pptx；旧二进制 doc/ppt/xls 经 soffice 转 OOXML），产出 markdown → IR/切块/索引；表格类另经 struct-query 表格阶段。`OFFICE_PARSER_BASE_URL` 与 `office-parser-up.sh/down.sh` 已退役删除。
