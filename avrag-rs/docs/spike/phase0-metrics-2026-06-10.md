# Phase 0 Metrics (auto-generated)

- PDF: `docs/spike/fixtures/phase0-mini.pdf`（8 页合成夹具；完整 Black Swan 567 页待 `E2E_LLM_REAL_BLACK_SWAN_PDF` 挂载后重跑）
- Pages rendered: 8

**结论摘要（夹具）**：1 页/chunk 单图 token ≈280；4 页 fusion 单请求 token ≈1120（≈4×），embed 成功率 2/2。完整语料召回对比见 `recall_probe` JSON 字段。

## Strategies

### one_page
- pages_per_chunk: 1
- render_ms: 111.2
- embed_ok/fail: 8/0
- avg_embed_latency_ms: 326.3
- image_tokens_avg: 280.0

### four_page_fusion
- pages_per_chunk: 4
- render_ms: 85.8
- embed_ok/fail: 2/0
- avg_embed_latency_ms: 521.7
- image_tokens_avg: 1120.0
