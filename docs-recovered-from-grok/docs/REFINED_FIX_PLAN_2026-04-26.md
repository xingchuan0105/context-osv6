# 完善后的补全计划：Graph Provenance + Citations + Milvus Safety

> 基于 GPT 审查 + 大虾深度审查的交叉验证
> 生成时间: 2026-04-26
> 状态: 待执行

---

## 交叉验证结论

### GPT 发现 vs 我的发现

| 问题 | GPT | 大虾 | 交叉结论 |
|------|-----|------|---------|
| Graph provenance 不可靠 | P0 | 未单独发现 | **确认 P0** —— 代码证据确凿 |
| Graph citations 丢失 | P0 | P0 #2 的一部分 | **确认 P0** —— 两个 builder 都只读 bundle.chunks |
| Milvus delete-first 风险 | P1 | 未单独发现 | **升级 P0** —— 数据丢失风险不可接受 |
| Entity extraction 边界 | 未提及 | P0 #1 | **新增 P0** —— 架构级问题，需同步修复 |
| Graph retrieval 欠完整 | 未提及（本轮不重构算法） | P0 #2 | **本轮降级为 P1** —— 按 GPT 计划先修 provenance/citation |

### 关键洞察

**GPT 的视角**: 聚焦具体代码缺陷（provenance、 citation、数据安全）
**我的视角**: 聚焦架构边界和职责划分

**互补点**:
- GPT 发现了具体 bug（triplet 绑定到整个 batch）
- 我发现了架构问题（entity extraction 做了两次）
- 两者都指向 **Graph 链路不完整**

---

## 完善后的修复计划

### 优先级重排（按风险 + 依赖关系）

```
P0 #1: Milvus replace safety (delete-first → insert-first)
   ↓ 因为所有后续索引操作都依赖这个
P0 #2: Graph triplet provenance (batch-level → chunk-level)
   ↓ 因为 citation 修复依赖正确的 chunk 绑定
P0 #3: Graph citations (bundle.chunks → all_chunks helper)
   ↓ 因为用户可见的输出依赖这个
P0 #4: Entity extraction 边界清理
   ↓ 架构级优化，可独立进行
P1: Graph retrieval 算法完善 (本轮不做)
```

---

## 详细修复方案

### P0 #1: Milvus replace_document_index 安全性

**当前代码** (`storage-milvus/src/lib.rs:463-465`):
```rust
async fn replace_document_index(&self, batch: DocumentIndexBatch) -> anyhow::Result<IndexWriteReport> {
    self.ensure_schema().await?;
    let auth = AuthContext::new(batch.org_id, avrag_auth::SubjectKind::System);
    self.delete_document_index(&auth, batch.document_id).await?;  // ← 先删除！
    // ... 然后逐个 collection insert
}
```

**问题**: 如果 insert 中途失败，旧索引已删除，新索引不完整 = **数据丢失**

**修复方案**:

```rust
async fn replace_document_index(&self, batch: DocumentIndexBatch) -> anyhow::Result<IndexWriteReport> {
    self.ensure_schema().await?;
    let names = self.config.collection_names();
    
    // Phase 1: Insert new data (all collections)
    let insert_results = vec![
        self.insert_text_chunks(&names, &batch).await,
        self.insert_multimodal_chunks(&names, &batch).await,
        self.insert_entities(&names, &batch).await,
        self.insert_relations(&names, &batch).await,
        self.insert_graph_passages(&names, &batch).await,
    ];
    
    // Phase 2: Check all inserts succeeded
    let any_failed = insert_results.iter().any(|r| r.is_err());
    if any_failed {
        // 新数据写入失败，不删除旧索引
        // 返回错误，但旧索引仍然可用
        return Err(anyhow!("Insert failed, old index preserved"));
    }
    
    // Phase 3: All inserts OK, now delete old parse_run_ids
    let auth = AuthContext::new(batch.org_id, avrag_auth::SubjectKind::System);
    self.delete_old_parse_runs(&auth, batch.document_id, batch.parse_run_id).await?;
    
    Ok(IndexWriteReport { ... })
}
```

**关键变更**:
1. 先写入新数据（所有 collection）
2. 确认全部成功后，再删除旧 parse_run_id 的数据
3. 写入失败时，旧索引保留，返回错误

---

### P0 #2: Graph Triplet Provenance

**当前代码** (`worker/src/main.rs:2056-2066`):
```rust
fn build_triplet_extraction_messages(batch: &TripletExtractionBatch) -> Vec<ChatMessage> {
    // prompt 只要求返回三元组，不要求 chunk_id
}

// 解析后:
let triplets = raw_triplets
    .into_iter()
    .map(|(subject, predicate, object)| ExtractedTriplet {
        subject, predicate, object,
        supporting_chunk_ids: batch.chunk_ids.clone(),  // ← 绑定到整个 batch！
    })
    .collect::<Vec<_>>();
```

**问题**: 
- LLM 不返回 chunk_id → 不知道 triplet 来自哪个 chunk
- 绑定到整个 batch → citation 可能指向无关 chunk
- `GraphPassageIndexRecord.chunk_id` 取 `supporting_chunk_ids.first()` → 任意取第一个

**修复方案**:

**Step 1: 修改 prompt 要求返回 chunk_id**

```rust
const TRIPLET_EXTRACTION_SYSTEM_PROMPT: &str = r#"
Extract graph triplets from the provided text chunks.
Return raw JSON only:
{
  "triplets": [
    {
      "chunk_id": "uuid-of-source-chunk",
      "subject": "entity name",
      "predicate": "relation",
      "object": "entity name"
    }
  ]
}
Rules:
- chunk_id must be one of the provided chunk IDs
- Only extract concrete, factual relationships
- Return empty array if no valid triplets found
"#;
```

**Step 2: 修改解析逻辑，验证 chunk_id**

```rust
fn parse_triplet_response(content: &str, valid_chunk_ids: &[Uuid]) -> Result<Vec<ExtractedTriplet>> {
    let value: Value = serde_json::from_str(content)?;
    let triplets = value.get("triplets")
        .and_then(Value::as_array)
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|t| {
            let chunk_id = t.get("chunk_id")?.as_str()?;
            let chunk_uuid = Uuid::parse_str(chunk_id).ok()?;
            // 验证 chunk_id 在 batch 内
            if !valid_chunk_ids.contains(&chunk_uuid) {
                return None;  // 丢弃非法 chunk_id
            }
            Some(ExtractedTriplet {
                subject: t.get("subject")?.as_str()?.to_string(),
                predicate: t.get("predicate")?.as_str()?.to_string(),
                object: t.get("object")?.as_str()?.to_string(),
                supporting_chunk_ids: vec![chunk_uuid],  // ← 只绑定到标注的 chunk
            })
        })
        .collect();
    Ok(triplets)
}
```

**Step 3: 合并相同 triplet 的真实 supporting chunks**

```rust
// 在去重时合并 supporting_chunk_ids
let mut seen = HashMap::new();
for triplet in triplets {
    let key = (triplet.subject.to_lowercase(), triplet.predicate.to_lowercase(), triplet.object.to_lowercase());
    if let Some(existing) = seen.get_mut(&key) {
        // 合并 supporting_chunk_ids
        existing.supporting_chunk_ids.extend(triplet.supporting_chunk_ids);
        existing.supporting_chunk_ids.sort_unstable();
        existing.supporting_chunk_ids.dedup();
    } else {
        seen.insert(key, triplet);
    }
}
```

---

### P0 #3: Graph Citations 修复

**当前代码** —— 两个 builder 都只读 `bundle.chunks`:

```rust
// main_agent/mod.rs:460
let ordered_chunks = if cited_chunk_ids.is_empty() {
    execute_response.bundle.chunks.clone()
} else {
    execute_response.bundle.chunks.iter()
        .filter(|chunk| cited_chunk_ids.contains(&chunk.chunk_id))
        .cloned()
        .collect()
};

// rag-core/src/runtime/response.rs:557
if execute_response.bundle.chunks.is_empty() 
    && execute_response.bundle.summary_chunks.is_empty() {
    return Ok(no_chunks_response(...));  // ← graph_supported_chunks 被忽略！
}
```

**修复方案**:

**Step 1: 在 RetrievalBundle 增加 helper**

```rust
impl RetrievalBundle {
    /// 返回所有可用于 citation 的 chunks，去重并保持优先级
    pub fn citation_chunks(&self) -> Vec<&RetrievedChunk> {
        let mut all_chunks = Vec::with_capacity(
            self.chunks.len() + self.graph_supported_chunks.len()
        );
        
        // Regular chunks 优先
        all_chunks.extend(&self.chunks);
        
        // Graph chunks 补充（去重）
        let regular_ids: HashSet<_> = self.chunks.iter()
            .map(|c| &c.chunk_id)
            .collect();
        for chunk in &self.graph_supported_chunks {
            if !regular_ids.contains(&chunk.chunk_id) {
                all_chunks.push(chunk);
            }
        }
        
        all_chunks
    }
    
    /// 检查是否有任何 evidence
    pub fn has_evidence(&self) -> bool {
        !self.chunks.is_empty()
            || !self.graph_supported_chunks.is_empty()
            || !self.summary_chunks.is_empty()
    }
}
```

**Step 2: 修改两个 builder**

```rust
// main_agent/mod.rs
let all_chunks = execute_response.bundle.citation_chunks();
let ordered_chunks = if cited_chunk_ids.is_empty() {
    all_chunks.iter().cloned().collect()  // ← 包含 graph chunks
} else {
    all_chunks.iter()
        .filter(|chunk| cited_chunk_ids.contains(&chunk.chunk_id))
        .cloned()
        .collect()
};

// rag-core/src/runtime/response.rs
if !execute_response.bundle.has_evidence() {
    return Ok(no_chunks_response(...));  // ← 考虑所有 evidence
}
```

---

### P0 #4: Entity Extraction 边界清理（新增）

**问题**: Main Agent 和 RAG API 都做了 entity extraction

**修复**（可独立进行，不影响其他修复）:

**Option A: 精简 Main Agent（推荐）**
1. `RAG_PLAN_SYSTEM_PROMPT` → 移除 `query_entities` 和 `graph_hints`
2. `ExecutePlanRequest` → 移除这两个字段
3. `retrieve_graph_stage` → 始终使用 `planner.extract_query_entities`

**Option B: 删除 RAG API 兜底**
1. 保留 Main Agent 输出
2. 删除 `retrieve_graph_stage` 中的兜底逻辑

**建议**: 选 Option A，因为:
- 架构文档明确说 RAG API 可以做 query entity extraction
- Main Agent 应该聚焦对话策略
- 减少 LLM 调用次数

---

## 测试计划（完善）

### Worker 测试

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p avrag-worker --no-fail-fast
```

新增测试:
1. `test_triplet_with_chunk_id_parses` — 新 JSON shape 可解析
2. `test_triplet_missing_chunk_id_rejected` — 缺 chunk_id 不产生 triplet
3. `test_triplet_invalid_chunk_id_rejected` — 非法 chunk_id 丢弃
4. `test_triplet_cross_batch_rejected` — 跨 batch chunk_id 丢弃
5. `test_duplicate_triplet_merges_supporting_chunks` — 相同三元组合并

### RAG Core 测试

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p avrag-rag-core --no-fail-fast
```

新增测试:
1. `test_bundle_citation_chunks_includes_graph` — helper 包含 graph chunks
2. `test_bundle_has_evidence_with_graph_only` — graph-only 返回 true
3. `test_no_chunks_response_considers_graph` — graph-only 不触发 no_chunks

### Main Agent 测试

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p app main_agent --no-fail-fast
```

新增测试:
1. `test_graph_only_response_has_citations` — graph-only evidence 返回非空 citations
2. `test_graph_only_response_has_sources` — graph-only evidence 返回非空 sources

### Milvus 测试

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p avrag-storage-milvus --test milvus_adapter --no-fail-fast
```

新增测试:
1. `test_replace_twice_keeps_latest` — 同一 doc 连续 replace 两次，第二次后只能检索到最新 parse_run_id
2. `test_replace_insert_failure_preserves_old` — 模拟 insert 失败，旧索引保留

### Live Smoke（需 Milvus）

```bash
MILVUS_INTEGRATION_TEST=1 cargo test -p avrag-storage-milvus --test milvus_adapter --no-fail-fast
```

验证:
1. 20 条 sanity 全部返回非空 evidence
2. Graph 类允许 graph degrade，但必须有 dense/BM25 fallback
3. Reindex 后旧数据不可检索

---

## 执行顺序

```
Day 1:
  - P0 #1: Milvus replace safety
  - 运行 Milvus 测试

Day 2:
  - P0 #2: Graph triplet provenance
  - 运行 Worker 测试

Day 3:
  - P0 #3: Graph citations
  - 运行 RAG Core + Main Agent 测试

Day 4:
  - P0 #4: Entity extraction 边界（可选，可延后）
  - 全量测试
  - Live smoke
```

---

## 与 GPT 原计划的差异

| 方面 | GPT 原计划 | 完善后 |
|------|-----------|--------|
| 优先级 | provenance → citation → Milvus | **Milvus → provenance → citation** |
| Entity extraction | 未提及 | **新增 P0 #4** |
| Graph retrieval 算法 | 本轮不做 | 本轮不做（一致） |
| 测试覆盖 | 已有计划 | 增加 `has_evidence` 和 `citation_chunks` 测试 |

---

## 下一步

伙计，确认:
1. **优先级顺序** 是否接受？（Milvus 安全 → provenance → citation）
2. **Entity extraction 边界** 是否本轮处理？
3. **Option A 或 B**？（精简 Main Agent vs 删除 RAG API 兜底）

确认后我立即开始改代码 🦐