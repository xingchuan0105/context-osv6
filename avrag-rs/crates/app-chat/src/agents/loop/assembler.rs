use super::config::ModeConfig;
use super::disclosure_plan::{DisclosurePlanner, DisclosureRenderer, parse_synthesis_choices};
use crate::agents::capability::CapabilityRegistry;
use crate::agents::runtime::AgentRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopPhase {
    Retrieve,
    Synthesis,
}

#[derive(Debug, Clone, Default)]
pub struct DisclosedState {
    pub disclosed_skill_ids: std::collections::HashSet<String>,
    pub last_skill_request: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct AssembledContext {
    pub system_content: String,
    pub tools: Vec<contracts::ToolSpec>,
    pub newly_disclosed_skills: Vec<String>,
}

pub struct ContextAssembler;

impl ContextAssembler {
    pub fn assemble_retrieve(
        iteration: u8,
        max_iterations: u8,
        mode: &ModeConfig,
        request: &AgentRequest,
        registry: &CapabilityRegistry,
        disclosed: &mut DisclosedState,
        last_assistant_content: Option<&str>,
    ) -> AssembledContext {
        let _ = last_assistant_content;
        let base = super::config::load_system_prompt(&mode.system_prompt_base).unwrap_or_default();
        let first_round = iteration == 0;

        let skill_request = disclosed.last_skill_request.as_deref();
        let plan = DisclosurePlanner::plan_retrieve(
            mode,
            first_round,
            skill_request,
            &disclosed.disclosed_skill_ids,
        );
        let renderer = DisclosureRenderer::new(registry);
        let rendered = renderer.render(&plan, mode, request, disclosed);

        let tools = if memory_cluster_disclosed(disclosed) {
            let mut merged = mode.tools_for_retrieve(registry);
            merged.extend(mode.resolve_tool_specs(
                registry,
                &[
                    "conversation_history_load".to_string(),
                    "user_profile_load".to_string(),
                ],
            ));
            dedupe_tools(merged)
        } else {
            mode.tools_for_retrieve(registry)
        };

        let budget_hint = build_iteration_budget_hint(iteration, max_iterations);
        let system_content = if rendered.text.is_empty() {
            format!("{base}\n\n{budget_hint}")
        } else {
            format!("{base}\n\n{}\n\n{budget_hint}", rendered.text)
        };

        AssembledContext {
            system_content,
            tools,
            newly_disclosed_skills: rendered.newly_disclosed,
        }
    }

    pub fn assemble_synthesis(
        mode: &ModeConfig,
        request: &AgentRequest,
        registry: &CapabilityRegistry,
        disclosed: &mut DisclosedState,
    ) -> AssembledContext {
        let base = super::config::load_system_prompt(&mode.system_prompt_base).unwrap_or_default();
        let mut hint_parts = Vec::new();

        if let Some(hint) = request.format_hint.as_deref() {
            hint_parts.push(format!(
                "\n<format_hint>\nUser prefers format skill: {hint}. You may still choose a different format if inappropriate.\n</format_hint>"
            ));
        }

        if let Some(hint) = request
            .metadata
            .get("writing_hint")
            .and_then(|v| v.as_str())
        {
            hint_parts.push(format!(
                "\n<writing_hint>\nUser prefers writing style: {hint}. You may override if inappropriate.\n</writing_hint>"
            ));
        }

        let choices = parse_synthesis_choices(request);
        let plan = DisclosurePlanner::plan_synthesis(
            mode,
            request,
            &choices,
            &disclosed.disclosed_skill_ids,
        );
        let renderer = DisclosureRenderer::new(registry);
        let rendered = renderer.render(&plan, mode, request, disclosed);

        let mut parts = vec![base];
        if !rendered.text.is_empty() {
            parts.push(rendered.text);
        }
        parts.extend(hint_parts);

        AssembledContext {
            system_content: parts.join("\n\n"),
            tools: vec![],
            newly_disclosed_skills: rendered.newly_disclosed,
        }
    }
}

fn memory_cluster_disclosed(disclosed: &DisclosedState) -> bool {
    disclosed
        .disclosed_skill_ids
        .iter()
        .any(|key| key == "memory" || key.starts_with("memory:"))
}

fn dedupe_tools(tools: Vec<contracts::ToolSpec>) -> Vec<contracts::ToolSpec> {
    let mut seen = std::collections::BTreeSet::new();
    tools
        .into_iter()
        .filter(|tool| seen.insert(tool.name.clone()))
        .collect()
}

/// Build the per-round iteration budget hint injected into every retrieve-phase
/// system prompt. `iteration` is 0-indexed; `round` and `remaining` are derived
/// for LLM consumption.
///
/// Format: `<iteration_budget round="1" max="4" remaining="3" />`
pub fn build_iteration_budget_hint(iteration: u8, max_iterations: u8) -> String {
    let round = iteration + 1;
    let remaining = max_iterations.saturating_sub(round);
    format!(
        "<iteration_budget round=\"{round}\" max=\"{max_iterations}\" remaining=\"{remaining}\" />"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rag_retrieve_tools_empty_until_memory_cluster_disclosed() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let registry = CapabilityRegistry::standard_cached();
        assert!(mode.tools_for_retrieve(registry).is_empty());

        let mut disclosed = DisclosedState::default();
        disclosed.disclosed_skill_ids.insert("memory".to_string());
        let tools = mode.resolve_tool_specs(
            registry,
            &[
                "conversation_history_load".to_string(),
                "user_profile_load".to_string(),
            ],
        );
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["conversation_history_load", "user_profile_load"]
        );
    }

    #[test]
    fn rag_round_zero_discloses_codegen_bundle() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let registry = CapabilityRegistry::standard_cached();
        let mut disclosed = DisclosedState::default();
        let ctx = ContextAssembler::assemble_retrieve(
            0,
            4,
            &mode,
            &crate::agents::runtime::AgentRequest {
                kind: crate::agents::AgentKind::Rag,
                query: "test".to_string(),
                notebook_id: None,
                session_id: None,
                doc_scope: vec![],
                messages: vec![],
                user_preferences: None,
                debug: false,
                stream: false,
                language: None,
                auth_context: serde_json::json!({}),
                docscope_metadata: None,
                metadata: Default::default(),
                cancellation_token: None,
                guard_pipeline: None,
                preferred_tools: vec![],
                format_hint: None,
                max_iterations: None,
            },
            &registry,
            &mut disclosed,
            None,
        );
        assert!(ctx.system_content.contains("dense_search"));
        assert!(!ctx.system_content.contains("rag-codegen-guide"));
        assert!(ctx.system_content.contains("Retrieval query: test"));
        assert!(
            ctx.system_content
                .contains("<iteration_budget round=\"1\" max=\"4\" remaining=\"3\" />")
        );
        assert_eq!(
            ctx.tools.len(),
            0,
            "round0 must not expose memory tools until memory cluster is disclosed"
        );
    }

    #[test]
    fn rag_round_one_re_injects_codegen_skill() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let registry = CapabilityRegistry::standard_cached();
        let mut disclosed = DisclosedState::default();
        disclosed.disclosed_skill_ids.insert("codegen".to_string());
        let request = crate::agents::runtime::AgentRequest {
            kind: crate::agents::AgentKind::Rag,
            query: "test".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            auth_context: serde_json::json!({}),
            docscope_metadata: None,
            metadata: Default::default(),
            cancellation_token: None,
            guard_pipeline: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
        };
        let ctx = ContextAssembler::assemble_retrieve(
            1,
            4,
            &mode,
            &request,
            &registry,
            &mut disclosed,
            None,
        );
        assert!(
            ctx.system_content.contains("dense_search"),
            "iteration 1 must still include codegen SDK signatures"
        );
        assert!(
            !ctx.system_content.contains("Retrieval query:"),
            "retrieval query injection is first-round only"
        );
    }

    #[test]
    fn search_round_zero_exposes_configured_tool_pool() {
        let mode = super::super::config::load_mode_config("search").unwrap();
        let registry = CapabilityRegistry::standard_cached();
        let mut disclosed = DisclosedState::default();
        let ctx = ContextAssembler::assemble_retrieve(
            0,
            4,
            &mode,
            &crate::agents::runtime::AgentRequest {
                kind: crate::agents::AgentKind::Search,
                query: "latest rust release".to_string(),
                notebook_id: None,
                session_id: None,
                doc_scope: vec![],
                messages: vec![],
                user_preferences: None,
                debug: false,
                stream: false,
                language: None,
                auth_context: serde_json::json!({}),
                docscope_metadata: None,
                metadata: Default::default(),
                cancellation_token: None,
                guard_pipeline: None,
                preferred_tools: vec![],
                format_hint: None,
                max_iterations: None,
            },
            registry,
            &mut disclosed,
            None,
        );

        let names: Vec<&str> = ctx.tools.iter().map(|tool| tool.name.as_str()).collect();
        assert!(names.contains(&"web_search"));
        assert!(names.contains(&"web_fetch"));
    }

    #[test]
    fn rag_retrieve_attaches_memory_tools_after_skill_request_disclosure() {
        let mode = super::super::config::load_mode_config("rag").unwrap();
        let registry = CapabilityRegistry::standard_cached();
        let mut disclosed = DisclosedState::default();
        disclosed.last_skill_request = Some(vec!["memory".to_string()]);
        let ctx = ContextAssembler::assemble_retrieve(
            1,
            4,
            &mode,
            &crate::agents::runtime::AgentRequest {
                kind: crate::agents::AgentKind::Rag,
                query: "test".to_string(),
                notebook_id: None,
                session_id: None,
                doc_scope: vec![],
                messages: vec![],
                user_preferences: None,
                debug: false,
                stream: false,
                language: None,
                auth_context: serde_json::json!({}),
                docscope_metadata: None,
                metadata: Default::default(),
                cancellation_token: None,
                guard_pipeline: None,
                preferred_tools: vec![],
                format_hint: None,
                max_iterations: None,
            },
            &registry,
            &mut disclosed,
            None,
        );
        assert!(ctx.system_content.contains("memory"));
        assert_eq!(ctx.tools.len(), 2);
        assert_eq!(ctx.tools[0].name, "conversation_history_load");
        assert_eq!(ctx.tools[1].name, "user_profile_load");
    }

    #[test]
    fn parse_skill_request_rejects_heuristic_phrases() {
        use crate::agents::r#loop::skill_request::parse_skill_request;
        assert!(parse_skill_request("请求 **codegen**").is_empty());
        assert!(parse_skill_request("request cluster `codegen`").is_empty());
    }
}
