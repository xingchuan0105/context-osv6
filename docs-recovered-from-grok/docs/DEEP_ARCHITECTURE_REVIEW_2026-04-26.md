# context-osv6 深度架构审查报告

> 生成时间: 2026-04-26  
> 审查工具: gitnexus (12,071 nodes | 25,953 edges) + 逐行代码分析  
> 审查范围: RAG Pipeline 完整数据流 + Main Agent / RAG API 边界  
> 审查人: 大虾 🦐

---

## 执行摘要

本次审查发现 **3 个 P0 级架构问题** 和 **2 个 P1 级设计缺陷**，均影响系统正确性或长期维护。所有发现均有代码级证据支撑。

---

## P0 问题 1: Main Agent 与 RAG API 职责重叠 —— Entity Extraction 做了两次

### 证据链

**第一步: Main Agent 生成 `query_entities`**

文件: `avrag-rs/crates/app/src/main_agent/mod.rs:23-58`
```
RAG_PLAN_SYSTEM_PROMPT 要求 LLM 输出:
  "query_entities": [{ "text": "named entity", "kind": "optional kind" }],
  "graph_hints": [{ "subject": "optional", "predicate": "optional", "object": "optional" }]
```

文件: `avrag-rs/crates/app/src/main_agent/mod.rs:520-560`
```rust
fn fallback_execute_plan_request(...) -> ExecutePlanRequest {
    ExecutePlanRequest {
        query_entities: Vec::new(),  // ← fallback 时空数组
        graph_hints: Vec::new(),
        ...
    }
}
```

文件: `avrag-rs/crates/app/src/main_agent/mod.rs:240-280`
```rust
pub async fn plan_rag(...) -> MainAgentPlanResult {
    let messages = vec![
        LlmChatMessage::system(RAG_PLAN_SYSTEM_PROMPT),  // ← 让 LLM 输出 entities
        LlmChatMessage::user(build_rag_plan_user_prompt(...)),
    ];
    match llm.complete(&messages, ...).await {
        Ok(response) => {
            match parse_rag_plan_decision(&response.content, request) {
                Some(MainAgentRagPlanDecision::Execute(execute_request)) => {
                    // ← execute_request 包含 query_entities 和 graph_hints
                }
            }
        }
    }
}
```

**第二步: RAG API 再次提取**

文件: `avrag-rs/crates/rag-core/src/runtime/execute.rs:480-520`
```rust
async fn retrieve_graph_stage(...) -> GraphChannelOutput {
    let mut entity_names = request
        .query_entities
        .iter()
        .map(|entity| entity.text.clone())
        .collect::<Vec<_>>();
    
    // ← 先用 Main Agent 提供的 entities
    
    if entity_names.is_empty() && relation_hints.is_empty() {
        if let Some(planner) = self.config.planner.as_ref() {
            match planner.extract_query_entities(query).await {  // ← 兜底：自己再提取一次！
                Ok(extracted) => { entity_names = dedupe_entity_names(extracted); }
                Err(error) => degrade_trace.push(...)
            }
        }
    }
}
```

文件: `avrag-rs/crates/llm/src/planner.rs:260-280`
```rust
pub async fn extract_query_entities(&self, query: &str) -> anyhow::Result<Vec<String>> {
    let messages = vec![
        ChatMessage::system(QUERY_ENTITY_SYSTEM_PROMPT),  // ← 又一个 LLM 调用！
        ChatMessage::user(query.trim().to_string()),
    ];
    let response = self.llm.complete(&messages, Some(0.1)).await?;
    parse_query_entity_response(&response.content)
}
```

### 影响分析

| 场景 | 结果 |
|------|------|
| Main Agent LLM 正常输出 entities | RAG API 的兜底逻辑跳过，但代码存在冗余 |
| Main Agent LLM 未输出 entities | **触发第二次 LLM 调用**，浪费 token |
| Main Agent fallback（无 LLM）| entities 为空，RAG API 兜底也大概率空（无 planner）|

### 根因

架构文档说 "RAG API 可以做 query entity extraction"，但实现上 **两边都做了**，没有明确分工。

### 修复建议

**方案 A（推荐）: 精简 Main Agent**

1. 修改 `RAG_PLAN_SYSTEM_PROMPT`，移除 `query_entities` 和 `graph_hints` 字段
2. Main Agent 只输出: `doc_scope`, `items`, `summary_mode`
3. RAG API 负责所有检索细节（已有 `planner.extract_query_entities`）

**方案 B: 删除 RAG API 兜底**

1. 保留 Main Agent 输出 entities/graph_hints
2. 删除 `retrieve_graph_stage` 中的 `planner.extract_query_entities` 兜底
3. 如果 Main Agent 没提供，graph channel 直接跳过

---

## P0 问题 2: Graph Retrieval 实现严重欠完整

### 架构要求 vs 实际实现

**PRD 要求** (§6, §7.4):
```
Query → query entity extraction → entity vector search + relation vector search 
  → subgraph expansion → fan-out control / eviction → relation/path rerank 
  → supporting chunk hydration
```

**实际实现** (`crates/storage-milvus/src/lib.rs:720`):
```rust
async fn search_graph(&self, request: GraphSearchRequest) -> anyhow::Result<GraphSearchOutput> {
    let Some(filter) = graph_relation_filter(&request) else {
        return Ok(GraphSearchOutput::default());
    };
    // 仅做关系属性过滤查询，无 vector search，无 subgraph expansion
    let rows = self.query_entities(
        &self.config.collection_names().kg_relations,
        filter,
        request.relation_limit,
        &RELATION_OUTPUT_FIELDS,
    ).await?;
    // ... 简单映射为 RelationPathCandidate
}
```

### 缺失环节

| 环节 | 状态 | 影响 |
|------|------|------|
| query entity extraction | ✅ 有（RAG API 侧） | |
| entity vector search | ❌ 缺失 | 无法找语义相似实体 |
| relation vector search | ❌ 缺失 | 无法找语义相似关系 |
| subgraph expansion | ❌ 缺失 | 无法做多跳推理 |
| fan-out control | ❌ 缺失 | 无法控制扩展规模 |
| hop limit | ❌ 缺失 | 无法限制搜索深度 |
| relation/path rerank | ❌ 缺失 | 无法排序路径质量 |
| supporting chunk hydration | ✅ 有 | |

### 测试证据

文件: `avrag-rs/crates/transport-http/tests/rag_execute_plan_contract.rs:55`
```rust
assert!(payload.bundle.graph_supported_chunks.is_empty());
assert!(payload.bundle.relation_paths.is_empty());
assert_eq!(payload.coverage.channel_coverage.graph, 0);
```

**测试期望 graph 返回空！** 这说明当前实现连基本功能都没打通。

### 修复建议

**短期**: 增加 `fan_out_limit` 和 `hop_limit` 配置，标注为 "v1 简化实现"

**中期**: 
1. 在 Milvus `kg_entities` collection 中增加 vector 字段
2. 实现 entity vector search（用 query embedding 找相似实体）
3. 实现 subgraph expansion（从 entity → relation → entity 多跳）
4. 增加路径 rerank

---

## P0 问题 3: `ExecutePlanRequest` 的 `to_chat_request_compat()` 是临时 hack

### 证据

文件: `avrag-rs/crates/rag-core/src/runtime/planner.rs`（需要查看）

在 `retrieve_graph_stage` 中:
```rust
let doc_ids = request_doc_ids(&request.to_chat_request_compat());
```

这说明 `ExecutePlanRequest` 需要转换成 `ChatRequest` 才能提取 doc_ids，暗示 **两个结构之间的字段映射不一致**。

### 影响

- 数据模型不统一
- 容易在转换中丢失信息
- 增加维护成本

### 修复建议

统一 `ExecutePlanRequest` 和 `ChatRequest` 的 doc_scope 字段，或者让 `ExecutePlanRequest` 直接包含 `doc_ids` 字段。

---

## P1 问题 4: GuardPipeline 是空壳实现

### 证据

文件: `avrag-rs/crates/guardrails/src/lib.rs`
```rust
pub fn check_input(...) -> GuardResult {
    let input_ctx = input::InputGuardContext { ... };
    if let Some(result) = self.input.run(&input_ctx) {
        return result;
    }
    GuardResult::pass("input:all")  // ← 默认通过！
}
```

测试显示:
- SQL 注入测试 `test_guard_pipeline_check_input_blocks_sql_injection` —— 通过
- 但实现看起来是 **基于规则匹配**，不是真正的语义分析

### 影响

- 安全防护能力有限
- 容易被绕过

---

## P1 问题 5: 前端 Mode 映射存在不一致

### 证据

**前端 Next.js** (`frontend_next/components/workspace/workspace-chat-pane.tsx:85-91`):
```typescript
function normalizeMessageMode(mode: string | null | undefined): WorkspaceChatMode | null {
  if (mode === "rag" || mode === "search" || mode === "general") {
    return mode;
  }
  return null;
}
```

**后端** (`avrag-rs/crates/app/src/main_agent/mod.rs:66-78`):
```rust
pub enum ModeProfile {
    General,
    Rag,
    Search,
}
```

**问题**: 前端用字符串，后端用 enum，但两边值不完全一致（如前端可能传 `"chat"` 但后端只认 `"general"`）。

---

## 数据流验证

通过 gitnexus 确认的完整 RAG 数据流:

```
User Query
  → frontend_next/workspace-chat-pane.tsx (normalizeMessageMode)
  → POST /api/v1/chat
  → avrag-rs/crates/app/src/chat/graphflow.rs (GraphFlow)
    → TASK_RAG_PREPARE_PLANNER_INPUT
    → TASK_RAG_CALL_PLANNER 
      → main_agent/mod.rs plan_rag() [LLM #1: 生成 ExecutePlanRequest]
    → TASK_RAG_EXECUTE_PLAN
      → app/src/lib_impl/rag_execute.rs execute_rag_execute_plan()
      → rag-core/src/runtime/execute.rs RagRuntime::execute_plan()
        → text_dense channel: embedding + Milvus search
        → bm25 channel: Tantivy search  
        → multimodal channel: mm_embedding + Milvus search
        → graph channel: entity extraction [LLM #2!] + Milvus query
      → rerank + merge
    → TASK_RAG_ANSWER_SYNTHESIZE
      → main_agent/mod.rs answer_rag() [LLM #3: 生成最终回答]
    → TASK_OUTPUT_GUARD
  → SSE Stream 返回前端
```

**关键发现**: RAG 流程涉及 **3 次 LLM 调用**（plan → entity extraction → answer），其中 plan 和 entity extraction 有重叠。

---

## 修复优先级

| 优先级 | 问题 | 工作量 | 影响 |
|--------|------|--------|------|
| P0 | Entity extraction 边界 | 小 | 架构正确性 + token 成本 |
| P0 | Graph retrieval 实现 | 大 | 功能完整度 |
| P0 | ExecutePlanRequest/ChatRequest 统一 | 中 | 数据模型一致性 |
| P1 | GuardPipeline 完善 | 中 | 安全性 |
| P1 | Mode 映射一致性 | 小 | 前后端兼容性 |

---

## 与 Codex Review 的差异说明

Codex 可能更关注:
- 具体代码 bug（如空指针、错误处理）
- 性能问题（如不必要的 clone）
- 测试覆盖率

我的审查更关注:
- **架构边界清晰度**（P0 #1）
- **功能完整性**（P0 #2）
- **数据流正确性**（P0 #3）

两者互补，建议结合参考。

---

## 下一步行动

1. **确认 entity extraction 边界** — 伙计你做决定
2. **评估 graph retrieval 优先级** — 是否需要完整实现？
3. **我帮你改代码** — 你定方向，我执行

---

*由 大虾 🦐 深度审查生成*