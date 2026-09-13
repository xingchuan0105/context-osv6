# Challenge-20 文档语义树构建评审

日期：2026-09-13。评审对象：独立工程 `repository-tree` 的文档语义树（构建代码、已构建产物与构建回执），承接 [run-9 正式分析](2026-09-13-vgi-rag-challenge20-tree-run9-analysis.md)。

**结论：该树在"形状与可达性、标签信息量、特征区分度、对照一致性"四个方面都有实质缺陷。run-9 的四臂结果不能解释为对"文档语义树"机制的评价——它评价的是一棵在此语料上既无法导航、也没有可判读线索的树。**

## 1. 它是怎么建的（事实）

- 输入：100,195 篇文档，每篇**只取前 8,192 字节**（`tree_index.clip_text` / `build_features`）；bm25s 英文分词（Porter 词干；停用词表不完整，实测放行 `his`/`your`）。
- 特征：`hashed_tf`（semantic_tree.py:23）——每个词项按 `1+log(tf)` 加权、CRC32 取模 **256 维**累加、L2 归一。**没有 IDF**；256 维对 ~10 万文档意味着严重哈希冲突。算法与指纹却命名为 `hashed-tfidf-bisect-v1`（名实不符）。
- 建树：自顶向下，取节点成员向量均值为方向做 2-means 二分（`_bisect`，种子为"最反均值点"与"距其最远点"，最多 20 轮），反复二分最大分组直到 8 组、或最大组 ≤24 篇、或不可分（`_split`）；`len>24 且 depth<8` 才继续分裂。
- 节点载荷：`terms` = 成员文档各自 top-8 原始词频词的众数 top-5（`document_terms`/`node_terms`，无 IDF/停用词/长度过滤）；`representative` = **`min(members)`** 的 {document_id, title, url}。
- 随机对照：`_random_split` 用 seed 20 把成员随机切成 8 份（每层都切满 8 份）。

## 2. 实测结构：名义参数与实际不符

| 指标 | 设计意图 | 实测 |
|---|---|---|
| 分枝 | 8 路 | 均值 **5.02**；2,256 个内部节点中 2 路 486、3 路 366、…、8 路仅 722 |
| 叶大小 | 约 24 篇 | p50=**10**、p90=22、max=39；≤4 篇的叶 2,288（25%），≤10 篇 4,620（51%） |
| 深度 | ≤8 | 叶分布在 2–8；深度 5–6 占 87% |

各深度节点规模（导航成本）：

| 深度 | 节点数 | 平均规模 | 叶数 |
|---|---|---|---|
| 0 | 1 | 100,195 | — |
| 1 | 8 | 12,524 | — |
| 2 | 64 | 1,566 | 14 |
| 3 | 394 | 254 | 128 |
| 4 | 1,890 | 52 | 1,000 |
| 5 | 5,414 | 16 | 4,395 |
| 6 | 3,488 | 12 | 3,468 |
| 7–8 | 66 | ~9 | 64 |

**可达性推算**：`tree_overview` 每层只能看到直接子节点卡片、必须逐层下行。要到达典型叶子需 5–6 次调用，而整轮预算是 5 个工具轮（实测 ~6.3 次调用/题）——**即使不搜索、不阅读、不回答，预算也不够走到叶子**。深度 ≤4 的叶只占 13%（1,142 个），其中深度 4 节点平均 52 篇，深度 3 节点平均 254 篇——即使到达，也远不够收窄证据。

## 3. 语义质量：标签与聚类都接近随机

- **标签**：停用词/数字/短碎片占比——深度 0：**100%**（根标签 `['s','i','his','he','you']`）、深度 1：70%、深度 2：52%，深层 39–51%；兄弟节点 **46% 的 top-1 标签相同**（无法区分兄弟）；常见碎片 `titl`/`placehold`/`universiti`。
- **代表文档**：取成员最小 ID；且该语料的文档 `name` 就是数字 ID（如 `0`、`100003`）——所以代表标题永远等于 ID，导航线索为零。
- **聚类凝聚度**（审计脚本抽样）：叶内文档余弦 **0.659** vs 随机对 **0.573**；深度 3 节点内部 **0.577–0.626** ≈ 随机基线；每个文档 top-10 最相似邻居中只有 **0.26/10** 在自己叶内。区分度存在但很弱，越往上层越接近随机分组。
- **与题集的匹配**：每题证据跨 depth-1 分支的中位数是 **4/8**（范围 1–5）；**19/20 题证据集合的最小覆盖节点就是树根**。构建回执（`evidence/challenge20-semantic-tree.json`）在建成当天 00:14 就已记录 `mean_evidence_cover_size = 96,400`（≈96% 语料）——这是建树阶段就存在的红灯，四臂仍然照跑（流程问题，非代码缺陷）。

## 4. 随机对照的有效性

- `tree-random` 实测：**17,333 叶、平均严格 8.00 路、叶中位 3 篇、深度仅 4–5**；内容树：9,069 叶、5.02 路、叶中位 10 篇、深度 2–8。所谓"相同 fanout / leaf_size / max_depth"只对参数成立：随机臂同时改变了**分枝规则性**与**成员分配**，是混杂对照，`tree-random` 与 `tree`/`tree-nolabel` 的差异不能归因于"分枝是否来自内容"。
- 两次指纹与 run profile 一致（`f937476c…`、`153f9b36…`），产物加载本身没有错。
- 元数据错标：随机树的 `algorithm` 字段被 `save_tree` 硬编码为 `hashed-tfidf-bisect-v1`；当前版本控制中的建树脚本只构建内容树，随机树的构建入口不在脚本里（可复现性缺口）。

## 5. 代码级发现

| 位置 | 发现 |
|---|---|
| `semantic_tree.py:23-30` | `hashed_tf`：无 IDF；256 维 CRC 哈希冲突；命名/指纹称 TF-IDF 但实为 hashed TF |
| `semantic_tree.py:33-39` | `document_terms`/`node_terms` 用原始词频众数，无 IDF/停用词/长度过滤 → 标签噪声 |
| `semantic_tree.py:150-153` | `_representative = min(members)`，任意且无语义 |
| `semantic_tree.py:77-88` | `_split` 早停（≤leaf_size/不可分）→ 多数节点不足 8 路、叶规模偏小 |
| `semantic_tree.py:127-148` | 深度/叶判据在 max_depth 截断时产生 39 篇的超大叶 |
| `semantic_tree.py:164-178` | `view` 只暴露静态卡片：无查询路由、无质心距离、无节点摘要，导航全凭标签 |
| `tree_index.py:14-17, 38-50` | 特征只用前 8 KB 文本 |
| `tree_index.py:53-79` | `save_tree`/`meta.json` 硬编码 algorithm 字段 |

## 6. 建议（按优先级）

1. **先修标签与代表**：IDF/停用词/词长过滤；代表改为最近质心文档（并接受语料无标题这一事实，或在卡片上给出片段摘要）。
2. **让形状成立**：以容量目标（如叶 20–40 篇、深度 ≤4）做构建验收，把分布检查写进建树脚本；不足 8 路的节点在实验中显式报告。
3. **修对照**：随机树改为"固定内容树的分枝形状，仅随机打乱成员"，消除规则性混杂。
4. **若要测导航价值**：增加 query→node 路由（质心/标签打分）或节点摘要，并把"每层一次调用"的导航成本纳入预算设计（预算 ≥ 深度 + 搜索 + 阅读 + 回答）；否则先用**节点级检索指标**（证据能否定位到 top-k 节点）证明结构可分，再做端到端。
5. **题集**：选证据在层级上可收窄的题集；对 BrowseComp-Plus 这类跨主题证据，明确声明"层次结构不可收窄"，不做端到端导航对照。
6. **复现**：随机树构建进入版本控制脚本；meta 记录 `split`/`rng_seed`。

现成方案与文献依据的系统调研（bisecting k-means 文献、Scatter/Gather、RAPTOR、BERTopic、GraphRAG、ANN 树等）见 [检索最佳实践调研](../research/2026-09-13-semantic-tree-building-best-practices.md)。

## 7. 复算入口

```powershell
# 结构、标签、凝聚度、对照形状、题目覆盖（无模型调用）
./.venv/Scripts/python.exe scripts/analysis/audit_semantic_tree.py `
  .eval/hard/challenge20-index-v1/semantic-tree `
  .eval/hard/challenge20-index-v1/semantic-tree-random `
  .eval/hard/challenge20-tree-run-9/questions.json
```

- 代码：`src/repository_tree/retrieval/semantic_tree.py`、`src/repository_tree/evaluation/hard/tree_index.py`、`scripts/build-challenge20-tree.py`
- 产物：`.eval/hard/challenge20-index-v1/semantic-tree{,-random}/`；构建回执 `evidence/challenge20-semantic-tree.json`
- 凝聚度与邻居纯度指标为固定种子抽样估计（seed 20260913），脚本可复算
