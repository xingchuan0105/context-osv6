# VGI M0 overnight report

Date: 2026-09-14. Workspace: `/home/chuan/vgi-rs`. Spec: `/home/chuan/context-osv6/docs/vgi/`.

## Result: done

All overnight gates passed. Full-corpus ingest and token fill also completed (not required for partial-done).

## Passed

| Gate | Evidence |
|---|---|
| Anchors | `python3 /home/chuan/context-osv6/docs/vgi/anchors/test_anchors.py` → 4 tests, OK |
| Cargo workspace | `vgi-core`, `vgi-ingest`, bin `vgi`; `.cargo/config.toml` has `jobs = 2` |
| `cargo test` | `CARGO_BUILD_JOBS=2 cargo test` in `/home/chuan/vgi-rs` exits 0 (9 unit tests) |
| Two-doc fixture ingest ×2 | `corpus_fp = 5f76e86df681122f018c02203b95a682d692f37e5c88de93ba3643c786d500b2` both runs |
| zvec spike report | `/home/chuan/vgi-rs/.vgi/jobs/zvec-spike.json`: `ok=true`, `n=10000`, `dim=1024`, `dtype=fp16`, `backend=zvec-rust`, `p95_query_ms=2.671295`, `rss_bytes=97501184` |
| Full ingest ×2 | `n_docs=100195`, same `corpus_fp=d2e699e3a92363473af1b9bea7b1e7be0c664ec9b03144a1944fadc00af6d3fb` |
| Token fill | every row has `token_count`; `sum_token_count=819220179`; `n_windows=176818`; `truncated_docs=86`; `tokenizer=BAAI/bge-m3` |

## Skipped

Nothing required. HTTP embedding / Milvus / MCP / sandbox / docker-compose / git push / product-workspace crates were not used.

## Commands

```bash
python3 /home/chuan/context-osv6/docs/vgi/anchors/test_anchors.py
cd /home/chuan/vgi-rs
CARGO_BUILD_JOBS=2 cargo test
./target/debug/vgi ingest --fixture two-doc --data-dir /tmp/vgi-two-doc   # ×2
./target/debug/vgi spike-zvec --data-dir /home/chuan/vgi-rs/.vgi --n 10000 --dim 1024
./target/debug/vgi ingest \
  --corpus-uri /mnt/c/Users/xingc/Documents/Codex/repository-tree/.eval/hard/challenge20-v1/shards \
  --data-dir /home/chuan/vgi-rs/.vgi \
  --revision b27b02bc3e45511b8b82a13e6f90ce761df726f6 \
  --expected-n 100195   # ×2
# tokenizer.json via curl (ureq had no proxy); sha256 21106b6d7dab2952c1d496fb21d5dc9db75c28ed361a05f5020bbba27810dd08
CARGO_BUILD_JOBS=2 cargo build --release -p vgi
./target/release/vgi tokens --data-dir /home/chuan/vgi-rs/.vgi
```

## Paths

- Spec DAG: `/home/chuan/context-osv6/docs/vgi/README.md`
- Code: `/home/chuan/vgi-rs/{Cargo.toml,vgi-core,vgi-ingest,vgi}`
- Data: `/home/chuan/vgi-rs/.vgi/corpus.sqlite`, `manifest.json`, `jobs/zvec-spike.json`
- Parquet columns written: `doc_id=docid`, `body=text`, `heading=""`, `url=url`
- Misreads: `/home/chuan/context-osv6/docs/vgi/log.md`

## Notes

- `libzvec_c_api.so` is not on the default loader path; `vgi/build.rs` copies it to `target/{debug,release}/native` and sets RUNPATH.
- First zvec attempts failed (pre-created collection dir; fp16 query byte length). Fallback `hnsw_rs` produced a valid report; after those two API fixes the spike backend is `zvec-rust`.
- Token fill used the official XLM-RoBERTa `tokenizer.json` with special tokens off. Window count `176818` is from real `token_count`, not the 176,438 4-character heuristic.
