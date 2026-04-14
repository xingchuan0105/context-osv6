# RAG Answer Prompt 优化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 优化 RAG Answer Prompt，通过注入 Retrieval Index 和收紧 [INSUFFICIENT_EVIDENCE] 触发条件，让模型更自信地基于召回的 Chunk 回答问题。

**Architecture:** 在 AnswerSynthesizer 中重构 User Prompt 结构，从平铺的 `Context:` 改为 `Retrieval Index:` + `Context:` 的分层结构。同时更新 System Prompt 指令，明确模型在有 Chunk 召回时必须合成答案。

**Tech Stack:** Rust (avrag-rs), LLM Client, RAG Core Runtime.

---

## 任务 1: 重构 `crates/llm/src/synthesizer.rs`

**Files:**
- Modify: `crates/llm/src/synthesizer.rs`

- [ ] **Step 1: 更新 System Prompt**

修改 `SYNTHESIZER_SYSTEM_PROMPT`：
- 在 Guidelines 中加入：`Interpret the Retrieval Index...`
- 修改 `[INSUFFICIENT_EVIDENCE]` 触发条件：`Only respond with [INSUFFICIENT_EVIDENCE] if zero chunks were recalled...`

```rust
const SYNTHESIZER_SYSTEM_PROMPT: &str = r#"You are a helpful assistant that answers questions based on the provided context chunks.

Guidelines:
- Use the provided context chunks to answer the user's question
- **Interpret the Retrieval Index to understand the retrieval strategy, keywords/rewrites, and chunk attribution. Use this context to build your answer strategy.**
- Cite relevant information from the context using the chunk_id reference if available
- Be concise but comprehensive
- Format your response clearly
- **CRITICAL**: Only respond with [INSUFFICIENT_EVIDENCE] if **zero chunks were recalled**. If even one relevant chunk exists, synthesize an answer based on it.
"#;
```

- [ ] **Step 2: 修改 `synthesize` 函数签名**

由于需要生成 Index，必须接收 `RagPlan` 和 `item_traces` (包含 source_ids)。

首先，检查 `RagPlan` 和 `RagTraceItem` 是否已在 `avrag_llm` 的依赖中引入。如果 `synthesizer.rs` 目前是独立的，需要引入 common crate。

```rust
// 假设需要引入 common crate 或者直接内联定义一个轻量级的 Index 结构
// 为了简化，我们在内层构建字符串，而不是传递完整的结构体依赖

pub async fn synthesize(
    &self,
    query: &str,
    context_chunks: &[(String, String)], // (chunk_id, content)
    rag_plan: &Option<common::RagPlan>, // 用于生成 Index
    item_traces: &[common::RagTraceItem], // 用于获取 Recall 归属
    history: Option<&[ChatMessage]>,
) -> anyhow::Result<String>
```

**注意**：需要确认 `common` crate 是否在 `avrag_llm` 的 `Cargo.toml` 中作为依赖。如果是新引入，注意不要产生循环依赖（通常 `rag-core` 依赖 `llm`，所以 `llm` 不应依赖 `rag-core`，但 `llm` 依赖 `common` 是安全的）。

如果担心循环依赖，可以只传递必要的字段（Vec<String> 列表），但设计文档推荐传递完整 `RagPlan` 以保留扩展性。先尝试引入 `common`，如果遇到编译问题再调整。

- [ ] **Step 3: 实现 Index 生成逻辑**

在 `synthesize` 函数内部，添加 Index 生成的辅助函数 `build_retrieval_index`。

```rust
fn build_retrieval_index(
    rag_plan: &Option<RagPlan>,
    item_traces: &[RagTraceItem],
) -> String {
    // 如果没有 plan 或 traces，返回空或占位符
    // 遍历 items，为每个 item 生成：
    // [Retrieval Intent #N]
    //   Type: item_type
    //   Retrieval Mode: retrieval_mode
    //   Query: effective_query (如果 query 为空，用 bm25_terms 拼接)
    //   Recall: [source_ids from trace]
}
```

- [ ] **Step 4: 重写 User Prompt 拼接逻辑**

修改 `messages.push(ChatMessage::user(...))` 部分：

```rust
let index_section = build_retrieval_index(rag_plan, item_traces);

let context_section = if context_chunks.is_empty() {
    "No relevant context provided.".to_string()
} else {
    context_chunks
        .iter()
        .map(|(chunk_id, content)| format!("[{}]\n{}\n", chunk_id, content))
        .collect::<Vec<_>>()
        .join("\n")
};

messages.push(ChatMessage::user(format!(
    "Retrieval Index:\n{}\n\nContext:\n{}\n\nQuestion:\n{}",
    index_section, context_section, query
)));
```

- [ ] **Step 5: 编译检查**

运行 `cargo check -p avrag-llm`。
如果出现循环依赖，执行 Step 2.5 (Fallback 方案)。

---

## 任务 2: 适配 `crates/rag-core/src/runtime.rs`

**Files:**
- Modify: `crates/rag-core/src/runtime.rs`

- [ ] **Step 1: 更新 `synthesizer.synthesize` 调用**

在 `execute` 函数中，找到调用 `synthesizer.synthesize` 的位置（大约在 Line 397）。

原调用：
```rust
match synthesizer.synthesize(query, &context_chunks, None).await
```

修改为：
```rust
match synthesizer.synthesize(
    query,
    &context_chunks,
    &Some(rag_plan.clone()),
    &item_trace,
    None
).await
```

**注意**：`item_trace` 变量在前面已经构建好（Line 192-267），可以直接使用。`rag_plan` 也在 Line 113-128 处理过。

- [ ] **Step 2: 编译检查**

运行 `cargo check -p avrag-core` 确保调用匹配。

---

## 任务 3: 测试与验证

**Files:**
- Create/Modify: `crates/llm/src/synthesizer.rs` (tests module)

- [ ] **Step 1: 编写合成器单元测试**

在 `synthesizer.rs` 末尾添加测试模块 `#[cfg(test)]`。
测试目标：
1. 验证 Index 生成格式正确（包含 Type, Retrieval Mode, Query, Recall）。
2. 验证多个 Item（并联）时 Index 正确区分。
3. 验证没有 Plan 时 Index 的 Fallback 行为（建议为空或 "N/A"）。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_retrieval_index_single_item() {
        // 构造 RagPlanItem 和 RagTraceItem
        // 调用 build_retrieval_index
        // assert contains "Retrieval Intent #1"
        // assert contains "Type: primary"
        // assert contains "Recall: [chunk_1, chunk_2]"
    }

    #[test]
    fn test_build_retrieval_index_empty() {
        // 传入 None, []
        // assert result.is_empty()
    }
}
```

- [ ] **Step 2: E2E 验证 (Manual/Script)**

由于 RAG 涉及 Embedding 和 LLM 调用，需要一个集成测试脚本。
建议在 `scripts/` 目录添加一个 `test-prompt-optimization.sh`，模拟发送一个会触发多个 Retrieval Items 的查询（如包含关键词和改写意图）。

---

## 任务 4: Fallback 与边界处理

- [ ] **Step 1: Planner 不可用时的 Index 生成**

在 `runtime.rs` Line 114-127 中，如果 `planner.plan` 失败，`rag_plan` 会保留 `default_rag_plan`。这种情况下 `RagPlan` 存在，但可能是默认的。
Index 生成函数 `build_retrieval_index` 应能处理这种情况：如果 `items` 为空或只有一个默认的 `primary` item，`Recall` 列表应正确反映实际召回的 `source_ids`。

**关键逻辑**：Index 的 `Recall` 部分应该来自 `item_traces`（实际召回结果），而不是 `rag_plan.items` 中的预估值。

---

## 验收标准

1. `cargo check --all` 通过。
2. `cargo test -p avrag-llm` 通过。
3. 生成的 Prompt 结构符合设计：`Retrieval Index` 在 `Context` 之前。
4. `[INSUFFICIENT_EVIDENCE]` 只在 `top_chunks.is_empty()` (Line 270) 时才应该被考虑（但由于 System Prompt 约束，实际触发取决于 LLM 行为，测试主要验证条件被传达）。

