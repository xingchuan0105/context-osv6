# Overnight goal — VGI M0 regenerate

给 `/goal` 用的目标原文（本会话已用同内容启动 workflow `vgi-m0-overnight`；不必再敲一遍，除非 TUI 被关掉后要续）。

```
/goal VGI M0 regenerate from docs/vgi into /home/chuan/vgi-rs. Unattended. --budget 800000
```

## Objective (copy for /goal)

Regenerate VGI milestone M0 from the design-doc DAG at `/home/chuan/context-osv6/docs/vgi/` into an independent Rust workspace at `/home/chuan/vgi-rs`. Do not wait for a human. Follow `docs/vgi/AGENTS.md`: code is G(spec); one module at a time; worked examples win; log misreads in `docs/vgi/log.md`.

## Done when all of these exist on disk (evidence review must reproduce them)

1. `python3 /home/chuan/context-osv6/docs/vgi/anchors/test_anchors.py` exits 0.
2. `/home/chuan/vgi-rs` is a Cargo workspace (`vgi-core`, `vgi-ingest`, bin `vgi`) with `.cargo/config.toml` `jobs = 2`. `CARGO_BUILD_JOBS=2 cargo test` in that tree exits 0. Do not build `avrag-rs`.
3. Fixture ingest of the two-doc worked example in `corpus.md` (`rev0`, hello/world) produces `corpus_fp = 5f76e86df681122f018c02203b95a682d692f37e5c88de93ba3643c786d500b2` on two consecutive runs.
4. `/home/chuan/vgi-rs/.vgi/jobs/zvec-spike.json` exists with `ok=true`, `n=10000`, `dim=1024`, `dtype=fp16`, `backend` one of `zvec-rust`|`hnsw_rs`.
5. `/home/chuan/context-osv6/docs/vgi/overnight-report.md` lists what passed, what was skipped, exact commands, and file paths.

## Full-corpus ingest (best-effort, not required for "partial done")

Parquet shards: `/mnt/c/Users/xingc/Documents/Codex/repository-tree/.eval/hard/challenge20-v1/shards` (revision `b27b02bc3e45511b8b82a13e6f90ce761df726f6`). If present and `df` avail ≥ 10G: ingest into `/home/chuan/vgi-rs/.vgi/`. `n_docs` must equal 100195; if not, record the measured N in the report and in `docs/vgi/log.md` and do not silently accept. Two full ingests must yield the same `corpus_fp`. Then fill `token_count` for every row (tokenizer `BAAI/bge-m3`). If shards missing or disk would drop under 8G, skip and say so.

## Hard stops (never do)

HTTP embedding, Milvus, MCP, code-sandbox, `vgi install`, `docker-compose`, prune `avrag-test-pg-*`, `git push`, spend paid embedding tokens, add crates to the context-osv6 workspace, invent 18M `chunk_id`.

## Repair policy

If zvec-rust fails to link, use `hnsw_rs` with the same `SpikeReport`. If `cargo test` fails, fix from the spec (not by weakening the anchor). After two repair rounds still red: write the report as **partial** with the failing command output. Do not pause for a human.

## Session

Leave this TUI running. Closing the process interrupts the workflow (not resumable). Windows sleep may suspend WSL.
