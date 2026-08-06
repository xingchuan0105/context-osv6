//! Assemble a runtime [`ModeConfig`] from a product [`CapabilitySet`].
//!
//! Wired by `chat::pipeline_steps::dispatch_agent_mode` into AgentRequest metadata
//! (`assembled_mode_config`, `system_prompt_parts`, `capabilities`).

use crate::capabilities::CapabilitySet;
use agent_loop::r#loop::config::{
    AnswerContractKind, ModeConfig, SkillCatalogConfig, load_mode_config,
};
use common::AppError;

/// Always-on single-agent main system voice.
const AGENT_BASE: &str = "prompts/system/agent-base.md";
/// Mounted when product knowledge-base retrieval is enabled (internal mode id may still be `rag`).
const CAPABILITY_KNOWLEDGE_BASE: &str = "prompts/capabilities/knowledge-base/contract.md";
/// Mounted when product web retrieval is enabled.
const CAPABILITY_WEB: &str = "prompts/capabilities/web/contract.md";

#[derive(Debug, Clone)]
pub struct AssembledMode {
    pub config: ModeConfig,
    /// Prompt paths joined for system: always `agent-base`, then optional
    /// `capabilities/knowledge-base` and/or `capabilities/web` when mounted.
    pub system_prompt_parts: Vec<String>,
}

/// Build a `ModeConfig` by unioning capability mode YAML on top of chat defaults.
///
/// **System prompts:** `agent-base` is always first. Knowledge-base / web
/// capability contracts are appended only when the corresponding product capability is on.
///
/// Budget: pure chat keeps `chat` YAML; with capabilities, **sum** selected
/// capability modes' `max_iterations` / tier maps (not max, not +chat base).
/// Temperature: chat/rag/search YAML are unified; last applied value is fine.
pub fn assemble_mode(caps: CapabilitySet) -> Result<AssembledMode, AppError> {
    let mut config = load_mode_config("chat")?;
    // SaC: main voice always present; capability modules only when mounted.
    let mut system_prompt_parts = vec![AGENT_BASE.to_string()];

    config.tool_pool.clear();
    config.system_prompt_base = AGENT_BASE.to_string();

    // Capability path: budget = sum of selected modes only (exclude chat base).
    if caps.rag || caps.search {
        config.budget.max_iterations = 0;
        config.budget.by_user_tier = None;
        config.budget.max_tokens = None;
        config.budget.max_tokens_by_user_tier = None;
        config.budget.no_chunk_grace_tokens = None;
    }

    if caps.rag {
        let rag = load_mode_config("rag")?;
        // A1: do not merge native retrieval tool_pool — SaC SDK only.
        merge_skill_catalog(&mut config.skill_catalog, &rag.skill_catalog);
        add_budget(&mut config, &rag);
        config.inject_retrieval_query = true;
        apply_single_agent_loop_exit(&mut config, &rag);
        config.auto_fallback = rag.auto_fallback.clone();
        if let Some(t) = rag.temperature {
            config.temperature = Some(t);
        }
        system_prompt_parts.push(CAPABILITY_KNOWLEDGE_BASE.to_string());
    }

    if caps.search {
        let search = load_mode_config("search")?;
        // A1: native web_search/web_fetch stay off the LLM tool surface.
        merge_skill_catalog(&mut config.skill_catalog, &search.skill_catalog);
        add_budget(&mut config, &search);
        config.inject_retrieval_query = true;
        apply_single_agent_loop_exit(&mut config, &search);
        if !caps.rag {
            config.auto_fallback = search.auto_fallback.clone();
        }
        // Dual keeps knowledge-base auto_fallback (set above when rag was applied).
        if let Some(t) = search.temperature {
            config.temperature = Some(t);
        }
        system_prompt_parts.push(CAPABILITY_WEB.to_string());
    }

    if caps.rag || caps.search {
        // Single agent answers in-loop (A2); no monomode answer skill mandatory.
        config.skill_catalog.mandatory.synthesis.clear();
        config.tool_pool.clear();
    }

    if caps.is_pure_chat() {
        config.loop_exit.require_evidence = false;
        config.loop_exit.allow_content_early_stop = true;
        config.loop_exit.skip_synthesis_on_direct_answer = true;
        config.synthesis_output.contract = AnswerContractKind::ProseOnly;
        config.inject_retrieval_query = false;
        config.auto_fallback = None;
        // D11: native tool surface closed — the pure-chat trio
        // (user_context/calculator/weather_query) is served via sandbox
        // `client.*` SDK primitives (base capability), never as native tools.
        config.tool_pool.clear();
        config.skill_catalog.mandatory.synthesis.clear();
    }

    config.id = caps.agent_type_label().to_string();
    config.system_prompt_base = system_prompt_parts
        .first()
        .cloned()
        .unwrap_or_else(|| AGENT_BASE.to_string());

    // D9: mandatory retrieve skills derived straight from the capability set
    // (memory base + capability skills); the YAML `skill_catalog.mandatory`
    // indirect layer is retired for SaC modes.
    config.skill_catalog.mandatory.retrieve =
        agent_loop::r#loop::derive_mandatory_retrieve(caps.rag, caps.search);

    // A3: SaC SDK subset for this capability set (sandbox host enforces).
    config.sdk_primitives = agent_loop::r#loop::sdk_primitives_for_caps(caps.rag, caps.search)
        .into_iter()
        .map(str::to_string)
        .collect();

    Ok(AssembledMode {
        config,
        system_prompt_parts,
    })
}

/// Single-agent loop exit (A2): one ReAct run produces the user-facing answer.
///
/// Grounding is **skill-owned** (`require_evidence` is not a host hard gate).
/// `worker_handoff` is **false** — no orchestrator brief / handoff JSON.
///
/// Three-loop switches (`forbid_retrieve_direct_answer`, `short_judge`,
/// `judge_max_fail_rounds`) are **inherited from capability mode YAML**
/// (`rag` / `search`) and OR-merged across dual capabilities — they must not
/// be wiped by this function (acceptance 2026-08-07).
fn apply_single_agent_loop_exit(config: &mut ModeConfig, capability_yaml: &ModeConfig) {
    config.loop_exit.require_evidence = false;
    config.loop_exit.allow_content_early_stop = false;
    // Inherit / OR-merge three-loop flags from the capability mode being applied.
    config.loop_exit.forbid_retrieve_direct_answer |=
        capability_yaml.loop_exit.forbid_retrieve_direct_answer;
    config.loop_exit.short_judge |= capability_yaml.loop_exit.short_judge;
    if capability_yaml.loop_exit.judge_max_fail_rounds > 0 {
        config.loop_exit.judge_max_fail_rounds = config
            .loop_exit
            .judge_max_fail_rounds
            .max(capability_yaml.loop_exit.judge_max_fail_rounds);
    }
    // Three-loop: retrieve never ships final prose; always synthesize (+ judge).
    // Legacy single-path (no three-loop flags) still allows skip-synthesis DirectAnswer.
    config.loop_exit.skip_synthesis_on_direct_answer =
        !(config.loop_exit.forbid_retrieve_direct_answer || config.loop_exit.short_judge);
    config.synthesis_output.contract = AnswerContractKind::ProseOnly;
    config.worker_handoff = false;
}

fn merge_skill_catalog(dst: &mut SkillCatalogConfig, src: &SkillCatalogConfig) {
    union_strings(&mut dst.retrieve_clusters, &src.retrieve_clusters);
    union_strings(&mut dst.synthesis_clusters, &src.synthesis_clusters);
    union_strings(&mut dst.mandatory.retrieve, &src.mandatory.retrieve);
    union_strings(&mut dst.mandatory.synthesis, &src.mandatory.synthesis);
    for cluster in &src.clusters {
        if !dst.clusters.iter().any(|c| c.id == cluster.id) {
            dst.clusters.push(cluster.clone());
        }
    }
}

fn union_strings(dst: &mut Vec<String>, src: &[String]) {
    for s in src {
        if !dst.iter().any(|x| x == s) {
            dst.push(s.clone());
        }
    }
}

/// Sum iteration **and token** budgets from a selected capability mode into `dst`.
/// Used for multi-select: dual = rag.budget + search.budget (not max).
fn add_budget(dst: &mut ModeConfig, src: &ModeConfig) {
    dst.budget.max_iterations = dst
        .budget
        .max_iterations
        .saturating_add(src.budget.max_iterations);
    match (&mut dst.budget.by_user_tier, &src.budget.by_user_tier) {
        (Some(dst_map), Some(src_map)) => {
            for (k, v) in src_map {
                dst_map
                    .entry(k.clone())
                    .and_modify(|cur| *cur = (*cur).saturating_add(*v))
                    .or_insert(*v);
            }
        }
        (None, Some(src_map)) => {
            dst.budget.by_user_tier = Some(src_map.clone());
        }
        (Some(dst_map), None) => {
            let _ = dst_map;
        }
        (None, None) => {}
    }
    // Token caps: sum Options (None treated as 0 for addend; keep None if both None).
    match (dst.budget.max_tokens, src.budget.max_tokens) {
        (None, None) => {}
        (a, b) => {
            dst.budget.max_tokens = Some(a.unwrap_or(0).saturating_add(b.unwrap_or(0)));
        }
    }
    match (
        &mut dst.budget.max_tokens_by_user_tier,
        &src.budget.max_tokens_by_user_tier,
    ) {
        (Some(dst_map), Some(src_map)) => {
            for (k, v) in src_map {
                dst_map
                    .entry(k.clone())
                    .and_modify(|cur| *cur = (*cur).saturating_add(*v))
                    .or_insert(*v);
            }
        }
        (None, Some(src_map)) => {
            dst.budget.max_tokens_by_user_tier = Some(src_map.clone());
        }
        _ => {}
    }
    match (
        dst.budget.no_chunk_grace_tokens,
        src.budget.no_chunk_grace_tokens,
    ) {
        (None, None) => {}
        (a, b) => {
            dst.budget.no_chunk_grace_tokens = Some(a.unwrap_or(0).saturating_add(b.unwrap_or(0)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_loop::r#loop::config::AnswerContractKind;

    #[test]
    fn pure_chat_has_empty_tool_pool_and_one_prompt_part() {
        let assembled = assemble_mode(CapabilitySet::default()).expect("assemble pure chat");
        assert_eq!(assembled.config.id, "chat");
        // D11: native tool surface closed — pure-chat trio served via sandbox
        // client.* SDK primitives; never retrieval / delegate tools.
        assert!(assembled.config.tool_pool.is_empty(), "tool_pool: {:?}", assembled.config.tool_pool);
        assert_eq!(assembled.system_prompt_parts.len(), 1);
        assert_eq!(
            assembled.system_prompt_parts[0],
            "prompts/system/agent-base.md"
        );
        assert_eq!(
            assembled.config.synthesis_output.contract,
            AnswerContractKind::ProseOnly
        );
        assert!(!assembled.config.loop_exit.require_evidence);
        assert!(assembled.config.loop_exit.allow_content_early_stop);
        assert!(assembled.config.loop_exit.skip_synthesis_on_direct_answer);
        assert!(!assembled.config.inject_retrieval_query);
        // P0-2: no mandatory synthesis/chat.md on pure chat.
        assert!(
            assembled
                .config
                .skill_catalog
                .mandatory
                .synthesis
                .is_empty(),
            "pure chat must not mandatory-inject synthesis skills: {:?}",
            assembled.config.skill_catalog.mandatory.synthesis
        );
        // A3: pure chat opens memory + fs + trio (SDK) only.
        assert!(
            assembled.config.sdk_primitives.contains(&"history".into())
                && assembled.config.sdk_primitives.contains(&"save".into())
                && assembled.config.sdk_primitives.contains(&"user_context".into())
                && assembled.config.sdk_primitives.contains(&"calculator".into())
                && assembled.config.sdk_primitives.contains(&"weather_query".into())
                && !assembled.config.sdk_primitives.contains(&"dense".into())
                && !assembled.config.sdk_primitives.contains(&"web".into()),
            "pure chat sdk_primitives: {:?}",
            assembled.config.sdk_primitives
        );
    }

    #[test]
    fn dual_has_two_capability_prompt_parts_worker_handoff_contract() {
        let caps = CapabilitySet {
            rag: true,
            search: true,
        };
        let assembled = assemble_mode(caps).expect("assemble dual");
        assert_eq!(assembled.config.id, "rag+search");
        // agent-base always, then mounted capability contracts.
        assert_eq!(
            assembled.system_prompt_parts,
            vec![
                "prompts/system/agent-base.md".to_string(),
                "prompts/capabilities/knowledge-base/contract.md".to_string(),
                "prompts/capabilities/web/contract.md".to_string(),
            ]
        );
        assert!(
            assembled
                .config
                .skill_catalog
                .mandatory
                .synthesis
                .is_empty(),
            "capability paths must not mandate monomode answer skills: {:?}",
            assembled.config.skill_catalog.mandatory.synthesis
        );
        assert!(
            !assembled
                .config
                .tool_pool
                .iter()
                .any(|t| t == "user_context"),
            "capability workers must not inherit chat user_context: {:?}",
            assembled.config.tool_pool
        );
        // A1: native web_search not on LLM surface — SaC client.web instead.
        assert!(
            assembled.config.tool_pool.is_empty(),
            "single-agent capability modes must not expose native retrieval tools: {:?}",
            assembled.config.tool_pool
        );
        assert!(
            assembled.config.sdk_primitives.contains(&"web".into())
                && assembled.config.sdk_primitives.contains(&"dense".into())
                && assembled.config.sdk_primitives.contains(&"grep".into()),
            "dual sdk_primitives: {:?}",
            assembled.config.sdk_primitives
        );
        // A2 single agent: ProseOnly user answer, not worker handoff.
        assert_eq!(
            assembled.config.synthesis_output.contract,
            AnswerContractKind::ProseOnly
        );
        assert!(!assembled.config.worker_handoff);
        assert!(assembled.config.inject_retrieval_query);
        assert!(
            !assembled.config.loop_exit.require_evidence,
            "require_evidence is skill-owned, not host-forced"
        );
        assert!(!assembled.config.loop_exit.allow_content_early_stop);
        // Three-loop from rag/search YAML (must survive apply_single_agent_loop_exit).
        assert!(
            assembled.config.loop_exit.forbid_retrieve_direct_answer,
            "product dual must forbid retrieve DirectAnswer"
        );
        assert!(
            assembled.config.loop_exit.short_judge,
            "product dual must enable short_judge"
        );
        assert!(
            assembled.config.loop_exit.judge_max_fail_rounds >= 3,
            "judge_max_fail_rounds: {}",
            assembled.config.loop_exit.judge_max_fail_rounds
        );
        assert!(
            !assembled.config.loop_exit.skip_synthesis_on_direct_answer,
            "three-loop must not skip synthesis"
        );
        // Budget is sum of rag(12) + search(8) = 20 (not max; chat base not included)
        assert_eq!(assembled.config.budget.max_iterations, 20);
        // Skill catalog union includes knowledge-base (rag) and search cluster.
        assert!(
            assembled
                .config
                .skill_catalog
                .cluster_by_id("knowledge-base")
                .is_some()
        );
        assert!(
            assembled
                .config
                .skill_catalog
                .cluster_by_id("search")
                .is_some()
        );
        // auto_fallback from rag when dual
        let fb = assembled.config.auto_fallback.expect("rag fallback");
        assert_eq!(fb.tool_id, "dense_retrieval");
        // Temperature unified across modes
        assert_eq!(assembled.config.temperature, Some(0.4));
    }

    #[test]
    fn rag_only_budget_is_rag_not_sum_with_chat() {
        let assembled = assemble_mode(CapabilitySet {
            rag: true,
            search: false,
        })
        .expect("assemble rag");
        assert_eq!(assembled.config.budget.max_iterations, 12);
        assert_eq!(assembled.config.budget.max_tokens, Some(28_000));
        assert_eq!(assembled.config.temperature, Some(0.4));
        assert!(assembled.config.sdk_primitives.contains(&"dense".into()));
        assert!(assembled.config.sdk_primitives.contains(&"grep".into()));
        assert!(!assembled.config.sdk_primitives.contains(&"web".into()));
    }

    #[test]
    fn search_only_budget_is_search() {
        let assembled = assemble_mode(CapabilitySet {
            rag: false,
            search: true,
        })
        .expect("assemble search");
        assert_eq!(assembled.config.budget.max_iterations, 8);
        assert_eq!(assembled.config.budget.max_tokens, Some(16_000));
        assert_eq!(assembled.config.temperature, Some(0.4));
        assert!(assembled.config.sdk_primitives.contains(&"web".into()));
        assert!(assembled.config.sdk_primitives.contains(&"fetch".into()));
        assert!(assembled.config.sdk_primitives.contains(&"dense".into()));
        assert!(!assembled.config.sdk_primitives.contains(&"grep".into()));
    }

    #[test]
    fn pure_chat_keeps_chat_budget_and_unified_temp() {
        let assembled = assemble_mode(CapabilitySet::default()).expect("pure chat");
        assert_eq!(assembled.config.budget.max_iterations, 4);
        assert_eq!(assembled.config.budget.max_tokens, Some(8_000));
        assert_eq!(assembled.config.temperature, Some(0.4));
    }

    #[test]
    fn rag_only_single_agent_contract_and_prompt() {
        let assembled = assemble_mode(CapabilitySet {
            rag: true,
            search: false,
        })
        .expect("assemble rag");
        assert_eq!(assembled.config.id, "rag");
        assert_eq!(
            assembled.system_prompt_parts,
            vec![
                "prompts/system/agent-base.md".to_string(),
                "prompts/capabilities/knowledge-base/contract.md".to_string(),
            ]
        );
        assert_eq!(
            assembled.config.synthesis_output.contract,
            AnswerContractKind::ProseOnly
        );
        assert!(!assembled.config.worker_handoff);
        assert!(!assembled.config.loop_exit.allow_content_early_stop);
        assert_three_loop_enabled(&assembled.config);
        assert!(
            assembled
                .config
                .skill_catalog
                .mandatory
                .synthesis
                .is_empty()
        );
        assert!(
            assembled
                .config
                .skill_catalog
                .mandatory
                .retrieve
                .iter()
                .any(|s| s == "knowledge-base"),
            "knowledge-base retrieve mandatory retained: {:?}",
            assembled.config.skill_catalog.mandatory.retrieve
        );
        assert!(
            assembled.config.tool_pool.is_empty(),
            "rag single-agent tool_pool empty (SaC only): {:?}",
            assembled.config.tool_pool
        );
    }

    fn assert_three_loop_enabled(config: &ModeConfig) {
        assert!(
            config.loop_exit.forbid_retrieve_direct_answer,
            "forbid_retrieve_direct_answer"
        );
        assert!(config.loop_exit.short_judge, "short_judge");
        assert!(
            config.loop_exit.judge_max_fail_rounds >= 3,
            "judge_max_fail_rounds={}",
            config.loop_exit.judge_max_fail_rounds
        );
        assert!(
            !config.loop_exit.skip_synthesis_on_direct_answer,
            "skip_synthesis must be false under three-loop"
        );
    }

    #[test]
    fn search_only_single_agent_contract_and_fallback() {
        let assembled = assemble_mode(CapabilitySet {
            rag: false,
            search: true,
        })
        .expect("assemble search");
        assert_eq!(assembled.config.id, "search");
        assert_eq!(
            assembled.config.synthesis_output.contract,
            AnswerContractKind::ProseOnly
        );
        assert!(!assembled.config.worker_handoff);
        assert!(!assembled.config.loop_exit.allow_content_early_stop);
        assert_three_loop_enabled(&assembled.config);
        assert!(
            assembled
                .config
                .skill_catalog
                .mandatory
                .synthesis
                .is_empty()
        );
        let fb = assembled.config.auto_fallback.expect("search fallback");
        assert_eq!(fb.tool_id, "web_search");
        assert!(
            assembled.config.tool_pool.is_empty(),
            "search single-agent tool_pool empty (SaC web): {:?}",
            assembled.config.tool_pool
        );
        assert!(assembled.config.sdk_primitives.contains(&"web".into()));
    }

    #[test]
    fn pure_chat_does_not_enable_three_loop() {
        let assembled = assemble_mode(CapabilitySet::default()).expect("chat");
        assert!(!assembled.config.loop_exit.forbid_retrieve_direct_answer);
        assert!(!assembled.config.loop_exit.short_judge);
        assert!(assembled.config.loop_exit.skip_synthesis_on_direct_answer);
    }
}
