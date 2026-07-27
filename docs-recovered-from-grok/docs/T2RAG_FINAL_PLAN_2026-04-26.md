# T²RAG 架构补全计划（基于 TRAG.py 参考实现）

> 生成时间: 2026-04-26
> 基准: GPT plan + T²RAG 方向 + TRAG.py 参考代码
> 状态: 待执行

---

## T²RAG 核心流程（基于 TRAG.py 分析）

### 4 步迭代流程

```
Step 1: reason_and_form_triples(query)
  → LLM 分析 query，输出带 ? 占位符的三元组
  → 分类: fuzzy(2+?) / traceable(1?) / resolved(0?)

Step 2: retrieve_and_double_check(traceable_clues)
  → 用已知实体（非占位符部分）检索 proposition embeddings
  → 返回相关 passages + propositions

Step 3: resolve_clues(traceable, fuzzy, retrieval_results)
  → LLM 用检索结果填充占位符
  → fuzzy → traceable → resolved
  → 迭代直到全部 resolved 或达到 max_qa_steps

Step 4: final_qa(query, all_resolved_clues)
  → 用 resolved clues 生成最终答案
```

### 关键设计

1. **占位符语法**: `?` 表示未知，`?directorA` 表示命名占位符
2. **检索策略**: 只用 **已知实体**（非 ? 部分）做向量检索
3. **迭代填充**: 一次填充一个占位符，逐步缩小未知范围
4. **Proposition embeddings**: 预计算所有三元组的 embedding，检索时用 cosine similarity

---

## 当前代码与 T²RAG 的映射

### 已映射部分

| T²RAG 组件 | 当前代码 | 状态 |
|-----------|---------|------|
| Step 1: triple formation | Main Agent `RAG_PLAN_SYSTEM_PROMPT` | ✅ 已要求输出 placeholder_triplets |
| Step 1: triple classification | 未实现 | ❌ 需要增加 fuzzy/traceable/resolved 分类 |
| Step 2: proposition retrieval | `RetrievalDataPlane::search_graph` | ⚠️ 需要改造为向量检索 |
| Step 3: clue resolution | 未实现 | ❌ 需要新增 LLM 填充逻辑 |
| Step 4: final QA | Main Agent `answer_rag` | ✅ 已有 answer generation |

### 缺失的核心逻辑

1. **Triple 分类**: `ExecutePlanRequest.placeholder_triplets` 需要分类为 fuzzy/traceable/resolved
2. **Proposition embedding 存储**: 需要预计算和存储三元组 embedding
3. **向量检索**: `search_graph` 需要从属性过滤升级为向量相似度检索
4. **Clue resolution**: 需要新增 LLM 调用填充占位符

---

## 修正后的修复计划

### P0 #1: T²RAG Triple 分类与检索链路

**目标**: 让 Main Agent 输出的 placeholder_triplets 能被 RAG Runtime 正确处理

**修改 1: 在 common 中增加 Triple 分类**

```rust
// common/src/rag_execute.rs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaceholderTripletType {
    Fuzzy,      // 2+ ? placeholders
    Traceable,  // 1 ? placeholder
    Resolved,   // 0 ? placeholders
}

impl PlaceholderTriplet {
    pub fn classify(&self) -> PlaceholderTripletType {
        let question_count = self.subject.matches('?').count()
            + self.predicate.matches('?').count()
            + self.object.matches('?').count();
        match question_count {
            0 => PlaceholderTripletType::Resolved,
            1 => PlaceholderTripletType::Traceable,
            _ => PlaceholderTripletType::Fuzzy,
        }
    }
    
    /// 返回已知实体（非占位符部分）
    pub fn known_entities(&self) -> Vec<String> {
        [self.subject.clone(), self.predicate.clone(), self.object.clone()]
            .into_iter()
            .filter(|s| !s.starts_with("?"))
            .collect()
    }
}
```

**修改 2: 在 RAG Runtime 中处理 placeholder_triplets**

```rust
// rag-core/src/runtime/execute.rs
async fn retrieve_graph_stage(...) -> GraphChannelOutput {
    // 1. 分类 triplets
    let mut fuzzy_clues = Vec::new();
    let mut traceable_clues = Vec::new();
    let mut resolved_clues = Vec::new();
    
    for triplet in &request.placeholder_triplets {
        match triplet.classify() {
            PlaceholderTripletType::Fuzzy => fuzzy_clues.push(triplet),
            PlaceholderTripletType::Traceable => traceable_clues.push(triplet),
            PlaceholderTripletType::Resolved => resolved_clues.push(triplet),
        }
    }
    
    // 2. 用 traceable clues 的已知实体检索（T²RAG Step 2）
    let known_entities: Vec<String> = traceable_clues.iter()
        .flat_map(|t| t.known_entities())
        .collect();
    
    // 3. 向量检索（需要改造 search_graph）
    let search_results = self.data_plane.search_graph(GraphSearchRequest {
        auth: auth.clone(),
        doc_ids,
        entity_names: known_entities,  // 只用已知实体
        relation_hints: graph_relation_hints(request),
        relation_limit,
        supporting_chunk_limit,
    }).await;
    
    // 4. 返回 enriched output（包含 resolved clues）
    // ...
}
```

### P0 #2: Graph Provenance（同 GPT 计划）

**修改**: Worker triplet extraction prompt 要求返回 chunk_id

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
"#;
```

### P0 #3: Graph Citations（同 GPT 计划）

**修改**: `RetrievalBundle` 增加 `citation_chunks()` helper

```rust
impl RetrievalBundle {
    pub fn citation_chunks(&self) -> Vec<&RetrievedChunk> {
        // chunks + graph_supported_chunks，去重，regular 优先
    }
    
    pub fn has_evidence(&self) -> bool {
        !self.chunks.is_empty()
            || !self.graph_supported_chunks.is_empty()
            || !self.summary_chunks.is_empty()
    }
}
```

### P0 #4: Milvus Replace Safety（同 GPT 计划）

**修改**: insert-first + delete-old-parse-runs

```rust
async fn replace_document_index(&self, batch: DocumentIndexBatch) -> anyhow::Result<IndexWriteReport> {
    // Phase 1: Insert new data
    // Phase 2: Check all inserts succeeded
    // Phase 3: Delete old parse_run_ids
}
```

---

## T²RAG 特有新增工作

### 新增 1: Proposition Embedding 存储

**文件**: `crates/rag-core/src/proposition_embedding.rs`

```rust
use avrag_retrieval_data_plane::RelationIndexRecord;

/// 预计算和存储三元组 proposition 的 embedding
pub struct PropositionEmbeddingStore {
    embeddings: HashMap<String, Vec<f32>>,  // proposition text -> embedding
    proposition_to_relation: HashMap<String, Uuid>,  // proposition -> relation_id
}

impl PropositionEmbeddingStore {
    pub fn new() -> Self {
        Self {
            embeddings: HashMap::new(),
            proposition_to_relation: HashMap::new(),
        }
    }
    
    /// 从 RelationIndexRecord 构建 proposition
    pub fn add_relation(&mut self, relation: &RelationIndexRecord, embedding: Vec<f32>) {
        let proposition = format!("{} {} {}", relation.subject, relation.predicate, relation.object);
        self.embeddings.insert(proposition.clone(), embedding);
        self.proposition_to_relation.insert(proposition, relation.relation_id);
    }
    
    /// 用已知实体检索相似 proposition
    pub fn search(&self, known_entities: &[String], top_k: usize) -> Vec<(String, f32)> {
        // 用 known entities 构建 query embedding
        // cosine similarity 检索 top_k propositions
    }
}
```

### 新增 2: Clue Resolution LLM 调用

**文件**: `crates/rag-core/src/clue_resolution.rs`

```rust
use avrag_llm::{ChatMessage, LlmClient};

/// T²RAG Step 3: 用检索结果填充占位符
pub async fn resolve_clues(
    llm: &LlmClient,
    traceable_clues: &[PlaceholderTriplet],
    fuzzy_clues: &[PlaceholderTriplet],
    retrieved_propositions: &[RetrievedProposition],
) -> anyhow::Result<ClueResolutionResult> {
    let prompt = build_clue_resolution_prompt(traceable_clues, fuzzy_clues, retrieved_propositions);
    let messages = vec![
        ChatMessage::system(CLUE_RESOLUTION_SYSTEM_PROMPT),
        ChatMessage::user(prompt),
    ];
    let response = llm.complete(&messages, Some(0.1)).await?;
    parse_clue_resolution_response(&response.content)
}
```

---

## 修正后的优先级

```
P0 #1: T²RAG Triple 分类 + 检索链路（新增核心）
   ↓ T²RAG 功能的基础
P0 #2: Graph Provenance（chunk-level）
   ↓ 保证检索结果可信
P0 #3: Graph Citations（bundle helper）
   ↓ 用户可见输出
P0 #4: Milvus Replace Safety
   ↓ 基础设施安全
P1: Proposition Embedding 存储（T²RAG 优化）
   ↓ 提升检索效率
P1: Clue Resolution LLM（T²RAG 完整实现）
   ↓ 完整 T²RAG 迭代流程
```

**说明**:
- P0 #1 是 T²RAG 的最小可用实现（MVP）
- P1 是 T²RAG 的完整优化，可以后续迭代

---

## 测试计划（新增 T²RAG 测试）

### T²RAG 核心测试

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p avrag-rag-core -- t2rag
```

新增测试:
1. `test_triple_classification` — fuzzy/traceable/resolved 分类正确
2. `test_known_entities_extraction` — 正确提取非占位符实体
3. `test_placeholder_triplet_retrieval` — 已知实体触发 graph 检索
4. `test_t2rag_end_to_end` — Main Agent 输出 → RAG Runtime 处理 → 返回 enriched response

### 其他测试（同 GPT 计划）

见 `docs/REFINED_FIX_PLAN_2026-04-26.md`

---

## 与 GPT 原计划的差异

| 方面 | GPT 原计划 | 修正后（基于 T²RAG） |
|------|-----------|---------------------|
| 核心问题 | provenance + citation + Milvus | **新增 T²RAG triple 分类 + 检索链路** |
| placeholder_triplets | 未提及使用 | **P0 #1: 必须实现分类和检索** |
| 迭代填充 | 未提及 | **P1: 新增 clue resolution** |
| Proposition embedding | 未提及 | **P1: 新增预计算存储** |

---

## 执行顺序（修正）

```
Day 1:
  - P0 #1: T²RAG triple 分类 + 检索链路
  - 修改 common/src/rag_execute.rs（增加 classify + known_entities）
  - 修改 rag-core/src/runtime/execute.rs（处理 placeholder_triplets）
  - 运行 T²RAG 核心测试

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
  - 全量测试

Day 5-6（可选）:
  - P1: Proposition embedding 存储
  - P1: Clue resolution LLM
  - 完整 T²RAG 迭代流程
```

---

## 关键确认

伙计，请确认:

1. **T²RAG MVP 范围** — 先实现 triple 分类 + 已知实体检索（不实现迭代填充）？
2. **Proposition embedding** — 本轮用现有 `search_graph` 还是新增向量检索？
3. **Clue resolution** — 本轮用简单填充（直接返回检索结果）还是完整 LLM 填充？

确认后我立即开始实现 🦐