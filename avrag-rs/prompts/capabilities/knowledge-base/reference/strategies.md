# 检索策略层（knowledge-base/strategies）

随 knowledge-base skill 首轮披露；可用 `{"skill_request": ["knowledge-base/strategies"]}` 重新加载。

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

## Known gotchas

轨迹中反复出现的读法陷阱（均可用虚构表自检；细节见 how-to-read-tables）：

| 现象 | 回传实际含义 | 常见误读 |
|------|--------------|----------|
| 标签 `ROW-04` 数值小于 `ROW-03` | 标签是名字，不是排序键 | 把「第一个」读成编号最小/字典序最先 |
| 表中先出现 `STEP-03` 再出现 `STEP-04` | 「第一个」= 出现顺序在前的那一行 | 按步骤号重排后再取 min |
| `total_hits=12` 且品名列有重复 | 12 = **命中行数** | 按品名去重后改成更小的数 |
| `dense` 高分片段只有概念叙述 | 主题相关；目标数字/表行可能仍未知 | 用叙述段「推」出未出现的数字 |
| `struct_catalog` 中 `confidence=low` | 灌入监督未全部通过；该表数字处于低置信状态 | 与 high 置信表同等引用 |
| `struct_query` 的 `row_count` | SQL **结果集**的行数；COUNT 的答案在 `rows` 单元格内 | 把结果行数当成 COUNT 值 |
| `truncated=true` 或 hits 长度 < total_hits | hits 是样本；计数以 `total_hits` 为准 | 用 `len(hits)` 当全库计数 |
| 问题字面与某段很像 | 相似 ≠ 主张已覆盖 | 跳过 lexical/grep 精确核对 |
| 多数字题只见一个数 | 其余主张仍未知 | 只答一半即结束 |
| 知识库与联网同时挂载，问题含「文章称/文中提到/报告称」 | 该前提通常指向**文档库**，可用 `dense`/`grep` 直接核实原文 | 当成外部事实只走 `client.web`，文档侧前提未覆盖 |
| 谓词类 `grep`（废弃/deprecat/remove/obsolete…）连续 0 命中 | 谓词可能是概念性的，不一定逐字出现在正文；需要**实体侧探测**（`grep` 目标文件/类/方法名、`doc_profile` 看章节结构）换角度 | 0 命中直接得出「未覆盖」并停止 |
| `dense` / `lexical` chunk 正文已含目标表述，同词 `grep` 0 命中 | 覆盖判定以回传正文为准；`grep` 未命中只反映 pattern 形态与词面差异 | 以 `grep` 未命中为由宣布「未覆盖」 |

## 默认低自由度路径（易碎结论）

表类问题（表内计数 / 过滤 / 表序 / 排序 / 聚合）的默认工作流是**「摸范围 → 收窄 → 下钻」一条链**：

1. **摸范围**：并行扇出 `dense` / `lexical` / `grep` 或 `struct_catalog`，确认问题落在哪个 doc、哪张表。
2. **收窄**：取到 doc_id 或表名后，后续调用带 `doc_ids=[...]`（多 doc 同名表时防止静默归属首个 doc）。
3. **下钻**：`struct_catalog` 给出可见表名与列名后，**继续**用 `struct_query` 发单条 SELECT 取答案——catalog 只描述表，答案只在 `rows` 里。

分流规则：**`grep` 数的是文本行，`struct_query` 的 COUNT/SUM 数的是记录**。表内计数、过滤、排序、聚合场景下，grep 是近似路径（按文本行/子串，可能与表结构错位），`struct_query` 是确定路径（按列与谓词）——两类场景一律先走 struct 两段式，`grep` 降为无表格存储（`relations=[]`）或纯子串/邻域场景的退路。

- **表内计数 / 过滤 / 表序 / 聚合（表类问题首选）** → **struct 两段式**：`struct_catalog`（看可见表名与列名）→ `struct_query`（COUNT/WHERE/ORDER BY/GROUP BY，单条 SELECT）；「第一个」= `row_ord` 升序第一行（表出现序），非编号字典序；`struct_query` 的 `row_count` = 结果集行数，COUNT 数值在 `rows` 单元格。
- **行计数 / 纯文本行** → `grep` + 采用 `total_hits`（不要肉眼数 hits、不要按列去重）；`struct_catalog` 返回 `relations=[]`（该 doc 无表格存储）时 grep 是可用退路。
- **表内总数（如某类对象的总数）** → `struct_query` 聚合（COUNT/SUM/GROUP BY）是确定路径；看到部分分域计数而未见总数时，总数仍处于未覆盖状态，聚合查询可闭合它。表级证据未水合（回传无 alias 编号）时，以 `evidence`/`rows` 文本核对，勿虚构编号。
- **表内「第一个 / 先后」** → 按 **回传中该过滤条件下的出现顺序**（或显式序号列）；编码字符串不做排序键。
- **金额 / 活动号 / 表内字面** → 优先 `lexical` 或 `grep`；`dense` 用作定位线索——定位到的 chunk 正文里出现的数字与表述即为有效证据，`grep` 同词 0 命中不改变已回传 chunk 的覆盖状态。
- **元数据字段（日期/状态/作者/阶段数）** → 语料字段常为**英文**（如 `Date`、`Status`、`Phase`），中文语料正文用中文——检索词**中英双词并行**（`grep "Date"` 与 `grep "日期"` 都试；`Phase` 与 `阶段` 都试）；英文 0 命中不代表中文侧也无，反之亦然。
