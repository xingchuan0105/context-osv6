
## Job

Extract grounded `(subject, predicate, object)` triples for a knowledge graph. Edges use a **small ontological relation set** among entities (kinds, individuals, processes)—not free natural-language verbs. Domain meaning sits on **nodes**; edges are foundational links only.

**Output:** one single-line JSON only (no fences, no preamble):

```json
{"triplets":[{"chunk_id":"uuid","subject":"...","predicate":"类型","object":"..."}]}
```

Input shape: `Valid chunk IDs: …` + `Chunks: {"chunks":[…]}` + `Extract triplets with chunk_id:`.

Limits: ≤40 triples/batch; invalid/missing `chunk_id` dropped; empty `{"triplets":[]}`. Prefer **few high-quality edges** over padding (software ADRs and short sections often yield ≤3 per chunk when only clear entities exist).

---

## Closed ontological predicates

| predicate | Role | Direction (S → O) |
|-----------|------|-------------------|
| `类型` | classification / is-a | individual or named item → **kind, category, phase, role-class** (short class label) |
| `部分` | mereology | **part → whole** (S is a component of O; both independently nameable aggregates) |
| `参与` | participation | continuant (agent, system, artifact) → **process / activity / event** |
| `依赖` | dependence | dependent entity → bearer / required entity / resource |
| `位于` | location | entity → **spatial or temporal region** stated as place/time |
| `标识` | denotation | **stable catalog/code id → short display name** (same table row) |

No other `predicate` strings. Facts that do not fit one of the six relations are simply absent from `triplets`.

---

## Few-shot (情境 → 回传观察 → 读出的图事实)

### F1 — Catalog row (table)

**情境：** 管道表一行：`PAC-05 | 概念启动 | 概念阶段 | …`

**观察：** 同一行同时出现稳定活动号、短名、阶段列。

**读出的图事实：**

```json
{"triplets":[
  {"chunk_id":"<id>","subject":"PAC-05","predicate":"标识","object":"概念启动"},
  {"chunk_id":"<id>","subject":"概念启动","predicate":"类型","object":"概念阶段"}
]}
```

（未把职责长句做成 `标识` 的 object。）

### F2 — Module and component (prose)

**情境：** 「`EvidenceGate` 是 ExecuteRetrieve 阶段的质量过滤组件。」

**观察：** 组件名与阶段名均可独立指称；文中是归属/参与过程，不是地理坐标。

**读出的图事实（择一清晰边即可）：**

```json
{"triplets":[
  {"chunk_id":"<id>","subject":"EvidenceGate","predicate":"参与","object":"ExecuteRetrieve"}
]}
```

（未建 `(EvidenceGate, 类型, pure-code fast filter)` 这类描述短语类；未建 `(EvidenceGate, 位于, avrag-llm)`。）

### F3 — Document title denotation

**情境：** 标题「ADR-0004: RAG Agent Loop & Native Tool Calling」

**观察：** 稳定文档编号与标题短语成对出现。

**读出的图事实：**

```json
{"triplets":[
  {"chunk_id":"<id>","subject":"ADR-0004","predicate":"标识","object":"RAG Agent Loop & Native Tool Calling"}
]}
```

### F4 — Dependence without process framing

**情境：** 「Loop 边界依赖 StrategyExecutor 驱动。」

**观察：** 两个可指称构件；文中是依赖/基于，不是「谁在执行哪场活动」的参与句。

**读出的图事实：**

```json
{"triplets":[
  {"chunk_id":"<id>","subject":"Loop Boundary","predicate":"依赖","object":"StrategyExecutor"}
]}
```

### F5 — Mereology only for real part–whole

**情境：** 「验证计划含 Slice 1 / Slice 2 / Slice 3 三个切片。」

**观察：** 切片是计划下的可独立命名部分。

**读出的图事实：**

```json
{"triplets":[
  {"chunk_id":"<id>","subject":"Slice 1","predicate":"部分","object":"验证计划"},
  {"chunk_id":"<id>","subject":"Slice 2","predicate":"部分","object":"验证计划"},
  {"chunk_id":"<id>","subject":"Slice 3","predicate":"部分","object":"验证计划"}
]}
```

### F6 — Sparse when structure fields only

**情境：** 结构体字段列表：`ChatMessage { role, content, tool_calls }`，无整体–部分自然语言。

**观察：** 字段名是记录槽位，不是可独立生命周期的组件聚合关系。

**读出的图事实：**

```json
{"triplets":[]}
```

（或仅当文中明确「Message 由 … 组成」时才出现 `部分`。）

### F7 — Location is place/time, not code path

**情境：** 「测试 `test_rag_agent_loop` 写在 `strategy_rag.rs`。」

**观察：** 文件路径是仓库坐标，不是地理/时间区域。

**读出的图事实：** 通常 `{"triplets":[]}` 或仅保留与测试类型相关的边；**不出现** `(test_rag_agent_loop, 位于, strategy_rag.rs)`。

---

## Gotchas（轨迹中反复出现的误建边）

| 现象 | 图上合法读法 | 常见误建 |
|------|--------------|----------|
| 状态机状态名 `Plan` / `Answer` 与「线性管道」同段出现 | 状态是过程节点；无明确「部分–整体」措辞时 **可不建边** | `(Plan, 部分, one-way linear pipeline)` |
| 结构体/消息字段 `role`、`tool_calls` | 字段是属性槽；默认可 **omit** | `(tool_calls, 部分, ChatMessage)` |
| 「X 是一种 Y 过滤器/管道」式**行为描述** | O 不是类标签 → 不建 `类型`，或只建参与/依赖 | `(RagStrategy, 类型, one-way linear pipeline)` |
| 「Slice 1: Native Tools in avrag-llm」标题行 | 标题≠类；编号切片对计划可用 `部分` | `(Slice 1, 类型, Native Tools in …)` |
| crate / 文件 / 模块路径 | **不是** `位于` 的 locus | `(Slice 1, 位于, avrag-llm)`、`(test, 位于, foo.rs)` |
| 职责/程序长句作 object | `标识` 的 object 仅为短名 | `(ME-10, 标识, 探索可选概念和…整句)` |
| 执行行号 + 职责，无目录短名 | 无 `标识`；有活动名时可用 `参与` | 执行行 ID `标识` 到职责全文 |
| 同 chunk 重复同一 (S,p,O) | 一条即可 | 为凑条数重复 |
| 谓词写成 `属于`/`包含`/`implements` 等开放词 | 合法输出只有六 id；表面同义由 host 归一或丢弃 | 输出开放动词导致边进不了封闭图 |
| 多跳「然后 / 下一步」状态转移 | 六关系**无顺序边**；无参与/依赖承载时 **omit** | 硬拧成 `部分`/`类型` |
| 金额、日期、单纯形容词 | 宜属性/omit，不是边 | 为每个数字建 `位于`/`类型` |

---

## Tabular column roles (compact)

| Role | Typical edge |
|------|----------------|
| Catalog ID + Short name (same row) | `(ID, 标识, Short name)` |
| Short name + Category/phase | `(Short name, 类型, Category)` |
| Named sub-step under named parent | `(sub-step, 部分, parent)` |
| Role/org + named activity | `(role/org, 参与, activity)` |
| Duty prose only | often omit; never `标识` to duty text |

---

## Fields

- `subject` / `object`: grounded noun-phrase **entities** attested in the chunk.
- `predicate`: exactly one of `类型|部分|参与|依赖|位于|标识`.
- `chunk_id`: one of the valid UUIDs from the batch.
