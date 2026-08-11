# Search as Code SDK + 单 Agent 架构设计（决策定稿）

> **部分取代（2026-08-11）** — **A2「单 agent / 无 worker」** 在产品 agent-lane（rag/search/dual）上被 **Lead + RAG/Web Workers** 取代：见 `docs/plans/2026-08-11-lead-rag-web-workers-design.md`。  
> **仍有效：** A1/A3–A8 检索进 SDK、capability 门控原语子集、无 topk 聚合原语等 — SaC **下沉为 RAG Worker 执行引擎**，不是删除 SDK。

**日期**：2026-07-30  
**状态**：决策定稿（锚点不可偏离）— **A2 已修订见上文横幅**  
**依据**：Perplexity *Rethinking Search as Code* + 现状盘点 + 负责人决策  
**核心**：所有检索工具进一个 SDK；单 agent；前端 capabilities 不变，选哪个就开通对应的提示词 + SDK 子集。

---

## §0. 设计锚点（验收目标，不可偏离）

**以下 8 条是验收标准，实施过程任何偏离都需显式提请评审。**

| # | 锚点 | 验收方式 |
|---|---|---|
| **A1** | **检索无 function calling 并存**：所有检索能力在 codegen 沙箱 SDK 内，native 侧不再有检索 tool（`dense_retrieval`/`web_search`/`web_fetch` 删除或改为非检索） | grep 全仓：native tool 注册里无检索类 |
| **A2** | **单 agent**：无 orchestrator / worker session / brief / handoff / synthesize 分层 | 一个 ReAct loop 从收到指令到出答案 |
| **A3** | **capabilities 开关**：前端按 mode 选，后端注入「该 mode 开通的 SDK 子集 + 提示词」；沙箱只暴露开通的原语 | chat 模式下调不到 grep；rag 模式调不到 web 之外的 |
| **A4** | **原语极简**：`dense(query)` / `lexical(query)` 只传 query，topk/topn 交后端现成机制决定；**无聚合原语**（无 count/dedupe/extract） | SDK 接口签名检查 |
| **A5** | **search = dense + lexical 二合一**（`method` 选）；**graph 不独立暴露**——绑定 lexical（`method="lexical"` 时后端自动并行 BM25 + graph_context，现成机制不破坏）。理由：LLM 不会单独调 graph，绑定后自动返更丰富上下文，提高检索成功率 | 无 `graph`/`graph_search` 原语；lexical 自带 graph_context |
| **A6** | **web 在 SDK**：`web(query)` / `fetch(url)` 进 SDK，可代码 fan-out | 沙箱内可调，native 无 web_search |
| **A7** | **filesystem 跨 turn**：中间状态 `save`/`load` 文件，不走 token（无 handoff JSON 传递） | 无 handoff 数据结构 |
| **A8** | **capability 提示词 < 2000 tokens**，聚焦“如何用 grep 按行读理解内容（表格等连续结构）”，非列举接口 | wc 各 SKILL.md |

---

## §1. 架构总览

```
前端选 capability（mode，不变）
        │
        ▼
┌──────────────────────────────────────────────┐
│ Agent 启动：注入                              │
│  - 该 capability 的提示词（< 2000 tok）       │
│  - 该 capability 开通的 SDK 原语子集（沙箱限） │
└──────────────────────────────────────────────┘
        │
        ▼
┌──────────────────────────────────────────────┐
│ 单 Agent ReAct Loop（A2，无多层）             │
│  1. LLM 写一段 Python（组合开通的 SDK 原语）   │
│     可 fan-out / 并行 / 条件                  │
│  2. 沙箱批量执行（一段代码 → 多检索）         │
│  3. 结果回灌：print 观测 + filesystem 持久(A7)│
│  4. LLM 读结果 → 继续写 or 直接出答案         │
│  → 最终答案直接出（无 handoff，A7）           │
└──────────────────────────────────────────────┘
        │
        ▼
   Compute Sandbox ── Agentic Search SDK（原语）── 检索基础设施
```

**砍掉**：orchestrator / worker / brief / handoff / E105-107 / alias / resume compaction / 多格式 handoff / native 检索 function calling。

---

## §2. SDK 原语设计（A4/A5/A6）

### 2.1 原语清单（10 个，无聚合）

| 原语 | 签名 | 取代（现状）| 说明 |
|---|---|---|---|
| `search` | `search(query, method="dense"\|"lexical")` | dense_search + lexical_search + dense_retrieval + lexical_retrieval | **二合一**；method 选向量/BM25；topk 后端定（A4）。lexical 时后端自动带 graph_context（graph 不独立暴露，见 A5）|
| `grep` | `grep(pattern, *, doc_ids, regex, context, max_hits)` | grep + read_lines | **按行读**（唯一行级原语）；`context` 读邻域，克服 chunk 断裂（表格理解专用）。read_lines 砍（grep+context 覆盖）|
| `web` | `web(query)` | native web_search | **进 SDK**（A6），可 fan-out |
| `fetch` | `fetch(url)` | native web_fetch | **进 SDK**（A6）|
| `doc_profile` | `doc_profile(doc_ids, *, fields)` | doc_profile | 文档结构 |
| `doc_summary` | `doc_summary(doc_ids, *, level)` | doc_summary | 文档摘要 |
| `history` | `history(*, limit)` | conversation_history_load | 进 SDK |
| `user_profile` | `user_profile()` | user_profile_load | 进 SDK |

**明确不要**：~~count~~ / ~~dedupe~~ / ~~extract~~ / ~~graph 独立~~（绑 lexical）/ ~~top_k 参数~~ / ~~chunk_fetch~~（dense 返回全文）/ ~~read_lines~~（grep+context 覆盖行级读）。

### 2.2 为什么不要 count / read（A4 的设计哲学）

**agent 的职责是“检索 + 理解”，不是“检索 + 加工”。**

- `grep` 是**按行读**工具（coding-agent 风格）：返回 `hits[]`（行内容：doc_id/line/text/before/after），`context` 读邻域——agent 用它读表格/连续结构的**完整行**，克服 chunk 切分断裂
- 不给 agent 写聚合代码的机会——没有 count 原语，agent 不会“自作主张去重”
- 不给 agent 冗余读取入口——read_lines 砍（grep + context 覆盖行级读取）
- q088 治理：agent 用 grep 读表格行、自己理解，不加工；没有 count/dedupe 入口 = 没有改错的机会

```python
# 表格理解：grep 按行读（context 带邻域，完整不断裂）
rows = await grep(r"\|.*验证阶段.*\|", regex=True, context=3, doc_ids=[doc_id])
for hit in rows["hits"]:
    print(hit["text"], hit["before"], hit["after"])  # ← 完整行 + 邻域，agent 自己读理解
# 没有 count/dedupe/read——agent 碰不到去重逻辑，也不会用 chunk 断裂的内容
```

### 2.3 dense 极简（A4）

```python
async def dense(query: str) -> list[Chunk]:
    """稠密检索。只传 query——topk 由后端 dense 检索的现成机制决定。
    返回 Chunk 列表（chunk_id/text/doc_id/score）。"""
```

不暴露 topk/topn：后端 `dense_retrieval` 已有默认 topk 机制（现成），SDK 不重复暴露。agent 要更多结果就用 grep（行级全量）或多次 query。

---

## §3. Capabilities 开关（A3/A8）

**前端不变**（仍按 mode 选）。后端按 mode 注入：

| capability | 开通原语 | 提示词重点（< 2000 tok，A8）|
|---|---|---|
| `chat` | history, user_profile | 轻量对话 |
| `rag` | dense, lexical, grep, read, doc_profile, doc_summary | grep+read 组合理解；**信 total_hits 禁加工** |
| `search` | web, fetch, dense | web fan-out + 去噪 |
| `table` | grep, read, doc_profile | **表格语义**（整行=记录，grep 定位列，total_hits 精确）|
| `cross_doc` | dense, lexical, read | 多 doc fan-out + 半载自检 |

**沙箱按 capability 限制原语**（A3）：chat 模式 `grep` 不可用；开通什么才能调什么。

```rust
let cap = resolve_capability(request.mode);
let sdk_subset = cap.sdk_primitives();      // 沙箱只暴露这些
let skill = load_skill(cap.id());           // < 2000 tok，聚焦组合
```

---

## §4. 单 Agent 流程（A2）

### 砍掉的多层（当前）
```
orchestrator → worker session → brief#1 → codegen → handoff
                              → brief#2 → codegen → handoff
            → synthesize（读 handoff）→ 答案
```

### 单 agent（目标）
```
单 ReAct loop：
  loop:
    LLM 写 Python（组合开通的原语，可 fan-out/并行/条件）
    沙箱批量执行 → 观测（print + filesystem）
    LLM 决定：继续写代码 / 够了出答案
  → 直接出答案（无 handoff、无 synthesize）
```

**多步靠**：① 代码内 fan-out（一段代码查多文档，取代多 brief）② ReAct 多轮（不够再写一轮，取代 handoff+synthesize）③ filesystem 跨轮（save/load，取代 handoff JSON）。

---

## §5. 跨 Turn 状态（A7）

SaC 明确选 filesystem 而非 REPL。沙箱提供 per-session 持久目录：

```python
# Turn 1：检索 + 持久化
hits = await dense("关键差异")
save("cands.json", hits)
# Turn 2：读回 + 继续
cands = load("cands.json")
for c in cands: ...
```

显式 serde，可追溯，不污染 token 上下文。**取代 handoff 全部数据结构**。

---

## §6. 沙箱

- 批量执行（一段代码多操作）需安全沙箱
- 先用现有 `code-interpreter`（已能跑 Python），放开多操作 + 加持久目录
- 若并发/隔离不足，评估开源沙箱（e2b 等）；**SDK 原语层自研（产品差异点），执行环境不造轮子**

---

## §7. 实施（一步到位，不分独立验证阶段）

负责人决策：**起步直接一步到位**。作为一个整体推进，不做 W1→W5 的分阶段独立验证。实施有序，但目标是一个完整的单 agent + 单 SDK 体系。

### 实施任务（整体）
1. **SDK 原语层**（A4/A5/A6）：`bridge.rs` 重写——dense(query)/lexical(query)/grep/read/web/fetch/doc_profile/doc_summary/history/user_profile；删 graph 独立、删 topk 参数、删 native 检索
2. **沙箱**：放开批量执行（dispatch_codegen 单 block → 多操作）+ 持久目录（save/load）
3. **单 agent**（A2）：删 orchestrator/worker/brief/handoff/synthesize；一个 ReAct loop 直贯
4. **capabilities**（A3/A8）：mode → SDK 子集 + < 2000 tok 提示词；沙箱按 mode 限原语
5. **filesystem**（A7）：save/load 取代 handoff
6. **全量 E2E 回归**：149 题，对照 8 个锚点验收

### 验收（对照锚点）
- A1：native 无检索 tool
- A2：单 ReAct loop
- A3：沙箱按 mode 限原语
- A4：dense(query) 无 topk，无聚合原语
- A5：无 graph 独立
- A6：web 在 SDK
- A7：无 handoff，filesystem 跨 turn
- A8：各 SKILL < 2000 tok
- **业务**：149 题 PASS 率 ≥ 现状（135/149），且 q088（表格）/跨文档簇回升

---

## §8. 与交接文档 14 题的关系

| 14 题簇 | 本设计怎么治（锚点）|
|---|---|
| 双文档半载（58/100/101/105/107）| 单 agent 代码 fan-out（A2），无 handoff 截断 |
| 表格去重（88）| agent 信 grep.total_hits 不加工（A4 无 count），SKILL 钉死 |
| handoff 半成品（6/14 过程故障）| 无 handoff（A2/A7）|
| code_gen_error（5/14）| 原语极简（A4）+ < 2000 tok skill（A8），减签名错 |
| 检索错位（17/18/65/86）| 模型可代码编排自定义策略（dense/lexical/grep 组合）|

---

## §9. 未决（实施中定）

- `lexical` 含 graph 的具体机制（后端 lexical 调用是否自动带 graph triplet，还是 graph 独立但 SDK 不暴露为 method）——遵循"现成机制不破坏"，实施时确认后端 lexical 的 graph 行为
- web 进 SDK 后 native `web_search`/`web_fetch` 的删除时机（整体重构时一并删）
- 单 agent 下原 `dense_retrieval` auto_fallback 逻辑的去留（后端默认 topk 机制保留，只是 SDK 不暴露 topk 参数）

---

*8 个锚点是验收红线。实施任何偏离需提请评审。下一步：按 §7 任务清单整体推进。*
