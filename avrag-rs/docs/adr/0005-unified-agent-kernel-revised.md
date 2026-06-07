# ADR-0005-revised: 基于 v5 的增量扩展 — 阶段级工具动态加载与跨 Mode 历史共享

| 项目 | 内容 |
|---|---|
| 状态 | 审议中（替代原 ADR-0005） |
| 决策日期 | 2026-06-06 |
| 提出者 | AI 助手（与用户共同决策） |
| 影响范围 | `crates/app/src/agents/unified/mod.rs`、`crates/app/src/agents/strategy/*.rs`、`crates/app/src/agents/strategy/prompts.rs` |

---

## 1. 背景与动机

### 1.1 承认 v5 现状

当前 `avrag-rs` v5 架构已经实现了以下设施：

- **`UnifiedAgent`**（`unified/mod.rs`）：按 `request.kind` 统一分发到 Chat / RAG / Search 三种 Strategy；
- **`StrategyExecutor`**（`strategy/executor.rs`）：通用状态机驱动器，带 cancellation、budget guard、trace span、audit record、replay snapshot；
- **`CapabilityRegistry`**（`capability/registry.rs`）：统一工具/技能注册表，已支持 `plan_tools(strategy)` 和 `answer_format_skills(strategy)` 的阶段级查询；
- **`State` / `StepOutcome`**（`strategy/mod.rs`）：状态机抽象，支撑 `replay.rs`（26KB）和 `eval_framework.rs`（56KB）的细粒度断点采集；
- **`events.rs`**：统一的 `AgentEvent` 枚举（15+ 变体），SSE 事件已统一。

**原问题陈述的修正**：问题**不是**"三条独立链路各自拥有独立策略实现"，而是：

1. **工具加载硬编码**：三个 Strategy 的 plan/answer 阶段工具列表部分硬编码，未充分利用 `CapabilityRegistry` 的阶段级查询能力；
2. **Answer 阶段不调工具**：三个 Strategy 的 `Answer` state 目前只调 LLM 合成文本，**不暴露格式工具**给 LLM（如 html-renderer、ppt-generation）；
3. **跨 mode 历史未共享**：`UnifiedAgent::run` 分发前未统一处理历史，各 Context 各自构造，Chat→RAG 切换时上下文断链；
4. **前端 SSE 事件名不统一**：`rag.rs` 发 `answer.format_skills`，而 `events.rs` 已有 `PlanDecision` / `ToolResult` / `Evaluation` 等通用变体。

### 1.2 目标

在**完全保留** v5 基础设施的前提下，通过**增量扩展**解决上述 4 个问题。

**非目标**：
- 不替换 `StrategyExecutor`（不动 cancellation/budget/trace/audit/replay）；
- 不替换 `State` 抽象（不动 `replay.rs` / `eval_framework.rs` 的 state 边界采集）；
- 不引入 Rig / GraphFlow（理由见 §7 替代方案）；
- 不改前端 SSE 协议的事件名（复用现有 `AgentEvent` 变体）。

---

## 2. 决策

### 2.1 核心决策

采用 **"v5 增量扩展"** 方案：

1. **UnifiedAgent 层**：分发前统一注入跨 mode 共享历史（只取 user 角色，`[prior_user_query]` 前缀）；
2. **Strategy plan step**：统一查 `CapabilityRegistry::plan_tools(strategy_id)` 加载 Plan 阶段工具，替代硬编码列表；
3. **Strategy answer step**：扩展支持 tool calls，查 `CapabilityRegistry::answer_format_skills(strategy_id)` 加载格式工具；
4. **events.rs**：如需新事件，**加** enum 变体，不改现有事件名。

### 2.2 保留的设施清单

| 设施 | 文件 | 处理方式 |
|---|---|---|
| `StrategyExecutor` | `strategy/executor.rs` | **完全复用**，不改 |
| `State` / `StepOutcome` | `strategy/mod.rs` | **完全复用**，不改 |
| `TraceSpan` / `AgentTrace` | `runtime.rs` | **完全复用**，不改 |
| `ReplaySnapshot` / `CapturedRunResult` | `replay.rs` | **完全复用**，不改 |
| `EvalRun` / `EvalCase` / `EvalScore` | `eval_framework.rs` | **完全复用**，评估影响见 §6 |
| `CapabilityRegistry` | `capability/registry.rs` | **复用查询接口**，确保三个 Strategy 都用 |
| `AgentEvent` 枚举 | `events.rs` | **复用现有变体**，仅扩展 |

---

## 3. 具体改造点

### 3.1 改造点 A：UnifiedAgent 跨 mode 历史注入

**文件**：`crates/app/src/agents/unified/mod.rs`

**当前问题**：`UnifiedAgent::run` 分发前未处理 `request.messages`，各 Strategy 的 `Context::from_request` 各自决定是否注入历史。RagContext 只注入 `session_summary`，ChatContext 未注入，导致跨 mode 切换时上下文断链。

**改造**：在 `run()` 分发前，统一过滤历史并注入到各 Context。

```rust
// 在 UnifiedAgent::run 中，match request.kind 之前加入：
let cross_mode_history = filter_cross_mode_history(&request.messages);
```

```rust
/// 跨 mode 共享历史过滤策略：
/// - 只保留 role = user 的消息（agent 的推理和答案不进历史，避免污染）
/// - 加 [prior_user_query] 前缀，明确告诉 LLM "这是历史查询，不是当前证据"
pub fn filter_cross_mode_history(history: &[avrag_llm::ChatMessage]) -> Vec<avrag_llm::ChatMessage> {
    history
        .iter()
        .filter(|m| m.role == "user")
        .map(|m| avrag_llm::ChatMessage {
            role: "user".to_string(),
            content: format!("[prior_user_query] {}", m.content),
            ..m.clone()
        })
        .collect()
}
```

**注入方式**：各 `Context::from_request` 接收 `cross_mode_history` 参数，在构造初始 `history` 时追加：

```rust
// RagContext::from_request
pub fn from_request(
    request: AgentRequest,
    trace_id: String,
    budget: LoopBudget,
    sink: Box<dyn AgentEventSink>,
    cancel: CancellationToken,
    rag_runtime: Arc<avrag_rag_core::RagRuntime>,
    cross_mode_history: Vec<avrag_llm::ChatMessage>,  // 新增参数
) -> Result<Self, AppError> {
    // ... 现有逻辑 ...
    if !cross_mode_history.is_empty() {
        history.extend(cross_mode_history);
    }
    // ...
}
```

同理修改 `ChatContext::from_request` 和 `SearchContext::from_request`。

**工作量**：~30 行（UnifiedAgent）+ 3 × ~5 行（各 Context）。

---

### 3.2 改造点 B：Plan 阶段统一查 CapabilityRegistry

**文件**：`crates/app/src/agents/strategy/rag.rs`、`chat.rs`、`search.rs`

**当前问题**：
- `RagStrategy::step_plan` 已用 `collect_rag_tool_specs()` 查 registry（`capability/registry.rs:340-383` 证据）；
- `ChatStrategy` 和 `SearchStrategy` 可能仍硬编码工具列表。

**改造**：确保三个 Strategy 的 plan step 都通过 `CapabilityRegistry::plan_tools(strategy_id)` 获取工具。

**RagStrategy**（已部分实现，只需确认）：
```rust
// 当前 collect_rag_tool_specs() 内部已查 registry
let plan_tools = self.collect_rag_tool_specs();  // 保持现有逻辑
```

**ChatStrategy**：
```rust
// 改造前（假设硬编码）：
// let tools = vec![calculator_spec, code_execution_spec, ...];

// 改造后：
let cap_registry = CapabilityRegistry::standard_cached();
let plan_tools = cap_registry.plan_tools("chat");
let tool_specs: Vec<common::ToolSpec> = plan_tools.iter().map(|t| t.into()).collect();
```

**SearchStrategy**：
```rust
let cap_registry = CapabilityRegistry::standard_cached();
let plan_tools = cap_registry.plan_tools("search");
```

**工作量**：低（~20 行，主要改 Chat/Search）。

---

### 3.3 改造点 C：Answer 阶段扩展支持 Tool Calls（格式工具）

**文件**：`crates/app/src/agents/strategy/rag.rs`、`chat.rs`、`search.rs`

**当前问题**：三个 Strategy 的 Answer state 只调 `LlmClient::complete`（无工具），LLM 无法调用 html-renderer、ppt-generation 等格式工具。

**改造**：

#### 3.3.1 扩展 RagState（加 `ExecuteFormat` 状态）

```rust
#[derive(Debug)]
pub enum RagState {
    Plan,
    ExecuteRetrieve,
    Answer,
    ExecuteFormat { calls: Vec<common::ToolCall> },  // 新增：执行格式工具
}

impl State for RagState {
    fn state_id(&self) -> &'static str {
        match self {
            // ... 现有 ...
            RagState::ExecuteFormat { .. } => "execute_format",
        }
    }
    fn state_kind(&self) -> StateKind {
        match self {
            // ... 现有 ...
            RagState::ExecuteFormat { .. } => StateKind::Execute,
        }
    }
}
```

**为什么加新状态**：保持 `StrategyExecutor` 的 state 边界可观测性，`replay.rs` 和 `eval_framework.rs` 能记录到 "Answer 阶段调了格式工具" 这个事件。

#### 3.3.2 改造 `RagStrategy::step_answer`

```rust
async fn step_answer(
    &self,
    _state: Box<dyn State>,
    ctx: &mut RagContext,
) -> Result<StepOutcome, AgentErrorKind> {
    // 1. 查 Answer 阶段格式工具
    let cap_registry = CapabilityRegistry::standard_cached();
    let format_skills = cap_registry.answer_format_skills("rag");
    let format_tools: Vec<common::ToolSpec> = format_skills
        .iter()
        .filter_map(|s| skill_metadata_to_tool_spec(s))
        .collect();

    // 2. 构造 Answer 阶段 messages
    let mut answer_system = build_answer_system_prompt(
        crate::agents::strategy::prompts::rag::ANSWER_SKILL_ID,
        "rag",
        &ctx.selected_skills,
        &ctx.selected_writing_styles,
    );
    // 追加格式工具目录到 system prompt
    if !format_tools.is_empty() {
        let tool_catalog = format_tool_catalog(&format_tools);
        answer_system.push_str("\n\n---\n\n## Available Format Tools\n\n");
        answer_system.push_str(&tool_catalog);
        answer_system.push_str("\n\nIf you need to format the output (e.g., render HTML, generate PPT), call the appropriate format tool first. Then synthesize the final answer based on the tool result.");
    }

    let mut messages = vec![avrag_llm::ChatMessage::system(answer_system)];
    messages.extend(ctx.loop_messages.clone());  // ReAct 内部历史
    messages.push(avrag_llm::ChatMessage::user(
        "基于上述收集到的证据，合成最终答案。"
    ));

    // 3. 调 LLM（带格式工具）
    let answer_response = self.llm
        .complete_with_tools(&messages, &format_tools, self.temperature)
        .await
        .map_err(|e| AgentErrorKind::from(e))?;

    ctx.request_count += 1;
    ctx.aggregated_usage = Some(helpers::merge_usage(
        ctx.aggregated_usage.as_ref(),
        &answer_response.usage,
    ));

    // 4. 如果 LLM 调了格式工具 → 进入 ExecuteFormat 状态
    if let Some(tool_calls) = answer_response.tool_calls {
        if !tool_calls.is_empty() {
            // 记录 assistant 的 tool_call 决策
            ctx.loop_messages.push(build_assistant_message_with_tool_calls(&answer_response));
            return Ok(StepOutcome::Next(Box::new(RagState::ExecuteFormat {
                calls: tool_calls,
            })));
        }
    }

    // 5. LLM 直接给答案 → 终止
    let content = answer_response.content.unwrap_or_default();
    let result = build_run_result(ctx, content);
    Ok(StepOutcome::Terminate(result))
}
```

#### 3.3.3 新增 `step_execute_format`

```rust
async fn step_execute_format(
    &self,
    state: Box<dyn State>,
    ctx: &mut RagContext,
) -> Result<StepOutcome, AgentErrorKind> {
    let calls = state.as_any()
        .downcast_ref::<RagState>()
        .and_then(|s| match s {
            RagState::ExecuteFormat { calls } => Some(calls.clone()),
            _ => None,
        })
        .ok_or_else(|| AgentErrorKind::Unknown("Invalid state for execute_format".to_string()))?;

    // 执行格式工具
    let results = execute_tool_calls(&calls, ctx).await?;

    // 记录 tool results
    ctx.loop_messages.push(build_tool_message(&results));
    ctx.all_tool_results.extend(results.clone());

    // 回到 Answer 状态（这次 LLM 应该直接给最终答案）
    Ok(StepOutcome::Next(Box::new(RagState::Answer)))
}
```

#### 3.3.4 同步改造 ChatStrategy / SearchStrategy

ChatStrategy 和 SearchStrategy 的 Answer state 同样扩展：
- `ChatState` 加 `ExecuteFormat { calls }` 变体（如果需要格式输出）；
- `SearchState` 加 `ExecuteFormat { calls }` 变体；
- `step_answer` 查 `answer_format_skills` + 调 `complete_with_tools`；
- 新增 `step_execute_format`。

**工作量**：
- RagStrategy：~150 行（状态扩展 + step_answer 改造 + step_execute_format 新增）；
- ChatStrategy：~50 行（较简单，工具少）；
- SearchStrategy：~50 行。

---

### 3.4 改造点 D：events.rs 扩展（如需）

**文件**：`crates/app/src/agents/events.rs`

**当前问题**：`rag.rs` 的 `answer.format_skills` 事件与 `events.rs` 的 `AgentEvent::PlanDecision` / `ToolResult` 功能重叠但命名不同。

**改造**：**不改现有事件名**，复用现有变体：

| ADR-0005 提议的新事件 | 现有对应变体 | 处理方式 |
|---|---|---|
| `plan.start` | `Activity { stage: "plan", ... }` | **复用** |
| `plan.thinking` | `ReasoningSummaryDelta { text }` | **复用** |
| `tool.call` | `PlanDecision { selected_tools }` 或 `ToolResult` | **复用** |
| `tool.result` | `ToolResult { tool, status, data }` | **复用** |
| `evidence.gate` | `Evaluation { signals, decision, reasoning }` | **复用** |
| `answer.start` | `Activity { stage: "answer", ... }` | **复用** |
| `answer.chunk` | `MessageDelta { text }` | **复用** |
| `answer.format_tool` | `ToolResult { tool, ... }` | **复用** |

**如果确实需要新变体**（例如区分 "Plan 阶段 tool call" 和 "Answer 阶段 tool call"），在 `AgentEvent` 上加：

```rust
/// Answer-phase tool execution result (distinguishes from plan-phase tool calls).
AnswerToolResult {
    tool: String,
    status: common::ToolStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    elapsed_ms: u64,
},
```

**工作量**：极低（如需新增变体 ~10 行）。

---

## 4. 改造后的架构

```
┌─────────────────────────────────────────────────────────────────────┐
│  Frontend                                                           │
│  POST /chat { agent_type: "chat" | "rag" | "search", messages, ... }│
└────────────────────┬────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────────────┐
│  UnifiedAgent::run (unified/mod.rs)                                 │
│  1. RouterPolicy → routing_decision                                 │
│  2. filter_cross_mode_history(request.messages) → cross_history     │  ← 改造点 A
│  3. match request.kind → Chat / RAG / Search                        │
│     各 Context::from_request(request, cross_history, ...)           │
└────────────────────┬────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────────────┐
│  StrategyExecutor::run<S> (strategy/executor.rs) — 完全复用         │
│  - cancellation check                                               │
│  - budget guard + audit record                                      │
│  - trace span (root + per-state child)                              │
│  - state_history recording                                          │
│  - replay snapshot                                                  │
│  - prometheus metrics                                               │
└────────────────────┬────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Strategy::step (strategy/{rag,chat,search}.rs)                     │
│                                                                     │
│  Plan 阶段：                                                         │
│    - 查 CapabilityRegistry::plan_tools(strategy_id)                │  ← 改造点 B
│    - 调 complete_with_tools(plan_msgs, plan_tools)                 │
│    - 如果 tool_calls → Execute 状态 → loop back                    │
│                                                                     │
│  Answer 阶段（改造点 C）：                                           │
│    - 查 CapabilityRegistry::answer_format_skills(strategy_id)      │
│    - 调 complete_with_tools(answer_msgs, format_tools)             │
│    - 如果 tool_calls → ExecuteFormat 状态 → 回到 Answer            │
│    - 如果直接 content → Terminate                                  │
└────────────────────┬────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────────────┐
│  AgentRunResult → SSE events (events.rs)                            │
│  复用现有 AgentEvent 变体（Activity/StateTransition/ToolResult/     │
│  Evaluation/MessageDelta/Done）                                     │  ← 改造点 D
└─────────────────────────────────────────────────────────────────────┘
```

---

## 5. 关键设计细节

### 5.1 为什么 Answer 阶段也调 `complete_with_tools`

用户的业务约束：
> "answer 阶段也需要 tool call，只是 tool 不一样而已，plan 阶段是检索工具，answer 阶段是一些格式工具而已。"

这意味着：
- Plan 阶段工具 = 检索/计算类（dense_retrieval, web_search, calculator...）
- Answer 阶段工具 = 格式/输出类（html-renderer, ppt-generation, presentation-html...）
- **两个阶段的 LLM 调用都走 `complete_with_tools`**，只是传入的 `tools` 参数不同。

### 5.2 为什么加 `ExecuteFormat` 状态而不是在 `step_answer` 内部循环

为了保留 `StrategyExecutor` 的 state 边界可观测性：
- `replay.rs` 需要知道 "Answer 阶段调了格式工具" 这个事件；
- `eval_framework.rs` 可能在 `ExecuteFormat` 边界采集断言；
- `audit.rs` 按 state_id 上报。

如果 `step_answer` 内部隐藏子循环，这些设施会丢失 granularity。

### 5.3 跨 mode 历史注入的边界

- **只注入 user 角色**：agent 的推理过程和答案**不**进历史，避免"上一个 mode 的错误推理污染下一个 mode"；
- **`[prior_user_query]` 前缀**：明确告诉 LLM "这是历史查询，不是当前证据"，防止 LLM 把历史 query 当作当前需要回答的问题；
- **不注入 assistant 角色**：包括 tool_call 决策和 tool_result——这些是 mode 内部的 ReAct 轨迹，不是跨 mode 共享资产。

### 5.4 Evidence Gate 的处理

RagStrategy 和 SearchStrategy 的 Evidence Gate **保持现有逻辑**：
- `Pass` → 跳出 Plan↔Execute 循环，进 Answer；
- `NeedsFocus` → 继续迭代（loop_messages 保留）；
- `Degrade` → RAG 返回固定文案；Search 靠 skill prompt 处理。

ChatStrategy **不需要** Evidence Gate（`LoopBudget::chat` 已控制最大迭代）。

### 5.5 硬降级文案

仅 RAG mode 使用空检索硬降级文案（`"未找到相关文档..."`）。

实现方式：不在 trait 上暴露 `empty_evidence_response()`，而是在 `RagContext` 或 `RagStrategy` 内部常量配置：

```rust
impl RagStrategy {
    const EMPTY_EVIDENCE_RESPONSE: &str = "未找到相关文档，请尝试更换关键词或上传相关文档。";
}
```

Search mode 的 "部分结果" 行为由 skill body 驱动，不走代码硬降级。

### 5.6 业务降级 vs 硬 fallback

| 场景 | 处理方式 |
|---|---|
| LLM API 超时/5xx | 硬 fallback：返回 error SSE，提示重试 |
| 向量库连接失败 | 工具返回 error → LLM 收到 error message → skill body 引导修复 |
| Evidence Gate `Degrade` | RAG 硬降级文案；Search 由 skill 处理 |
| LLM Plan 阶段直接给答案（未调工具） | Chat：**接受**；RAG/Search：skill body 约束避免；code-level 兜底见 §5.7 |
| LLM Answer 阶段调了检索工具 | `answer_format_skills` 里**不包含**检索工具，LLM 物理上无法调 |

### 5.7 Code-level 兜底：Plan 阶段 LLM 未调工具直接给答案

RAG/Search mode 下，如果 Plan 阶段 LLM 直接返回 content（未调任何工具），**不应直接返回给用户**，因为：
- 可能没有 evidence；
- 答案可能无引用/幻觉。

**兜底逻辑**：

```rust
// 在 RagStrategy::step_plan 中
if response.tool_calls.is_none() && response.content.is_some() {
    // Chat mode: 接受
    if is_chat_mode { /* OK */ }
    
    // RAG/Search mode: 强制进入 Answer 阶段，但用强约束 prompt
    // "你未收集到任何证据。基于你已有的知识简要回答，但必须明确说明'未检索到相关文档'。"
    let forced_answer = self.force_answer_without_evidence(ctx, response.content.unwrap()).await?;
    return Ok(StepOutcome::Terminate(forced_answer));
}
```

或者更严格的兜底：**标记 degrade**。

---

## 6. 对 replay / eval / audit 的影响评估

### 6.1 replay.rs

**影响**：新增 `ExecuteFormat` 状态会被 `StrategyExecutor` 自动记录到 `state_history` 和 `ReplaySnapshot` 中。

**无需改动**：`replay.rs` 的 `CapturedRunResult` 从 `AgentRunResult` 构建，不依赖具体 state 类型，只依赖 `state_history` 字段。

### 6.2 eval_framework.rs

**影响**：需要确认 eval runner 的断点是绑在 `StateKind::Evaluate`（已不存在，ADR-0004 已移除）还是 `IterationRecord` 上。

**评估**：
- 如果 eval 在 `RagStrategy::step_plan` 的 `IterationRecord` 处采集（当前实现），**不受影响**；
- 如果 eval 在 `StateKind::Evaluate` 处断点（遗留代码），需要迁移到 `IterationRecord` 或 `ExecuteFormat` 边界。

**建议**：在改造前做一次 `eval_framework.rs` 的断点依赖审计（grep `StateKind::Evaluate` 和 `state_id == "evaluate"`）。

### 6.3 audit.rs

**影响**：新增 `ExecuteFormat` 状态的 `state_id = "execute_format"`，audit 记录会自动包含（`StrategyExecutor` 在每次 state transition 时都 emit audit）。

**无需改动**。

---

## 7. 替代方案评估

### 7.1 方案 A：ADR-0005 原方案（统一 AgentKernel + 废弃 Strategy）

**已否决**：
- 会推翻 `StrategyExecutor`（cancellation/budget/trace/audit/replay 全部重写）；
- 会推翻 `State` 抽象（`replay.rs` / `eval_framework.rs` 的 state 边界采集丢失）；
- 会重复造 `CapabilityRegistry` 的轮子（`AgentMode::plan_tools/answer_tools` 是简化版副本）；
- 与 `events.rs` 事件命名冲突。

### 7.2 方案 B：引入 Rig

**已否决**：
- Rig 的 `Agent` 在构造时静态绑定 tools，不支持"Plan 阶段检索工具 / Answer 阶段格式工具"的阶段级动态切换；
- `avrag-llm` 已覆盖 provider 抽象 + 工具调用（48/48 测试）；
- 引入 Rig 增加 breaking change 风险（v0.x，快速发展期）。

### 7.3 方案 C：引入 GraphFlow

**已否决**：
- GraphFlow 的 tick 粒度是 Task 级，看不到 Strategy 内部每轮 LLM/tool 行为；
- 当前业务无 interruptible / 人类介入 / 长时任务恢复需求；
- 社区成熟度不足（v0.2.3，still in progress）。

**重新评估触发条件**（同 ADR-0005 §10.1）。

---

## 8. 迁移计划

### 阶段 1：基础设施改造（1 周）

- [ ] `UnifiedAgent::run` 加 `filter_cross_mode_history`（改造点 A）；
- [ ] 三个 `Context::from_request` 接收 `cross_mode_history` 参数；
- [ ] 集成测试：`test_cross_mode_history_injected`。

**验证标准**：`cargo test -p app --lib` 不回归 + 新增测试通过。

### 阶段 2：Plan 阶段 registry 统一（1 周）

- [ ] `ChatStrategy::step_plan` 改用 `CapabilityRegistry::plan_tools("chat")`；
- [ ] `SearchStrategy::step_plan` 改用 `CapabilityRegistry::plan_tools("search")`；
- [ ] 确认 `RagStrategy::collect_rag_tool_specs()` 已走 registry；
- [ ] 集成测试：验证三个 Strategy 的 plan_tools 与 registry 一致。

**验证标准**：`cargo test -p app --lib` 全绿。

### 阶段 3：Answer 阶段调格式工具（2 周）

- [ ] `RagState` / `ChatState` / `SearchState` 加 `ExecuteFormat` 变体；
- [ ] `RagStrategy::step_answer` 扩展查 `answer_format_skills` + 调 `complete_with_tools`；
- [ ] 新增 `step_execute_format`（三个 Strategy）；
- [ ] `ChatStrategy::step_answer` 和 `SearchStrategy::step_answer` 同样扩展；
- [ ] 集成测试：
  - `test_rag_answer_with_format_tool`
  - `test_rag_answer_no_format_tool`
  - `test_chat_answer_with_format_tool`

**验证标准**：现有 442 lib tests 不回归 + 新增测试全绿。

### 阶段 4：评估与收尾（1 周）

- [ ] `eval_framework.rs` 断点依赖审计（grep `StateKind::Evaluate`）；
- [ ] 如需，新增 `AgentEvent::AnswerToolResult` 变体；
- [ ] 更新文档；
- [ ] E2E 测试验证（需环境变量）。

**总估算**：~5 周（比 ADR-0005 原方案的 8-10 周大幅减少）。

---

## 9. 测试策略

### 9.1 单元测试

- `filter_cross_mode_history`：验证只保留 user 角色 + 前缀正确；
- `CapabilityRegistry::plan_tools` 一致性：验证三个 strategy 查到的工具与 registry 注册的一致。

### 9.2 集成测试（Mock LLM + Mock DataPlane）

复用 ADR-0004 的 `ScriptedLlmProvider` + `ScriptedDataPlane`：

| 测试名 | 目标 |
|---|---|
| `test_cross_mode_history_chat_to_rag` | Chat→RAG 切换时 user query 被注入 |
| `test_plan_tools_from_registry` | Plan 阶段工具与 registry 一致 |
| `test_rag_answer_calls_format_tool` | Answer 阶段 LLM 调 html-renderer |
| `test_rag_answer_no_format_tool` | Answer 阶段 LLM 直接给答案 |
| `test_rag_answer_format_tool_then_synthesize` | 调格式工具 → 执行 → 再合成 |
| `test_chat_answer_with_format_tool` | Chat Answer 阶段调轻量格式工具 |
| `test_search_answer_with_format_tool` | Search Answer 阶段调格式工具 |
| `test_plan_direct_answer_rag_degrades` | RAG Plan 阶段 LLM 直接给答案 → code-level 兜底 |

---

## 10. 开放问题

### 10.1 eval_framework.rs 断点审计

需在阶段 1 之前完成：确认 `eval_framework.rs` 是否还有绑在 `StateKind::Evaluate` 的断点。如果有，需在阶段 3 之前迁移到 `IterationRecord`。

### 10.2 ChatStrategy 是否需要 Answer 阶段调工具

Chat mode 的 Answer 阶段是否需要格式工具？如果不需要，ChatStrategy 的 `step_answer` 可以保持现状（不调工具），仅 Rag/Search 扩展。

**建议**：阶段 3 先只做 Rag/Search，Chat 按需后续扩展。

### 10.3 格式工具的 `ToolSpec` 生成

`CapabilityRegistry::answer_format_skills` 返回的是 `SkillMetadata`，需要 `skill_metadata_to_tool_spec()` 转换函数。这个函数是否已存在？如果不存在，需在 `capability/registry.rs` 或 `strategy/prompts.rs` 中实现。

### 10.4 `selected_skills` / `selected_writing_styles` / `behavior_mode` 字段

当前三个 Context 都带有这些字段，但 ADR-0004 已指出它们" misleading"。

**建议**：本 ADR **不处理**这些字段的清理——作为独立技术债后续处理。

---

## 11. 参考文档

- ADR-0004: RAG Agent Loop with Native Tool Calling
- ADR-0005（原）：Unified Agent Kernel（已否决，仅留档参考）
- `crates/app/src/agents/unified/mod.rs`
- `crates/app/src/agents/strategy/executor.rs`
- `crates/app/src/agents/strategy/mod.rs`
- `crates/app/src/agents/capability/registry.rs`
- `crates/app/src/agents/events.rs`
- `crates/app/src/agents/replay.rs`
- `crates/app/src/agents/eval_framework.rs`

---

*本文档是 ADR-0005 的修正版，基于用户评审（"与 v5 架构重大重叠与冲突"）和逐文件代码对照后重新编写。核心方向从"重写 Kernel"调整为"v5 增量扩展"。*
