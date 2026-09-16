# Challenge-20：上一轮冻结 + 本轮向量

日期：2026-09-15。上一轮配置不重跑。Agent 正确性 ≠ 索引 recall@k。

**qwen-flash 索引不完整，不能当公平对照。** Batch 提交 100,011 窗，成功写入 **29,601**（约 29%）；7 个 shard 全部 `completed` 但 error jsonl 合计约 7.2 万行，多为 `InternalError.Algo.ForwardingTransportError`（百炼 vLLM 转发失败）。bge-m3 是全库 176,818 窗。2026-09-16：Batch 复测仍不可用（探针见 [log](log.md)），72,076 条失败窗改走**同步**补齐（`vgi embed --profile qwen-flash`，进行中，约 10-13 h）；下表 qwen 行待补齐后重算。

## 索引级 evidence recall（Challenge-20 `evidence_ids`）

| arm | @5 | @100 | @1000 | n | 来源 | 协议 |
|---|---:|---:|---:|---:|---|---|
| 文档级 BM25（冻结） | — | 0.1110 | 0.2750 | 20 | 冻结不重跑 | bm25s lucene k1=1.2 b=0.75; whole document; whole question as bag; no agent. @10 headline 2.4% (per-question mean 2.7%). @100 11.1% (9/20 had a hit); @1000 27.5% (14/20). |
| bge-m3 向量 | 0.0528 | 0.2110 | 0.4712 | 20 | 本轮 | `bge-m3-7372-overlap256-max` |
| qwen-flash 向量 | 0.0211 | 0.0653 | 0.1233 | 20 | 本轮 | `qwen-flash-128k-overlap256-dim256-max` |

官方稠密 Qwen3-0.6B 全库 qrels：@100=0.264 @1000=0.597（**不是这 20 题，不是及格线**）。

## Agent 级 run-9（冻结，5 轮、循环里无排序检索）

| arm | 正确性均分 | evidence_recall | n | 来源 |
|---|---:|---:|---:|---|
| run-9 agent+grep（冻结） | 0.140 | 0.1277 | 20 | 冻结不重跑 |
| run-9 agent+tree（冻结） | 0.125 | 0.1014 | 20 | 冻结不重跑 |
| run-9 tree-nolabel（冻结） | 0.050 | 0.1013 | 20 | 冻结不重跑 |
| run-9 tree-random（冻结） | 0.050 | 0.0828 | 20 | 冻结不重跑 |

Gold = `evidence_ids`. 本轮两臂无 BM25/RRF/tree/agent。
