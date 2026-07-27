# context-osv6 架构 vs 代码实现 Gap Analysis

> 生成时间: 2026-04-26
> 基于: 2026-04-26-current-product-rag-architecture.md vs 当前代码实现
> 评审者: 大虾 (AI Assistant)

---

## 1. 总体评估

| 维度 | 状态 | 说明 |
|------|------|------|
| 架构方向 | ✅ 一致 | Main Agent + RAG API + Milvus 方向已确立 |
| 后端实现 | 🟡 部分完成 | 核心框架就绪，多处细节待完善 |
| 前端实现 | 🟢 基准确立 | `frontend_next/` 是当前权威基准 |
| 文档一致性 | ❌ 大量矛盾 | DESIGN.md 等旧文档与实现不符 |

---

## 2. 后端 Gap 详细分析

### 2.1 ✅ 已实现（符合架构）

| 组件 | 实现状态 | 位置 |
|------|----------|------|
| Main Agent 模式路由 | ✅ | `crates/app/src/main_agent/mod.rs` |
| RAG API 输入/输出契约 | ✅ | `crates/common/src/rag_execute.rs` |
| 4 通道检索执行 | ✅ | `crates/rag-core/src/runtime/execute.rs` |
| 通道预算分配 | ✅ | 默认 35/25/15/25 (text/BM25/mm/graph) |
| Milvus DataPlane | ✅ | `crates/storage-milvus/src/lib.rs` |
| 检索数据平面 trait | ✅ | `crates/retrieval-data-plane/src/lib.rs` |
| Triplet 提取 | ✅ | `bins/worker/src/main.rs` |
| Graph 索引记录构建 | ✅ | `bins/worker/src/main.rs` |
| SSE 流式输出 | ✅ | `crates/app/src/lib_impl/chat_streaming.rs` |
| 前端 stream 解析 | ✅ | `frontend_next/lib/workspace/stream.ts` |

### 2.2 ⚠️ 部分实现（有 Gap）

#### Gap 1: RAG API 边界不清 — Main Agent 仍在做 RAG 执行

**架构要求**:
```
Main Agent → 生成 RAG tool plan schema → RAG API → 检索服务 → Main Agent 生成回答
```

**当前实现** (`crates/app/src/main_agent/mod.rs`):
```rust
pub enum MainAgentDecision {
    Clarify { message: String },
    ExecutePlan,      // ← 这里只生成 plan
    DirectChat,
    ExternalSearch,
}
```

**问题**: 
- `MainAgent` 的 `RAG_PLAN_SYSTEM_PROMPT` 让 LLM 直接输出 `ExecutePlanRequest`
- 但 `ExecutePlanRequest` 包含 `query_entities` 和 `graph_hints`，这些是检索算子级别的输入
- **架构要求**: RAG API 负责 `query entity extraction`，Main Agent 只输出 plan schema
- **现状**: Main Agent 可能越界做了 entity extraction

**建议**: 确认 `RAG_PLAN_SYSTEM_PROMPT` 是否只输出 plan schema，不包含 entity/graph hint。如果包含，需要拆分到 RAG API 侧。

---

#### Gap 2: Milvus Schema 未完全对齐架构

**架构要求** (§2):
```
Milvus 应包含: text chunks, multimodal chunks, BM25 sparse, dense text vectors, multimodal vectors, kg_entities, kg_relations, graph passages, semantic memory vectors
```

**当前实现** (`crates/storage-milvus/src/lib.rs`):
```rust
pub struct MilvusCollectionNames {
    pub text_chunks: String,           // ✅
    pub multimodal_chunks: String,   // ✅
    pub kg_entities: String,           // ✅
    pub kg_relations: String,          // ✅
    pub graph_passages: String,        // ✅
    // ❌ 缺少: BM25 sparse collection
    // ❌ 缺少: semantic memory vectors collection
}
```

**问题**:
1. **BM25 sparse**: 架构要求 BM25 在 Milvus 中，但当前 `search_bm25` 实现可能仍依赖 PG BM25
2. **Semantic memory vectors**: 架构 §9 要求语义记忆向量存 Milvus，未见对应 collection

**验证**:
```rust
// crates/storage-milvus/src/lib.rs:657
async fn search_bm25(&self, request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput> {
    // 实现细节需确认是否真正使用 Milvus BM25 还是 PG fallback
}
```

---

#### Gap 3: Graph Relation Retrieval 实现过简

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
    
    let relation_rows = self.query_entities(
        &self.config.collection_names().kg_relations,
        filter,
        request.relation_limit,
        &RELATION_OUTPUT_FIELDS,
    ).await?;
    
    // 仅做简单查询，缺少:
    // - query entity extraction
    // - entity vector search
    // - subgraph expansion
    // - fan-out control / eviction
    // - relation/path rerank
}
```

**问题**:
- 当前实现只是简单的 relation 属性过滤查询
- 缺少架构要求的完整 pipeline
- ** fan-out limit, hop limit** 未实现

---

#### Gap 4: RAG API 降级策略不完整

**架构要求** (§11):
```
- 图抽取失败时必须降级到 BM25 + dense + multimodal retrieval
- BM25 失败时必须降级到 dense + multimodal + graph retrieval
- RAG API trace 必须显示最终上下文由哪些通道贡献
```

**当前实现** (`crates/rag-core/src/runtime/execute.rs`):
- 有 `DegradeTraceItem` 结构
- 但未见明确的通道级降级逻辑
- 各通道错误处理独立，缺少跨通道降级协调

---

#### Gap 5: ACL Filter 强制约束

**架构要求** (§11):
```
每次 Milvus 查询必须强制带服务端 ACL filter，例如 org_id, workspace_id, doc_scope
```

**当前实现**:
```rust
// crates/storage-milvus/src/lib.rs
fn doc_filter(auth: &AuthContext, doc_ids: Option<&[Uuid]>) -> Value {
    // 有 filter 构建，但需确认是否覆盖所有查询路径
}
```

**需验证**: `search_text_dense`, `search_bm25`, `search_multimodal`, `search_graph` 是否都强制带 ACL filter

---

#### Gap 6: 融合与预算实现 vs 架构要求

**架构要求** (§8):
```
1. 并行执行 BM25、text dense、multimodal dense、graph retrieval
2. 构建带 source label 和 score breakdown 的候选池
3. 使用 RRF 或 channel-aware normalization 做第一层融合
4. 对可比较的 chunk candidates 复用现有 reranker
5. 保留 graph-supported chunks 的最低预算
6. 在 token budget 内裁剪最终上下文
```

**当前实现** (`crates/rag-core/src/runtime/execute.rs`):
- ✅ 并行执行 4 通道
- ✅ 有 ChannelCandidateBudgets 预算分配
- ⚠️ RRF 融合逻辑需确认是否带 source label
- ❓ 是否保留 graph chunks 最低预算（20-30%）
- ❓ token budget 裁剪逻辑

---

### 2.3 ❌ 未实现或待确认

| 项目 | 架构要求 | 当前状态 |
|------|----------|----------|
| BM25 sparse vectors in Milvus | §2, §7.1 | 未确认是否真正使用 Milvus BM25 |
| Semantic memory vectors | §9 | 未见 collection |
| Query entity extraction | §3.1, §6 | 可能在 Main Agent 越界实现 |
| Graph subgraph expansion | §6 | 未实现 |
| Graph fan-out/hop limit | §11 | 未实现 |
| Channel 级降级协调 | §11 | 不完整 |
| RAG API trace 通道贡献 | §11 | 需验证 |
| 完整评测集 (20+ 问题) | §10 | `tests/rag_quality/` 有 sample |
| URL source 真实摄取 | PRD §8.2 | placeholder (已知问题) |

---

## 3. 前端 Gap 分析

### 3.1 ✅ 已实现（符合产品需求）

| 功能 | 状态 | 位置 |
|------|------|------|
| Workspace 三栏布局 | ✅ | `components/workspace/workspace-surface.tsx` |
| Chat SSE 流式接收 | ✅ | `lib/workspace/stream.ts` |
| Session 管理 | ✅ | workspace UI state |
| Citation 展示 | ✅ | `WorkspaceCitationModal` |
| Share 中心 | ✅ | `dashboard/[id]/share/` |
| API Access | ✅ | `dashboard/[id]/api-access/` |
| Admin 面板 | ✅ | `app/admin/` |
| 设计令牌系统 | ✅ | `app/design-tokens.css` |

### 3.2 ⚠️ 需确认

| 项目 | 说明 |
|------|------|
| Evidence/degrade UX | PRD 要求 evidence-aware UI，需确认实现完整度 |
| Session lifecycle | rename, pin, session management UX |
| Public share page | 需确认产品完整度 |
| 多语言 i18n | 当前以英文为主，中文支持需确认 |

---

## 4. 文档矛盾问题（需修正）

### 4.1 DESIGN.md — Notion 暖色调设计系统

**问题**: DESIGN.md 描述的是 Notion 风格暖色调系统（`#f6f5f4` warm white, NotionInter 字体等）

**实际前端** (`frontend_next/app/design-tokens.css`):
```css
:root {
  --background: 0 0% 100%;           /* 纯白，非暖白 */
  --foreground: 240 10% 10%;        /* 冷色调 near-black */
  --primary: 240 10% 9%;            /* 冷色调 primary */
  --workspace-border: 240 8% 86%;    /* 冷灰 border */
  /* ... 全部使用 HSL 冷色调系统 */
}
```

**结论**: DESIGN.md 完全过时，与实现不符。

**建议操作**:
1. 删除 DESIGN.md 或标记为 `ARCHIVED`
2. 将 `frontend_next/app/design-tokens.css` 作为设计系统真源
3. 如有需要，从 design-tokens.css 反向生成 DESIGN.md

---

### 4.2 其他可能过时的文档

| 文档 | 风险 | 建议 |
|------|------|------|
| `CHAT_GRAPHFLOW_MIGRATION_PLAN_2026-03-21.md` | ⚠️ 历史计划 | 标记为历史参考 |
| `frontend_rust/FRONTEND_PRD.md` | ⚠️ 可能部分过时 | 以 `frontend_next/` 实现为准 |
| `PRD_RUST.md` | ⚠️ 有修正案 | 以 2026-04-26 架构文档为准 |

---

## 5. 优先修复建议

### P0: 立即修复（影响架构正确性）

1. **确认 Main Agent 边界**: 检查 `RAG_PLAN_SYSTEM_PROMPT` 是否越界做 entity extraction
2. **验证 Milvus BM25**: 确认 `search_bm25` 是否真正使用 Milvus 还是 PG fallback
3. **强制 ACL filter**: 审计所有 Milvus 查询路径

### P1: 短期修复（影响功能完整度）

4. **Graph retrieval 完善**: 实现 subgraph expansion, fan-out control
5. **降级策略完整化**: 实现跨通道降级协调
6. **文档清理**: 归档/删除过时文档（DESIGN.md 等）

### P2: 中期完善

7. **Semantic memory collection**: 在 Milvus 中增加
8. **评测集执行**: 跑通 `tests/rag_quality/golden_set.sample.json`
9. **URL source 真实摄取**: 替换 placeholder

---

## 6. 附录：关键文件索引

### 架构真源
- `avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md`

### 后端实现
- Main Agent: `avrag-rs/crates/app/src/main_agent/mod.rs`
- RAG 执行: `avrag-rs/crates/rag-core/src/runtime/execute.rs`
- RAG 检索: `avrag-rs/crates/rag-core/src/runtime/retrieval.rs`
- Milvus 存储: `avrag-rs/crates/storage-milvus/src/lib.rs`
- Worker/摄取: `avrag-rs/bins/worker/src/main.rs`
- 配置: `avrag-rs/crates/app/src/lib_impl/config.rs`

### 前端实现（权威基准）
- 设计令牌: `frontend_next/app/design-tokens.css`
- Workspace: `frontend_next/components/workspace/workspace-surface.tsx`
- Stream: `frontend_next/lib/workspace/stream.ts`

### 需归档文档
- `DESIGN.md` — Notion 暖色调，与实现不符

---

*本报告由 大虾 🦐 生成，基于 2026-04-26 代码状态。*
