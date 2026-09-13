# VGI-RAG Challenge-20 树消融 run-9 正式分析

日期：2026-09-13。承接 [Challenge-20 题集交接](2026-09-12-vgi-rag-challenge20.md)。独立工程四臂运行 run-9 于 2026-09-13 16:18–17:06 完成：80/80 试验正常收束、零基础设施错误；随后完成原生 Eval v2 盲评 80/80、零判分错误。**结论：相对 `agent+grep` 主对照，文档语义树导航没有可测质量增益；本题集证据跨主题（19/20 题的最小覆盖节点就是树根），树结构不构成可用收窄面；四臂都逼近地板。**

## 口径

- 四臂 `tree`（语义树＋词标签）、`tree-nolabel`、`tree-random`、`grep`（主对照：agent+grep）× 20 题 × 1 次；全部在完整 100,194 文档上运行；无 BM25、无 embedding、无 SAC；`qwen3.8-flash`、`max_rounds=5`、`max_tool_calls=32`、每组并发 2、总并发 8。
- 全部 80 个试验都是 5 轮上限下的 `budget_closeout`；答案级超时 0（grep 级 `tool_timeout` 记工具错误：tree 36、nolabel 33、random 44、grep 28）。
- 判分：原生 Eval v2 rubric 不改写；盲评隐藏组名/耗时/工具数并固定随机顺序；上下文只取该试验真实返回的块，逐块取观察到的最长文本，并渲染 Agent 可见的标题/日期、locator 出处与 `reference_id`；同模型族裁判（qwen3.8-flash），非独立复评。判分用量 81 请求 / 625,134 reported tokens。

## 主结果（n=20/臂）

| 组 | 正确性均分 | correct/partial/incorrect | 忠实度 | 上下文充分度 | evidence_recall | gold any/complete | 引用题数 | 工具/题 | known tokens |
|---|---:|---|---:|---:|---:|---|---:|---:|---:|
| `tree` | 0.125 | 2 / 1 / 17 | 0.70 | 0.14 | 0.1014 | 2 / 1 | 0 | 6.35 | 550,017 |
| `tree-nolabel` | 0.050 | 1 / 0 / 19 | 0.78 | 0.20 | 0.1013 | 2 / 2 | 0 | 5.80 | 522,992 |
| `tree-random` | 0.050 | 1 / 0 / 19 | 0.58 | 0.095 | 0.0828 | 1 / 1 | 0 | 6.00 | 495,415 |
| `grep`（主对照） | 0.140 | 2 / 1 / 17 | 0.85 | 0.20 | 0.1277 | 4 / 4 | 13 | 6.75 | 972,528 |

20 道四臂共同可评分题的成对比较（95% bootstrap 区间）：

| 比较 | 均差 | 区间 | 右好/左好/平 |
|---|---:|---|---:|
| tree − grep | −0.015 | −0.18 … +0.15 | 2 / 2 / 16 |
| tree-nolabel − grep | −0.09 | −0.23 … 0.00 | 0 / 2 / 18 |
| tree-random − grep | −0.09 | −0.23 … 0.00 | 0 / 2 / 18 |
| tree-nolabel − tree | −0.075 | −0.20 … 0.00 | 0 / 2 / 18 |
| tree-random − tree | −0.075 | −0.20 … 0.00 | 0 / 2 / 18 |
| tree-random − tree-nolabel | 0.00 | 0.00 … 0.00 | 0 / 0 / 20 |

只有 5 题出现过 correct/partial：`bcp:435` 四臂全对；`bcp:449` 只有 tree 对；`bcp:342` 只有 grep 对；`bcp:20` 只有 grep partial；`bcp:732` 只有 tree partial。其余 15 题四臂全错；51/80 答案是对 5 轮预算的显式拒答。树/标签/分枝来源三个消融问题都没有得到正向证据。

## 过程与结构诊断

- **收窄面失效（主因）**：19/20 题的标注证据集合最小覆盖节点就是树根（100,195 篇），唯一例外 `bcp:20`（24,300，深度 1）；金标文档所在叶大小中位 13.5（范围 2–23）。BrowseComp-Plus 证据天然跨主题，树导航在结构上无从收窄。
- **树实际使用率低**：grep 调用中带 `node_id` 限定的比例 tree 13/100、nolabel 5/83、random 9/89，其余为全库 grep；每题触及节点数 1–2。
- **无引用行为**：三个树臂 20 题 0 条 `[[E..]]` 标记（grep 臂 13/20 题、65 个标记），树臂判分上下文全部走 `retrieved_fallback`。
- **成本**：grep 972,528 tokens ≈ 树臂（495k–550k）的 1.9 倍；耗时四臂相近（245–270 秒/题）。上下文充分度均值仅 0.10–0.20。

## 补充诊断：树失效的根因（同日追加）

1. **结构无收窄面**：19/20 题证据集合最小覆盖节点 = 树根。见上节。
2. **导航线索是噪声（构建缺陷）**：节点词标签 = 成员文档原始词频 top-5（无 IDF/信息量加权），停用词过滤不完整（实测 `lexical_tokens` 放行 `his`/`your`，`s` 等残片常见）。根节点标签 `['s', 'i', 'his', 'he', 'you']`，深度 1 节点 `['s', '1', 'from', '2', 'game']`；代表文档取 `min(members)`（最小文档 ID）而非中心示例，根的代表是 doc `0`、标题 `0`。agent 首轮看到的分枝卡几乎不携带可判读语义。
3. **提示词有描述、无策略，且有残留错误**：`tree-skill.md` 与 `tree_overview.md`/`tree_list.md` 描述了树能力，但没有使用策略；所有臂共用的 `grep.md` 仍写着"范围宽时先用混合检索收窄"并提到 outline/语义图的 `ranges` 句柄——这些工具在本实验不存在（工具描述未为树实验改写）。
4. **行为上被放弃**：首调用为根 overview 的题 tree 18/20、nolabel 17/20、random 17/20；此后 `tree_list` 全场 0–2 次、带 `node_id` 的 grep 占 5–13%、每题触及节点 0.35–0.65。随后退回全库 grep，与对照相同——本轮实际比较的是"一次噪声 overview + grep"与"grep"。
5. **预算机制**：`max_rounds=5` 工具轮 + 第 6 轮"预算已用完"closeout 提示（无工具），80/80 全部由此收尾、51/80 为显式拒答。grep 全库字面扫描：成功一次 26–28 秒；范围 >1000 文档时 45 秒硬超时且超时无任何返回，超时占各臂 grep 的 26–52%、占每题墙钟 30–47%；成功调用 20–37% 零命中；`read` 仅 0.2–0.45 次/题。主对照首调用 20/20 是 `list_documents`（10 万文档平铺清单，同样近似浪费一轮）。
6. **语料构成与基准协议**：100,195 文档是 BrowseComp-Plus 的官方固定语料（830 题共享），与本题集的 207 份标注证据（0.21%）并非逐题绑定；该基准的标准协议是先用检索器取 top-k 再作答（作者公布：BM25 证据 Recall@5 1.2%、Agent+BM25 答案准确率 14.58%、Agent+Qwen3-Embedding-8B 35.42%、直接给标注证据 93.49%，见 [hard-benchmark 调研](2026-09-10-vgi-rag-hard-benchmark-research.md)）。保留全库本身是基准的正确用法（上一轮 248 文档材料库正因饱和被否）；使本轮对照失去区分力的是"移除全部排序检索 + 5 轮 + 45 秒超时"的约束组合。

因此本轮不能解释为"树机制无价值"。有效重测至少需要：修标签（IDF/停用词/代表文档）、选一个证据存在可收窄结构的题集、给足轮次与检索预算，并修正工具描述。

## 结论与限制

1. 该树对该题集在该预算下没有质量增益证据；标签没有增益证据；内容分枝优于随机分枝没有证据。本结果不能外推为"任意树无价值"，也不能用于否决 VGI 管道（本轮不含 BM25/embedding/SAC）。
2. 失败的共同形态是早停拒答：5 轮、每题约 6 次工具调用，在 10 万文档全库上远不足以定位跨主题证据；主对照 grep 也仅 2/20 全对，说明当前没有可用的工作点来区分架构。
3. 限制：单一 20 题开发集、单重复、无保留集与独立重复；裁判与回答模型同族；证据覆盖按"工具读到"计，不等于引用准确；token 为客户端 reported 值；BRIGHT 诊断未跑；40 道旧保留题仍未运行；未做大库性能与费用结论。

## 回执入口

- 独立工程回执：[challenge20-tree-run-9.md](C:/Users/xingc/Documents/Codex/repository-tree/evidence/challenge20-tree-run-9.md)、[challenge20-tree-run-9.json](C:/Users/xingc/Documents/Codex/repository-tree/evidence/challenge20-tree-run-9.json)
- 运行目录：`C:/Users/xingc/Documents/Codex/repository-tree/.eval/hard/challenge20-tree-run-9/`（`summary.json`、`analysis.json`、`native-judge/`）
- 判分与汇总脚本：`scripts/analysis/judge_challenge20_tree.py`、`scripts/analysis/summarize_challenge20_tree.py`
- 运行轨迹：run-1/2 回执与 09-13 修复链（`17c58c5` → `f9d1b59`）；本轮为首次 80/80 无基础设施错误的完整四臂数据
