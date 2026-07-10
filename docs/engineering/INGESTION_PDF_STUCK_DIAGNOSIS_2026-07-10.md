# PDF Ingestion 卡住诊断与交接（2026-07-10）

**状态**: 已诊断、部分运维缓解；**根因代码未修**  
**工作区**: `http://localhost:3000/dashboard/f57a24e8-fc4a-4edf-872f-ed9841e20ef5`  
**用途**: 现场证据与运维交接；**修复任务与 Org 移除方案以统一文档为准**。

> **权威修复方案（必读）**  
> [`INGESTION_AND_ORG_REMOVAL_UNIFIED_PLAN_2026-07-10.md`](./INGESTION_AND_ORG_REMOVAL_UNIFIED_PLAN_2026-07-10.md)  
> - **A 部**订正根因：非 create 卡住，而是 `build_ir_chunk_plan` 对 ~1500 micro-block 重复 `cl100k_base()`（高 CPU → 300s timeout；有 `running` parse_run + blocks、无 chunks）。  
> - **B 部**：彻底去掉 org 概念（owner_user / workspace）。  
> 查 `document_parse_runs` 须设租户 GUC（现 `app.current_org`）；只设 `super_admin` 会假阴性「无行」。

---

## 1. 复现对象

| 字段 | 值 |
|------|-----|
| Workspace ID | `f57a24e8-fc4a-4edf-872f-ed9841e20ef5`（标题：工作区1） |
| Org ID | `f0ccc1ce-dbbe-46b3-88bd-f0f23f93bcbf` |
| Document ID | `9b9a1c86-605d-477c-b6b8-d9216ce8aeed` |
| 文件名 | `2606.10209v1.pdf`（约 380 583 bytes，**17 页**） |
| Object path | `f0ccc1ce-dbbe-46b3-88bd-f0f23f93bcbf/f57a24e8-fc4a-4edf-872f-ed9841e20ef5/9b9a1c86-605d-477c-b6b8-d9216ce8aeed/2606.10209v1.pdf` |
| 本地对象 | `/home/chuan/.local/share/avrag-dev/objects/<object_path>`（**文件在**） |

---

## 2. 现象

- UI：内容源长时间 **processing / 处理中**。
- DB：`documents.status = processing`，`chunk_count = 0`。
- **无** `document_parse_runs` 行，**无** `chunks` / `document_blocks`。
- `ingestion_tasks`：`processing`，`locked_by` 有 worker，**lease 约 60s 续一次**。
- 约 **300s** 后 `last_error = task timeout after 300s`，`attempt_count++`，再 claim 重试 → 用户感知「一直卡在 ingestion」。
- 曾短暂出现 **`completed` + `chunk_count = 0`**（假完成，RAG 无索引）——属终态 bug。

前端 `localhost:3000` 代理后端正常时，**不是前端假死**，是 worker 解析管线挂住。

---

## 3. 根因（按证据优先级）

> **§3.1 已订正（2026-07-10 复检）** — 详见统一方案 A 部。  
> 设 `app.current_org` 后可见：**多条 `document_parse_runs`（status=running）** + **1502 `document_blocks`** + **0 chunks**。  
> 卡点在 IR project **之后** 的 `build_ir_chunk_plan`（per-block `cl100k_base()`），不是 create 前后。

### 3.1 主因：`build_ir_chunk_plan` 重复构造 tokenizer（代码）

Worker 日志（`/tmp/avrag-worker-ingest-diag.log`）最后有效行：

```text
Document routing decision filename=2606.10209v1.pdf route=Pdf reason=simple_pdf
PDF page routing plan prepared ... total_pages=17 liteparse_text_pages=17 visual_raster_pages=0
```

之后（订正）：

- **有** parse_run（`running`）与 **1502** blocks；**无** chunks  
- 阶段日志不足：plan prepared 到 `IR chunk plan generated` 之间几乎无 info  
- 主线程 **R + 高 CPU** 持续数分钟  
- 直至 `AVRAG_INGESTION_TASK_TIMEOUT_SECS` 默认 **300** 触发  

根因：`crates/ingestion/src/chunker.rs` 对每个 text block 调用 `token_chunk_config` → `cl100k_base()`（~80ms/次 × ~1502 micro-blocks；debug 更慢）。

同 PDF 用 PyMuPDF 抽全文约 **0.07s / ~6 万字符** → **不是 PDF 坏文件**。

**代码入口**

| 步骤 | 路径 |
|------|------|
| Claim / process | `avrag-rs/bins/worker/src/pipeline/processor.rs`（`PgTaskProcessor::process`） |
| PDF 执行 | `avrag-rs/bins/worker/src/pdf/parse.rs`（`execute_pdf_parse`） |
| 超时 | `AVRAG_INGESTION_TASK_TIMEOUT_SECS` 默认 300（`bins/worker/src/lib.rs`） |
| Stale lease | `STALE_PROCESSING_TIMEOUT_SECS = 30 * 60`（`storage-pg` repository_ingestion_queue） |

### 3.2 加重：pdf-visual-renderer 当时未起

| 配置 | `PDF_RENDERER_BASE_URL=http://127.0.0.1:9091`（`.env`） |
|------|----------------------------------------------------------|
| 诊断时 | **9091 无进程** |
| 本 PDF | `visual_raster_pages=0`，**主路径不依赖** 渲图；扫描件/E-class 回退会挂 |
| 已做 | `bash avrag-rs/scripts/pdf-renderer-up.sh` → healthz OK |

### 3.3 加重：timeout 取消不彻底 / lease 占坑

- 外层 `tokio::time::timeout(300s, …)` 到期后任务会 fail/requeue。  
- 内层若 `spawn_blocking` / 未协作取消，**旧任务仍占 CPU + advisory lock**。  
- 曾出现：任务已 `queued`，worker 仍刷 `poll completed with no tasks`。

### 3.4 终态 bug：允许 completed + 0 chunks

- 文档可标 `completed` 且 `chunk_count=0`，任务行删除。  
- UI 可能不再转圈，RAG 仍无内容。  
- 应：**0 chunk 不得 completed** → `failed` + 明确错误。

### 3.5 次要（非主卡死）

| 项 | 说明 |
|----|------|
| ClamAV `:3310` | 未起 → **fail-open** WARN only |
| `user_profiles` RLS | agent preference 任务失败，与 PDF 无关 |
| kind 映射 WARN | `unknown ingestion kind "ingest_document"` fallback 仍 IngestDocument |

---

## 4. 环境与进程（诊断时）

| 服务 | 端口 / 路径 | 备注 |
|------|-------------|------|
| Next | `:3000` | OK |
| avrag-api | `:8080` `/health` | OK |
| avrag-worker | debug binary | 需带日志文件跑 |
| Postgres | `DATABASE_URL` → `avrag_rs` | RLS：查表需 `set_config('app.current_role','super_admin',false)` |
| 对象 | `AVRAG_OBJECT_ROOT=~/.local/share/avrag-dev/objects` | 本 PDF 文件在 |
| MinIO | `:9000` | 本地 dev 也可能走 filesystem object root |
| pdf-visual-renderer | `:9091` | **dev 必须 up**（扫描/回退） |
| Embedding | Dashscope | 探活 OK |
| INGESTION_LLM | DeepSeek | 探活 OK |
| Paddle OCR | 远端 AI Studio | 本 PDF 文本页为主，非主路径 |

**有用日志**

```text
/tmp/avrag-worker-ingest-diag.log          # 诊断时 nohup
avrag-rs/.dev-logs/pdf-visual-renderer.log
avrag-rs/scripts/pdf-renderer-up.sh
avrag-rs/scripts/pdf-renderer-down.sh
```

---

## 5. 已做运维（勿重复踩坑）

1. 启动 pdf-visual-renderer（`:9091`）。  
2. kill 旧 worker、`nohup ./target/debug/avrag-worker > /tmp/avrag-worker-ingest-diag.log`。  
3. 多次 requeue / 手工 `INSERT ingestion_tasks`。  
4. **根因未修**：plan prepared 后仍 CPU 空转至 timeout。

新窗口开工前请先 `ps` / `curl` 确认 api、worker、9091 是否仍存活（进程可能已换 PID）。

---

## 6. 新窗口建议执行顺序

### 6.1 快速确认现场

```bash
cd /home/chuan/context-osv6/avrag-rs
set -a && source .env && set +a

curl -sS -m 2 http://127.0.0.1:8080/health
curl -sS -m 2 http://127.0.0.1:9091/v1/healthz
ps aux | rg 'avrag-worker|avrag-api|pdf-visual'

psql "$DATABASE_URL" -c "
SELECT set_config('app.current_role','super_admin',false);
SELECT id, file_name, status, chunk_count, updated_at
FROM documents WHERE id = '9b9a1c86-605d-477c-b6b8-d9216ce8aeed';
SELECT task_id, status, attempt_count, last_error, locked_at, now()-locked_at
FROM ingestion_tasks
WHERE document_id = '9b9a1c86-605d-477c-b6b8-d9216ce8aeed'
ORDER BY enqueued_at DESC LIMIT 5;
"
```

### 6.2 代码修复（P0）

| ID | 任务 | 验收 |
|----|------|------|
| **I1** | 在 `processor.rs` plan 日志后、`create_document_parse_run` 前后、`run_document_pipeline` 各阶段、`execute_pdf_parse` 入口/出口打 **info 阶段日志**（含 elapsed） | 再跑该 PDF 能看到卡在哪一行 |
| **I2** | 定位 simple_pdf + 17 text pages 的 **CPU 空转**（LiteParse 复用 / merge / materialize / 误走 OCR） | 该 PDF 5 分钟内 `chunk_count > 0` 且 `status=completed` |
| **I3** | timeout 后 **取消内层任务 + 释放 lock**（含 advisory / heartbeat） | requeue 后下一轮能立刻 claim，无「queued 但 poll no tasks」 |
| **I4** | **禁止 completed + 0 chunks**；改为 failed + 错误码 | 库约束或 worker 终态校验 |
| **I5** | `scripts/dev` / product-dev-up **默认拉起 pdf-renderer** | 文档写清 |
| **I6** | worker 默认日志落到 `.dev-logs/worker.log` | 不依赖 pts |

### 6.3 最小复现命令（修完后）

```bash
# 确保 renderer + worker + api
bash scripts/pdf-renderer-up.sh
# 启动 worker（示例）
# RUST_LOG=info,avrag_worker=info ./target/debug/avrag-worker 2>&1 | tee .dev-logs/worker.log

# API 侧：对该 document reindex
# POST /api/v1/workspaces/{workspace_id}/documents/{document_id}/reindex
# 或 UI 删源重传 2606.10209v1.pdf
```

期望：

- 日志越过 `PDF page routing plan prepared`  
- 出现 parse_run  
- `chunk_count >= 1`  
- UI 源状态可勾选 / RAG 可引用  

### 6.4 手工 requeue SQL（调试用）

```sql
SELECT set_config('app.current_role', 'super_admin', false);

UPDATE documents
SET status = 'pending', chunk_count = 0, updated_at = now()
WHERE id = '9b9a1c86-605d-477c-b6b8-d9216ce8aeed';

INSERT INTO ingestion_tasks (
  task_id, org_id, workspace_id, document_id, kind, requested_by,
  idempotency_key, payload, status, attempt_count, max_attempts,
  available_at, enqueued_at, updated_at, queue_group
) VALUES (
  gen_random_uuid(),
  'f0ccc1ce-dbbe-46b3-88bd-f0f23f93bcbf',
  'f57a24e8-fc4a-4edf-872f-ed9841e20ef5',
  '9b9a1c86-605d-477c-b6b8-d9216ce8aeed',
  'ingest_document',
  'c3e4a529-46b8-4f37-850a-62be1c2f4f2b',
  'manual-requeue-' || gen_random_uuid()::text,
  jsonb_build_object(
    'type', 'ingest_document',
    'filename', '2606.10209v1.pdf',
    'file_size', 380583,
    'mime_type', 'application/pdf',
    'source_uri', 'object://f0ccc1ce-dbbe-46b3-88bd-f0f23f93bcbf/f57a24e8-fc4a-4edf-872f-ed9841e20ef5/9b9a1c86-605d-477c-b6b8-d9216ce8aeed/2606.10209v1.pdf',
    'object_path', 'f0ccc1ce-dbbe-46b3-88bd-f0f23f93bcbf/f57a24e8-fc4a-4edf-872f-ed9841e20ef5/9b9a1c86-605d-477c-b6b8-d9216ce8aeed/2606.10209v1.pdf'
  ),
  'queued', 0, 5, now(), now(), now(), 'default'
);
```

正式路径优先用 API **`POST .../documents/{id}/reindex`**。

---

## 7. 与前端 UI 工作的边界

| 本交接文档 | 并行 UI 窗口 |
|------------|--------------|
| worker / PDF / chunk / reindex | Dashboard / Chat / Settings 等 `frontend_next` |
| 不阻塞 UI 波次（U1–U14） | 不依赖本 PDF 修好 |

UI 侧若需展示 ingestion 错误：可读 `status=failed` + 文案；**0-chunk completed** 修好前可对 `completed && chunk_count=0` 显示「索引为空，请重试」。

---

## 8. 相关文档

- 多站点 / UI 升级总计划：`docs/engineering/VISUAL_SYSTEM_AND_MULTI_SITE_UPGRADE_PLAN_2026-07-10.md`  
- 样式基准：`docs/design/STYLE_BASELINE.md`  
- 第三方注意：PyMuPDF AGPL → 未授权 SaaS 勿开 renderer（`THIRD_PARTY_NOTICES.md`）；**本地 dev 可开**

---

## 9. 一句话交接

**上传与对象 OK；worker 领到任务并完成 simple_pdf 路由（17 文本页）后 CPU 空转，无 parse_run/chunk，300s 超时循环。先加阶段日志钉死卡点，再修超时取消与 0-chunk 终态；pdf-renderer 必须在 dev 常开。**
