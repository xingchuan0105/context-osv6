# ADR 0003: Agent v5 架构 —— 能力统一注册，策略各自定义

> Status: Accepted
> Date: 2026-05-21
> Author: Context OS Team
> Version: 1.0

## 背景

### v0 → v4 的演进与根因

| 阶段 | 实现 | 解决的问题 | 引入的新问题 |
|------|------|-----------|------------|
| v0 代码驱动 | LLM 只在特定节点执行命令，检索/聊天/websearch 由代码驱动 | — | LLM 只是文本生成器，没有决策能力 |
| v1 Main Agent | 合并成一个通用 Agent | 统一入口 | 一个 Agent 无法同时做好简单对话和复杂检索 |
| v2 三 Agent 独立 | Chat/RAG/Search 各自有独立 loop | 差异化体验 | 能力各自为政，无法统一扩展 |
| v3 Perplexity 渐进披露 | tool/skill 标准化注册，Index/Load/Runtime 三级披露 | 能力可扩展 | 假设了"通用 Agent 自我探索"，但 Context OS 的 Agent 角色是前端预确定的，导致映射漂移 |
| v4 Bundle | 每个模式预定义环境（planner skill + tool catalog + format skills） | 修正映射漂移 | 保留了统一的 ProgressiveLoop，造成 Chat Evaluate 空转、Search Plan 空转、API 黑盒 |

### v4 的核心矛盾

**Bundle 修正了"能力预定义"，但没有修正"流程假设"。**

`ProgressiveLoop` 假设所有模式共享同一套固定相位序列：`Init → Plan → Execute → Evaluate → Answer`。

但三个模式的实际需求完全不同：

- **Chat**：`Init → Plan → [Execute] → Answer`（不需要 Evaluate）
- **RAG**：`Init → Plan → Execute → Evaluate[*N] → Answer`（需要迭代）
- **Search**：`Init(=Plan) → Execute → Evaluate[*N] → Answer`（Plan 在 Init 完成）

为通用性而保留的 Evaluate 相位，对 Chat 是空转；为统一性而保留的 Plan 相位，对 Search 是空转。

### 根本问题：两个维度被混在了一起

```
维度 A：交互体验（前端产品层）
  Chat = 对话式、单轮、轻量
  RAG  = 文档式、多轮检索、深度
  Search = 网络式、查询分解、聚合
  → 这是"策略"，每个模式有自己的决策逻辑

维度 B：系统能力（后端平台层）
  Tools：dense_retrieval, calculator, web_search, code_interpreter, ...
  Skills：rag-plan, chat-answer, ppt-generation, teaching, ...
  → 这是"能力"，统一注册，谁都可以调用
```

v0-v4 每次改造都是把 A 和 B 绑在一起处理。v4 Bundle 是"把 A 和 B 绑得更紧"。

**v5 的核心思路：解耦 A 和 B。能力统一注册，策略各自定义。**

---

## 设计目标

1. **消除流程空转**：每个模式只走自己需要的阶段，没有伪相位
2. **统一能力扩展**：新增 tool/skill 注册到能力层，所有策略自动可见
3. **API 白盒化**：外部 Agent 可以看到完整的状态转换、可以干预、可以理解决策依据
4. **保留前端体验**：Chat/RAG/Search 三按钮的交互不变
5. **预留跨模式空间**：未来支持"RAG + Search 并行"的复杂流程，不需要重写架构

---

## 架构总览

```
┌─────────────────────────────────────────────────────────┐
│                     API / Frontend                       │
│              Chat        RAG        Search               │
└─────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│                   Router（路由层）                        │
│  request.kind → 选择 Strategy 实例                       │
│  同时初始化 state + 注入所需 runtime 依赖                 │
└─────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│                   Strategy（策略层）                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │
│  │ChatStrategy │  │ RagStrategy │  │ SearchStrategy  │  │
│  │  状态机      │  │  状态机      │  │   状态机         │  │
│  └─────────────┘  └─────────────┘  └─────────────────┘  │
│                                                          │
│  每个策略自己决定：                                        │
│  - 有哪些状态（不是固定的 Plan/Execute/Evaluate）         │
│  - 状态之间怎么转移                                       │
│  - 每个状态调用什么能力                                   │
└─────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│                  Capability（能力层）                     │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  ToolRegistry：所有工具统一注册                       │ │
│  │    dense_retrieval, lexical_retrieval, graph_...    │ │
│  │    calculator, code_interpreter, weather_query      │ │
│  │    web_search, doc_summary, ...                     │ │
│  └─────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  SkillRegistry：所有技能统一注册                      │ │
│  │    rag-plan, rag-eval, rag-answer                   │ │
│  │    chat-plan, chat-answer, search-plan              │ │
│  │    ppt-generation, html-renderer, teaching          │ │
│  └─────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  LLM Provider：completion / stream / embeddings      │ │
│  └─────────────────────────────────────────────────────┘ │
│                                                          │
│  新增能力 = 注册到这里，所有策略自动可见                   │
└─────────────────────────────────────────────────────────┘
```

---

## 详细设计

### 1. 能力层（Capability Layer）

保留 v4 的 `PromptRegistry` 和 `ToolRegistry`，但升级为**全局统一注册表**。

```rust
/// 全局能力注册表，所有策略共享
pub struct CapabilityRegistry {
    tools: HashMap<String, Tool>,
    skills: HashMap<String, Skill>,
    llm: LlmClient,
}

impl CapabilityRegistry {
    /// 注册新工具，所有策略立即可用
    pub fn register_tool(&mut self, meta: ToolMetadata, impl: Box<dyn Tool>);
    /// 注册新技能，所有策略立即可用
    pub fn register_skill(&mut self, meta: SkillMetadata, body: SkillBody);
    /// 按 ID 获取工具
    pub fn tool(&self, id: &str) -> Option<&Tool>;
    /// 按 ID 获取技能
    pub fn skill(&self, id: &str) -> Option<&Skill>;
    /// 列出所有工具元数据（用于 /capabilities 接口）
    pub fn list_tools(&self) -> Vec<&ToolMetadata>;
    /// 列出所有技能元数据（用于 /capabilities 接口）
    pub fn list_skills(&self) -> Vec<&SkillMetadata>;
}

/// 工具元数据 —— 治理、审计、生命周期管理
pub struct ToolMetadata {
    pub id: String,
    pub version: String,
    pub owner: String,           // 负责团队/个人
    pub description: String,
    pub input_schema: serde_json::Value,   // JSON Schema
    pub output_schema: serde_json::Value,  // JSON Schema
    pub risk_level: RiskLevel,   // Low / Medium / High / Critical
    pub permissions: Vec<Permission>,      // 需要什么权限才能调用
    pub external_deps: Vec<String>,        // 依赖的外部服务
    pub deprecation: Option<Deprecation>,  // 退役计划
    pub retry_policy: RetryPolicy,         // 默认重试策略
}

/// 技能元数据
pub struct SkillMetadata {
    pub id: String,
    pub version: String,
    pub owner: String,
    pub description: String,
    pub applicable_strategies: Vec<String>, // 适用于哪些策略
    pub required_tools: Vec<String>,        // 依赖哪些工具
    pub risk_level: RiskLevel,
    pub deprecation: Option<Deprecation>,
}

pub enum RiskLevel { Low, Medium, High, Critical }

pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_ms: u64,
    pub backoff_multiplier: f64,
    pub idempotent: bool,        // 是否幂等
    pub idempotency_key_header: Option<String>,
}
```

**关键变化：**
- v4 的 Bundle 里硬编码了 `format_skill_ids: &["ppt-generation", ...]`，v5 改为策略在运行时从 Registry 查询
- v4 的 `plan_tools()` 在 Bundle 里静态定义，v5 改为 ToolRegistry 统一维护，策略按需要筛选

### 2. 策略层（Strategy Layer）

每个策略是一个**独立的状态机**，自己管理状态定义、状态转移、循环逻辑。

#### 2.1 策略 trait

```rust
/// 策略状态机接口
#[async_trait::async_trait]
pub trait Strategy: Send + Sync {
    type State;
    type Context;

    /// 初始化状态机，返回初始状态
    async fn init(&self, ctx: &Self::Context) -> Result<StateBox, AppError>;

    /// 执行一个状态，返回下一个状态或终止
    async fn step(
        &self,
        state: StateBox,
        ctx: &mut Self::Context,
    ) -> Result<StepOutcome, AppError>;
}

pub enum StepOutcome {
    /// 进入下一个状态
    Next(StateBox),
    /// 终止，返回结果
    Terminate(AgentRunResult),
}
```

`StateBox` 是类型擦除的状态容器，让 Strategy trait 可以支持异构状态机。

#### 2.2 ChatStrategy

```rust
enum ChatState {
    Plan,
    ExecuteAtomic { calls: Vec<ToolCall> },
    Answer,
}

impl Strategy for ChatStrategy {
    type State = ChatState;
    type Context = ChatContext;

    async fn step(&self, state: ChatState, ctx: &mut ChatContext) -> Result<StepOutcome, AppError> {
        match state {
            ChatState::Plan => {
                let decision = self.plan(ctx).await?;
                match decision.action {
                    Action::Clarify(q) => Ok(StepOutcome::Terminate(clarify_result(q))),
                    Action::Answer if decision.calls.is_empty() => Ok(StepOutcome::Next(ChatState::Answer)),
                    Action::Answer => Ok(StepOutcome::Next(ChatState::ExecuteAtomic { calls: decision.calls })),
                }
            }
            ChatState::ExecuteAtomic { calls } => {
                let results = self.execute_tools(calls).await?;
                ctx.tool_results = results;
                Ok(StepOutcome::Next(ChatState::Answer))
            }
            ChatState::Answer => {
                let result = self.answer(ctx).await?;
                Ok(StepOutcome::Terminate(result))
            }
        }
    }
}
```

**关键特征：**
- 没有 Evaluate 状态
- 没有循环（Plan 最多触发一次 Execute）
- 流程：`Plan → Execute → Answer` 或 `Plan → Answer`

#### 2.3 RagStrategy

```rust
enum RagState {
    Plan,
    ExecuteRetrieve { calls: Vec<ToolCall> },
    Evaluate,
    Answer,
}

impl Strategy for RagStrategy {
    async fn step(&self, state: RagState, ctx: &mut RagContext) -> Result<StepOutcome, AppError> {
        match state {
            RagState::Plan => {
                let decision = self.plan(ctx).await?;
                ctx.budget.tick();
                Ok(StepOutcome::Next(RagState::ExecuteRetrieve { calls: decision.calls }))
            }
            RagState::ExecuteRetrieve { calls } => {
                let results = self.execute_retrieval(calls).await?;
                ctx.accumulated.add(results);
                Ok(StepOutcome::Next(RagState::Evaluate))
            }
            RagState::Evaluate => {
                let eval = self.evaluate(ctx).await?;
                match eval.decision {
                    EvalDecision::Synthesize => Ok(StepOutcome::Next(RagState::Answer)),
                    EvalDecision::Replan if !ctx.budget.exhausted() => Ok(StepOutcome::Next(RagState::Plan)),
                    EvalDecision::Replan => Ok(StepOutcome::Next(RagState::Answer)), // degrade
                }
            }
            RagState::Answer => {
                let result = self.answer(ctx).await?;
                Ok(StepOutcome::Terminate(result))
            }
        }
    }
}
```

**关键特征：**
- 有 Evaluate 状态，支持 `Replan → Plan` 循环
- 预算检查在 Evaluate 转移时进行
- 流程：`Plan → Execute → Evaluate → {Plan | Answer}`

#### 2.4 SearchStrategy

```rust
enum SearchState {
    Decompose,           // 对应 v4 的 Init 阶段 planning
    ParallelSearch { queries: Vec<String> },
    Aggregate,
    Evaluate,
    Answer,
}

impl Strategy for SearchStrategy {
    async fn step(&self, state: SearchState, ctx: &mut SearchContext) -> Result<StepOutcome, AppError> {
        match state {
            SearchState::Decompose => {
                let plan = self.decompose(ctx).await?;
                ctx.sub_queries = plan.sub_queries;
                ctx.budget.tick();
                Ok(StepOutcome::Next(SearchState::ParallelSearch {
                    queries: plan.sub_queries,
                }))
            }
            SearchState::ParallelSearch { queries } => {
                // 并行执行所有 sub_queries
                let results = futures::future::join_all(
                    queries.into_iter().map(|q| self.web_search(q))
                ).await;
                ctx.search_results = results;
                Ok(StepOutcome::Next(SearchState::Aggregate))
            }
            SearchState::Aggregate => {
                self.deduplicate_and_rank(ctx).await?;
                Ok(StepOutcome::Next(SearchState::Evaluate))
            }
            SearchState::Evaluate => {
                let eval = self.evaluate(ctx).await?;
                ctx.budget.tick();
                match eval.decision {
                    EvalDecision::Synthesize => Ok(StepOutcome::Next(SearchState::Answer)),
                    EvalDecision::Broaden if !ctx.budget.exhausted() => {
                        ctx.sub_queries = eval.new_queries;
                        Ok(StepOutcome::Next(SearchState::ParallelSearch {
                            queries: eval.new_queries,
                        }))
                    }
                    EvalDecision::Broaden => Ok(StepOutcome::Next(SearchState::Answer)), // degrade
                }
            }
            SearchState::Answer => {
                let result = self.answer(ctx).await?;
                Ok(StepOutcome::Terminate(result))
            }
        }
    }
}
```

**关键特征：**
- 没有 Plan 状态（planning 在 Decompose 完成）
- ParallelSearch 是并行节点
- Evaluate 可以循环回 ParallelSearch（换 query 重搜）
- 流程：`Decompose → ParallelSearch → Aggregate → Evaluate → {ParallelSearch | Answer}`

### 3. 编排层（Orchestration Layer）

编排层只负责一件事：**驱动状态机直到终止**。

```rust
pub struct StrategyExecutor;

impl StrategyExecutor {
    pub async fn run<S: Strategy>(
        &self,
        strategy: &S,
        mut ctx: S::Context,
    ) -> Result<AgentRunResult, AppError> {
        let trace_id = ctx.trace_id().to_string();
        let start_time = Instant::now();

        // 创建顶层 span
        let mut root_span = TraceSpan::new(&trace_id, "agent.run", None);
        root_span.set_attribute("strategy", std::any::type_name::<S>());

        let mut state = strategy.init(&ctx).await?;
        let mut state_history: Vec<StateRecord> = Vec::new();

        loop {
            ctx.cancel.check()?;
            let state_entered_at = Instant::now();
            let state_id = state.state_id().to_string();

            // emit 观测事件
            ctx.sink.emit(StateEvent::Entered {
                state_id: state_id.clone(),
                state_kind: state.state_kind().to_string(),
                timestamp_ms: timestamp(),
            }).await?;

            // 执行状态
            let mut state_span = TraceSpan::new(&trace_id, &format!("state.{}", state_id), Some(&root_span.id));
            let outcome = strategy.step(state, &mut ctx).await;
            let state_elapsed = state_entered_at.elapsed();
            state_span.set_attribute("elapsed_ms", state_elapsed.as_millis() as u64);
            state_span.finish();

            match outcome {
                Ok(StepOutcome::Next(next_state)) => {
                    state_history.push(StateRecord {
                        state_id: state_id.clone(),
                        entered_at: state_entered_at,
                        elapsed_ms: state_elapsed.as_millis() as u64,
                        outcome: "next".to_string(),
                    });
                    ctx.sink.emit(StateEvent::StateCompleted {
                        state_id,
                        elapsed_ms: state_elapsed.as_millis() as u64,
                    }).await?;
                    state = next_state;
                }
                Ok(StepOutcome::Terminate(result)) => {
                    root_span.set_attribute("total_elapsed_ms", start_time.elapsed().as_millis() as u64);
                    root_span.set_attribute("budget_used", ctx.budget().current);
                    root_span.finish();

                    let mut result = result;
                    result.trace_id = Some(trace_id);
                    result.state_history = Some(state_history);
                    result.total_elapsed_ms = Some(start_time.elapsed().as_millis() as u64);
                    return Ok(result);
                }
                Err(e) => {
                    root_span.set_error(&e.to_string());
                    root_span.finish();
                    return Err(e);
                }
            }
        }
    }
}
```

**与 v4 ProgressiveLoop 的区别：**
- v4 的 LoopDriver 硬编码了 `Plan → Execute → Evaluate` 的固定相位序列
- v5 的 Executor 不假设任何状态名称或转移规则，只负责"驱动状态机一步"
- 状态转移逻辑完全在 Strategy 内部

### 3.5 路由层设计

v4 的路由是隐式的：`request.kind` 直接映射到固定代码路径。v5 引入显式的 Router Policy：

```rust
/// 路由策略：根据请求特征选择最合适的 Strategy
pub struct RouterPolicy {
    pub rules: Vec<RouterRule>,
}

pub struct RouterRule {
    /// 匹配条件
    pub condition: RouterCondition,
    /// 目标策略
    pub strategy: String,
    /// 优先级（高优先级的规则先匹配）
    pub priority: u16,
    /// 是否允许用户覆盖（如 request.kind 显式指定）
    pub user_overridable: bool,
}

pub enum RouterCondition {
    /// 按 agent_type 匹配
    Kind(AgentKind),
    /// 按 doc_scope 非空匹配
    HasDocScope,
    /// 按 query 意图分类匹配
    IntentClassified(Intent),
    /// 按上下文长度匹配
    ContextLength { max_tokens: u64 },
    /// 组合条件
    All(Vec<RouterCondition>),
    Any(Vec<RouterCondition>),
}
```

**默认路由规则：**

| 优先级 | 条件 | 策略 | 说明 |
|-------|------|------|------|
| 100 | `kind == Chat` | ChatStrategy | 前端 Chat 按钮 |
| 90 | `kind == Rag && doc_scope 非空` | RagStrategy | 前端 RAG 按钮 |
| 80 | `kind == Search` | SearchStrategy | 前端 Search 按钮 |
| 70 | `query 意图为 factual && doc_scope 非空` | RagStrategy | API 自动路由 |
| 60 | `query 意图为 external_knowledge` | SearchStrategy | API 自动路由 |
| 50 | `default` | ChatStrategy | 兜底 |

**路由决策输出：**

```rust
pub struct RoutingDecision {
    pub strategy_id: String,
    pub matched_rule: String,
    pub confidence: f64,
    pub overridable: bool,
    pub explanation: String,  // "用户显式选择了 RAG 模式" / "query 包含'搜索'关键词，自动路由到 Search"
}
```

这样前端或 API 调用方可以：
- 显式指定 `request.kind`（用户主动选择）
- 不指定 `kind`，由 Router 根据 query 特征自动选择
- 查看 `RoutingDecision.explanation` 理解决策依据

**优先级冲突语义（确定性决策树）：**

当多条规则同时命中时，按以下顺序确定最终策略：

1. **用户显式指定**（`request.kind` 不为空且 `user_overridable == true`）→ 直接用用户指定的策略
2. **最高优先级规则**（priority 值最大）→ 匹配该规则的策略
3. **风险更低优先**（同优先级同置信度下，选择 `max_risk_level` 更低的策略）
4. **确定性 tie-break**（若仍并列，按策略名字典序升序取第一个）
5. **兜底规则**（priority 最低的 default 规则）→ 兜底策略

```rust
pub fn resolve(request: &AgentRequest, rules: &[RouterRule]) -> RoutingDecision {
    // 1. 用户显式指定
    if let Some(kind) = request.kind {
        if let Some(rule) = rules.iter().find(|r| r.user_overridable && r.condition.matches_kind(kind)) {
            return RoutingDecision {
                strategy_id: rule.strategy.clone(),
                matched_rule: rule.name.clone(),
                confidence: 1.0,
                overridable: true,
                rejected_candidates: vec![],
                explanation: format!("用户显式选择了 {:?} 模式", kind),
            };
        }
    }

    // 收集所有命中规则
    let mut candidates: Vec<&RouterRule> = rules.iter()
        .filter(|r| r.condition.evaluate(request))
        .collect();

    // 2. 按优先级降序
    candidates.sort_by_key(|r| std::cmp::Reverse(r.priority));
    let max_priority = candidates.first().map(|r| r.priority).unwrap_or(0);
    candidates.retain(|r| r.priority == max_priority);

    // 3. 同优先级按置信度降序
    let max_confidence = candidates.iter().map(|r| r.confidence).fold(0.0, f64::max);
    candidates.retain(|r| (r.confidence - max_confidence).abs() < f64::EPSILON);

    // 收集被淘汰的候选（用于解释）
    let rejected: Vec<String> = rules.iter()
        .filter(|r| r.condition.evaluate(request) && !candidates.contains(&r))
        .map(|r| format!("{}(优先级={}, 置信度={})", r.name, r.priority, r.confidence))
        .collect();

    // 4. 风险更低优先
    candidates.sort_by_key(|r| r.max_risk_level as u8);

    // 5. 确定性 tie-break：按策略名字典序
    candidates.sort_by(|a, b| a.strategy.cmp(&b.strategy));

    if let Some(best) = candidates.first() {
        return RoutingDecision {
            strategy_id: best.strategy.clone(),
            matched_rule: best.name.clone(),
            confidence: best.confidence,
            overridable: best.user_overridable,
            rejected_candidates: rejected,
            explanation: format!(
                "命中 {} 条规则，按优先级={}、置信度={}、风险等级={} 选择 {}；未选: {:?}",
                candidates.len(), best.priority, best.confidence, best.max_risk_level, best.strategy, rejected
            ),
        };
    }

    // 6. 兜底
    default_routing_decision()
}
```

**关键保证：**
- **确定性**：相同输入永远产生相同路由结果（无随机性）
- **可解释**：`explanation` 包含"为什么选这个"和"为什么没选其他"
- **安全优先**：并列时优先选择风险等级更低的策略

---

### 4. API 白盒化与观测设计

v4 的 API 是黑盒：外部 Agent 只能看到 `iterations: [...]` 的聚合结果，看不到每轮 Plan 选了什么工具、Execute 返回了什么原始数据、Evaluate 为什么决定继续。

v5 将观测设计从"事件输出"升级为"可追踪运行链路"，包含三个层级：

#### 4.1 Trace / Span 结构

```rust
/// 一次 agent run 的全链路追踪
pub struct AgentTrace {
    pub trace_id: String,
    pub request: AgentRequest,
    pub routing_decision: RoutingDecision,
    pub spans: Vec<TraceSpan>,
    pub total_elapsed_ms: u64,
    pub budget_used: u8,
    pub final_result: AgentRunResult,
}

pub struct TraceSpan {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,           // "state.plan", "tool.dense_retrieval", "llm.completion"
    pub started_at: Instant,
    pub elapsed_ms: u64,
    pub attributes: HashMap<String, serde_json::Value>,
    pub error: Option<String>,
}
```

**每个状态转换自动生成 span：**
- `agent.run` —— 顶层 span，覆盖整个请求
- `state.plan` / `state.execute` / `state.evaluate` —— 状态级 span
- `tool.{tool_name}` —— 工具调用 span，含参数、耗时、结果状态
- `llm.completion` —— LLM 调用 span，含 model、prompt_tokens、completion_tokens
- `budget.tick` —— 预算消耗 span

#### 4.2 状态事件（SSE 输出）

```rust
pub enum StateEvent {
    /// 路由决策
    RoutingDecision { strategy: String, explanation: String },
    /// 进入新状态
    Entered { state_id: String, state_kind: String, timestamp_ms: u64 },
    /// 状态完成
    StateCompleted { state_id: String, elapsed_ms: u64 },
    /// Plan/Decompose 阶段的决策输出
    PlanDecision {
        selected_tools: Vec<ToolCall>,
        selected_skills: Vec<String>,
        reasoning: String,
    },
    /// Execute 阶段的工具调用结果
    ToolResult {
        tool: String,
        status: ToolStatus,
        data: serde_json::Value,
        elapsed_ms: u64,
    },
    /// Evaluate 阶段的评估输出
    Evaluation {
        signals: EvaluationSignals,
        decision: String,
        reasoning: String,
    },
    /// 预算状态变化
    BudgetTick { current: u8, max: u8 },
    /// 终止决策
    Terminal { decision: FinalDecision },
    /// Trace 完成（含完整链路信息，debug 模式下发）
    TraceSummary { trace_id: String, total_elapsed_ms: u64 },
}
```

#### 4.3 Metrics（指标）

| 指标名 | 类型 | 说明 |
|-------|------|------|
| `agent_run_total` | Counter | 总运行次数 |
| `agent_run_duration_ms` | Histogram | 单次运行耗时 |
| `agent_state_duration_ms` | Histogram | 每个状态的耗时（按 state_id 分组） |
| `agent_tool_call_total` | Counter | 工具调用次数（按 tool_name, status 分组） |
| `agent_tool_call_duration_ms` | Histogram | 工具调用耗时 |
| `agent_llm_request_total` | Counter | LLM 请求次数（按 model 分组） |
| `agent_llm_tokens_total` | Counter | Token 消耗（按 prompt/completion 分组） |
| `agent_budget_exhausted_total` | Counter | 预算耗尽次数 |
| `agent_error_total` | Counter | 错误次数（按 error_kind 分组） |

#### 4.4 能力发现接口（版本化 + 权限声明）

```rust
/// GET /agent/capabilities
pub struct CapabilitiesResponse {
    pub api_version: String,
    pub registry_version: String,
    pub tools: Vec<ToolCapability>,
    pub skills: Vec<SkillCapability>,
    pub strategies: HashMap<String, StrategySchema>,
}

pub struct ToolCapability {
    pub id: String,
    pub version: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub risk_level: String,
    pub permissions: Vec<String>,
    pub external_deps: Vec<String>,
    pub deprecated: bool,
    pub deprecation_note: Option<String>,
    pub retry_policy: RetryPolicySchema,
}

pub struct SkillCapability {
    pub id: String,
    pub version: String,
    pub description: String,
    pub applicable_strategies: Vec<String>,
    pub required_tools: Vec<String>,
    pub risk_level: String,
    pub deprecated: bool,
}

pub struct StrategySchema {
    pub states: Vec<String>,
    pub transitions: Vec<TransitionSchema>,
    /// 该策略会调用哪些外部工具
    pub external_tools_used: Vec<String>,
    /// 该策略是否会联网
    pub requires_internet: bool,
    /// 该策略的最大预算消耗
    pub max_budget: u8,
}
```

这样 API 调用方可以在调用前知道：
- "RAG 策略会调用内部检索工具，不会联网"
- "Search 策略会调用 web_search，会联网，有风险"
- "ppt-generation 技能需要 graph_retrieval + doc_summary 工具"

#### 4.5 干预接口

```rust
pub struct AgentRequest {
    // ... v4 已有字段 ...

    /// 可选：干预 Plan 阶段的工具选择
    #[serde(default)]
    pub preferred_tools: Vec<String>,

    /// 可选：指定回答格式技能
    #[serde(default)]
    pub format_hint: Option<String>,

    /// 可选：限制最大迭代次数
    #[serde(default)]
    pub max_iterations: Option<u8>,

    /// 可选：是否返回完整 trace（debug 模式）
    #[serde(default)]
    pub debug: bool,
}
```

#### 4.6 运行结果结构化

```rust
pub struct AgentRunResult {
    pub answer: String,
    pub answer_blocks: Vec<AnswerBlock>,
    pub citations: Vec<Citation>,
    pub sources: Vec<SourceRef>,

    // v5 新增结构化字段
    pub trace_id: Option<String>,
    pub routing_decision: Option<RoutingDecision>,
    pub decisions: Vec<DecisionRecord>,        // 每轮 Plan/Evaluate 的决策
    pub tool_calls: Vec<ToolCallRecord>,        // 所有工具调用记录
    pub budget_used: Option<BudgetUsage>,
    pub state_history: Option<Vec<StateRecord>>,
    pub total_elapsed_ms: Option<u64>,
    pub eval_summary: Option<String>,
    pub degrade_trace: Vec<DegradeTraceItem>,
    pub usage: Option<AgentRunUsage>,
    pub final_decision: Option<FinalDecision>,
}

pub struct DecisionRecord {
    pub phase: String,          // "plan", "evaluate"
    pub iteration: u8,
    pub decision: String,
    pub reasoning: String,
    pub selected_tools: Vec<String>,
}

pub struct ToolCallRecord {
    pub tool: String,
    pub iteration: u8,
    pub args: serde_json::Value,
    pub status: ToolStatus,
    pub elapsed_ms: u64,
}

pub struct BudgetUsage {
    pub current: u8,
    pub max: u8,
    pub exhausted: bool,
}
```

### 5. Policy Enforcement 层（运行时策略强制）

能力层注册了 `permissions` 和 `risk_level`，但这些不能只是静态展示字段。v5 引入独立的 **Policy Enforcement Layer**，在 tool 调用前做运行时强制检查：

```rust
/// 策略强制检查器
pub struct PolicyEnforcer {
    pub rules: Vec<EnforcementRule>,
}

pub struct EnforcementRule {
    pub name: String,
    pub condition: EnforcementCondition,
    pub action: EnforcementAction,
}

pub enum EnforcementCondition {
    RiskLevelExceeds(RiskLevel),           // 工具风险等级超过阈值
    RequiresPermission(Permission),        // 需要特定权限
    ExternalNetworkAccess,                  // 涉及外部网络访问
    ToolNotInAllowlist(Vec<String>),       // 工具不在白名单
    ContextContainsSensitiveData,           // 上下文包含敏感数据
    RateLimitExceeded { tool: String, max_qps: u32 },
}

pub enum EnforcementAction {
    Allow,                                  // 允许执行（仅在明确匹配 allow 规则时）
    Deny { reason: String },               // 拒绝执行（默认行为）
    RequireApproval { approver: String },  // 需要人工审批
    LogOnly,                                // 允许但记录审计日志
    MaskOutput { fields: Vec<String> },    // 执行但脱敏输出
}

/// PolicyEnforcer 核心原则：Default Deny
/// 任何工具调用必须通过至少一条 Allow 规则，否则默认拒绝
impl PolicyEnforcer {
    pub fn evaluate(&self, tool: &Tool, ctx: &StrategyContext) -> EnforcementAction {
        // 1. 先检查是否有明确的 Deny 规则命中 → 直接拒绝
        for rule in &self.rules {
            if rule.condition.evaluate(tool, ctx) && matches!(rule.action, EnforcementAction::Deny{..}) {
                return EnforcementAction::Deny {
                    reason: format!("规则 '{}' 明确拒绝", rule.name),
                };
            }
        }

        // 2. 检查是否有明确的 Allow 规则命中 → 允许
        for rule in &self.rules {
            if rule.condition.evaluate(tool, ctx) && matches!(rule.action, EnforcementAction::Allow) {
                return EnforcementAction::Allow;
            }
        }

        // 3. 无任何规则匹配 → Default Deny
        EnforcementAction::Deny {
            reason: "未匹配任何允许规则（Default Deny）".to_string(),
        }
    }
}
```

**强制执行点：**

```rust
impl StrategyExecutor {
    async fn execute_tool_with_enforcement(
        &self,
        tool: &Tool,
        args: &serde_json::Value,
        ctx: &StrategyContext,
    ) -> Result<ToolResult, AgentErrorKind> {
        // 1. 检查权限
        for permission in &tool.metadata().permissions {
            if !ctx.auth().has_permission(permission) {
                return Err(AgentErrorKind::PermissionDenied {
                    tool: tool.id(),
                    required: vec![permission.clone()],
                });
            }
        }

        // 2. 检查风险等级
        if tool.metadata().risk_level >= RiskLevel::High {
            ctx.audit_log().record(AuditEvent::HighRiskToolAccess {
                tool: tool.id(),
                args: args.clone(),
                user: ctx.auth().user_id(),
            });
        }

        // 3. 检查外部网络
        if tool.metadata().external_deps.is_empty() {
            // 内部工具，直接执行
        } else {
            // 外部工具，额外检查网络权限
            if !ctx.auth().can_access_external_network() {
                return Err(AgentErrorKind::PermissionDenied {
                    tool: tool.id(),
                    required: vec![Permission::ExternalNetwork],
                });
            }
        }

        // 4. 执行
        tool.execute(args).await
    }
}
```

**MaskOutput 执行层级：**

```
Tool 执行 ──► Tool 原始返回 ──► PolicyEnforcer.MaskOutput ──► 脱敏后数据 ──► Strategy 使用
                    │
                    └──► AuditLog（记录原始数据，用于审计，不进入 LLM）
```

- `MaskOutput` 发生在 tool 返回后、进入 strategy 使用前
- 脱敏后的数据进入 answer LLM prompt
- 原始数据进入审计日志（权限隔离，只有审计系统可读）
- 这样保证：LLM 看不到敏感字段，但审计需要时可以追溯

**与 v4 guard pipeline 的关系：**
- v4 的 `guard_pipeline` 保留，负责 prompt injection 和输出内容安全
- v5 的 `PolicyEnforcer` 是新增的独立层，负责权限、风险、审计、速率控制
- 两者互补：guard pipeline 管"内容安全"，policy enforcer 管"权限与合规"

### 6. 安全边界：输入分层与隔离

v5 明确区分四类输入的来源和信任等级，定义哪些内容可以进入哪些阶段：

| 输入类型 | 来源 | 信任等级 | 允许进入的阶段 | 隔离要求 |
|---------|------|---------|--------------|---------|
| **系统提示** | 平台预定义 | 高 | Plan / Evaluate / Answer | 不可被用户输入覆盖 |
| **用户输入** | 前端/API | 中 | Plan（经过 guard） | 必须过 prompt injection 检测 |
| **检索内容** | RAG/Search 工具 | 低 | Answer（只读引用） | **不可进入 Plan/Evaluate**，防止检索投毒 |
| **工具输出** | 外部 API | 低 | Answer（结构化引用） | 不可直接进入 planner 的 reasoning |

**关键规则：**
- Planner（Plan 阶段）只能看到**系统提示 + 用户输入 + 工具 schema**，不能看到工具的实际输出内容
- Evaluator（Evaluate 阶段）只能看到**评估信号（结构化数字）+ 用户输入**，不能看到原始检索文本
- Answer 阶段可以看到**系统提示 + 用户输入 + 检索内容 + 工具输出**，用于生成自然语言回答
- 任何外部内容（检索结果、网页、API 返回）进入 LLM 前必须经过 guard pipeline

**输入数据流允许矩阵：**

| 输入类型 | 来源 | 信任等级 | Plan | Evaluate | Answer | 审计日志 | 隔离要求 |
|---------|------|---------|------|---------|--------|---------|---------|
| **系统提示** | 平台预定义 | 高 | ✅ 允许 | ✅ 允许 | ✅ 允许 | ❌ 不包含 | 不可被用户输入覆盖 |
| **用户输入** | 前端/API | 中 | ✅ 允许（guard 后） | ✅ 允许（guard 后） | ✅ 允许（guard 后） | ✅ 记录原始输入 | 必须过 prompt injection 检测 |
| **工具 schema** | Registry 定义 | 高 | ✅ 允许（只读） | ❌ 不允许 | ❌ 不允许 | ❌ 不包含 | 只暴露签名，不暴露实现 |
| **评估信号** | Evaluator 计算 | 高 | ❌ 不允许 | ✅ 允许 | ⚠️ 仅摘要 | ✅ 记录完整信号 | 结构化数字指标 |
| **检索内容** | RAG/Search 工具 | 低 | ❌ **禁止** | ❌ **禁止** | ✅ 允许（处理后） | ✅ 记录原始内容 | 必须经 Sanitization |
| **工具输出** | 外部 API | 低 | ❌ **禁止** | ❌ **禁止** | ✅ 允许（处理后） | ✅ 记录原始内容 | 必须经 Sanitization |
| **推理过程** | Planner/Evaluator | 中 | ✅ 内部使用 | ✅ 内部使用 | ❌ 不暴露给用户 | ✅ 记录（debug） | 不进入最终 answer |

> ✅ 允许 / ❌ 禁止 / ⚠️ 受限

**关键规则：**
- 任何标记为 ❌ **禁止** 的输入如果进入了对应阶段，视为安全漏洞
- 审计日志与 LLM prompt 完全隔离：审计系统可以读取原始内容，LLM 只能读取处理后的版本
- `PolicyEnforcer` 在运行时校验：若某阶段试图访问禁止的输入类型，立即 Deny

**核心安全原则（不可违背）：**

> **原始检索内容和工具输出只能进入审计层与受限调试层，默认不得进入 planner/evaluator 的完整上下文。**
>
> 违反此原则即视为安全漏洞。具体执行：
> 1. 检索结果进入 Answer 阶段前必须经过 `UntrustedInputProcessor` 处理（清洗/摘要/结构化封装至少一种）
> 2. 审计系统可以读取原始内容用于追溯，但审计日志与 LLM prompt 完全隔离（不同存储、不同权限、不同查询接口）
> 3. Debug 模式下 trace 可以包含原始内容的摘要或引用标记，但不得包含完整原始文本
> 4. `PolicyEnforcer` 在运行时发现某阶段试图将检索内容/工具输出注入 Plan 或 Evaluate，立即触发 `EnforcementAction::Deny`

**不可信输入处理原则：**

检索内容和工具输出默认视为**不可信输入**，不能直接注入 planner 的 system prompt 或 user message。必须经过以下处理之一：

1. **清洗（Sanitization）**：移除潜在的 prompt injection 模式（如 `\n\n---\n\nSYSTEM:`、`ignore previous instructions`）
2. **摘要（Summarization）**：将原始检索结果压缩为结构化摘要，只保留关键事实
3. **证据抽取（Evidence Extraction）**：从检索结果中提取可引用的片段，标注来源
4. **结构化封装（Structured Wrapping）**：将外部内容放入 JSON/XML 容器中，明确标注为"外部数据"

```rust
/// 不可信输入处理器
pub struct UntrustedInputProcessor;

impl UntrustedInputProcessor {
    /// 处理检索内容，返回安全版本
    pub fn sanitize_retrieval(raw: &str) -> SanitizedContent {
        // 1. 检测并标记潜在注入
        let injection_score = detect_prompt_injection(raw);
        // 2. 结构化封装
        let wrapped = format!("<ExternalEvidence source=\"retrieval\" trust=\"low\" injection_score=\"{}\">\n{}\n</ExternalEvidence>", injection_score, raw);
        // 3. 如果 injection_score 过高，拒绝使用该内容
        if injection_score > 0.8 {
            SanitizedContent::Rejected { reason: "潜在 prompt injection".to_string() }
        } else {
            SanitizedContent::Safe(wrapped)
        }
    }
}
```

**这对 tool chaining 尤其重要**：当工具输出需要作为下一个 planner 的输入时，必须经过上述处理，否则攻击面会随着"工具输出再喂给模型"快速扩大。

### 7. 跨模式编排（未来扩展）

v5 架构预留了"跨模式并行"的扩展点，不需要引入外部 graphflow 框架。

当未来需要支持"一个请求同时触发 RAG + Search"时：

```rust
/// 组合策略：并行执行多个子策略，然后汇聚
pub struct CompositeStrategy {
    pub branches: Vec<Box<dyn Strategy>>,
    pub aggregator: Box<dyn Aggregator>,
}

impl Strategy for CompositeStrategy {
    async fn step(&self, state: StateBox, ctx: &mut Context) -> Result<StepOutcome, AppError> {
        // 并行驱动所有子策略
        let results = futures::future::join_all(
            self.branches.iter().map(|s| s.run(ctx.fork()))
        ).await;
        // 汇聚结果
        let aggregated = self.aggregator.merge(results).await?;
        Ok(StepOutcome::Terminate(aggregated))
    }
}
```

这仅在需要跨模式协作时引入，不影响 Chat/RAG/Search 的独立状态机。

### 4.9 策略契约与状态边界

为了让 Executor 与 Strategy 之间的契约足够严格，同时支持后续新增 `CompositeStrategy`、`Handoff` 或 supervisor 模式，状态定义必须分层：

```rust
/// 所有状态必须实现的通用接口，供 Executor 使用
trait State {
    /// 状态标识符，用于观测和调试
    fn state_id(&self) -> &'static str;
    /// 状态分类，供 Executor 做通用处理（如超时、取消）
    fn state_kind(&self) -> StateKind;
    /// 序列化供观测使用
    fn to_observable(&self) -> serde_json::Value;
}

enum StateKind {
    Plan,       // 涉及 LLM planning
    Execute,    // 纯工具执行，不涉及 LLM
    Evaluate,   // 涉及评估/判断
    Answer,     // 最终生成
    Control,    // 控制流（Decompose, Aggregate 等）
}

**StateKind 映射表（具体状态 → 通用分类）：**

| 策略 | 具体状态 | StateKind | 说明 |
|------|---------|-----------|------|
| ChatStrategy | `ChatState::Plan` | Plan | LLM 决策是否调用工具 |
| ChatStrategy | `ChatState::ExecuteAtomic` | Execute | 纯工具执行 |
| ChatStrategy | `ChatState::Answer` | Answer | 最终回答生成 |
| RagStrategy | `RagState::Plan` | Plan | 检索策略规划 |
| RagStrategy | `RagState::ExecuteRetrieve` | Execute | 检索工具执行 |
| RagStrategy | `RagState::Evaluate` | Evaluate | 召回质量评估 |
| RagStrategy | `RagState::Answer` | Answer | 答案合成 |
| SearchStrategy | `SearchState::Decompose` | Plan | 查询分解（含 LLM） |
| SearchStrategy | `SearchState::ParallelSearch` | Execute | 并行 web_search |
| SearchStrategy | `SearchState::Aggregate` | Control | 去重排序（无 LLM） |
| SearchStrategy | `SearchState::Evaluate` | Evaluate | 结果充分性评估 |
| SearchStrategy | `SearchState::Answer` | Answer | 答案聚合生成 |

> **为什么需要 StateKind：** Executor 通过 `state.state_kind()` 判断当前状态类型，从而应用通用策略（如 Execute 状态可以设置更短的 LLM 超时——实际上 Execute 根本不调用 LLM；Evaluate 状态可以启用更严格的取消检查）。具体状态的命名和数量由 Strategy 自行决定，但分类必须归入这 5 种之一，以保证 Executor 的通用处理能力。

/// Strategy 上下文通用接口
/// 所有具体 Context 必须实现，供 Executor 注入通用能力
trait StrategyContext {
    fn trace_id(&self) -> &str;
    fn budget(&self) -> &LoopBudget;
    fn budget_mut(&mut self) -> &mut LoopBudget;
    fn sink(&self) -> &dyn EventSink;
    fn registry(&self) -> &CapabilityRegistry;
    fn cancel(&self) -> &CancellationToken;
}
```

**策略边界与复用关系：**

| 能力 | ChatStrategy | RagStrategy | SearchStrategy | 复用方式 |
|------|-------------|-------------|----------------|---------|
| planner skill | chat-plan | rag-plan | search-plan | 各策略独占 |
| evaluator skill | 无 | rag-eval | search-eval | 各策略独占 |
| answer skill | chat-answer | rag-answer | search-answer | 各策略独占 |
| LLM completion | ✅ | ✅ | ✅ | 能力层统一 |
| tool dispatch | ✅ (atomic) | ✅ (retrieval) | ✅ (web_search) | 能力层统一 |
| budget tick | ✅ (plan 时) | ✅ (plan 时) | ✅ (decompose/evaluate 时) | 各策略自行控制 |
| format skills | 无 | ppt/html/teaching | 关键词启发式 | RagStrategy 在 Plan 时选择 |

**关键原则：**
- Planner/Evaluator/Answer skill 是策略私有的（因为每个模式的输出格式和约束不同）
- LLM Provider、Tool Dispatch、Registry 查询是能力层共享的
- 策略之间不直接调用对方，跨模式协作通过 `CompositeStrategy` 实现

---

### 7. 评估体系（Eval as First-Class Citizen）

生产级 agent 不仅需要观测运行，还需要**持续评估质量**。v5 将评估体系纳入主设计：

#### 7.1 评估维度

**质量指标：**

| 维度 | 指标 | 采集方式 | 目标 |
|------|------|---------|------|
| **任务完成率** | 用户问题是否被完整回答 | 人工标注 + 自动评估 | > 95% |
| **引用正确率** | 引用与原文是否一致（RAG/Search） | 人工抽查 | > 98% |
| **答案可执行性** | 回答中的指令/代码是否可执行 | 自动化测试 | > 90% |
| **幻觉率** | 回答中无证据支持的断言比例 | 人工标注 + RAGAS | < 5% |
| **用户满意度** | 显式反馈（👍/👎）+ 隐式信号（是否追问） | 前端埋点 | > 90% |

**系统指标：**

| 维度 | 指标 | 采集方式 | 目标 |
|------|------|---------|------|
| **工具成功率** | 工具调用成功 / 失败 / 超时 / 降级的比例 | Metrics | > 99% |
| **延迟分布** | P50 / P95 / P99 的端到端耗时 | TraceSpan | P95 < 30s |
| **Token 效率** | 每轮迭代的 prompt/completion tokens | LLM span | 可控 |
| **成本** | 单次运行的预估费用（按 model 定价） | Usage 数据 | 可控 |
| **预算耗尽率** | BudgetExhausted / 总运行次数 | FinalDecision | < 3% |
| **策略重规划率** | Replan 次数 / 总运行次数 | StateEvent | < 20% |
| **工具失败恢复率** | 失败后成功降级或重试的比例 | Metrics | > 95% |
| **平均每任务 tool call 数** | 单次运行的平均工具调用次数 | Metrics | Chat < 2, RAG < 8 |
| **回放一致性** | 同一快照多次回放的结果一致性 | ReplaySnapshot | > 99% |

#### 7.2 评估流程

```rust
pub enum EvalTrigger {
    /// PR 级别：每次代码变更后自动跑小样本评测
    PreMerge { dataset: String, sample_size: usize },
    /// 夜间回归：每天凌晨跑全量评测集
    NightlyRegression { dataset: String },
    /// 线上采样：对生产流量的 1% 做实时评估
    OnlineSampling { rate: f64 },
    /// 红队测试：定期注入对抗样本
    RedTeam { attack_vectors: Vec<AttackVector> },
}

pub struct EvalResult {
    pub trigger: EvalTrigger,
    pub pass_rate: f64,
    pub avg_latency_ms: u64,
    pub avg_tokens: u64,
    pub failures: Vec<EvalFailure>,
    pub comparison: Option<EvalComparison>, // 与上一版本的对比
}
```

#### 7.3 评估数据集

```
eval/
  datasets/
    chat_basics.jsonl          # Chat 基础问答
    chat_tools.jsonl           # Chat 工具调用
    rag_single_turn.jsonl      # RAG 单轮检索
    rag_multi_turn.jsonl       # RAG 多轮迭代
    search_factual.jsonl       # Search 事实查询
    search_opinion.jsonl       # Search 观点聚合
  redteam/
    prompt_injection.jsonl     # Prompt 注入攻击
    tool_abuse.jsonl           # 工具滥用尝试
    data_exfiltration.jsonl    # 数据外泄尝试
```

---

### 8. ReplaySnapshot 升级

```rust
pub struct ReplaySnapshot {
    pub trace_id: String,
    pub request: AgentRequest,

    // 环境版本快照
    pub environment: EnvironmentSnapshot,

    // 运行时响应快照
    pub llm_responses: Vec<LlmResponse>,
    pub tool_responses: Vec<ToolResponse>,
    pub rng_seed: u64,
}

pub struct EnvironmentSnapshot {
    pub strategy_version: String,
    pub registry_version: String,
    pub router_version: String,
    pub model_versions: HashMap<String, String>, // model_id -> version
    pub tool_versions: HashMap<String, String>,  // tool_id -> version
    pub skill_versions: HashMap<String, String>, // skill_id -> version
}

pub struct ToolResponse {
    pub tool: String,
    pub args: serde_json::Value,
    pub result: serde_json::Value,
    pub is_replayable: bool,    // 是否可安全重放
    pub replay_note: Option<String>, // "网页内容已过期，不可重放"
}
```

**外部依赖可重放标记：**

| 工具类型 | 可重放性 | 说明 |
|---------|---------|------|
| 内部检索（dense/bm25/graph） | ✅ 可重放 | 同一 doc_scope + query 的结果稳定 |
| 向量数据库查询 | ✅ 可重放 | 向量索引版本化后可回放 |
| Web Search | ❌ 不可重放 | 网页内容实时变化 |
| 天气查询 | ❌ 不可重放 | 时间敏感 |
| 计算工具 | ✅ 可重放 | 纯函数，相同输入必相同输出 |

不可重放的工具在回放时必须 mock，或标记为"结果可能不同"。

---

## 9. 运行治理（Governance）

### 9.1 权限与审批

| 操作 | 默认权限 | 是否需要审批 |
|------|---------|------------|
| 调用 Low Risk 工具 | 所有用户 | 否 |
| 调用 Medium Risk 工具 | 认证用户 | 否 |
| 调用 High Risk 工具 | 高级用户 | 首次需要，后续自动 |
| 调用 Critical Risk 工具 | 管理员 | 每次人工审批 |
| 访问外部网络 | 认证用户 | 否（但审计日志） |
| 修改 RouterPolicy | 管理员 | 是（双人审批） |
| 注册新工具 | 工具 owner | 是（安全审核） |

### 9.2 风险等级与上下文

```rust
pub enum ContextRiskLevel {
    Internal,       // 内部知识库查询，无敏感数据
    Confidential,   // 包含商业机密或用户隐私
    Public,         // 对外公开发布的内容
}

/// 高风险工具在低风险上下文中可以自动执行
/// 低风险工具在高风险上下文中需要额外确认
pub fn tool_allowed(tool_risk: RiskLevel, context_risk: ContextRiskLevel) -> bool {
    match (tool_risk, context_risk) {
        (RiskLevel::Low, _) => true,
        (RiskLevel::Medium, ContextRiskLevel::Internal) => true,
        (RiskLevel::High, ContextRiskLevel::Internal) => false, // 需要审批
        (RiskLevel::Critical, _) => false, // 必须人工审批
        _ => false,
    }
}
```

### 9.3 审计与留痕

所有以下事件必须写入审计日志：
- 策略路由决策（`RoutingDecision`）
- 高风险工具调用（`risk_level >= High`）
- Policy Enforcement 的 Deny / RequireApproval 决策
- 预算耗尽或降级事件
- 权限被拒绝事件
- 人工审批结果

审计日志保留期：90 天在线查询 + 1 年冷存储。

### 9.4 红队与回归测试

**上线前（Pre-release）：**
- 运行 `redteam/prompt_injection.jsonl` —— 验证 guard pipeline 有效性
- 运行 `redteam/tool_abuse.jsonl` —— 验证 policy enforcement 有效性
- 运行 `redteam/data_exfiltration.jsonl` —— 验证数据隔离边界

**上线后（Post-release）：**
- 每周自动运行红队测试（随机抽取 100 条对抗样本）
- 每月人工审查审计日志中的异常模式
- 每季度更新评估数据集（加入新发现的 failure case）

---

## 文档阅读指南

本文档按以下五层组织，建议按顺序阅读：

| 层级 | 内容 | 章节 | 读者 |
|------|------|------|------|
| **原则层** | 为什么这么设计 | 背景、设计目标、与 v4 的差异 | 产品经理、架构师 |
| **架构层** | 核心抽象与分层 | 能力层、策略层、编排层、路由层、PolicyEnforcer | 后端工程师 |
| **运行层** | 状态机、事件、trace | API 白盒化、观测设计、失败处理 | SRE、观测工程师 |
| **治理层** | 权限、风险、评估、回放 | 运行治理、评估体系、ReplaySnapshot | 安全工程师、QA |
| **实现层** | Rust trait、struct、示例代码 | 所有代码块 | 实现工程师 |

**文档结构建议（实现阶段拆分）：**

本文档当前是"全合一"架构规范。进入实现阶段后，建议拆分为：

- **主文档（本文档）**：保留原则层、架构层、运行层、治理层的核心描述和决策表格，作为架构共识的单一来源
- **附录 A —— 代码契约**：所有 Rust trait/struct/enum 定义（`Strategy`、`State`、`AgentErrorKind`、`StateEvent` 等）
- **附录 B —— 错误处理矩阵**：`AgentErrorKind` 全变体 × 策略的降级行为表、`ErrorHandlingStrategy` 映射、`RetryPolicy` 默认值
- **附录 C —— 指标与观测**：Metrics 指标名、标签、采集方式；`StateEvent` 完整字段；TraceSpan 结构
- **附录 D —— 序列图与时序**：端到端时序图（已在本章 15 节）、状态转换图、数据流图
- **附录 E —— 评估数据集**：`eval/` 目录结构、数据集格式、红队测试用例模板

拆分原则：主文档回答"为什么和是什么"，附录回答"具体怎么定义"。主文档修改需架构评审，附录修改可由实现 owner 直接更新。

---

## 10. 失败处理与可靠性

### 6.1 失败分类

v4 的错误处理是统一的 `AppError`，v5 将失败明确分类，策略才能做出合理降级：

```rust
pub enum AgentErrorKind {
    // 工具层错误 —— 可重试
    ToolExecutionFailed { tool: String, reason: String },
    ToolTimeout { tool: String, timeout_ms: u64 },
    ToolRateLimited { tool: String, retry_after_ms: Option<u64> },

    // 工具层错误 —— 不可重试
    ToolDeprecated { tool: String },
    ToolSchemaMismatch { tool: String, expected: String, got: String },
    ToolOutputMalformed { tool: String, raw: String },

    // 模型层错误 —— 可重试
    ModelUnavailable { provider: String, model: String },
    ModelRateLimited,

    // 模型层错误 —— 不可重试
    ModelContextExceeded { used_tokens: u64, max_tokens: u64 },
    ModelOutputInvalid { expected_schema: String, got: String },
    ModelOutputSchemaMismatch { expected: String, got: serde_json::Value },

    // 预算/资源错误 —— 不可重试
    BudgetExhausted { current: u8, max: u8 },
    ContextWindowExceeded,

    // 权限错误 —— 不可重试
    PermissionDenied { tool: String, required: Vec<Permission> },

    // 外部依赖错误 —— 可重试
    ExternalDependencyFailed { service: String, error: String },

    // 未知错误
    Unknown(String),
}

impl AgentErrorKind {
    /// 错误是否可重试
    pub fn is_retriable(&self) -> bool {
        matches!(self,
            AgentErrorKind::ToolExecutionFailed { .. } |
            AgentErrorKind::ToolTimeout { .. } |
            AgentErrorKind::ToolRateLimited { .. } |
            AgentErrorKind::ModelUnavailable { .. } |
            AgentErrorKind::ModelRateLimited |
            AgentErrorKind::ExternalDependencyFailed { .. }
        )
    }

    /// 错误是否可降级（跳过该工具/模型，继续其他流程）
    pub fn is_degradable(&self) -> bool {
        matches!(self,
            AgentErrorKind::ToolExecutionFailed { .. } |
            AgentErrorKind::ToolTimeout { .. } |
            AgentErrorKind::ToolSchemaMismatch { .. } |
            AgentErrorKind::ToolOutputMalformed { .. } |
            AgentErrorKind::ExternalDependencyFailed { .. }
        )
    }

    /// 错误的最小处理策略——策略实现必须至少达到此处理级别
    pub fn minimum_strategy(&self) -> ErrorHandlingStrategy {
        match self {
            // 可重试错误：至少重试一次
            AgentErrorKind::ToolExecutionFailed { .. } => ErrorHandlingStrategy::Retry,
            AgentErrorKind::ToolTimeout { .. } => ErrorHandlingStrategy::Retry,
            AgentErrorKind::ToolRateLimited { .. } => ErrorHandlingStrategy::Retry,
            AgentErrorKind::ModelUnavailable { .. } => ErrorHandlingStrategy::Retry,
            AgentErrorKind::ModelRateLimited => ErrorHandlingStrategy::Retry,
            AgentErrorKind::ExternalDependencyFailed { .. } => ErrorHandlingStrategy::Retry,

            // 可降级但不可重试：至少跳过该工具继续
            AgentErrorKind::ToolSchemaMismatch { .. } => ErrorHandlingStrategy::Skip,
            AgentErrorKind::ToolOutputMalformed { .. } => ErrorHandlingStrategy::Skip,

            // 模型输出格式错误：尝试 fallback（简化 prompt 或换模型）
            AgentErrorKind::ModelOutputInvalid { .. } => ErrorHandlingStrategy::Fallback,
            AgentErrorKind::ModelOutputSchemaMismatch { .. } => ErrorHandlingStrategy::Fallback,

            // 上下文超限：尝试压缩或截断后重试
            AgentErrorKind::ModelContextExceeded { .. } => ErrorHandlingStrategy::Fallback,

            // 预算耗尽：用已有结果合成答案
            AgentErrorKind::BudgetExhausted { .. } => ErrorHandlingStrategy::Fallback,

            // 权限错误：不暴露原因，返回通用拒绝
            AgentErrorKind::PermissionDenied { .. } => ErrorHandlingStrategy::MaskAndContinue,

            // 工具已废弃/严重错误：直接终止
            AgentErrorKind::ToolDeprecated { .. } => ErrorHandlingStrategy::Abort,
            AgentErrorKind::ContextWindowExceeded => ErrorHandlingStrategy::Abort,
            AgentErrorKind::Unknown(_) => ErrorHandlingStrategy::Abort,
        }
    }
}

/// 错误处理策略的最低要求——策略实现可以选择更强处理，不能更弱
pub enum ErrorHandlingStrategy {
    /// 重试：使用退避策略重新执行同一操作
    Retry,
    /// 降级：使用简化方案替代（如 fallback plan、压缩上下文、单工具替代多工具）
    Fallback,
    /// 跳过：忽略当前失败的工具/步骤，继续其他流程
    Skip,
    /// 终止：立即停止运行，返回已积累的结果或错误
    Abort,
    /// 脱敏后继续：掩盖敏感信息后返回通用响应，不暴露内部错误细节
    MaskAndContinue,
}
```

**策略降级行为：**

| 错误类型 | 可重试 | 可降级 | ChatStrategy | RagStrategy | SearchStrategy |
|---------|--------|--------|-------------|-------------|----------------|
| ToolExecutionFailed | ✅ | ✅ | 返回错误 | 跳过该工具，继续其他检索 | 跳过该查询，继续其他 sub_queries |
| ToolTimeout | ✅ | ✅ | 返回错误 | 跳过该工具，继续其他检索 | 跳过该查询，继续其他 sub_queries |
| ToolRateLimited | ✅ | ✅ | 等待后重试 | 等待后重试 | 等待后重试 |
| ToolDeprecated | ❌ | ❌ | 返回错误 | 返回错误 | 返回错误 |
| ToolSchemaMismatch | ❌ | ✅ | 返回错误 | 跳过该工具 | 跳过该工具 |
| ToolOutputMalformed | ❌ | ✅ | 返回错误 | 跳过该工具 | 跳过该工具 |
| ModelUnavailable | ✅ | ❌ | 返回错误 | 降级到 fallback plan（直接查原始 query） | 降级到 fallback（单 query 搜索） |
| ModelRateLimited | ✅ | ❌ | 等待后重试 | 等待后重试 | 等待后重试 |
| ModelContextExceeded | ❌ | ❌ | 压缩上下文后重试 | 压缩上下文后重试 | 压缩上下文后重试 |
| ModelOutputInvalid | ❌ | ❌ | 返回错误 | 返回错误 | 返回错误 |
| ModelOutputSchemaMismatch | ❌ | ❌ | 返回错误 | 返回错误 | 返回错误 |
| BudgetExhausted | ❌ | ❌ | 直接 Answer | Synthesize（用已有结果回答） | Synthesize（用已有结果回答） |
| PermissionDenied | ❌ | ❌ | 返回错误 | 跳过该工具，记录审计日志 | 跳过该工具，记录审计日志 |
| ExternalDependencyFailed | ✅ | ✅ | 返回错误 | 跳过该依赖 | 跳过该依赖 |

### 6.2 重试与幂等

每个工具在注册时声明自己的重试策略：

```rust
pub struct RetryPolicy {
    pub max_retries: u32,           // 最大重试次数
    pub backoff_ms: u64,            // 初始退避时间
    pub backoff_multiplier: f64,    // 退避倍数（指数退避）
    pub max_backoff_ms: u64,        // 最大退避时间
    pub idempotent: bool,           // 是否幂等
    pub idempotency_key_header: Option<String>, // 幂等键 header
}

// 默认策略
impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff_ms: 1000,
            backoff_multiplier: 2.0,
            max_backoff_ms: 30000,
            idempotent: false,
            idempotency_key_header: None,
        }
    }
}
```

**工具注册时指定策略：**

```rust
registry.register_tool(
    ToolMetadata {
        id: "web_search".to_string(),
        retry_policy: RetryPolicy {
            max_retries: 2,
            backoff_ms: 500,
            idempotent: true,           // 相同 query 的搜索结果可以安全重试
            idempotency_key_header: Some("X-Idempotency-Key".to_string()),
            ..Default::default()
        },
        ..,
    },
    Box::new(WebSearchTool),
);
```

### 6.3 回放与复现

v5 保证：给定同一输入、同一策略版本、同一工具版本、同一上下文快照，可以回放同一次运行。

**实现方式：**

```rust
pub struct ReplaySnapshot {
    pub trace_id: String,
    pub request: AgentRequest,
    pub strategy_version: String,           // strategy 代码版本
    pub registry_version: String,           // tool/skill registry 版本
    pub llm_responses: Vec<LlmResponse>,   // 所有 LLM 调用的响应（用于 mock）
    pub tool_responses: Vec<ToolResponse>, // 所有工具调用的响应（用于 mock）
    pub rng_seed: u64,                      // 随机数种子（如果有随机性）
}

/// 运行一次并生成回放快照
pub async fn run_with_snapshot(
    strategy: &dyn Strategy,
    request: AgentRequest,
) -> (AgentRunResult, ReplaySnapshot);

/// 用快照回放
pub async fn replay(
    snapshot: &ReplaySnapshot,
) -> AgentRunResult;
```

**用途：**
- 调试：用户反馈"上次结果不对"，加载快照回放定位问题
- 评估：同一快照用不同策略版本回放，对比输出差异
- 回归测试：将生产快照加入测试套件

---

## 11. 非功能性要求

### 7.1 延迟目标

| 指标 | 目标 | 说明 |
|------|------|------|
| 首 token 延迟 | < 2s (Chat) / < 3s (RAG/Search) | 从请求到第一个 answer token |
| Plan 阶段延迟 | < 1.5s | LLM planning 完成时间 |
| 工具调用延迟 | < 3s (P99) | 单次工具调用 |
| 总完成时间 | < 10s (Chat) / < 30s (RAG/Search) | 端到端 |

### 7.2 成功率目标

| 指标 | 目标 |
|------|------|
| Agent run 成功率 | > 99.5% |
| 工具调用成功率 | > 99% |
| LLM 调用成功率 | > 99.5% |
| 最终回答可用率 | > 98% |

### 7.3 观测指标

- 每次 run 必须生成完整 trace（含 span、event、metrics）
- 所有 LLM 调用必须记录 prompt/completion tokens
- 所有工具调用必须记录耗时、成功率、错误类型
- 预算消耗必须实时上报

### 7.4 安全与权限

- 每个 tool/skill 必须声明 `risk_level` 和 `permissions`
- 高风险工具（如 web_search、code_interpreter）必须审计日志
- Prompt injection 防护在 guard pipeline 中处理（保留 v4 机制）
- 敏感数据（如 auth_context）不进入 LLM prompt，不进入 trace

### 7.5 兼容性与版本策略

- Strategy、Skill、Tool 都有版本号
- Registry 支持多版本共存（如 `rag-plan@v1` 和 `rag-plan@v2` 同时存在）
- 策略通过 `strategy_version` 指定使用哪个版本的 skill/tool
- 退役流程：标记 deprecated → 6 个月兼容期 → 强制升级

### 版本兼容规则

**谁控制兼容性：**

```rust
/// 兼容性矩阵
pub struct CompatibilityMatrix {
    /// strategy 依赖的最小 tool 版本
    pub strategy_tool_deps: HashMap<String, HashMap<String, SemVerReq>>,
    /// strategy 依赖的最小 skill 版本
    pub strategy_skill_deps: HashMap<String, HashMap<String, SemVerReq>>,
    /// registry 允许多版本共存的最大数量
    pub max_concurrent_versions: u32,
    /// 回放时版本不一致的判定规则
    pub replay_compatibility: ReplayCompatibility,
}

pub enum ReplayCompatibility {
    /// 版本必须完全一致，否则标记为不可重放
    Strict,
    /// 允许 patch 级别差异（如 v1.2.3 vs v1.2.4）
    PatchLevel,
    /// 允许 minor 级别差异（如 v1.2.x vs v1.3.x）
    MinorLevel,
    /// 允许任何差异，但标记置信度
    BestEffort { confidence: f64 },
}
```

**Registry 多版本共存规则：**
- 同一 tool/skill 最多同时存在 3 个版本（当前 + deprecated + beta）
- strategy 启动时锁定依赖版本，运行期间不自动升级
- 若回放时发现 strategy_version 与 snapshot 不一致：
  - `Strict` → 标记为不可重放，停止回放
  - `PatchLevel` → 允许继续，但记录版本差异警告
  - `MinorLevel` → 允许继续，评估置信度降级
  - `BestEffort` → 继续回放，结果仅供参考

**版本不一致时的判定流程：**
```
回放请求 ──► 读取 snapshot.environment ──► 对比当前环境版本
    │
    ├── 完全一致 ──► ✅ 可重放
    │
    ├── patch 差异 ──► ReplayCompatibility::PatchLevel ──► ⚠️ 可重放（带警告）
    │
    ├── minor 差异 ──► ReplayCompatibility::MinorLevel ──► ⚠️ 可重放（置信度降级）
    │
    └── major 差异 ──► ReplayCompatibility::Strict ──► ❌ 不可重放
```

**回放结果标签：**

每次回放完成后，结果必须标记以下标签之一，供评估系统和调试界面使用：

| 标签 | 条件 | 说明 |
|------|------|------|
| `replayed_exact` | 环境版本完全一致，所有可重放工具输出与快照一致 | 完全复现，结果可信 |
| `replayed_with_warning` | 环境存在 patch 级差异，或部分不可重放工具被 mock | 结果近似，差异在可控范围 |
| `best_effort` | 环境存在 minor 级差异，或 ReplayCompatibility 为 BestEffort | 结果仅供参考，不用于精确对比 |
| `not_replayable` | 环境存在 major 级差异，或缺少必需快照数据 | 无法回放，需要重新运行 |

```rust
pub enum ReplayResultTag {
    /// 完全复现
    ReplayedExact,
    /// 近似复现（有警告）
    ReplayedWithWarning { warnings: Vec<String> },
    /// 尽力回放（置信度降级）
    BestEffort { confidence: f64, notes: Vec<String> },
    /// 不可回放
    NotReplayable { reason: String },
}

pub struct ReplayResult {
    pub result: AgentRunResult,
    pub tag: ReplayResultTag,
    pub environment_diff: Option<EnvironmentDiff>, // 环境差异详情
}
```

---

## 与 v4 的关键差异

| 维度 | v4 (Bundle + ProgressiveLoop) | v5 (Strategy + Executor) |
|------|------------------------------|-------------------------|
| **流程假设** | 所有模式共享 `Plan→Execute→Evaluate→Answer` | 每个模式定义自己的状态和转移 |
| **Chat Evaluate** | 空转（直接 `Continue`） | 不存在 Evaluate 状态 |
| **Search Plan** | 空转（planning 在 Init 完成） | 不存在 Plan 状态，Decompose 替代 |
| **能力扩展** | Bundle 里硬编码 tool/skill 列表 | ToolRegistry/SkillRegistry 统一注册 |
| **API 可见性** | `iterations: [...]` 聚合结果 | 每个状态转换都有结构化事件 |
| **Plan 阶段 prompt** | Bundle 构建完整 system prompt | Strategy 调用 SkillRegistry 构建 |
| **预算管理** | `state.budget` + LoopDriver 检查 | Strategy 自行 tick 和检查 |
| **新增模式成本** | 需要适配 LoopDriver 的固定相位 | 只需实现 Strategy trait |

---

## 迁移路径

### Phase 1：保留能力层（1-2 天）
- 保留 `PromptRegistry` 和 `ToolRegistry`
- 保留 `Skill` / `Tool` / `DisclosureUnit` 定义
- 移除 `DisclosureTier` 中的 `Index`（已死）

### Phase 2：拆分策略（3-5 天）
- 新建 `ChatStrategy`、`RagStrategy`、`SearchStrategy`
- 从 `mode_chat.rs` / `mode_rag.rs` / `mode_search.rs` 提取状态转移逻辑
- 每个 Strategy 内部使用 `match state { ... }` 模式

### Phase 3：替换 LoopDriver（1-2 天）
- 新建 `StrategyExecutor`
- `UnifiedAgent::run()` 改为 `match kind { Chat => executor.run(ChatStrategy, ...), ... }`
- 移除 `ProgressiveLoop` 和 `LoopAdapter`

### Phase 4：API 白盒化（2-3 天）
- 新增 `StateEvent` 事件类型
- `StrategyExecutor` 在每个 step 前后 emit 事件
- 新增 `/agent/capabilities` 接口

### Phase 5：清理（1 天）
- 移除 `Phase` / `PhaseConfig` / `DisclosureContext` 中不再使用的字段
- 更新测试

---

## 决策

1. **废弃 `ProgressiveLoop` 和 `LoopAdapter`**，替换为 `Strategy` trait + `StrategyExecutor`
2. **保留并强化能力层**：`ToolRegistry` / `SkillRegistry` 成为全局统一注册表
3. **每个模式实现独立的 `Strategy`**：ChatStrategy / RagStrategy / SearchStrategy
4. **引入 `StateEvent` 事件流**：实现 API 白盒化
5. **暂不引入 graphflow**：跨模式并行通过 `CompositeStrategy` 预留，等有真实需求时实现

---

## 12. 差距分析：v5 ADR 初稿 vs 落地所需

| 主题 | 初稿已有 | 评审后补充 | 状态 |
|------|---------|-----------|------|
| **Strategy 状态机** | `Strategy` trait + `StepOutcome` | 明确 `State` / `StrategyContext` 通用接口、状态分类 (`StateKind`)、策略边界与复用关系表 | ✅ 已补 |
| **Registry** | `ToolRegistry` / `SkillRegistry` | 能力元数据模型 (`ToolMetadata` / `SkillMetadata`)、owner、version、`risk_level`、permissions、`retry_policy`、deprecation | ✅ 已补 |
| **Observability** | `StateEvent` 枚举 | Trace/Span 结构、Metrics 指标表、事件流与 trace 的关系 | ✅ 已补 |
| **Routing** | `request.kind` 隐式映射 | 显式 `RouterPolicy` + `RouterRule`、条件匹配、优先级、可覆盖性、决策解释 | ✅ 已补 |
| **Reliability** | `budget.tick` / `replan` / `degrade` | 失败分类 (`AgentErrorKind`)、策略降级行为表、重试与幂等 (`RetryPolicy`)、回放与复现 (`ReplaySnapshot`) | ✅ 已补 |
| **API 治理** | `GET /agent/capabilities` | 版本化 (`api_version` / `registry_version`)、权限声明 (`permissions` / `external_tools_used` / `requires_internet`)、退役标记 | ✅ 已补 |
| **运行结果** | `answer` + `iterations` | 结构化字段 (`trace_id` / `decisions` / `tool_calls` / `budget_used` / `state_history` / `eval_summary`) | ✅ 已补 |
| **非功能性要求** | 无 | 延迟目标、成功率目标、观测指标、安全与权限、兼容性与版本策略 | ✅ 已补 |

---

## 13. 下一步行动

> ✅ ADR 评审已完成（2026-05-21），架构方向已冻结，状态 Accepted。

1. **Phase 1-2 并行启动** —— 能力层元数据模型 + ChatStrategy 原型可以同时开工
2. **定义 `AgentErrorKind` 的精确变体** —— 与现有 `AppError` 对齐，避免重复
3. **确定 Metrics 上报方式** —— Prometheus / OTLP / 内部指标系统？
4. **ReplaySnapshot 的存储策略** —— 存在哪、存多久、谁可以访问
5. **按"文档结构建议"拆分附录** —— 实现阶段将代码契约、错误矩阵、指标表、序列图、评估数据集拆分到独立附录文件

---

## 14. 架构评审清单

### P0 —— 必改（不实现则架构不完整）

| # | 项 | 说明 | 所在章节 |
|---|----|------|---------|
| 1 | `Strategy` trait + `StrategyExecutor` | 废弃 `ProgressiveLoop`，每个模式独立状态机 | 4.2 |
| 2 | `CapabilityRegistry` 统一注册 | Tool/Skill 全局注册，新增能力自动可见 | 4.1 |
| 3 | `RouterPolicy` 确定性决策 | 优先级、风险更低、tie-break、可解释 | 4.4 |
| 4 | `PolicyEnforcer` Default Deny | 权限、风险、审计、速率控制 | 5 |
| 5 | `AgentErrorKind` 失败分类 | 工具/模型/预算/权限/外部依赖，策略降级 | 10 |
| 6 | `StateEvent` 事件流 | PlanDecision/ToolResult/Evaluation/BudgetTick | 4.5 |
| 7 | 安全边界 —— 输入分层隔离 | 系统提示/用户输入/检索内容/工具输出的信任等级 | 6 |

### P1 —— 建议改（显著提升可运维性）

| # | 项 | 说明 | 所在章节 |
|---|----|------|---------|
| 8 | `TraceSpan` 全链路追踪 | trace_id、parent_id、attributes、error | 4.5.1 |
| 9 | Metrics 指标表 | agent_run、tool_call、llm_request、budget 等指标 | 4.5.2 |
| 10 | `ToolMetadata` / `SkillMetadata` | owner、version、risk_level、permissions、retry_policy | 4.1 |
| 11 | `ReplaySnapshot` + `EnvironmentSnapshot` | 版本化回放，可重放边界标记 | 8 |
| 12 | `AgentRunResult` 结构化字段 | trace_id、decisions、tool_calls、budget_used | 4.6 |
| 13 | `/agent/capabilities` 版本化 | api_version、registry_version、权限声明、退役标记 | 4.5.3 |
| 14 | 评估体系（EvalTrigger） | PR/夜间/线上/红队，质量指标 + 系统指标 | 7 |
| 15 | `RetryPolicy` 重试与幂等 | max_retries、backoff、idempotency_key | 10.2 |

### P2 —— 可后置（有收益但不阻塞上线）

| # | 项 | 说明 | 所在章节 |
|---|----|------|---------|
| 16 | `CompositeStrategy` 跨模式编排 | RAG + Search 并行，预留扩展点 | 7 |
| 17 | `UntrustedInputProcessor` | 检索内容/工具输出的清洗、摘要、证据抽取 | 6 |
| 18 | 红队数据集与定期测试 | prompt_injection、tool_abuse、data_exfiltration | 9.4 |
| 19 | 审计日志 90 天在线 + 1 年冷存 | 路由决策、高风险调用、权限拒绝 | 9.3 |
| 20 | EvalResult.comparison | 与上一版本或黄金集的对比 | 7.2 |

---

## 15. 端到端时序图（RAG 模式）

```
Frontend          API              Router           RagStrategy         Executor          PolicyEnforcer      ToolRegistry      LLM
  │                │                 │                  │                   │                   │                  │             │
  │──POST /chat───▶│                 │                  │                   │                   │                  │             │
  │                │───route()──────▶│                  │                   │                   │                  │             │
  │                │                 │──resolve()──────▶│                   │                   │                  │             │
  │                │                 │◀──RoutingDecision│                   │                   │                  │             │
  │                │                 │                  │──init()──────────▶│                   │                  │             │
  │                │                 │                  │◀──RagState::Plan  │                   │                  │             │
  │                │                 │                  │                   │──step()──────────▶│                  │             │
  │                │                 │                  │                   │                   │                  │             │
  │                │                 │                  │                   │◀──StateEvent::Entered{state_id:"plan"}              │
  │                │                 │                  │                   │──skill("rag-plan")─▶│                  │             │
  │                │                 │                  │                   │◀──Skill body       │                  │             │
  │                │                 │                  │                   │                   │                  │             │
  │                │                 │                  │                   │─────────────────────────────────────────────────────▶│
  │                │                 │                  │                   │                   │                  │             │──plan()
  │                │                 │                  │                   │◀──────────────────────────────────────────────────────│
  │                │                 │                  │                   │                   │                  │             │
  │                │                 │                  │                   │──StateEvent::PlanDecision─────────────────────────────▶│
  │                │                 │                  │                   │                   │                  │             │
  │                │                 │                  │                   │──step()──────────▶│                  │             │
  │                │                 │                  │                   │◀──StateEvent::Entered{state_id:"execute"}             │
  │                │                 │                  │                   │                   │                  │             │
  │                │                 │                  │                   │──tool_call()──────▶│──check()────────▶│             │
  │                │                 │                  │                   │                   │◀──Allow          │             │
  │                │                 │                  │                   │                   │                  │──execute()  │
  │                │                 │                  │                   │                   │◀──ToolResult     │             │
  │                │                 │                  │                   │◀───────────────────│                  │             │
  │                │                 │                  │                   │                   │                  │             │
  │                │                 │                  │                   │──StateEvent::ToolResult───────────────────────────────▶│
  │                │                 │                  │                   │                   │                  │             │
  │                │                 │                  │                   │──step()──────────▶│                  │             │
  │                │                 │                  │                   │◀──StateEvent::Entered{state_id:"evaluate"}            │
  │                │                 │                  │                   │                   │                  │             │
  │                │                 │                  │                   │─────────────────────────────────────────────────────▶│
  │                │                 │                  │                   │◀──────────────────────────────────────────────────────│
  │                │                 │                  │                   │                   │                  │             │
  │                │                 │                  │                   │──StateEvent::Evaluation───────────────────────────────▶│
  │                │                 │                  │                   │                   │                  │             │
  │                │                 │                  │                   │──step()──────────▶│                  │             │
  │                │                 │                  │                   │◀──StateEvent::Entered{state_id:"answer"}              │
  │                │                 │                  │                   │                   │                  │             │
  │                │                 │                  │                   │─────────────────────────────────────────────────────▶│
  │                │                 │                  │                   │◀──────────────────────────────────────────────────────│
  │                │                 │                  │                   │                   │                  │             │
  │                │                 │                  │                   │──StateEvent::Terminal─────────────────────────────────▶│
  │                │                 │                  │                   │                   │                  │             │
  │                │◀──AgentRunResult│                  │                   │                   │                  │             │
  │◀──SSE stream───│                 │                  │                   │                   │                  │             │
```

**时序图说明：**
- 实线箭头 = 同步调用
- 虚线箭头 = 事件/回调
- `PolicyEnforcer` 在每次 tool_call 前拦截检查
- `StateEvent` 在每个状态转换时 emit（SSE 输出）
- LLM 调用在 Plan、Evaluate、Answer 三个阶段各发生一次
- 若 Evaluate 决定 Replan，时序图中 Plan→Execute→Evaluate 会循环
