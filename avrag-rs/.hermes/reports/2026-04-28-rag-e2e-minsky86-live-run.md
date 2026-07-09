# Live RAG E2E — minsky86.pdf — 2026-04-28
## Scope
- File: `/mnt/e/Download/minsky86.pdf` (`5599744` bytes)
- Notebook ID: `7cdd0997-2f06-4046-a716-c1747a590976`
- Document ID: `873f52ca-eb2b-4d9f-aef2-7dadd590feb6`
- Secrets redacted.

## Timeline
- [15:46:15] Starting live RAG E2E for minsky86.pdf
- [15:46:15] PDF exists: /mnt/e/Download/minsky86.pdf (5599744 bytes)
- [15:46:15] GET /health -> 200 {"components":["api","postgres:ok"],"status":"ok"}
- [15:46:15] GET /ready -> 200 {"checks":["postgres:ok"],"ready":true}
- [15:46:15] Milvus collections/list -> 200 {"code":0,"data":["avrag_rag_text_chunks","avrag_rag_multimodal_chunks","avrag_rag_kg_entities","avrag_rag_kg_relations","avrag_rag_graph_passages"]}
- [15:46:16] POST /api/auth/register -> 201
- [15:46:16] GET /api/auth/me -> 200
- [15:46:16] POST /api/v1/workspaces -> 201 {"notebook":{"id":"7cdd0997-2f06-4046-a716-c1747a590976","org_id":"edd22a8f-4e86-447e-8479-dbc6e7471725","owner_id":"ea047241-0519-49ac-a190-0ec9f1ac2fd3","name":"e2e-minsky86-1777362376","title":"e2e-minsky86-1777362376","description":"Live E2E minsky86.pdf","created_at":"2026-04-28T07:46:16.548784
- [15:46:16] POST /api/v1/workspaces/7cdd0997-2f06-4046-a716-c1747a590976/documents -> 201 {"document_id":"873f52ca-eb2b-4d9f-aef2-7dadd590feb6","upload_url":"http://127.0.0.1:8080/uploads/873f52ca-eb2b-4d9f-aef2-7dadd590feb6?expires=1777363276&signature=6a57905b6b18b6083444bbbd892207c05e33f677266f0c0cab7f5e68f9e95ffc","status":"pending"}
- [15:46:16] PUT /dev-upload/873f52ca-eb2b-4d9f-aef2-7dadd590feb6 -> 200 {"status":"queued"}
- [15:46:16] status poll 00 elapsed=0s -> HTTP 200, status=queued, body={"status": "queued"}
- [15:46:21] status poll 01 elapsed=4s -> HTTP 200, status=processing, body={"status": "processing"}
- [15:46:31] status poll 03 elapsed=14s -> HTTP 200, status=failed, body={"status": "failed"}
- [15:46:31] Final document status: failed
- [15:46:31] Milvus collections after upload -> 200 {"code":0,"data":["avrag_rag_text_chunks","avrag_rag_multimodal_chunks","avrag_rag_kg_entities","avrag_rag_kg_relations","avrag_rag_graph_passages"]}
- [15:46:31] Skipping RAG chat because document did not complete ingestion.

## Result
- Final document status: `failed`
- RAG chat skipped because ingestion did not complete.

## Status history last entries
- elapsed=0s http=200 status=queued body=`{"status": "queued"}`
- elapsed=4s http=200 status=processing body=`{"status": "processing"}`
- elapsed=9s http=200 status=processing body=`{"status": "processing"}`
- elapsed=14s http=200 status=failed body=`{"status": "failed"}`


## Post-run root-cause investigation

After the API-level upload attempt, database inspection showed the ingestion task failed/retried before RAG query could run.

Current observed DB state:

```json
{
  "doc_status": "failed",
  "chunk_count": "0",
  "task_status": "dead_letter",
  "attempt_count": "6",
  "max_attempts": "5",
  "last_error": "state sink error: MinerU OCR parse failed for minsky86.pdf: MinerU v4 requires an HTTP(S) source URL; got file://edd22a8f-4e86-447e-8479-dbc6e7471725/7cdd0997-2f06-4046-a716-c1747a590976/873f52ca-eb2b-4d9f-aef2-7dadd590feb6/minsky86.pdf for minsky86.pdf",
  "available_at": "2026-04-28 15:54:27.351512+08"
}
```

Root blocker:

- `minsky86.pdf` is routed through mixed PDF parsing: digital pages via `edge_parse_pdf`, OCR pages via `mineru_pdf_ocr`.
- MinerU is configured in v4 mode (`MINERU_BASE_URL=https://mineru.net/api/v4`). The v4 path requires an HTTP(S) source URL.
- The current local object store produced a `file://.../minsky86.pdf` source URL, so MinerU rejected the OCR fallback before indexing.
- Because ingestion did not complete, the test did not reach embedding, Milvus upsert/search, or RAG answer generation.

Additional weak point:

- The task row showed `attempt_count` greater than `max_attempts` while still cycling through retry/processing state during observation. This suggests the retry/dead-letter guard should be reviewed separately.

Recommended next choices:

1. Provide MinerU v4 with a public HTTP(S) URL for uploaded PDFs, e.g. real S3-compatible object storage with a public presigned GET URL.
2. Or switch MinerU to a working upload-mode endpoint if the legacy upload API is still valid for the configured account/base URL.
3. Or change the worker design to support local-file upload to MinerU v4 instead of URL-only for local dev.



## 2026-04-28 OCR batching optimization

User requested keeping the same sample (`/mnt/e/Download/minsky86.pdf`) while optimizing MinerU OCR handling:

- Do not change sample PDF.
- Batch-upload OCR pages to MinerU instead of one request per page.
- Detect and skip blank / low-value OCR pages.

Implemented in `crates/ingestion/src/parser/mineru.rs`:

- Local MinerU v4 OCR pages now use one `/file-urls/batch` request for all selected OCR pages.
- The worker uploads all returned signed URLs and waits for the whole batch via `/extract-results/batch/{batch_id}`.
- Page-filtered local uploads still split the original PDF into per-page PDFs before upload because MinerU upload mode does not accept remote `page_ranges` for already-uploaded local files.
- Blank PDF pages are skipped before upload when the page content stream has no renderable text/image operations.
- Empty or page-number-only MinerU results are treated as low-value and skipped after OCR.
- Skipped pages are represented by empty `mineru_pdf_ocr` page placeholders so the PDF merge stage can preserve the page plan without generating text chunks for those skipped pages.

Verification:

- `cargo fmt --all -- --check` passed.
- `cargo test -p ingestion v4_ -- --nocapture` passed: 10 tests.
- `cargo check -p ingestion -p avrag-worker` passed.

Live E2E retry against the unchanged `minsky86.pdf`:

- API health/ready passed.
- Worker route plan remained unchanged: `total_pages=407`, `edgeparse_pages=363`, `mineru_ocr_pages=44`.
- MinerU OCR used one batch upload: `file_count=44`.
- MinerU batch completed successfully.
- Low-value OCR pages skipped after OCR: pages `120`, `341`, `342`.
- OCR output after skip placeholders: `units=252`.
- The previous state-sink error `mineru_pdf_ocr did not produce requested page 120` was fixed by preserving empty page placeholders.
- IR chunk plan generated successfully: `text_chunks=881`, `multimodal_chunks=45`.
- DB observed `chunk_count=881` for latest test document.

New downstream blocker after OCR/index preparation:

- Retrieval indexing failed with:
  `retrieval data plane indexing failed: vector dimension mismatch for multimodal_chunks[0].multimodal_dense: expected 1024, got 2560`
- This is after OCR batching and chunk generation, so it is not a MinerU upload/OCR batching issue.
- Worker/E2E were stopped after observing this to avoid repeated MinerU retries and quota consumption.

Next recommended fix:

- Investigate multimodal embedding model/config dimension versus Milvus schema dimension for `multimodal_dense`.
- Either align multimodal embedding output dimension to the collection schema, recreate/update the Milvus multimodal field schema in dev, or disable multimodal upsert for this E2E path until dimensions are intentionally aligned.

## 2026-04-28 18:12 CST — qwen3-vl-embedding 1024 维对齐后 E2E 结果

变更：
- 将项目默认 `qwen3-vl-embedding` 多模态输出维度对齐为 `1024`。
- `.env` / `.env.example` 增加 `MM_EMBEDDING_DIMENSIONS=1024`。
- 保持 `MILVUS_MULTIMODAL_VECTOR_DIM=1024` 不变，避免重建已有 dev collection。

验证：
- Provider probe：`requested_dimension=1024`，实际返回向量长度 `1024`。
- 新增回归测试：`app_config_defaults_qwen3_vl_embedding_to_multimodal_schema_dimension`，RED 后 GREEN。
- `cargo fmt --all -- --check` 通过。
- `cargo test -p app app_config_defaults_qwen3_vl_embedding_to_multimodal_schema_dimension -- --nocapture` 通过。
- `cargo check -p app -p avrag-worker -p avrag-storage-milvus` 通过。
- `git diff --check` 通过。

真实链路结果：
- API/worker/Milvus 均运行。
- `minsky86.pdf` 重新进入 worker 后，MinerU OCR batch 完成。
- OCR 低价值页跳过：page 120、341、342。
- IR chunk plan：`text_chunks=881`，`multimodal_chunks=45`。
- `document_parse_runs` 最新记录完成：`status=completed`，`duration_ms=326612`。
- 之前的 `expected 1024, got 2560` 多模态维度错误未再出现。
- 手动 RAG SSE 查询成功：事件包含 `start`、`activity`、`answer_start`、`token`、`citations`、`done`。
- SSE 统计：`token=367`，`activity=4`，`citations=1`，`done=1`。
- 最终中文回答长度约 `689` 字符，并返回 citations，包含 `milvus_multimodal_dense` 命中。

新增/仍需关注的弱点：
- 摘要生成失败后使用 naive fallback：`Summary generation failed, keeping naive fallback`。
- triplet extraction 多次 degraded，LLM 调用返回 401；日志中仅记录 provider 返回的掩码 key 片段，报告中不保留任何真实凭证。该问题未阻断 ingestion/RAG 主链路，但会影响 KG/triplet 增强质量。
- 当前 Playwright 外层规格仍缺 `@playwright/test` 依赖；本次用 API + SSE 手动链路验证替代。

## 2026-04-28 21:47 CST — SUMMARY_LLM restored to DMXAPI/Gemini

用户要求摘要和三元组抽取都走 DMXAPI / Gemini 3.1 Flash-family。

处理：
- `.env` 中 `SUMMARY_LLM_BASE_URL` 改为 `https://www.dmxapi.cn/v1`。
- `.env` 中 `SUMMARY_LLM_MODEL` 改为当前 DMXAPI token group 可用的 `gemini-3.1-flash-lite-preview`。
- `SUMMARY_LLM_ENABLE_THINKING=false`。
- worker 已重启以加载新环境变量。

验证：
- DMXAPI probe 成功：`gemini-3.1-flash-lite-preview` 返回 `ok`。
- 同一 DMXAPI token group 下 `gemini-3.1-flash` 与 `gemini-3.1-flash-preview` 暂无可用渠道，返回 503；因此使用已验证可用的 Flash Lite Preview。
- 小文档 smoke ingestion 完成。
- 摘要日志：`successfully updated document summary with LLM result`。
- 三元组/KG 输出：`entity_count=6`、`relation_count=3`、`graph_passage_count=3`、`graph_degrade_count=0`。

结论：摘要与三元组抽取已恢复到 DMXAPI/Gemini 路径，且 smoke test 未再出现 401。

## 2026-04-28 22:26 CST — Full live E2E after SUMMARY_LLM restore

Scope:
- Sample: `/mnt/e/Download/minsky86.pdf` (`5,599,744` bytes)
- Primary notebook: `abc47464-3b9c-4b8f-b4fe-58c40baf559e`
- Primary document: `e1a2052e-53f9-4e93-b817-ed82e35cebeb`
- Runtime config confirmed on both API and worker: `SUMMARY_LLM_BASE_URL=https://www.dmxapi.cn/v1`, `SUMMARY_LLM_MODEL=gemini-3.1-flash-lite-preview`, `MM_EMBEDDING_DIMENSIONS=1024`, `MILVUS_MULTIMODAL_VECTOR_DIM=1024`.

Preflight:
- `scripts/sync-keys.sh --check`: all required keys configured.
- API `/health`: 200, `api` + `postgres:ok`.
- API `/ready`: 200, `postgres:ok`.
- Milvus collections present: text chunks, multimodal chunks, KG entities, KG relations, graph passages.

Ingestion result:
- Final document status: `completed`.
- PDF route plan: `total_pages=407`, `edgeparse_pages=363`, `mineru_ocr_pages=44`.
- MinerU OCR batch upload used `file_count=44`.
- Low-value OCR pages skipped after OCR: `120`, `341`, `342`.
- IR chunk plan: `text_chunks=881`, `multimodal_chunks=45`.
- `document_parse_runs`: `status=completed`, `duration_ms=646719`.
- Postgres chunks: `body=881`, `summary=1`; Postgres multimodal chunks: `45`.
- The summary chunk is an LLM summary (`length=1765`, starts with `【文档组织型】`), not the previous naive fallback.

Milvus / retrieval-plane counts for the primary document:
- `avrag_rag_text_chunks`: `881`
- `avrag_rag_multimodal_chunks`: `45`
- `avrag_rag_kg_entities`: `894`
- `avrag_rag_kg_relations`: `640`
- `avrag_rag_graph_passages`: `640`

RAG chat validation:
- `POST /api/v1/chat` returned HTTP `200`.
- SSE events: `start=1`, `activity=4`, `answer_start=1`, `token=183`, `citations=1`, `done=1`.
- Final Chinese answer length: `375` characters.
- Citation items returned: `30`.
- Chat artifact: `.hermes/runs/e2e-minsky86-1777384972-chat.json`.

Additional observation:
- A duplicate foreground upload created document `9695f744-49d7-48b0-a42f-010d3c23cdac`; it also reached `completed` with `chunk_count=881`. Chat validation above used the primary completed document.

Conclusion:
- Full live E2E passed: upload -> OCR/parse -> summary -> text/multimodal/KG indexing -> RAG SSE answer -> citations.
- The previous SUMMARY_LLM 401/degrade issue did not reproduce in this full run; KG entities/relations/graph passages were populated at scale.

