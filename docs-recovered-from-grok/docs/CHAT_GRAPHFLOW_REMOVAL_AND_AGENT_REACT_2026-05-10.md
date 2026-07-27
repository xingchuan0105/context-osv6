# Chat 层移除 GraphFlow + Agent 层引入 ReAct 状态机 — 设计稿

**日期**:2026-05-10
**作者**:Claude (与 chuan 协同设计)
**状态**:Draft,待开工
**关联**:延续 P0-1 (RagAgent tool-call 范式) 与 P0-2 (取消令牌贯通)

---

## 0. 一句话摘要

把 `graph-flow` 框架从 chat 编排层**完全删除**(降级为线性管线),同时在 agent 层(RagAgent / WebSearchAgent)**引入裸循环 + 强类型状态结构体**,实现 plan-execute-evaluate-{retry|synthesize|degrade} 的 ReAct 行为,以信号驱动(非自评)的方式提升答案下限,同时严格控制 LLM/检索调用的成本。

---

## 1. 背景与动机

### 1.1 现状

- chat 编排层使用 `graph-flow` v0.4.0 构建 11 节点 DAG:
  - `Preflight → Session → StreamSetup → ModeSelect → {Memory|General|Search|Rag}Mode → OutputGuard → Persist → Usage → Notify → BuildResponse`
- `RagModeTask` / `SearchModeTask` / `GeneralModeTask` 已是 thin-wrapper,仅调用 `agent_service.run(...)`
- agent 层(RagAgent / WebSearchAgent / ChatAgent)目前**单轮执行**,LLM 一次调用即结束

### 1.2 GraphFlow 在 chat 层的实际收益(评估结果)

| 框架承诺特性 | chat 层是否用上 |
|---|---|
| 动态分支 / 跳转 | ❌ 路径完全静态 |
| 并行节点 | ❌ 全部串行 |
| 持久化 / 恢复 | ❌ 内存内一次性执行 |
| 失败重试 | ❌ 无 |
| 状态可监测 | ❌ 没有 introspection 调用方 |
| 节点级日志 | ⚠️ 有,但 `tracing::info!` 直接写也能做到 |

**结论**:框架重量 ≫ 实际收益。引入它的理由(plan-retrieve-react 循环)其实属于 agent 层,不属于 chat 层。

### 1.3 GraphFlow 引入时的初心

> "状态可监测,图编排也可以更具备 agentic 能力"

**初心是对的,只是放错了层**:chat 是会话生命周期编排,本质静态;agent 内部的"想-做-看-再想"才需要动态状态机。本设计稿把框架移到正确的层(以更轻量的形式)。

### 1.4 为何 ReAct 在检索场景仍有价值(且不至于失控)

**核心担忧**:LLM 自评检索质量是不闭环的,可能"不知道自己不知道",自评反而引入幻觉,只能提升下限不能提升上限。

**解决方式**:不靠 LLM 自评,靠**客观信号**驱动循环决策:

| 评估信号 | 是否客观 | 用途 |
|---|---|---|
| `recall_count` (检索条数) | ✅ | 0 → escalate |
| `max_score` (最高相关性分) | ✅ | 低于阈值 → broaden |
| `term_coverage` (查询词命中比例) | ✅ | 低 → 拆词重检 |
| `zero_hits_per_subquery` | ✅ | 部分子查询完全无结果 → 替换 |
| LLM 自评"满不满意" | ❌ | **本设计禁用**,只在最末端做 synth 用 |

ReAct 在本设计中的边界:**只做客观信号能驱动的事**(替换查询、切换 vertical、抓取全文),**不做主观自评**。这样能提升下限(零结果场景能 fallback),同时不污染上限。

---

## 2. 全局设计决定(锁定)

| # | 决定 | 选择 | 理由速记 |
|---|---|---|---|
| ① | 函数 `execute_chat_graphflow` 改名 | ✅ → `execute_chat_pipeline` | 名字必须反映实际行为 |
| ② | 结构 `ChatGraphExecution` 改名 | ✅ → `ChatExecution` | 同上 |
| ③ | 死代码清理 | ✅ 一次清完 | 包含 5 个 `KEY_*` 常量 + `mod.rs` 过期注释 + `#[allow(dead_code)]` |
| ④ | ReAct 默认预算 | RAG=3 / Search=1+强 Planner | Search 重试边际收益低,投资 Planner 收益高 |
| ⑤ | `SearchFetchPage` 是否实现 | ❌ 留接口、不实现 | 等 P2-B 上线数据再决定是否值得 +2 天 |
| ⑥ | RAG 累积 / Search 重置 | ✅ 保持不对称 | 反映两种检索的本质,非 bug |
| ⑦ | "回退必须改输入"在哪强制 | ✅ 类型系统(LoopDecision schema) | 编译期防错 |
| ⑧ | ReAct 用什么实现 | ✅ 裸 `loop {}` + 状态 struct | GraphFlow 弱类型已坑过,小循环不值得引框架 |
| ⑨ | `AgentRunResult` 是否扩展 | ✅ 必须扩展 | 不扩展等于丢可观测性,违背初心 |

---

## 3. P2-A:Chat 层移除 GraphFlow

### 3.1 目标

- 删除 `crates/app/src/chat/graphflow.rs` 及 4 个 include 文件:
  - `graphflow_context.rs`
  - `graphflow_tasks.rs`
  - `graphflow_tasks_core.rs`
  - (注:`graphflow_tasks_rag.rs` 早已删除)
- 删除 `crates/app/Cargo.toml` 中的 `graph-flow.workspace = true`
- 工作区 `avrag-rs/Cargo.toml` 中的 `graph-flow = "0.4.0"` 一并删除
- `chat/mod.rs` 改为导出新的 `pipeline.rs`
- 重写 `chat/graphflow_tests.rs` → `chat/pipeline_tests.rs`(6 个测试中 5 个需重写,1 个移到别处)

### 3.2 替代结构

新增 2 个文件:

**`crates/app/src/chat/pipeline.rs`**(主流程)
```rust
pub(crate) struct ChatExecution {
    pub mode: String,
    pub input_usage_text: String,
    pub apply_output_guard: bool,
    pub response: ChatResponse,
    pub llm_usage: Option<avrag_llm::LlmUsage>,
    pub debug_metadata: Option<serde_json::Value>,
    pub tokens_emitted: bool,
    pub citations_emitted: bool,
}

pub(crate) struct ChatPreflight {
    pub trace_id: String,
    pub user_uuid: Uuid,
    pub notebook_uuid: Option<Uuid>,
}

pub(crate) async fn execute_chat_pipeline(
    state: AppState,
    req: ChatRequest,
) -> Result<ChatResponse, AppError>;

pub(crate) async fn execute_chat_pipeline_stream(
    state: AppState,
    req: ChatRequest,
    request_id: String,
    sender: UnboundedSender<ChatEvent>,
    token: CancellationToken,
) -> Result<(), AppError>;
```

**`crates/app/src/chat/pipeline_steps.rs`**(每个阶段一个 free function)
```rust
async fn preflight(state: &AppState, req: &ChatRequest) -> Result<ChatPreflight, AppError>;
async fn ensure_session(state: &AppState, req: &ChatRequest, preflight: &ChatPreflight) -> Result<ChatSession, AppError>;
async fn dispatch_mode(state: &AppState, req: &ChatRequest, session: &ChatSession, ...) -> Result<ChatExecution, AppError>;
async fn apply_output_guard(state: &AppState, exec: &mut ChatExecution) -> Result<(), AppError>;
async fn persist_response(state: &AppState, exec: &ChatExecution, session: &ChatSession) -> Result<(), AppError>;
async fn record_usage(state: &AppState, exec: &ChatExecution) -> Result<(), AppError>;
async fn dispatch_notifications(state: &AppState, exec: &ChatExecution) -> Result<(), AppError>;
async fn build_response(exec: ChatExecution, stream_cfg: Option<&StreamConfig>) -> ChatResponse;
```

调用方就是一段普通的 async 直线:
```rust
let preflight = preflight(&state, &req).await?;
let session = ensure_session(&state, &req, &preflight).await?;
let mut exec = dispatch_mode(&state, &req, &session, &preflight).await?;
if exec.apply_output_guard {
    apply_output_guard(&state, &mut exec).await?;
}
persist_response(&state, &exec, &session).await?;
record_usage(&state, &exec).await?;
dispatch_notifications(&state, &exec).await?;
Ok(build_response(exec, stream_cfg))
```

### 3.3 必须保留的行为

| 行为 | 当前实现位置 | 迁移后位置 |
|---|---|---|
| Stream `Start` 事件在 Session 创建后立刻发 | `StreamSetupTask` | `execute_chat_pipeline_stream` 在 `ensure_session` 后直接发 |
| Clarify 模式短路 | `ModeSelectTask::route` | `dispatch_mode` 内的 if-let |
| Memory 模式特殊路径(legacy adapter) | `MemoryModeTask` | `dispatch_mode` 内的 match arm,调用现有 `execute_memory_chat_compat` |
| Share-source 跳过 persist/usage | 各 task 的 guard | 阶段函数顶部的 `if session.is_share_token_session() { return Ok(()) }` |
| Token / Citation 终态事件去重 | `tokens_emitted` / `citations_emitted` flags | 同样保留这两个字段 |
| 错误的 `AppError` 语义保留 | `FlowAppErrorData` 桥接 | **直接用 `AppError`,无需桥接**(这是删掉框架的关键收益之一) |

### 3.4 调用方更新

| 调用点 | 当前 | 改为 |
|---|---|---|
| `services/chat_service.rs:41` | `state.execute_chat_graphflow(req)` | `state.execute_chat_pipeline(req)` |
| `lib_impl/chat_streaming.rs:106` | `chat::execute_graphflow_chat_stream(...)` | `chat::execute_chat_pipeline_stream(...)` |

### 3.5 测试迁移

`graphflow_tests.rs` 中 6 个测试的处置:

| 测试 | 处置 | 理由 |
|---|---|---|
| `mode_select_routes_memory_runtime_to_canonical_agent_task` | **重写** → 测 `dispatch_mode` 直接返回 search 路径 | 验逻辑而非 task |
| `mode_select_keeps_memory_rag_compat_for_memory_adapters` | **重写** → 测 `dispatch_mode` 走 memory 路径 | 同上 |
| `app_error_roundtrip_preserves_code_and_status` | **删除** | `FlowAppErrorData` 一起删,测试无意义 |
| `normalize_rag_plan_injects_original_query_as_text_dense_item` | **保留并迁移** → 移到 `lib_impl/rag_execute.rs` 单测 | 测的是 `ExecutePlanRequest::ensure_*`,与 chat 层无关 |
| `build_response_task_persists_final_chat_response` | **重写** → 测 `build_response` free function | 同等覆盖 |
| `graph_builder_contains_all_chat_tasks` | **删除** | 验证图构建,框架删了就没意义 |

### 3.6 验收标准

- `cargo check --workspace` 干净
- `cargo test -p app` 全绿
- `grep -r "graph_flow\|GraphFlow\|graphflow" crates/app/src/` 应**完全无命中**(除了 git 历史)
- `crates/app/Cargo.toml` 不再依赖 `graph-flow`
- 工作区 `Cargo.toml` 不再声明 `graph-flow` workspace 项

---

## 4. P2-B:Agent 层引入 ReAct 状态机

### 4.1 通用骨架

**位置**:`crates/app/src/agents/react_loop.rs`(新增)

```rust
pub trait ReactStep<S> {
    async fn execute(&self, state: &mut S, ctx: &ReactContext) -> Result<StepOutcome, AppError>;
}

pub struct ReactContext<'a> {
    pub sink: &'a SseSink,
    pub cancel: &'a CancellationToken,
    pub trace_id: &'a str,
}

pub struct LoopBudget {
    pub max_iterations: u8,
    pub current: u8,
}

pub enum LoopDecision {
    Continue {
        next_step: NextStep,
        // ⑦ 类型系统强制:回退必须提供新参数
        new_params: ReactParams,
        reason: &'static str,
    },
    Synthesize,
    Degrade { reason: DegradeReason },
    Clarify { question: String },
}

pub enum NextStep {
    Replan,
    BroadenQuery,
    EscalateVertical,   // search-only
    EscalateToSearch,    // rag → search
    FetchFullPage,       // search-only, P2-B v1 仅留接口
}
```

### 4.2 共享评估器

**位置**:`crates/app/src/agents/evaluator.rs`(新增)

纯函数,无副作用:

```rust
pub struct EvaluationSignals {
    pub recall_count: usize,
    pub max_score: f32,
    pub term_coverage: f32,
    pub zero_hits_per_subquery: Vec<String>,
}

pub fn evaluate_rag_iteration(
    signals: &EvaluationSignals,
    budget: &LoopBudget,
    accumulated: &AccumulatedRagResults,
) -> LoopDecision;

pub fn evaluate_search_iteration(
    signals: &EvaluationSignals,
    budget: &LoopBudget,
    last_results: &[SearchHit],
) -> LoopDecision;
```

阈值(初版,可调):

| 信号 | RAG 阈值 | Search 阈值 |
|---|---|---|
| `recall_count == 0` | broaden 或 escalate | 切 vertical |
| `max_score < 0.30` | broaden bm25 | broaden query |
| `term_coverage < 0.50` | replan | 切关键词 |

### 4.3 RagAgent 状态机

**位置**:重构 `crates/app/src/agents/rag_agent.rs`

```text
            ┌─────────┐
       ┌───▶│  Plan   │
       │    └────┬────┘
       │         ▼
       │   ┌──────────────┐     budget exhausted
       │   │ ExecuteTools │────────────────────┐
       │   └──────┬───────┘                    │
       │          ▼                            │
       │    ┌──────────┐                       │
       └────│ Evaluate │──── synthesize ──┐    │
            └─────┬────┘                  ▼    ▼
                  │ clarify          ┌────────────┐
                  ▼                  │  Synthesize│ ── or ── Degrade
            ┌──────────┐             └────────────┘
            │ Clarify  │
            └──────────┘
```

**节点职责**:
- `Plan`:复用现有 `RetrievalPlanner`,生成多子查询
- `ExecuteTools`:复用 P0-1 的 tool-call 路径,产生 `Vec<ToolResult>`
- `Evaluate`:调 `evaluate_rag_iteration`
- `Synthesize`:复用 `synthesize_from_tool_results`(已有)
- `Clarify`:在零结果且其他 fallback 都试过后触发,问用户

**累积语义**:`AccumulatedRagResults` 在每次 `ExecuteTools` 后 merge,`Synthesize` 用累积结果一次合成。

### 4.4 WebSearchAgent 状态机

**位置**:重构 `crates/app/src/agents/web_search_agent.rs`

```text
   ┌────────────────────┐
   │ Plan (multi-query) │   <-- ④ 投资点:planner 一次给 3-5 条互补子查询
   └─────────┬──────────┘
             ▼
   ┌──────────────────┐
   │  ExecuteBrave    │   (single vertical, all subqueries in parallel)
   └─────────┬────────┘
             ▼
   ┌────────────┐
   │  Evaluate  │── synthesize ──▶ Synthesize
   └─────┬──────┘
         │
         ├── EscalateVertical (general → news / discussions)
         ├── BroadenQuery (drop modifier)
         ├── FetchFullPage (P2-B v1 stub)
         └── Degrade
```

**预算**:默认 1 次主迭代 + 1 次 escalate(共 2 次 Brave 调用上限)。

**为什么 Search 累积语义不同**:每次 escalate 会换 vertical 或换查询,旧结果的 URL 重叠会污染排序。所以 Search 是"最近一轮 + 历史 URL 黑名单"模式,而非全量 merge。

### 4.5 `AgentRunResult` 扩展(决定 ⑨)

```rust
pub struct AgentRunResult {
    // 既有字段...
    pub answer: String,
    pub citations: Vec<Citation>,

    // 新增
    pub iterations: Vec<IterationRecord>,
    pub total_tool_calls: u32,
    pub total_tokens: TokenUsage,
    pub degrade_trace: Vec<DegradeTraceItem>,
    pub final_decision: FinalDecision,
}

pub struct IterationRecord {
    pub iteration: u8,
    pub plan: serde_json::Value,
    pub signals: EvaluationSignals,
    pub decision: String,  // serialized LoopDecision
    pub elapsed_ms: u64,
}
```

迁移成本:大约 10-20 个调用方需要适配,但大部分只需 `.iterations.last()` 即可保持旧行为。

### 4.6 流式事件契约

| 阶段 | 发出的 SSE 事件 |
|---|---|
| Plan | `Activity { kind: "planning", text: "..." }` |
| ExecuteTools | `ToolCall { ... }` + `ToolResult { ... }`(P0-1 已有) |
| Evaluate | `Activity { kind: "evaluating", text: "..." }` |
| Replan / Broaden / Escalate | `Activity { kind: "retrying", text: "<reason>" }` |
| Synthesize | `MessageDelta { ... }`(只在此阶段产生) |
| Degrade | `DegradeNotice { reason: ... }` |

---

## 5. 已识别风险(自我审查)

| # | 风险 | 缓解 |
|---|---|---|
| R1 | 评估器阈值首版调不准,导致过度 replan 烧成本 | 默认预算极保守(Search=1, RAG=3),阈值通过环境变量可调 |
| R2 | 累积 RAG 结果跨轮去重后仍有同 chunk 不同 score 的混乱 | `AccumulatedRagResults` 用 `(doc_id, chunk_id)` 去重,保留最高分 |
| R3 | 多轮的取消令牌响应延迟 | 每个 Step 入口 `if cancel.is_cancelled() { return Err(...) }`(P0-2 模式) |
| R4 | Streaming 中 `Activity` 噪音过多 | 只在状态转换时发,不在每个内部步骤发 |
| R5 | `AgentRunResult` schema 变更冲击下游 | 新字段全部 `#[serde(default)]`,旧调用方零修改 |

---

## 6. 不在本次范围(Out of Scope)

- ❌ 跨 provider 搜索(Brave + Bing + Google):成本翻倍,无 ROI
- ❌ `SearchFetchPage` 完整实现:留接口
- ❌ chat agent(对话型)加 ReAct:对话无检索,不需要
- ❌ ReAct 的图形化 UI:用 `Activity` 事件就够
- ❌ 持久化中间状态:本次 ReAct 仅在单次请求内活
- ❌ 把 chat layer 改异步并行:静态线性已足够,改并行 ROI 低

---

## 7. 实施顺序

```
P2-A:删 GraphFlow + 改名
  └── 验证 cargo check / test 干净
       └── P2-B:引 ReactLoop / Evaluator
            └── 重构 RagAgent 用 ReAct
                 └── 重构 WebSearchAgent 用 ReAct
                      └── 扩展 AgentRunResult
                           └── 全链路 e2e 验证
```

P2-A 与 P2-B 之间**严格串行**(P2-B 依赖 P2-A 的结果类型清理)。

---

## 8. 验收

P2-A 完成后:
- [ ] `grep -r "graph_flow" crates/app/` 无命中
- [ ] `cargo test -p app` 全绿
- [ ] 实测一次 chat,行为与改造前一致(stream + non-stream 各一)

P2-B 完成后:
- [ ] RAG 零结果 query 触发 broaden,日志可见
- [ ] Search 单轮主流程不退化
- [ ] `AgentRunResult.iterations.len()` 在多轮场景下 > 1
- [ ] 取消令牌在循环中段触发能在 100ms 内返回
- [ ] cargo check / test / clippy 全绿
