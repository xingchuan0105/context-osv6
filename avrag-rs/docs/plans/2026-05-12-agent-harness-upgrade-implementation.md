# Agent Harness 三项升级实施计划

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.
> **关联文档:** `docs/superpowers/specs/2026-05-12-agent-harness-upgrades.md`
> **目标:** 将当前 "planner 一次出计划→runtime 跑→synthesizer 出答" 硬流水线升级为真 tool-use 循环，并引入滑动窗口记忆和 Skill 按需加载。

**Goal:** 实现 Agent Harness 三项升级：(1) 真 tool-use 循环，(2) 滑动窗口替换 session_summary，(3) Skill 按需加载。

**Architecture:** 在现有 `react_loop.rs` + `RigModelClient` 基础上，引入 `AgentLoop` 共享循环驱动、`AgentToolRegistry` 工具注册表、`LayeredHistory` 三层滑动记忆、`SkillRegistry` 技能注册表。所有升级在 feature flag 后，老路径保留。

**Tech Stack:** Rust, rig-core (已有 `rig_adapter.rs`), tokio, PostgreSQL, serde_json

---

## Phase A: 工具表 + LlmClient.complete_with_tools + ChatAgent 跑通 tool_use 循环

### Task A1: 在 `common` crate 新增 `ToolSpec` + `StopReason` 类型

**Objective:** 定义模型可见的工具描述格式和停止原因枚举。

**Files:**
- Create: `crates/common/src/tool_spec.rs`
- Modify: `crates/common/src/lib.rs`

**Step 1: 写类型定义**

```rust
// crates/common/src/tool_spec.rs
use serde::{Deserialize, Serialize};

/// Tool specification exposed to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}

/// Why the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    StopSequence,
    ToolUse,
    MaxTokens,
}

/// A single tool call requested by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Response from a model that supports tool calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAwareResponse {
    pub content: String,
    pub tool_calls: Vec<ModelToolCall>,
    pub stop_reason: StopReason,
}
```

**Step 2: 导出**

在 `crates/common/src/lib.rs` 中加入 `pub use tool_spec::*;`

**Step 3: 验证编译**

Run: `cd /home/chuan/context-osv6/avrag-rs && cargo check -p common`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/common/src/tool_spec.rs crates/common/src/lib.rs
git commit -m "feat(common): add ToolSpec, StopReason, ModelToolCall for agent tool-use loop"
```

---

### Task A2: LlmClient 增加 `complete_with_tools` 方法

**Objective:** 让 `LlmClient` 支持带工具调用的 completion（非流式先实现，流式后续）。

**Files:**
- Modify: `crates/llm/src/client.rs`

**Step 1: 修改 `build_chat_completion_request_body` 支持 tools**

在现有函数签名增加 `tools: Option<&[common::ToolSpec]>` 参数。当 `tools` 存在时，在 request body 中加入 `"tools"` 字段（OpenAI 格式）。

**Step 2: 新增 `complete_with_tools` 方法**

```rust
pub async fn complete_with_tools(
    &self,
    messages: &[ChatMessage],
    tools: &[common::ToolSpec],
    temperature: Option<f32>,
) -> anyhow::Result<ToolAwareResponse> {
    // 复用现有 HTTP 客户端逻辑
    // request body 包含 tools 字段
    // 解析 response，提取 content + tool_calls + finish_reason
}
```

**Step 3: 验证编译**

Run: `cargo check -p avrag-llm`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/llm/src/client.rs
git commit -m "feat(llm): add complete_with_tools to LlmClient"
```

---

### Task A3: 新增 `AgentToolRegistry` 工具注册表

**Objective:** 把 RAG 工具、搜索工具、记忆工具包成 agent 视角的统一工具表。

**Files:**
- Create: `crates/app/src/agents/tool_registry.rs`
- Modify: `crates/app/src/agents/mod.rs`

**Step 1: 定义注册表**

```rust
// crates/app/src/agents/tool_registry.rs
use common::{ToolSpec, ToolResult, ToolStatus};
use std::collections::HashMap;

pub struct AgentToolRegistry {
    tools: HashMap<String, Box<dyn AgentTool>>,
    specs: Vec<ToolSpec>,
}

#[async_trait::async_trait]
pub trait AgentTool: Send + Sync {
    fn spec(&self) -> ToolSpec;
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult>;
}

impl AgentToolRegistry {
    pub fn new() -> Self { ... }
    pub fn register(&mut self, tool: Box<dyn AgentTool>) { ... }
    pub fn specs_for_kind(&self, kind: AgentKind) -> Vec<ToolSpec> { ... }
    pub async fn execute(&self, name: &str, args: serde_json::Value) -> anyhow::Result<ToolResult> { ... }
}
```

**Step 2: 实现占位工具**

Phase A 只需要两个占位工具（后续 Phase B/C/D 替换为真实实现）：
- `load_skill` — 返回 noop
- `compact_history` — 返回 noop

```rust
pub struct PlaceholderTool {
    name: String,
    description: String,
}

#[async_trait::async_trait]
impl AgentTool for PlaceholderTool {
    fn spec(&self) -> ToolSpec { ... }
    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            status: ToolStatus::Success,
            data: serde_json::json!({"status": "noop", "reason": "not_yet_implemented"}),
        })
    }
}
```

**Step 3: 按 AgentKind 过滤工具可见性**

```rust
pub fn specs_for_kind(&self, kind: AgentKind) -> Vec<ToolSpec> {
    match kind {
        AgentKind::Chat => vec!["load_skill", "compact_history"],
        AgentKind::Rag => vec!["load_skill", "compact_history", "dense_retrieval", "lexical_retrieval", "graph_retrieval", "index_lookup", "doc_summary", "doc_metadata", "search_web"],
        AgentKind::Search => vec!["load_skill", "compact_history", "brave_search", "fetch_full_page"],
    }.iter().filter_map(|name| self.tools.get(*name).map(|t| t.spec())).collect()
}
```

**Step 4: 验证编译**

Run: `cargo check -p app`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/app/src/agents/tool_registry.rs crates/app/src/agents/mod.rs
git commit -m "feat(app): add AgentToolRegistry with placeholder tools"
```

---

### Task A4: 新增 `AgentLoop` 共享循环驱动

**Objective:** 实现所有 agent 共享的 tool-calling 循环。

**Files:**
- Create: `crates/app/src/agents/agent_loop.rs`
- Modify: `crates/app/src/agents/mod.rs`

**Step 1: 定义循环状态**

```rust
// crates/app/src/agents/agent_loop.rs
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::react_loop::{LoopBudget, DegradeReason};
use crate::agents::tool_registry::AgentToolRegistry;
use common::{AppError, StopReason, ToolAwareResponse};
use tokio_util::sync::CancellationToken;

pub struct AgentLoopState {
    pub messages: Vec<avrag_llm::ChatMessage>,
    pub tool_registry: AgentToolRegistry,
    pub budget: LoopBudget,
    pub tool_specs: Vec<common::ToolSpec>,
}

pub struct AgentLoopResult {
    pub content: String,
    pub usage: Option<crate::agents::events::AgentUsage>,
    pub iterations: u8,
    pub degrade_trace: Vec<common::DegradeTraceItem>,
}
```

**Step 2: 实现循环**

```rust
pub async fn agent_loop(
    state: &mut AgentLoopState,
    llm: &avrag_llm::LlmClient,
    sink: &dyn AgentEventSink,
    cancel: &CancellationToken,
    temperature: Option<f32>,
) -> Result<AgentLoopResult, AppError> {
    let mut iterations = 0u8;
    let mut degrade_trace = Vec::new();
    let mut final_usage = None;

    loop {
        if cancel.is_cancelled() {
            return Err(AppError::internal("request cancelled"));
        }

        let response = llm.complete_with_tools(
            &state.messages,
            &state.tool_specs,
            temperature,
        ).await.map_err(|e| AppError::internal(format!("LLM tool completion failed: {}", e)))?;

        state.messages.push(avrag_llm::ChatMessage::assistant(&response.content));

        match response.stop_reason {
            StopReason::EndTurn | StopReason::StopSequence => {
                // finalize
                return Ok(AgentLoopResult {
                    content: response.content,
                    usage: final_usage,
                    iterations,
                    degrade_trace,
                });
            }
            StopReason::ToolUse => {
                if state.budget.exhausted() {
                    degrade_trace.push(common::DegradeTraceItem {
                        stage: "agent_loop".to_string(),
                        reason: "budget_exhausted".to_string(),
                        impact: "LoopBudget max_iterations reached".to_string(),
                    });
                    return Ok(AgentLoopResult {
                        content: "[Agent stopped: iteration budget exhausted]".to_string(),
                        usage: final_usage,
                        iterations,
                        degrade_trace,
                    });
                }

                // Emit activity for each tool call
                for tc in &response.tool_calls {
                    let _ = sink.emit(AgentEvent::Activity {
                        stage: "tool_use".to_string(),
                        message: format!("{}({})", tc.name, tc.arguments),
                    }).await;
                }

                // Execute all tools in parallel
                let mut tool_results = Vec::new();
                for tc in &response.tool_calls {
                    match state.tool_registry.execute(&tc.name, tc.arguments.clone()).await {
                        Ok(result) => tool_results.push(result),
                        Err(e) => tool_results.push(common::ToolResult {
                            status: common::ToolStatus::Error,
                            data: serde_json::json!({"error": e.to_string()}),
                        }),
                    }
                }

                // Build tool result message
                let tool_result_text = serde_json::to_string(&tool_results)
                    .unwrap_or_else(|_| "[tool results serialization failed]".to_string());
                state.messages.push(avrag_llm::ChatMessage::user(format!(
                    "<tool_results>{}</tool_results>", tool_result_text
                )));

                state.budget.tick();
                iterations += 1;
            }
            StopReason::MaxTokens => {
                degrade_trace.push(common::DegradeTraceItem {
                    stage: "agent_loop".to_string(),
                    reason: "max_tokens".to_string(),
                    impact: "Context overflow".to_string(),
                });
                return Ok(AgentLoopResult {
                    content: response.content,
                    usage: final_usage,
                    iterations,
                    degrade_trace,
                });
            }
        }
    }
}
```

**Step 3: 验证编译**

Run: `cargo check -p app`
Expected: PASS (可能有未解析的 import，需调整)

**Step 4: Commit**

```bash
git add crates/app/src/agents/agent_loop.rs crates/app/src/agents/mod.rs
git commit -m "feat(app): add AgentLoop shared tool-calling loop driver"
```

---

### Task A5: ChatAgent 迁移到 agent_loop

**Objective:** 让 ChatAgent 通过 `agent_loop` 运行，工具集 = `[load_skill, compact_history]`。

**Files:**
- Modify: `crates/app/src/agents/chat_agent.rs`

**Step 1: 修改 `ChatAgent::run`**

保留现有 `build_chat_messages` 逻辑（system prompt + history + query），但把 LLM 调用替换为 `agent_loop`。

```rust
#[async_trait::async_trait]
impl Agent for ChatAgent {
    async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        let Some(ref llm) = self.llm_client else { ... };

        let messages = build_chat_messages(&request);

        let mut tool_registry = AgentToolRegistry::new();
        tool_registry.register(Box::new(PlaceholderTool::load_skill()));
        tool_registry.register(Box::new(PlaceholderTool::compact_history()));

        let tool_specs = tool_registry.specs_for_kind(AgentKind::Chat);

        let mut state = AgentLoopState {
            messages,
            tool_registry,
            budget: LoopBudget::new(3), // Chat 默认 3 次 tool_use
            tool_specs,
        };

        let token = request.cancellation_token.clone().unwrap_or_default();

        let loop_result = agent_loop(
            &mut state,
            llm,
            sink,
            &token,
            self.temperature,
        ).await?;

        // Emit final answer
        let _ = sink.emit(AgentEvent::MessageDelta {
            text: loop_result.content.clone(),
        }).await;

        let _ = sink.emit(AgentEvent::Done {
            final_message: Some(loop_result.content.clone()),
            usage: loop_result.usage,
        }).await;

        Ok(AgentRunResult {
            answer: loop_result.content,
            ..AgentRunResult::default()
        })
    }
}
```

**Step 2: 保留 stream 路径**

当前 `agent_loop` 不支持流式。Phase A 先让非流式路径走 `agent_loop`，流式路径保留现有 `complete_stream` 逻辑。加 feature flag `AGENT_TOOL_LOOP_ENABLED` 控制。

```rust
if !std::env::var("AGENT_TOOL_LOOP_ENABLED").map(|v| v == "true").unwrap_or(false) {
    // 走老路径
    return legacy_chat_run(self, request, sink).await;
}
```

**Step 3: 验证编译 + 测试**

Run: `cargo check -p app`
Run: `cargo test -p app chat_agent`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/app/src/agents/chat_agent.rs
git commit -m "feat(app): migrate ChatAgent to AgentLoop behind AGENT_TOOL_LOOP_ENABLED flag"
```

---

## Phase B: RagAgent / WebSearchAgent 迁移到 agent_loop

### Task B1: 在 `AgentToolRegistry` 中接入真实 RAG 工具

**Objective:** 把 `rag-core` 的 6 个工具包成 `AgentTool` 实现。

**Files:**
- Modify: `crates/app/src/agents/tool_registry.rs`

**Step 1: 实现 `RagDenseRetrievalTool`**

```rust
pub struct RagDenseRetrievalTool {
    rag_runtime: Arc<avrag_rag_core::RagRuntime>,
}

#[async_trait::async_trait]
impl AgentTool for RagDenseRetrievalTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "dense_retrieval".to_string(),
            description: "Retrieve relevant chunks using dense vector similarity".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query"},
                    "doc_scope": {"type": "array", "items": {"type": "string"}},
                    "top_k": {"type": "integer", "default": 10}
                },
                "required": ["query", "doc_scope"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        // 调用 rag_runtime 的 dense retrieval
        // 返回 ToolResult { status: Success, data: chunks }
    }
}
```

类似实现其余 5 个工具：`lexical_retrieval`, `graph_retrieval`, `index_lookup`, `doc_summary`, `doc_metadata`。

**Step 2: 实现 `SearchWebTool`**

```rust
pub struct SearchWebTool {
    executor: avrag_search::SearchExecutor,
}

#[async_trait::async_trait]
impl AgentTool for SearchWebTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "search_web".to_string(),
            description: "Search the web using Brave LLM Context".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "vertical": {"type": "string", "enum": ["web", "news"]}
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let query = args["query"].as_str().unwrap_or("");
        let results = self.executor.execute_search(query, None).await?;
        Ok(ToolResult {
            status: ToolStatus::Success,
            data: serde_json::to_value(results)?,
        })
    }
}
```

**Step 3: 验证编译**

Run: `cargo check -p app`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/app/src/agents/tool_registry.rs
git commit -m "feat(app): wire real RAG and Search tools into AgentToolRegistry"
```

---

### Task B2: RagAgent 迁移到 agent_loop

**Objective:** 删除 plan→execute→evaluate 显式三阶段，改为 agent_loop 驱动。

**Files:**
- Modify: `crates/app/src/agents/rag_agent.rs`

**Step 1: 重写 `RagAgent::run`**

```rust
#[async_trait::async_trait]
impl Agent for RagAgent {
    async fn run(&self, request: AgentRequest, sink: &dyn AgentEventSink) -> Result<AgentRunResult, AppError> {
        if !std::env::var("AGENT_TOOL_LOOP_ENABLED").map(|v| v == "true").unwrap_or(false) {
            return legacy_rag_run(self, request, sink).await;
        }

        // 验证 doc_scope
        if request.doc_scope.is_empty() { ... }
        let Some(rag) = self.rag_runtime.clone() else { ... };
        let llm = self.llm_client.clone().ok_or_else(|| ...)?;

        // 构建 messages
        let mut messages = build_rag_messages(&request)?;

        // 构建 tool registry
        let mut tool_registry = AgentToolRegistry::new();
        tool_registry.register(Box::new(RagDenseRetrievalTool::new(rag.clone())));
        tool_registry.register(Box::new(RagLexicalRetrievalTool::new(rag.clone())));
        // ... 其他工具
        tool_registry.register(Box::new(SearchWebTool::new(...)));
        tool_registry.register(Box::new(PlaceholderTool::load_skill()));
        tool_registry.register(Box::new(PlaceholderTool::compact_history()));

        let tool_specs = tool_registry.specs_for_kind(AgentKind::Rag);

        let mut state = AgentLoopState {
            messages,
            tool_registry,
            budget: LoopBudget::new(6), // RAG 默认 6 次 tool_use
            tool_specs,
        };

        let token = request.cancellation_token.clone().unwrap_or_default();

        // evaluator 改为 hint 注入器：在 tool result 后附加客观信号
        let loop_result = agent_loop_with_evaluator_hints(
            &mut state,
            &llm,
            sink,
            &token,
            self.temperature,
            &request,
        ).await?;

        // streaming 处理...

        Ok(AgentRunResult {
            answer: loop_result.content,
            degrade_trace: loop_result.degrade_trace,
            iterations: vec![], // TODO: 从 agent_loop 收集 iteration 记录
            final_decision: Some(FinalDecision::Synthesized),
            ..AgentRunResult::default()
        })
    }
}
```

**Step 2: 实现 `agent_loop_with_evaluator_hints`**

在标准 `agent_loop` 基础上，每次 tool result 回来后，运行 evaluator 计算客观信号，如果信号不足则在下一轮 user message 中附加 hint。

```rust
async fn agent_loop_with_evaluator_hints(
    state: &mut AgentLoopState,
    llm: &LlmClient,
    sink: &dyn AgentEventSink,
    cancel: &CancellationToken,
    temperature: Option<f32>,
    request: &AgentRequest,
) -> Result<AgentLoopResult, AppError> {
    // 复用 agent_loop 的核心逻辑，但在 ToolUse 分支后插入 evaluator
    // ...
}
```

**Step 3: 验证编译 + 测试**

Run: `cargo check -p app`
Run: `cargo test -p app rag_agent`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/app/src/agents/rag_agent.rs
git commit -m "feat(app): migrate RagAgent to AgentLoop with evaluator hints"
```

---

### Task B3: WebSearchAgent 迁移到 agent_loop

**Objective:** 同 RagAgent，让 WebSearchAgent 走 agent_loop。

**Files:**
- Modify: `crates/app/src/agents/web_search_agent.rs`

**Step 1: 重写 `WebSearchAgent::run`**

类似 RagAgent，工具集 = `[brave_search, fetch_full_page, load_skill, compact_history]`。

**Step 2: 验证编译 + 测试**

Run: `cargo check -p app`
Run: `cargo test -p app web_search_agent`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/app/src/agents/web_search_agent.rs
git commit -m "feat(app): migrate WebSearchAgent to AgentLoop"
```

---

## Phase C: 滑动窗口记忆 (LayeredHistory)

### Task C1: messages 表 migration

**Objective:** 给 `chat_messages` 表加 `layer` 和 `summary_text` 列。

**Files:**
- Create: `migrations/00XX_messages_layer.sql`

**Step 1: 写 migration**

```sql
-- migrations/00XX_messages_layer.sql
ALTER TABLE chat_messages
    ADD COLUMN layer SMALLINT NOT NULL DEFAULT 1,
    ADD COLUMN summary_text TEXT NULL;

CREATE INDEX idx_chat_messages_layer ON chat_messages(session_id, layer, created_at);
```

**Step 2: Commit**

```bash
git add migrations/00XX_messages_layer.sql
git commit -m "feat(db): add layer and summary_text to chat_messages"
```

---

### Task C2: `LayeredHistory` 数据模型 + PG 查询

**Objective:** 实现三层记忆的查询和构建。

**Files:**
- Create: `crates/storage-pg/src/repositories/messages_layered.rs`
- Modify: `crates/storage-pg/src/lib.rs`

**Step 1: 定义模型**

```rust
#[derive(Debug, Clone)]
pub struct LayeredHistory {
    pub layer1: Vec<ChatMessageRow>, // 最近 N 轮，完整原文
    pub layer2: Vec<ChatMessageRow>, // N+1 ~ 2N，摘要段
    pub layer3: Option<String>,      // 更早，单段长摘要
}

#[derive(Debug, Clone)]
pub struct ChatMessageRow {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub summary_text: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

**Step 2: 实现查询**

```rust
pub async fn select_layered(
    pool: &PgPool,
    session_id: &str,
    hot_window: usize,
) -> Result<LayeredHistory, sqlx::Error> {
    // Layer 1: 最近 hot_window 条 layer=1
    // Layer 2: 接下来的 hot_window 条 layer=2
    // Layer 3: 最早的 layer=3 摘要
}
```

**Step 3: 验证编译**

Run: `cargo check -p avrag-storage-pg`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/storage-pg/src/repositories/messages_layered.rs crates/storage-pg/src/lib.rs
git commit -m "feat(storage-pg): add LayeredHistory query model"
```

---

### Task C3: `compact_history` 工具实现

**Objective:** 让模型能调用 `compact_history` 触发 promote。

**Files:**
- Modify: `crates/app/src/agents/tool_registry.rs`
- Create: `crates/app/src/lib_impl/memory_layered.rs`

**Step 1: 实现 promote 逻辑**

```rust
pub async fn promote_layers(
    pool: &PgPool,
    session_id: &str,
    hot_window: usize,
    llm: &LlmClient,
) -> Result<(), AppError> {
    // 1. 检查 layer1 数量 > hot_window
    // 2. 把超出的 layer1  oldest 移到 layer2（生成摘要）
    // 3. 检查 layer2 数量 > hot_window
    // 4. 把超出的 layer2 oldest 合并到 layer3（生成长摘要）
}
```

**Step 2: 实现 `CompactHistoryTool`**

```rust
pub struct CompactHistoryTool {
    pool: PgPool,
    llm: LlmClient,
    hot_window: usize,
}

#[async_trait::async_trait]
impl AgentTool for CompactHistoryTool {
    fn spec(&self) -> ToolSpec { ... }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let keep_recent = args["keep_recent"].as_u64().unwrap_or(self.hot_window as u64);
        promote_layers(&self.pool, &session_id, keep_recent as usize, &self.llm).await?;
        Ok(ToolResult {
            status: ToolStatus::Success,
            data: serde_json::json!({"status": "compacted"}),
        })
    }
}
```

**Step 3: Commit**

```bash
git add crates/app/src/lib_impl/memory_layered.rs crates/app/src/agents/tool_registry.rs
git commit -m "feat(app): implement compact_history tool with layer promotion"
```

---

### Task C4: AgentRequest 接入 LayeredHistory

**Objective:** 替换 `session_summary` 为 `layered_history`。

**Files:**
- Modify: `crates/app/src/agents/runtime.rs`
- Modify: `crates/app/src/agents/chat_agent.rs`
- Modify: `crates/app/src/agents/rag_agent.rs`
- Modify: `crates/app/src/agents/web_search_agent.rs`

**Step 1: 在 `AgentRequest` 中新增 `layered_history` 字段**

```rust
pub struct AgentRequest {
    // ... existing fields ...
    pub session_summary: Option<String>, // 保留一个 release，但不再写入
    pub layered_history: Option<LayeredHistory>, // 新增
    // ...
}
```

**Step 2: 修改 `build_chat_messages` 使用 layered_history**

```rust
fn build_chat_messages(request: &AgentRequest) -> Vec<ChatMessage> {
    let mut system = String::from(include_str!("..."));

    if let Some(ref layered) = request.layered_history {
        if let Some(ref layer3) = layered.layer3 {
            system.push_str("\n\nConversation context:\n");
            system.push_str(layer3);
        }
    }

    let mut messages = vec![ChatMessage::system(system)];

    // Layer 2: 作为 assistant/user 交替消息注入
    if let Some(ref layered) = request.layered_history {
        for msg in &layered.layer2 {
            match msg.role.as_str() {
                "assistant" => messages.push(ChatMessage::assistant(&msg.summary_text.unwrap_or(msg.content.clone()))),
                _ => messages.push(ChatMessage::user(&msg.summary_text.unwrap_or(msg.content.clone()))),
            }
        }
    }

    // Layer 1: 原文
    if let Some(ref layered) = request.layered_history {
        for msg in &layered.layer1 {
            match msg.role.as_str() {
                "assistant" => messages.push(ChatMessage::assistant(&msg.content)),
                _ => messages.push(ChatMessage::user(&msg.content)),
            }
        }
    }

    messages.push(ChatMessage::user(&request.query));
    messages
}
```

**Step 3: Commit**

```bash
git add crates/app/src/agents/runtime.rs crates/app/src/agents/chat_agent.rs crates/app/src/agents/rag_agent.rs crates/app/src/agents/web_search_agent.rs
git commit -m "feat(app): integrate LayeredHistory into AgentRequest and message building"
```

---

## Phase D: Skill 按需加载

### Task D1: `skills/` 目录初始化

**Objective:** 创建 skills 目录和初始 skill 文件。

**Files:**
- Create: `avrag-rs/skills/citation-format.md`
- Create: `avrag-rs/skills/refusal-templates.md`
- Create: `avrag-rs/skills/rag-answer-style.md`
- Create: `avrag-rs/skills/web-search-synthesis.md`

**Step 1: 创建 skill 文件**

每个 skill 文件格式：

```markdown
---
name: citation-format
description: COS6 citation format specification
applicable_when: agent_kind == "rag"
agent_kinds: [rag]
languages: [zh, en]
---

# Citation Format

- Use [^cite:doc_id:chunk_id] format
- ...
```

**Step 2: Commit**

```bash
git add skills/
git commit -m "feat(skills): initialize skill directory with 4 core skills"
```

---

### Task D2: `SkillRegistry` 实现

**Objective:** 编译期嵌入 skills 目录，提供查询和加载接口。

**Files:**
- Create: `crates/app/src/skills/mod.rs`
- Modify: `crates/app/src/lib.rs` 或 `crates/app/src/agents/mod.rs`

**Step 1: 实现 SkillRegistry**

```rust
// crates/app/src/skills/mod.rs
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};

static SKILLS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../skills");

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub applicable_when: String,
    pub agent_kinds: Vec<String>,
    pub languages: Vec<String>,
    pub content: String,
}

pub struct SkillRegistry {
    skills: Vec<Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        let mut skills = Vec::new();
        for entry in SKILLS_DIR.files() {
            if let Some(content) = entry.contents_utf8() {
                if let Some(skill) = Self::parse_skill(content) {
                    skills.push(skill);
                }
            }
        }
        Self { skills }
    }

    fn parse_skill(content: &str) -> Option<Skill> {
        // 解析 YAML frontmatter + markdown body
    }

    pub fn get(&self, name: &str, lang: Option<&str>) -> Option<&Skill> {
        // 按 name + lang 匹配
    }

    pub fn list_for_agent(&self, kind: &str) -> Vec<&Skill> {
        self.skills.iter().filter(|s| s.agent_kinds.contains(&kind.to_string())).collect()
    }
}
```

**Step 2: 实现 `LoadSkillTool`**

```rust
pub struct LoadSkillTool {
    registry: SkillRegistry,
}

#[async_trait::async_trait]
impl AgentTool for LoadSkillTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "load_skill".to_string(),
            description: "Load a skill file to get domain-specific instructions".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "enum": self.registry.list_names()},
                    "lang": {"type": "string", "default": "zh"}
                },
                "required": ["name"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let name = args["name"].as_str().unwrap_or("");
        let lang = args["lang"].as_str();
        match self.registry.get(name, lang) {
            Some(skill) => Ok(ToolResult {
                status: ToolStatus::Success,
                data: serde_json::json!({"content": skill.content}),
            }),
            None => Ok(ToolResult {
                status: ToolStatus::Error,
                data: serde_json::json!({"error": "skill not found"}),
            }),
        }
    }
}
```

**Step 3: Commit**

```bash
git add crates/app/src/skills/mod.rs crates/app/src/agents/tool_registry.rs
git commit -m "feat(app): add SkillRegistry with compile-time skill embedding"
```

---

### Task D3: 精简 system prompt + 默认 skill 预注入

**Objective:** 把现有 prompts 中的领域知识迁出到 skills，system prompt 只保留通用协议。

**Files:**
- Create: `prompts/agent_protocol.txt`
- Modify: `prompts/chat_agent_system.txt`
- Modify: `prompts/rag_plan_system.txt`
- Modify: `prompts/rag_answer_system.txt`
- Modify: `prompts/web_search_system.txt`

**Step 1: 创建通用协议 prompt**

```
# prompts/agent_protocol.txt
You are an AI assistant for Context OS.

## Protocol
- You have access to tools. Use them when needed.
- When you are done, stop and wait for the user.
- Do not make up information. If you need facts, use retrieval tools.
```

**Step 2: 精简现有 prompts**

把 `chat_agent_system.txt` 中的 citation 规范、风格指南等迁到 skills，只保留角色定义和基本约束。

**Step 3: 在 agent_loop 启动时预注入默认 skill**

```rust
// 在 agent_loop 初始化时，根据 AgentKind 自动加载默认 skill
let default_skills = match kind {
    AgentKind::Rag => vec!["rag-answer-style", "citation-format"],
    AgentKind::Search => vec!["web-search-synthesis"],
    AgentKind::Chat => vec![],
};
for skill_name in default_skills {
    if let Some(skill) = skill_registry.get(skill_name, Some(lang)) {
        messages.push(ChatMessage::user(format!("<skill name=\"{}\">{}</skill>", skill_name, skill.content)));
    }
}
```

**Step 4: Commit**

```bash
git add prompts/agent_protocol.txt prompts/chat_agent_system.txt prompts/rag_plan_system.txt prompts/rag_answer_system.txt prompts/web_search_system.txt crates/app/src/agents/agent_loop.rs
git commit -m "feat(prompts): slim down system prompts, add agent_protocol, pre-inject default skills"
```

---

## Phase E: Feature flag 默认开启 + 老路径 deprecation

### Task E1: Feature flag 配置化

**Objective:** 把 `AGENT_TOOL_LOOP_ENABLED` 从 env var 升级为配置项。

**Files:**
- Modify: `crates/app/src/lib_impl/config.rs`
- Modify: `.env.example`

**Step 1: 加配置项**

```rust
pub struct AgentConfig {
    pub tool_loop_enabled: bool,
    pub layered_memory_enabled: bool,
    pub skill_registry_enabled: bool,
}
```

**Step 2: Commit**

```bash
git add crates/app/src/lib_impl/config.rs .env.example
git commit -m "feat(config): add agent harness upgrade feature flags"
```

---

### Task E2: 全量验证 + 清理

**Objective:** 运行全量测试，确保无回归。

**Step 1: 编译检查**

Run: `cargo check --workspace`
Expected: PASS

**Step 2: 运行测试**

Run: `cargo test --workspace`
Expected: PASS（老路径测试仍通过；新路径测试新增）

**Step 3: Commit**

```bash
git commit -m "chore: agent harness upgrade complete, all tests passing"
```

---

## 验收标准

1. `cargo check --workspace` 通过
2. `cargo test --workspace` 通过
3. ChatAgent 在 `AGENT_TOOL_LOOP_ENABLED=true` 下能调用 `load_skill` 和 `compact_history`
4. RagAgent 在 tool-use 模式下能自主调用 RAG 工具 + `search_web` 兜底
5. WebSearchAgent 能自主再调搜索（不同 vertical）
6. 滑动窗口：50 轮会话 token 占用 < 12k
7. Skill：baseline system prompt < 400 tokens
8. 所有升级在 feature flag 后，关闭 flag 回到老路径

---

## 风险与回滚

| 风险 | 缓解 |
|------|------|
| Rig tool-calling 在 DeepSeek 上不稳定 | Phase A 第一周做 spike，失败则推迟 |
| 模型调爆工具 | LoopBudget 硬上限 |
| 新路径召回率劣化 | 100 条回归 query A/B 测试 |
| 迁移期老数据无 layer 字段 | migration 默认值 + 后台 backfill |

**回滚：** 关闭 `AGENT_TOOL_LOOP_ENABLED` / `MEMORY_LAYERED_ENABLED` / `SKILL_REGISTRY_ENABLED` 即回到当前代码路径。
