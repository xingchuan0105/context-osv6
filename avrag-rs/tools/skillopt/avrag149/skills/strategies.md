# 检索策略层（knowledge-base/strategies）

首轮随 knowledge-base 披露的**薄层**：覆盖清单、entity-first 原则、场景 spoke 目录。  
Few-shot 与长 gotcha 表在场景 spoke 中，按需 `skill_request` 加载（见下）。

## 多主张覆盖（轻量清单）

用户问题含 **多个可独立核验的主张**（多个数字、多个阶段、两篇文档对照、知识库+联网各一侧等）时，常见覆盖形态：

```
Claim checklist (copy and tick against returns):
- [ ] claim A — 回传中出现支撑字段/数字/表行
- [ ] claim B — 同上
- [ ] …（按问题拆）
- [ ] 联合结论 — 仅在 A/B… 均有回传支撑时写出；缺侧标「当前回传未覆盖」
```

- 只覆盖部分主张时，未覆盖侧保持 **未知**，不拿已覆盖侧的叙述填补。
- 双源（知识库 + 联网）时，两侧证据分源引用；一侧未取回传则该侧未知。
- 最终答复前，清单上仍为未勾的项对应「回传未覆盖」，而非「语料一定没有」。

## Entity-first 与 dense 种子（原则）

`client.dense(query)` 的 `query` 同时驱动向量召回，并作为宿主关系扩邻（VGRAG）的种子来源。沙箱无 `client.graph`；可控的是 query 粒度与并行次数。

| 问题形态 | 常见 dense 形态 | 读出的机制 |
|----------|-----------------|------------|
| 单主张、单实体 | `dense("实体名")` 或 `dense("实体名 属性槽")` | 一种子方向 |
| 关系型（A 与 B） | 同块两次 `dense`，种子分别为 A、B | 两端邻域有机会汇合 |
| 多主张 | 每主张/实体一次 `dense`，同块并行 | 清单可分项打勾 |
| 跨文档 | 每侧实体各一次 + 可选 `doc_ids` | 避免一侧淹没另一侧 |

- 整句多实体作唯一种子时扩邻更散；种子宜贴近语料原始术语与最短属性槽。
- 同实体整句同义改写饱和时，换实体角度通常比再改写更有效。
- 细则与 few-shot：**knowledge-base/strategies-graph**。

## 场景 spoke 目录（按需加载）

| spoke | 内容 | 典型时机 |
|-------|------|----------|
| `knowledge-base/strategies-graph` | 图扩邻种子、entity-first FS、图向 gotcha | 关系/多实体 dense、扩邻方向散、饱和换端 |
| `knowledge-base/strategies-tables` | 表路径「摸范围→收窄→下钻」、行数/去重、grep vs struct | 表内计数/过滤/排序、管道行、struct 可用 |
| `knowledge-base/strategies-grounding` | 结构人数≠访谈、跨文档联系、未覆盖边界 FS | 调研人数、跨文档「有何联系」、半截覆盖 |
| `knowledge-base/how-to-read-tables` | 管道表 ontology 与误读对照 | 读 `| … |` 行、row_ord、total_hits |

加载形态（环境事实）：`{"skill_request": ["knowledge-base/strategies-graph"]}` 等；本层可 `["knowledge-base/strategies"]` 重载。

## 默认路径（一屏）

- **表类**（计数/过滤/表序/聚合）→ 优先 struct 两段式（catalog→query）；`grep` 的 `total_hits` 数文本行；未声明去重时行数口径优先。细项：**strategies-tables** + **how-to-read-tables**。
- **金额/编号/表内字面** → `lexical` / `grep`；`dense` 作定位线索。
- **元数据 Date/Status** → 中英双词并行探测。
- **证据** → 终答主张指向回传 alias；`SELECTED: #n`。

## 通用 gotcha（短表）

| 现象 | 回传实际含义 | 常见误读 |
|------|--------------|----------|
| `dense` 高分只有概念叙述 | 主题相关；目标数字可能仍未知 | 从叙述「推」出未出现数字 |
| 多数字题只见一个数 | 其余主张仍未知 | 只答一半即结束 |
| 连续轮次新 alias≈0 | 该查询形态饱和 | 同义重扫却期待新覆盖 |
| 零轮 `client.*` 即终答意图 | 文档侧均未覆盖 | 常识当库内已检索 |
| `dense` 有 alias 终答无 `SELECTED` | 主张无引用圈定 | 有 hit 仍不圈 alias |
| 执行失败 / stderr 非空 | 检索面未更新 | 读成「语料不存在」 |

表/图/拒答类长对照见对应 spoke；沙箱写码噪声见 **strategies-codegen**（若已加载）或本 skill 方法表。

## 沙箱 Python 噪声（短）

| 现象 | 含义 |
|------|------|
| `graph_search` / `dense_search` / `top_k=` | 契约是 `client.dense` 等且无 `top_k` |
| 忘记 `await` | 协程未执行 |
| 有依赖却 `gather` 并行 | 空 doc_ids / 错参 |
| `import os` 等 | 沙箱禁止 |
| 只 `print` 大段正文 | 回传窗口被占满 |

更全的 codegen 对照：`{"skill_request": ["knowledge-base/strategies-codegen"]}`。
