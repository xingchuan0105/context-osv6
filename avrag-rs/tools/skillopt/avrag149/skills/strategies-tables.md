# 表与行级路径（knowledge-base/strategies-tables）

按需加载：`{"skill_request": ["knowledge-base/strategies-tables"]}`。  
管道表 ontology 与误读对照：**knowledge-base/how-to-read-tables**。

## 默认工作流（摸范围 → 收窄 → 下钻）

表类问题（表内计数 / 过滤 / 表序 / 排序 / 聚合）常见链：

1. **摸范围**：并行 `dense` / `lexical` / `grep` 或 `struct_catalog`，确认 doc 与表。
2. **收窄**：后续调用带 `doc_ids=[...]`（多 doc 同名表时防止静默归属首个 doc）。
3. **下钻**：`struct_catalog` 看表名列名后，**继续** `struct_query` 单条 SELECT——catalog 只描述表，答案在 `rows`。

分流：**`grep` 数文本行，`struct_query` 的 COUNT/SUM 数记录**。表内计数/过滤/排序/聚合场景优先 struct 两段式；`grep` 是近似路径或 `relations=[]` 时的退路。

- **表内计数 / 过滤 / 表序 / 聚合** → struct 两段式；「第一个」= `row_ord` 升序第一行，非编号字典序；`row_count` = 结果集行数，COUNT 在 `rows` 单元格。
- **行计数 / 纯文本行** → `grep` + `total_hits`（不肉眼数 hits、不按列静默去重）；无表格存储时 grep 是退路。
- **表内总数** → `struct_query` 聚合是确定路径；部分分域计数 ≠ 总数已覆盖。
- **「第一个 / 先后」** → 回传中该过滤条件下的出现顺序（或显式序号列）。
- **金额 / 活动号 / 表内字面** → 优先 `lexical` 或 `grep`；dense 作定位线索。
- **元数据 Date/Status/Phase** → 中英双词并行探测。
- **非表类题（文档元数据 / 论点综述 / 行业·市场数据 / 代码通道）** → 正文 `dense`/`lexical`/`grep` 或文档画像即可，**不走 struct 两段式**；struct 仅在问题明确指向表格内容（计数/过滤/排序/聚合）时启用。

## Few-shot

### FS3 — 表内 / 行级计数

**情境：** 虚构「园区检修工单表」中状态「待派工」有多少行。

**观察：** 摸范围可有 `dense("检修工单 待派工")`；闭合计数时出现 catalog→query 的 COUNT，或 `grep` 后采用 `total_hits`；连续同义 dense 无行级数字时转向 struct/grep。

**读出的事实：** 计数不建立在 dense 散文猜个数；**行/记录数** 来自行级或 SQL 回传。

## Gotchas（表 / 行数）

| 现象 | 回传实际含义 | 常见误读 |
|------|--------------|----------|
| 标签 `ROW-04` 数值小于 `ROW-03` | 标签是名字，不是排序键 | 「第一个」= 编号最小 |
| 表中先出现 `STEP-03` 再 `STEP-04` | 「第一个」= 出现顺序在前 | 按步骤号重排取 min |
| `total_hits=12` 且品名列重复 | 12 = **命中行数** | 按品名去重改小数 |
| 问题问「有多少活动/行」且未声明去重 | 与表一致的读法常是**行数** | 静默去重当唯一答案 |
| 阶段「有多少个活动」且仅有 `total_hits` | 未声明去重 → **命中行数** | 自行去重后只报更小数 |
| 同一活动名多角色多行 | 行数与去重逻辑项数是两种口径 | 只报一种且不标明 |
| 合并与行数都能从回传推出 | 可并列并标明 | 只交付一种 |
| `struct_query.row_count` | 结果集行数；COUNT 在 `rows` 里 | 把结果行数当 COUNT |
| `truncated=true` | hits 是样本 | `len(hits)` 当全库计数 |
| `confidence=low` | 低置信表 | 与 high 同等引用 |
| `relations=[]` 但正文有枚举数字 | 无表格存储；正文仍有效 | 因 struct 无 Ok 拒报正文数 |
| 表类题 grep 十余次且 catalog 曾非空 | 确定路径仍是 catalog→query | 用 grep 风暴代替 struct |
| 表内「多少」上同 pattern 反复 | total_hits/COUNT 已是计数面 | 重复 pattern 少新信息 |
| 知识库检索命中无关主题 doc（行业/话题错配） | 该主题在库内无覆盖，可转 web 或如实说明缺数据 | 强行套表路径或编造数字 |
| 问文档类型/体裁/语言 | 来自文档画像，正文检索佐证 | 走 struct 表路径 |
