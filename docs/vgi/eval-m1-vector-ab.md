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
