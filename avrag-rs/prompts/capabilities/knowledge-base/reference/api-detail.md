---
name: knowledge-base-api-detail
description: >-
  L2 runtime detail for knowledge-base: empty/truncate/fail tables, minimal
  success code shapes, SELECTED/KEEP full tables, citation markers and
  code_execution_result line format. Disclosed on first retrieve round or after
  sandbox error; not every-round always-on.
---

## 并行与依赖（KB 侧）

相互独立的 `client.*` 调用在**同一代码块**内并行发出是默认形态（入口与 `gather` 形态见 agent-base）。同块内各方法空/非空彼此独立；存在依赖时（如 `struct_query` 的表名来自 `struct_catalog`，或 `doc_summary` 的 `doc_id` 来自 docscope）按顺序 await。

宿主 `<loop_budget round baseline_rounds max_rounds>` 描述 retrieve 回合进度：常见基线 2 为软参考（如进度 3/2 表示已超过基线）；不依赖前序回传的 dense/lexical/grep 等适合首块一次扇出，有依赖链的步骤再进下一回合。

## 最小可成功首块（示例形态）

**A. 独立检索同块并行（无依赖）**

```python
import asyncio
chunks, hits = await asyncio.gather(
    client.dense("实体名"),
    client.lexical("关键编号或金额字面"),
)
print("dense n=", len(chunks), "lexical n=", len(hits))
for c in chunks[:3]:
    print(c.get("alias"), (c.get("content") or c.get("text") or "")[:120])
```

**B. 有依赖：catalog → query 串行**

```python
cat = await client.struct_catalog()
print("relations", [r.get("name") for r in cat.get("relations") or []])
# 表名来自上一回传后再 query（勿对空 catalog 盲 COUNT）
q = await client.struct_query("SELECT COUNT(*) AS n FROM 表名")
print(q)
```

**C. 关系型：双端 dense 并行**

```python
import asyncio
side_a, side_b = await asyncio.gather(
    client.dense("实体甲"),
    client.dense("实体乙"),
)
print("side_a n=", len(side_a), "| side_b n=", len(side_b))
```

## 空结果、截断与失败

| 观察 | 含义 |
|------|------|
| `dense`/`lexical` 返回 `[]` | 该 query 下无片段入选；换说法/换方法后可能仍有结果 |
| `grep`：`total_hits=0` | 该 pattern 无行命中；pattern 形态（如 `\| 值 \|`）常影响结果 |
| `struct_catalog`：`relations=[]` | 当前 scope 无表格存储；grep/dense 仍可用，非回归 |
| `struct_query`：`ok=false, code=unknown_relation` | 所查表名不在 catalog；catalog 中有当前可见表名列表 |
| `truncated=true` 或 hits 达 `max_hits` | 回传是样本，不是全库枚举；计数结论以 `total_hits` 为准，**已见** hits/content 仍是有效证据 |
| list 非空但无目标字段 | 主题相关 ≠ 主张已覆盖 |
| content 已含条目，却称「被截断无法作答」 | 与 observation 不一致：已见正文覆盖状态仍为已覆盖 |
| `stderr` 非空 | 执行失败；下一轮可给修正后的同一形式代码块 |
| 未调用某方法 | 该方法下的证据状态仍为未知，不是 0 命中 |
| 连续轮次新 alias = 0 | 同一查询形态下检索面已饱和；收窄或定稿是常见下一步。未试角度不受此推断 |

同块并行多种方法时，各方法的空/非空彼此独立。

## struct / fts 细节

- `struct_catalog` 的 `relations[]` 常见字段：`name`、`headers`、`n_rows`、`sample_rows`、`caption`、`unit`、`confidence`、`fts`。catalog 只描述表；表内数值与答案只出现在 `struct_query` 的 `rows`。
- `struct_query`：`ok=false` 时含 `error.code`（`forbidden` / `unknown_relation` / `no_relations` 等）。`row_count` 是 SQL **结果集**行数；COUNT/SUM 数值在 `rows` 单元格内。
- `fts: true`：`WHERE fts_main_<表名>.match_bm25(row_ord, '关键词') IS NOT NULL`；空格分隔 token 有效；整串中文常是单 token。`fts: false` 时 match_bm25 会报 schema 不存在。
- 多 doc 同名表：响应可含 `ambiguous_relations`；查询静默归属首个出现的 doc——用 `doc_ids` 收窄。

## SELECTED / KEEP 细则

命中常带 **`alias`**（如 `#1`）。

| 观察 | 含义 |
|------|------|
| 句级标记 | 有回传支撑的文档侧主张**句末**常见 `（#n）` / `(#n)`；n 为回传 **`#alias`**（不是 chunk_id） |
| SELECTED 位置 | **末行**（其后无更多散文）；前缀 `SELECTED` 或 `选择`，后接 `:` / `：` |
| 条目 | 回传中的 **`#alias`**；历史轮次已出现的 alias 仍有效 |
| 与主张 | 终答写出的每个文档侧主张宜在句末带对应 alias，并出现在末行 SELECTED；只写 SELECTED 无句内标记时，用户侧只见文末角标 |
| 与 KEEP | 宜 ⊆ 本 run 曾 `KEEP` / 工作集中的 alias |
| 空集 | 无可用 alias 或全文未采用命中时，正文已表明未覆盖/未采用即可；无强制空行 `SELECTED:` |
| 双源 | 与联网同挂时：doc 侧句末 `（#n）` + 末行 `SELECTED`；联网侧句末 `[[web:n]]` |
| 宿主交付 | `（#n）` → 可点击引用；SELECTED 协议行不进入用户主气泡 |

| KEEP 观察 | 含义 |
|-----------|------|
| 前缀 | `KEEP` 或 `保留`，后接 `:` / `：` |
| 条目 | **`#alias`**（不是 chunk_id） |
| 无 KEEP / 空列表且无法解析 | 宿主**保持**上一轮工作集（sticky） |
| demote | `KEEP_DROP: #5`（或 `保留删除`） |
| 与 SELECTED | 终答 `SELECTED` 宜来自本 run 中 KEEP 过的集合 |

出现 `KEEP` 后下一轮主上下文带 `[ews_active]`；进入合成前宿主末端再注 `[evidence_reread]`。  
**KNOCKOUT** 硬抑制已关闭。离开检索前的覆盖闭合 / 饱和 / SELECTED 末行条件见薄层 **strategies**。

## 引用标记与回传块格式

- `[[cite:<chunk_id>]]` —— 终答文档块引用；只圈 cite 不圈 alias 时引用状态与 SELECTED 无关。
- `[[image:<chunk_id>]]` —— 内联图片块引用。
- `[[web:n]]` —— 联网引用索引。

`<code_execution_result>` 内部为逐块观察行：

`[block N] stdout: …\nstderr: …`

- 成功块固定 `stdout:` / `stderr:`；失败块为 `[block N] Execution failed: …`。
- 证据判定按块内 stdout 是否携带 chunk 载体（uuid 形 id）；占位输出不算证据。
