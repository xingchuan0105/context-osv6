# Challenge-20：上一轮冻结 + 本轮向量

日期：2026-09-15。上一轮配置不重跑。Agent 正确性 ≠ 索引 recall@k。

**qwen-flash 索引已补全（2026-09-16 22:34）。** Batch 通道故障导致首轮只写入 29,601/101,677 窗（`InternalError.Algo.ForwardingTransportError`，探针与工单见 [log](log.md)）；09-16 13:17 起改走**同步**补跑 72,076 窗 / 576,645,028 token，22:34 完成 → `window_vectors_qwen_flash` = **101,677 行**、`zvec-docs-qwen-flash` 已重建；途中 16 条超长窗由裁剪守卫自动处理。下表两臂均为完整索引。

## 索引级 evidence recall（Challenge-20 `evidence_ids`）

| arm | @5 | @100 | @1000 | n | 来源 | 协议 |
|---|---:|---:|---:|---:|---|---|
| 文档级 BM25（冻结） | — | 0.1110 | 0.2750 | 20 | 冻结不重跑 | bm25s lucene k1=1.2 b=0.75; whole document; whole question as bag; no agent. @10 headline 2.4% (per-question mean 2.7%). @100 11.1% (9/20 had a hit); @1000 27.5% (14/20). |
| bge-m3 向量 | 0.0528 | 0.2110 | 0.4712 | 20 | 本轮 | `bge-m3-7372-overlap256-max` |
| qwen-flash 向量 | 0.0396 | 0.2167 | 0.4498 | 20 | 09-16 补全索引后重跑 | `qwen-flash-128k-overlap256-dim256-max` |

官方稠密 Qwen3-0.6B 全库 qrels：@100=0.264 @1000=0.597（**不是这 20 题，不是及格线**）。

## M2 词法路 + hybrid（2026-09-16 深夜）

Oracle 用 bm25s lucene k1=1.2 b=0.75 作官方 Lucene 索引的轻量替身（不下 2.17GB 官方包、不装 Java）：文档级复测 @100=0.141 @1000=0.300（≈冻结值，略高因分词细节）；**段落级**（W=512/重叠 64 raw-token 窗，max-pool 到 doc_id）@100=0.181 @1000=0.383——段落粒度明确优于文档级，与 PRD 判断一致。引擎实现为 **tantivy**（索引只存 doc_id、passage 文本 indexed-not-stored WithFreqs；索引 565MB / 1,147,822 passages / 构建 82s），`vgi search|eval --mode lexical|hybrid`。

| arm | @5 | @100 | @1000 | n | 协议 |
|---|---:|---:|---:|---|---|
| bm25s 段落级 oracle | 0.0534 | 0.1808 | 0.3829 | 20 | Python 标定；停用词 english、无 1-char token |
| tantivy 段落级 BM25 | 0.0567 | 0.2169 | 0.4798 | 20 | `vgi --mode lexical`；不去停用词、含 1-char token |
| bge-m3 向量（对照） | 0.0528 | 0.2110 | 0.4712 | 20 | 同上 |
| **hybrid RRF(60)** | 0.0523 | **0.2358** | **0.5066** | 20 | `rrf_merge([vector, lexical], k=60)` |

**M2 索引级门：过。** hybrid @100 / @1000 均高于任一单路（@100 +12% 相对 vs vector，@1000 0.5066 > 0.480）；@5 持平。tantivy 词法单路已逼近向量路（@1000 0.480 vs 0.471）。agent 级对照见 HANDOFF 更新。

## rerank 阶段（2026-09-17 凌晨，Bailian gte-rerank-v2）

`--rerank 200`：对 RankedList 前 200 个 doc 送 `body[:1500]` 摘要给 Bailian `gte-rerank-v2`，按 relevance_score 重排（只动 top-200 内位置；@1000 恒不变）。oracle 与 Rust 实现逐位一致（`rerank-oracle-search-lists-challenge20.json` = `eval-bge-m3-hybrid-rr200.json`）。

| arm | @5 | @10 | @100 | @1000 |
|---|---:|---:|---:|---:|
| vector | 0.0528 | — | 0.2110 | 0.4712 |
| vector + rr200 | 0.0461 | 0.0868 | 0.2468 | 0.4712 |
| lexical | 0.0567 | — | 0.2169 | 0.4798 |
| lexical + rr200 | 0.0494 | 0.0751 | 0.2382 | 0.4798 |
| hybrid | 0.0523 | — | 0.2358 | 0.5066 |
| **hybrid + rr200** | **0.0550** | **0.0901** | **0.2683** | **0.5066** |

读法：rerank 把 100–200 区间的证据拉进 top-100（hybrid @100 +13.6% 相对，超过官方稠密 @100=0.264 的量级），但 **@5 基本不动**（0.052→0.055）——被 rerank 挤出的头名本来就是错的，进的也只是"可识别"不是"可答"。

## M3 语义树 oracle（2026-09-17，门未过）

2 级球面 kmeans（bge-m3 doc 质心，K1=64、~80 docs/叶 → 1,216 叶）。route recall = 查询向量沿 top-b L1 × top-c L2 质心下钻后证据是否落在叶集合内：

| beam | 暴露文档数 | route recall | flat vector 对照 |
|---|---:|---:|---|
| b1c1 | ~106 | 0.055 | @100=0.211 |
| b4c1 | ~422 | 0.217 | — |
| b8c2 | ~1,713 | 0.452 | @1000=0.471 |

**逐档皆劣于平铺**：~200 文档暴露只换回 0.121（flat @200 约 0.28）；~1,700 暴露才追平 flat @1000。证据文档不在主题中心，聚类边界把它们切散了。另：`heading` 字段全空，L1 自动打标需要 body 高频词（已无必要）。**M3 不做。**

## Agent 级 run-9（冻结，5 轮、循环里无排序检索）

| arm | 正确性均分 | evidence_recall | n | 来源 |
|---|---:|---:|---:|---|
| run-9 agent+grep（冻结） | 0.140 | 0.1277 | 20 | 冻结不重跑 |
| run-9 agent+tree（冻结） | 0.125 | 0.1014 | 20 | 冻结不重跑 |
| run-9 tree-nolabel（冻结） | 0.050 | 0.1013 | 20 | 冻结不重跑 |
| run-9 tree-random（冻结） | 0.050 | 0.0828 | 20 | 冻结不重跑 |
| vgi bge-m3 向量（09-16） | 0.250 | — | 20 | `challenge20-answer-eval.py`；PASS 486/435/684/380/20；52.3s/题；410k tok |
| vgi hybrid RRF(60)（09-17） | 0.200 | — | 20 | 同上但 `search --mode hybrid`；PASS 435/503/684/20；44.3s/题；420k tok |
| vgi hybrid+rr200（09-17） | 0.175 | — | 20 | 同上但 `--mode hybrid --rerank 200`；PASS 435/503/684 + PARTIAL 20；59.1s/题；434k tok |
| vgi hybrid+rr200 **放宽预算**（09-17） | **0.350** | — | 20 | 20 轮/64 调用/1800s；PASS 435/411/503/89/684/380/20；REFUSAL_WRONG 清零；98.9s/题；3.87M tok |
| vgi vector **放宽预算**（09-17） | **0.350** | — | 20 | 同预算纯向量；PASS 486/435/411/503/684/422/380；76.2s/题；4.19M tok |

Gold = `evidence_ids`. 本轮两臂无 BM25/RRF/tree/agent。

**答题级对照**：三臂 25% → 20% → 17.5%（vector → hybrid → hybrid+rr200 @5轮），但**放宽预算后同臂 hybrid+rr200 达 35%**（7/20，REFUSAL_WRONG 清零，token×9）——瓶颈确认为 agent 预算而非检索；预算放开后检索增益兑现（411/89 为三臂首答对）。**vector@20轮对照同为 35%**：两臂放宽预算后收敛同分——预算 +10pp 是唯一大杠杆，检索臂间差异在答题级不转化；但两臂通过题集互补（vector 独中 486/422，hybrid+rr 独中 89/20，并集 9/20=45%），臂间 ensemble 是答题级剩余空间。注：results.json 内 `arm` 字段为脚本写死标签 "bge-m3 vector"，以产物目录区分为准（`.eval/challenge20-answer-eval{,-hybrid,-hybrid-rr200,-rr200-r20,-vector-r20}/`）。预算环境变量：`ANSWER_EVAL_{MAX_ROUNDS,MAX_TOOLS,DEADLINE_S}`。

## M4：命中定位 + 关联图 + thinking（进行中）

**索引面新增**（spec：[retrieval.md](retrieval.md)，实现：vgi-rs `f2d4228`）：

- `search` 输出 `[{doc_id, offset, snippet}]`——命中位置直达 `read`；词法 passage 存 char `off` 字段，向量窗经 `window_offsets` 表（176,818 行，`vgi index --offsets` 全库重分词一遍填充）
- `vgi related --doc-id X --k N`：文档质心（`zvec-centroids`，100,195 docs，`vgi index --centroids` ~40s）ANN 近邻；**平铺关联图**
- 质心 oracle：`scripts/doclevel_oracle.py` 实测文档级向量 @100=0.211 与窗口级**打平**（@5 0.037 vs 0.053，@1000 0.447 vs 0.471）
- 关联边 oracle：`scripts/graph_oracle.py` 实测 top-200 命中 → top-30 邻居覆盖 44/207 证据（1164: 1→8、422: 2→8）；散点题无效
- 逐题导航归因：`scripts/failure_attribution.py` 分 NEVER_RETRIEVED / RETRIEVED_NOT_READ / READ_NOT_SOLVED 三桶
- 词法池化重构 sanity：@5/@100/@1000 = 0.0567/0.2169/0.4798 与重构前逐位一致

**thinking 答题臂部分结果**（hybrid+rr200、r20 预算、新工具面，4/20 题后暂停，`challenge20-answer-eval-hits-thinking/`）：

| qid | label | 诊断 |
|---|---|---|
| bcp:22 | **PASS**（3min/10调） | 正例：17K字符参数化假设穷举（~15个候选运动交叉排除）→ 精准验证 |
| bcp:342 | INCORRECT→重采 **PASS**（19min/22调） | 首轮 20 search/0 read/TimeoutError；重采链里 read+rg 交叉验证后答对——thinking 方差大 |
| bcp:618 | INCORRECT（33min/24调） | deadline 空答案；推理链涨到 51K 字符，22 次 search 只 2 次 read |
| bcp:70 | INCORRECT（22min/32调） | **ev_read=5**（命中定位生效，证据真被读到了），综合错实体 |

**thinking 病理**（正反例抽取自 `.eval/chains-samples/reasoning-*.txt`）：正例=先推理穷举假设空间再碰工具、命中即读、rg 字面验证约束；反例=纯 search 改写循环不 read、deadline 白卷、`related` 0 调用。已把正反例转写成认识论陈述对齐 zvec-rg 描述性风格重写 system prompt（预算移出 prompt，由 closeout 信号负责）；冒烟 bcp:435 PASS 78s/5调。
