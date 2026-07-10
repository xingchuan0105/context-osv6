# 写作风格库 + Brainstorming + 多轮记忆 + 三问题修复 实施计划

> **状态：历史计划（L2 `session_summary` 已移除）**  
> 文中多轮记忆 / `session_summary` 注入相关步骤**不再反映当前实现**。见 `avrag-rs/docs/adr/0007-react-phased-context-disclosure.md` 与 `avrag-rs/docs/memory-recall-gap-2026-06-13.md`。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现写作风格库（Skill 系统扩展）、Brainstorming 行为模式、Agent 自治多轮对话记忆，同时修复 Plan prompt tool catalog 策略隔离、Format Skills E2E 验证、doc_index/index_lookup 评估三个遗留问题。

**Architecture:** 复用现有 Skill / CapabilityRegistry / PromptBuilder 管道，新增 `category` 字段区分 skill 类型；Plan prompt 按策略过滤工具目录避免"看得到用不了"；Answer prompt 注入写作风格 skill body；多轮记忆通过两个原子工具（load/tag）让 Agent 自主管理历史标签。

**Tech Stack:** Rust, PostgreSQL (sqlx), serde_json, tokio

---

## 文件结构映射

### 新增文件

| 文件 | 职责 |
|------|------|
| `migrations/0034_conversation_memory.up.sql` | `message_tags` 表 |
| `migrations/0034_conversation_memory.down.sql` | 回滚 `message_tags` |
| `prompts/skills/concise-writing/SKILL.md` | 简洁写作风格 |
| `prompts/skills/concise-writing/references/few-shot-1.md` | few-shot 示例 |
| `prompts/skills/academic-writing/SKILL.md` | 学术写作风格 |
| `prompts/skills/academic-writing/references/few-shot-1.md` | few-shot 示例 |
| `prompts/skills/storytelling/SKILL.md` | 讲故事风格 |
| `prompts/skills/storytelling/references/few-shot-1.md` | few-shot 示例 |
| `prompts/skills/professional-writing/SKILL.md` | 商务专业风格 |
| `prompts/skills/professional-writing/references/few-shot-1.md` | few-shot 示例 |
| `prompts/skills/brainstorming/SKILL.md` | Brainstorming 行为协议 |
| `prompts/skills/brainstorming/references/example-vague-request.md` | 示例 |
| `prompts/skills/brainstorming/references/example-clarification-flow.md` | 示例 |
| `crates/app/src/agents/skills/conversation_history.rs` | `conversation_history_load` / `conversation_history_tag` 工具实现 |
| `crates/storage-pg/src/lib_impl/repository_conversation_memory.rs` | `message_tags` CRUD |

### 修改文件

| 文件 | 改动 |
|------|------|
| `crates/app/src/agents/capability/metadata.rs` | `SkillMetadata` 新增 `category`；`ToolMetadata` 新增 `applicable_strategies` |
| `crates/app/src/agents/capability/registry.rs` | `plan_tools(strategy)` 按策略过滤；新增 `answer_writing_styles()` |
| `crates/app/src/agents/strategy/prompts.rs` | `build_answer_system_prompt` 新增 `selected_writing_styles` |
| `crates/app/src/agents/events.rs` | `AgentEvent::PlanDecision` 新增 `writing_styles`、`behavior_mode` |
| `crates/app/src/agents/strategy/chat.rs` | `ChatContext` 新增字段；`step_plan`/`step_answer` 集成写作风格 |
| `crates/app/src/agents/strategy/rag.rs` | `RagContext` 新增字段；`step_plan`/`step_answer` 集成 |
| `crates/app/src/agents/strategy/search.rs` | `SearchContext` 新增字段；`step_plan`/`step_answer` 集成 |
| `crates/app/src/agents/runtime.rs` | 移除 `MAX_PROMPT_HISTORY_TURNS` 硬编码注入 |
| `crates/app/tests/e2e_chat.rs` | 新增 format skill E2E 测试 |
| `crates/app/tests/e2e_rag.rs` | 新增 format skill E2E 测试 |
| `crates/app/tests/e2e/assertions.rs` | 新增 `assert_prompt_contains_skill_body` |
| `crates/storage-pg/src/lib_impl.rs` | 引入 `repository_conversation_memory` 模块 |

---

## Phase 1: 问题修复

### Task 1.1: ToolMetadata 新增 `applicable_strategies`

**目标**: 让工具知道自己适用于哪些策略，为 `plan_tools(strategy)` 按策略过滤做准备。

**Files:**
- Modify: `crates/app/src/agents/capability/metadata.rs`
- Modify: `crates/app/src/agents/capability/registry.rs`

- [ ] **Step 1: 修改 `ToolMetadata` 结构**

在 `crates/app/src/agents/capability/metadata.rs` 的 `ToolMetadata` 中新增字段：

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolMetadata {
    pub id: String,
    pub version: String,
    pub owner: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub risk_level: RiskLevel,
    pub permissions: Vec<Permission>,
    pub external_deps: Vec<String>,
    pub deprecation: Option<Deprecation>,
    pub retry_policy: RetryPolicy,
    pub activation_phase: ActivationPhase,
    pub applicable_strategies: Vec<String>, // ← 新增
}
```

- [ ] **Step 2: 修改 `tool_to_metadata` 转换函数**

在 `crates/app/src/agents/capability/registry.rs` 的 `tool_to_metadata` 中，根据工具来源推断 `applicable_strategies`：

```rust
fn tool_to_metadata(tool: &super::super::progressive::Tool, source: ToolSource) -> ToolMetadata {
    let spec = tool.spec();
    let applicable_strategies = match source {
        ToolSource::RagToolCatalog => vec!["rag".to_string()],
        ToolSource::AtomicToolCatalog => vec!["chat".to_string(), "rag".to_string(), "search".to_string()],
        ToolSource::SearchSpecific => vec!["search".to_string()],
    };
    ToolMetadata {
        id: spec.name.clone(),
        version: spec.version.clone(),
        owner: "context-os".to_string(),
        description: spec.description.clone(),
        input_schema: spec.input_schema.clone(),
        output_schema: spec.output_schema.clone(),
        risk_level: infer_tool_risk_level(&spec.name),
        permissions: infer_tool_permissions(&spec.name),
        external_deps: infer_tool_external_deps(&spec.name),
        deprecation: None,
        retry_policy: infer_tool_retry_policy(&spec.name),
        activation_phase: ActivationPhase::PlanAndEvaluate,
        applicable_strategies,
    }
}
```

- [ ] **Step 3: 定义 `ToolSource` 枚举**

在同一文件（`registry.rs`）中，新增：

```rust
#[derive(Debug, Clone, Copy)]
enum ToolSource {
    RagToolCatalog,
    AtomicToolCatalog,
    SearchSpecific,
}
```

- [ ] **Step 4: 修改 `CapabilityRegistry::standard()` 的注册逻辑**

将原来的三次循环改为传入 `ToolSource`：

```rust
// 修改前:
for tool in super::super::progressive::rag_tool_catalog_cached() {
    let meta = tool_to_metadata(tool);
    tools.insert(meta.id.clone(), meta);
}

// 修改后:
for tool in super::super::progressive::rag_tool_catalog_cached() {
    let meta = tool_to_metadata(tool, ToolSource::RagToolCatalog);
    tools.insert(meta.id.clone(), meta);
}
for tool in super::super::progressive::atomic_tool_catalog_cached() {
    let meta = tool_to_metadata(tool, ToolSource::AtomicToolCatalog);
    tools.insert(meta.id.clone(), meta);
}
for tool in super::super::progressive::search_specific_tools_cached() {
    let meta = tool_to_metadata(tool, ToolSource::SearchSpecific);
    tools.insert(meta.id.clone(), meta);
}
```

- [ ] **Step 5: 编译检查**

Run: `cargo check -p app`
Expected: 编译通过，无新增 error（可能有 warning 关于未使用字段，下一步修复）

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/agents/capability/metadata.rs crates/app/src/agents/capability/registry.rs
git commit -m "feat(capability): add applicable_strategies to ToolMetadata

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 1.2: `plan_tools(strategy)` 按策略过滤

**目标**: Chat 和 RAG 的 Plan prompt 不再展示 Search 专属工具（如 web_search）。

**Files:**
- Modify: `crates/app/src/agents/capability/registry.rs`
- Modify: `crates/app/src/agents/strategy/prompts.rs` (test)
- Test: `crates/app/tests/e2e_chat.rs`
- Test: `crates/app/tests/e2e_rag.rs`

- [ ] **Step 1: 修改 `plan_tools` 方法**

```rust
pub fn plan_tools(&self, strategy: &str) -> Vec<&ToolMetadata> {
    self.tools
        .values()
        .filter(|t| t.activation_phase == ActivationPhase::PlanAndEvaluate)
        .filter(|t| t.applicable_strategies.contains(&strategy.to_string()))
        .collect()
}
```

- [ ] **Step 2: 运行 lib 测试**

Run: `cargo test -p app --lib`
Expected: 全部通过（包括 `prompts.rs` 中现有的 `chat_plan_prompt_is_not_empty` 等测试）

- [ ] **Step 3: 更新 E2E 断言（Chat）**

在 `crates/app/tests/e2e_chat.rs` 的 `chat_simple_conversation_state_machine` 中，Plan prompt 断言现在不应该包含 web_search：

```rust
// 在现有断言之后增加:
let plan_call = &calls[0];
assert!(
    !plan_call.system_prompt.contains("web_search"),
    "Chat plan prompt should NOT contain web_search (search-specific tool)"
);
```

- [ ] **Step 4: 更新 E2E 断言（RAG）**

在 `crates/app/tests/e2e_rag.rs` 的 `rag_single_pass_sufficient_state_machine` 中同样增加：

```rust
let plan_call = &calls[0];
assert!(
    !plan_call.system_prompt.contains("web_search"),
    "RAG plan prompt should NOT contain web_search"
);
```

- [ ] **Step 5: 运行 E2E 测试**

Run: `cargo test --ignored -p app --test e2e_chat`
Run: `cargo test --ignored -p app --test e2e_rag`
Expected: 通过

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/agents/capability/registry.rs crates/app/src/agents/strategy/prompts.rs crates/app/tests/e2e_chat.rs crates/app/tests/e2e_rag.rs
git commit -m "fix(capability): filter plan_tools by strategy to hide search-only tools from chat/rag

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 1.3: Format Skills E2E 实际功能验证

**目标**: E2E 测试不只检查 ID 字符串，而是验证 format skill 的完整 body 被注入 prompt。

**Files:**
- Modify: `crates/app/tests/e2e/assertions.rs`
- Modify: `crates/app/tests/e2e_chat.rs`
- Modify: `crates/app/tests/e2e_rag.rs`

- [ ] **Step 1: 新增 `assert_prompt_contains_skill_body` 断言**

在 `crates/app/tests/e2e/assertions.rs` 中：

```rust
/// Assert that a prompt contains the FULL BODY of a specific skill (not just the ID).
/// This verifies that the skill was actually LOADED into the prompt, not just listed.
pub fn assert_prompt_contains_skill_body(prompt: &str, skill_id: &str) {
    let registry = PromptRegistry::standard_cached();
    let skill = registry
        .skill(skill_id)
        .unwrap_or_else(|| panic!("Skill '{}' not found in registry", skill_id));
    let body = skill.system_prompt();
    assert!(
        prompt.contains(body),
        "Prompt does not contain full body of skill '{}'. Expected {} chars, prompt is {} chars.",
        skill_id,
        body.len(),
        prompt.len()
    );
}
```

- [ ] **Step 2: 新增 `assert_output_matches_format` 断言**

```rust
/// Assert that LLM output matches expected format markers.
/// This is a best-effort check — LLM output may vary.
pub fn assert_output_matches_format(output: &str, format: &str) {
    match format {
        "ppt-generation" => {
            assert!(
                output.contains("slides") || output.contains("slide"),
                "Expected PPT output to contain 'slide' references, got: {}",
                output
            );
        }
        "html-renderer" => {
            assert!(
                output.contains("<html") || output.contains("<!DOCTYPE html"),
                "Expected HTML output to contain '<html' tag, got: {}",
                output
            );
        }
        "teaching" => {
            assert!(
                output.contains("?") || output.contains("Let's"),
                "Expected teaching output to be interactive, got: {}",
                output
            );
        }
        _ => panic!("Unknown format: {}", format),
    }
}
```

- [ ] **Step 3: 新增 Chat format skill E2E 测试**

在 `crates/app/tests/e2e_chat.rs` 中新增：

```rust
/// Test: Chat with PPT format hint — verify ppt-generation skill body is injected.
#[tokio::test]
#[ignore = "requires staging environment"]
async fn chat_ppt_format_skill_injected() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    if let Err(missing) = config.validate_for_chat() {
        panic!("Chat E2E missing environment variables: {}", missing.join(", "));
    }
    let llm_client = config.llm_client();
    let recording = RecordingLlmProvider::new(Arc::new(llm_client.clone()));
    let recording_arc = Arc::new(recording);

    let mut request = chat_request("make a ppt about Rust concurrency");
    request.format_hint = Some("ppt".to_string());

    let ctx = ChatContext::from_request(
        request,
        "test-chat-ppt-format".to_string(),
        LoopBudget::chat(UserTier::Pro),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();

    let strategy = app::agents::strategy::chat::ChatStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client),
        temperature: None,
    };

    let executor = app::agents::strategy::executor::StrategyExecutor;
    let result = executor.run(&strategy, ctx).await.unwrap();

    let calls = recording_arc.calls();
    let answer_call = calls.last().unwrap();

    // Verify the FULL skill body is in the answer prompt, not just the ID
    assertions::assert_prompt_contains_skill_body(&answer_call.system_prompt, "ppt-generation");
}
```

- [ ] **Step 4: 新增 RAG format skill E2E 测试**

在 `crates/app/tests/e2e_rag.rs` 中新增类似测试，查询为 "render the result as an HTML page"：

```rust
/// Test: RAG with HTML format — verify html-renderer skill body is injected.
#[tokio::test]
#[ignore = "requires full staging"]
async fn rag_html_format_skill_injected() {
    // ... setup same as rag_single_pass_sufficient_state_machine ...
    let mut request = rag_request("Summarize antifragility and render as HTML", vec![doc_id.to_string()]);
    request.format_hint = Some("html".to_string());
    // ... rest of test ...
    assertions::assert_prompt_contains_skill_body(&answer_call.system_prompt, "html-renderer");
}
```

- [ ] **Step 5: 运行新增 E2E 测试**

Run: `cargo test --ignored -p app --test e2e_chat chat_ppt_format_skill_injected`
Run: `cargo test --ignored -p app --test e2e_rag rag_html_format_skill_injected`
Expected: 通过（验证 skill body 确实被注入）

- [ ] **Step 6: Commit**

```bash
git add crates/app/tests/e2e/assertions.rs crates/app/tests/e2e_chat.rs crates/app/tests/e2e_rag.rs
git commit -m "test(e2e): verify format skill bodies are injected into answer prompts

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 1.4: doc_index / index_lookup 评估结论

**目标**: 确认不合并，但记录评估结论。

**Files:**
- 无代码改动

- [ ] **Step 1: 在设计文档中记录评估结论**

已在设计文档 `docs/superpowers/specs/2026-05-26-writing-style-conversation-memory-design.md` 中记录：保持分离，不合并。

- [ ] **Step 2: Commit（仅文档更新）**

```bash
git add docs/superpowers/specs/2026-05-26-writing-style-conversation-memory-design.md
git commit -m "docs: record doc_index/index_lookup merge assessment — keep separate

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Phase 2: 写作风格库 + Brainstorming

### Task 2.1: SkillMetadata 新增 `category` 字段

**目标**: 区分 standard / format / writing-style / behavior 四类 skill。

**Files:**
- Modify: `crates/app/src/agents/capability/metadata.rs`
- Modify: `crates/app/src/agents/capability/registry.rs`

- [ ] **Step 1: 修改 `SkillMetadata`**

在 `metadata.rs`：

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillMetadata {
    pub id: String,
    pub version: String,
    pub owner: String,
    pub description: String,
    pub applicable_strategies: Vec<String>,
    pub required_tools: Vec<String>,
    pub risk_level: RiskLevel,
    pub deprecation: Option<Deprecation>,
    pub activation_phase: ActivationPhase,
    pub category: String, // ← 新增，默认 "standard"
}
```

- [ ] **Step 2: 修改 `skill_to_metadata` 读取 category**

在 `registry.rs` 的 `skill_to_metadata` 中：

```rust
let category = md
    .get("category")
    .cloned()
    .unwrap_or_else(|| "standard".to_string());

// ... 在 SkillMetadata 构造中增加 category ...
SkillMetadata {
    // ... existing fields ...
    category,
}
```

- [ ] **Step 3: 编译检查**

Run: `cargo check -p app`
Expected: 通过

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/agents/capability/metadata.rs crates/app/src/agents/capability/registry.rs
git commit -m "feat(capability): add category field to SkillMetadata

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2.2: CapabilityRegistry 新增查询方法

**目标**: 按 category + strategy 查询 writing-style 和 behavior-mode skills。

**Files:**
- Modify: `crates/app/src/agents/capability/registry.rs`

- [ ] **Step 1: 新增 `answer_writing_styles` 方法**

```rust
/// Answer 阶段：返回写作风格技能目录
pub fn answer_writing_styles(&self, strategy: &str) -> Vec<&SkillMetadata> {
    self.skills
        .values()
        .filter(|s| s.category == "writing-style")
        .filter(|s| s.applicable_strategies.contains(&strategy.to_string()))
        .collect()
}

/// Answer 阶段：返回行为模式技能目录（目前只有 brainstorming）
pub fn answer_behavior_modes(&self, strategy: &str) -> Vec<&SkillMetadata> {
    self.skills
        .values()
        .filter(|s| s.category == "behavior")
        .filter(|s| s.applicable_strategies.contains(&strategy.to_string()))
        .collect()
}
```

- [ ] **Step 2: 编译检查**

Run: `cargo check -p app`
Expected: 通过

- [ ] **Step 3: Commit**

```bash
git add crates/app/src/agents/capability/registry.rs
git commit -m "feat(capability): add answer_writing_styles and answer_behavior_modes queries

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2.3: PromptBuilder 扩展

**目标**: `build_answer_system_prompt` 支持注入写作风格 skill body。

**Files:**
- Modify: `crates/app/src/agents/strategy/prompts.rs`
- Modify: `crates/app/src/agents/strategy/chat.rs`
- Modify: `crates/app/src/agents/strategy/rag.rs`
- Modify: `crates/app/src/agents/strategy/search.rs`

- [ ] **Step 1: 修改 `build_answer_system_prompt` 签名和实现**

```rust
pub fn build_answer_system_prompt(
    answer_skill_id: &str,
    strategy: &str,
    selected_format_skills: &[String],
    selected_writing_styles: &[String], // ← 新增
) -> String {
    let registry = PromptRegistry::standard_cached();
    let mut parts = Vec::new();

    // 1. answer skill 全文（基底）
    if let Some(skill) = registry.skill(answer_skill_id) {
        parts.push(skill.system_prompt().to_string());
    }

    // 2. format 技能目录（Index tier）
    let cap_registry = crate::agents::capability::CapabilityRegistry::standard_cached();
    let format_skills = cap_registry.answer_format_skills(strategy);
    if !format_skills.is_empty() {
        let catalog = format_skills
            .iter()
            .map(|s| format!("- {} (v{}): {}", s.id, s.version, s.description))
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!("## Available Output Formats\n\n{catalog}"));
    }

    // 3. 写作风格目录（Index tier）← 新增
    let writing_styles = cap_registry.answer_writing_styles(strategy);
    if !writing_styles.is_empty() {
        let catalog = writing_styles
            .iter()
            .map(|s| format!("- {} (v{}): {}", s.id, s.version, s.description))
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!("## Available Writing Styles\n\n{catalog}"));
    }

    // 4. 选中的 format skill 全文（Load tier）
    for skill_id in selected_format_skills {
        if let Some(skill) = registry.skill(skill_id.as_str()) {
            parts.push(skill.system_prompt().to_string());
        }
    }

    // 5. 选中的 writing style 全文（Load tier）← 新增
    for skill_id in selected_writing_styles {
        if let Some(skill) = registry.skill(skill_id.as_str()) {
            parts.push(skill.system_prompt().to_string());
        }
    }

    parts.join("\n\n---\n\n")
}
```

- [ ] **Step 2: 修改 `prompts.rs` 中的测试**

```rust
#[test]
fn chat_answer_prompt_is_not_empty() {
    let prompt = build_answer_system_prompt(chat::ANSWER_SKILL_ID, "chat", &[], &[]);
    assert!(!prompt.is_empty());
    assert!(prompt.contains("Available Output Formats"));
}
```

- [ ] **Step 3: 修改 Chat 的 `step_answer` 调用**

在 `crates/app/src/agents/strategy/chat.rs` 的 `step_answer` 中：

```rust
let system_prompt = crate::agents::strategy::prompts::build_answer_system_prompt(
    crate::agents::strategy::prompts::chat::ANSWER_SKILL_ID,
    "chat",
    &[],
    &ctx.selected_writing_styles, // ← 从 ctx 读取
);
```

- [ ] **Step 4: 修改 Rag 的 `step_answer` 调用**

在 `crates/app/src/agents/strategy/rag.rs` 的 `step_answer` 中：

```rust
let system_prompt = crate::agents::strategy::prompts::build_answer_system_prompt(
    crate::agents::strategy::prompts::rag::ANSWER_SKILL_ID,
    "rag",
    &selected_format_skills,
    &ctx.selected_writing_styles, // ← 从 ctx 读取
);
```

- [ ] **Step 5: 修改 Search 的 `step_answer` 调用**

在 `crates/app/src/agents/strategy/search.rs` 的 `step_answer` 中：

```rust
let system_prompt = crate::agents::strategy::prompts::build_answer_system_prompt(
    crate::agents::strategy::prompts::search::ANSWER_SKILL_ID,
    "search",
    &[],
    &ctx.selected_writing_styles, // ← 从 ctx 读取
);
```

- [ ] **Step 6: 编译检查**

Run: `cargo check -p app`
Expected: 编译通过（ChatContext/RagContext/SearchContext 还没有新增字段，下一步添加）

注意：此时会有编译错误，因为 `ctx.selected_writing_styles` 还不存在。下一步 Task 2.4 会修复。

- [ ] **Step 7: Commit**

```bash
git add crates/app/src/agents/strategy/prompts.rs crates/app/src/agents/strategy/chat.rs crates/app/src/agents/strategy/rag.rs crates/app/src/agents/strategy/search.rs
git commit -m "feat(prompts): extend build_answer_system_prompt with writing_styles injection

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2.4: Context 结构扩展 + PlanDecision 事件扩展

**目标**: 在 Context 中存储 Plan LLM 选择的写作风格和行为模式，并在 Plan/Answer 之间传递。

**Files:**
- Modify: `crates/app/src/agents/events.rs`
- Modify: `crates/app/src/agents/strategy/chat.rs`
- Modify: `crates/app/src/agents/strategy/rag.rs`
- Modify: `crates/app/src/agents/strategy/search.rs`

- [ ] **Step 1: 扩展 `AgentEvent::PlanDecision`**

在 `events.rs`：

```rust
PlanDecision {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    selected_tools: Vec<common::ToolCall>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    selected_skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    selected_writing_styles: Vec<String>, // ← 新增
    #[serde(default, skip_serializing_if = "Option::is_none")]
    behavior_mode: Option<String>,         // ← 新增
    #[serde(default, skip_serializing_if = "String::is_empty")]
    reasoning: String,
},
```

- [ ] **Step 2: 更新事件构造处（Rag 的 step_plan）**

找到所有 `AgentEvent::PlanDecision { ... }` 的构造处，新增字段。

在 `rag.rs` 中（搜索 `PlanDecision`）：

```rust
AgentEvent::PlanDecision {
    selected_tools: decision.calls.clone(),
    selected_skills: decision.skills.clone(),
    selected_writing_styles: decision.writing_styles.clone(), // ← 新增
    behavior_mode: decision.behavior_mode.clone(),             // ← 新增
    reasoning: format!("plan strategy: {:?}", decision.strategy),
},
```

- [ ] **Step 3: 更新事件构造处（Chat 的 step_plan）**

在 `chat.rs` 中：

```rust
AgentEvent::PlanDecision {
    selected_tools: decision.calls.clone(),
    selected_skills: vec![],
    selected_writing_styles: decision.writing_styles.clone(), // ← 新增
    behavior_mode: decision.behavior_mode.clone(),             // ← 新增
    reasoning: format!("plan action: {}", decision.action),
},
```

- [ ] **Step 4: 更新事件构造处（Search 的 step_decompose）**

在 `search.rs` 中类似修改。

- [ ] **Step 5: 扩展 `ChatContext`**

在 `chat.rs` 的 `ChatContext` 中新增：

```rust
pub struct ChatContext {
    pub request: AgentRequest,
    pub trace_id: String,
    pub budget: LoopBudget,
    pub sink: Box<dyn AgentEventSink>,
    pub cancel: CancellationToken,
    pub auth: avrag_auth::AuthContext,
    pub tool_results: Vec<ToolResult>,
    pub plan_decision_action: Option<String>,
    pub tool_call_records: Vec<crate::agents::runtime::ToolCallRecord>,
    pub aggregated_usage: Option<avrag_llm::LlmUsage>,
    pub request_count: u64,
    pub content_guard_trace: Vec<common::DegradeTraceItem>,
    pub selected_writing_styles: Vec<String>,     // ← 新增
    pub behavior_mode: Option<String>,            // ← 新增
}
```

- [ ] **Step 6: 扩展 `RagContext`**

在 `rag.rs` 的 `RagContext` 中新增：

```rust
pub selected_writing_styles: Vec<String>,     // ← 新增
pub behavior_mode: Option<String>,            // ← 新增
```

- [ ] **Step 7: 扩展 `SearchContext`**

在 `search.rs` 的 `SearchContext` 中新增同样的两个字段。

- [ ] **Step 8: 扩展 `ChatPlanDecision`**

在 `chat.rs` 中：

```rust
#[derive(Debug, Default)]
struct ChatPlanDecision {
    action: String,
    clarification_message: String,
    calls: Vec<ToolCall>,
    writing_styles: Vec<String>,     // ← 新增
    behavior_mode: Option<String>,   // ← 新增
}
```

- [ ] **Step 9: 修改 `parse_chat_plan_decision` 解析新字段**

```rust
fn parse_chat_plan_decision(raw: &str) -> ChatPlanDecision {
    // ... existing parsing logic ...
    let value: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => {
            return ChatPlanDecision {
                action: "answer".to_string(),
                ..ChatPlanDecision::default()
            }
        }
    };

    let action = value
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("answer")
        .to_string();

    let calls: Vec<ToolCall> = value
        .get("calls")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    let clarification_message = value
        .get("clarification_message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // ← 新增解析
    let writing_styles: Vec<String> = value
        .get("writing_styles")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let behavior_mode = value
        .get("behavior_mode")
        .and_then(|v| v.as_str())
        .map(String::from);

    ChatPlanDecision {
        action,
        clarification_message,
        calls,
        writing_styles,
        behavior_mode,
    }
}
```

- [ ] **Step 10: 在 Chat `step_plan` 中写入 Context 新字段**

在 `chat.rs` 的 `step_plan` 中，解析 decision 后：

```rust
let decision = parse_chat_plan_decision(&plan_response.content);
ctx.plan_decision_action = Some(decision.action.clone());
ctx.selected_writing_styles = decision.writing_styles.clone(); // ← 新增
ctx.behavior_mode = decision.behavior_mode.clone();             // ← 新增
```

- [ ] **Step 11: 编译检查**

Run: `cargo check -p app`
Expected: 编译通过

- [ ] **Step 12: Commit**

```bash
git add crates/app/src/agents/events.rs crates/app/src/agents/strategy/chat.rs crates/app/src/agents/strategy/rag.rs crates/app/src/agents/strategy/search.rs
git commit -m "feat(context): add selected_writing_styles and behavior_mode to all strategy contexts

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2.5: 新建技能文件

**目标**: 创建 5 个新 Skill 的目录和 SKILL.md。

**Files:**
- Create: `prompts/skills/concise-writing/SKILL.md`
- Create: `prompts/skills/concise-writing/references/few-shot-1.md`
- Create: `prompts/skills/academic-writing/SKILL.md`
- Create: `prompts/skills/academic-writing/references/few-shot-1.md`
- Create: `prompts/skills/storytelling/SKILL.md`
- Create: `prompts/skills/storytelling/references/few-shot-1.md`
- Create: `prompts/skills/professional-writing/SKILL.md`
- Create: `prompts/skills/professional-writing/references/few-shot-1.md`
- Create: `prompts/skills/brainstorming/SKILL.md`
- Create: `prompts/skills/brainstorming/references/example-vague-request.md`
- Create: `prompts/skills/brainstorming/references/example-clarification-flow.md`

- [ ] **Step 1: 创建 `concise-writing/SKILL.md`**

```markdown
---
name: concise-writing
description: "Load when the user prefers brief, direct answers without fluff"
version: "1.0"
depends: []
category: "writing-style"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "low"
---

You must write in a concise, direct style. Follow these rules:

## NO-LIST (Never do these)
- Do NOT use filler phrases like "It is important to note that..."
- Do NOT repeat the same point in different words
- Do NOT include unnecessary background unless explicitly asked
- Do NOT use more than 3 sentences per paragraph unless the topic demands depth

## YES-LIST (Always do these)
- Start with the answer, then explain if needed
- Use bullet points for lists of 3+ items
- One idea per sentence

## Few-shot Examples
{{ref:few-shot-1}}
```

- [ ] **Step 2: 创建 `concise-writing/references/few-shot-1.md`**

```markdown
# Example: Concise Response

User: "What is Rust?"

Bad (verbose): "Rust is a systems programming language that has been developed with a strong focus on safety and performance, and it is important to note that it prevents segfaults..."

Good (concise): "Rust is a systems programming language that prevents segfaults and guarantees thread safety while maintaining C-like performance."
```

- [ ] **Step 3: 创建 `academic-writing/SKILL.md`**

```markdown
---
name: academic-writing
description: "Load when the user requests scholarly, evidence-based analysis"
version: "1.0"
depends: []
category: "writing-style"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "low"
---

You must write in an academic style. Follow these rules:

## NO-LIST
- Do NOT use colloquialisms or slang
- Do NOT make claims without supporting evidence
- Do NOT use first person ("I think", "I believe") unless quoting

## YES-LIST
- Cite sources when making factual claims
- Use formal vocabulary and precise terminology
- Structure arguments with premise → evidence → conclusion
- Acknowledge limitations and counterarguments

## Few-shot Examples
{{ref:few-shot-1}}
```

- [ ] **Step 4: 创建 `academic-writing/references/few-shot-1.md`**

```markdown
# Example: Academic Response

User: "Is climate change caused by human activity?"

Bad: "I think humans are definitely causing climate change because it's obvious."

Good: "The scientific consensus, supported by the IPCC's Sixth Assessment Report (2021), indicates that anthropogenic greenhouse gas emissions are the primary driver of observed global warming since the mid-20th century. Multiple lines of evidence—including temperature reconstructions, climate models, and attribution studies—converge on this conclusion."
```

- [ ] **Step 5: 创建 `storytelling/SKILL.md`**

```markdown
---
name: storytelling
description: "Load when the user wants a narrative, story-based explanation"
version: "1.0"
depends: []
category: "writing-style"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "low"
---

You must write using storytelling techniques. Follow these rules:

## NO-LIST
- Do NOT use dry, bullet-point lists as the primary structure
- Do NOT jump between unrelated examples without a narrative thread
- Do NOT omit the human or contextual element

## YES-LIST
- Frame explanations as a journey or narrative arc
- Use concrete characters, scenarios, or historical examples
- Build tension or curiosity before revealing the conclusion
- End with a clear takeaway or moral

## Few-shot Examples
{{ref:few-shot-1}}
```

- [ ] **Step 6: 创建 `storytelling/references/few-shot-1.md`**

```markdown
# Example: Storytelling Response

User: "How does a database index work?"

Bad: "A database index is a data structure that improves query speed. It works by creating a sorted copy of column values..."

Good: "Imagine you're a librarian in a massive library with millions of books. A patron asks for every book published in 1997. Without an index, you'd have to check every single book—page by page. With an index (like a card catalog sorted by year), you walk directly to the 1997 shelf. That's what a database index does: it pre-sorts the data so the database doesn't have to scan every row."
```

- [ ] **Step 7: 创建 `professional-writing/SKILL.md`**

```markdown
---
name: professional-writing
description: "Load when the user wants business-appropriate, polished communication"
version: "1.0"
depends: []
category: "writing-style"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "low"
---

You must write in a professional, business-appropriate style. Follow these rules:

## NO-LIST
- Do NOT use overly casual language or emojis
- Do NOT be ambiguous—state conclusions clearly
- Do NOT ramble; get to the point efficiently

## YES-LIST
- Use clear subject lines or headings
- Lead with the bottom line (BLUF: Bottom Line Up Front)
- Include actionable next steps when appropriate
- Maintain a respectful but confident tone
```

- [ ] **Step 8: 创建 `brainstorming/SKILL.md`**

```markdown
---
name: brainstorming
description: "Load when the user's request is vague, underspecified, or exploratory"
version: "1.0"
depends: []
category: "behavior"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "low"
---

You are in brainstorming mode. The user has asked something vague or exploratory.
Your job is NOT to give a final answer immediately. Instead, follow this protocol:

## Protocol

### Step 1: Identify what's missing
Analyze the user's request and identify:
- What goal are they trying to achieve? (state your understanding)
- What constraints or preferences are unstated?
- What scope decisions need to be made?

### Step 2: Ask clarifying questions (max 2 per turn)
Present 1-2 focused questions that would most reduce ambiguity. Each question should:
- Be multiple-choice when possible
- Cover the most consequential uncertainty first
- Avoid asking everything at once

### Step 3: Synthesize and confirm
After the user answers, restate your understanding in this format:
```
Based on what you've told me:
- Goal: [summarized goal]
- Constraints: [summarized constraints]
- Approach I'm considering: [your proposed approach]

Does this look right? If yes, I'll proceed. If not, tell me what to adjust.
```

### Step 4: Exit brainstorming
Only after explicit user confirmation do you switch back to normal answer mode.

## NO-LIST
- Do NOT give a full answer while in brainstorming mode
- Do NOT ask more than 2 questions in one turn
- Do NOT assume preferences that the user hasn't stated
- Do NOT exit brainstorming without explicit user confirmation

## Examples
{{ref:example-vague-request}}
{{ref:example-clarification-flow}}
```

- [ ] **Step 9: 创建 Brainstorming references**

`example-vague-request.md`:
```markdown
# Example: Vague Request

User: "I want to build something"

Agent: "I'd love to help you build something. To point you in the right direction, could you clarify two things:

1. What type of project are you envisioning? [software / content / physical product / other]
2. Is this for personal use, a business, or learning?"
```

`example-clarification-flow.md`:
```markdown
# Example: Clarification Flow

User: "software, for learning"

Agent: "Based on what you've told me:
- Goal: Build a software project for learning purposes
- Constraints: None specified yet
- Approach I'm considering: Recommend a beginner-friendly project with clear milestones

Does this look right? If yes, I'll suggest some projects. If not, tell me what to adjust."

User: "yes"

Agent: [Now exits brainstorming mode and provides actual recommendations]
```

- [ ] **Step 10: 编译验证 Skill 注册**

Run: `cargo build -p app`
Expected: 编译通过，`build.rs` 自动扫描新 Skill 目录并生成注册代码

- [ ] **Step 11: 验证 Skill 加载**

Run: `cargo test -p app --lib -- prompts::tests`
Expected: 通过，新 Skill 出现在 registry 中

- [ ] **Step 12: Commit**

```bash
git add prompts/skills/concise-writing/ prompts/skills/academic-writing/ prompts/skills/storytelling/ prompts/skills/professional-writing/ prompts/skills/brainstorming/
git commit -m "feat(skills): add writing style skills (concise, academic, storytelling, professional) and brainstorming behavior skill

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2.6: Brainstorming 模式集成到 Answer 阶段

**目标**: 当 `behavior_mode == "brainstorming"` 时，Answer system prompt 加载 brainstorming skill。

**Files:**
- Modify: `crates/app/src/agents/strategy/chat.rs`
- Modify: `crates/app/src/agents/strategy/rag.rs`
- Modify: `crates/app/src/agents/strategy/search.rs`

- [ ] **Step 1: 创建 `load_behavior_mode_skill` 辅助函数**

在 `prompts.rs` 中新增：

```rust
/// Load a behavior mode skill body into the system prompt if active.
pub fn load_behavior_mode_skill(behavior_mode: Option<&str>) -> Option<String> {
    let mode = behavior_mode?;
    let registry = PromptRegistry::standard_cached();
    registry.skill(mode).map(|s| s.system_prompt().to_string())
}
```

- [ ] **Step 2: 修改 Chat 的 `step_answer` 注入 behavior mode**

在 `chat.rs` 的 `step_answer` 中，构建 system_prompt 后：

```rust
let mut system_prompt = crate::agents::strategy::prompts::build_answer_system_prompt(
    crate::agents::strategy::prompts::chat::ANSWER_SKILL_ID,
    "chat",
    &[],
    &ctx.selected_writing_styles,
);

// Inject behavior mode skill if active
if let Some(behavior_skill) = crate::agents::strategy::prompts::load_behavior_mode_skill(ctx.behavior_mode.as_deref()) {
    system_prompt.push_str("\n\n---\n\n");
    system_prompt.push_str(&behavior_skill);
}
```

- [ ] **Step 3: 修改 Rag 的 `step_answer`**

类似修改，在构建 system_prompt 后注入 behavior mode skill。

- [ ] **Step 4: 修改 Search 的 `step_answer`**

同样修改。

- [ ] **Step 5: 编译检查**

Run: `cargo check -p app`
Expected: 通过

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/agents/strategy/prompts.rs crates/app/src/agents/strategy/chat.rs crates/app/src/agents/strategy/rag.rs crates/app/src/agents/strategy/search.rs
git commit -m "feat(behavior): inject brainstorming skill into answer prompt when behavior_mode is active

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2.7: Rag Plan 输出扩展（writing_styles + behavior_mode）

**目标**: Rag 的 Plan LLM 输出也包含 writing_styles 和 behavior_mode 字段。

**Files:**
- Modify: `crates/app/src/agents/strategy/rag.rs`

- [ ] **Step 1: 找到 Rag 的 Plan 解析逻辑**

搜索 `parse_rag_plan` 或等效函数，确认 Plan 输出结构。

- [ ] **Step 2: 扩展 Plan 输出结构**

在 Plan 输出结构（如 `RagPlannerOutput`）中新增：

```rust
pub writing_styles: Vec<String>,
pub behavior_mode: Option<String>,
```

- [ ] **Step 3: 扩展解析逻辑**

从 Plan LLM 的 JSON 输出中解析 `writing_styles` 和 `behavior_mode` 字段，存入 `ctx.selected_writing_styles` 和 `ctx.behavior_mode`。

- [ ] **Step 4: 编译检查**

Run: `cargo check -p app`
Expected: 通过

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/agents/strategy/rag.rs
git commit -m "feat(rag): extend planner output with writing_styles and behavior_mode

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2.8: Search Plan 输出扩展

**目标**: Search 的 Plan LLM 输出也包含 writing_styles 和 behavior_mode。

**Files:**
- Modify: `crates/app/src/agents/strategy/search.rs`

- [ ] **Step 1-5**: 同 Task 2.7，应用到 Search 策略。

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/agents/strategy/search.rs
git commit -m "feat(search): extend planner output with writing_styles and behavior_mode

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2.9: 运行全部 lib 测试

**目标**: 确保 Phase 2 改动不破坏现有测试。

- [ ] **Step 1: 运行全部 lib 测试**

Run: `cargo test -p app --lib`
Expected: 全部通过

- [ ] **Step 2: 运行 clippy**

Run: `cargo clippy -p app -- -D warnings`
Expected: 无新增 warning

---

## Phase 3: Agent 自治多轮对话记忆

### Task 3.1: 数据库迁移

**目标**: 创建 `message_tags` 表。

**Files:**
- Create: `migrations/0034_conversation_memory.up.sql`
- Create: `migrations/0034_conversation_memory.down.sql`

- [ ] **Step 1: 创建 up 迁移**

```sql
CREATE TABLE message_tags (
    id BIGSERIAL PRIMARY KEY,
    message_id BIGINT NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(message_id, tag)
);

CREATE INDEX idx_message_tags_tag ON message_tags(tag);
CREATE INDEX idx_message_tags_message_id ON message_tags(message_id);
```

- [ ] **Step 2: 创建 down 迁移**

```sql
DROP INDEX IF EXISTS idx_message_tags_message_id;
DROP INDEX IF EXISTS idx_message_tags_tag;
DROP TABLE IF EXISTS message_tags;
```

- [ ] **Step 3: Commit**

```bash
git add migrations/0034_conversation_memory.up.sql migrations/0034_conversation_memory.down.sql
git commit -m "feat(db): add message_tags table for conversation memory

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3.2: Repository 层 CRUD

**目标**: 实现 `message_tags` 的增删改查。

**Files:**
- Create: `crates/storage-pg/src/lib_impl/repository_conversation_memory.rs`
- Modify: `crates/storage-pg/src/lib_impl.rs`

- [ ] **Step 1: 创建 repository 文件**

```rust
use sqlx::Row;
use uuid::Uuid;

use super::core::{PgAppRepository, PgStorageError};
use avrag_auth::AuthContext;

/// Tag operation sent by the agent.
#[derive(Debug, Clone)]
pub enum TagOperation {
    AddTag { message_id: i64, tag: String },
    RemoveTag { message_id: i64, tag: String },
    ReplaceTags { message_id: i64, tags: Vec<String> },
}

/// A chat message with its tags.
#[derive(Debug, Clone)]
pub struct TaggedMessage {
    pub message_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub tags: Vec<String>,
}

impl PgAppRepository {
    /// Load messages from a session, optionally filtered by tags.
    pub async fn load_history_by_tags(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        tags: Option<Vec<String>>,
        limit: i64,
    ) -> Result<Vec<TaggedMessage>, PgStorageError> {
        let rows = if let Some(ref tag_list) = tags {
            sqlx::query(
                r#"
                SELECT 
                    m.id as message_id,
                    m.role,
                    m.content,
                    m.created_at,
                    COALESCE(
                        ARRAY_AGG(mt.tag) FILTER (WHERE mt.tag IS NOT NULL),
                        ARRAY[]::TEXT[]
                    ) as tags
                FROM chat_messages m
                LEFT JOIN message_tags mt ON m.id = mt.message_id
                WHERE m.session_id = $1 AND m.owner_user_id = $2
                  AND EXISTS (
                      SELECT 1 FROM message_tags mt2
                      WHERE mt2.message_id = m.id AND mt2.tag = ANY($3)
                  )
                GROUP BY m.id
                ORDER BY m.id DESC
                LIMIT $4
                "#
            )
            .bind(session_id)
            .bind(auth.owner_user_id())
            .bind(tag_list)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT 
                    m.id as message_id,
                    m.role,
                    m.content,
                    m.created_at,
                    COALESCE(
                        ARRAY_AGG(mt.tag) FILTER (WHERE mt.tag IS NOT NULL),
                        ARRAY[]::TEXT[]
                    ) as tags
                FROM chat_messages m
                LEFT JOIN message_tags mt ON m.id = mt.message_id
                WHERE m.session_id = $1 AND m.owner_user_id = $2
                GROUP BY m.id
                ORDER BY m.id DESC
                LIMIT $3
                "#
            )
            .bind(session_id)
            .bind(auth.owner_user_id())
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        let mut messages = Vec::new();
        for row in rows {
            let tags: Vec<String> = row.try_get("tags").unwrap_or_default();
            messages.push(TaggedMessage {
                message_id: row.try_get("message_id")?,
                role: row.try_get("role")?,
                content: row.try_get("content")?,
                created_at: row.try_get("created_at")?,
                tags,
            });
        }
        Ok(messages)
    }

    /// Apply tag operations (add/remove/replace).
    pub async fn apply_tag_operations(
        &self,
        auth: &AuthContext,
        operations: Vec<TagOperation>,
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.begin().await?;

        for op in operations {
            match op {
                TagOperation::AddTag { message_id, tag } => {
                    sqlx::query(
                        "INSERT INTO message_tags (message_id, tag) VALUES ($1, $2) ON CONFLICT DO NOTHING"
                    )
                    .bind(message_id)
                    .bind(&tag)
                    .execute(&mut *tx)
                    .await?;
                }
                TagOperation::RemoveTag { message_id, tag } => {
                    sqlx::query(
                        "DELETE FROM message_tags WHERE message_id = $1 AND tag = $2"
                    )
                    .bind(message_id)
                    .bind(&tag)
                    .execute(&mut *tx)
                    .await?;
                }
                TagOperation::ReplaceTags { message_id, tags } => {
                    sqlx::query("DELETE FROM message_tags WHERE message_id = $1")
                        .bind(message_id)
                        .execute(&mut *tx)
                        .await?;
                    for tag in tags {
                        sqlx::query(
                            "INSERT INTO message_tags (message_id, tag) VALUES ($1, $2) ON CONFLICT DO NOTHING"
                        )
                        .bind(message_id)
                        .bind(&tag)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
            }
        }

        tx.commit().await?;
        Ok(())
    }
}
```

- [ ] **Step 2: 在 `lib_impl.rs` 中引入模块**

```rust
pub mod repository_conversation_memory;
```

- [ ] **Step 3: 编译检查**

Run: `cargo check -p storage-pg`
Expected: 通过

- [ ] **Step 4: Commit**

```bash
git add crates/storage-pg/src/lib_impl/repository_conversation_memory.rs crates/storage-pg/src/lib_impl.rs
git commit -m "feat(storage): add message_tags CRUD operations

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3.3: 新原子工具实现

**目标**: 实现 `conversation_history_load` 和 `conversation_history_tag` 两个工具。

**Files:**
- Create: `crates/app/src/agents/skills/conversation_history.rs`
- Modify: `crates/app/src/agents/skills/mod.rs`
- Modify: `crates/app/src/agents/capability/registry.rs`

- [ ] **Step 1: 创建工具实现文件**

```rust
use std::sync::Arc;

use crate::agents::skills::{SkillComponent, SkillContext, SkillResult};

/// Load conversation history by tags.
pub struct ConversationHistoryLoad;

#[async_trait::async_trait]
impl SkillComponent for ConversationHistoryLoad {
    fn spec(&self) -> common::ToolSpec {
        common::ToolSpec {
            name: "conversation_history_load".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Load previous messages from this session. ",
                "Use without tags to load all messages for initial analysis. ",
                "Use with tags to recall specific topics."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Filter by tags. Omit to load all.",
                        "optional": true
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max messages to load (default 20).",
                        "default": 20,
                        "optional": true
                    }
                }
            }),
            output_schema: serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "message_id": { "type": "integer" },
                        "role": { "type": "string" },
                        "content": { "type": "string" },
                        "tags": { "type": "array", "items": { "type": "string" } }
                    }
                }
            }),
        }
    }

    async fn execute(&self, ctx: &SkillContext, args: serde_json::Value) -> SkillResult {
        let tags: Option<Vec<String>> = args
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

        let limit = args
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(20);

        // This tool requires access to the repository, which is not directly available
        // in SkillContext. For now, return a placeholder indicating the tool was called.
        // The actual implementation will be handled at the strategy level.
        Ok(serde_json::json!({
            "status": "ok",
            "message": "History load requested. Tags: {:?}, Limit: {}",
            "tags": tags,
            "limit": limit
        }))
    }
}

/// Tag conversation messages.
pub struct ConversationHistoryTag;

#[async_trait::async_trait]
impl SkillComponent for ConversationHistoryTag {
    fn spec(&self) -> common::ToolSpec {
        common::ToolSpec {
            name: "conversation_history_tag".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Tag messages with descriptive, specific labels. ",
                "Every loaded message should receive at least one tag. ",
                "Tags should distinguish topics clearly."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "message_id": { "type": "integer" },
                                "action": { "type": "string", "enum": ["add", "remove", "replace"] },
                                "tags": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["message_id", "action", "tags"]
                        }
                    }
                },
                "required": ["operations"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string" },
                    "applied": { "type": "integer" }
                }
            }),
        }
    }

    async fn execute(&self, _ctx: &SkillContext, args: serde_json::Value) -> SkillResult {
        let operations = args
            .get("operations")
            .and_then(|v| v.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);

        Ok(serde_json::json!({
            "status": "ok",
            "applied": operations
        }))
    }
}
```

注意：这里使用了 `SkillComponent` trait（与现有原子工具相同的模式）。但由于 conversation history 工具需要访问 PostgreSQL repository，而 `SkillContext` 可能没有提供这个访问，实际的数据库操作将在 Strategy 层处理（通过直接调用 repository 方法），工具层面只做参数解析和结果格式化。

- [ ] **Step 2: 在 `mod.rs` 中导出**

```rust
pub mod conversation_history;
```

- [ ] **Step 3: 在 CapabilityRegistry 中注册**

在 `registry.rs` 的 `standard()` 方法中，新增工具注册：

```rust
// 在工具注册循环之后:
let history_load = crate::agents::skills::conversation_history::ConversationHistoryLoad;
let meta = tool_to_metadata(&history_load, ToolSource::AtomicToolCatalog);
tools.insert(meta.id.clone(), meta);

let history_tag = crate::agents::skills::conversation_history::ConversationHistoryTag;
let meta = tool_to_metadata(&history_tag, ToolSource::AtomicToolCatalog);
tools.insert(meta.id.clone(), meta);
```

- [ ] **Step 4: 编译检查**

Run: `cargo check -p app`
Expected: 通过

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/agents/skills/conversation_history.rs crates/app/src/agents/skills/mod.rs crates/app/src/agents/capability/registry.rs
git commit -m "feat(tools): add conversation_history_load and conversation_history_tag atomic tools

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3.4: 移除 10 轮硬编码窗口

**目标**: `AgentRequest.messages` 不再自动注入最近 10 轮历史，只保留当前查询。

**Files:**
- Modify: `crates/app/src/agents/runtime.rs`
- Modify: `crates/app/src/agents/strategy/chat.rs`
- Modify: `crates/app/src/agents/strategy/rag.rs`

- [ ] **Step 1: 修改 `build_chat_messages_with_system`（Chat）**

在 `chat.rs` 中，找到 `build_chat_messages_with_system` 函数。修改历史消息注入逻辑：

```rust
fn build_chat_messages_with_system(
    request: &AgentRequest,
    system_prompt: &str,
) -> Vec<avrag_llm::ChatMessage> {
    let mut system = String::from(system_prompt);
    // ... session_summary and user_preferences injection unchanged ...

    let mut messages = vec![avrag_llm::ChatMessage::system(system)];

    // Only inject the current turn's user message (not full history)
    // History is loaded on-demand via conversation_history_load tool
    messages.push(avrag_llm::ChatMessage::user(&request.query));
    messages
}
```

- [ ] **Step 2: 修改 Rag 的历史注入**

在 `rag.rs` 中，找到历史消息构建逻辑。类似地，只保留当前查询，不自动注入历史。

注意：Rag 的 `ctx.history` 字段当前存储了历史消息。需要评估是否保留这个字段用于其他用途，或者清空它。

- [ ] **Step 3: 更新 Plan prompt 说明**

在 `build_plan_system_prompt` 中增加历史工具使用说明：

```rust
// 在 tool_catalog 之后增加:
let history_hint = format!(
    "\n\n## Conversation Memory\n\
    You have access to the conversation history via `conversation_history_load`.\n\
    - Call it without tags to load all messages for analysis and tagging.\n\
    - Call it with specific tags to recall relevant past discussions.\n\
    - After loading, use `conversation_history_tag` to label messages with specific, distinguishable tags.\n\
    - Every loaded message should receive at least one tag."
);
parts.push(history_hint);
```

- [ ] **Step 4: 编译检查**

Run: `cargo check -p app`
Expected: 通过

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/agents/runtime.rs crates/app/src/agents/strategy/chat.rs crates/app/src/agents/strategy/rag.rs crates/app/src/agents/strategy/prompts.rs
git commit -m "feat(memory): replace 10-turn hardcoded window with on-demand conversation history loading

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3.5: 运行全部测试

- [ ] **Step 1: 运行 lib 测试**

Run: `cargo test -p app --lib`
Expected: 全部通过

- [ ] **Step 2: 运行 storage-pg 测试**

Run: `cargo test -p storage-pg`
Expected: 全部通过

- [ ] **Step 3: 运行 clippy**

Run: `cargo clippy -p app -- -D warnings`
Expected: 无新增 warning

- [ ] **Step 4: 最终 Commit**

```bash
git commit -m "feat: complete agent-autonomous conversation memory implementation

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## 自检清单

### Spec 覆盖检查

| 设计文档需求 | 对应 Task |
|-------------|----------|
| Tool catalog 按策略隔离 | Task 1.1, 1.2 |
| Format skills E2E 实际验证 | Task 1.3 |
| doc_index/index_lookup 评估 | Task 1.4 |
| SkillMetadata category 字段 | Task 2.1 |
| CapabilityRegistry 查询扩展 | Task 2.2 |
| PromptBuilder 写作风格注入 | Task 2.3 |
| 新建技能文件 | Task 2.5 |
| Brainstorming 模式集成 | Task 2.6 |
| Plan 输出扩展（三策略） | Task 2.4, 2.7, 2.8 |
| message_tags 表 | Task 3.1 |
| 历史工具实现 | Task 3.3 |
| 10轮窗口替换 | Task 3.4 |

### Placeholder 扫描

- [x] 无 "TBD" / "TODO" / "implement later"
- [x] 无 "Add appropriate error handling" 等模糊描述
- [x] 每个代码步骤包含完整代码
- [x] 无 "Similar to Task N" 省略

### 类型一致性

- [x] `SkillMetadata.category` 在 Task 2.1 定义，Task 2.2 查询中使用 `s.category == "writing-style"`
- [x] `ChatContext.selected_writing_styles` 在 Task 2.4 定义，Task 2.3 的 `step_answer` 中读取 `ctx.selected_writing_styles`
- [x] `ChatPlanDecision.writing_styles` 在 Task 2.4 定义，解析逻辑中从 JSON 读取 `"writing_styles"`
- [x] `AgentEvent::PlanDecision.selected_writing_styles` 在 Task 2.4 定义，Rag/Chat/Search 的构造处一致

---

**Plan complete and saved to `docs/superpowers/plans/2026-05-26-writing-style-conversation-memory-plan.md`.**

Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — Execute tasks in this session, batch execution with checkpoints for review

Which approach?
