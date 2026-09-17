# VGI-RAG 交接（M3 之前）

日期：2026-09-17 凌晨。给**接着做 M3/M4 或继续评测**的 agent。先读本页，再动代码。过程规则仍在 [AGENTS.md](AGENTS.md)，踩坑合并进 [log.md](log.md)。不要从 `docs/plans/2026-09-13-vgi-rag-rust-zg-aligned-design.md` 开工——那篇已 SUPERSEDED。

VGI 是 **BrowseComp-Plus 十万篇上的评测工具**。循环属于宿主（Codex/Claude），VGI 只给检索。代码在独立仓 **`/home/chuan/vgi-rs`**，规格在本目录 **`docs/vgi/`**；不要把 crate 加进 `avrag-rs` workspace，不要 `docker-compose`，不要 push。凭据读 `avrag-rs/.env`（`DASHSCOPE_API_KEY`、`EMBEDDING_*`），不要打印。

---

## 一、当前状态：M0/M1/M2 全部完成

| 里程碑 | 状态 |
|---|---|
| M0 | **完成**（ingest + token + zvec spike 四门绿） |
| M1 | **完成**（bge-m3 176,818 窗 + qwen-flash 101,677 窗两套向量索引；索引级 recall 已报；答题级基线 25%） |
| **M2** | **完成**（tantivy 段落级 BM25 + `rrf_merge` hybrid；索引级门**已过**；答题级对照已跑） |
| M3 | **门未过，不做**（tree oracle：2 级球面 kmeans 路由逐档劣于 flat，见 results） |
| **M4** | **实现完成，答题臂在验**（命中定位 search_hits + 文档质心图 related + thinking + zvec 式 prompt；依据 [research-non-llm-structures.md](research-non-llm-structures.md)） |

**M2 索引级结果**（Challenge-20 `evidence_ids`，全文见 [eval-m1-vector-ab-results.md](eval-m1-vector-ab-results.md)）：

| arm | @5 | @100 | @1000 |
|---|---:|---:|---:|
| bge-m3 向量 | 0.053 | 0.211 | 0.471 |
| qwen-flash 向量 | 0.040 | 0.217 | 0.450 |
| tantivy 段落级 BM25 | 0.057 | 0.217 | 0.480 |
| **hybrid RRF(60)** | 0.052 | **0.236** | **0.507** |

门判定：`hybrid ≥ 单路最优` 在 @100（0.236 > 0.217）与 @1000（0.507 > 0.480）成立，@5 持平。oracle 用 **bm25s lucene k1=1.2/b=0.75 替身**（官方 2.17GB Lucene+Java 被磁盘否掉；文档级复测 0.141/0.300 ≈ 冻结 0.111/0.275）。

**M2 答题级对照**（同 20 题、同 Eval v2 裁判）：5 轮预算下 vector **25%** → hybrid **20%** → hybrid+rr200 **17.5%**；放宽到 20 轮/64 调用/1800s 后 **vector 35% = hybrid+rr200 35%**（各 7/20）。归因干净：预算是唯一大杠杆（+10pp），检索臂间差异在答题级不转化——但两臂题集互补（并集 9/20=45%），ensemble/投票是答题级剩余空间。

**rerank 阶段**（M2 后追加，非新里程碑）：`--rerank N` 对任意 mode 的 RankedList 前 N 位走 Bailian `gte-rerank-v2` 重排（`body[:1500]` 摘要、~90k 字符/请求分块、同分保原序）。索引级：hybrid+rr200 @100=**0.268**（+13.6% 相对 vs hybrid）是当前最强单指标；@5 持平。oracle（`scripts/rerank_oracle.py`）与 Rust 路径逐位一致。

## 二、检索面（现行）

`vgi search --mode vector|lexical|hybrid` / `vgi eval --mode …`：

- **vector**：zvec（bge-m3 或 qwen-flash profile）→ `max_pool` → doc_id
- **lexical**：tantivy 段落索引 `.vgi/tantivy-passage/`（**1,147,822** passages，W=512/重叠64 raw-token 窗；`doc_id` STRING stored、body indexed-not-stored `WithFreqs`；BM25 k1=1.2/b=0.75；默认 `passage_k=20000` → max_pool 到 doc_id）
- **hybrid**：`rrf_merge([vector, lexical], k=60)`
- **rerank**：`--rerank N`（默认 0 关）对任意 mode 重排前 N 位
- **search 命中定位（M4）**：`vgi search` 输出 `[{doc_id, offset, snippet}]`——offset/snippet 是该 doc 最强命中单元（向量窗 batch→`window_offsets` 表查 char 起点；词法 passage 的 `off` 字段在建索引时落盘）的 char 位置与 600 字符片段；hybrid 取排名更靠前一路的单元
- **related（M4）**：`vgi related --doc-id X --k 10` → 文档质心（窗向量均值 L2 归一化，`zvec-centroids` 集合）ANN top-k 邻居，含 url+snippet 头；**平铺关联图，非树**
- `vgi read`（char 窗）、`vgi rg`（doc_id 必填）不变

agent 级脚本 `scripts/challenge20-answer-eval.py`：环境变量切臂 `ANSWER_EVAL_{MODE,RERANK,THINKING,MAX_ROUNDS,MAX_TOOLS,DEADLINE_S,OUT}`。arm 标签已动态化。思维链落盘 `reasoning-{qid}.txt`（需在进程启动前改代码——Python 不热加载）。

**M4 部分结果（4/20 题后暂停）**：thinking=True 的 hybrid+rr200 臂 — bcp:22 **PASS**（此前两臂均 INCORRECT，3min/10调用）、bcp:342 首轮 INCORRECT（llm_error 超时，20次search零read）但重采思维链时 PASS（19min）、bcp:618 INCORRECT（deadline 空答案）、bcp:70 INCORRECT（但 ev_read=5，命中定位让 agent 真读到了证据）。thinking 病理：长链挤占 read、deadline 时空答案、`vgi_related` 0 次使用。思维链样本：`.eval/chains-samples/reasoning-{22,342}.txt`。

**Prompt 已对齐 zvec-rg 风格**：描述性语义（候选≠完备、相似≠支持、零命中≠不存在），无指令式规则，预算不嵌 skill（closeout 信号负责）。冒烟 bcp:435 PASS 78s/5调用。

## 三、oracle 与预测工具（.vgi 数据）

| 东西 | 状态 |
|---|---|
| `.vgi/tantivy-passage/` | M2 引擎索引，565MB；`vgi index --lexical` 重建（~82s，全量重建） |
| `.vgi/bm25s-passage-idx/` | bm25s 段落索引存档 2.1GB，mmap 可载，`bm25s_passage_oracle.py --load-idx` 秒级复现实验 |
| `.vgi/jobs/bm25s-oracle-challenge20.json` | 文档级 oracle（含 per-question hits） |
| `.vgi/jobs/bm25s-passage-challenge20.json` | 段落级 oracle + Python 侧 hybrid 预测 |
| `.vgi/jobs/hybrid-oracle-challenge20.json` | 含每题 `vector_list`（可复用免跑 embed） |
| `.vgi/jobs/eval-bge-m3{,-lexical,-hybrid}.json` `eval-qwen-flash.json` | 索引级报告 |
| `.vgi/zvec-centroids/` | M4 文档质心索引（100,195 × 1024d，`vgi index --centroids` 从 window_vectors 派生） |
| `window_offsets` 表（corpus.sqlite） | M4 向量窗 char 偏移（`vgi index --offsets` 一遍重分词填充） |
| `scripts/{graph_oracle,doclevel_oracle,failure_attribution}.py` | M4 oracle：图边覆盖 44/207、文档质心 @100=0.211 ≈ 窗口级、逐题导航归因 |
| `.eval/challenge20-answer-eval{,-hybrid}/` | 答题级两臂 trace/裁判理由 |

Python 环境：`.venv-oracle/`（bm25s 0.3.11）。**磁盘紧张**（~5GB 余量）：bm25s-passage-idx 与 tantivy-passage 可再生，空间不够先删 bm25s 存档。

## 四、现行规格（只信这些）

| 读 | 路径 |
|---|---|
| 过程 | [AGENTS.md](AGENTS.md) |
| DAG / 里程碑 | [README.md](README.md) |
| 意图与评审决议 | [overview.md](overview.md) |
| 语料 / 指纹 | [corpus.md](corpus.md) |
| bge 窗公式 | [token-batch.md](token-batch.md) |
| 嵌入 / qwen profile / 超限守卫 | [embed.md](embed.md) |
| 检索身份 + **词法路/hybrid（M2 已落地）** | [retrieval.md](retrieval.md) |
| 评测口径 | [eval.md](eval.md) |
| 本轮 20 题对照 | [eval-m1-vector-ab.md](eval-m1-vector-ab.md) · 结果 [eval-m1-vector-ab-results.md](eval-m1-vector-ab-results.md) |
| 踩坑 | [log.md](log.md) |
| 树 / MCP / 沙箱 / 云 | [deferred.md](deferred.md) — **默认不要实现** |

已取代、仅供对照：`docs/plans/2026-09-13-vgi-rag-rust-zg-aligned-design.md`。

Windows 原型与 run-9 回执在 `C:/Users/xingc/Documents/Codex/repository-tree/`（WSL：`/mnt/c/Users/xingc/Documents/Codex/repository-tree/`）：`src/repository_tree/evaluation/{agent,judge}.py` 是 agent/裁判的原始实现，`.eval/hard/challenge20-tree-run-9/` 是冻结产物。

## 不要重跑、不要推翻

- run-9 四臂（grep/tree/tree-nolabel/tree-random）：冻结在 `fixtures/challenge20-prior-frozen.json`
- 文档级 BM25 校准（@100=0.111 @1000=0.275）：同一冻结文件
- bge-m3 全库向量（176,818 窗，`.vgi/zvec-docs/`）：禁止 `DROP`
- qwen-flash 全库向量（101,677 窗，`.vgi/zvec-docs-qwen-flash/`）：同样禁止重灌
- Batch 通道（`ForwardingTransportError` 未修复，工单在 `/home/chuan/ticket-batch-2026-09-16/`）：**别再用 Batch 灌库**
- 金标是 `evidence_ids`；融合身份永远是 `doc_id`

## 代码与数据落点

`/home/chuan/vgi-rs` crates：`vgi-core`、`vgi-ingest`、`vgi-search`（M2 新增 `lexical.rs`），bin `vgi`；`CARGO_BUILD_JOBS=2`。新增依赖：**tantivy 0.26.2**。数据目录 `/home/chuan/vgi-rs/.vgi/`：

| 东西 | 状态 |
|---|---|
| `corpus.sqlite` `documents` | 100,195 行 |
| `window_vectors` + `zvec-docs/` | bge-m3 完成（176,818 窗） |
| `window_vectors_qwen_flash` + `zvec-docs-qwen-flash/` | **完成**（101,677 窗，09-16 22:34） |
| `tantivy-passage/` | M2 词法索引（565MB） |
| `batch-qwen-flash*/` | 旧 Batch 证据件，不再使用 |
| `tokenizer.json` / `tokenizer-qwen3.json` | 已缓存；sha 在 `vgi-core/src/profile.rs` |

CLI 摘要（`--data-dir /home/chuan/vgi-rs/.vgi`）：

```bash
ingest --uri <shards> --revision b27b02bc3e45511b8b82a13e6f90ce761df726f6 --expected-n 100195
tokens --profile bge-m3|qwen-flash
embed --profile bge-m3|qwen-flash        # 均已完成
index --profile bge-m3|qwen-flash        # zvec 重建
index --lexical                          # tantivy 段落索引重建
index --centroids                        # 文档质心 zvec 索引（M4）
index --offsets                          # window_offsets char 偏移表（M4）
related --doc-id X --k 10                # 质心近邻（M4）
search --query '…' [--mode vector|lexical|hybrid] [--passage-k 20000] [--rerank 200] --profile … --env avrag-rs/.env
read --doc-id 5412 --offset 0 --limit 80
rg --pattern Arwa --doc-id 5412          # doc-id 必填
eval --questions docs/vgi/fixtures/challenge20-questions.json [--mode …] --profile … --env …
```

答题级评测：`ANSWER_EVAL_MODE=hybrid python3 scripts/challenge20-answer-eval.py`（断点续跑；oracle 脚本在 `.venv-oracle` 里跑）。

## 已知坑（不要再踩）

- **Batch 转发故障**未修复；同步嵌入是唯一可用通道（1M TPM 硬顶，429 自动退避）。
- **本地 Qwen3-8B tokenizer 不是 API 计数的上界**（同窗可达 1.17×）；嵌入路径的裁剪守卫已验证。
- 全表 `SELECT body` 进 `Vec` → RSS 4–5GB；按篇流式。
- `create_and_open` 不能对着已存在目录；fp16 查询必须走 packed bytes。
- 代理别让百炼走新加坡（`dashscope.aliyuncs.com` 已加入 `no_proxy`）。
- HuggingFace 在 WSL 常不通，tokenizer 用 curl。
- CLI 是 `--uri` / `--fixture`，不是 overnight 旧稿里的 `--corpus-uri`。
- 检索式以 `-` 开头会被 clap 当 flag——CLI/脚本一律 `--query=<文本>` 形式。
- `challenge20-answer-eval.py` 的 `arm` 标签已改动态拼接（mode/rerank/thinking）；旧产物里的写死标签仍不可信。
- **Python 不热加载**：跑着改 eval 脚本不会影响在跑进程——reasoning 落盘代码若在启动后才写入文件，该次运行不会产出 `reasoning-*.txt`（已踩）。

## 里程碑对照（避免按旧 PRD 铺 12 crate）

| 旧稿 | 现行 |
|---|---|
| 12 crate + vgi-store + Milvus | 3 crate + 1 bin；无云 |
| 自研倒排默认 | **tantivy 段落级**（自研 postings 仍未开，字段 boost 未做） |
| M1 向量 ≥ 官方 26.4/59.7 | 只报 Challenge-20 绝对 recall + 答题级 20 题 |
| M3 优于 run-9 根覆盖 | 不作为门 |
| 对齐 zg 四工具 + 托管 rg | 默认将来 `vgi_search`（已含 hybrid）+`vgi_read`；rg 必须带 `doc_ids` |
