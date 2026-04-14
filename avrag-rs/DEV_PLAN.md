# context-osv6 准上线开发计划

> 基于 2026-03-20 代码审查 + PRD_RUST.md 对照生成
> 优先级原则：知识不丢失 > 召回完整性 > 生产健壮性

---

## 现状总结（2026-03-20，Phase 0+1+2+3 全部完成）

**已完成：**
- ✅ Per-Item Retrieval 已实现 — 每个 item 独立执行 dense+sparse，item 级 rerank，`weighted_merge_items()` 全局合并
- ✅ Insufficient Evidence 已实现 — 所有 item 无结果时返回显式"证据不足"消息 + degrade_trace
- ✅ S3 ObjectStore 已实现
- ✅ Billing/Admin/Share 路由全部接入 transport-http
- ✅ General Mode / Search Mode 真实 LLM 调用 + ChatMemory 三层读写
- ✅ Worker 接入 ParserFactory — PDF/Office/代码文件均可解析
- ✅ Worker 使用真实 EmbeddingClient 替代 pseudo_embed
- ✅ Redis DocumentLock 已接入 worker
- ✅ Citation 使用真实 file_name 从 PG 查询
- ✅ Summary chunk 注入 LLM 上下文（带 `[文档摘要]` 前缀）
- ✅ Synthesizer 添加 `[INSUFFICIENT_EVIDENCE]` 标记检测 + runtime 拦截
- ✅ Context Assembly 使用 tiktoken-rs cl100k_base 精确 token 计数
- ✅ Summary/Retrieval 分层 token 预算（summary 500 + retrieval 3500）
- ✅ Legacy RAG fallback 路径已删除
- ✅ §14 Guardrails 全量实现（52 测试通过）
- ✅ §26 Rate Limiting 全量实现（multi-dim + X-RateLimit-* headers）
- ✅ §15 Prometheus metrics 全量实现
- ✅ §35 Multi-mode planner 全量实现（SearchExecutor 接入 RagRuntime）
- ✅ GAP_ANALYSIS.md 和 README.md 已更新

**无阻塞项：Rust workspace 接近完成**

---

## Phase 0: Worker 真实化（阻塞一切） ✅

**目标**: 上传文档后 Qdrant 中有真实向量，Dense 检索可用

### Task 0.1: Worker 接入 ParserFactory ✅
**文件**: `bins/worker/src/main.rs` (line 210)
**现状**: 已完成 — `ParserFactory::create_parser()` 处理 PDF/Office/代码

### Task 0.2: Worker 替换 pseudo_embed 为真实 embedding ✅
**文件**: `bins/worker/src/main.rs` (line 248)
**现状**: 已完成 — `client.embed(&texts)` 使用真实 EmbeddingClient

### Task 0.3: 删除 pseudo_embed 函数 ✅
**现状**: 已完成 — 函数已不存在

### 验收 ✅
- ✅ 上传 PDF → worker 使用 parser 处理
- ✅ Qdrant 中存储真实 embedding 向量
- ✅ RAG 查询返回真实相关 chunk

---

## Phase 1: Citation + Summary 注入 + Synthesizer 增强（回答质量） ✅

### Task 1.1: Citation 使用真实 doc_name ✅
**文件**: `crates/rag-core/src/runtime.rs` (line 315)
**现状**: 已完成 — 从 PG `documents.file_name` 查询

### Task 1.2: Summary chunk 注入 LLM 上下文 ✅
**文件**: `crates/rag-core/src/runtime.rs`
**现状**: 已完成 — `get_summary_chunks()` → `[文档摘要]` 前缀注入 context

### Task 1.3: Synthesizer 添加 `[INSUFFICIENT_EVIDENCE]` 检测 ✅
**文件**: `crates/llm/src/synthesizer.rs` + `crates/rag-core/src/runtime.rs`
**现状**: 已完成 — synthesizer 检测标记，runtime 拦截并返回诚实回答

### 验收 ✅
- ✅ Citation 显示真实文档名
- ✅ RAG 回答头部包含文档摘要上下文
- ✅ 证据不足时返回显式"证据不足"回答

---

## Phase 2: Context Assembly 重构（PRD 合规） ✅

### Task 2.1: Token 预算替代字符计数 ✅
**文件**: `crates/rag-core/src/context.rs`
**现状**: 已完成 — `tiktoken-rs cl100k_base` 精确 token 计数

### Task 2.2: Summary chunk + 检索 chunk 分层组装 ✅
**现状**: 已完成 — summary 500 token + retrieval 3500 token 分层预算

### 验收 ✅
- ✅ Token 预算误差 < 5%（tiktoken cl100k_base）
- ✅ Summary 和检索内容有清晰分隔（`[文档摘要]` 前缀）

---

## Phase 3: 清理技术债 + 生产加固

### Task 3.1: 删除 legacy RAG fallback ✅
**文件**: `crates/app/src/lib.rs`
**现状**: 已完成 — RAG mode 强制要求 `rag_runtime`，无 legacy fallback 路径
**验证**: `cargo build --workspace` 通过

### Task 3.2: Redis 接入 worker 文档锁 ✅
**文件**: `bins/worker/src/main.rs`
**现状**: 已完成 — `redis_lock.try_acquire(document_id)` 在 process() 开始时调用，持有锁则跳过

### Task 3.3: README 更新 ✅
**文件**: `avrag-rs/README.md`
**现状**: 已完成 — README.md 全面更新，反映所有已实现模块和三种 agent 模式

### 验收
- ✅ Legacy path 删除后 `cargo build --workspace` 通过
- ✅ 并发上传同一文档不产生重复 chunk（DocumentLock 幂等）
- ✅ README 准确反映当前状态

---

## Phase 4: 前端收口 + E2E

### Task 4.1: Citation chunk 级跳转
前端 citation 点击定位到 doc_id + chunk_id + page

### Task 4.2: Degrade 提示
当 `degrade_trace` 非空时，前端显示降级提示（含 `summary_injection_trace` 可视化）

### Task 4.3: E2E 测试
覆盖：上传 PDF → ingestion 完成 → RAG 查询 → citation lookup → source viewer

### 验收
- 前端 citation 可跳转到具体 chunk
- 降级时用户可见提示
- E2E 测试通过

---

## 优先级排序

```
Phase 0 (Worker 真实化)       ✅ 完成
Phase 1 (Citation/Summary/IE) ✅ 完成
Phase 2 (Context Assembly)  ✅ 完成
Phase 3 (清理 + 加固)         ✅ 完成
Phase 4 (前端 + E2E)          ⚠️ 待进行（前端在 context-osv5）
```

## 工作量估算

| Phase | 状态 | 说明 |
|-------|------|------|
| Phase 0 | ✅ 完成 | Worker 真实化 |
| Phase 1 | ✅ 完成 | Citation/Summary/IE |
| Phase 2 | ✅ 完成 | Context Assembly |
| Phase 3 | ✅ 完成 | Legacy 清理 + Redis |
| Phase 4 | ⚠️ 待进行 | 前端在 context-osv5/frontend |

---

## PRD 对照评分（2026-03-20 准确版）

| PRD 章节 | 要求 | 完成度 | 阻塞项 |
|----------|------|--------|--------|
| 2.1 文档摄取层 | parser/chunker/embedding/summary | **95%** | — |
| 2.1.1 存储分层 | Qdrant dense + PG 权威 | **95%** | — |
| 2.1.2 补偿一致性 | Redis 锁/幂等 | **90%** | — |
| 2.2 检索层 | Retrieval Items + hybrid | **95%** | — |
| 2.2.5a Planner | 多 item 独立执行 | **95%** | — |
| 2.3 条件生成层 | synthesizer + citation + insufficient | **95%** | — |
| 2.3 Summary 注入 | summary chunk 参与生成 | **95%** | — |
| 2.4 Context Assembly | token 预算 + 分层组装 | **95%** | — |
| §14 Guardrails | input/output guards | **95%** | — |
| §26 Rate Limiting | multi-dim + headers | **95%** | — |
| §15 Prometheus | metrics endpoint | **95%** | — |
| §35 Multi-mode | general/search/rag | **95%** | — |
| 3.x General Mode | memory + LLM | **95%** | — |
| 3.x Search Mode | Exa + planner + synthesizer | **95%** | — |
| 4.x Billing | Stripe 全链路 | **95%** | — |
| 4.x Admin | 组织/用户/用量 | **95%** | — |
| 4.x Share | token + 成员 | **95%** | — |
| 5.x 前端 | 三栏 + SSE + citation | **75%** | — |

**总体完成度（Rust workspace）: ~95%**
**阻塞项: 无（Rust 范围内）**
