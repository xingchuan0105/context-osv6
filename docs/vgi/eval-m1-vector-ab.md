# M1 向量对照：bge-m3 vs qwen-flash（Challenge-20）

等 `qwen-flash` 索引建完后跑。只比两条**向量路**，不做 BM25 / RRF / rg / 树 / agent。

## 方案（锁死）

| 项 | 值 |
|---|---|
| 题集 | Challenge-20，20 题，`docs/vgi/fixtures/challenge20-questions.json` |
| 金标 | 每题 `evidence_ids`（文档级，约 9–13 篇/题），**不用** `gold_doc_ids` |
| 检索 | `search_vector`：整题一条 query 向量 → ANN `window_k=10000` 窗 → **max** 聚到 `doc_id` |
| 不做 | 词法、RRF、rg、语义树、代码模式、agent 循环 |
| 指标 | 20 题 **macro-average** recall@5 / @100 / @1000；`recall@k = \|hits[:k] ∩ evidence\| / \|evidence\|` |
| 官方 26.4%/59.7% | 可并排打印，**不是及格线**（全库 qrels + 另一 encoder） |

## 两臂

1. **bge-m3**：SiliconFlow `Pro/BAAI/bge-m3`，窗 7372 / overlap 256 / 1024-d fp16，表 `window_vectors`，索引 `zvec-docs`
2. **qwen-flash**：百炼 `qwen3.7-text-embedding-flash`，窗 128000 / overlap 256 / 256-d fp16，表 `window_vectors_qwen_flash`，索引 `zvec-docs-qwen-flash`

同一语料、同一 20 题、同一聚合与 k。输出：`.vgi/jobs/eval-bge-m3.json`、`eval-qwen-flash.json`、`eval-vector-ab.md`。

## 与上一轮合并（不重跑）

上一轮配置**冻结**，数字来自已有文档，不重跑 grep / tree / 文档级 BM25。冻结表：`fixtures/challenge20-prior-frozen.json`。

两张表分开，不要把 agent 正确性和索引 recall 混成一列：

1. **索引级 evidence recall@k**（同一 Challenge-20、`evidence_ids`）：冻结的文档级 BM25 + 本轮 bge-m3 / qwen-flash 向量。官方稠密 26.4%/59.7% 只旁注（全库 qrels，不是这 20 题）。
2. **Agent 级 run-9**（5 轮、无排序检索）：grep / tree 的正确性与 `evidence_recall`（工具是否读到证据），不能直接和 @100/@1000 比。

