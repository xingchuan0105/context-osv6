# Regeneration log

Record misread prose and failed anchors here. Point at the module doc, not the generated file.

- 2026-09-14 `embed.md`: first embed CLI `SELECT body` 全表进 `Vec`，RSS 3.6–4.9 GB。文档级 fp16 全库只有 ~0.36 GB。已改成按篇流式，禁止一次性装正文。
- 2026-09-14 `retrieval.md` / zvec: `Doc::get_score` on cosine HNSW is a **distance** (hello/hello ≈ 0, hello/world ≈ 0.45). Spec max_pool 要相似度。实现取负后再 max_pool。已写进 retrieval.md。

- 2026-09-14 `zvec-spike.md`: `Collection::create_and_open` rejects an already-created directory (`path exists, create expects a path that does not exist`). First spike run created the dir then failed; fallback `hnsw_rs` wrote a valid report. Regenerated spike after passing a non-existent path.
- 2026-09-14 `zvec-spike.md`: `SearchQuery::new(&[f32])` passes `size_of_val` bytes; against a VectorFp16 field zvec counted 1024 f32 as 2048 fp16 dims. Query path now sets packed fp16 bytes via the C API.
- 2026-09-14 `token-batch.md`: `ureq` could not reach HuggingFace (`Network is unreachable`) while `curl` through the WSL proxy could. Tokenizer download falls back to `curl`. Official `tokenizer.json` sha256 `21106b6d7dab2952c1d496fb21d5dc9db75c28ed361a05f5020bbba27810dd08`.
- 2026-09-14 overnight-report（旧稿，非规格）：写「9 unit tests」及 spike `p95_query_ms=2.671295` / `rss_bytes=97501184`。本会话 `CARGO_BUILD_JOBS=2 cargo test` 为 20 个单元测试（vgi 0 + vgi_core 5 + vgi_ingest 15）；磁盘 `/home/chuan/vgi-rs/.vgi/jobs/zvec-spike.json` 为 `p95_query_ms=2.572029`、`rss_bytes=97734656`。报告不当证据，四门仍绿。
- 2026-09-14 overnight-report（旧稿命令）：`--corpus-uri`、`--fixture two-doc` 与现行 CLI 不符。`corpus.md` 接口名是 `corpus_uri`；bin 旗标是 `--uri` 与布尔 `--fixture`。
