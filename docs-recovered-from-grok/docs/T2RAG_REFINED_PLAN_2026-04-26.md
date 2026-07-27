# 基于 T²RAG 架构的完善补全计划

> 生成时间: 2026-04-26
> 基准: GPT plan + T²RAG 方向（Main Agent 输出 placeholder triplets，RAG Runtime 检索补齐）
> 状态: 待执行

---

## T²RAG 架构理解（基于代码验证）

### 已实现部分

1. **Main Agent Prompt** (`main_agent/mod.rs:23-58`):
   ```
   "placeholder_triplets": [
     { "subject": "known entity or ?placeholder", "predicate": "relationship", "object": "known entity or ?placeholder" }
   ]
   ```

2. **数据结构** (`common/src/rag_execute.rs:81-85`):
   ```rust
   pub struct PlaceholderTriplet {
       pub subject: String,
       pub predicate: String,
       pub object: String,
   }
   
   pub struct ExecutePlanRequest {
       // ...
       pub placeholder_triplets: Vec<PlaceholderTriplet>,
   }
   ```

### 未实现部分

3. **RAG Runtime 使用 placeholder_triplets** —— ❌ **缺失！**

   当前 `retrieve_graph_stage` (`rag-core/src/runtime/execute.rs:500-560`):
   ```rust
   async fn retrieve_graph_stage(...) -> GraphChannelOutput {
       let mut entity_names = request.query_entities.iter().map(|e| e.text.clone()).collect();
       let relation_hints = graph_relation_hints(request);  // ← 只读 graph_hints，不读 placeholder_triplets！
       // ...
   }
   ```

   **问题**: `placeholder_triplets` 被 Main Agent 输出，但 RAG Runtime 完全没使用！

---

## 修正后的问题分析

### P0 #1: T²RAG 链路未打通 —— placeholder_triplets 未被使用

**影响**: Main Agent 输出的 placeholder triplets 被忽略，graph retrieval 仍然依赖传统的 entity extraction，没有实现 "推测 → 检索补齐" 的 T²RAG 流程。

**修复**: 在 `retrieve_graph_stage` 中增加对 `placeholder_triplets` 的处理：
- 解析 placeholder triplets（处理 `?` 占位符）
- 用已知实体检索 graph 数据
- 用检索结果填充占位符

### P0 #2: Graph Provenance 不可靠（GPT 发现）

**问题**: triplet 绑定到整个 batch，而非具体 chunk

**修复**: 同 GPT plan（prompt 要求返回 chunk_id，解析时验证）

### P0 #3: Graph Citations 丢失（GPT 发现）

**问题**: builder 只读 `bundle.chunks`，忽略 `graph_supported_chunks`

**修复**: 同 GPT plan（增加 `citation_chunks()` helper）

### P0 #4: Milvus replace safety（GPT 发现，升级 P0）

**问题**: delete-first 导致数据丢失风险

**修复**: 同 GPT plan（insert-first + delete-old-parse-runs）

---

## 修正后的优先级（按 T²RAG 依赖关系）

```
P0 #1: T²RAG 链路打通（placeholder_triplets → retrieval）
   ↓ T²RAG 的核心功能
P0 #2: Graph triplet provenance（chunk-level）
   ↓ 保证检索结果可信
P0 #3: Graph citations（bundle helper）
   ↓ 用户可见输出
P0 #4: Milvus replace safety
   ↓ 基础设施安全
```

**说明**: 
- T²RAG 链路是当前架构方向的核心，优先打通
- Provenance 和 Citations 是 GPT 发现的具体 bug，必须修
- Milvus safety 是基础设施，可以并行处理

---

## 详细修复方案

### P0 #1: T²RAG Placeholder Triplets 检索补齐

**新增**: `rag-core/src/runtime/graph_placeholder.rs`

```rust
use avrag_retrieval_data_plane::{GraphSearchRequest, RelationPathCandidate};

/// 解析 placeholder triplet，区分已知实体和占位符
pub fn parse_placeholder_triplets(triplets: &[PlaceholderTriplet]) -> Vec<ResolvedTripletQuery> {
    triplets.iter().map(|t| {
        let subject_known = !t.subject.starts_with("?");
        let object_known = !t.object.starts_with("?");
        
        ResolvedTripletQuery {
            known_entities: [
                subject_known.then_some(t.subject.clone()),
                object_known.then_some(t.object.clone()),
            ].into_iter().flatten().collect(),
            predicate_hint: t.predicate.clone(),
            subject_placeholder: subject_known.then_some(t.subject.clone()),
            object_placeholder: object_known.then_some(t.object.clone()),
        }
    }).collect()
}

/// 用检索结果填充占位符
pub fn fill_placeholders(
    triplets: &[PlaceholderTriplet],
    relation_paths: &[RelationPathCandidate],
) -> Vec<FilledTriplet> {
    // 匹配 relation_paths 和 placeholders
    // 返回填充后的 triplet
}
```

**修改**: `retrieve_graph_stage`

```rust
async fn retrieve_graph_stage(...) -> GraphChannelOutput {
    // 1. 处理传统 query_entities + graph_hints（保持兼容）
    let mut entity_names = request.query_entities.iter().map(|e| e.text.clone()).collect();
    let relation_hints = graph_relation_hints(request);
    
    // 2. 新增：处理 placeholder_triplets
    let placeholder_queries = parse_placeholder_triplets(&request.placeholder_triplets);
    for query in placeholder_queries {
        entity_names.extend(query.known_entities);
        if !query.predicate_hint.is_empty() {
            relation_hints.push(GraphRelationHint {
                subject: query.subject_placeholder,
                predicate: Some(query.predicate_hint),
                object: query.object_placeholder,
            });
        }
    }
    
    entity_names = dedupe_entity_names(entity_names);
    
    // 3. 检索 graph（现有逻辑）
    match self.data_plane.search_graph(GraphSearchRequest {
        auth: auth.clone(),
        doc_ids,
        entity_names,
        relation_hints,
        relation_limit,
        supporting_chunk_limit,
    }).await {
        Ok(output) => {
            // 4. 新增：用检索结果填充 placeholders
            let filled = fill_placeholders(&request.placeholder_triplets, &output.relation_paths);
            // ... 返回 enriched output
        }
        // ...
    }
}
```

### P0 #2-4: 同 GPT 原计划

见 `docs/REFINED_FIX_PLAN_2026-04-26.md`

---

## 测试计划（新增 T²RAG 测试）

### T²RAG 专用测试

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p avrag-rag-core -- graph_placeholder
```

新增测试:
1. `test_placeholder_triplet_parsing` — 解析 `?placeholder` 和已知实体
2. `test_placeholder_fill_with_relation_paths` — 用检索结果填充占位符
3. `test_placeholder_triplet_end_to_end` — Main Agent 输出 → RAG Runtime 处理 → 返回 filled triplets
4. `test_mixed_placeholder_and_known` — 混合占位符和已知实体的 triplet

---

## 与 GPT 原计划的差异

| 方面 | GPT 原计划 | 修正后（基于 T²RAG） |
|------|-----------|---------------------|
| 核心问题 | provenance + citation + Milvus | **新增 T²RAG 链路未打通** |
| placeholder_triplets | 未提及使用 | **P0 #1: 必须实现检索补齐** |
| 优先级 | provenance → citation → Milvus | **T²RAG 链路 → provenance → citation → Milvus** |
| Entity extraction | 未提及 | 本轮不处理（T²RAG 方向已覆盖） |

---

## 执行顺序（修正）

```
Day 1:
  - P0 #1: T²RAG placeholder triplets 检索补齐
  - 新增 graph_placeholder.rs 模块
  - 修改 retrieve_graph_stage
  - 运行 T²RAG 测试

Day 2:
  - P0 #2: Graph triplet provenance
  - 修改 worker triplet extraction prompt
  - 运行 Worker 测试

Day 3:
  - P0 #3: Graph citations
  - 增加 bundle helper
  - 运行 RAG Core + Main Agent 测试

Day 4:
  - P0 #4: Milvus replace safety
  - 运行 Milvus 测试
  - 全量测试 + Live smoke
```

---

## 关键确认

伙计，请确认:

1. **T²RAG 理解是否正确？**
   - Main Agent 输出 placeholder triplets（带 `?` 占位符）
   - RAG Runtime 用已知实体检索，填充占位符
   - 最终返回 filled triplets 给用户

2. **Placeholder 语法？**
   - `?` 前缀表示占位符？
   - 还是其他约定（如 `?directorA`）？

3. **填充后的 triplets 用途？**
   - 仅用于 graph retrieval 内部？
   - 还是返回给 Main Agent 用于 answer generation？
   - 还是展示给用户？

确认后我立即开始实现 🦐