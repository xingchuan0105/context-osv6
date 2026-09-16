# VGI-RAG 交接（M2 之前）

日期：2026-09-16 晚。给**接着做 M2 或继续评测**的 agent。先读本页，再动代码。上一版交接（09-14，Batch 灌库时代）已被本页取代；过程规则仍在 [AGENTS.md](AGENTS.md)，踩坑合并进 [log.md](log.md)。不要从 `docs/plans/2026-09-13-vgi-rag-rust-zg-aligned-design.md` 开工——那篇已 SUPERSEDED。

VGI 是 **BrowseComp-Plus 十万篇上的评测工具**。循环属于宿主（Codex/Claude），VGI 只给检索。代码在独立仓 **`/home/chuan/vgi-rs`**，规格在本目录 **`docs/vgi/`**；不要把 crate 加进 `avrag-rs` workspace，不要 `docker-compose`，不要 push。凭据读 `avrag-rs/.env`（`DASHSCOPE_API_KEY`、`EMBEDDING_*`），不要打印。

---

## 一、向量灌库进展（qwen-flash 第二套索引）

目标：为百炼 `qwen3.7-text-embedding-flash`（北京）建第二套索引（256-d fp16，`LIMIT=120000`/overlap 256），与 bge-m3（176,818 窗）并排。**总窗数 101,677**（= Batch 请求 100,011 + 更早一次同步尝试的 1,666 窗）。

| 时间 | 事件 |
|---|---|
| 09-14 23:44 → 09-15 03:55 | 用 **Batch File（半价）** 灌库：7 个 shard 全部 `completed`，但 **失败 72,076 / 100,011（72%）**，错误统一 `<5000405> InternalError.Algo.ForwardingTransportError: vLLM Error: `（详情为空） |
| 09-16 全天 | 排查 Batch 故障（见下「已知坑」）；报工单；18:18 原样复测**仍 0/391、0/20**，未修复 |
| 09-16 13:17 起 | 改**同步**补跑：`vgi embed --profile qwen-flash`，pending 72,076 窗 / **576,645,028 token**；速率稳定 **≈135 窗/分钟**；预计 **22:15 前后**完成并自动重建 `zvec-docs-qwen-flash` |

写本文时（21:05）表内 **92,000 / 101,677** 行；429 退避 632 次（1M TPM 边缘，每次只拖慢单路 2-4 s）；**守卫裁剪触发 12/14**（14 条 120k 本地窗被 API 数成 131,221–140,166 token；自动裁剪后实测 126,959 / 126,316 < 131,072，不再中断跑批）。

**跑完后的收尾清单**（按顺序）：

1. `sqlite3 .vgi/corpus.sqlite 'select count(*) from window_vectors_qwen_flash;'` 应为 **101,677**；日志 `/home/chuan/vgi-rs/.vgi/jobs/sync-embed-qwen-flash.log` 末尾应出现 `zvec-docs: inserted … `（embed 结束会自动重建索引）。
2. `vgi eval --questions docs/vgi/fixtures/challenge20-questions.json --profile qwen-flash --env avrag-rs/.env` → 更新 [eval-m1-vector-ab-results.md](eval-m1-vector-ab-results.md) 的 qwen 行（此前只有 29% 窗时的不可比数字）。
3. 若给 answer-level 也跑 qwen 臂：同一脚本、同一预算、同一裁判（见下节），加进同一张表。

**不要重跑**：bge-m3 全库向量（176,818 窗已完成）；Batch 通道（当前不可用，重试只会再花半天）。

---

## 二、本次测试：Challenge-20 答题级（bge-m3 臂）

目的：不测 recall，测**20 题能不能答上来 + 耗时 + token 消耗**。

**协议**（与 run-9 冻结臂对齐，便于直接对比）

- Agent：`qwen3.8-flash`，temperature 0、`enable_thinking=false`；`max_rounds=5`、`max_tool_calls=32`、600 s/题；工具输出裁剪 8000 字符
- 工具（M1 检索面，**纯向量，无 BM25/RRF**）：`vgi search`（bge-m3 → zvec → max_pool → doc_id，top-k 只给 id 排名）、`vgi read`（字符窗）、`vgi rg`（文档内正则，doc_id 必填）
- 裁判：native **Eval v2** prompt（从 run-9 产物逐字复用，存于 `docs/vgi/fixtures/judge-v2-reference/`）+ `qwen3.8-flash` temp 0、thinking off
- 脚本：**`vgi-rs/scripts/challenge20-answer-eval.py`**（题目/裁判从 fixtures 读，输出默认 `vgi-rs/.eval/challenge20-answer-eval/`；支持断点续跑）

**结果（20 题，2026-09-16）**

| 指标 | 本次 bge-m3 臂 | run-9 冻结对照（同模型/预算/裁判） |
|---|---:|---|
| PASS | **5/20 = 25%** | grep 14% · tree 12.5% · tree-nolabel 5% · tree-random 5% |
| answer_correctness 均分 | 0.25（INCORRECT 13、REFUSAL_WRONG 2） | — |
| 平均耗时/题 | **52.3 s**（中位 47.4 s；20 题合计 845 s） | ~270 s |
| token（agent） | **410,271**（prompt 349,432 + completion 60,839） | ~31.8k/题/臂（整批 2.54M/80 题） |
| token（裁判） | ≈5.6k/题 → 20 题 ≈ **110k** | 未单列 |
| 平均工具调用/题 | 8.5（上限 32） | 6.35–6.75 |

PASS 题：`bcp:486`、`bcp:435`、`bcp:684`、`bcp:380`、`bcp:20`。

**为什么是 25%**（配套诊断，`fixtures/challenge20-gold-rank-rawquestion.json`）：用**题目原文**直接检索，只有 **7/20** 题的金标文档进 top-10（13/20 进 top-100，其余排名 116–1521 或缺失）。5 个 PASS 里 4 个金标就在 top-10（名次 1/2/5/9）；第 5 个（`bcp:486`，金标 rank 50）是 agent 改写检索式后捞回来的。失败多数是金标排太深 + 5 轮预算内改写不中。

**口径注意**：① 工具面与 run-9 不同（排序检索替代 grep/树导航），这正是"加排序检索"的对照变量；② 裁判与 agent 同族（run-9 亦然），同族裁判局限照记；③ 本臂**未含词法通道**。

**产物**：`fixtures/challenge20-answer-eval-2026-09-16-bge-m3.{summary,results}.json`（results 含每题 trace/答案/裁判理由）；题目与金标答案变体：`fixtures/challenge20-questions-judge.json`。

---

## 三、M2 前状态

**已就绪**

- `rrf_merge`（vgi-core，有实现 + `test_rrf_four_docs` 锚点；`retrieval.md` 规定 **M1 不调用**）
- `search_vector`：zvec cosine `get_score` 是**距离** → 取负 → `max_pool` 到 `doc_id`，`window_k=10000`
- 两层评测面：索引级 recall（`vgi eval`）、答题级 20 题（上节脚本）
- 冻结对照：文档级 BM25 校准（bm25s lucene k1=1.2 b=0.75，@100=0.111 @1000=0.275）——**只是数字**，代码里没有引擎

**未做**

- 没有任何 BM25 通道：无 tantivy、无 zvec FTS、无自研 postings（`grep -rn bm25 vgi-rs` 为空）
- 官方预构建 BM25 **oracle 未下载、未跑**（`deferred.md` 要求先 oracle 再选引擎）
- hybrid 工具面（vector+bm25 → RRF）未给 agent

**M2 门（README）**：先跑官方预构建 BM25 oracle；hybrid ≥ 本系统单路最优。

**最小可落地路径**：① oracle（同 20 题）；② `rrf_merge(vector, bm25, k=60)` 做成 hybrid 检索（可作为 `vgi_search` 的第二模式）；③ 同一 20 题 / 同预算 / 同裁判重跑 answer-level，与 25% 对照。按旧规矩，**开 M2 要用户点名**。

---

## 现行规格（只信这些）

| 读 | 路径 |
|---|---|
| 过程 | [AGENTS.md](AGENTS.md) |
| DAG / 里程碑 | [README.md](README.md) |
| 意图与评审决议 | [overview.md](overview.md) |
| 语料 / 指纹 | [corpus.md](corpus.md) |
| bge 窗公式 | [token-batch.md](token-batch.md) |
| 嵌入 / qwen profile / 超限守卫 | [embed.md](embed.md) |
| 检索身份（RRF 锁死待 M2） | [retrieval.md](retrieval.md) |
| 评测口径 | [eval.md](eval.md) |
| 本轮 20 题对照（含冻结上一轮） | [eval-m1-vector-ab.md](eval-m1-vector-ab.md) · 结果 [eval-m1-vector-ab-results.md](eval-m1-vector-ab-results.md) |
| 踩坑 | [log.md](log.md) |
| 树 / BM25 引擎 / MCP / 沙箱 | [deferred.md](deferred.md) — **默认不要实现** |

已取代、仅供对照：`docs/plans/2026-09-13-vgi-rag-rust-zg-aligned-design.md`。

Windows 原型与 run-9 回执在 `C:/Users/xingc/Documents/Codex/repository-tree/`（WSL：`/mnt/c/Users/xingc/Documents/Codex/repository-tree/`）：`src/repository_tree/evaluation/{agent,judge}.py` 是 agent/裁判的原始实现，`.eval/hard/challenge20-tree-run-9/` 是冻结产物（profile/questions/usage/native-judge）。

## 不要重跑、不要推翻

- run-9 四臂（grep/tree/tree-nolabel/tree-random）：冻结在 `fixtures/challenge20-prior-frozen.json`
- 文档级 BM25 校准（@100=0.111 @1000=0.275）：同一冻结文件，不要为对照再扫 10 万篇
- bge-m3 全库向量（176,818 窗，`.vgi/zvec-docs/`）：禁止 `DROP`
- 免费测试模型 `batch-test-model` 的结果（10/10 管道正常）不必重跑
- 官方稠密 26.4%@100 / 59.7%@1000：**不是及格线**（另一 encoder × 全库 qrels）
- 金标是 `evidence_ids`（不是更窄的 `gold_doc_ids`）；融合身份永远是 `doc_id`

## 代码与数据落点

`/home/chuan/vgi-rs` crates：`vgi-core`、`vgi-ingest`、`vgi-search`，bin `vgi`；`CARGO_BUILD_JOBS=2`。数据目录 `/home/chuan/vgi-rs/.vgi/`：

| 东西 | 状态（21:05） |
|---|---|
| `corpus.sqlite` `documents` | 100,195 行 |
| `window_vectors` + `zvec-docs/` | bge-m3 完成（176,818 窗） |
| `window_vectors_qwen_flash` | 92,000 / 101,677（同步补跑中） |
| `zvec-docs-qwen-flash/` | 仍是 09-15 旧版；跑完自动重建 |
| `batch-qwen-flash/` | 旧 Batch 输入/结果，保留作证据；**不再使用** |
| `batch-qwen-flash-probe/` | 391 条复现批次（含 error 文件） |
| `jobs/sync-embed-qwen-flash.log` | 本次同步补跑日志 |
| `tokenizer.json` / `tokenizer-qwen3.json` | 已缓存；sha 在 `vgi-core/src/profile.rs` |

CLI 摘要（`--data-dir /home/chuan/vgi-rs/.vgi`）：

```bash
ingest --uri <shards> --revision b27b02bc3e45511b8b82a13e6f90ce761df726f6 --expected-n 100195
tokens --profile bge-m3|qwen-flash
embed --profile bge-m3|qwen-flash        # 同步；bge 已完成；qwen 正在补跑
index --profile bge-m3|qwen-flash
search --query '…' --profile bge-m3|qwen-flash --env avrag-rs/.env
read --doc-id 5412 --offset 0 --limit 80
rg --pattern Arwa --doc-id 5412          # doc-id 必填
eval --questions docs/vgi/fixtures/challenge20-questions.json --profile bge-m3|qwen-flash --env …
```

答题级评测：`python3 /home/chuan/vgi-rs/scripts/challenge20-answer-eval.py [qid …]`（断点续跑）。

## 已知坑（不要再踩）

- **Batch 转发故障**：`ForwardingTransportError`（详情为空）与输入大小无关；同一请求可确定性复现，同步 100% 可用。Batch 崩溃现场日志显示 batch 走 `/internal/api/v1/translate/input`（载荷 ~2× 膨胀）+ `...-batch-prod-bj-beta`。工单进行中，复现件在 `/home/chuan/ticket-batch-2026-09-16/`。**别再用 Batch 灌库**，除非它被官方确认修复。
- **本地 Qwen3-8B tokenizer 不是 API 计数的上界**：同窗可达 1.17×；120k 本地窗有 14 条 API 超 131,072。嵌入路径已加守卫（多文本拆单条 + 按上报比例裁剪到 97%），`embed.md` 有语义、`anchors/test_anchors.py::test_trim_keep_chars` 有手算锚点。
- 全表 `SELECT body` 进 `Vec` → RSS 4–5GB；按篇流式。
- `create_and_open` 不能对着已存在目录；fp16 查询必须走 packed bytes。
- 同步 1M TPM 是硬顶（429 退避自动处理）；代理别让百炼走新加坡（`dashscope.aliyuncs.com` 已加入 `no_proxy`，见 `docs/agent/wsl-services.md`）。
- HuggingFace 在 WSL 常不通，tokenizer 用 curl。
- CLI 是 `--uri` / `--fixture`，不是 overnight 旧稿里的 `--corpus-uri`。

## 里程碑对照（避免按旧 PRD 铺 12 crate）

| 旧稿 | 现行 |
|---|---|
| 12 crate + vgi-store + Milvus | 3 crate + 1 bin；无云 |
| 自研倒排默认 | 冻结 BM25 数字；引擎待 M2 先 oracle |
| M1 向量 ≥ 官方 26.4/59.7 | 只报 Challenge-20 绝对 recall + 答题级 20 题 |
| M3 优于 run-9 根覆盖 | 不作为门 |
| 对齐 zg 四工具 + 托管 rg | 默认将来 `vgi_search`+`vgi_read`；rg 必须带 `doc_ids` |

| 里程碑 | 状态 |
|---|---|
| M0 | **完成**（ingest + token + zvec spike 四门绿） |
| M1 | 向量路代码完成；bge 索引完成；qwen 索引今晚补齐；索引级 recall 已报；**答题级基线 25%（本次）** |
| M2 | **未开**（无 BM25 引擎、无 oracle、无 hybrid 工具面） |
| M3 / M4 | 未开 |
