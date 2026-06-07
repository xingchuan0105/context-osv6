# ADR-0006-revised: 统一 AgentLoop — ReAct 循环 + 原生 Tool/Skill 分层

| 项目 | 内容 |
|---|---|
| 状态 | 已采纳（替代 ADR-0005 / 0005-revised / 0006） |
| 决策日期 | 2026-06-07 |
| 提出者 | AI 助手（与用户共同决策） |
| 影响范围 | `crates/app/src/agents/unified/`（新建）、`crates/app/src/agents/strategy/`（废弃）、`prompts/skills/`（扩展）、`modes/`（新建） |

---

## 1. 背景与动机

### 1.1 v5 现状与痛点

当前 `avrag-rs` v5 架构采用 **3 个独立 Strategy state machine + StrategyExecutor 驱动**：

- `ChatStrategy`：Plan → ExecuteAtomic → Answer（单轮或简单循环）
- `RagStrategy`：Plan → ExecuteRetrieve → [循环] → Answer（多轮检索循环）
- `SearchStrategy`：Decompose → ParallelSearch → Aggregate → Answer（搜索专用流程）

**痛点**：
1. **三个循环骨架重复**：每个 Strategy 各自实现 ReAct 风格的"思考→行动→观察"逻辑，修一个循环 bug 要改 3 处；
2. **状态机过度工程化**：`State` / `StepOutcome` / `StateKind` 枚举 + 状态转换边界，对"LLM 自己决定何时停"的 ReAct 范式是累赘；
3. **工具碎片化**：14 个功能性 tool schema（dense_retrieval、lexical_retrieval、rerank、chunk_fetch、calculator、weather_query...）全量暴露在 LLM context 中，每轮 ~2000 token，认知负担大；
4. **调优需要改 Rust 代码**：AI 行为微调（如"RAG 下什么时候该用 graph search"）需要改 Strategy 内部逻辑，产品/运营无法独立迭代。

### 1.2 核心目的

用户明确提出的核心目的：

> "三个模式共享一套基于 ReAct 范式的 AgentLoop。底层功能（codegen、websearch）作为原生 tool 加载，其他功能作为 Skill，由 LLM 读 Skill 描述后完成调用。同时按模式隔离原生 tool：RAG 下只给 dense，Search 下只给 web_search，Chat 下不给任何原生 tool。在 ReAct 范式下采用渐进式披露方式给 agent 传递 Skill，实现 token 效率和上下文简洁。"

### 1.3 关键洞察

基于 [pi.dev ReAct 最佳实践](https://pi.dev)：

- **原生 Tool** 用于"可验证执行"——严格参数、确定性结果、外部副作用、高信号 observation；
- **Skill** 用于"行为引导"——教模型何时/为什么/怎么选/怎么组织步骤；
- **不**把每个底层 API 端点暴露为原生 tool，而是收敛为少量高层语义清晰的原生 tool，复杂编排通过 Skill 引导的 codegen/SDK 完成；
- Skill 最有价值的部分是"选择标准"（什么时候必须查证、什么时候允许并行），而不是"伪造工具接口"。

### 1.4 非目标

- 不引入 Rig / LangGraph / DSPy 等外部框架；
- 不替换 `avrag-llm`（已 production-tested）；
- 不改前端 SSE 事件协议主体（复用 `AgentEvent` 变体，可能扩展）；
- 不改数据库 schema；
- 不改动 `avrag-auth` / `avrag-storage-pg` / `avrag-retrieval-data-plane` 等基础设施。

---

## 2. 决策

### 2.1 核心决策

采用 **"统一 ReActLoop + 原生 Tool/Skill 分层 + 按模式隔离 + YAML 配置驱动"** 架构：

| 维度 | 决策 | 理由 |
|---|---|---|
| **循环** | **统一 ReAct 循环 + 独立合成阶段 (Synthesis Phase)** | ReAct 循环负责内部检索、规划和 Codegen 执行，不进行用户端流式输出；一旦检索完成或达到 Limit，进入 Synthesis Phase，用 LLM 统一流式回答用户。 |
| **原生 Tool 数量** | **每模式 0-1 个**（RAG: dense, Search: web_search, Chat: 空） | 只保留"稳定、高频、可严格 schema 化"的兜底能力 |
| **功能性能力** | **SDK 层（codegen）** | 新增/删除功能只改 SDK + Skill 描述，不改原生 tool schema |
| **行为引导** | **Skill 累积式渐进披露** | 采用配置驱动在各轮次中披露 Skill。已被加载的 Skill 将**在后续迭代间累积**，防止模型遗忘格式与约束。 |
| **配置** | **YAML 驱动 + 规则匹配** | `disclosure.rounds` 结合 rule-based/regex 进行高速低延迟的 Skill 动态披露，避免语义向量计算或额外 LLM 推断。 |
| **模式隔离** | **前端开关不可违反** | `agent_type` 决定进入哪个 mode config，原生 tool 严格按 mode 加载 |

### 2.2 三个 Mode 的分工

| 模式 | 原生 Tool（LLM tool_call 可见） | Skill 重点 | 兜底行为 |
|---|---|---|---|
| **RAG** | `dense_retrieval` | codegen 引导（SDK 检索）、检索策略选择、chunk 引用格式、memory 管理 | codegen/SKD 全失败时，**系统自动**用原 query 调 dense 检索 |
| **Search** | `web_search` | 搜索策略、结果交叉验证、URL 引用格式 | web_search 直接执行 |
| **Chat** | **空（0 个）** | 对话策略、指代消解、语气引导、何时查询/计算 | **无兜底**——LLM 直接生成答案，或通过 codegen 调 SDK |

### 2.3 关键边界决策

| 决策项 | 结论 | 理由 |
|---|---|---|
| Chat 是否有原生 tool | **无** | Chat 95% 场景纯对话；需要计算/查询时走 codegen/SDK |
| dense 是否只在 RAG 模式暴露 | **是** | Chat/Search 物理上看不到 dense_retrieval schema |
| dense 是否也在 SDK 中 | **是** | SDK 有 `client.dense_search(query)`，LLM 可通过 codegen 调用；原生 tool 是兜底 |
| codegen 是否覆盖所有工具 | **是** | 包括 dense、web_search（Search 模式 SDK 也可调）、calculator 等 |
| 新增功能性能力 | **SDK + Skill** | 不改原生 tool schema，只加 SDK 方法 + 更新 Skill 描述 |
| 渐进披露控制 | **YAML 配置 + 累积加载** | `disclosure.rounds` 定义每轮加载哪些 Skill，已披露 Skill 自动累积保留在系统提示词中 |
| 流式输出与合成 | **ReAct 内部不流式，答案生成走独立的 Synthesis Phase** | 避免中间思考或 `<code>` 块等内部格式在流式中破坏前端 UI 体验，同时保障 100% 的流式答案输出 |
| 取消 | **每次 LLM call 边界检查** | 避免沙箱卡死 |
| 沙箱错误预算 | **连续 2 次沙箱报错触发降级/兜底** | 防止 LLM 陷入编写 Bug 代码的无限自我纠错循环中，浪费 Token |
| Free Tier 预算 | **最低 budget >= 2 轮** | 开启了 codegen/action 技能的模式在 Free Tier 下统一限制最低 budget >= 2 轮（RAG/Search/Chat 均为最低 2 轮），允许 Free Tier 用户正常执行单次 Codegen 动作并获取合成的最终回答。 |

### 2.4 已否决的替代方案

| 方案 | 否决理由 |
|---|---|
| 保留 v5 状态机（ADR-0005-revised） | 三个循环重复、状态机对 ReAct 是累赘、微调状态机不够优雅 |
| ADR-0006 原方案（1+1 工具） | 把 codegen 当成原生 tool、把 12 个功能性能力完全废弃（用户明确要保留为 Skill/SDK） |
| 14 个 tool 全量暴露 | token 浪费、认知负担大、与"渐进披露"冲突 |
| 引入 Rig / LangGraph | 增加外部依赖；当前需求单一 ReAct 循环足够 |
| Chat 保留 compute 原生 tool | 用户明确否决；Chat 无原生 tool |

---

## 3. 整体架构

```
Frontend
  POST /chat { agent_type: "chat"|"rag"|"search", ... }
        │
        ▼
┌─────────────────────────────────────────────────────────────┐
│  UnifiedAgent::run (crates/app/src/agents/unified/)         │
│  1. 按 agent_type 加载 mode config（YAML）                   │
│  2. 调用 ReActLoop::run(mode, request, sink)                │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│  ReActLoop（统一 ReAct 循环）                                │
│                                                              │
│  state = {                                                   │
│    mode: ModeConfig,            # YAML 加载                  │
│    messages: Vec<ChatMessage>,  # 累积对话                   │
│    iteration: u8,               # 当前轮次                   │
│    disclosed_skills: Vec<SkillId>,  # 本轮已披露 skills      │
│  }                                                           │
│                                                              │
│  loop:                                                       │
│    1. 渐进披露 → 选择本轮要加载的 skills                      │
│    2. 构造 system prompt = 基础 prompt + 本轮 skills 描述     │
│    3. 调 LLM (complete_with_tools，只带该模式的原生 tool)      │
│    4. LLM 输出:                                               │
│       a) tool_call(dense_retrieval)    — RAG 原生            │
│       b) tool_call(web_search)         — Search 原生         │
│       c) content 中含 <code>...</code>  — Skill 引导的 codegen │
│       d) 直接 content                   — 最终答案           │
│    5. 执行 → observation 回环 → 继续或终止                    │
└────────────────────┬────────────────────────────────────────┘
                     │
        ┌────────────┼────────────┐
        ▼            ▼            ▼
┌──────────────┐ ┌──────────┐ ┌────────────────┐
│ 原生 Tool 层  │ │ Skill 层 │ │ 兜底路径        │
│              │ │          │ │                │
│ • dense      │ │ • codegen│ │ 系统自动       │
│   _retrieval │ │   _guide │ │ dense 检索     │
│   (RAG)      │ │ • retriev│ │ (RAG only)     │
│              │ │   al_stra│ │                │
│ • web_search │ │   tegy   │ │                │
│   (Search)   │ │ • citation│ │               │
│              │ │   _format│ │                │
│              │ │ • memory_│ │                │
│              │ │   mgmt   │ │                │
│              │ │ • search_│ │                │
│              │ │   strategy│ │               │
│              │ │ • chat_  │ │                │
│              │ │   strategy│ │               │
└──────────────┘ └──────────┘ └────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│  SDK 层（Python 沙箱内可用）                                  │
│  client.dense_search(query, top_k, method)                  │
│  client.lexical_search(...)                                 │
│  client.graph_search(...)                                   │
│  client.rerank(...)                                         │
│  client.chunk_fetch(chunk_id)                               │
│  client.web_search(query, vertical)                         │
│  client.calculate(expression)                               │
│  client.recall(tags, limit)                                 │
│  client.remember(operations)                                │
│  ...                                                        │
└─────────────────────────────────────────────────────────────┘
```

---

## 4. ReActLoop 设计

### 4.1 核心接口

```rust
pub struct ReActLoop {
    llm: Arc<LlmClient>,
    sandbox: Arc<Sandbox>,
    web_search: Arc<dyn SearchProvider>,
    skill_registry: Arc<CapabilityRegistry>,
}

/// 从 YAML 加载的模式配置
pub struct ModeConfig {
    pub id: String,                    // "chat" | "rag" | "search"
    pub system_prompt_base: String,    // 基础 system prompt（SKILL.md 路径或内联）
    pub native_tools: Vec<ToolSpec>,   // 该模式暴露的原生 tool schema（0-1 个）
    pub skill_catalog: Vec<SkillId>,   // 该模式可用的全部 skills
    pub disclosure: DisclosureConfig,  // 渐进披露策略
    pub budget: BudgetConfig,
    pub auto_fallback: Option<AutoFallbackConfig>, // 兜底配置（仅 RAG）
}

pub struct DisclosureConfig {
    pub rounds: Vec<DisclosureRound>,
}

pub struct DisclosureRound {
    pub round_idx: u8,                 // 第几轮 ReAct 迭代
    pub load: DisclosureLoad,          // 本轮加载什么
}

pub enum DisclosureLoad {
    Index,                             // 只加载技能索引（id + 一句话描述）
    Skills(Vec<SkillId>),              // 加载指定 skills 的完整描述
    Auto,                              // 根据 LLM 意图自动匹配加载
}

pub struct AgentRequest {
    pub mode_id: String,
    pub user_query: String,
    pub session_id: Option<String>,
    pub cancellation: CancellationToken,
}
```

### 4.2 ReAct 循环伪代码

```rust
impl ReActLoop {
    pub async fn run(
        &self,
        mode: &ModeConfig,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        let mut messages: Vec<ChatMessage> = vec![];
        let mut iteration: u8 = 0;
        let mut consecutive_errors: u8 = 0;
        let mut consecutive_sandbox_errors: u8 = 0;
        let mut disclosed_skills: Vec<SkillMetadata> = vec![]; // 累积披露的 Skills

        // Phase 1: ReAct 内部决策与工具执行循环（内部进行，不流式输出给用户）
        let react_result = loop {
            // ── 终止条件 ──
            if iteration >= mode.budget.max_iterations {
                break Err("budget_exhausted");
            }
            if consecutive_errors >= MAX_RETRY_PER_LOOP {
                break Err("llm_cannot_recover");
            }
            if request.cancellation.is_cancelled() {
                break Err("cancelled");
            }

            // ── 渐进披露（累积式）：选择本轮新增 Skills ──
            let new_skills = self.progressive_disclose(
                &mode.disclosure,
                &mode.skill_catalog,
                &messages,
                iteration,
            );
            // 累积保存已披露的 Skills，防止 LLM 在后续 iteration 遗忘约束
            for skill in new_skills {
                if !disclosed_skills.iter().any(|s| s.id == skill.id) {
                    disclosed_skills.push(skill);
                }
            }

            // ── 构造 system prompt ──
            let system_prompt = self.build_system_prompt(
                &mode.system_prompt_base,
                &disclosed_skills,
            );
            let mut round_messages = vec![ChatMessage::system(system_prompt)];
            round_messages.extend(messages.clone());

            // ── 调 LLM（只带该模式的原生 tool，要求输出思考和工具调用，内部执行）──
            let response = match self.llm
                .complete_with_tools(&round_messages, &mode.native_tools, Some(0.7))
                .await
            {
                Ok(r) => {
                    consecutive_errors = 0;
                    r
                }
                Err(e) => {
                    consecutive_errors += 1;
                    tracing::warn!("llm call failed: {}", e);
                    continue;
                }
            };

            // ── 处理 LLM 输出 ──
            match response {
                // 情况 A：原生 tool_call
                HasNativeToolCall(call) => {
                    let result = self.execute_native_tool(&call).await;
                    messages.push(build_assistant_message_with_tool_call(&call));
                    messages.push(build_tool_message(&call, &result));
                    sink.emit(AgentEvent::ToolResult { ... }).await;
                    iteration += 1;
                    consecutive_sandbox_errors = 0;
                    continue;
                }

                // 情况 B：content 中含 <code> 标签（Skill 引导的 codegen）
                HasCodeBlock(code) => {
                    let result = self.sandbox.execute(&code).await;
                    messages.push(ChatMessage::assistant(format!(
                        "<code>{}</code>", code
                    )));
                    
                    let is_success = result.is_ok();
                    messages.push(ChatMessage::tool(
                        format!("沙箱执行结果:\n{}", result)
                    ));
                    sink.emit(AgentEvent::ToolResult { tool: "code_gen", ... }).await;
                    iteration += 1;
                    
                    if !is_success {
                        consecutive_sandbox_errors += 1;
                        if consecutive_sandbox_errors >= 2 {
                            // 连续沙箱报错 2 次，提前退出 ReAct 循环以触发兜底与合成
                            break Err("consecutive_sandbox_errors");
                        }
                    } else {
                        consecutive_sandbox_errors = 0;
                    }
                    continue;
                }

                // 情况 C：直接 content（LLM 认为已收集完信息，主动完成并进入合成阶段）
                DirectContent(text) => {
                    break Ok(text);
                }

                // 情况 D：无有效输出（不应发生，但兜底）
                Empty => {
                    break Err("no_valid_output");
                }
            }
        };

        // Phase 2: Synthesis Phase（合成与流式输出阶段）
        // 无论是主动完成还是异常中断/超时，我们都在这里进行流式输出，确保用户体验
        match react_result {
            Ok(answer) => {
                // LLM 已经主动产出了答案或关键段落，直接流式发送给前端
                self.stream_response_to_client(&answer, sink).await;
                return Ok(AgentRunResult { answer, ... });
            }
            Err(reason) => {
                // 异常或超时路径：执行系统自动兜底（如 RAG dense 检索）并合成回答
                if mode.id == "rag" && (reason == "budget_exhausted" || reason == "consecutive_sandbox_errors" || reason == "no_valid_output") {
                    if let Some(fallback) = &mode.auto_fallback {
                        let dense_result = self.auto_dense_retrieval(
                            &request.user_query,
                            fallback,
                        ).await;
                        messages.push(ChatMessage::system(
                            format!("自动兜底检索结果:\n{}", dense_result)
                        ));
                    }
                }
                
                // 强制进行一次最终 Synthesis LLM 调用（流式），不带任何工具，基于所有收集的上下文合成
                let synthesis_prompt = self.build_synthesis_system_prompt(&mode.system_prompt_base, &disclosed_skills);
                let mut synthesis_messages = vec![ChatMessage::system(synthesis_prompt)];
                synthesis_messages.extend(messages.clone());
                
                let final_answer = self.llm
                    .complete_stream(&synthesis_messages, sink) // 流式调用并直接写入 SSE sink
                    .await?;
                    
                sink.emit(AgentEvent::Done { final_message: Some(final_answer.clone()), ... }).await;
                return Ok(AgentRunResult { answer: final_answer, ... });
            }
        }
    }
}
```

### 4.3 dense 的三重路径（RAG 模式详解）

```
┌───────────────────────────────────────────────────────────────┐
│  RAG 模式下，dense_retrieval 的三条使用路径                    │
├───────────────────────────────────────────────────────────────┤
│                                                               │
│  路径 A：Skill 引导 of codegen（主路径，推荐）                 │
│  ─────────────────────────────────────────                    │
│  Skill 描述教 LLM："当你需要复杂检索时，写 Python 调 SDK"      │
│  LLM 输出：                                                    │
│    <code language="python">                                   │
│    chunks = await client.dense_search(                        │
│        query=user_query, top_k=10, method="auto"              │
│    )                                                          │
│    </code>                                                    │
│  系统：提取 → 送沙箱 → SDK 执行 → 返回 chunk 列表              │
│                                                               │
│  路径 B：原生 tool_call（简单场景，LLM 自选）                  │
│  ─────────────────────────────────────────                    │
│  LLM 认为简单，直接输出：                                      │
│    tool_call(dense_retrieval, {query: "...", top_k: 10})      │
│  系统：严格参数校验 → 直接执行 Rust 函数 → 返回 chunk 列表     │
│                                                               │
│  路径 C：系统自动兜底与合成阶段（codegen 失败时）              │
│  ─────────────────────────────────────────                    │
│  触发条件：                                                   │
│    - LLM 连续 N 轮未产生有效 action                            │
│    - 沙箱执行连续报错（>= 2 次）                               │
│    - iteration 接近 budget 上限但仍无答案                      │
│  系统行为：                                                   │
│    退出 ReAct 循环。自动用 request.user_query 调 dense        │
│    将结果作为 Observation 注入，强制进入 Synthesis 阶段。      │
│    LLM 在 Synthesis 阶段以 100% 流式回答用户并返回。           │
│  目的：确保 RAG 始终能返回基础内容且保证最终答案 100% 流式输出 │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

**为什么保留路径 B**：
- 简单场景（"直接用原 query 检索"）不需要启动沙箱，+100-300ms 延迟；
- 原生 tool 有严格 JSON schema，参数类型安全；
- 兜底路径 C 复用同一份 Rust 实现，不重复开发。

---

## 5. Skill 设计与渐进披露

### 5.1 Skill 的本质

根据 [pi.dev 最佳实践](https://pi.dev)：

> "Skill 最有价值的部分不是'伪造一个工具接口'，而是把选择标准写清楚：什么时候必须查证、什么时候先读文件、什么时候允许并行、什么时候必须先确认用户意图。"

Skill **不**直接承接外部副作用，而是：
- 教 LLM "怎么思考"（策略选择）
- 提供 few-shot 示例（XML 结构化）
- 约束输出格式（引用格式、语气等）

### 5.2 Skill 内容格式

采用 XML 结构化标签，降低歧义：

```markdown
<!-- prompts/skills/rag-codegen-guide/SKILL.md -->

<context>
你是 Context OS 的 RAG 助手。你基于用户上传的文档回答问题。
当用户的问题涉及文档内容时，你应该优先通过检索获取证据。
</context>

<instructions>
1. 分析用户意图，确定需要检索的方向
2. 如果需要复杂检索（多条件、跨文档、聚合分析），在回复中输出 <code language="python"> 标签包裹的 Python 代码
3. 简单检索可以直接调用 dense_retrieval 原生工具
4. 每次检索后评估证据充分性：充分则回答，不充分则调整查询继续检索
</instructions>

<examples>
<example>
<user>我的合同里有哪些付款条款？</user>
<reasoning>用户问的是文档中的特定条款，需要 dense search 找"付款"相关内容。</reasoning>
<action>
<code language="python">
chunks = await client.dense_search(
    query="付款条款 支付条件",
    top_k=10,
    method="auto"
)
</code>
</action>
</example>

<example>
<user>总结一下我上传的所有 PDF 的核心内容</user>
<reasoning>需要跨文档聚合，先取文档摘要再综合。</reasoning>
<action>
<code language="python">
docs = await client.get_doc_summary(doc_ids=scope, level="doc")
summaries = [d.summary for d in docs]
# 然后基于 summaries 综合回答
</code>
</action>
</example>
</examples>

<constraints>
- 答案必须带引用：[1] 引用内容
- 不确定时告诉用户，不要编造
- 每次只调 1-3 个工具/方法
</constraints>
```

### 5.3 渐进披露配置

```yaml
# modes/rag.yaml
mode: rag
system_prompt_base: prompts/skills/rag-system/SKILL.md
native_tools: [dense_retrieval]
skill_catalog:
  - rag-codegen-guide
  - retrieval-strategy
  - citation-format
  - memory-management
  - doc-summary-guide

disclosure:
  rounds:
    - round_idx: 0
      load: Index                    # 第一轮：只告诉 LLM 有哪些技能
    - round_idx: 1
      load: Skills
        - rag-codegen-guide
        - retrieval-strategy        # 第二轮：加载核心技能详情
    - round_idx: 2
      load: Auto                   # 第三轮及以后：根据上下文自动匹配

budget:
  max_iterations: 4

auto_fallback:
  enabled: true
  tool_id: dense_retrieval
  top_k: 10
```

```yaml
# modes/search.yaml
mode: search
system_prompt_base: prompts/skills/search-system/SKILL.md
native_tools: [web_search]
skill_catalog:
  - search-strategy
  - result-validation
  - url-citation-format

disclosure:
  rounds:
    - round_idx: 0
      load: Index
    - round_idx: 1
      load:
        - search-strategy
        - result-validation

budget:
  max_iterations: 3
```

```yaml
# modes/chat.yaml
mode: chat
system_prompt_base: prompts/skills/chat-system/SKILL.md
native_tools: []                    # 空数组：Chat 模式无原生 tool
skill_catalog:
  - chat-strategy
  - anaphora-resolution
  - tone-guidance

disclosure:
  rounds:
    - round_idx: 0
      load:
        - chat-strategy
        - anaphora-resolution

budget:
  max_iterations: 2
```

### 5.4 渐进披露的实现

```rust
fn progressive_disclose(
    &self,
    config: &DisclosureConfig,
    catalog: &[SkillId],
    conversation: &[ChatMessage],
    iteration: u8,
) -> Vec<SkillMetadata> {
    // 找到对应当前轮次的披露规则
    let round_config = config.rounds
        .iter()
        .find(|r| r.round_idx == iteration)
        .or_else(|| config.rounds.last())  // 超出配置轮次，用最后一轮规则
        .expect("disclosure config must not be empty");

    match &round_config.load {
        DisclosureLoad::Index => {
            // 只返回索引：id + 一句话描述
            catalog.iter()
                .map(|id| self.skill_registry.skill_index(id))
                .collect()
        }
        DisclosureLoad::Skills(ids) => {
            // 返回指定 skills 的完整内容
            ids.iter()
                .map(|id| self.skill_registry.skill_full(id))
                .collect()
        }
        DisclosureLoad::Auto => {
            // 规则匹配 (Rule-based / Regex matching)：快速判断上文关键字以过滤 Skills，避免 LLM/Vector 延迟
            let last_msg_content = conversation.last()
                .map(|m| m.content.to_lowercase())
                .unwrap_or_default();
                
            catalog.iter()
                .filter(|id| {
                    match id.as_str() {
                        "retrieval-strategy" => last_msg_content.contains("search") || last_msg_content.contains("retrieve"),
                        "memory-management" => last_msg_content.contains("remember") || last_msg_content.contains("recall"),
                        "doc-summary-guide" => last_msg_content.contains("summary") || last_msg_content.contains("summarize"),
                        _ => false
                    }
                })
                .map(|id| self.skill_registry.skill_full(id))
                .collect()
        }
    }
}
```

---

## 6. 原生 Tool 设计

### 6.1 dense_retrieval（RAG 模式）

```json
{
  "name": "dense_retrieval",
  "description": "基于语义相似度检索文档片段。当需要查找与用户问题语义相关的文档内容时使用。",
  "parameters": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "检索查询，建议包含用户问题的核心概念"
      },
      "top_k": {
        "type": "integer",
        "description": "返回最相关的 N 个片段",
        "default": 10,
        "minimum": 1,
        "maximum": 50
      }
    },
    "required": ["query"]
  }
}
```

**返回值**（高信号、简洁）：
```json
{
  "chunks": [
    {
      "id": "chunk_123",
      "score": 0.92,
      "content_preview": "付款条款：买方应在收到发票后 30 天内...",
      "doc_name": "合同_v1.pdf"
    }
  ],
  "total_found": 42,
  "retrieval_method": "dense"
}
```

### 6.2 web_search（Search 模式）

```json
{
  "name": "web_search",
  "description": "搜索互联网获取实时信息。用于查找最新新闻、事实、数据等。",
  "parameters": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "搜索查询"
      },
      "vertical": {
        "type": "string",
        "enum": ["web", "news"],
        "default": "web"
      }
    },
    "required": ["query"]
  }
}
```

### 6.3 Chat 模式无原生 Tool

Chat 的 `ModeConfig.native_tools = []`，LLM 调用 `complete_with_tools(messages, &[], ...)`。
LLM 看不到任何 tool schema，纯靠 system prompt + skill 引导输出 `<code>` 或直接回答。

---

## 7. SDK 设计（Python 沙箱内）

SDK 封装所有功能性能力，供 LLM 在 `<code>` 中调用：

```python
class Client:
    """SDK for LLM to call from inside code_gen sandbox."""

    async def dense_search(self, query: str, top_k: int = 10, method: str = "auto") -> list[Chunk]:
        """Semantic search in workspace documents."""

    async def lexical_search(self, query: str, top_k: int = 10) -> list[Chunk]:
        """Exact keyword search."""

    async def graph_search(self, query: str, depth: int = 2) -> list[Chunk]:
        """Entity relationship search."""

    async def rerank(self, query: str, chunks: list[Chunk], top_n: int = 5) -> list[Chunk]:
        """Rerank chunks by relevance."""

    async def chunk_fetch(self, chunk_id: str) -> Chunk:
        """Fetch full content of a chunk."""

    async def doc_summary(self, doc_ids: list[str], level: str = "doc") -> list[DocSummary]:
        """Get document summaries."""

    async def recall(self, tags: list[str] | None = None, limit: int = 20) -> list[Message]:
        """Load previous messages."""

    async def remember(self, operations: list[dict]) -> dict:
        """Tag messages for future recall."""

    async def web_search(self, query: str, vertical: str = "web") -> list[WebResult]:
        """Search the web. (Alternative to native web_search tool.)"""

    async def calculate(self, expression: str) -> float:
        """Evaluate math expression."""
```

**实现**：复用现有 `crates/rag-core/src/runtime/tools/*.rs`，包装为 async Python 方法。

---

## 8. 与 v5 设施的关系

### 8.1 废弃的设施

| 设施 | 原因 |
|---|---|
| `StrategyExecutor` | 状态机驱动器，与 ReAct 循环互斥 |
| `State` / `StepOutcome` / `StateKind` | 状态机抽象，ReAct 不需要 |
| `ChatStrategy` / `RagStrategy` / `SearchStrategy` | 合并为统一 ReActLoop，差异下沉为 ModeConfig |
| `strategy/executor.rs` | 同上 |
| `strategy/mod.rs` 中的状态机 trait | 同上 |

### 8.2 复用的设施

| 设施 | 复用方式 |
|---|---|
| `CapabilityRegistry` | **核心复用**：管理 Skill 元数据（id, description, applicable_strategies），为渐进披露提供查询 |
| `ToolMetadata` / `SkillMetadata` | 原生 tool 和 skill 的描述来源 |
| `AgentEvent` / `events.rs` | ReAct 循环发射事件（Activity, ToolResult, Done, Error 等） |
| `TraceSpan` / `AgentTrace` | 可观测性，每轮 ReAct 迭代生成 span |
| `LlmClient::complete_with_tools` | 底层 LLM 调用 |
| `code_gen_query` 沙箱 | 执行层，执行 LLM 输出的 Python 代码 |
| `web_search` 实现 | 原生 tool 实现 |
| `dense_retrieval` 实现 | 原生 tool 实现 + SDK 方法底层 |
| `replay.rs` | **适配**：从 `state_history` 改为 `ReActIteration` 记录 |
| `eval_framework.rs` | **适配**：断点从 `StateKind` 改为 `iteration_idx` + `disclosed_skills` |
| `audit.rs` | **适配**：从 `state_id` 改为 `iteration_idx` + `action_type` |

### 8.3 replay / eval / audit 的适配策略

**replay.rs**：
- 旧：`Vec<StateRecord>`（state_id, state_kind, entered_at, completed_at）
- 新：`Vec<ReActIterationRecord>`（iteration, disclosed_skills, llm_call, action_type, observation, elapsed_ms）
- 保留 `ReplaySnapshot` 外壳，内部结构更新

**eval_framework.rs**：
- 旧：断点绑在 `StateKind::Evaluate` 或 `state_id == "answer"`
- 新：断点绑在 `iteration_idx`（如 "第 2 轮迭代后"）或 `action_type`（如 "code_gen 执行后"）
- `EvalCase::result` 仍用 `AgentRunResult`，字段兼容

**audit.rs**：
- 旧：按 `state_id` 上报（如 "plan", "execute_retrieve"）
- 新：按 `iteration_idx` + `action_type`（如 "iteration=2, action=code_gen"）
- 审计语义不变：记录"AI 在什么时候做了什么"

---

## 9. 迁移计划

### 阶段 1：ReActLoop 骨架 + Chat 模式（2 周）

- [ ] 新建 `crates/app/src/agents/loop/mod.rs`（ReActLoop 主体）
- [ ] 新建 `modes/chat.yaml` + `prompts/skills/chat-system/SKILL.md`
- [ ] 实现渐进披露（Index/Skills/Auto）
- [ ] 实现 `<code>` 标签解析 + 沙箱执行
- [ ] 集成测试：`test_chat_pure_dialogue`、`test_chat_codegen_calculate`
- [ ] 旧 `ChatStrategy` 标记 `#[deprecated]`，保留并行

**验证**：`cargo test -p app --test loop_chat` 通过。

### 阶段 2：RAG 模式（2 周）

- [ ] 新建 `modes/rag.yaml` + `prompts/skills/rag-codegen-guide/SKILL.md`
- [ ] 实现 `dense_retrieval` 原生 tool（参数严格 schema）
- [ ] SDK 封装：`client.dense_search`、`client.lexical_search`、`client.chunk_fetch`...
- [ ] 实现 dense 兜底路径（codegen 失败时自动调 dense）
- [ ] 集成测试：
  - `test_rag_dense_direct_tool_call`（路径 B）
  - `test_rag_codegen_search`（路径 A）
  - `test_rag_auto_fallback`（路径 C）
  - `test_rag_progressive_disclosure`（渐进披露）
- [ ] 旧 `RagStrategy` 标记 `#[deprecated]`

**验证**：现有 RAG 集成测试不回归 + 新测试通过。

### 阶段 3：Search 模式（1 周）

- [ ] 新建 `modes/search.yaml` + `prompts/skills/search-system/SKILL.md`
- [ ] 复用 `web_search` 原生 tool
- [ ] SDK 封装：`client.web_search`
- [ ] 集成测试
- [ ] 旧 `SearchStrategy` 标记 `#[deprecated]`

**验证**：现有 Search 集成测试通过。

### 阶段 4：适配 replay / eval / audit + 删除旧代码（2-3 周）

- [ ] `replay.rs`：适配 `ReActIterationRecord`
- [ ] `eval_framework.rs`：断点迁移到 `iteration_idx`
- [ ] `audit.rs`：上报字段更新
- [ ] 删除 `crates/app/src/agents/strategy/` 全部文件
- [ ] 更新 E2E 测试
- [ ] 更新文档

**验证**：`cargo test -p app --lib` 全绿 + E2E 通过 + `cargo clippy` 干净。

**总估算**：7-8 周。

---

## 10. 测试策略

### 10.1 单元测试

- `progressive_disclose`：验证每轮加载的 skills 符合 YAML 配置
- `build_system_prompt`：验证 XML 标签正确拼接
- `parse_code_block`：验证 `<code>` 标签提取
- `auto_fallback`：验证 RAG 兜底触发条件

### 10.2 集成测试（Mock LLM + Mock SDK）

| 测试名 | 目标 |
|---|---|
| `test_chat_no_native_tool` | Chat 模式下 LLM 看不到任何 tool schema |
| `test_chat_codegen_for_math` | Chat 模式输出 `<code>client.calculate(...)</code>` |
| `test_rag_dense_direct_call` | RAG 模式直接 tool_call(dense_retrieval) |
| `test_rag_codegen_complex_search` | RAG 模式写 Python 调 `client.dense_search` + `client.rerank` |
| `test_rag_auto_fallback_on_codegen_failure` | 沙箱报错 N 次后系统自动 dense 检索 |
| `test_rag_progressive_disclosure_round_0` | 第 0 轮只加载技能索引 |
| `test_rag_progressive_disclosure_round_1` | 第 1 轮加载 codegen-guide + retrieval-strategy |
| `test_search_web_search_only` | Search 模式只有 web_search 原生 tool |
| `test_search_no_dense_exposed` | Search 模式物理上看不到 dense_retrieval |
| `test_budget_exhaustion` | 超过 max_iterations 触发降级 |
| `test_cancellation_mid_loop` | cancellation token 立即终止 |
| `test_disclosure_auto_matching` | Auto 模式下根据意图匹配相关 skills |

### 10.3 E2E 测试

- 复用现有前端 E2E 测试套件
- 验证 3 个 mode 的端到端行为
- 验证 SSE 事件流（Activity, ToolResult, Done）

---

## 11. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| **LLM `<code>` 标签格式不规范** | 中 | 解析失败，循环卡住 | 容错解析（支持 markdown code block 作为 fallback）；连续 2 次解析或执行失败触发兜底并进入 Synthesis Phase |
| **沙箱性能瓶颈** | 中 | +100-300ms / code_gen | 预热沙箱；高频 SDK 方法（如 dense_search）鼓励走原生 tool_call（路径 B） |
| **渐进披露策略不准** | 低 | 该加载的技能没加载，LLM 不知道能做什么 | 阶段 1/2 充分测试 Auto 匹配准确率；留手动 override 通道 |
| **replay / eval 适配超期** | 中 | 阶段 4 从 2 周拖到 4 周 | 阶段 1 就开始并行评估 replay/eval 的依赖；预留 buffer |
| **Chat 模式 0 tool 导致 LLM 频繁误调 codegen** | 低 | 简单问题也写 Python，token 浪费 | system prompt 强化 "只有需要计算/查询时才写代码"；budget 限制（max_iterations=2） |
| **dense 兜底路径误触发** | 低 | 不该兜底时兜底，返回无关内容 | 只在 ReAct 循环异常退出（如连续报错、budget 耗尽）时触发；兜底检索结果直接送入 Synthesis Phase，不在循环中继续迭代，保证生成答案的稳定性与 100% 流式输出 |
| **YAML 配置校验不足** | 低 | 启动时缺字段导致 panic | 启动时严格校验所有 YAML；缺字段 = 启动失败并提示 |
| **Free Tier 预算耗尽导致 Action 未完成** | 低 | 无法回答 | 对 RAG/Search/Chat 的 Free Tier 统一设定最低 budget >= 2 轮，从而保证基本的 Action 及 Synthesis 流程能够闭环 |

---

## 12. 影响与后果

### 12.1 正面影响

- **统一循环**：1 个 ReActLoop 替代 3 个 Strategy，循环 bug 只需修 1 处；
- **配置驱动**：调 AI 行为 = 改 YAML/Skill.md，产品/运营可独立迭代；
- **Token 效率**：从 14 个 tool schema 全量加载 → 1 个原生 tool + 渐进披露 skills；
- **扩展性**：新增功能 = SDK 加方法 + Skill 加描述，不改原生 tool schema；
- **模式隔离严格**：Chat 物理上 0 tool，Search 看不到 dense，符合业务预期；
- **兜底可靠**：RAG 始终有 dense 保底，不空手而归。

### 12.2 负面影响

- **迁移成本**：7-8 周；replay/eval/audit 需适配；
- **沙箱依赖**：复杂编排走 codegen，性能受沙箱影响；
- **LLM 格式可靠性**：`<code>` 标签解析比原生 tool_call 脆弱；
- **调试复杂度**：ReAct 循环比状态机更难单步调试（LLM 决策黑盒）。

### 12.3 兼容性

- **前端协议**：`POST /chat` 的 `agent_type` 语义不变；
- **数据库**：`chat_messages` / `message_tags` 表不变；
- **SSE**：复用现有 `AgentEvent` 变体，可能扩展 `ReActIteration` 事件；
- **LLM API**：继续调 `avrag_llm::complete_with_tools`（OpenAI 协议）。

---

## 13. 开放问题

### 13.1 Skill 调用是否最终需要升格为原生 tool

根据 [pi.dev 最佳实践](https://pi.dev)：
> "当某个 skill 驱动的动作被频繁使用、输入输出已稳定、错误成本较高时，就该升格为原生 tool。"

当前设计把所有功能性能力放在 SDK/Skill 层。未来如果 `dense_search` 通过 codegen 的调用频率远高于原生 tool_call，可能需要：
- 方案 A：保持现状（用户通过 Skill 引导优先用原生 tool_call）
- 方案 B：把 `dense_search` 等高频能力也暴露为原生 tool（但违背"每模式 0-1 个原生 tool"原则）

**建议**：通过 eval 跟踪"原生 tool_call 使用率 vs codegen 使用率"，数据驱动决策。

### 13.2 Chat 模式是否需要轻量原生 tool

当前 Chat 0 原生 tool。如果 eval 发现 LLM 频繁写 `<code>client.calculate(...)</code>` 做简单计算，可以考虑：
- 把 `calculate` 升为 Chat 的原生 tool（但用户当前明确否决）
- 或者优化 Skill 引导，让 LLM 直接心算/文本回答简单计算

### 13.3 多轮对话中的 Skill 记忆

如果第 1 轮披露了 `retrieval-strategy` skill，第 2 轮是否还需要重新加载？
- **已采纳决策**：采用**累积式披露 (Cumulative Disclosure)**。已加载的 Skill 会持续保留在 System Prompt 的 disclosed_skills 列表中，直到整个 Session/Run 结束，避免 LLM 在后续迭代中遗忘格式规范或业务约束。

### 13.4 SDK 版本管理

SDK 接口变更是否需要向后兼容？
- 建议：SDK 加 `version` 字段，sandbox 检查版本匹配；旧版本 SDK 方法保留至少 2 个版本。

---

## 14. 参考文档

- ADR-0003: v5 Agent Architecture
- ADR-0004: RAG Agent Loop with Native Tool Calling
- ADR-0005: Unified Agent Kernel（已否决）
- ADR-0005-revised: 基于 v5 的增量扩展（已否决）
- ADR-0006: Unified AgentLoop 原方案（已否决）
- [pi.dev ReAct 最佳实践](https://pi.dev)
- `crates/app/src/agents/strategy/`（将被废弃）
- `crates/app/src/agents/capability/registry.rs`
- `crates/app/src/agents/events.rs`
- `crates/rag-core/src/runtime/tools/code_gen_query.rs`

---

*本文档基于多轮讨论收敛：从 ADR-0005 的"重写 Kernel"到 ADR-0005-revised 的"v5 增量扩展"，再到 ADR-0006 的"1+1 工具"，最终收敛为"统一 ReActLoop + 原生 Tool/Skill 分层 + 按模式隔离 + YAML 配置"。核心驱动力是用户明确的需求：三个模式共享 ReAct 循环、底层功能作为原生 tool、其他功能作为 Skill 渐进披露、严格按模式隔离原生 tool。*
