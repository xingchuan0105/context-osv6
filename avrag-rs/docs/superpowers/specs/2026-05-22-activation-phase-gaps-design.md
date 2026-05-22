# ActivationPhase 缺口修复：EvaluateOutput + Eval 工具目录 + Dead Code 清理

> Status: Approved
> Date: 2026-05-22
> Scope: 修复 ActivationPhase 实现与 spec 的 3 个差距

---

## 1. 背景

ActivationPhase 主体实现（10 个 task）已完成，但 review 发现 3 个缺口：

| 缺口 | 优先级 | 描述 |
|------|--------|------|
| EvaluateOutput 结构 | P0 | spec 要求 `EvalDecision` + `NextAction::ToolCall`，当前仍用旧 `StrategyRecommendation` |
| Eval system prompt 缺工具目录 | P1 | spec 要求 eval prompt 包含工具目录，让模型 replan 时知道能换什么工具 |
| Legacy dead code | P2 | `chat::plan_tools()` 等 6 个旧 helper 不再被调用但未删除 |

---

## 2. 数据模型变更

### 2.1 新增类型（`crates/app/src/rag_prompts.rs`）

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalDecision {
    /// 证据充分，进入 Answer
    Sufficient,
    /// 证据不足，需要补充
    Insufficient,
    /// 放弃，降级回答
    GiveUp,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NextAction {
    SubQuery { query: String },
    ToolCall { tool: String, args: serde_json::Value, reason: String },
}
```

### 2.2 扩展现有类型（共存模式）

保留 `dimensions` / `missing_dimensions` / `weak_dimensions` 作为可选字段（可观测性价值），旧字段变为 `Option`/`default`，新增 `decision` + `next_actions` + `reasoning` 为必填。

```rust
pub struct RagStrategyEvaluation {
    // 保留：维度分析（可选，向后兼容）
    #[serde(default)]
    pub dimensions: Vec<StrategyDimension>,
    #[serde(default)]
    pub missing_dimensions: Vec<String>,
    #[serde(default)]
    pub weak_dimensions: Vec<String>,
    // 保留但可选（向后兼容）
    #[serde(default)]
    pub recommendation: Option<StrategyRecommendation>,
    #[serde(default)]
    pub suggested_followup_queries: Vec<String>,
    // 新增（必填）
    pub decision: EvalDecision,
    pub next_actions: Vec<NextAction>,
    pub reasoning: String,
}
```

`SearchStrategyEvaluation` 同理扩展。

消费代码统一读新字段（`decision` / `next_actions` / `reasoning`），不再匹配旧 `recommendation`。

---

## 3. Eval Skill 文件 + System Prompt 改造

### 3.1 Eval Skill 文件更新

`prompts/skills/rag-eval/SKILL.md` 和 `prompts/skills/search-eval/SKILL.md` 的 JSON schema 更新为：

```json
{
  "dimensions": [ ... ],
  "missing_dimensions": [...],
  "weak_dimensions": [...],
  "decision": "sufficient" | "insufficient" | "give_up",
  "next_actions": [
    {"type": "sub_query", "query": "..."} |
    {"type": "tool_call", "tool": "dense_retrieval", "args": {...}, "reason": "..."}
  ],
  "reasoning": "one-sentence explanation"
}
```

旧的 `recommendation` 和 `suggested_followup_queries` 从 schema 中删除。Rust 端 `Option`/`default` 兜底，避免 LLM 偶尔漏字段时报错。

### 3.2 Decision 映射规则

| EvalDecision | 对应旧值 | 触发条件 |
|-------------|---------|---------|
| `sufficient` | `synthesize` | 所有维度至少 covered_weak |
| `insufficient` | `replan` + `broaden` | 有 missing 或 weak 维度 |
| `give_up` | （新增） | 预算已用尽且无改善空间 |

### 3.3 Next Actions 规则

- `insufficient` 时必须给出至少一个 `sub_query` 或 `tool_call`
- `tool_call` 仅在模型判断换工具更有效时发出（如 dense 搜不到 → 建议 graph_retrieval）
- `sufficient` / `give_up` 时 `next_actions` 为空数组

### 3.4 Eval System Prompt 注入工具目录

rag.rs 和 search.rs 的 `build_eval_system_prompt()` 从只加载 skill body 改为：

```rust
fn build_eval_system_prompt(strategy: &str) -> String {
    let skill_id = match strategy {
        "rag" => "rag-eval",
        "search" => "search-eval",
        _ => unreachable!(),
    };
    let skill_body = registry.skill(skill_id)
        .map(|s| s.system_prompt().to_string())
        .unwrap_or_default();

    let cap_registry = CapabilityRegistry::standard_cached();
    let plan_tools = cap_registry.plan_tools(strategy);
    let tool_catalog = plan_tools.iter()
        .map(|t| format!("- {} (v{}): {}", t.id, t.version, t.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!("{skill_body}\n\n---\n\n## Available Tools for Replanning\n\n{tool_catalog}")
}
```

---

## 4. 策略代码消费新 EvaluateOutput

### 4.1 RagStrategy `step_evaluate`

```rust
match eval.decision {
    EvalDecision::Sufficient => {
        Ok(StepOutcome::Next(Box::new(RagState::Answer)))
    }
    EvalDecision::GiveUp => {
        self.finalize_degrade(ctx, DegradeReason::NoResultsAfterAllFallbacks)
            .await
            .map(StepOutcome::Terminate)
    }
    EvalDecision::Insufficient => {
        let mut sub_queries = Vec::new();
        let mut tool_hints = Vec::new();
        for action in &eval.next_actions {
            match action {
                NextAction::SubQuery { query } => sub_queries.push(query.clone()),
                NextAction::ToolCall { tool, args, reason } => {
                    tool_hints.push(format!("{tool}: {} ({reason})",
                        serde_json::to_string(args).unwrap_or_default()));
                }
            }
        }

        let mut directive_parts = vec![format!("replan: {}", eval.reasoning)];
        if !tool_hints.is_empty() {
            directive_parts.push(format!("suggested tools: {}", tool_hints.join(", ")));
        }

        ctx.iteration_params = RagIterationParams {
            query: original_query.clone(),
            directive: Some(directive_parts.join("\n")),
            suggested_queries: sub_queries,
        };
        Ok(StepOutcome::Next(Box::new(RagState::Plan)))
    }
}
```

ToolCall 处理方式：把 tool hint 塞进 directive 传给下一轮 Plan LLM，让 Plan 决定是否换工具。

### 4.2 SearchStrategy `map_search_strategy_to_advice`

```rust
fn map_search_strategy_to_advice(
    eval: &SearchStrategyEvaluation,
    current_vertical: Option<&str>,
) -> EvalAdvice {
    match eval.decision {
        EvalDecision::Sufficient => EvalAdvice::Synthesize,
        EvalDecision::GiveUp => EvalAdvice::Degrade {
            reason: DegradeReason::NoResultsAfterAllFallbacks,
        },
        EvalDecision::Insufficient => {
            let has_vertical_hint = eval.next_actions.iter().any(|a|
                matches!(a, NextAction::ToolCall { tool, .. } if tool == "web_search"));
            if has_vertical_hint && next_vertical_step(current_vertical).is_some() {
                EvalAdvice::EscalateVertical { reason: "llm_strategy_escalate_vertical".into() }
            } else {
                EvalAdvice::Replan { reason: eval.reasoning.clone() }
            }
        }
    }
}
```

### 4.3 旧 recommendation 消费路径删除

`StrategyRecommendation` 和 `SearchStrategyRecommendation` 枚举保留定义（`Option<StrategyRecommendation>` 向后兼容），但 step_evaluate 中不再匹配旧字段。

---

## 5. Dead Code 清理

删除 `crates/app/src/agents/strategy/prompts.rs` 中 6 个不再被调用的 legacy helper：

- `chat::plan_tools()` / `chat::format_skills()`
- `rag::plan_tools()` / `rag::format_skills()`
- `search::plan_tools()` / `search::format_skills()`

同时删除 `find_tool` helper（如不再被其他代码使用）。

---

## 6. 改动清单

| 文件 | 改动 |
|------|------|
| `crates/app/src/rag_prompts.rs` | 新增 `EvalDecision`、`NextAction`；`RagStrategyEvaluation` 和 `SearchStrategyEvaluation` 扩展新字段 |
| `crates/app/src/agents/strategy/rag.rs` | `build_eval_system_prompt` 注入工具目录；`step_evaluate` 匹配 `eval.decision` |
| `crates/app/src/agents/strategy/search.rs` | `build_eval_system_prompt` 注入工具目录；`map_search_strategy_to_advice` 匹配 `eval.decision` |
| `crates/app/src/agents/strategy/prompts.rs` | 删除 6 个 legacy helper |
| `prompts/skills/rag-eval/SKILL.md` | JSON schema 改为 `decision` + `next_actions` + `reasoning` |
| `prompts/skills/search-eval/SKILL.md` | 同上 |

---

## 7. 验收标准

1. `cargo test -p app --lib` 全部通过
2. Eval 输出包含 `decision` + `next_actions` 字段
3. Eval system prompt 包含工具目录（`Available Tools for Replanning`）
4. prompts.rs 无 dead code
5. Eval skill 文件 schema 与代码对齐

---

## 8. 风险与缓解

| 风险 | 缓解 |
|------|------|
| LLM 输出新字段不稳定 | Rust 端旧字段保留为 `Option`/`default`，fallback 到旧 `recommendation` 映射 |
| `ToolCall` 的 `args` 结构因工具而异 | `args: serde_json::Value` 保持灵活，Plan 阶段自行解析 |
| Eval prompt 变大（加入工具目录） | 工具目录只列 ID + version + description（Index tier），不含 schema |
