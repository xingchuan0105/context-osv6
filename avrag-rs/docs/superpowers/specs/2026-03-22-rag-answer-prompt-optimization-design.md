# RAG Answer Prompt 优化设计

## 1. 背景与目标

**当前问题**：
RAG Answer 偏保守。根因在于 System Prompt 中的硬限制：
> If the provided context does not contain sufficient information to answer the question, you MUST respond with EXACTLY: [INSUFFICIENT_EVIDENCE]...

此限制写得过于模糊，模型会倾向于在“证据不够完美”时也触发该指令，导致回答过于保守或直接放弃。

**优化目标**：
1. 收紧 `[INSUFFICIENT_EVIDENCE]` 的触发条件：**必须是没有召回任何 Chunk**，而非“证据不够充分”。
2. 注入检索上下文（Retrieval Index），让 LLM 理解每个 Chunk 是怎么被找出来的（意图、策略、关键词），从而更自信地组织答案。

---

## 2. 核心变更

### 2.1 System Prompt 调整

**修改前**：
```markdown
You are a helpful assistant that answers questions based on the provided context chunks.

Guidelines:
- Use the provided context chunks to answer the user's question
- If the provided context does not contain sufficient information to answer the question, you MUST respond with EXACTLY: [INSUFFICIENT_EVIDENCE] followed by what specific information is missing
- Cite relevant information from the context using the chunk_id reference if available
- Be concise but comprehensive
- Format your response clearly
```

**修改后**：
```markdown
You are a helpful assistant that answers questions based on the provided context chunks.

Guidelines:
- Use the provided context chunks to answer the user's question
- **Interpret the Retrieval Index to understand the retrieval strategy, keywords/rewrites, and chunk attribution. Use this context to build your answer strategy.**
- Cite relevant information from the context using the chunk_id reference if available
- Be concise but comprehensive
- Format your response clearly
- **CRITICAL**: Only respond with [INSUFFICIENT_EVIDENCE] if **zero chunks were recalled**. If even one relevant chunk exists, synthesize an answer based on it and state your confidence level if necessary.
```

### 2.2 User Prompt 结构重设计

将原来的平铺结构改为带 Index Header 的分层结构。

**修改后 User Prompt 模板**：
```markdown
Retrieval Index:
[Retrieval Intent #1]
  Type: primary
  Retrieval Mode: hybrid
  Query: 用户原始问题或改写
  Recall: [chunk_id_1, chunk_id_2, ...]

[Retrieval Intent #2]
  Type: keyword
  Retrieval Mode: sparse
  Query: bm25_term_1, bm25_term_2
  Recall: [chunk_id_3]

...

Context:
[chunk_id_1]
chunk content 1

[chunk_id_2]
chunk content 2

...

Question:
用户问题
```

### 2.3 Index 数据映射规则

数据来源为 `RagPlan.items`（并行执行结果）。

| RagPlanItem 字段 | Index 对应项 | 说明 |
| :--- | :--- | :--- |
| `item_type` | `Type` | primary / keyword / rewrite / summary / metadata |
| `retrieval_mode` | `Retrieval Mode` | dense / sparse / hybrid / summary_only / metadata_only |
| `effective_query` | `Query` | 如果 `query` 非空则用它，否则拼接 `bm25_terms` |
| `source_ids` (RagTraceItem) | `Recall` | 该 Item 召回的 chunk_id 列表 |

**多策略归属**：
由于 Item 之间是并联的，每个 Item 的 `Recall` 列表是独立的。LLM 通过 Index 可以清晰地看到“语义检索找到了 chunk_1, chunk_2；关键词检索找到了 chunk_3”。

---

## 3. 代码修改点

### 3.1 `crates/llm/src/synthesizer.rs`

1. 更新 `SYNTHESIZER_SYSTEM_PROMPT`，加入索引解读指令。
2. 修改 `synthesize` 函数签名，接收 `RagPlan` 和 `item_results`（含 chunk_id 归属）。
3. 重写 `context_section` 的拼接逻辑，生成 `Retrieval Index` Header。

**新签名草案**：
```rust
pub async fn synthesize(
    &self,
    query: &str,
    context_chunks: &[(String, String)], // (chunk_id, content)
    rag_plan: &Option<RagPlan>,          // 新增：用于生成 Index
    item_traces: &[RagTraceItem],        // 新增：用于生成 Recall 归属
    history: Option<&[ChatMessage]>,
) -> anyhow::Result<String>
```

### 3.2 `crates/rag-core/src/runtime.rs`

1. 修改 `execute` 函数中调用 `synthesizer.synthesize` 的部分，传入新的 `rag_plan` 和 `item_trace` 参数。
2. 清理不再使用的 `compose_evidence_summary_answer` 和硬编码的中文 fallback（如果该逻辑已被 Prompt 吸收）。

---

## 4. 效果预期

- **保守度下降**：LLM 不会因为“证据不够完美”而放弃回答。
- **答案质量提升**：LLM 能理解 Chunk 的召回路径，可以更好地判断 Chunk 与问题的关联性强弱。
- **可观测性增强**：Answer 的推理过程（Index 部分）清晰可见，便于后期人工审查和 Prompt 调优。

---

## 5. 待验证

- **Index Token 消耗**：需监控带 Index 的 Prompt 是否会超过 LLM 的上下文限制。
- **LLM 遵循度**：验证更新后的 Prompt 是否能让 LLM 正确解读 Index 并据此组织答案。
- **Fallback 逻辑**：检查当 `item_results` 全为空或 `rag_plan` 缺失时（如 Planner 不可用），Index 是否能优雅降级为 “No Index”。
