# Progressive Disclosure ReAct Loop Framework

> **Agent 渐进式披露 ReAct 循环统一框架**
>
> 将 RAG / WebSearch / Chat 三个 agent 归纳为一个可配置的状态机框架，通过每轮渐进式加载不同的 Tools 和 Skills，减少 token 消耗和模型认知负荷，同时保持可扩展性。

---

## 1. Background

当前三个 agent（RagAgent、WebSearchAgent、ChatAgent）分别实现了各自的 ReAct 循环，但存在以下问题：

- **Prompt 冗余**：每轮调用都发送完整的 system prompt（如 RAG planner 的 215 行工具目录），即使第二轮已经不需要全部工具说明
- **架构重复**：三个 agent 的核心循环结构相似（plan → execute → evaluate → answer），但实现分散
- **扩展困难**：新增工具需要同时修改 planner prompt、执行器、evaluator 三处
- **无状态假设未利用**：LLM 每次调用都是无状态的，但历史 context 中的真实 tool call 可以替代 system prompt 中的示例

本框架的目标：
1. 统一三个 agent 的循环抽象
2. 引入渐进式披露（Progressive Disclosure）——每轮只加载当前阶段需要的 Tools 和 Skills
3. 明确 Tool（外部能力）与 Skill（LLM 提示词模块）的分层边界
4. 保持严格的 PLAN → EXECUTE 两步分离，支持未来扩展更多工具

---

## 2. Core Concepts

### 2.1 Phase（阶段）

统一状态机的五个阶段：

```
┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐
│   INIT   │────→│   PLAN   │────→│ EXECUTE  │────→│   EVAL   │────→│  ANSWER  │
│加载基础  │     │选择工具  │     │调用工具  │     │评估结果  │     │生成回答  │
│上下文    │     │          │     │          │     │          │     │          │
└──────────┘     └──────────┘     └──────────┘     └────┬─────┘     └──────────┘
                                                        │
                                                   继续 / 中止 / 调整
                                                        │
                                                   └────┘
                                              (循环回 PLAN，或结束到 ANSWER)
```

| Phase | 职责 | 输出 |
|-------|------|------|
| **INIT** | 加载基础上下文（角色定义、会话历史、用户偏好） | 无（仅组装） |
| **PLAN** | 加载工具目录，让 LLM 选择需要的工具 | `PlanOutput`（工具选择列表） |
| **EXECUTE** | 加载选中工具的详细 spec，输出实际调用 | `ToolCallSchema`（JSON 调用） |
| **EVAL** | 加载工具结果和 eval skill，评估是否满足 | `EvalSignal`（中止/继续/调整） |
| **ANSWER** | 加载回答 skill，生成带引用的最终答案 | `FinalAnswer` |

**EVAL → PLAN 的循环次数受 `LoopBudget` 约束**（免费 tier 3 轮，付费 tier 通过配置接口预留）。

### 2.2 Turn（回合）

每轮 LLM 调用是一次 Turn，包含：

```rust
struct Turn {
    phase: Phase,
    /// 本轮新加载的披露单元（Tool 或 Skill）
    disclosed_units: Vec<Box<dyn DisclosureUnit>>,
    /// 组装后的完整 messages（system + history + user prompt）
    assembled_messages: Vec<ChatMessage>,
    /// LLM 原始输出
    raw_output: String,
    /// 解析后的结构化输出
    parsed: TurnOutput,
}
```

### 2.3 DisclosureUnit（披露单元）

渐进式披露的基本加载单元，Tool 和 Skill 都实现此 trait：

```rust
trait DisclosureUnit: Send + Sync {
    fn id(&self) -> &str;
    /// 渲染成本轮 prompt 中实际插入的文本
    fn render(&self, ctx: &DisclosureContext) -> String;
}
```

---

## 3. Tool vs Skill 分层抽象

### 3.1 语义区分

| | **Tool** | **Skill** |
|---|---|---|
| 执行者 | 代码（Rust 程序） | LLM（通过生成文本） |
| 契约 | JSON Schema | Prompt 文本 |
| LLM 输出 | `{"tool":"dense_retrieval","args":{...}}` | 自由文本或结构化 JSON |
| 例子 | `dense_retrieval`、`brave_search` | `coverage_eval`、`rag_answer` |
| 权限点 | 免费/付费 tier 限制调用次数 | 通常无权限限制 |

**核心区别**：Tool 是"调用外部系统的能力"，Skill 是"指导 LLM 怎么思考/回答的 prompt 模块"。

### 3.2 Tool 定义

```rust
struct Tool {
    id: String,
    name: String,
    description: String,
    /// LLM 输出 tool call 时遵循的 JSON Schema
    json_schema: serde_json::Value,
}

impl DisclosureUnit for Tool {
    fn render(&self, _ctx: &DisclosureContext) -> String {
        format!(
            "### {}\n{}\nSchema:\n{}\n",
            self.name,
            self.description,
            serde_json::to_string_pretty(&self.json_schema).unwrap()
        )
    }
}
```

**注意**：Tool 的 `render` 只负责"让 LLM 知道怎么调用"，实际执行器在另一个模块（如 `crates/rag-core/src/runtime/tools/`）。

### 3.3 Skill 定义

```rust
struct Skill {
    id: String,
    /// 来自 prompts/*.txt 的 system prompt 内容
    system_prompt: String,
}

impl DisclosureUnit for Skill {
    fn render(&self, _ctx: &DisclosureContext) -> String {
        self.system_prompt.clone()
    }
}
```

Skill 没有执行器——它就是一段提示词，LLM 通过阅读它来生成符合要求的文本。

---

## 4. Progressive Disclosure Mechanism

### 4.1 核心原则

每轮 LLM 调用时，system prompt 由三部分组成：

1. **Base**（始终加载）：角色定义、输出格式约束、禁止性规则
2. **History**（自动携带）：前几轮的全部对话（assistant 输出 + tool results）
3. **Disclosure**（渐进加载）：本轮新附加的 Tool specs 或 Skill prompts

```
Round 1 (PLAN):
  system: [base + tool_catalog]
  user:   [query + context]
  → output: PlanOutput

Round 2 (EXECUTE):
  system: [base + selected_tool_specs]        ← 工具目录被替换为具体工具说明
  user:   [query]
  assistant: [Round 1 的 PlanOutput]           ← history 携带了计划
  user:   "Execute the selected tools"
  → output: ToolCallSchema

Round 3 (EVAL):
  system: [base + eval_skill]                 ← 工具说明被替换为评估 skill
  user:   [query]
  assistant: [Round 1 PlanOutput]
  user:   [tool results]
  assistant: [Round 2 ToolCallSchema]
  user:   [tool results from execution]
  → output: EvalSignal

Round N (ANSWER):
  system: [base + answer_skill + citation_rules]
  user:   [全部历史 + query]
  → output: FinalAnswer
```

### 4.2 为什么能省略

第二轮可以省略第一轮的部分内容，是因为：

| 可省略内容 | 替代来源 |
|-----------|---------|
| 未选中工具的详细说明 | 不需要了（PLAN 已经选定了工具） |
| 示例（Examples） | History 中的真实 tool call 成了更好的 few-shot |
| 工具选择指南 | History 中的 PlanOutput 已经示范了选择逻辑 |

**绝对不能省略的**：
- 输出 schema / 格式约束（每轮都需要）
- 禁止性规则（不要输出 markdown 等）
- 当前轮次需要的工具语义或 skill 指令

---

## 5. Three-Agent Configuration

### 5.1 RAG Agent

```rust
fn rag_config() -> AgentConfig {
    AgentConfig {
        phase_sequence: vec![
            Phase::Init,
            Phase::Plan,
            Phase::Execute,
            Phase::Eval,
            Phase::Answer,
        ],
        disclosure: vec![
            // INIT: 基础上下文
            PhaseConfig { units: vec![] },
            // PLAN: 加载全部 6 种检索工具
            PhaseConfig {
                units: vec![
                    Box::new(Tool::dense_retrieval()),
                    Box::new(Tool::lexical_retrieval()),
                    Box::new(Tool::graph_retrieval()),
                    Box::new(Tool::index_lookup()),
                    Box::new(Tool::doc_summary()),
                    Box::new(Tool::doc_metadata()),
                ],
            },
            // EXECUTE: 动态加载 PLAN 选中的工具
            PhaseConfig { units: vec![] /* 运行时填充 */ },
            // EVAL: 加载覆盖率评估 skill
            PhaseConfig {
                units: vec![Box::new(Skill::coverage_eval())],
            },
            // ANSWER: 加载回答 skill + 引用规则
            PhaseConfig {
                units: vec![
                    Box::new(Skill::rag_answer()),
                    Box::new(Skill::citation_rules()),
                ],
            },
        ],
        budget: LoopBudget::new(UserTier::Free, 3),
    }
}
```

**特点**：
- 有完整的 PLAN → EXECUTE 两步分离
- 工具目录最大（6 种），PLAN 阶段的渐进式披露收益最高
- EVAL 循环最多 3 次（免费 tier）

### 5.2 WebSearch Agent

```rust
fn websearch_config() -> AgentConfig {
    AgentConfig {
        phase_sequence: vec![
            Phase::Init,
            Phase::Execute,  // 跳过 PLAN（只有 1 种工具，不需要选择）
            Phase::Eval,
            Phase::Answer,
        ],
        disclosure: vec![
            PhaseConfig { units: vec![] },
            // EXECUTE: 直接加载 brave_search 工具
            PhaseConfig {
                units: vec![Box::new(Tool::brave_search())],
            },
            // EVAL: 搜索覆盖率评估
            PhaseConfig {
                units: vec![Box::new(Skill::search_coverage_eval())],
            },
            // ANSWER: Web 搜索回答 skill
            PhaseConfig {
                units: vec![
                    Box::new(Skill::websearch_answer()),
                    Box::new(Skill::citation_rules()),
                ],
            },
        ],
        budget: LoopBudget::new(UserTier::Free, 3),
    }
}
```

**特点**：
- 跳过 PLAN 阶段（工具单一，无需选择）
- 工具目录最小，渐进式披露收益有限
- 但 Skill 层面仍可优化（示例按需匹配）

### 5.3 Chat Agent

```rust
fn chat_config() -> AgentConfig {
    AgentConfig {
        phase_sequence: vec![
            Phase::Init,
            Phase::Plan,     // 判断是否需要工具
            Phase::Execute,
            Phase::Eval,
            Phase::Answer,
        ],
        disclosure: vec![
            PhaseConfig { units: vec![] },
            // PLAN: 加载可用工具目录（代码执行、图像生成等）
            PhaseConfig {
                units: vec![
                    Box::new(Tool::code_execution()),
                    Box::new(Tool::image_generation()),
                    // ... 其他 chat 可用工具
                ],
            },
            // EXECUTE: 动态加载选中的工具或 skill
            PhaseConfig { units: vec![] /* 运行时填充 */ },
            // EVAL: 工具结果评估（可选，如果用了工具）
            PhaseConfig {
                units: vec![Box::new(Skill::tool_result_eval())],
            },
            // ANSWER: 通用聊天回答 skill
            PhaseConfig {
                units: vec![Box::new(Skill::chat_answer())],
            },
        ],
        budget: LoopBudget::new(UserTier::Free, 3),
    }
}
```

**特点**：
- PLAN 阶段可能直接输出 "不需要工具"，进入 ANSWER
- 工具类型更偏向"外部服务"而非"检索"
- 预算控制与 RAG/WebSearch 一致

---

## 6. Runtime Flow

### 6.1 统一循环驱动器

```rust
struct ProgressiveLoop {
    config: AgentConfig,
    /// 已完成的 turns
    history: Vec<Turn>,
    /// 当前 EVAL 循环次数
    eval_rounds: usize,
}

impl ProgressiveLoop {
    async fn run(&mut self, request: AgentRequest) -> Result<String, AgentError> {
        let mut current_phase = self.config.phase_sequence[0];

        loop {
            // 1. 组装本轮 messages
            let messages = self.assemble_messages(current_phase, &request);

            // 2. 调用 LLM
            let response = self.llm.complete(&messages).await?;

            // 3. 解析输出
            let output = self.parse_output(current_phase, &response.content)?;

            // 4. 记录 turn
            self.history.push(Turn {
                phase: current_phase,
                disclosed_units: self.get_disclosed_units(current_phase),
                assembled_messages: messages,
                raw_output: response.content,
                parsed: output.clone(),
            });

            // 5. 状态转换
            match (current_phase, output) {
                // PLAN → EXECUTE
                (Phase::Plan, TurnOutput::Plan(plan)) => {
                    current_phase = Phase::Execute;
                    self.populate_execute_units(&plan)?;
                }

                // EXECUTE → EVAL
                (Phase::Execute, TurnOutput::ToolCall(calls)) => {
                    let results = self.execute_tools(calls).await?;
                    current_phase = Phase::Eval;
                    self.inject_tool_results(results);
                }

                // EVAL → 继续或回答
                (Phase::Eval, TurnOutput::EvalSignal(signal)) => {
                    match signal {
                        EvalSignal::Continue => {
                            self.eval_rounds += 1;
                            if self.eval_rounds >= self.config.budget.max_eval_rounds() {
                                current_phase = Phase::Answer;
                            } else {
                                current_phase = Phase::Plan;
                            }
                        }
                        EvalSignal::Synthesize => {
                            current_phase = Phase::Answer;
                        }
                        EvalSignal::Adjust(params) => {
                            self.eval_rounds += 1;
                            self.apply_adjustment(params)?;
                            current_phase = Phase::Plan;
                        }
                    }
                }

                // ANSWER → 结束
                (Phase::Answer, TurnOutput::Answer(text)) => {
                    return Ok(text);
                }

                // Chat 直接从 PLAN/EXECUTE 跳到 ANSWER
                (Phase::Plan, TurnOutput::Answer(text)) => return Ok(text),
                (Phase::Execute, TurnOutput::Answer(text)) => return Ok(text),

                _ => return Err(AgentError::InvalidPhaseTransition),
            }
        }
    }
}
```

### 6.2 Prompt 组装逻辑

```rust
fn assemble_messages(&self, phase: Phase, request: &AgentRequest) -> Vec<ChatMessage> {
    let mut messages = vec![ChatMessage::system(self.base_prompt())];

    // 1. 把历史 turns 全部塞进去（这是"记忆"）
    for turn in &self.history {
        messages.push(ChatMessage::user(turn.input_prompt()));
        messages.push(ChatMessage::assistant(turn.raw_output.clone()));
    }

    // 2. 组装本轮的披露内容
    let phase_config = &self.config.disclosure[phase.index()];
    let disclosure_text = phase_config.units
        .iter()
        .map(|u| u.render(&self.context))
        .collect::<Vec<_>>()
        .join("\n---\n");

    // 3. 构建本轮 user prompt
    let user_prompt = if disclosure_text.is_empty() {
        self.build_user_prompt(request)
    } else {
        format!(
            "{}\n\n[Disclosed Context]\n{}",
            self.build_user_prompt(request),
            disclosure_text
        )
    };

    messages.push(ChatMessage::user(user_prompt));
    messages
}
```

---

## 7. Extension Guide

### 7.1 添加新 Tool

以给 RAG 添加 `sql_retrieval` 为例：

**Step 1: 定义 Tool**

```rust
impl Tool {
    fn sql_retrieval() -> Self {
        Tool {
            id: "sql_retrieval",
            name: "SQL Retrieval",
            description: "Execute SQL queries against structured knowledge base...",
            json_schema: json!({
                "query": { "type": "string", "description": "SQL SELECT statement" },
                "tables": { "type": "array", "items": { "type": "string" } }
            }),
        }
    }
}
```

**Step 2: 注册到 Agent Config**

```rust
// 在 rag_config() 的 PLAN phase 中添加
PhaseConfig {
    units: vec![
        Box::new(Tool::dense_retrieval()),
        Box::new(Tool::lexical_retrieval()),
        Box::new(Tool::graph_retrieval()),
        Box::new(Tool::sql_retrieval()), // ← 新增
        // ...
    ],
}
```

**Step 3: 实现执行器**

```rust
// crates/rag-core/src/runtime/tools/sql_retrieval.rs
pub async fn execute(args: SqlArgs) -> Result<ToolResult, ToolError> {
    // 实际 SQL 查询逻辑
}
```

**Step 4: Done**

Planner 自动知道了 `sql_retrieval` 的存在，可以在 PLAN 阶段选中它，EXECUTE 阶段调用它。

### 7.2 添加新 Skill

以给 RAG 添加 `multi_step_reasoning` skill 为例：

**Step 1: 定义 Skill Prompt**

新建 `prompts/skills/multi_step_reasoning.txt`：

```
When the user's question requires multi-step reasoning:
1. Break down the question into dependent sub-questions
2. For each sub-question, identify which evidence chunk supports it
3. Chain the sub-answers in logical order
4. Mark uncertainty at each step where evidence is weak
```

**Step 2: 注册到 Agent Config**

```rust
PhaseConfig {
    units: vec![
        Box::new(Skill::multi_step_reasoning()), // ← 新增
        Box::new(Skill::rag_answer()),
        Box::new(Skill::citation_rules()),
    ],
}
```

**Step 3: Done**

不需要实现执行器——Skill 是纯提示词，LLM 通过阅读它来改变生成行为。

---

## 8. Mapping to Current Codebase

| 本框架概念 | 当前代码位置 | 状态 |
|-----------|-------------|------|
| `Phase::Plan` (RAG) | `rag_agent.rs` 中的 planner 调用 | 已实现，但 PLAN 和 EXECUTE 合并为一步 |
| `Phase::Eval` (RAG) | `evaluator.rs` 中的 `evaluate_rag_iteration` | 已实现 |
| `Phase::Answer` (RAG) | `rag_agent.rs` 中的 answer 合成 | 已实现 |
| `Phase::Execute` (WebSearch) | `web_search_agent.rs` 中的搜索执行 | 已实现 |
| `Phase::Eval` (WebSearch) | `evaluator.rs` 中的 `evaluate_search_iteration` | 已实现 |
| `DisclosureUnit` (Tool) | `rag-core/src/runtime/tools/` 中的工具定义 | 部分实现（有执行器，缺统一接口） |
| `DisclosureUnit` (Skill) | `prompts/*.txt` | 已存在，但未抽象为 `Skill` struct |
| `ProgressiveLoop` | 分散在 `rag_agent.rs`、`web_search_agent.rs`、`chat_agent.rs` | 重复实现，未统一 |
| `LoopBudget` | `react_loop.rs` 中的 `LoopBudget` | 已实现 |

### 8.1 迁移路径

1. **Phase 1（低风险）**：将 `prompts/*.txt` 抽象为 `Skill` struct，统一加载接口
2. **Phase 2（中风险）**：将工具定义抽象为 `Tool` struct，统一 `DisclosureUnit` trait
3. **Phase 3（中风险）**：重构 `rag_agent.rs`，将 planner 拆分为严格的 PLAN → EXECUTE 两步
4. **Phase 4（高风险）**：提取 `ProgressiveLoop`，统一三个 agent 的循环逻辑

---

## 9. Open Questions

1. **PLAN 阶段的工具选择准确率**：将 6 个工具全部暴露给 planner，是否会导致选择错误？是否需要引入轻量分类器预先筛选候选工具？
2. **Skill 的依赖关系**：某些 Skill 可能依赖其他 Skill（如 `rag_answer` 依赖 `citation_rules`），是否需要显式声明依赖？
3. **History 压缩**：当 EVAL 循环次数增加时，history context 会线性增长。是否需要引入 history 摘要机制？
4. **付费 tier 的 LoopBudget 接口**：当前预留了配置接口，但具体实现（如 `UserTier::Pro` 对应 5 轮还是 10 轮）需要产品决策。

---

## 10. SkillComponent — Declarative Atomic Tools

### 10.1 从硬编码到声明式

2026-05-18 重构后，原子工具（calculator、code_interpreter、weather_query、web_search）从硬编码 `match` 迁移到 **声明式 SkillComponent** 架构：

```
crates/app/src/agents/skills/
├── mod.rs              # SkillComponent trait + ExecutionContext
├── registry.rs         # SkillRegistry (HashMap dispatch)
├── eval.rs             # Routing eval suites
└── builtin/
    ├── mod.rs          # register_all()
    ├── calculator.rs   # SkillComponent + evaluate_calculator_expression
    ├── code_interpreter.rs
    ├── weather_query.rs
    └── web_search.rs
```

每个 SkillComponent 捆绑了三层 Perplexity 式上下文：

| Tier | 内容 | 来源 |
|------|------|------|
| **Index** | `name: description` (~50 words) | `SkillComponent::description()` |
| **Load** | 完整 `ToolSpec` + JSON Schema | `SkillComponent::spec()` |
| **Runtime** | Gotchas（负例） | `SkillComponent::gotchas()` |

### 10.2 新增一个原子工具（只需 2 步）

```rust
// 1. 在 builtin/my_tool.rs 实现 SkillComponent
pub struct MyToolSkill;

#[async_trait::async_trait]
impl SkillComponent for MyToolSkill {
    fn id(&self) -> &str { "my_tool" }
    fn description(&self) -> &str {
        "Load when the user asks to ..."
    }
    fn spec(&self) -> ToolSpec { /* JSON schema */ }
    fn render_hint(&self) -> &str { "json" }
    async fn execute(&self, args: &Value, ctx: &ExecutionContext<'_>) -> ToolResult {
        // 执行逻辑
    }
}

// 2. 在 builtin/mod.rs 注册
registry.register(Box::new(my_tool::MyToolSkill));
```

**无需修改**：`atomic_tools.rs`、`tool_catalog.rs`、planner prompt —— 全部自动通过 `SkillRegistry` 注入。

### 10.3 前端渲染解耦

前端通过 `render_hint` 映射表决定如何渲染 `ToolResult.data`，而非硬编码工具名：

```ts
const TOOL_RENDER_HINTS: Record<string, string> = {
  calculator: "calculator",
  code_interpreter: "code",
  weather_query: "weather",
};
```

新增工具若使用现有 render_hint（如 `"json"`），前端**零改动**。

## 11. References

- `docs/superpowers/specs/2026-05-12-architecture-baseline.md` — 当前架构基线
- `crates/app/src/agents/rag_agent.rs` — RAG Agent 实现
- `crates/app/src/agents/web_search_agent.rs` — WebSearch Agent 实现
- `crates/app/src/agents/evaluator.rs` — Evaluator 实现
- `crates/app/src/agents/react_loop.rs` — LoopBudget 实现
- `crates/app/src/agents/skills/` — SkillComponent 实现
- `prompts/` — 所有 Skill prompt 模板
