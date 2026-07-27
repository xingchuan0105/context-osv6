# context-osv6 关键架构问题确认与修复建议

> 生成时间: 2026-04-26
> 问题级别: P0 (影响架构正确性)

---

## 问题 1: Main Agent vs RAG API 边界 —— ENTITY EXTRACTION 位置争议

### 现状分析

**架构文档要求** (§3, §4):
```
Main Agent: 负责用户交互、mode routing、workspace 指代消解、记忆使用、工具规划和最终回答
RAG API: 负责检索服务，可以调用 LLM 完成有边界的检索子任务（如 query entity extraction）
```

**当前实现**:

1. **Main Agent 侧** (`crates/app/src/main_agent/mod.rs`):
   - `RAG_PLAN_SYSTEM_PROMPT` 要求 LLM 输出 `query_entities` 和 `graph_hints`
   - Main Agent 直接生成 `ExecutePlanRequest`，包含 `query_entities` 字段

2. **RAG API 侧** (`crates/rag-core/src/runtime/execute.rs`):
   - `retrieve_graph_stage()` 方法中:
   ```rust
   // 1. 先用 Main Agent 提供的 query_entities
   let mut entity_names = request.query_entities.iter().map(|e| e.text.clone()).collect();
   
   // 2. 如果 Main Agent 没提供，RAG API 自己用 planner.extract_query_entities(query) 兜底
   if entity_names.is_empty() {
       if let Some(planner) = self.config.planner.as_ref() {
           match planner.extract_query_entities(query).await { ... }
       }
   }
   ```

### 问题判定

**这不是 Bug，是冗余设计**:
- Main Agent 已经做了 entity extraction（通过 prompt 让 LLM 输出）
- RAG API 有兜底机制（如果 Main Agent 没提供，自己再做一次）
- 这导致 **entity extraction 做了两次**，浪费 token

### 修复建议

**方案 A: 精简 Main Agent，把 entity extraction 移到 RAG API** (推荐)

1. 修改 `RAG_PLAN_SYSTEM_PROMPT`，移除 `query_entities` 和 `graph_hints` 字段
2. Main Agent 只输出 plan schema（doc_scope, items, summary_mode）
3. RAG API 负责:
   - query entity extraction（已有 `planner.extract_query_entities`）
   - graph hint 推导

**方案 B: 保留现状，移除 RAG API 兜底** (简单)

1. 保留 Main Agent 输出 `query_entities`/`graph_hints`
2. 删除 RAG API 中的 `planner.extract_query_entities` 兜底逻辑
3. 如果 Main Agent 没提供，graph channel 直接跳过

**建议采用方案 A**，因为:
- 架构文档明确说 RAG API 可以做 "query entity extraction"
- Main Agent 应该聚焦对话策略，不做检索细节
- 避免重复调用 LLM

---

## 问题 2: Graph Retrieval 实现过简

### 现状

**架构要求** (§6, §7.4):
```
Query -> query entity extraction -> entity vector search + relation vector search -> subgraph expansion -> fan-out control / eviction -> relation/path rerank -> supporting chunk hydration
```

**当前实现** (`crates/storage-milvus/src/lib.rs:720`):
```rust
async fn search_graph(&self, request: GraphSearchRequest) -> anyhow::Result<GraphSearchOutput> {
    let Some(filter) = graph_relation_filter(&request) else {
        return Ok(GraphSearchOutput::default());
    };
    
    // 仅做关系属性过滤查询，缺少:
    // - entity vector search (用 entity embedding 找相似实体)
    // - relation vector search (用 relation embedding 找相似关系)
    // - subgraph expansion (从命中实体扩展多跳)
    // - fan-out limit / hop limit
}
```

### 修复建议

**短期** (保持当前简单实现，加限制):
1. 增加 `fan_out_limit` 和 `hop_limit` 配置
2. 在 `GraphSearchRequest` 中增加这些参数
3. 当前实现先用属性过滤，标注为 "v1 简化实现"

**中期** (完整实现):
1. 在 Milvus 中增加 `kg_entities` 的 vector 字段
2. 实现 entity vector search
3. 实现 subgraph expansion（从 entity -> relation -> entity 多跳）
4. 增加 relation/path rerank

---

## 问题 3: DESIGN.md 过时

### 现状

**DESIGN.md** 描述 Notion 暖色调设计系统:
- `#f6f5f4` warm white
- NotionInter 字体
- 暖灰色调

**实际前端** (`frontend_next/app/design-tokens.css`):
- 冷色调 HSL 系统
- Inter + 系统字体栈
- 现代冷灰 UI

### 修复建议

1. **删除 DESIGN.md** 或重命名为 `DESIGN.md.ARCHIVED`
2. 在 `frontend_next/` 中增加 `DESIGN_SYSTEM.md`，从 `design-tokens.css` 反向生成
3. 更新所有引用 DESIGN.md 的文档

---

## 问题 4: Milvus Schema 完整性

### 现状

**架构要求** (§2):
```
Milvus 应包含: text chunks, multimodal chunks, BM25 sparse, dense text vectors, multimodal vectors, kg_entities, kg_relations, graph passages, semantic memory vectors
```

**当前实现**:
- ✅ text_chunks
- ✅ multimodal_chunks  
- ✅ kg_entities
- ✅ kg_relations
- ✅ graph_passages
- ❓ BM25 sparse (可能仍用 PG)
- ❌ semantic memory vectors

### 修复建议

1. **确认 BM25**: 检查 `search_bm25` 是否真正使用 Milvus 还是 PG fallback
2. **增加 semantic memory collection**: 用于 Main Agent 长期记忆

---

## 修复优先级

| 优先级 | 问题 | 工作量 | 影响 |
|--------|------|--------|------|
| P0 | 确认 entity extraction 边界 | 小 | 架构正确性 |
| P0 | 归档 DESIGN.md | 极小 | 文档一致性 |
| P1 | Graph retrieval 增加 fan-out/hop limit | 中 | 功能完整度 |
| P1 | 验证 Milvus BM25 | 中 | 架构一致性 |
| P2 | Semantic memory collection | 中 | 记忆层完整度 |
| P2 | Graph retrieval 完整 pipeline | 大 | 多跳检索质量 |

---

## 下一步行动建议

1. **先确认**: 伙计你希望 entity extraction 在 Main Agent 还是 RAG API？
2. **立即做**: 归档 DESIGN.md
3. **然后**: 根据你的选择修复 entity extraction 边界
4. **最后**: 处理 Graph retrieval 和 Milvus schema

---

*由 大虾 🦐 分析生成*