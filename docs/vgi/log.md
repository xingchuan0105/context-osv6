# Regeneration log

Record misread prose and failed anchors here. Point at the module doc, not the generated file.

- 2026-09-14 `embed.md`: first embed CLI `SELECT body` 全表进 `Vec`，RSS 3.6–4.9 GB。文档级 fp16 全库只有 ~0.36 GB。已改成按篇流式，禁止一次性装正文。
- 2026-09-14 `retrieval.md` / zvec: `Doc::get_score` on cosine HNSW is a **distance** (hello/hello ≈ 0, hello/world ≈ 0.45). Spec max_pool 要相似度。实现取负后再 max_pool。已写进 retrieval.md。

- 2026-09-14 `zvec-spike.md`: `Collection::create_and_open` rejects an already-created directory (`path exists, create expects a path that does not exist`). First spike run created the dir then failed; fallback `hnsw_rs` wrote a valid report. Regenerated spike after passing a non-existent path.
- 2026-09-14 `zvec-spike.md`: `SearchQuery::new(&[f32])` passes `size_of_val` bytes; against a VectorFp16 field zvec counted 1024 f32 as 2048 fp16 dims. Query path now sets packed fp16 bytes via the C API.
- 2026-09-14 `token-batch.md`: `ureq` could not reach HuggingFace (`Network is unreachable`) while `curl` through the WSL proxy could. Tokenizer download falls back to `curl`. Official `tokenizer.json` sha256 `21106b6d7dab2952c1d496fb21d5dc9db75c28ed361a05f5020bbba27810dd08`.
- 2026-09-14 overnight-report（旧稿，非规格）：写「9 unit tests」及 spike `p95_query_ms=2.671295` / `rss_bytes=97501184`。本会话 `CARGO_BUILD_JOBS=2 cargo test` 为 20 个单元测试（vgi 0 + vgi_core 5 + vgi_ingest 15）；磁盘 `/home/chuan/vgi-rs/.vgi/jobs/zvec-spike.json` 为 `p95_query_ms=2.572029`、`rss_bytes=97734656`。报告不当证据，四门仍绿。
- 2026-09-14 overnight-report（旧稿命令）：`--corpus-uri`、`--fixture two-doc` 与现行 CLI 不符。`corpus.md` 接口名是 `corpus_uri`；bin 旗标是 `--uri` 与布尔 `--fixture`。
- 2026-09-16 `embed.md`（qwen-flash profile）："HF tokenizer 会少计约 3%" 不成立。同一窗 API 实测/本地可达 1.17×（数字密集内容更高）；全库 14 条 120k 本地窗被 API 数成 131,221–140,166 token，400 `InvalidParameter: maximum context length is 131072`。第一次同步全库就是死在这 14 条中的第一条。嵌入路径已改为：多文本请求遇超限先拆单条重试；单条按上报比例裁剪（`keep = ⌊chars × 0.97·max/got⌋`）后重试，最多 4 次。裁剪后实测 126,959 / 126,316 token，低于 131,072。
- 2026-09-16 `embed.md`（Batch File）：百炼 Batch 转发层大面积 `ForwardingTransportError`（`vLLM Error:` 后为空，HTTP 400）。原始批次 100,011 请求失败 72,076（72%，各尺寸/位置/窗口序号均匀）；当日探针复现：~60 token 文本 20/20 通过、~500 token 0/20、~3000 token 1/20；同请求重复提交结果稳定，走新加坡代理与走杭州直连结果一致，`dimensions`/`custom_id`/多行文本均无影响。同步接口对同样样本 100% 成功（含 122,072 token 单请求）。结论：Batch 当前不可用，qwen-flash 灌库改走同步 `vgi embed --profile qwen-flash`；embed.md 原「灌库走 Batch File」已改。隔离实验：免费测试模型 `batch-test-model`（跳过推理）10/10 成功、约 60 s 完成 → 上传/调度/结果文件管道正常，故障只在「batch → vLLM 推理」这一段；管理接口无并发压力（上限 1000 并行任务，我们最多 ~30）。
