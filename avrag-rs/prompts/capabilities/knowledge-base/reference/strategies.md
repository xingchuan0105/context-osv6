# 检索策略层（knowledge-base/strategies）

首轮随 knowledge-base 披露的**薄层**：覆盖清单、entity-first 原则、**唯一** spoke 目录（含加载触发）、默认路径、终止观察、短 gotcha。  
Few-shot 与长 gotcha 默认在场景 spoke 中，按需 `skill_request` 加载。最小 dense 可执行形态（FS-C1）在 knowledge-base skill 常驻。

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
- 装配 pack 前，清单上仍为未勾的项写入 `gaps`（「回传未覆盖」），而非「语料一定没有」。

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

## 场景 spoke 目录（按需加载 · 本包唯一全表）

| spoke | 内容 | 加载触发（题干或本轮计划中出现） | skill_request 例 |
|-------|------|----------------------------------|------------------|
| `knowledge-base/strategies-graph` | 图扩邻种子、entity-first FS、图向 gotcha | 关系 / A 与 B / 两端 / 「联系」「对应」/ 扩邻方向 | `["knowledge-base/strategies-graph"]` |
| `knowledge-base/strategies-tables` | 表路径、行数/去重、grep vs struct | 表格 / 多少行 / COUNT / 去重 / 管道行 / struct | `["knowledge-base/strategies-tables"]` |
| `knowledge-base/how-to-read-tables` | 管道表通用读法（薄） | 读 `\| … \|` 行、第一个/表序、total_hits | `["knowledge-base/how-to-read-tables"]` |
| `knowledge-base/strategies-grounding` | 结构人数≠访谈、跨文档、未覆盖边界 | 调研人数、跨文档联系、半截覆盖、业界对照槽 | `["knowledge-base/strategies-grounding"]` |
| `knowledge-base/strategies-codegen` | 依赖链 gather、大段 print 占窗 | 沙箱报错、catalog→query 并行空回、print 占窗 | `["knowledge-base/strategies-codegen"]` |
| `knowledge-base/strategies` | 重载本薄层 | 需重新置顶薄层清单时 | `["knowledge-base/strategies"]` |

表类题常 **tables + how-to-read-tables** 一并请求。一次可并请求多个 spoke；环境不默认塞入全部 few-shot。

## 默认路径（一屏）

- **表类**（计数/过滤/表序/聚合）→ 优先 struct 两段式（catalog→query）；行级字面用 `grep`。`total_hits` / row_ord / 多口径计数的**权威说明**见 **how-to-read-tables** 与 **strategies-tables**（本层不重复展开）。
- **金额/编号/表内字面** → `lexical` / `grep`；`dense` 作定位线索。
- **元数据 Date/Status** → 中英双词并行探测。
- **同一主张上** dense 叙述与 lexical/grep 精确数字并存时，精确数字侧通常是更硬的回传支撑（叙述侧仍可保留主题定位）。
- **证据** → 收入 pack 的条目指向本轮 observation / alias；本通道不写用户终答。
- **多轮工作集** → 检索后输出 `KEEP: #n,#m`（支撑当前主张的命中）；宿主优先注入工作集并折叠更早轮正文（协议见 skill「KEEP」）。
- **多口径 / 干扰** → 各口径 alias 都收入 evidence / KEEP；不依赖 chunk 可见面敲除。

## 离开检索前的观察条件（终止 checklist）

下列在回传上**同时可读出**时，停止继续写代码、交宿主装配 pack 是环境中的常见收束点——不是第二套 host 硬闸：

1. Brief 拆出的**每个独立主张**均已有覆盖状态：回传已支撑 / `gaps` 已写明「当前回传未覆盖」。
2. **且** 下列之一成立：
   - 最近 1–2 次**已换方法或换种子**的检索，未再出现新的高价值 alias；或
   - 未覆盖侧已在 gaps 中显式落边界（不把未知写成「语料不存在」）。
3. 采用的文档命中带 alias，写入 evidence；本通道不写用户终答，也不写文末 `SELECTED`。

仍有未试过的实体面 / 英文面 / 结构面时，「新 alias≈0」只描述**已试查询形态**的饱和，不自动等于全库穷尽。

## 通用 gotcha（最短提醒）

| 现象 | 回传实际含义 | 常见误读 |
|------|--------------|----------|
| `dense` 高分只有概念叙述 | 主题相关；目标数字可能仍未知 | 从叙述「推」出未出现数字 |
| 连续轮次新 alias≈0 | 该查询形态饱和 | 同义重扫却期待新覆盖 |
| 零轮 `client.*` 即停止 | 文档侧均未覆盖 | 常识当库内已检索 |
| `stderr` 非空 | 检索面未更新 | 读成「语料不存在」 |

表通用读法 → **how-to-read-tables**；表路径/多口径 FS → **strategies-tables**；拒答与跨文档边界 → **strategies-grounding**。沙箱报错形态见本轮 `[sandbox_error]`。最小 dense 写码形态见 knowledge-base skill（FS-C1）；依赖链 / print 噪声 → `{"skill_request": ["knowledge-base/strategies-codegen"]}`。
