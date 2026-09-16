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

## Agent 级 run-9（冻结，5 轮、循环里无排序检索）

| arm | 正确性均分 | evidence_recall | n | 来源 |
|---|---:|---:|---:|---|
| run-9 agent+grep（冻结） | 0.140 | 0.1277 | 20 | 冻结不重跑 |
| run-9 agent+tree（冻结） | 0.125 | 0.1014 | 20 | 冻结不重跑 |
| run-9 tree-nolabel（冻结） | 0.050 | 0.1013 | 20 | 冻结不重跑 |
| run-9 tree-random（冻结） | 0.050 | 0.0828 | 20 | 冻结不重跑 |
| vgi bge-m3 向量（09-16） | 0.250 | — | 20 | `challenge20-answer-eval.py`；PASS 486/435/684/380/20；52.3s/题；410k tok |
| vgi hybrid RRF(60)（09-17） | 0.200 | — | 20 | 同上但 `search --mode hybrid`；PASS 435/503/684/20；44.3s/题；420k tok |

Gold = `evidence_ids`. 本轮两臂无 BM25/RRF/tree/agent。

**答题级对照**：hybrid 20% vs 向量 25%——n=20 噪声内（一题 5pp）。索引级 hybrid 明确更强（@1000 0.5066 vs 0.4712）但未转化为答题分；瓶颈在 5 轮预算内的综合/验证，不在检索召回。注：results.json 内 `arm` 字段为脚本写死标签 "bge-m3 vector"，hybrid 臂产物实际为 `--mode hybrid`（目录 `.eval/challenge20-answer-eval-hybrid/`）。
