# VGI-RAG 交接（PRD 之后）

日期：2026-09-14 深夜。给**从 zg 对齐设计稿写完后就没跟进**的 agent。先读本页，再动代码。不要从 `docs/plans/2026-09-13-vgi-rag-rust-zg-aligned-design.md` 开工——那篇已 SUPERSEDED。

## 你接手的是什么

VGI-RAG 是 **BrowseComp-Plus 十万篇上的评测工具**，不是云端工作台，也不是 Subtex 目录插件。循环属于 Codex/Claude 宿主；VGI 只给检索。代码在独立仓 **`/home/chuan/vgi-rs`**，规格在本目录 **`docs/vgi/`**。不要把 crate 加进 `avrag-rs` workspace，不要 `docker-compose`，不要 push。

产品仓：`/home/chuan/context-osv6`（`master`，未推远程）。  
实现仓：`/home/chuan/vgi-rs`（独立 git，也未推）。  
凭据：`avrag-rs/.env`（SiliconFlow `EMBEDDING_*`、百炼 `DASHSCOPE_API_KEY` / `E2E_EMBEDDING_BASE_URL`）。不要打印 key。

根 `AGENTS.md` 路由：`VGI-RAG` → [AGENTS.md](AGENTS.md)。过程是 Kushnir 范式的子集：改规格再再生代码，worked example 赢过散文，锚点在 `anchors/test_anchors.py`。

## PRD 之后发生了什么（按时间）

1. **Challenge-20 题集与 run-9 树消融**（Windows 原型 `C:/Users/xingc/Documents/Codex/repository-tree/`，本仓只留分析稿）。四臂 80/80。结论：树无增益；根因是循环里**没有排序检索** + 证据跨主题（19/20 题最小覆盖=根）+ 5 轮 grep 超时。不是「树这个想法被否」。
2. **zg 对齐设计稿** `docs/plans/2026-09-13-vgi-rag-rust-zg-aligned-design.md`：独立 `vgi-rs`、本机 zvec + 自研 BM25、文档级向量、树 v2、工具/代码双模态。评审 **needs revision**（1 critical / 9 major）：指纹空、非目标与 Milvus/SAC 矛盾、12 crate、M1/M3 门不可比或达不到。见 [评审](../reviews/2026-09-13-vgi-rag-rust-zg-aligned-design-review.md)。
3. **按评审重写为设计文档 DAG** `docs/vgi/`（2026-09-14）。M0 收成 ingest + token + zvec spike。云/SAC/自研倒排/代码模式进 [deferred.md](deferred.md)。
4. **过夜 M0** workflow `vgi-m0-overnight`：四门绿。全库 `n_docs=100195`，`corpus_fp=d2e699e3…`，bge-m3 `token_count` 合计 819,220,179，176,818 窗。
5. **M1 向量路**：`read` / scoped `rg` / `search_vector`（max 到 `doc_id`）。bge-m3 全库 embed **已完成**（SiliconFlow `Pro/BAAI/bge-m3`，1024-d fp16，zvec 约 361 MB）。
6. **第二套索引 qwen-flash**：百炼 `qwen3.7-text-embedding-flash`，256-d fp16，窗 120k（见下）。同步灌库被 TPM 1M 卡成 ~13h 且 429，已停，改 **Batch File（半价）**。JSONL 准备进行中。

## 现行规格（只信这些）

| 读 | 路径 |
|---|---|
| 过程 | [AGENTS.md](AGENTS.md) |
| DAG / 里程碑 | [README.md](README.md) |
| 意图与评审决议 | [overview.md](overview.md) |
| 语料 / 指纹 | [corpus.md](corpus.md) |
| bge 窗公式 | [token-batch.md](token-batch.md) |
| 嵌入 / qwen profile / Batch | [embed.md](embed.md) |
| 检索身份 | [retrieval.md](retrieval.md) |
| 评测口径 | [eval.md](eval.md) |
| 本轮 20 题对照（含冻结上一轮） | [eval-m1-vector-ab.md](eval-m1-vector-ab.md) |
| 踩坑 | [log.md](log.md) |
| 树 / BM25 引擎 / MCP / 沙箱 | [deferred.md](deferred.md) — **默认不要实现** |

已取代、仅供对照：`docs/plans/2026-09-13-vgi-rag-rust-zg-aligned-design.md`。

Windows 原型与 run-9 回执仍在 `C:/Users/xingc/Documents/Codex/repository-tree/`（WSL：`/mnt/c/Users/xingc/Documents/Codex/repository-tree/`）。语料 parquet：`.eval/hard/challenge20-v1/shards/`，revision `b27b02bc3e45511b8b82a13e6f90ce761df726f6`。题集副本：`docs/vgi/fixtures/challenge20-questions.json`（`id` / `question` / `evidence_ids`）。

## 不要重跑、不要推翻

- **run-9** grep / tree / tree-nolabel / tree-random：冻结在 `fixtures/challenge20-prior-frozen.json`。
- **文档级 BM25** 校准（@100=0.111 @1000=0.275）：同一冻结文件。不要为了对照再扫一遍 10 万篇。
- **bge-m3 全库向量**：`window_vectors` 176,818 行，`.vgi/zvec-docs/`。禁止 `DROP`、禁止写进同一张表。
- 官方稠密 26.4%@100 / 59.7%@1000：全库 qrels + 另一 encoder，**不是及格线**。
- 金标是 **`evidence_ids`**，不是更窄的 `gold_doc_ids`。
- 融合身份永远是 **`doc_id`**。没有 18M `chunk_id`。zvec cosine `get_score` 是**距离**，max_pool 前取负。

## 代码与数据落点

`/home/chuan/vgi-rs` crates：`vgi-core`、`vgi-ingest`、`vgi-search`、bin `vgi`。`CARGO_BUILD_JOBS=2`（`.cargo/config.toml`）。

数据目录默认 `/home/chuan/vgi-rs/.vgi/`：

| 东西 | 状态（写本文时） |
|---|---|
| `corpus.sqlite` `documents` | 100,195 行，正文在 sqlite |
| `documents.token_count` | bge-m3 XLM-R，sum=819,220,179 |
| `window_vectors` + `zvec-docs/` | **bge-m3 完成** |
| `doc_tokens` profile=`qwen-flash` | 100,195 行已填（Qwen/Qwen3-8B tokenizer） |
| `window_vectors_qwen_flash` | 仅 1,666 行（同步半路停下，Batch 会跳过） |
| `zvec-docs-qwen-flash/` | 还没有 |
| `batch-qwen-flash/input-*.jsonl` | **正在 prepare**（约 4 万条 / 目标 ~10 万，已有约 1.4GB 分片） |
| `jobs/eval-bge-m3.json` | 还没有（waiter 等 qwen 索引） |
| `tokenizer.json` / `tokenizer-qwen3.json` | 已缓存；sha 在 `vgi-core/src/profile.rs` |

CLI 摘要：

```bash
# 数据目录
/home/chuan/vgi-rs/target/release/vgi --data-dir /home/chuan/vgi-rs/.vgi …

ingest --uri <shards> --revision b27b02bc3e45511b8b82a13e6f90ce761df726f6 --expected-n 100195
ingest --fixture
tokens --profile bge-m3|qwen-flash
embed --profile bge-m3          # 同步；bge 已完成，不要重跑全库
embed --profile qwen-flash      # 同步；已废弃，走 Batch
batch-prepare --profile qwen-flash
batch-ingest --profile qwen-flash --results .vgi/batch-qwen-flash
index --profile bge-m3|qwen-flash
search --query '…' --profile bge-m3|qwen-flash --env avrag-rs/.env
read --doc-id 5412 --offset 0 --limit 80
rg --pattern Arwa --doc-id 5412     # doc-id 必填
eval --questions docs/vgi/fixtures/challenge20-questions.json --profile bge-m3|qwen-flash --env …
```

Batch 流水线脚本（当前应在跑）：

- `scripts/qwen-batch-run.sh` → prepare → `qwen-batch-submit.py` → ingest → zvec
- 日志：`.vgi/jobs/qwen-batch.log`
- 评测 waiter：`scripts/wait-eval-vector-ab.sh` → `eval-bge-m3.json` + `eval-qwen-flash.json` + `eval-vector-ab.md`（合并冻结上一轮）

## 本轮 20 题测试方案（已锁）

只比**两条向量路**。无 BM25、RRF、rg、树、agent。

- 题：Challenge-20，20 道，fixture 如上
- 金标：`evidence_ids`
- 检索：整题一条 query 向量 → ANN `window_k=10000` → **max** 到 `doc_id`
- 指标：20 题 macro-average recall@5 / @100 / @1000
- 臂 A：bge-m3，窗 7372 / overlap 256 / 1024-d
- 臂 B：qwen-flash，窗 **120000** / overlap 256 / 256-d（用户要 128k；HF tokenizer 少计约 3%，128000 会被 API 报 131841>131072，所以 cap 在 120k）
- 与上一轮合并：索引级一张表（冻结 BM25 + 两臂向量）；agent 级 run-9 另表。见 `eval-m1-vector-ab.md`

## 当前机器上在跑什么

写本文时（日志停在 `batch prepare: 44000 requests` 一带）：

- `qwen-batch-run.sh` + `vgi batch-prepare --profile qwen-flash`：出 JSONL
- `wait-eval-vector-ab.sh`：等 `zvec-docs-qwen-flash` 或日志 `DONE`
- **没有**同步 `embed --profile qwen-flash`

prepare 之后才会上传百炼（单文件 ≤500MB / 4.9 万行，endpoint `/v1/embeddings`，`completion_window=24h`）。Batch **不保证**比 13 小时同步更快，只保证大约半价（flash 同步 ~0.000125 元/千 token，Batch ~0.000063）。

磁盘：`/` 约 17G 空闲。JSONL 全文大约数 GB，注意不要把盘写满。

## 你接手后先做什么

1. `tail -f /home/chuan/vgi-rs/.vgi/jobs/qwen-batch.log`。prepare 结束后应出现 `n_requests=` 和 `qwen-batch-submit.py` 的 `uploaded` / `batch … status=`。
2. 若 prepare/submit 死了：不要重跑 bge；从 `batch-qwen-flash/jobs.json` 和已有 `input-*.jsonl` 续（submit 脚本按 shard 名跳过已提交）。
3. Batch `completed` 后确认 `vgi batch-ingest` 与 `vgi index --profile qwen-flash`。然后跑（或让 waiter 跑）两次 `eval`。
4. 对照写进 `docs/vgi/eval-m1-vector-ab-results.md`（waiter 也会写 `.vgi/jobs/eval-vector-ab.md`）。
5. **不要**开 M2 词法、M3 树、M4 MCP，除非用户点名。

## 已知坑（不要再踩）

- 全表 `SELECT body` 进 `Vec` → RSS 4–5GB。流式按篇。文档级向量本身只有 ~0.36GB（bge 1024-d）/ ~52MB（qwen 256-d）。
- `create_and_open` 不能对着已存在目录。
- fp16 查询必须走 packed bytes，不能按 f32 字节数。
- HuggingFace 在 WSL 常不通，tokenizer 用 curl。
- ureq 把 HTTP 429 变成 `Error::Status`；同步路径已退避。Batch 不应再打 TPM。
- 漏桶从满桶启动会 7 路打爆 1M token → 429。已改空桶；Batch 替代同步。
- CLI 是 `--uri` / `--fixture`，不是 overnight 旧稿里的 `--corpus-uri`。

## 里程碑对照（避免按旧 PRD 铺 12 crate）

| 旧稿 | 现行 |
|---|---|
| 12 crate + vgi-store + Milvus | 3 crate + 1 bin；无云 |
| 自研倒排默认 | 冻结 BM25 数字；引擎待 M2 先 oracle |
| M1 向量 ≥ 官方 26.4/59.7 | 只报 Challenge-20 绝对 recall |
| M3 优于 run-9 根覆盖 | 不作为门 |
| 代码模式 fd3/4 | deferred；Windows 实为 TCP |
| 对齐 zg 四工具 + 托管 rg | 默认将来 `vgi_search`+`vgi_read`；rg 必须带 `doc_ids` |

M0 **已完成**。M1 向量路代码 **已完成**；bge 索引 **已完成**；qwen 索引与 20 题数字 **未完成**（卡在 Batch JSONL / 上传）。
