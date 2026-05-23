# ActivationPhase Gaps Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 3 gaps between ActivationPhase spec and implementation: EvaluateOutput unified struct, Eval system prompt tool catalog, and legacy dead code cleanup.

**Architecture:** Add `EvalDecision`/`NextAction` types to rag_prompts.rs, extend existing eval structs with new fields (coexistence mode), update eval system prompt builders to inject tool catalog, update strategy step_evaluate to consume new fields, delete dead helpers.

**Tech Stack:** Rust, serde, existing CapabilityRegistry infrastructure

---

## Task 1: Add EvalDecision and NextAction types to rag_prompts.rs

**Files:**
- Modify: `crates/app/src/rag_prompts.rs` (after line 80, before SubQueryItem)
- Test: `crates/app/src/rag_prompts.rs` (tests module)

- [ ] **Step 1: Write failing tests for EvalDecision and NextAction serialization**

Add these tests inside the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn eval_decision_serializes_snake_case() {
    let d = EvalDecision::Sufficient;
    assert_eq!(serde_json::to_string(&d).unwrap(), "\"sufficient\"");
    let d: EvalDecision = serde_json::from_str("\"give_up\"").unwrap();
    assert!(matches!(d, EvalDecision::GiveUp));
}

#[test]
fn next_action_sub_query_serializes() {
    let a = NextAction::SubQuery { query: "test query".to_string() };
    let json = serde_json::to_value(&a).unwrap();
    assert_eq!(json["type"], "sub_query");
    assert_eq!(json["query"], "test query");
}

#[test]
fn next_action_tool_call_serializes() {
    let a = NextAction::ToolCall {
        tool: "graph_retrieval".to_string(),
        args: serde_json::json!({"query": "test"}),
        reason: "dense failed".to_string(),
    };
    let json = serde_json::to_value(&a).unwrap();
    assert_eq!(json["type"], "tool_call");
    assert_eq!(json["tool"], "graph_retrieval");
    assert_eq!(json["reason"], "dense failed");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p app --lib rag_prompts::tests::eval_decision`
Expected: FAIL — `EvalDecision` not found

- [ ] **Step 3: Add EvalDecision and NextAction types**

Insert after line 80 (after `SearchStrategyRecommendation` enum, before `SubQueryItem`):

```rust
// ---------------- Unified evaluation output ----------------

/// Decision emitted by Evaluate phase.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalDecision {
    /// Evidence sufficient, proceed to Answer.
    Sufficient,
    /// Evidence insufficient, replan with new actions.
    Insufficient,
    /// Give up, degrade gracefully.
    GiveUp,
}

/// Action the Evaluate phase recommends for replanning.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NextAction {
    SubQuery { query: String },
    ToolCall {
        tool: String,
        args: serde_json::Value,
        reason: String,
    },
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p app --lib rag_prompts::tests::eval_decision`
Run: `cargo test -p app --lib rag_prompts::tests::next_action`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/rag_prompts.rs
git commit -m "feat(eval): add EvalDecision and NextAction types

EvalDecision: Sufficient | Insufficient | GiveUp
NextAction: SubQuery | ToolCall (tagged enum)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 2: Extend RagStrategyEvaluation with new fields

**Files:**
- Modify: `crates/app/src/rag_prompts.rs:15-26`
- Test: `crates/app/src/rag_prompts.rs` (tests module)

- [ ] **Step 1: Write failing test for new fields**

```rust
#[test]
fn rag_strategy_evaluation_has_decision_and_next_actions() {
    let eval: RagStrategyEvaluation = serde_json::from_str(r#"{
        "decision": "insufficient",
        "next_actions": [
            {"type": "sub_query", "query": "new query"}
        ],
        "reasoning": "missing dimension",
        "dimensions": [],
        "missing_dimensions": [],
        "weak_dimensions": []
    }"#).unwrap();
    assert!(matches!(eval.decision, EvalDecision::Insufficient));
    assert_eq!(eval.next_actions.len(), 1);
    assert_eq!(eval.reasoning, "missing dimension");
}

#[test]
fn rag_strategy_evaluation_backwards_compat_with_recommendation() {
    let eval: RagStrategyEvaluation = serde_json::from_str(r#"{
        "decision": "sufficient",
        "next_actions": [],
        "reasoning": "all covered",
        "recommendation": "synthesize",
        "dimensions": [],
        "missing_dimensions": [],
        "weak_dimensions": []
    }"#).unwrap();
    assert!(matches!(eval.recommendation, Some(StrategyRecommendation::Synthesize)));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p app --lib rag_prompts::tests::rag_strategy_evaluation_has_decision`
Expected: FAIL — `decision` field not found

- [ ] **Step 3: Update RagStrategyEvaluation struct**

Replace lines 14-26 with:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RagStrategyEvaluation {
    #[serde(default)]
    pub dimensions: Vec<StrategyDimension>,
    #[serde(default)]
    pub missing_dimensions: Vec<String>,
    #[serde(default)]
    pub weak_dimensions: Vec<String>,
    #[serde(default)]
    pub recommendation: Option<StrategyRecommendation>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub suggested_followup_queries: Vec<String>,
    pub decision: EvalDecision,
    #[serde(default)]
    pub next_actions: Vec<NextAction>,
    #[serde(default)]
    pub reasoning: String,
}
```

Note: `reason` becomes `Option<String>` for backwards compat; `reasoning` is the new canonical field. `recommendation` becomes `Option`. `suggested_followup_queries` keeps `default`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p app --lib rag_prompts::tests::rag_strategy_evaluation`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/rag_prompts.rs
git commit -m "feat(eval): extend RagStrategyEvaluation with decision + next_actions

Coexistence mode: old recommendation/reason fields kept as Option/default
for backwards compatibility. New decision/next_actions/reasoning are canonical.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 3: Extend SearchStrategyEvaluation with new fields

**Files:**
- Modify: `crates/app/src/rag_prompts.rs:60-72`
- Test: `crates/app/src/rag_prompts.rs` (tests module)

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn search_strategy_evaluation_has_decision_and_next_actions() {
    let eval: SearchStrategyEvaluation = serde_json::from_str(r#"{
        "decision": "insufficient",
        "next_actions": [
            {"type": "tool_call", "tool": "web_search", "args": {}, "reason": "try news"}
        ],
        "reasoning": "need vertical escalation"
    }"#).unwrap();
    assert!(matches!(eval.decision, EvalDecision::Insufficient));
    assert_eq!(eval.next_actions.len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p app --lib rag_prompts::tests::search_strategy_evaluation_has_decision`
Expected: FAIL

- [ ] **Step 3: Update SearchStrategyEvaluation struct**

Replace lines 60-72 with:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchStrategyEvaluation {
    #[serde(default)]
    pub dimensions: Vec<StrategyDimension>,
    #[serde(default)]
    pub missing_dimensions: Vec<String>,
    #[serde(default)]
    pub weak_dimensions: Vec<String>,
    #[serde(default)]
    pub recommendation: Option<SearchStrategyRecommendation>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub suggested_followup_queries: Vec<String>,
    pub decision: EvalDecision,
    #[serde(default)]
    pub next_actions: Vec<NextAction>,
    #[serde(default)]
    pub reasoning: String,
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p app --lib rag_prompts::tests::search_strategy_evaluation`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/rag_prompts.rs
git commit -m "feat(eval): extend SearchStrategyEvaluation with decision + next_actions

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 4: Update rag.rs build_eval_system_prompt to inject tool catalog

**Files:**
- Modify: `crates/app/src/agents/strategy/rag.rs:1076-1082`

- [ ] **Step 1: Write failing test**

Add to the existing `#[cfg(test)] mod tests` in rag.rs:

```rust
#[test]
fn build_eval_system_prompt_contains_tool_catalog() {
    let prompt = super::build_eval_system_prompt("rag");
    assert!(prompt.contains("Available Tools for Replanning"));
    assert!(prompt.contains("dense_retrieval"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p app --lib strategy::rag::tests::build_eval_system_prompt_contains_tool_catalog`
Expected: FAIL

- [ ] **Step 3: Update build_eval_system_prompt**

Replace lines 1076-1082 with:

```rust
fn build_eval_system_prompt(strategy: &str) -> String {
    let registry = PromptRegistry::standard_cached();
    let skill_body = registry
        .skill("rag-eval")
        .map(|s| s.system_prompt().to_string())
        .unwrap_or_default();

    let cap_registry = crate::agents::capability::CapabilityRegistry::standard_cached();
    let plan_tools = cap_registry.plan_tools(strategy);
    let tool_catalog = plan_tools
        .iter()
        .map(|t| format!("- {} (v{}): {}", t.id, t.version, t.description))
        .collect::<Vec<_>>()
        .join("\n");

    if tool_catalog.is_empty() {
        skill_body
    } else {
        format!("{skill_body}\n\n---\n\n## Available Tools for Replanning\n\n{tool_catalog}")
    }
}
```

- [ ] **Step 4: Update the call site**

Find the call to `build_eval_system_prompt()` in step_evaluate (around line 618) and update it:

```rust
let eval_system = build_eval_system_prompt("rag");
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p app --lib strategy::rag::tests::build_eval_system_prompt_contains_tool_catalog`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/agents/strategy/rag.rs
git commit -m "feat(rag): eval system prompt includes tool catalog

Evaluate now sees available tools for replanning, enabling informed
NextAction::ToolCall decisions.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 5: Update search.rs build_eval_system_prompt to inject tool catalog

**Files:**
- Modify: `crates/app/src/agents/strategy/search.rs:1238-1244`

- [ ] **Step 1: Write failing test**

Add to the existing `#[cfg(test)] mod tests` in search.rs:

```rust
#[test]
fn build_eval_system_prompt_contains_tool_catalog() {
    let prompt = super::build_eval_system_prompt("search");
    assert!(prompt.contains("Available Tools for Replanning"));
    assert!(prompt.contains("web_search"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p app --lib strategy::search::tests::build_eval_system_prompt_contains_tool_catalog`
Expected: FAIL

- [ ] **Step 3: Update build_eval_system_prompt**

Replace lines 1238-1244 with:

```rust
fn build_eval_system_prompt(strategy: &str) -> String {
    let registry = PromptRegistry::standard_cached();
    let skill_body = registry
        .skill("search-eval")
        .map(|s| s.system_prompt().to_string())
        .unwrap_or_default();

    let cap_registry = crate::agents::capability::CapabilityRegistry::standard_cached();
    let plan_tools = cap_registry.plan_tools(strategy);
    let tool_catalog = plan_tools
        .iter()
        .map(|t| format!("- {} (v{}): {}", t.id, t.version, t.description))
        .collect::<Vec<_>>()
        .join("\n");

    if tool_catalog.is_empty() {
        skill_body
    } else {
        format!("{skill_body}\n\n---\n\n## Available Tools for Replanning\n\n{tool_catalog}")
    }
}
```

- [ ] **Step 4: Update the call site**

Find the call to `build_eval_system_prompt()` in step_evaluate (around line 722) and update it:

```rust
let eval_system = build_eval_system_prompt("search");
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p app --lib strategy::search::tests::build_eval_system_prompt_contains_tool_catalog`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/agents/strategy/search.rs
git commit -m "feat(search): eval system prompt includes tool catalog

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 6: Update rag.rs step_evaluate to consume new EvaluateOutput

**Files:**
- Modify: `crates/app/src/agents/strategy/rag.rs:640-712`

- [ ] **Step 1: Update the decision label mapping**

Replace lines 640-644 with:

```rust
let label = match eval.decision {
    crate::rag_prompts::EvalDecision::Sufficient => "sufficient".to_string(),
    crate::rag_prompts::EvalDecision::Insufficient => "insufficient".to_string(),
    crate::rag_prompts::EvalDecision::GiveUp => "give_up".to_string(),
};
```

- [ ] **Step 2: Update the strategy_advice match arm**

Replace lines 676-712 (the `match strategy_advice` block's inner `match eval.recommendation`) with:

```rust
Some((eval, _)) => match eval.decision {
    crate::rag_prompts::EvalDecision::Sufficient => {
        Ok(StepOutcome::Next(Box::new(RagState::Answer)))
    }
    crate::rag_prompts::EvalDecision::GiveUp => {
        self.finalize_degrade(ctx, DegradeReason::NoResultsAfterAllFallbacks)
            .await
            .map(StepOutcome::Terminate)
    }
    crate::rag_prompts::EvalDecision::Insufficient => {
        let mut sub_queries = Vec::new();
        let mut tool_hints = Vec::new();
        for action in &eval.next_actions {
            match action {
                crate::rag_prompts::NextAction::SubQuery { query } => {
                    sub_queries.push(query.clone());
                }
                crate::rag_prompts::NextAction::ToolCall { tool, args, reason } => {
                    tool_hints.push(format!(
                        "{tool}: {} ({reason})",
                        serde_json::to_string(args).unwrap_or_default()
                    ));
                }
            }
        }

        let mut directive_parts = vec![format!("replan: {}", eval.reasoning)];
        if !tool_hints.is_empty() {
            directive_parts.push(format!(
                "suggested tools: {}",
                tool_hints.join(", ")
            ));
        }
        if let Some(hint) = build_doc_index_directive_hint(&tool_results) {
            directive_parts.push(hint);
        }

        ctx.iteration_params = RagIterationParams {
            query: original_query.clone(),
            directive: Some(directive_parts.join("\n")),
            suggested_queries: sub_queries,
        };
        Ok(StepOutcome::Next(Box::new(RagState::Plan)))
    }
},
```

- [ ] **Step 3: Update the fallback reasoning field**

In the same function, find the line that reads `reasoning` from the JSON (around line 670-672):

```rust
.reasoning: llm_eval_json
    .and_then(|v| v.get("reason").and_then(|r| r.as_str().map(|s| s.to_string())))
    .unwrap_or_default(),
```

Update to try `reasoning` first, then fall back to `reason`:

```rust
reasoning: llm_eval_json
    .and_then(|v| v.get("reasoning").and_then(|r| r.as_str().map(|s| s.to_string())))
    .or_else(|| llm_eval_json.as_ref().and_then(|v| v.get("reason").and_then(|r| r.as_str().map(|s| s.to_string()))))
    .unwrap_or_default(),
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p app --lib strategy::rag`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/agents/strategy/rag.rs
git commit -m "feat(rag): step_evaluate consumes new EvaluateOutput

Match on eval.decision (Sufficient/Insufficient/GiveUp) instead of
old recommendation enum. ToolCall hints passed to Plan via directive.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 7: Update search.rs map_search_strategy_to_advice to consume new EvaluateOutput

**Files:**
- Modify: `crates/app/src/agents/strategy/search.rs:1331-1353`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn map_search_strategy_sufficient_maps_to_synthesize() {
    let eval = crate::rag_prompts::SearchStrategyEvaluation {
        dimensions: vec![],
        missing_dimensions: vec![],
        weak_dimensions: vec![],
        recommendation: None,
        reason: None,
        suggested_followup_queries: vec![],
        decision: crate::rag_prompts::EvalDecision::Sufficient,
        next_actions: vec![],
        reasoning: "all covered".to_string(),
    };
    let advice = super::map_search_strategy_to_advice(&eval, None);
    assert!(matches!(advice, super::EvalAdvice::Synthesize));
}

#[test]
fn map_search_strategy_give_up_maps_to_degrade() {
    let eval = crate::rag_prompts::SearchStrategyEvaluation {
        dimensions: vec![],
        missing_dimensions: vec![],
        weak_dimensions: vec![],
        recommendation: None,
        reason: None,
        suggested_followup_queries: vec![],
        decision: crate::rag_prompts::EvalDecision::GiveUp,
        next_actions: vec![],
        reasoning: "no results".to_string(),
    };
    let advice = super::map_search_strategy_to_advice(&eval, None);
    assert!(matches!(advice, super::EvalAdvice::Degrade { .. }));
}

#[test]
fn map_search_strategy_tool_call_web_search_maps_to_escalate_vertical() {
    let eval = crate::rag_prompts::SearchStrategyEvaluation {
        dimensions: vec![],
        missing_dimensions: vec![],
        weak_dimensions: vec![],
        recommendation: None,
        reason: None,
        suggested_followup_queries: vec![],
        decision: crate::rag_prompts::EvalDecision::Insufficient,
        next_actions: vec![crate::rag_prompts::NextAction::ToolCall {
            tool: "web_search".to_string(),
            args: serde_json::json!({}),
            reason: "try news".to_string(),
        }],
        reasoning: "need vertical".to_string(),
    };
    let advice = super::map_search_strategy_to_advice(&eval, None);
    assert!(matches!(advice, super::EvalAdvice::EscalateVertical { .. }));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p app --lib strategy::search::tests::map_search_strategy_sufficient`
Expected: FAIL — struct fields don't match new schema

- [ ] **Step 3: Update map_search_strategy_to_advice**

Replace lines 1331-1353 with:

```rust
fn map_search_strategy_to_advice(
    eval: &crate::rag_prompts::SearchStrategyEvaluation,
    current_vertical: Option<&str>,
) -> EvalAdvice {
    match eval.decision {
        crate::rag_prompts::EvalDecision::Sufficient => EvalAdvice::Synthesize,
        crate::rag_prompts::EvalDecision::GiveUp => EvalAdvice::Degrade {
            reason: DegradeReason::NoResultsAfterAllFallbacks,
        },
        crate::rag_prompts::EvalDecision::Insufficient => {
            let has_vertical_hint = eval.next_actions.iter().any(|a| {
                matches!(a, crate::rag_prompts::NextAction::ToolCall { tool, .. } if tool == "web_search")
            });
            if has_vertical_hint && next_vertical_step(current_vertical).is_some() {
                EvalAdvice::EscalateVertical {
                    reason: "llm_strategy_escalate_vertical".to_string(),
                }
            } else {
                EvalAdvice::Replan {
                    reason: eval.reasoning.clone(),
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p app --lib strategy::search::tests::map_search_strategy`
Expected: PASS (all 3 tests)

- [ ] **Step 5: Update llm_suggested to read next_actions**

Find the line around 732-735 that reads `suggested_followup_queries`:

```rust
let llm_suggested = strategy_eval
    .as_ref()
    .map(|(e, _)| e.suggested_followup_queries.clone())
    .unwrap_or_default();
```

Replace with:

```rust
let llm_suggested = strategy_eval
    .as_ref()
    .map(|(e, _)| {
        e.next_actions
            .iter()
            .filter_map(|a| match a {
                crate::rag_prompts::NextAction::SubQuery { query } => Some(query.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
```

- [ ] **Step 6: Run all search tests**

Run: `cargo test -p app --lib strategy::search`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/app/src/agents/strategy/search.rs
git commit -m "feat(search): map_search_strategy_to_advice consumes EvaluateOutput

Match on eval.decision. ToolCall(web_search) → EscalateVertical.
SubQuery actions → llm_suggested queries.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 8: Update eval skill files with new JSON schema

**Files:**
- Modify: `prompts/skills/rag-eval/SKILL.md`
- Modify: `prompts/skills/search-eval/SKILL.md`

- [ ] **Step 1: Update rag-eval/SKILL.md JSON schema section**

Find the JSON schema block (around lines 22-40) and replace it with:

```json
{
  "dimensions": [
    {
      "name": "dimension name",
      "attempted": true,
      "covered": true,
      "retrieved_count": 0,
      "query_ids": ["q1"],
      "status": "covered_strong"
    }
  ],
  "missing_dimensions": ["name1", "name2"],
  "weak_dimensions": ["name3"],
  "decision": "sufficient" | "insufficient" | "give_up",
  "next_actions": [
    {"type": "sub_query", "query": "follow-up query"} |
    {"type": "tool_call", "tool": "tool_id", "args": {}, "reason": "why this tool"}
  ],
  "reasoning": "one-sentence explanation"
}
```

Update the field definitions section:
- Remove `recommendation` and `suggested_followup_queries` definitions
- Add: `decision` must be exactly one of: `"sufficient"`, `"insufficient"`, `"give_up"`
- Add: `next_actions` is a list of actions. Each action has `"type"`: `"sub_query"` (with `"query"`) or `"tool_call"` (with `"tool"`, `"args"`, `"reason"`)
- Add: `reasoning` is a one-sentence explanation

Update the recommendation rules section to use decision rules:
- `"sufficient"` when all major dimensions are at least covered_weak and none are missing
- `"insufficient"` when one or more major dimensions are missing or weak
- `"give_up"` when retrieval has been attempted multiple times with no improvement and budget is nearly exhausted

Update the follow-up rules to use next_actions rules:
- Only provide `next_actions` when decision is `"insufficient"`
- Use `"sub_query"` for new queries targeting missing dimensions
- Use `"tool_call"` when switching to a different retrieval tool would help (e.g., from dense_retrieval to graph_retrieval)
- Leave `next_actions` empty when decision is `"sufficient"` or `"give_up"`

Update all three examples to use the new schema (replace `"recommendation"` with `"decision"`, `"suggested_followup_queries"` with `"next_actions"`).

- [ ] **Step 2: Update search-eval/SKILL.md similarly**

Apply the same schema changes. The search-eval skill has its own examples and field definitions — update them all consistently.

- [ ] **Step 3: Commit**

```bash
git add prompts/skills/rag-eval/SKILL.md prompts/skills/search-eval/SKILL.md
git commit -m "docs(skills): update eval skill schemas to EvaluateOutput

Replace recommendation + suggested_followup_queries with
decision + next_actions + reasoning.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 9: Delete legacy dead code from prompts.rs

**Files:**
- Modify: `crates/app/src/agents/strategy/prompts.rs:95-146`

- [ ] **Step 1: Verify no callers remain**

Run: `grep -rn "plan_tools()\|format_skills()" crates/app/src/ | grep -v "test" | grep -v "pub fn"`
Expected: no output (no callers)

- [ ] **Step 2: Delete chat module helpers**

Remove lines 95-106 (plan_tools and format_skills in chat module).

- [ ] **Step 3: Delete rag module helpers**

Remove lines 117-128 (plan_tools and format_skills in rag module).

- [ ] **Step 4: Delete search module helpers**

Remove lines 139-145 (plan_tools and format_skills in search module).

- [ ] **Step 5: Delete find_tool helper**

Check if `find_tool` (line 148) is still used anywhere. If not, delete it.

- [ ] **Step 6: Remove unused imports**

Check if `Tool` import is still needed in prompts.rs. If not, remove it.

- [ ] **Step 7: Verify compilation and tests**

Run: `cargo check -p app`
Run: `cargo test -p app --lib`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/app/src/agents/strategy/prompts.rs
git commit -m "refactor(prompts): remove legacy plan_tools/format_skills helpers

Replaced by CapabilityRegistry::plan_tools(strategy) and
answer_format_skills(strategy) in previous tasks.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 10: Full test suite verification

**Files:** None (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cargo test -p app --lib`
Expected: All 420+ tests pass

- [ ] **Step 2: Verify eval prompt contains tool catalog**

Run: `cargo test -p app --lib build_eval_system_prompt_contains_tool_catalog -- --nocapture`
Expected: Both rag and search tests pass

- [ ] **Step 3: Verify EvaluateOutput serialization**

Run: `cargo test -p app --lib eval_decision next_action -- --nocapture`
Expected: All serialization tests pass

- [ ] **Step 4: Final commit (if any fixups needed)**

```bash
git add -A
git commit -m "chore: verify ActivationPhase gaps fix

All tests pass.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Summary

**Total tasks:** 10

**Dependency order:**
- Task 1 (types) → Task 2 (RAG eval struct) → Task 3 (Search eval struct)
- Task 4 (rag eval prompt) + Task 5 (search eval prompt) — independent of 2-3
- Task 6 (rag step_evaluate) — depends on Tasks 2, 4
- Task 7 (search step_evaluate) — depends on Tasks 3, 5
- Task 8 (skill files) — independent
- Task 9 (dead code) — independent
- Task 10 (verification) — all above

**Estimated effort:** 1-2 hours

**Key deliverables:**
1. `EvalDecision` + `NextAction` types
2. Extended `RagStrategyEvaluation` + `SearchStrategyEvaluation` (coexistence mode)
3. Eval system prompts with tool catalog
4. Strategy code consuming new EvaluateOutput
5. Updated eval skill schemas
6. Cleaned dead code
