use super::config::ModeConfig;
use super::disclosure_plan::{DisclosurePlanner, DisclosureRenderer, parse_synthesis_choices};
use crate::runtime::AgentRequest;
use agent_tools::capability::CapabilityRegistry;

/// 用户偏好提示模板（prompts/system/hints/，第三人称观察式，{hint} 运行时替换）。
const FORMAT_HINT_TMPL: &str = include_str!("../../../../prompts/system/hints/format-hint.md");
const WRITING_HINT_TMPL: &str = include_str!("../../../../prompts/system/hints/writing-hint.md");

fn subst_hint(template: &str, hint: &str) -> String {
    template.replace("{hint}", hint)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopPhase {
    Retrieve,
    Synthesis,
}

#[derive(Debug, Clone, Default)]
pub struct DisclosedState {
    pub disclosed_skill_ids: std::collections::HashSet<String>,
    pub last_skill_request: Option<Vec<String>>,
    /// Snapshot for `<loop_budget … tokens_* />` (set each retrieve assemble).
    pub tokens_used_hint: Option<u32>,
    pub tokens_max_hint: Option<u32>,
    /// When true, next retrieve assemble re-discloses knowledge-base/api-detail
    /// (L2 empty-result tables + success shapes). Set on sandbox error; consumed
    /// by [`ContextAssembler::assemble_retrieve`].
    pub reexpose_kb_api_detail: bool,
}

#[derive(Debug, Clone)]
pub struct AssembledContext {
    pub system_content: String,
    pub tools: Vec<contracts::ToolSpec>,
    pub newly_disclosed_skills: Vec<String>,
    /// Per-round budget hint (`<loop_budget .../>`). Injected by the caller as a
    /// trailing user message, NOT in system_content, to keep the system prefix
    /// stable for provider prefix caching (P0, 2026-07-30).
    pub budget_hint: String,
}

pub struct ContextAssembler;

/// Path of the session base (identity, user channel, BASE tools).
const AGENT_BASE_PATH: &str = "prompts/system/agent-base.md";
/// Lead grounded-synthesis voice (Lead+Workers product modes).
const LEAD_BASE_PATH: &str = "prompts/system/lead-base.md";

/// Load system prompt base: if `request.metadata.system_prompt_parts` is a non-empty
/// array of path strings, load each and join with `\n\n---\n\n`; otherwise use
/// `mode.system_prompt_base`.
fn load_assembled_system_base(mode: &ModeConfig, request: &AgentRequest) -> String {
    if let Some(parts) = request
        .metadata
        .get("system_prompt_parts")
        .and_then(|v| v.as_array())
    {
        let paths: Vec<&str> = parts.iter().filter_map(|v| v.as_str()).collect();
        if !paths.is_empty() {
            let loaded: Vec<String> = paths
                .iter()
                .filter_map(|p| super::config::load_system_prompt(p).ok())
                .filter(|s| !s.trim().is_empty())
                .collect();
            if !loaded.is_empty() {
                return loaded.join("\n\n---\n\n");
            }
        }
    }
    super::config::load_system_prompt(&mode.system_prompt_base).unwrap_or_default()
}

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
        let base = load_assembled_system_base(mode, request);
        let first_round = iteration == 0;

        // Consume one-shot L2 reexpose (sandbox error recovery).
        let reexpose_kb_api_detail = disclosed.reexpose_kb_api_detail;
        if reexpose_kb_api_detail {
            disclosed
                .disclosed_skill_ids
                .remove("knowledge-base:api-detail");
            disclosed.reexpose_kb_api_detail = false;
        }

        let skill_request = disclosed.last_skill_request.as_deref();
        let plan = DisclosurePlanner::plan_retrieve(
            mode,
            first_round,
            skill_request,
            &disclosed.disclosed_skill_ids,
            Some(request),
            reexpose_kb_api_detail,
        );
        let renderer = DisclosureRenderer::new(registry);
        let rendered = renderer.render(&plan, mode, request, disclosed);

        // D8 (2026-08-02): memory is disclosed every round as prose, and the
        // memory SKILL teaches `client.history` / `client.user_profile` (base
        // SDK primitives, always open in the sandbox). The native memory tools
        // (`conversation_history_load` / `user_profile_load`) are the legacy
        // point-and-click surface; exposing them at round 0 pushed the
        // function-calling model onto the native tool path and away from
        // `<code language="python">` codegen — retrieval never fired. SaC
        // retrieve phases carry only the configured `tool_pool` (empty for
        // rag/search); the model reaches memory + retrieval via the sandbox.
        let tools = mode.tools_for_retrieve(registry);

        let tokens_used = disclosed.tokens_used_hint.unwrap_or(0);
        let tokens_max = disclosed.tokens_max_hint.unwrap_or(0);
        let budget_hint = build_loop_budget_hint(
            iteration,
            max_iterations,
            tokens_used,
            tokens_max,
            mode.budget.baseline_iterations,
        );
        // P0 (2026-07-30): budget_hint moved OUT of system_content into
        // AssembledContext.budget_hint; the caller injects it as a trailing
        // user message so the system + history prefix stays stable across
        // ReAct rounds → DeepSeek/OpenAI prefix cache can hit (was 0% because
        // budget_hint's round/tokens_used change every round broke the prefix).
        let system_content = if rendered.text.is_empty() {
            base
        } else {
            format!("{base}\n\n{}", rendered.text)
        };

        AssembledContext {
            system_content,
            tools,
            newly_disclosed_skills: rendered.newly_disclosed,
            budget_hint,
        }
    }

    pub fn assemble_synthesis(
        mode: &ModeConfig,
        request: &AgentRequest,
        registry: &CapabilityRegistry,
        disclosed: &mut DisclosedState,
    ) -> AssembledContext {
        // Lead+Workers: session base + lead-base (grounded synthesis). Pure chat:
        // agent-base only. Worker method tables stay off synthesis.
        let base = if super::config::is_lead_workers_path(mode) {
            let session = super::config::load_system_prompt(AGENT_BASE_PATH).unwrap_or_default();
            let lead = super::config::load_system_prompt(LEAD_BASE_PATH).unwrap_or_default();
            if lead.is_empty() {
                session
            } else if session.is_empty() {
                lead
            } else {
                format!("{session}\n\n{lead}")
            }
        } else {
            super::config::load_system_prompt(AGENT_BASE_PATH).unwrap_or_else(|_| {
                load_assembled_system_base(mode, request)
            })
        };
        let mut hint_parts = Vec::new();

        if let Some(hint) = request.format_hint.as_deref() {
            hint_parts.push(subst_hint(FORMAT_HINT_TMPL, hint));
        }

        if let Some(hint) = request
            .metadata
            .get("writing_hint")
            .and_then(|v| v.as_str())
        {
            hint_parts.push(subst_hint(WRITING_HINT_TMPL, hint));
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

        // Synthesis phase tools stay empty by design: utility tools (calculator,
        // weather_query, user_context) are exposed on the **retrieve** phase via
        // `ModeConfig.tool_pool` / `tools_for_retrieve`. Option D Answer packs
        // set that pool; do not re-open retrieval/delegate tools here.
        AssembledContext {
            system_content: parts.join("\n\n"),
            tools: vec![],
            newly_disclosed_skills: rendered.newly_disclosed,
            budget_hint: String::new(),
        }
    }
}

/// Soft pace observation when retrieve round exceeds `baseline_rounds`
/// (prompts/loop/budget-pace-over-baseline.tmpl.md). Third-person; not a hard stop.
const BUDGET_PACE_OVER_BASELINE: &str =
    include_str!("../../../../prompts/loop/budget-pace-over-baseline.tmpl.md");

/// Soft near-hard-ceiling observation when remaining rounds ≤ 1.
const BUDGET_PACE_NEAR_CEILING: &str =
    include_str!("../../../../prompts/loop/budget-pace-near-ceiling.tmpl.md");

/// Build the per-round loop budget hint injected into every retrieve-phase
/// system prompt. `iteration` is 0-indexed.
///
/// Tokens are the primary cost signal; rounds are a safety ceiling.
/// `baseline_rounds` is a soft pace (default 2) — not a hard stop.
///
/// Format:
/// `<loop_budget round="1" baseline_rounds="2" max_rounds="12" remaining_rounds="11" tokens_used="0" tokens_max="28000" tokens_remaining="28000" />`
pub fn build_iteration_budget_hint(iteration: u8, max_iterations: u8) -> String {
    build_loop_budget_hint(iteration, max_iterations, 0, 0, 2)
}

/// Full budget hint with cumulative LLM token usage.
///
/// `tokens_max == 0` means no token cap (rounds-only).
/// `baseline_rounds == 0` omits soft-pace fields and over-baseline prose.
pub fn build_loop_budget_hint(
    iteration: u8,
    max_iterations: u8,
    tokens_used: u32,
    tokens_max: u32,
    baseline_rounds: u8,
) -> String {
    let round = iteration + 1;
    let remaining_rounds = max_iterations.saturating_sub(round);
    let tokens_remaining = if tokens_max == 0 {
        0
    } else {
        tokens_max.saturating_sub(tokens_used)
    };
    let open = super::host_markers::HOST_OBSERVATION_MARKERS
        .iter()
        .find(|m| m.tag == "<loop_budget")
        .expect("loop_budget marker registered")
        .tag;
    let baseline_attr = if baseline_rounds == 0 {
        String::new()
    } else {
        format!(" baseline_rounds=\"{baseline_rounds}\"")
    };
    let mut hint = format!(
        "{open} round=\"{round}\"{baseline_attr} max_rounds=\"{max_iterations}\" remaining_rounds=\"{remaining_rounds}\" \
         tokens_used=\"{tokens_used}\" tokens_max=\"{tokens_max}\" tokens_remaining=\"{tokens_remaining}\" />"
    );
    // Soft urgency: past usual pace baseline (still under hard max).
    if baseline_rounds > 0 && round > baseline_rounds {
        let pace = BUDGET_PACE_OVER_BASELINE
            .replace("{round}", &round.to_string())
            .replace("{baseline}", &baseline_rounds.to_string())
            .replace("{max_rounds}", &max_iterations.to_string())
            .replace("{remaining_rounds}", &remaining_rounds.to_string());
        let pace = pace.trim();
        if !pace.is_empty() {
            hint.push_str("\n\n");
            hint.push_str(pace);
        }
    }
    // Soft near-ceiling: last hard round remaining (or already on last).
    if max_iterations > 0 && remaining_rounds <= 1 {
        let near = BUDGET_PACE_NEAR_CEILING
            .replace("{round}", &round.to_string())
            .replace("{baseline}", &baseline_rounds.to_string())
            .replace("{max_rounds}", &max_iterations.to_string())
            .replace("{remaining_rounds}", &remaining_rounds.to_string());
        let near = near.trim();
        if !near.is_empty() {
            hint.push_str("\n\n");
            hint.push_str(near);
        }
    }
    hint
}

#[cfg(test)]
mod budget_hint_tests {
    use super::build_loop_budget_hint;

    #[test]
    fn budget_hint_includes_soft_baseline() {
        let h = build_loop_budget_hint(0, 8, 100, 16000, 2);
        assert!(h.contains("round=\"1\""));
        assert!(h.contains("baseline_rounds=\"2\""));
        assert!(h.contains("max_rounds=\"8\""));
        assert!(!h.contains("3/2"), "round 1 must not inject over-baseline prose: {h}");
    }

    #[test]
    fn budget_hint_over_baseline_injects_pace_prose() {
        let h = build_loop_budget_hint(2, 8, 1000, 16000, 2);
        // iteration 2 → round 3 > baseline 2
        assert!(h.contains("round=\"3\""));
        assert!(h.contains("baseline_rounds=\"2\""));
        assert!(
            h.contains("3/2") || h.contains("**3/2**"),
            "expected soft pace 3/2 observation: {h}"
        );
        assert!(h.contains("硬顶仍为 8") || h.contains("max_rounds"));
    }

    #[test]
    fn budget_hint_baseline_zero_skips_soft_fields() {
        let h = build_loop_budget_hint(3, 8, 0, 0, 0);
        assert!(!h.contains("baseline_rounds"));
        assert!(!h.contains("3/2"));
    }

    #[test]
    fn budget_hint_near_ceiling_injects_pace() {
        // iteration 7 → round 8, max 8, remaining 0
        let h = build_loop_budget_hint(7, 8, 0, 0, 2);
        assert!(h.contains("round=\"8\""));
        assert!(
            h.contains("8/8") || h.contains("剩余硬顶"),
            "expected near-ceiling observation: {h}"
        );
    }
}

/// Render the per-run query card as a trailing user-message host observation
/// (L0, 2026-08-03). Mirrors `build_loop_budget_hint`'s P0 trailing-message
/// injection: the system + history prefix stays stable across ReAct rounds so
/// provider prefix cache can hit. `None` when no card was produced (card
/// absent = instrumentation inactive; generic evidence gate still on).
///
/// Format:
/// `<query_card type="calculation" required="calculator" />`
pub fn build_query_card_block(card: &super::query_card::QueryCard) -> Option<String> {
    if card.question_type == super::query_card::QuestionType::Other
        && card.required_actions.is_empty()
    {
        return None;
    }
    let open = super::host_markers::HOST_OBSERVATION_MARKERS
        .iter()
        .find(|m| m.tag == "<query_card")
        .expect("query_card marker registered")
        .tag;
    let required = if card.required_actions.is_empty() {
        String::new()
    } else {
        format!(" required=\"{}\"", card.required_actions.join(","))
    };
    Some(format!(
        "{open} type=\"{}\"{required} />",
        card.question_type.as_str()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // D9: mandatory retrieve = capability skills only (no memory full-body).
    fn rag_mode() -> super::super::config::ModeConfig {
        let mut mode = super::super::config::load_mode_config("rag").unwrap();
        mode.skill_catalog.mandatory.retrieve = super::super::derive_mandatory_retrieve(true, false);
        mode
    }

    fn search_mode() -> super::super::config::ModeConfig {
        let mut mode = super::super::config::load_mode_config("search").unwrap();
        mode.skill_catalog.mandatory.retrieve = super::super::derive_mandatory_retrieve(false, true);
        mode
    }

    #[test]
    fn rag_retrieve_tools_always_from_tool_pool_only() {
        let mode = rag_mode();
        let registry = CapabilityRegistry::standard_cached();
        assert!(mode.tools_for_retrieve(registry).is_empty());

        let mut disclosed = DisclosedState::default();
        disclosed.disclosed_skill_ids.insert("memory".to_string());
        // D8 (2026-08-02): memory is prose-only disclosure; its access is via
        // client.history / client.user_profile in the sandbox, never native tools.
        let tools = mode.resolve_tool_specs(registry, &[]);
        assert!(tools.is_empty());
    }

    #[test]
    fn rag_round_zero_discloses_codegen_bundle() {
        let mode = rag_mode();
        let registry = CapabilityRegistry::standard_cached();
        let mut disclosed = DisclosedState::default();
        let ctx = ContextAssembler::assemble_retrieve(
            0,
            4,
            &mode,
            &crate::runtime::AgentRequest {
                kind: crate::AgentKind::Rag,
                query: "test".to_string(),
                workspace_id: None,
                session_id: None,
                doc_scope: vec![],
                messages: vec![],
                user_preferences: None,
                debug: false,
                stream: false,
                language: None,
                auth: crate::runtime::stub_agent_auth(),
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
        assert!(
            ctx.system_content.contains("client.dense")
                || ctx.system_content.contains("dense(query)"),
            "knowledge-base skill should document dense retrieval"
        );
        assert!(!ctx.system_content.contains("rag-codegen-guide"));
        assert!(ctx.system_content.contains("Retrieval query: test"));
        assert!(
            ctx.budget_hint.contains(
                "<loop_budget round=\"1\" baseline_rounds=\"2\" max_rounds=\"4\" remaining_rounds=\"3\" \
                     tokens_used=\"0\" tokens_max=\"0\" tokens_remaining=\"0\" />"
            ) || ctx.budget_hint.contains("baseline_rounds=\"2\"")
                && ctx.budget_hint.contains("round=\"1\"")
                && ctx.budget_hint.contains("max_rounds=\"4\""),
            "budget_hint missing soft baseline: {}",
            ctx.budget_hint
        );
        // D8: memory is prose-only disclosure (client.history / client.user_profile
        // in the sandbox); no native tools are exposed — retrieval stays SDK-only.
        assert!(
            ctx.tools.is_empty(),
            "rag retrieve exposes no native tools: {:?}",
            ctx.tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn rag_round_one_re_injects_codegen_skill() {
        let mode = rag_mode();
        let registry = CapabilityRegistry::standard_cached();
        let mut disclosed = DisclosedState::default();
        disclosed
            .disclosed_skill_ids
            .insert("knowledge-base".to_string());
        let request = crate::runtime::AgentRequest {
            kind: crate::AgentKind::Rag,
            query: "test".to_string(),
            workspace_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            auth: crate::runtime::stub_agent_auth(),
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
            ctx.system_content.contains("client.dense")
                || ctx.system_content.contains("dense(query)"),
            "iteration 1 must still include SaC SDK signatures: {}",
            &ctx.system_content[..ctx.system_content.len().min(400)]
        );
        assert!(
            !ctx.system_content.contains("Retrieval query:"),
            "retrieval query injection is first-round only"
        );
    }

    #[test]
    fn search_round_zero_exposes_configured_tool_pool() {
        let mode = search_mode();
        let registry = CapabilityRegistry::standard_cached();
        let mut disclosed = DisclosedState::default();
        let ctx = ContextAssembler::assemble_retrieve(
            0,
            4,
            &mode,
            &crate::runtime::AgentRequest {
                kind: crate::AgentKind::Search,
                query: "latest rust release".to_string(),
                workspace_id: None,
                session_id: None,
                doc_scope: vec![],
                messages: vec![],
                user_preferences: None,
                debug: false,
                stream: false,
                language: None,
                auth: crate::runtime::stub_agent_auth(),
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
        // A1: search tool_pool empty — web is SaC only (client.web).
        assert!(
            names.is_empty()
                || !names
                    .iter()
                    .any(|n| *n == "web_search" || *n == "web_fetch"),
            "web_* must not be disclosed to LLM: {names:?}"
        );
    }

    #[test]
    fn rag_retrieve_stays_tool_free_after_memory_skill_request() {
        let mode = rag_mode();
        let registry = CapabilityRegistry::standard_cached();
        let mut disclosed = DisclosedState::default();
        disclosed.last_skill_request = Some(vec!["memory".to_string()]);
        let ctx = ContextAssembler::assemble_retrieve(
            1,
            4,
            &mode,
            &crate::runtime::AgentRequest {
                kind: crate::AgentKind::Rag,
                query: "test".to_string(),
                workspace_id: None,
                session_id: None,
                doc_scope: vec![],
                messages: vec![],
                user_preferences: None,
                debug: false,
                stream: false,
                language: None,
                auth: crate::runtime::stub_agent_auth(),
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
        assert!(
            ctx.system_content.contains("memory")
                || ctx.system_content.contains("client.history")
                || ctx.system_content.contains("user_profile"),
            "memory skill_request should inject memory body: {}",
            &ctx.system_content[..ctx.system_content.len().min(500)]
        );
        assert!(
            ctx.tools.is_empty(),
            "memory access is via client.history/user_profile in the sandbox, not native tools"
        );
    }

    #[test]
    fn synthesis_uses_thin_agent_base_without_kb_method_table() {
        let mode = rag_mode();
        let registry = CapabilityRegistry::standard_cached();
        let mut disclosed = DisclosedState::default();
        // Pretend retrieve already disclosed knowledge-base.
        disclosed
            .disclosed_skill_ids
            .insert("knowledge-base".to_string());
        let ctx = ContextAssembler::assemble_synthesis(
            &mode,
            &crate::runtime::AgentRequest {
                kind: crate::AgentKind::Rag,
                query: "test".to_string(),
                workspace_id: None,
                session_id: None,
                doc_scope: vec![],
                messages: vec![],
                user_preferences: None,
                debug: false,
                stream: false,
                language: None,
                auth: crate::runtime::stub_agent_auth(),
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
        );
        assert!(
            ctx.system_content.contains("沙箱基座") || ctx.system_content.contains("code language"),
            "synthesis keeps agent-base"
        );
        // L0 method table should not be re-injected on synthesis assembly.
        assert!(
            !ctx.system_content.contains("await client.dense")
                && !ctx.system_content.contains("struct_catalog"),
            "synthesis must not carry full KB method table: {}",
            &ctx.system_content[..ctx.system_content.len().min(600)]
        );
    }

    #[test]
    fn parse_skill_request_rejects_heuristic_phrases() {
        use crate::react_loop::skill_request::parse_skill_request;
        assert!(parse_skill_request("请求 **knowledge-base**").is_empty());
        assert!(parse_skill_request("request cluster `knowledge-base`").is_empty());
    }
}
