# ADR-0006: 统一 AgentLoop（1+1 工具 + ReAct 单一循环）

> **⛔ 归档 / ARCHIVED（2026-06-08）**  
> 已被 [0006-unified-agent-loop-revised.md](0006-unified-agent-loop-revised.md) 取代；披露与 orchestrator 以 [ADR-0007](0007-react-phased-context-disclosure.md) 为准。  
> 索引：[ARCHIVE-superseded-by-adr-0007.md](../agents/ARCHIVE-superseded-by-adr-0007.md)

| 项目 | 内容 |
|---|---|
| 状态 | 已采纳（替代 ADR-0005 / 0005-revised） |
| 决策日期 | 2026-06-07 |
| 提出者 | AI 助手（与用户共同决策） |
| 影响范围 | `crates/app/src/agents/unified/`、`crates/app/src/agents/strategy/`（删除）、`crates/rag-core/src/runtime/`（SDK 化）、`prompts/skills/` |

---

## 1. 背景与动机

### 1.1 必须保留的 3 个设计原则

avrag-rs v5 架构虽复杂，但沉淀了 3 个核心设计原则，新架构必须保留：

**原则 1：渐进式披露（Progressive Disclosure）**
- 业务诉求：每轮 LLM 调用只加载"当前阶段需要的工具 / Skill"，节省 token + 减轻 LLM 认知负担
- v5 实现：4 阶段（INIT/PLAN/EXECUTE/EVAL/ANSWER）状态机 + DisclosureTier (Index/Load/Runtime)
- 新架构中：**原则保留，实现机制改为 SDK 自动派发**。LLM 写 `client.search(query, method="auto")`，系统内部选最佳检索方式，LLM 无需关心

**原则 2：AI 自治的历史管理（指代消解）**
- 业务诉求：用户说"那个呢"时，AI 能回想起上文指什么；AI 决定何时读历史、按什么标签读
- v5 实现：`conversation_history_load` / `conversation_history_tag` 工具（LLM 在 Plan 阶段自主调用）
- 新架构中：**完整保留**，通过 SDK 的 `client.recall` / `client.remember` 暴露，LLM 完全自主

**原则 3：ReAct 风格的 LLM 思考循环**
- 业务诉求：LLM 自己决定"调什么工具"、"调几个"、"什么时候停"
- v5 实现：受限于 4 阶段状态机（每个阶段 LLM 角色固定）
- 新架构中：**彻底采用纯 ReAct 循环**（单一循环，LLM 完全自治）

### 1.2 v5 现状的核心问题

| 问题 | 业务影响 | 根因 |
|---|---|---|
| 3 个独立 Strategy state machine 重复实现循环骨架 | 修一个 loop bug 要改 3 处 | 缺乏抽象 |
| 14 个 tool schema 散落在 3 个 mode | LLM 每轮看 ~2000 token schema，token 浪费 | 工具碎片化 |
| 调 AI 行为要改 Rust 代码 | 产品 / 运营无法独立调优 | 配置与代码未分离 |
| 4 阶段状态机复杂 | 状态枚举、回放、调试都难 | 过度工程化 |
| Chat 模式还在 pre-ADR-0004 架构（XML 解析） | 落后于 RAG / Search | 迁移不彻底 |

### 1.3 目标

**彻底重做**：1 个 AgentLoop + 1+1 工具 + 1 个 system prompt + YAML 配置驱动。

**业务价值**：
- **调 AI 行为 = 改 YAML + 改 SKILL.md**，不动 Rust 代码
- **加新 mode = 加 1 份 YAML + 1 个 SKILL.md**，不写循环代码
- **未来加工具 = 改 SDK 1 处**，LLM 自动能用

### 1.4 非目标

- 不替换 `avrag-llm`（已 production-tested）
- 不引入 Rig / LangGraph / DSPy 等外部框架
- 不改前端 SSE 事件协议（复用现有 `AgentEvent` 变体）
- 不改 `avrag-auth` / `avrag-storage-pg` / `avrag-retrieval-data-plane` 等基础设施

---

## 2. 决策

### 2.1 核心决策

采用 **"1 个 AgentLoop + 1+1 工具 + ReAct 单一循环 + 1 个 system prompt"** 架构：

| 维度 | 决策 | 理由 |
|---|---|---|
| 循环 | **ReAct 单一循环**（无阶段） | 4 阶段是过度工程化；LLM 自己决定何时停 |
| 工具数量 | **1+1**：`code_gen_query` + `web_search` | 14 工具 = 过度暴露；SDK + 沙箱替代显式 schema |
| Tool call vs Skill | **2 个 tool_call**，其余 SDK 内化 | Python 沙箱 + ReAct 自纠错替代 schema 严格校验 |
| Role prompt | **1 个 system prompt**（3 段式） | 3 个 role 合并；LLM 自然涌现行为 |
| 配置 | **YAML 极简**（3 section：system_prompt / tools / budget） | 调 AI 行为 = 改 YAML |
| Replay / Eval | **Event 流 checkpoint** | 替代 State 边界枚举 |

### 2.2 关键边界决策

| 决策项 | 结论 | 理由 |
|---|---|---|
| 工具数量 | 1+1 | SDK 封装所有后端；LLM 写 Python 调 SDK；web_search 单列因成本 / 垂直分类特殊 |
| Tool call 接口 | OpenAI / Claude 原生 tool_call 协议 | 不引入自定义协议；保持与 LLM provider 兼容 |
| Chat 模式工具 | 0 tool 起步 | 95% Chat 不需要工具；LLM 按需调 code_gen_query |
| 错误恢复 | ReAct 自纠错（sandbox 错误 → LLM 修正） | 不预校验 schema；信任 LLM 通过反馈学习 |
| Sandbox 实现 | 复用现有 `code_gen_query` Rust 实现 | 已 production-tested；Python 解释器已就位 |
| SDK 入口 | 业务动词方法（`client.search` / `client.fetch`） | 屏蔽内部实现；LLM 表达"做什么"而非"调什么 API" |
| `method` 参数 | 默认 `"auto"`，系统选最佳 | 减少 LLM 决策负担；精细控制仍可显式传 |
| 并行 tool call | 支持（LLM 一次发多个 code_gen_query） | 提高效率；kernel 串行 / 并行执行可配置 |
| 流式输出 | 仅最终答案阶段流式 | Plan / Execute 阶段 LLM 输出是结构化 tool_call，无需流式 |
| 取消 | 每次 LLM call 边界检查 cancellation token | 避免 sandbox 卡死时无法取消 |

### 2.3 已否决的替代方案

| 方案 | 否决理由 |
|---|---|
| 保留 v5 状态机 | 循环重复、工具碎片、调优要改代码 |
| 4 阶段 + 14 工具 | 阶段划分不必要；14 工具 = LLM 认知负担 |
| 合并到 5 工具 | 仍需为每个工具写 schema；不如用 SDK 统一 |
| 1 tool（仅 code_gen_query） | web_search 简单关键词场景太常见，强制走 Python 沙箱有性能开销 |
| 引入 LangGraph / Rig | 增加外部依赖；社区成熟度不足；当前需求 ReAct 足够 |

---

## 3. 整体架构图

```
┌──────────────────────────────────────────────────────────┐
│  Frontend                                                │
│  POST /chat { agent_type: "chat"|"rag"|"search", ... }   │
└────────────────────┬─────────────────────────────────────┘
                     ↓
┌──────────────────────────────────────────────────────────┐
│  UnifiedAgent::run (crates/app/src/agents/unified/)      │
│  1. 解析 request, 选择 mode config                        │
│  2. 调用 AgentLoop::run(mode, request)                    │
└────────────────────┬─────────────────────────────────────┘
                     ↓
┌──────────────────────────────────────────────────────────┐
│  AgentLoop (单一 ReAct 循环)                              │
│                                                          │
│  state = {                                               │
│    mode: ModeConfig,           # YAML 加载               │
│    system_prompt: String,      # 来自 SKILL.md            │
│    messages: Vec<ChatMessage>, # 累积对话                │
│    iteration: u8,              # 当前迭代                │
│    events: Vec<TraceEvent>,    # event 流（replay 用）    │
│  }                                                       │
│                                                          │
│  loop:                                                   │
│    1. 构造 messages = [system] + history + user_query    │
│    2. 调 LLM (complete_with_tools)                       │
│    3. LLM 选:                                             │
│       a. tool_call(code_gen_query, {code: "..."})        │
│       b. tool_call(web_search, {query: "..."})           │
│       c. 直接生成 content (最终答案)                      │
│    4. 终止条件:                                           │
│       - LLM 给 content → 结束                             │
│       - iteration >= max_iterations → 结束                │
│       - cancellation → 结束                               │
└────────────────────┬─────────────────────┬───────────────┘
                     ↓                     ↓
┌─────────────────────────────┐  ┌──────────────────────┐
│  code_gen_query             │  │  web_search          │
│  Python 沙箱执行 LLM 代码    │  │  调 search API       │
│  SDK 暴露:                   │  │  vertical: web/news  │
│    client.search(...)        │  └──────────────────────┘
│    client.fetch(...)         │
│    client.recall(...)        │
│    client.remember(...)      │
│    client.web_search(...)    │
│  错误信息带回给 LLM           │
└─────────────────────────────┘
                     ↓
┌──────────────────────────────────────────────────────────┐
│  AgentRunResult → SSE events (events.rs, 复用现有变体)   │
│  AgentEvent::Activity / PlanDecision / ToolResult / ...   │
└──────────────────────────────────────────────────────────┘
```

**前端 / 用户视角**：问 → 答，看不到内部循环。
**产品 / 调优视角**：改 YAML + 改 SKILL.md，不动 Rust。
**工程视角**：1 个 loop + 1 个 sandbox + 1 个 web search 工具。

---

## 4. AgentLoop 设计

### 4.1 核心接口

```rust
pub struct AgentLoop {
    llm: Arc<LlmClient>,
    sandbox: Arc<Sandbox>,
    web_search: Arc<dyn SearchProvider>,
}

pub struct ModeConfig {
    pub id: String,                    // "chat" | "rag" | "search"
    pub system_prompt: String,         // 从 SKILL.md 加载
    pub tools: Vec<ToolSpec>,          // 1-2 个 tool schema
    pub budget: BudgetConfig,
    pub on_failure: Option<String>,    // 业务降级文案
}

pub struct AgentRequest {
    pub mode_id: String,
    pub user_query: String,
    pub session_id: Option<String>,
    pub cancellation: CancellationToken,
}

impl AgentLoop {
    pub async fn run(
        &self,
        mode: &ModeConfig,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError>;
}
```

### 4.2 ReAct 循环伪代码

```rust
pub async fn run(
    &self,
    mode: &ModeConfig,
    request: AgentRequest,
    sink: &dyn AgentEventSink,
) -> Result<AgentRunResult, AppError> {
    let mut messages = vec![ChatMessage::system(&mode.system_prompt)];
    messages.push(ChatMessage::user(&request.user_query));
    let mut events: Vec<TraceEvent> = vec![];
    let mut iteration: u8 = 0;
    let mut consecutive_errors: u8 = 0;

    loop {
        // 终止条件
        if iteration >= mode.budget.max_iterations {
            return Ok(self.degrade(mode, "budget_exhausted"));
        }
        if consecutive_errors >= MAX_RETRY_PER_LOOP {
            return Ok(self.degrade(mode, "llm_cannot_recover"));
        }
        request.cancellation.cancelled().await?;

        // 调 LLM
        let checkpoint = TraceEvent::llm_call_start(iteration, &messages);
        events.push(checkpoint.clone());
        sink.emit(checkpoint).await;

        let response = match self.llm
            .complete_with_tools(&messages, &mode.tools, Some(0.7))
            .await
        {
            Ok(r) => r,
            Err(e) if iteration == 0 => return Err(e.into()),  // 第一次失败直接抛
            Err(e) => {
                consecutive_errors += 1;
                tracing::warn!("llm call failed: {e}");
                continue;
            }
        };

        sink.emit(TraceEvent::llm_call_end(iteration, &response)).await;
        events.push(TraceEvent::llm_call_end(iteration, &response));
        consecutive_errors = 0;  // 重置错误计数

        match response.tool_calls {
            Some(calls) if !calls.is_empty() => {
                // LLM 调工具
                messages.push(build_assistant_message_with_tool_calls(&response));

                // 执行工具（可并行）
                let results = self.execute_tools_parallel(&calls).await;

                // 把 tool results 加入历史
                for (call, result) in calls.iter().zip(results.iter()) {
                    messages.push(build_tool_message(call, result));
                    sink.emit(TraceEvent::tool_call(call, result)).await;
                    events.push(TraceEvent::tool_call(call, result));
                }

                iteration += 1;
                continue;  // 继续 ReAct 循环
            }
            _ => {
                // LLM 直接给最终答案
                let content = response.content.unwrap_or_default();
                let result = AgentRunResult { answer: content, ..Default::default() };
                sink.emit(TraceEvent::final_answer(&result)).await;
                return Ok(result);
            }
        }
    }
}
```

### 4.3 终止条件

| 条件 | 行为 |
|---|---|
| LLM 直接生成 content（无 tool_call） | 终止，返回结果 |
| `iteration >= max_iterations` | 终止，返回 `on_failure` 降级文案（若配置） |
| 用户 cancellation | 立即终止，返回 partial result |
| LLM API 错误 / 超时 | 第 1 次失败直接抛；后续失败计入 `consecutive_errors`，达 3 次降级 |
| 连续 3 次错误 | 终止，返回"暂时无法处理" |

### 4.4 取消与超时

- `CancellationToken` 在每次 LLM call 边界检查
- 每次 LLM call 有独立 timeout（默认 60s，可在 mode config 覆盖）
- Sandbox 执行有独立 timeout（默认 30s，超出 kill 进程）

---

## 5. 工具设计

### 5.1 code_gen_query（Python 沙箱）

**作用**：LLM 写 Python 代码，调用 SDK 完成任何"非纯 web 搜索"的动作。

**JSON Schema**：
```json
{
  "name": "code_gen_query",
  "version": "1.0",
  "description": "Execute Python code that calls the avrag SDK to perform retrieval, computation, memory operations, or any other action. Use this when you need to combine multiple operations or perform complex logic.",
  "parameters": {
    "type": "object",
    "properties": {
      "code": {
        "type": "string",
        "description": "Python code. The last expression should be a JSON-serializable list of chunk dicts (or other data) — it will be returned as the tool result."
      },
      "context": {
        "type": "object",
        "description": "Optional. Variables to inject into the Python namespace as JSON-serializable values."
      }
    },
    "required": ["code"]
  }
}
```

**实现**：复用 `crates/rag-core/src/runtime/tools/code_gen_query.rs`（已 production-tested）。

### 5.2 web_search（独立工具）

**作用**：简单关键词搜索（不写 Python，节省沙箱开销）。

**JSON Schema**：
```json
{
  "name": "web_search",
  "version": "1.0",
  "description": "Search the web for up-to-date information. Prefer this for simple keyword queries; use code_gen_query for complex multi-step searches.",
  "parameters": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "Standalone search-engine-ready query."
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

**实现**：复用 `crates/app/src/agents/skills/builtin/web_search.rs`。

### 5.3 SDK 接口（业务动词方法）

LLM 通过 SDK 与后端交互。SDK 是 code_gen_query 沙箱内的 Python 模块。

```python
# avrag_sdk/client.py

class Client:
    """SDK entry point for the LLM to call from inside code_gen_query sandbox."""

    async def search(
        self,
        query: str,
        modality: str = "text",        # "text" | "mm" | "both"
        top_k: int = 10,
        method: str = "auto",          # "auto" | "dense" | "lexical" | "graph"
        doc_ids: list[str] | None = None,
    ) -> list[Chunk]:
        """Search for relevant chunks in workspace documents.

        'method="auto"' lets the system pick the best retrieval method
        based on query characteristics. Specify 'dense' for semantic,
        'lexical' for exact match, 'graph' for entity relationships.
        Returns a list of Chunk objects.
        """

    async def fetch(
        self,
        chunk_id: str,
        include_citations: bool = True,
    ) -> Chunk:
        """Fetch full content of a specific chunk by ID."""

    async def get_doc_summary(
        self,
        doc_ids: list[str],
        level: str = "doc",            # "doc" | "section"
    ) -> list[DocSummary]:
        """Get document-level or section-level summaries."""

    async def get_doc_metadata(
        self,
        doc_ids: list[str],
        fields: list[str] | None = None,
    ) -> list[DocMetadata]:
        """Get document metadata (name, size, mime_type, etc.)."""

    async def recall(
        self,
        tags: list[str] | None = None,
        limit: int = 20,
    ) -> list[Message]:
        """Load previous messages from current conversation.

        Without tags, returns recent messages. With tags, returns messages
        matching those tags. The LLM should call this when it needs to
        resolve anaphora or recall earlier context.
        """

    async def remember(
        self,
        operations: list[dict],
    ) -> dict:
        """Tag messages for future recall.

        operations: [{message_id, action: "add"|"remove"|"replace", tags: []}]
        """

    async def web_search(
        self,
        query: str,
        vertical: str = "web",
    ) -> list[WebResult]:
        """Search the web. (Alternative to the standalone web_search tool.)"""

    async def calculate(self, expression: str) -> float:
        """Evaluate a mathematical expression."""

    async def run_code(self, code: str) -> str:
        """Run arbitrary Python code (use sparingly; security reviewed)."""
```

**SDK 内部实现**：把现有的 `crates/rag-core/src/runtime/tools/*.rs` 和 `crates/app/src/agents/skills/builtin/*.rs` 包装成 async Python 方法。

### 5.4 Chat 模式 0 tool 策略

Chat mode 的 `mode.tools = [code_gen_query]`（仍注册，但不强制用）。

LLM 行为：
- **90%** 请求：直接生成答案，不调任何 tool
- **8%** 请求：调 `code_gen_query` 做计算或简单查询
- **2%** 请求：调 `web_search`（虽然 chat 模式不强调 search，但 LLM 视需要可用）

---

## 6. Role Prompt 设计

### 6.1 3 段式结构

每个 mode 1 个 SKILL.md，由 3 段组成：

```markdown
# [段 1: 角色] 你是 Context OS 的 [chat|rag|search] 助手。[1-2 句定位]

# [段 2: 工具] 你有 N 个工具可用：
- code_gen_query: 写 Python 代码调 SDK（client.search, client.fetch, ...）
- web_search: 简单关键词搜索（如适用）

何时用 code_gen_query：
- [触发条件 1]
- [触发条件 2]

何时直接回答：信息足够时。

# [段 3: ReAct 协议] 思考 → 调工具 → 看结果 → 继续或回答
- [协议规则 1：最大并发]
- [协议规则 2：输出格式]
- [协议规则 3：错误恢复]
```

### 6.2 3 个 mode 的 SKILL.md 模板

**prompts/skills/chat-system/SKILL.md**：
```markdown
# [角色] 你是 Context OS 的对话助手。
# [工具] 你有 1 个工具：
- code_gen_query: 写 Python 代码做计算、查文档、调任何 SDK
# [ReAct] 思考 → 调工具（如需要）→ 回答
```

**prompts/skills/rag-system/SKILL.md**：
```markdown
# [角色] 你是 Context OS 的 RAG 助手。你基于用户上传的文档回答问题。

# [工具] 你有 1 个工具：
- code_gen_query: 写 Python 代码调 SDK

SDK 关键方法：
- client.search(query, method="auto", top_k=10) - 检索文档
- client.fetch(chunk_id) - 拿完整 chunk
- client.recall(tags=["..."]) - 解决指代消解
- client.remember(operations=[...]) - 给历史打标签

何时调：
- 用户问"我的文档里有什么" → client.search
- 用户说"那个呢" / "刚才那个" → client.recall
- 拿到 chunk_ids 但要全文 → client.fetch

何时直接回答：信息足够时。

# [ReAct] 思考 → 调工具 → 看结果 → 继续或回答
- 每次只调 1-3 个工具
- 答案必须带引用：[1] 引用内容
- 不确定时告诉用户，不要编造
```

**prompts/skills/search-system/SKILL.md**：
```markdown
# [角色] 你是 Context OS 的网络搜索助手。你基于实时网络搜索回答问题。

# [工具] 你有 2 个工具：
- code_gen_query: 复杂多步搜索、计算
- web_search: 简单关键词搜索

何时用 web_search：单个 query 即可
何时用 code_gen_query：需要多步编排 / 调多个 SDK

# [ReAct] 思考 → 搜索 → 整合结果 → 回答
- 答案必须带引用：[1] URL + 标题
- 多个搜索结果要交叉验证
```

---

## 7. ReAct 自纠错机制

### 7.1 Sandbox 错误信息设计

LLM 写错 Python 时，sandbox 返回**建设性错误**：

```python
# LLM 写错
chunks = await client.dense(queries=["Atlas"])  # 错误：方法名错了

# Sandbox 错误返回
{
  "error": "AttributeError: 'Client' object has no attribute 'dense'",
  "did_you_mean": ["search", "fetch"],
  "available_methods": ["search", "fetch", "recall", "remember",
                        "web_search", "calculate", "run_code",
                        "get_doc_summary", "get_doc_metadata"]
}
```

**设计原则**：
- 错误信息必须包含**怎么改**（did you mean / available methods）
- 不暴露 stack trace（避免 token 浪费）
- Python 语法错误返回行号

### 7.2 SDK 错误信息设计

```python
# LLM 传错参数
chunks = await client.search(query="Atlas", modality="imagery")

# SDK 错误返回
{
  "error": "InvalidArgument: modality='imagery' is not supported",
  "valid_values": ["text", "mm", "both"],
  "default": "text"
}
```

### 7.3 3 次重试后强制 terminate

```rust
const MAX_RETRY_PER_LOOP: u8 = 3;

if consecutive_errors >= MAX_RETRY_PER_LOOP {
    return Ok(self.degrade(mode, "llm_cannot_recover"));
}
```

降级文案："暂时无法处理您的请求，请换个问法或稍后重试。"

---

## 8. YAML 配置

### 8.1 Schema

```yaml
mode: <string>                # 必填
system_prompt: <path>         # 必填，相对项目根
tools: [<tool_id>, ...]       # 必填，1-2 个
budget:
  max_iterations: <int>       # 必填
  by_user_tier: {             # 可选
    <tier>: <int>
  }
on_failure: <string>          # 可选，业务降级文案
```

### 8.2 3 个 mode 的配置

**modes/chat.yaml**：
```yaml
mode: chat
system_prompt: prompts/skills/chat-system/SKILL.md
tools: [code_gen_query]
budget:
  max_iterations: 2
```

**modes/rag.yaml**：
```yaml
mode: rag
system_prompt: prompts/skills/rag-system/SKILL.md
tools: [code_gen_query]
budget:
  max_iterations: 4
  by_user_tier:
    free: 3
    pro: 6
on_failure: "未找到相关文档，请尝试更换关键词或上传相关文档。"
```

**modes/search.yaml**：
```yaml
mode: search
system_prompt: prompts/skills/search-system/SKILL.md
tools: [code_gen_query, web_search]
budget:
  max_iterations: 3
on_failure: "网络搜索失败，请稍后重试或换个关键词。"
```

**3 个 mode 配置文件总计 ~25 行**。调任何 AI 行为 = 改对应 YAML 或 SKILL.md，不动 Rust。

### 8.3 加载与校验

- 启动时校验所有 YAML 字段（缺字段 / 类型错 = 启动失败）
- `system_prompt` 路径必须存在
- `tools` 中每个 ID 必须在 ToolRegistry 中注册
- `budget.max_iterations` 必须 > 0

---

## 9. 业务降级 vs 硬 Fallback

### 9.1 硬 Fallback（infra 失败）

| 场景 | 行为 |
|---|---|
| LLM API 超时 / 5xx | 第 1 次失败直接抛；后续失败计入 `consecutive_errors`，达 3 次降级 |
| Sandbox 启动失败 | error SSE，提示"内部错误" |
| Web search API 失败 | tool 返回 error message，LLM 看到后决定下一步 |

### 9.2 业务降级（LLM 行为）

| 场景 | 行为 |
|---|---|
| LLM 调 N 次工具仍无结果 | 终止，返回 `on_failure` 文案 |
| LLM 连续 3 次写错 Python / 调错工具 | 终止，返回"暂时无法处理，请换个问法" |
| LLM 在 Chat 模式无意义调工具 | 通过 system prompt 引导 + budget 限制 |

---

## 10. 迁移计划

### 阶段 1：AgentLoop 骨架 + Chat 模式（2 周）

- [ ] 新建 `crates/app/src/agents/loop/mod.rs`（AgentLoop 主体）
- [ ] 实现 ReAct 循环
- [ ] 集成 `code_gen_query` 和 `web_search` 2 个 tool
- [ ] 实现 YAML 加载与校验
- [ ] 加载 `modes/chat.yaml` + `prompts/skills/chat-system/SKILL.md`
- [ ] E2E 测试：Chat 模式纯对话 / Chat 模式调 code_gen_query 计算
- [ ] 旧 `ChatStrategy` 标记 `#[deprecated]`，保留并行

**验证**：`cargo test -p app` 不回归 + 新增测试通过

### 阶段 2：RAG 模式（2 周）

- [ ] SDK 封装 `dense_retrieval` / `lexical_retrieval` / `graph_retrieval` / `index_lookup` / `doc_metadata` / `doc_summary` 为 `client.search` / `client.fetch` / `client.get_doc_summary` / `client.get_doc_metadata`
- [ ] SDK 实现 `client.recall` / `client.remember`（封装 conversation_history_*）
- [ ] 加载 `modes/rag.yaml` + `prompts/skills/rag-system/SKILL.md`
- [ ] 端到端测试：单轮检索 / 多轮检索 / 跨 mode 切 Chat→RAG（用 `client.recall`）
- [ ] 旧 `RagStrategy` 标记 `#[deprecated]`

**验证**：现有 RAG 集成测试通过 + 新 SDK 调用路径测试通过

### 阶段 3：Search 模式（1 周）

- [ ] SDK 实现 `client.web_search`（封装 web_search tool）
- [ ] 加载 `modes/search.yaml` + `prompts/skills/search-system/SKILL.md`
- [ ] 端到端测试
- [ ] 旧 `SearchStrategy` 标记 `#[deprecated]`

**验证**：现有 Search 集成测试通过

### 阶段 4：replay / eval 适配 + 旧代码删除（1-2 周）

- [ ] `replay.rs` 改造：从 `state_history` 改为 `TraceEvent` 重放
- [ ] `eval_framework.rs` 改造：在 checkpoint 边界评估
- [ ] 删除 `crates/app/src/agents/strategy/{chat,rag,search}.rs`（约 159KB）
- [ ] 删除 `crates/app/src/agents/strategy/{executor,prompts,mod}.rs`（state machine 相关）
- [ ] 更新 E2E 测试
- [ ] 更新文档

**验证**：`cargo test -p app --lib` 全绿 + E2E 测试通过 + `cargo clippy` 干净

**总估算**：6-8 周

---

## 11. 测试策略

### 11.1 单元测试

- `AgentLoop::run` 的终止条件（max_iterations / cancellation / final answer）
- SDK 错误信息格式（"did you mean" 准确性）
- YAML 加载与校验
- Replay / Eval event 流生成

### 11.2 集成测试（Mock LLM + Mock SDK）

| 测试名 | 目标 |
|---|---|
| `test_chat_pure_dialogue` | Chat 模式不调任何 tool，直接答 |
| `test_chat_calls_code_gen_query_for_math` | Chat 模式调 code_gen_query 计算 |
| `test_rag_single_search` | RAG 模式 1 轮 search 后答 |
| `test_rag_multi_search_with_retry` | RAG 模式多轮 search（ReAct 自纠错） |
| `test_rag_recall_for_anaphora` | RAG 模式调 `client.recall` 解决指代 |
| `test_rag_auto_method_dispatch` | RAG 模式 `method="auto"` 时系统选最佳检索 |
| `test_search_combines_code_and_web` | Search 模式混合用 code_gen_query + web_search |
| `test_budget_exhaustion` | 超过 max_iterations 触发 on_failure 降级 |
| `test_cancellation_mid_loop` | 取消 token 触发时立即终止 |
| `test_sdk_error_self_correction` | SDK 报错 → LLM 在下一轮修正 |
| `test_consecutive_errors_force_terminate` | 连续 3 次错误后强制降级 |
| `test_yaml_validation_rejects_missing_field` | YAML 缺字段启动失败 |

### 11.3 E2E 测试

- 前端 E2E 测试已迁移到 `frontend_next/e2e/`
- 验证 3 个 mode 的端到端行为
- 验证 SSE 事件流

---

## 12. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| **Sandbox 性能差** | 中 | 每次 tool call +100-300ms | 预热沙箱；常用 query 模板缓存 |
| **LLM 写错 Python 概率高** | 中 | 多轮重试，token 浪费 | 优秀 system prompt + few-shot 示例；SDK 错误信息优化 |
| **未来加新工具要更新 SDK** | 低 | SDK 维护成本 | SDK 模块化；新工具 = 1 个 Python 方法 |
| **审计 / replay 难度** | 中 | 调试复杂 | TraceEvent 完整记录；eval 在 checkpoint 评估 |
| **442 个 lib tests 大部分要改** | 高 | 测试迁移工作量大 | 阶段 4 集中迁移；优先保证核心 e2e |
| **ReAct 循环死循环** | 低 | 用户卡死 | 3 次错误后强制 terminate + max_iterations |
| **format skill 集成问题** | 中 | html / ppt 输出可能失效 | 阶段 1 验证；通过 system prompt 加格式指引 |
| **Chat 模式 system prompt 不够强** | 中 | LLM 误调工具浪费 token | 阶段 1 强化 system prompt；few-shot 示例 |

---

## 13. 影响与后果

### 13.1 正面影响

- **代码大幅减少**：删除 ~159KB 旧 Strategy 代码，新增 ~50KB 新 AgentLoop
- **测试集中**：Mock 集中在 1 个 AgentLoop，覆盖率更高
- **调优零代码**：产品 / 运营改 YAML + SKILL.md 即可
- **加新 mode 简单**：1 份 YAML + 1 个 SKILL.md，~3 天
- **未来加工具简单**：SDK 加 1 个方法，LLM 自动会用
- **LLM 行为统一**：3 个 mode 走同一 loop，行为一致性好
- **渐进式披露保留**：SDK 内部 `method="auto"` 仍是渐进式披露
- **指代消解保留**：通过 `client.recall` / `client.remember` 实现

### 13.2 负面影响

- **初期迁移成本**：6-8 周期间新旧代码并存
- **Sandbox 依赖**：所有 RAG 行为都依赖 code_gen_query，沙箱性能成为瓶颈
- **LLM 写代码 vs 写 tool_call**：token 成本 +30-50%（schema 节省抵消一部分）
- **审计粒度变化**：从"state 边界"改为"event 流"，旧 replay 测试需重写

### 13.3 兼容性

- **前端协议不变**：`POST /chat` 的 `agent_type` 字段语义不变
- **数据库 schema 不变**：`chat_messages` / `message_tags` 表不变
- **SSE 事件名不变**：复用现有 `AgentEvent` 枚举变体
- **Tool schema 变更**：`dense_retrieval` 等 13 个 tool 不再直接暴露给 LLM；只暴露 `code_gen_query` + `web_search` 2 个

---

## 14. 已否决的替代方案（回顾）

| 方案 | 否决理由 |
|---|---|
| 保留 v5 状态机 | 循环重复、工具碎片、调优要改代码 |
| 4 阶段 + 14 工具 | 阶段划分不必要；14 工具 = LLM 认知负担 |
| 合并到 5 工具 | 仍需为每个工具写 schema；不如用 SDK 统一 |
| 1 tool（仅 code_gen_query） | web_search 简单关键词场景太常见，强制走 Python 沙箱有性能开销 |
| 引入 LangGraph / Rig | 增加外部依赖；社区成熟度不足；当前需求 ReAct 足够 |

---

## 15. 开放问题

### 15.1 SDK 版本管理

- SDK 接口变更是否需要向后兼容？
- 提议：SDK 加 `version` 字段，sandbox 检查版本匹配

### 15.2 Sandbox 安全

- code_gen_query 沙箱目前对 Python 有限制
- 是否需要更严格的 resource limit（CPU / memory / network）？
- 提议：阶段 4 评估

### 15.3 LLM Provider 切换

- 当前主要用 OpenAI 协议
- 是否要支持 Anthropic / 本地模型？
- 提议：暂不处理，作为未来工作

### 15.4 Format Skill 集成

- 现有 html-renderer / ppt-generation 等 format skill
- 是否要让 LLM 通过 SDK 调用？
- 提议：v1 不集成，保持 skill body 形式（在 system prompt 里加格式指引）

### 15.5 Sandbox 错误信息语言

- 当前设计错误信息用英文（"did you mean"）
- LLM 是否在中文 prompt 下能正确解析英文错误？
- 提议：阶段 1 验证，必要时支持中英双语错误信息

---

## 16. 参考文档

- ADR-0003: v5 Agent Architecture（被本 ADR 取代）
- ADR-0004: RAG Agent Loop with Native Tool Calling（设计思想保留：ReAct 风格循环）
- ADR-0005: Unified Agent Kernel（已否决）
- ADR-0005-revised: 基于 v5 的增量扩展（已否决）
- `docs/agents/progressive-disclosure-framework.md`（原则 1 的原始设计文档）
- `docs/superpowers/specs/2026-05-26-writing-style-conversation-memory-design.md`（原则 2 的原始设计文档）
- `crates/app/src/agents/unified/mod.rs`
- `crates/rag-core/src/runtime/tools/code_gen_query.rs`
- `crates/app/src/agents/skills/builtin/web_search.rs`
- `prompts/skills/` —— 现有 SKILL.md 体系

---

*本文档基于多轮讨论生成：14 → 5 → 1+1 工具收敛，4 阶段 → ReAct 单一循环，3 role → 1 system prompt。最终架构以"1 个 AgentLoop + 1+1 工具 + ReAct 单一循环 + 1 个 system prompt"为决策核心。*
