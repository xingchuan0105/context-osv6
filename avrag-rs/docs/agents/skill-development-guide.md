# Skill Development Guide

> 基于 Perplexity 的 [Agent Skills 工程方法论](https://www.perplexity.ai/hub/blog/designing-refining-and-maintaining-agent-skills-at-perplexity) 和本项目的 `SkillComponent` 架构。

---

## 1. What is a Skill?

在本项目中，**Skill** 不是代码，而是**给模型的上下文包装**。一个 Skill 决定：
- **什么时候**被调用（description = 路由触发器）
- **怎么调用**（JSON schema + rules）
- **什么不能做**（gotchas = 负例）

> "If you write a Skill like you do code, you will fail." — Perplexity

---

## 2. SkillComponent 架构

```rust
#[async_trait::async_trait]
pub trait SkillComponent: Send + Sync {
    fn id(&self) -> &str;                           // 唯一标识
    fn version(&self) -> &str;                      // 语义版本
    fn description(&self) -> &str;                  // Index-tier: "Load when..."
    fn spec(&self) -> ToolSpec;                     // Load-tier: JSON schema + rules
    fn gotchas(&self) -> &[&str];                  // Runtime-tier: 负例
    fn render_hint(&self) -> &str;                 // 前端渲染提示
    async fn execute(&self, args: &Value, ctx: &ExecutionContext<'_>) -> ToolResult;
}
```

### 三层上下文成本（Perplexity 模型）

| Tier | 内容 | Budget | 付费时机 |
|------|------|--------|---------|
| **Index** | `name: description` | ~50 words | 每次会话，每个用户 |
| **Load** | `ToolSpec` body | ~500 tokens | 加载时 |
| **Runtime** | Gotchas + assets | 按需 | 仅当需要时 |

**金句**："Every word in the description is paid by every session, every user."

---

## 3. 开发流程（Eval-first）

### Step 0: 写 Eval（在写 Skill 之前）

在 `crates/app/tests/planner_routing_eval.rs` 添加 eval case：

```rust
#[test]
fn eval_my_tool() {
    run_eval(&DisclosureEvalCase {
        query: "do something my tool handles",
        agent_kind: AgentKind::Chat,
        expected_tools: &["my_tool"],
    })
    .unwrap();
}
```

### Step 1: 写 Description（最难的部分）

**必须**：
- 以 `"Load when..."` 开头
- ≤50 个词
- 用用户真实查询中的词汇
- 不总结工作流

**好例子**：
```
"Load when the user asks to compute, evaluate, or solve a mathematical expression."
```

**坏例子**：
```
"Calculator evaluates math expressions using evalexpr. Supports sin, cos, sqrt, etc."
```

### Step 2: 写 Body（Spec + Schema）

在 `builtin/my_tool.rs` 中实现 `spec()`：

```rust
fn spec(&self) -> ToolSpec {
    ToolSpec {
        name: "my_tool".to_string(),
        version: "1.0".to_string(),
        description: "...".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "param1": { "type": "string", "description": "..." }
            },
            "required": ["param1"]
        }),
        output_schema: serde_json::json!({}),
    }
}
```

### Step 3: 写 Execute

```rust
async fn execute(&self, args: &Value, ctx: &ExecutionContext<'_>) -> ToolResult {
    let param1 = args.get("param1").and_then(|v| v.as_str()).unwrap_or_default();
    // ... 执行逻辑
}
```

### Step 4: 积累 Gotchas

从真实失败中积累，不是臆想：

```rust
fn gotchas(&self) -> &[&str] {
    &[
        "Empty input returns Error, not a default value.",
        "Parameter X must be lowercase; uppercase is silently ignored.",
    ]
}
```

### Step 5: 注册

在 `builtin/mod.rs`：
```rust
registry.register(Box::new(my_tool::MyToolSkill));
```

### Step 6: 运行 Eval

```bash
cargo test -p app --test planner_routing_eval
cargo test -p app --lib  # 确保所有现有测试通过
```

---

## 4. Gotcha Flywheel

Skill 是 **append-mostly** 的。维护循环：

```
Agent 失败          → 添加 gotcha
Agent 误加载 Skill   → 收紧 description，添加负例 eval
Agent 该加载未加载   → 添加关键词，添加正例 eval
System prompt 变更   → 检查冲突或重复
```

---

## 5. Action at a Distance

> "Every time you add an additional Skill, you risk making every other Skill slightly worse."

**原因**：planner prompt 中每个 Skill 的 description 都在争夺 LLM 的注意力。

**缓解**：
1. 修改 description 后必须运行 full eval suite
2. 新增 Skill 时检查边界冲突（calculator vs code_interpreter 的数学查询边界）
3. 使用 eval 中的 `forbidden_tools` 字段（未来 LLM-in-the-loop 路由 eval）

---

## 6. 检查清单（PR Review）

- [ ] Description 以 `"Load when..."` 开头
- [ ] Description ≤50 词
- [ ] 有至少 2 个 gotchas（即使是新 Skill）
- [ ] 有正例 eval（expected_tools）
- [ ] 所有现有测试通过
- [ ] `cargo clippy -p app --tests` 无新增 warning
- [ ] 前端 render_hint 已注册（如果使用新 hint）

---

## 7. Prompt Skill 文件（Frontmatter）

除了代码中的 `SkillComponent`，项目中还有 **prompt 文件层面的 Skill**（`prompts/*.txt`）。这些文件现在使用 YAML frontmatter：

```yaml
---
name: rag_plan
description: "Load when the user asks a question that requires retrieving evidence from workspace documents."
version: "1.0"
depends: []
---
```

### 规则

1. **description 是路由触发器**：必须是 `"Load when..."` 格式，≤50 词
2. **不要硬编码工具描述**：prompt 文件中不要包含 `### tool_name` 格式的工具目录。工具描述统一由 `tool_catalog.rs` 生成，通过 `PhaseConfig` 在 Plan/Execute 阶段披露。
3. **单一事实来源**：工具 spec（JSON Schema、参数、规则）只存在于 `tool_catalog.rs` 或 `SkillComponent::spec()` 中，不在 prompt 中重复。

### 解析

`PromptRegistry::standard()` 通过 `skill_frontmatter::parse_skill_file()` 自动解析 frontmatter。解析失败时回退到直接内容（兼容旧格式）。

---

## 8. References

- `crates/app/src/agents/skills/mod.rs` — SkillComponent trait
- `crates/app/src/agents/skills/builtin/` — 内置 Skill 示例
- `crates/app/src/agents/progressive/skill_frontmatter.rs` — Frontmatter 解析器
- `crates/app/src/agents/progressive/prompt_registry.rs` — Prompt Skill 注册表
- `crates/app/tests/planner_routing_eval.rs` — Eval suite
- [Perplexity Agent Skills Guide](https://www.perplexity.ai/hub/blog/designing-refining-and-maintaining-agent-skills-at-perplexity)
