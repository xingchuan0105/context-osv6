# Runtime 工具分发架构：从 monolithic plan 到 tool-call 范式

> 状态：2026-05-09 草案。待审阅后进入实施阶段。
> 本文档承接 2026-04-26-current-product-rag-architecture.md 的 RAG API 层设计，解决当前 `ExecutePlanRequest` monolithic schema 的扩展性与维护性问题。

## 1. 核心结论

将 RAG API 从**单一 monolithic plan schema** (`ExecutePlanRequest`) 重构为**工具目录 + 分发执行**模式：

- **工具目录（Tool Catalog）**：6 个检索/阅读工具各自独立定义输入输出 schema，由 planner 按需组合调用。
- **双端点 API**：
  - `POST /v1/runtime/execute` —— 面向外部 agent，返回原始 tool 结果（可选预合并）。
  - `POST /v1/chat/answer` —— 面向本地端用户，内部三阶段流水线（planner → runtime → synthesizer）。
- **Planner 只做一件事**：根据用户问题和工具目录，输出 `Vec<ToolCall>`。
- **Runtime 只做一件事**：校验 schema、并行分发、收集 `Vec<ToolResult>`。
- **Synthesizer 只做一件事**：读取带标签的多组证据，生成最终答案。

```text
外部 agent 调用路径
  POST /v1/runtime/execute
    → 外部 agent 自己就是 planner，发送 ToolCall[] + 可选 merge 指令
    → runtime dispatcher 并行执行各 tool pipeline
    → 返回 ToolResult[]（或按 merge 指令预合并）
    → 外部 agent 自行合成答案

本地端调用路径
  POST /v1/chat/answer
    → 内部 planner（LLM）读取用户问题 + 工具目录 → 输出 ToolCall[]
    → runtime dispatcher 并行执行各 tool pipeline
    → 返回 ToolResult[]（不合并，分组保留来源标签）
    → synthesizer（LLM）读取多组证据 → 生成答案 + 引用
```

## 2. 为什么不是 monolithic plan 了

当前 `ExecutePlanRequest` 把所有检索信号塞进一个 JSON 对象：

```json
{
  "plan_version": "rag-execute-v1",
  "doc_scope": [...],
  "items": [...],
  "bm25_terms": [...],
  "graph_hints": [...],
  "placeholder_triplets": [...],
  "summary_mode": "...",
  "query_entities": [...]
}
```

问题：

1. **扩展成本高**：增加新的检索维度（如 TOC 直取、web search）= 改 schema + 改 validator + 改 runtime 解析 + 改 planner prompt。
2. **Planner 被迫假装全能**：每个 plan 必须适配完整 schema，即使只用 1 个字段，其余留空。
3. **外部集成困难**：外部 agent 想调用 BM25 单工具，必须理解和构造整个 `ExecutePlanRequest`。
4. **融合逻辑被藏起来**：当前 schema 里没有显式的融合指令，runtime 内部的融合策略是硬编码的——外部 agent 无法干预。

## 3. 工具目录（Tool Catalog v1）

每个工具独立定义名称、描述、输入 schema、输出 shape、适用场景、成本模型。

| # | 工具名 | 职责 | 适用场景 |
|---|--------|------|----------|
| 1 | `dense_retrieval` | 向量检索（文本 + 多模态融合） | 语义型问题、跨段落综合、概念查找 |
| 2 | `lexical_retrieval` | BM25 精确字面检索 | 型号/ID/代码/版本号/专名等字面命中 |
| 3 | `graph_retrieval` | 三元组/关系检索 | 关系/比较/归属/多跳推理 |
| | | **后端状态**：ingestion 管线（`worker/src/main.rs:2463` 提取三元组 → `build_graph_index_records` → Milvus `kg_entities`/`kg_relations`/`graph_passages`）+ runtime 查询（`rag-core/src/runtime/execute.rs:612` `search_graph`）**已全部就绪** | |
| 4 | `index_lookup` | TOC → chunk_id 直取 | 章节定位类问题（"第三章 2.1 节"） |
| 5 | `doc_summary` | 读取预生成摘要 | 文档概述、"讲了什么"类问题 |
| 6 | `doc_metadata` | 读取文档元信息（标题/作者/章节大纲） | 元信息追问、planner 自身在多轮中先了解文档结构 |

### 输入/输出契约（示意）

```rust
// 每个工具独立版本号
pub struct ToolSpec {
    pub name: &'static str,
    pub version: &'static str,        // e.g., "1.0"
    pub description: &'static str,
    pub input_schema: &'static str,   // JSON Schema
    pub output_schema: &'static str,  // JSON Schema
}

// Planner 输出
pub struct ToolCall {
    pub tool: String,        // e.g., "dense_retrieval"
    pub version: String,     // e.g., "1.0"
    pub args: serde_json::Value,
}

// Runtime 返回（每个 ToolCall 对应一个 ToolResult）
pub struct ToolResult {
    pub tool: String,
    pub version: String,
    pub status: ToolStatus,  // ok | timeout | error | not_found
    pub data: serde_json::Value,
    pub trace: Option<ToolTrace>,  // 耗时、命中数、backend 等
}
```

## 4. 架构图

```text
┌─────────────────────────────────────────────────────────────────────┐
│  Tool Catalog (prompt 注入 + API 文档同源)                            │
│  - 6 个 ToolSpec，各自 name / version / description / schema         │
│  - 新增工具 = 新增一条目，旧工具不动                                   │
└─────────────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐
  │ 外部 agent    │    │ 内部 planner │    │ 内部 synthesizer  │
  │ (自带 answer  │    │ (LLM)        │    │ (LLM)            │
  │  LLM)         │    │              │    │                  │
  └──────┬───────┘    └──────┬───────┘    └──────────────────┘
         │                   │                    ▲
         │  ToolCall[]       │  ToolCall[]        │ ToolResult[]
         │  + optional merge │  (next_step 留口子)│ (不合并，分组)
         │                   │                    │
         └─────────┬─────────┴────────────────────┘
                   ▼
         ┌─────────────────────┐
         │  Runtime Dispatcher  │
         │  - schema 校验        │
         │  - 并行执行各 tool    │
         │  - 收集 ToolResult[]  │
         │  - optional merge    │
         └─────────────────────┘
                   │
        ┌──────────┼──────────┬──────────┐
        ▼          ▼          ▼          ▼
   dense_pipe  bm25_pipe  graph_pipe  index_pipe
   summary_pipe meta_pipe  ...         ...
        │          │          │          │
        └──────────┴──────┬───┴──────────┘
                          ▼
                   ToolResult[]
```

## 5. 四大架构决策

### 决策 1：谁合并证据（外部 A / 本地 C）

| 端点 | 策略 | 理由 |
|------|------|------|
| 外部 `runtime/execute` | **A**：客户在请求中显式指定 `merge` 策略和权重；也可以不指定，返回原始分组 | 外部 agent 自带 answer LLM，只有自己知道下游模型偏好什么形态的证据 |
| 本地 `chat/answer` | **C**：不预合并，`Vec<ToolResult>` 按来源标签原样上传 synthesizer | synthesizer 是唯一同时拥有"用户问题 + 所有证据 + 答案语境"的角色，它最有资格判断每份证据的分量 |

`merge` 字段是请求中的可选字段：

```json
{
  "calls": [...],
  "merge": {
    "strategy": "rrf",
    "weights": { "dense_retrieval": 2.0, "bm25": 1.0 }
  }
}
```

缺省 `merge` 时，runtime 原样返回 `ToolResult[]`。

### 决策 2：单轮 vs 多轮 agentic loop（本地端）

| | 策略 | 理由 |
|---|---|---|
| 外部端点 | **不在 runtime 内部实现多轮** | 外部 agent 自己重复调用 `runtime/execute` 就是多轮。runtime 保持无状态单次执行。 |
| 本地端 | **第一版单轮，schema 预留 `next_step` 口子** | 绝大多数本地问题单轮 1–3 个 tool 足够；多轮延迟不可预测、成本隐性翻倍、调试困难。`next_step: "answer" \| "replan"` 字段从第一天就存在，未来灰度开启。 |

Planner 输出 schema：

```json
{
  "calls": [...],
  "next_step": "answer"
}
```

第一版 `next_step` 永远为 `"answer"`（单轮）。后续复杂问题场景可开启 `"replan"`。

### 决策 3：工具输出 schema 版本契约

**策略**：每个 tool 的输入输出 schema 各自版本化；改格式时发新版（`dense_retrieval` v1 → v2），旧版并行保留一个 sunset 周期。

**理由**：外部 agent 的集成代码直接消费 `ToolResult` 的字段名和结构。字段改名 = 外部系统崩溃。这是公共契约，不是内部实现细节。

```text
dense_retrieval v1.0
  output: { results: [{ chunk_id, doc_id, text, score, page }] }

dense_retrieval v1.1  (向后兼容)
  output: { results: [{ chunk_id, doc_id, text, score, page, section_path? }] }

dense_retrieval v2.0  (不兼容，需外部 agent 显式升级)
  output: { hits: [{ id, content, relevance, metadata }] }
```

外部 agent 调用时显式声明 `"version": "1.0"`，缺省指向 latest。

### 决策 4：工具失败/超时语义

**策略**：Soft fail。

每个 `ToolResult` 带有 `status`：

```json
{
  "tool": "bm25",
  "version": "1.0",
  "status": "timeout",
  "data": null,
  "trace": { "elapsed_ms": 5000, "error": "upstream timeout" }
}
```

runtime 不阻断流程。失败的 tool 和成功的 tool 一起进入 `Vec<ToolResult>`。

- 外部 agent 看到 `status: "timeout"`，自行决定重试或跳过。
- 本地 synthesizer 看到 `status: "timeout"`，在答案中自然消化（如不提该来源，或提示"BM25 检索本次不可用"）。

**不采用 hard fail**：整单取消只因为一个 tool 挂掉，对用户体验和容错都是灾难。

## 6. 与现有系统的衔接

### 6.1 当前 `ExecutePlanRequest` 的映射

现有 monolithic schema 可以完整映射到新工具目录：

| 旧字段 | 映射到 | 说明 |
|--------|--------|------|
| `items[].query` | `dense_retrieval` tool call | 语义检索查询 |
| `bm25_terms` | `lexical_retrieval` tool call | BM25 字面检索 |
| `graph_hints` + `placeholder_triplets` | `graph_retrieval` tool call | 关系检索 |
| `summary_mode` | `doc_summary` tool call | 摘要读取 |
| `query_entities` | 可作为 `dense_retrieval` 的辅助信号 | 实体列表注入 query context |
| `doc_scope` | 跨所有 tool 的全局过滤 | 作为 runtime dispatcher 的输入参数，而非某个 tool 的专属参数 |

### 6.2 迁移路径（四阶段）

**Phase 1：类型与适配层（2–3 天）**
- 新增 `common/src/tool_call.rs`：定义 `ToolCall`、`ToolResult`、`ToolSpec`、`ToolStatus`、`ToolTrace` 类型
- 写 `ExecutePlanRequest::from_tool_calls` 适配器：把 `Vec<ToolCall>` 编译回现有 `ExecutePlanRequest`，复用现有 runtime
- 当前生产代码不变，只有类型层新增

**Phase 2：Planner prompt 重写（2–3 天）**
- 重写 `rag_plan_system.txt`：从 monolithic schema 改为 tool catalog 格式
- Planner 输出 `ToolCall[]` + `next_step`
- 内部 planner 调用适配器转换为 `ExecutePlanRequest`，复用现有 runtime

**Phase 3：Runtime 拆分为 tool pipeline（1 周）**
- 把现有 `ExecutePlanRequest` 处理逻辑按 tool 切片：每个 tool 一个独立 pipeline
- Runtime dispatcher 替换为 schema 校验 + 并行分发 + 收集
- 去掉适配器层，planner 直接对接新 runtime
- Synthesizer 升级：接收 `Vec<ToolResult>` 而非 `Vec<ScoredChunk>`

**Phase 4：TOC 索引与 `index_lookup` 工具（3–4 天）**
- 摄入完成后异步生成文档 TOC（章节结构 + chunk_id 绑定）
- 新增 `index_lookup` tool pipeline：SQL 按 chunk_ids 直取
- 这是 C 方案（章节定位精读）的最终落地点

**Phase 5：外部 API 端点开放（2–3 天）**
- 新增 `POST /v1/runtime/execute` HTTP handler
- 支持可选 `merge` 字段
- 版本化 API 文档

## 7. `index_lookup` 与 TOC 索引（C 方案落地点）

在当前架构下，C 方案不再是独立方案，而是工具目录中的 `index_lookup` 工具。

### 7.1 流程

1. **摄入完成后**：异步触发一次 TOC 生成任务。输入为文档的 `document_blocks`（按 page + paragraph_index 排序），由 LLM 输出章节结构并绑定到 `chunk_id`。
2. **TOC 存储**：与 `parse_run_id` 关联的版本化表。文档重摄入时自动重建。
3. **运行时**：
   - Planner 先调用 `doc_metadata` 读取 TOC → 了解章节结构
   - Planner 再调用 `index_lookup`，传入 `{ doc_id, chunk_ids: [c12, c13, ...] }`
   - Runtime 直接 SQL fetch，跳过向量检索和 rerank

### 7.2 TOC 生成的输入策略

推荐**chunk 头摘要聚类**（非全文直喂，非 map-reduce）：

- 把每个 chunk 的前 200 字符 + chunk_id 列表化
- 喂给 LLM，输出：
  ```json
  {
    "sections": [
      { "title": "第一章 概述", "level": 1, "chunk_ids": ["c1","c2","c3"] },
      { "title": "1.1 背景", "level": 2, "chunk_ids": ["c4","c5"] }
    ]
  }
  ```
- 输入压缩 10–100 倍，避免长文档超出 LLM 上下文

## 8. 文件清单（新增/修改）

| 新增/修改 | 路径 | 说明 |
|-----------|------|------|
| 新增 | `crates/common/src/tool_call.rs` | ToolCall / ToolResult / ToolSpec / ToolStatus 类型定义 |
| 修改 | `crates/llm/src/planner.rs` | 适配新 prompt 格式，输出 ToolCall[] |
| 修改 | `prompts/rag_plan_system.txt` | 重写为 tool catalog 格式 |
| 新增 | `crates/rag-core/src/runtime/dispatcher.rs` | Runtime dispatcher（schema 校验 + 并行分发 + 收集） |
| 新增 | `crates/rag-core/src/runtime/tools/` | 6 个 tool pipeline 各自独立模块 |
| 修改 | `crates/llm/src/synthesizer.rs` | 接收 Vec<ToolResult>，按来源标签分组展示 |
| 新增 | `migrations/0030_document_toc.up.sql` | TOC 表结构 |
| 新增 | `bins/worker/src/toc_generation.rs` | TOC 生成 worker 任务 |
| 新增 | `crates/app/src/runtime_api.rs` | `POST /v1/runtime/execute` handler |
| 本文档 | `docs/superpowers/specs/2026-05-09-runtime-tool-dispatch-architecture.md` | 本 spec |

---

> 审阅 checklist：
> - [ ] 工具目录是否覆盖了所有当前 RAG 能力？
> - [ ] `index_lookup` 的 TOC 生成策略（chunk 头聚类）是否被 ingestion 管线支持？
> - [ ] 外部 `runtime/execute` 的认证/鉴权/计费模型是否与 `chat/answer` 区分？
> - [x] `graph_retrieval` 后端已确认就绪：ingestion 三元组提取（`worker/src/main.rs:2463`）→ Milvus collection（`storage-milvus/src/lib.rs`）→ runtime `search_graph` 调用（`rag-core/src/runtime/execute.rs:612`），全链路已通。
