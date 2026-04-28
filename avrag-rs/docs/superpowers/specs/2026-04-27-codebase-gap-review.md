# avrag-rs 代码库全面审查报告

> 日期：2026-04-27
> 审查范围：架构对齐、PRD 差距、代码简化机会、功能 completeness
> 审查方法：4 个独立 AI agent（codex、Claude Code、forgecode、Hermes）并行审查 + 人工综合

---

## 一、评审方法与各 Agent 能力评分

| Agent | 深度 | 具体性（行级定位） | 覆盖度 | 可操作性 | 总分 |
|-------|------|-------------------|--------|----------|------|
| **codex** | 9/10 | **10/10** | 6/10 | **10/10** | **35/40** |
| **Claude Code** | 7/10 | 6/10 | **9/10** | 7/10 | 29/40 |
| **forgecode** | 8/10 | 8/10 | **9/10** | 8/10 | 33/40 |
| **Hermes** | 6/10 | 5/10 | 6/10 | 6/10 | 23/40 |

**评分说明**：
- **codex**：最强在具体性和可操作性，能精确定位到文件行号，提出的修复方案可直接执行。但覆盖的领域较窄。
- **Claude Code**：最均衡，五个重点领域都有涉及，架构视角好。但缺少精确的行级定位。
- **forgecode**：最详细，有 PRD 章节引用、工作量估计、代码简化建议。但篇幅冗余，某些结论基于可能过时的 PRD。
- **Hermes**：最简洁，聚焦核心问题。但缺少细节支撑。

**本报告以 codex 为基准，融合其他 agent 的发现，去重后综合输出。**

---

## 二、执行摘要

当前 codebase 的核心 RAG 链路（文本 dense + BM25 + multimodal + 摘要注入 + rerank + 回答合成）**基本可用**，测试覆盖良好（worker 11, rag-core 25, app 12, milvus 11+2）。但存在 **5 个 P0 级问题**直接影响正确性、成本或用户体验，以及 **8 个 P1 级问题**影响功能完整度。

**最关键的 5 个 P0 问题：**
1. 后端 router 未挂载 billing/admin API，前端设置页会 404
2. SSE 无取消链路，用户切换页面后 LLM 仍在跑并计费
3. Entity extraction 在 Main Agent 和 RAG API 各做一次，浪费 token
4. Graph retrieval 只有简单属性过滤，多跳推理不可用
5. Milvus insert 失败后 cleanup 不清理失败 collection 本身的部分写入

---

## 三、P0 问题：影响正确性 / 成本 / 安全

### P0-1: 后端 Router 缺失 billing / admin 路由

**发现者**：codex
**影响**：前端调用 `/api/v1/billing/*` 和 `/api/v1/admin/*` 会 404

| 维度 | 详情 |
|------|------|
| 前端调用点 | `frontend_next/lib/settings/client.ts:286` (billing), `frontend_next/lib/admin/client.ts:351` (admin) |
| 后端现状 | `router_core.rs:176` 只 merge 了 notebooks/chat/rag 路由 |
| 修复方案 | 在 `router_core.rs` 的 `/api/v1` merge 中加入 billing 和 admin handlers |
| 工作量 | 小（~50 行） |

### P0-2: SSE 无取消链路

**发现者**：codex + forgecode + Hermes
**影响**：用户切换页面或重新提交后，旧 SSE 流继续消耗 LLM token 和 search API 配额

| 维度 | 详情 |
|------|------|
| 前端问题 | `stream.ts:451` fetch 无 AbortSignal；`workspace-chat-pane.tsx:1631` 提交侧无 AbortController |
| 后端问题 | `chat_streaming.rs` sender 失败被忽略，tokio::spawn 无 CancellationToken |
| 修复方案 | 前端：创建 AbortController，重新提交/页面卸载时 abort；后端：spawn 返回 JoinHandle，在 SSE drop 时 cancel |
| 工作量 | 中（~100 行） |

### P0-3: Entity Extraction 重复做两次

**发现者**：forgecode
**影响**：浪费 LLM token，职责边界模糊

| 维度 | 详情 |
|------|------|
| 第一次 | Main Agent 的 `RAG_PLAN_SYSTEM_PROMPT` 输出 `query_entities` / `graph_hints` |
| 第二次 | rag-core 的 `retrieve_graph_stage` 兜底调用 `planner.extract_query_entities()` |
| 修复方案 | **方案 A**（推荐）：精简 Main Agent prompt，移除 `query_entities`，让 RAG API 统一做 extraction；**方案 B**：让 Main Agent 输出 entities，RAG API 复用不再重复提取 |
| 工作量 | 小（~50 行） |

### P0-4: Graph Retrieval 严重欠完整

**发现者**：forgecode + Hermes
**影响**：多跳推理、桥接证据完全不可用

| 维度 | 详情 |
|------|------|
| 当前实现 | `search_graph` 仅做 relation 属性过滤（subject/object/predicate），`crates/storage-milvus/src/lib.rs:1190` |
| 缺失能力 | entity vector search、relation vector search、subgraph expansion、fan-out control、rerank |
| 测试现状 | 测试甚至期望 graph 返回空（`GraphSearchOutput::default()` fallback） |
| 修复方案 | 短期：加 `fan_out_limit` + `hop_limit` 配置，防止无限制扩展；中期：完整实现 vector graph retrieval pipeline |
| 工作量 | 配置小（~30 行），完整实现大 |

### P0-5: Milvus Cleanup 不清理失败 Collection 的部分写入

**发现者**：codex
**影响**：如果 Milvus insert 在 collection 内部部分写入后返回错误，会留下 current parse_run 的半成品数据

| 维度 | 详情 |
|------|------|
| 当前语义 | `cleanup_current_parse_run` 只清理"已成功记录"的 collection（`successful` 列表），`lib.rs:451` |
| 问题 | 失败的 collection 本身可能有部分行已写入 Milvus，但不会被清理 |
| 测试断言 | 现有测试明确断言只清理 text_chunks，不清理失败的 multimodal_chunks：`lib.rs:1771` |
| 修复方案 | 扩展 cleanup 语义：清理所有已尝试的 collection（不只是成功的），使用 `parse_run_id == current` filter；或改为先 delete 再 insert 的原子语义 |
| 工作量 | 中（需改测试 + 实现） |

---

## 四、P1 问题：影响功能完整度

### P1-1: T²RAG 只打通了"placeholder_triplets -> graph filter hint"

**发现者**：codex
**现状**：`classify()` 已存在于 `common/src/rag_execute.rs:93`，但 runtime 只是把最多一个占位符的 triplet 转成 `GraphRelationHint`：`rag-core/src/runtime/execute.rs:245`。没有 vector proposition retrieval 和 clue resolution。

### P1-2: 文件上传 10MB Body Cap + 全内存处理

**发现者**：codex
**现状**：`router_core.rs:201` 有 10MB body cap；`infra_handlers.rs:164` 上传 handler 把整个 body 读入内存再写 S3。`create_document_upload` 没有显式文件大小上限/分片策略：`app/src/lib_impl/documents.rs:32`。

### P1-3: RAG 是"伪流式"

**发现者**：Hermes + forgecode + Claude Code
**现状**：
```
plan_rag_with_main_agent()     → [LLM #1, 阻塞]
  ↓
execute_rag_execute_plan()     → [检索 4 通道, 阻塞]
  ↓
answer_rag_with_main_agent_stream() → [LLM #2, 真流式]
```
用户看到的 "planning → retrieving → reading_sources → drafting_answer" 是**后端模拟的中间状态事件**（`activity` 事件），不是真正的流式。前面 3 个阶段全部阻塞完成后才发事件。

**修复方案**：
- 将 `plan_rag` 改为 streaming call，planner token 可选择性透传
- retrieval 阶段每完成一个通道就 emit `Activity` 事件
- 或至少让 planning 和 retrieval 并发，不等全部完成就开始 synthesis

### P1-4: Prompt 完全硬编码

**发现者**：forgecode + Hermes
**现状**：
- 仅 2 个 `.tmpl` 文件（summary_generation）
- `main_agent/mod.rs:19-71` 所有 RAG plan/answer/general prompt 全是 `const &str`
- `skill` 字段只是 envelope 字符串注入，无 skill catalog/registry/composition/loader
- `intent_version` 配置为 `"freeze-v2"` 但无对应模板文件

**修复方案（渐进式）**：
1. 将所有 `const &str` prompt 外置到 `.tmpl` 文件，用 `include_str!()` 加载
2. 建立 prompts 目录约定：`{name}.{version}.tmpl`
3. 后续再建 DB schema + CRUD API

### P1-5: Billing + Usage-Limit 两套独立系统

**发现者**：forgecode + Claude Code
**现状**：
- `billing` crate：Stripe 集成、plan/subscription、月度 usage_events
- `usage-limit` crate：rolling 5h/7d 限流、llm_usage_events
- 各自维护 quota 表，check_quota 逻辑独立
- `check_quota` 结果未被 chat flow 强制执行（只记录不阻止）

**修复方案**：统一为单个 crate，统一 quota 表，消除 `usage_events` vs `llm_usage_events` 重复。

### P1-6: BM25 后端未验证

**发现者**：forgecode
**现状**：架构要求 BM25 在 Milvus，但 `search_bm25` 可能仍走 PG 或 Tantivy fallback。需要确认并迁移。

### P1-7: ExecutePlanRequest ↔ ChatRequest 兼容 Hack

**发现者**：forgecode
**现状**：`to_chat_request_compat()` 存在说明字段映射不一致。建议给 `ExecutePlanRequest` 直接加 `doc_ids` 字段或统一 `doc_scope`。

### P1-8: GuardPipeline 是壳

**发现者**：forgecode + Hermes
**现状**：默认返回 `GuardResult::pass("input:all")`，输入检测是 regex 匹配（非语义分析），输出校验未确认是否真实生效。

---

## 五、P2 问题：架构/产品完善

### 可观测性（Observability）

| 能力 | 现状 | 差距 |
|------|------|------|
| 结构化日志 | 纯文本 tracing | 无 JSON 格式，无法被日志系统解析 |
| 分布式追踪 | 零实现 | 无 OpenTelemetry，无 `#[tracing::instrument]`，trace_id 始终 None |
| 错误追踪 | 零实现 | 无 Sentry/Bugsnag |
| 运行时分析 | 零实现 | 无 tokio-console |
| 前端观测 | 零实现 | 无 web-vitals、无 client error tracking |
| Dashboard | 零实现 | 无 Grafana、无告警规则 |

**建议**：先批量加 `#[tracing::instrument]` + request_id attach 到 span（投入产出比最高），再逐步引入 OTLP exporter。

### 计费统计（Billing）

| 能力 | 现状 | 差距 |
|------|------|------|
| Stripe 基础集成 | ✅ | — |
| Metered billing | ❌ | cost_events 未与 Stripe metered billing 联动 |
| Token 级成本归因 | ❌ | 一次 RAG 调用的完整成本无法端到端追踪 |
| 用量预警 | ❌ | soft limit 无邮件/通知 |
| 实时 dashboard | ❌ | rollups 是 daily |
| 计费维度 | ❌ | 无 workspace/document 级别拆分 |
| 计费补报 | ❌ | 链路异常时无容错补报 |

### 文件解析入库（Ingestion）

| 能力 | 现状 | 差距 |
|------|------|------|
| 重试机制 | ❌ | 无自动重试、无死信队列 |
| 熔断器 | ❌ | MinerU/Office parser 无 circuit breaker |
| 流式解析 | ❌ | 全内存加载，大文件 OOM |
| 进度追踪 | ❌ | 用户只看到"处理中" |
| 格式覆盖 | ⚠️ | 缺 epub、rtf、odt、email、archive |
| 文档版本 | ❌ | 无 rollback |
| URL 摄入 | ❌ | MinerU v4 需要 URL 但无直接路由 |
| 质量闸门 | ❌ | 无文本长度/OCR 置信度检查 |

### Skills & Prompts

| 能力 | 现状 | 差距 |
|------|------|------|
| Prompt registry | ❌ | 全硬编码 |
| Skill system | ❌ | 无 catalog/registry/loader |
| Prompt versioning | ❌ | 无历史/回滚 |
| A/B testing | ❌ | 无法对比不同 prompt |
| Evaluation suite | ❌ | 改 prompt 后无回归测试 |
| 用户自定义 | ❌ | 用户不能调风格 |

---

## 六、代码简化机会

### 6.1 重复/冗余

| 位置 | 问题 | 建议 |
|------|------|------|
| `billing` + `usage-limit` | 两套独立配额/用量系统 | 合并 crate，统一 quota 表 |
| `ParserFactory` + `ParseRouter` | 两个文件路由入口，功能重叠 | 统一为一个 Router |
| `NormalizedDocument` + `DocumentIr` | 两套中间模型 | 合并为统一 IR |
| `Entity extraction` x2 | Main Agent 和 RAG API 各做一次 | 归一到 RAG API |
| `request<T>()` helper xN | 每个 `lib/*/client.ts` 有自己的 fetch wrapper | 抽成 `lib/api-client.ts` |

### 6.2 死代码

| 位置 | 说明 | 建议 |
|------|------|------|
| `ChatEvent::Trace` | enum 变体存在但无 emit，前端 ignore | 实现 emit 或删除 |
| `intent_version` / `intent_template_path` | 配置存在但无对应模板文件 | 删除或补文件 |
| `ParserFactory` | 未被 worker 使用（worker 用 ParseRouter） | 删除 |
| `SummaryGenerator legacy path` | `build_chunk_plan` 用 NormalizedDocument，主路径用 `build_ir_chunk_plan` | 统一 |
| `Qdrant/Tantivy legacy` | 目标架构已切 Milvus | 确认后移除 |
| `storage-milvus/src/lib.rs` | 1500+ 行，schema/搜索/索引全在一个文件 | 拆分为 `schema.rs`、`search.rs`、`index.rs` |
| `app/src/lib_impl/chat_streaming.rs` | 30KB，过于庞大 | 拆分为 plan/retrieve/synthesize 模块 |

### 6.3 架构优化

| 问题 | 建议 |
|------|------|
| `ChatRequest::to_chat_request_compat()` hack | 给 `ExecutePlanRequest` 直接加 `doc_ids` |
| GraphFlow vs Runtime 双编排 | 确认方向：保留 GraphFlow（需加 streaming）还是统一用 Runtime |
| Dual frontend（Next.js + Leptos） | 确认最终前端技术栈，逐步废弃一套 |

---

## 七、优先级路线图

### Phase 1: 止血（1-2 周）

| # | 项 | 工作量 | 负责人 |
|---|-----|--------|--------|
| 1 | 挂载 billing/admin 后端路由 | 小 | Backend |
| 2 | SSE 取消机制（AbortController + CancellationToken） | 中 | Full-stack |
| 3 | Entity extraction 归一边界 | 小 | Backend |
| 4 | Milvus cleanup 收紧语义 | 中 | Backend |
| 5 | `#[tracing::instrument]` 批量添加 + request_id 进 span | 中 | Backend |

### Phase 2: 功能补齐（2-4 周）

| # | 项 | 工作量 |
|---|-----|--------|
| 6 | Graph retrieval 最小实现（fan_out_limit + hop_limit） | 中 |
| 7 | Prompt 外置化（`include_str!()`） | 中 |
| 8 | RAG 检索阶段 emit Activity 进度事件 | 小 |
| 9 | Billing + usage-limit 统一 | 中 |
| 10 | BM25 后端确认并迁移至 Milvus | 中 |

### Phase 3: 架构完善（1-2 月）

| # | 项 | 工作量 |
|---|-----|--------|
| 11 | Prompt management infra（DB + CRUD API） | 大 |
| 12 | Skill system（catalog + registry + composition） | 大 |
| 13 | Graph retrieval 完整 pipeline | 大 |
| 14 | OpenTelemetry / distributed tracing | 中 |
| 15 | Ingestion 重试 + DLQ + circuit breaker | 中 |

---

## 八、结论

当前 codebase 处于一个**"核心可用、外围薄弱"**的状态：

- **绿色（可用）**：文本/multimodal 检索、回答合成、文档解析、SSE 传输、Stripe 基础集成
- **黄色（可用但有债）**：Milvus 写入 cleanup 语义、BM25 后端、GraphFlow 双路径、Prompt 硬编码
- **红色（缺失）**：Graph retrieval 完整能力、Prompt/Skill 管理体系、可观测性（tracing/OTel）、流式取消、计费 metered billing

**最重要的原则**：不要在缺基础设施的情况下继续堆功能。Phase 1 的 5 项止血工作（路由 + 取消 + extraction 归一 + cleanup 收紧 + tracing）投入产出比最高，应优先完成。
