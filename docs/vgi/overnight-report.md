# VGI M0 过夜报告

日期：2026-09-14。规格：`/home/chuan/context-osv6/docs/vgi/`。代码：`/home/chuan/vgi-rs`。

## 结果

**done**

四门均绿。证据评审 `real=true`，`verify_ok=true`，`gaps=[]`。全库 ingest 已跑（`did_full=true`），不决定四门。

## 证据路径

本会话复跑与读取（不以旧报告为据）：

- `python3 /home/chuan/context-osv6/docs/vgi/anchors/test_anchors.py` → 4 tests, OK, EXIT:0
- `cd /home/chuan/vgi-rs && CARGO_BUILD_JOBS=2 cargo test` → Finished 0.18s；vgi 0、vgi_core 5（`corpus_fp_two_doc`、`corpus_fp_unsorted_without_sort_is_different`、`token_windows_anchors`、`stride_and_cap_constants`、`spike_report_json_fields`）、vgi_ingest 15（含 `two_doc_ingest_twice_same_fp`、`parquet_two_doc_shuffled_matches_anchor_fp`）；EXIT:0
- `/tmp/vgi-two-doc/manifest.json`：`corpus_fp=5f76e86df681122f018c02203b95a682d692f37e5c88de93ba3643c786d500b2`，`n_docs=2`，`corpus_revision=rev0`
- `/tmp/vgi-two-doc/corpus.sqlite`：`a/hello`（heading/url 空）、`b/world`（`W` / `http://x`）
- `/home/chuan/vgi-rs/.vgi/jobs/zvec-spike.json`：`ok=true`，`n=10000`，`dim=1024`，`dtype=fp16`，`backend=zvec-rust`，`error=null`，`p95_query_ms=2.572029`，`rss_bytes=97734656`
- `/home/chuan/vgi-rs/.cargo/config.toml`：`jobs = 2`

旧稿「9 unit tests」及 spike `p95=2.671295` / `rss=97501184` 与磁盘和本会话 `cargo test`（20）不符，不当证据。全库 `/.vgi/manifest.json` 只作 ingest 记录，不计入四门。

## 跳过项

未做：HTTP embedding、Milvus、MCP、sandbox、docker-compose、git push、向产品 workspace 加 crate、发明 `chunk_id`。未编 `avrag-rs`。

## 全库 ingest 是否跑

是。`/home/chuan/vgi-rs/.vgi/manifest.json`：`n_docs=100195`，`corpus_revision=b27b02bc3e45511b8b82a13e6f90ce761df726f6`，`corpus_fp=d2e699e3a92363473af1b9bea7b1e7be0c664ec9b03144a1944fadc00af6d3fb`，`tokenizer=BAAI/bge-m3`，列 `docid` / `text` / `""` / `url`。全表 `token_count` 非空；`sum=819220179`，`n_windows=176818`，`truncated_docs=86`。

现行 CLI 是 `vgi ingest --uri …` 与 `--fixture`（布尔），不是 `--corpus-uri` / `--fixture two-doc`。

## cargo/zvec 后端

- cargo：20 个单元测试绿；workspace `vgi-core`、`vgi-ingest`、bin `vgi`
- spike 后端：`zvec-rust`（未回退 `hnsw_rs`）。p95 记入报告，不是失败门

## 明天建议

M1：retrieval 向量路 + `read` + `rg`；eval 报绝对 recall，不用官方 26.4%/59.7% 当及格线。嵌入估价用 `sum_token_count=819220179` 与 176818 窗，勿沿用 176,438 的 4 字符启发式。融合身份是 `doc_id`，不要发明 `chunk_id`。
