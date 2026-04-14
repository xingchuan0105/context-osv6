# RAG 检索标准化改造计划

> 状态: Draft for Review
> 关联文档: /home/chuan/context-osv6/PRD_RUST.md
> 执行原则: 先审阅计划，再按阶段实施；当前仅更新文档，不执行代码改造。

目标：
将当前 RAG 检索、摘要、元数据与规划机制，统一收敛到新的标准：
- Planner 精简 schema
- query / bm25_terms / summary 三选一载荷
- 文档摘要与文档元数据分层生成
- 文档元数据注入 Planner
- 固定执行 Text Dense + Multimodal Dense
- BM25 + Text Dense 先做 RRF
- Text Pool 与 Multimodal Pool 再做 qwen3-vl-rerank
- 候选总预算 100
- 双阈值最终保留机制
- 在线 RAG 主链全部走 GraphFlow

---

## 0. 需要你确认的决策点

1. 是否确认新 Planner schema 只保留以下字段：
- plan_version
- plan_confidence
- clarify_needed
- clarify_message
- items[].priority
- items[].query
- items[].bm25_terms
- items[].summary

2. 是否确认每个 item 只能三选一载荷：
- query
- bm25_terms
- summary(all|related)

3. 是否确认删除旧设计：
- item_type
- retrieval_mode
- purpose
- summary_signal
- synonyms
- meta_filter
- include_visual
- needs_visual_evidence
- visual_queries
- text_queries
- summary_only
- metadata_only

4. 是否确认候选预算标准：
- 总预算 100
- priority 为 0~1 权重
- 归一化后分配 item candidate budget

5. 是否确认最终裁剪标准：
- score >= 0.7 全部保留
- 若不足 30 个，则补足到 30 个

6. 是否确认在线 rag 主链必须全部迁到 GraphFlow 节点上，不再允许 runtime 内部散落式编排。

---

## 1. Phase 1: Schema 与 Prompt 收缩

目标：先统一契约，避免旧字段与新逻辑并存。

### 任务 1.1: 重构 Planner Schema
涉及文件：
- crates/common/src/lib.rs
- frontend_rust/crates/web-sdk/src/lib.rs
- crates/llm/src/planner.rs
- crates/rag-core/src/runtime.rs
- crates/llm/src/synthesizer.rs

步骤：
- 删除旧字段与旧枚举语义。
- 定义新的 RagRetrievalPlan / RetrievalPlanItem。
- 确保前后端共享类型同步更新。
- 修正 planner_output 与 trace 输出结构。

验收标准：
- 编译通过
- 旧字段在主链中不再被消费
- planner 返回结构与 PRD 一致

### 任务 1.2: 重写 Planner Prompt
涉及文件：
- crates/llm/src/planner.rs
- prompts/ 或等效模板位置（如后续外置）

步骤：
- 重写 system prompt，显式声明 runtime 固定执行策略。
- 注入 docscope metadata index。
- 明确三选一载荷约束。
- 移除 visual route / retrieval_mode / metadata_only / summary_only 等旧规则。

验收标准：
- prompt 中不再出现旧字段
- planner 单元测试补全
- planner 输出可以被严格解析为新 schema

### 任务 1.3: 重写 Summary Prompt
涉及文件：
- crates/llm/src/summary.rs
- prompts/summary_generation*.tmpl

步骤：
- 摘要生成同时产出 summary_text 与 summary_metadata。
- summary_metadata 至少包含：doc_id / filename / docname / language / domain / genre / era。
- 输出使用结构化 schema，而不是自由文本补充说明。

验收标准：
- summary 模块测试通过
- 能稳定解析摘要正文与 metadata

---

## 2. Phase 2: 摘要与元数据落库

目标：让文档摘要和文档级 metadata 成为可查询的权威工件。

### 任务 2.1: 定义摘要与元数据持久化结构
涉及文件：
- crates/storage-pg/src/lib.rs
- migrations/*
- crates/common/src/lib.rs

建议：
- 若复用现有 summary chunk，可为其增加结构化 metadata 列。
- 或新增专门 document_summary_artifacts 表。

要求：
- 能按 doc_id 读取 summary_text。
- 能按 doc_id 读取 summary_metadata。
- filename/docname/language/domain/genre/era 必须可单独访问。

### 任务 2.2: Worker 接入摘要与元数据写入
涉及文件：
- bins/worker/src/main.rs
- crates/ingestion/src/*
- crates/storage-pg/src/lib.rs

步骤：
- 文档解析完成后生成 summary artifact。
- 将 summary_text 与 summary_metadata 一起落库。
- 与 doc_id 绑定。

验收标准：
- 任意文档都能查到对应 summary_text + summary_metadata
- 历史文档与新文档都可通过统一接口读取

---

## 3. Phase 3: Planner 输入改造

目标：让 planner 真正消费 docscope metadata，而不是盲猜语言与领域。

### 任务 3.1: 构建 DocScope Metadata Index
涉及文件：
- crates/app/src/lib.rs
- crates/app/src/chat/service.rs
- crates/chatmemory/src/lib.rs（如需要）
- crates/storage-pg/src/lib.rs

步骤：
- 在 query 进入 planner 前，按 doc_scope 加载文档级 metadata。
- 生成 docscope_profile 聚合视图：languages / domains / genres / eras。
- 将 documents 列表与聚合视图一起注入 planner。

验收标准：
- planner 在 prompt 侧能看到 docscope metadata index
- doc_scope 为空时，不再默认沉默走全库检索；按产品规则处理

### 任务 3.2: 文件名与内容源标题支持
涉及文件：
- crates/storage-pg/src/lib.rs
- crates/llm/src/planner.rs

步骤：
- 确保 filename 与 docname 都能进入 planner 输入。
- 支持 planner 基于 filename 生成 lexical 词项，基于 docname 生成语义 query。

验收标准：
- 文件名指向型问题与标题指向型问题都能被规划器区分处理

---

## 4. Phase 4: Runtime 检索链路重构

目标：把执行层彻底收敛到新标准，不再依赖旧 runtime 分支语义。

### 任务 4.1: 删除旧检索模式机制
涉及文件：
- crates/rag-core/src/runtime.rs
- crates/rag-core/src/retrieval.rs
- crates/rag-core/src/context.rs

步骤：
- 删除 summary_only / metadata_only 执行路径。
- 删除 query_requires_visual_evidence 等旧 heuristic。
- 删除基于 include_visual 的条件执行分支。

验收标准：
- runtime 主链不再依赖旧字段
- 没有双机制并存

### 任务 4.2: 固化新召回链路
涉及文件：
- crates/rag-core/src/runtime.rs
- crates/rag-core/src/retrieval.rs
- crates/llm/src/embedding.rs
- crates/llm/src/reranker.rs

标准：
- query item -> Text Dense + Multimodal Dense
- bm25_terms item -> BM25 Sparse
- 汇总全部 BM25 + Text Dense -> RRF -> text_pool
- 汇总全部 Multimodal Dense -> multimodal_pool
- text_pool + multimodal_pool -> qwen3-vl-rerank

验收标准：
- runtime 行为与 PRD 完全一致
- 没有 planner 级视觉路由开关残留

### 任务 4.3: 候选预算与双阈值裁剪
涉及文件：
- crates/rag-core/src/runtime.rs

步骤：
- 总候选预算固定为 100。
- priority 视为权重而非排序号。
- 实现 item 预算归一化分配。
- 实现双阈值 final cut：score>=0.7 全留，不足 30 补足到 30。

验收标准：
- 预算分配 trace 可观测
- 最终保留逻辑符合 PRD

### 任务 4.4: Summary 注入改造
涉及文件：
- crates/rag-core/src/runtime.rs
- crates/rag-core/src/context.rs
- crates/llm/src/synthesizer.rs

步骤：
- 删除 runtime 里
