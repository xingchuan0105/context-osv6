//! Reusable assertion helpers for E2E tests.

use app::agents::capability::{CapabilityRegistry, StrategySchema};
use app::agents::progressive::PromptRegistry;
use app::agents::runtime::StateRecord;

use super::recording_llm::LlmCall;

/// Map lowercase state_id to PascalCase schema state name.
fn to_schema_state(state_id: &str) -> String {
    match state_id {
        "plan" => "Plan".to_string(),
        "execute_atomic" => "ExecuteAtomic".to_string(),
        "execute_retrieve" => "ExecuteRetrieve".to_string(),
        "evaluate" => "Evaluate".to_string(),
        "answer" => "Answer".to_string(),
        "decompose" => "Decompose".to_string(),
        "parallel_search" => "ParallelSearch".to_string(),
        "aggregate" => "Aggregate".to_string(),
        other => other.to_string(), // pass through unknown
    }
}

/// Assert that state transitions match the schema.
pub fn assert_valid_transitions(schema: &StrategySchema, history: &[StateRecord]) {
    assert!(
        history.len() >= 2,
        "Expected at least 2 states in history, got {}",
        history.len()
    );

    for window in history.windows(2) {
        let from = to_schema_state(&window[0].state_id);
        let to = to_schema_state(&window[1].state_id);
        let valid = schema
            .transitions
            .iter()
            .any(|t| t.from == from && t.to == to);
        assert!(
            valid,
            "Invalid state transition: {} → {} (not in schema for strategy '{}')",
            from, to, schema.id
        );
    }
}

/// Assert that a prompt contains the expected skill body.
pub fn assert_prompt_contains_skill(prompt: &str, skill_id: &str) {
    let registry = PromptRegistry::standard_cached();
    let skill = registry
        .skill(skill_id)
        .unwrap_or_else(|| panic!("Skill '{}' not found in registry", skill_id));
    let body = skill.system_prompt();
    assert!(
        prompt.contains(body),
        "Prompt does not contain skill '{}' body. Expected {} chars, prompt is {} chars.",
        skill_id,
        body.len(),
        prompt.len()
    );
}

/// Assert that a prompt contains tool catalog entries for the given strategy.
///
/// Checks all three tiers of progressive disclosure:
/// - Tier 1 (Index): tool name header `### tool_name (v1.0)`
/// - Tier 1 (Index): tool description
/// - Tier 3 (Schema): parameter types from input_schema
pub fn assert_prompt_has_tool_catalog(prompt: &str, strategy: &str) {
    let registry = CapabilityRegistry::standard_cached();
    let plan_tools = registry.plan_tools(strategy);

    assert!(
        !plan_tools.is_empty(),
        "No plan tools registered for strategy '{}'",
        strategy
    );

    for tool in plan_tools {
        // Tier 1: Index — tool name and version
        let header = format!("### {} (v{})", tool.id, tool.version);
        assert!(
            prompt.contains(&header),
            "Prompt missing tool catalog header: {}",
            header
        );
        assert!(
            prompt.contains(&tool.description),
            "Prompt missing tool description for '{}'",
            tool.id
        );

        // Tier 3: Schema — parameters
        if let Some(props) = tool
            .input_schema
            .get("properties")
            .and_then(|p| p.as_object())
        {
            assert!(
                prompt.contains("Parameters:"),
                "Prompt missing 'Parameters:' section for tool '{}'",
                tool.id
            );
            for (name, schema_val) in props {
                if let Some(ty) = schema_val.get("type").and_then(|t| t.as_str()) {
                    let param_line = format!("{}: {}", name, ty);
                    assert!(
                        prompt.contains(&param_line),
                        "Prompt missing parameter '{}' for tool '{}'",
                        param_line,
                        tool.id
                    );
                }
            }
        }
    }
}

/// Assert that a prompt contains the format skills catalog (Answer phase).
pub fn assert_prompt_has_format_skills(prompt: &str) {
    assert!(
        prompt.contains("## Available Output Formats"),
        "Prompt missing '## Available Output Formats' section"
    );
    for skill_id in [
        "ppt-generation",
        "html-renderer",
        "teaching",
        "framework-extraction",
    ] {
        assert!(
            prompt.contains(skill_id),
            "Prompt missing format skill '{}'",
            skill_id
        );
    }
}

/// Assert state kinds match expected values for known state IDs.
pub fn assert_state_kinds(history: &[StateRecord]) {
    for record in history {
        let expected_kind = match record.state_id.as_str() {
            "plan" | "decompose" => "Plan",
            "execute_atomic" | "execute_retrieve" | "parallel_search" => "Execute",
            "evaluate" => "Evaluate",
            "aggregate" => "Control",
            "answer" => "Answer",
            _ => continue,
        };
        assert_eq!(
            record.state_kind, expected_kind,
            "State '{}' has kind '{}', expected '{}'",
            record.state_id, record.state_kind, expected_kind
        );
    }
}

/// Assert budget usage is within expected range.
pub fn assert_budget_usage(budget_used: u8, max_expected: u8) {
    assert!(
        budget_used <= max_expected,
        "Budget used {} exceeds max expected {}",
        budget_used,
        max_expected
    );
}

/// Find LLM call for a specific state (by matching user message content).
pub fn find_llm_call_for_state<'a>(calls: &'a [LlmCall], state_hint: &str) -> Option<&'a LlmCall> {
    calls.iter().find(|c| {
        c.user_messages
            .iter()
            .any(|m| m.content.contains(state_hint))
    })
}

/// Assert that a strategy's skill body does NOT appear in calls for other strategies.
pub fn assert_strategy_isolation(calls: &[LlmCall], strategy: &str, skill_id: &str) {
    let registry = PromptRegistry::standard_cached();
    let skill_body = registry
        .skill(skill_id)
        .map(|s| s.system_prompt().to_string())
        .unwrap_or_default();

    if skill_body.is_empty() {
        return;
    }

    for (i, call) in calls.iter().enumerate() {
        // We can't directly know which strategy a call belongs to, so we check
        // that the skill body only appears in calls that seem to belong to this strategy.
        // This is a best-effort check — if the skill body appears in a call that
        // doesn't look like it belongs to this strategy, that's a leak.
        if call.system_prompt.contains(&skill_body) {
            // Verify this call seems to belong to the expected strategy
            // by checking for other strategy markers in the prompt
            let other_strategies = ["chat", "rag", "search"]
                .iter()
                .filter(|&&s| s != strategy)
                .collect::<Vec<_>>();

            for &other in &other_strategies {
                let other_skill = format!("{}-plan", other);
                if let Some(other_skill_obj) = registry.skill(&other_skill) {
                    let other_body = other_skill_obj.system_prompt();
                    assert!(
                        !call.system_prompt.contains(other_body),
                        "LLM call {} contains skill bodies for both '{}' and '{}' strategies",
                        i,
                        skill_id,
                        other_skill
                    );
                }
            }
        }
    }
}
