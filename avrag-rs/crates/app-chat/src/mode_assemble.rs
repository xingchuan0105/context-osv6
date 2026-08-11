//! Assemble a runtime [`ModeConfig`] from a product [`CapabilitySet`].
//!
//! Wired by `chat::pipeline_steps::dispatch_agent_mode` into AgentRequest metadata
//! (`assembled_mode_config`, `system_prompt_parts`, `capabilities`).

use crate::capabilities::CapabilitySet;
use agent_loop::r#loop::config::{
    AnswerContractKind, ModeConfig, SkillCatalogConfig, load_mode_config,
};
use common::AppError;

/// Session base: identity, user channel, BASE tools (all product turns).
const AGENT_BASE: &str = "prompts/system/agent-base.md";
/// Lead voice when any retrieval capability is mounted (Lead+Workers).
const LEAD_BASE: &str = "prompts/system/lead-base.md";
/// Mounted when product knowledge-base retrieval is enabled (internal mode id may still be `rag`).
const CAPABILITY_KNOWLEDGE_BASE: &str = "prompts/capabilities/knowledge-base/contract.md";
/// Mounted when product web retrieval is enabled.
const CAPABILITY_WEB: &str = "prompts/capabilities/web/contract.md";

#[derive(Debug, Clone)]
pub struct AssembledMode {
    pub config: ModeConfig,
    /// Prompt paths joined for system: session `agent-base`, then optional
    /// `lead-base` (retrieval modes), then capability contracts when mounted.
    pub system_prompt_parts: Vec<String>,
}

/// Build a `ModeConfig` by unioning capability mode YAML on top of chat defaults.
///
/// **System prompts:** `agent-base` always first. With rag/search: `lead-base`,
/// then knowledge-base / web contracts when the corresponding capability is on.
///
/// Budget: pure chat keeps `chat` YAML; with capabilities, **max** of selected
/// modes' `max_iterations` / tier maps (not sum, not +chat base). Tokens optional
/// (omit = unlimited). Temperature: chat/rag/search YAML are unified.
pub fn assemble_mode(caps: CapabilitySet) -> Result<AssembledMode, AppError> {
    let mut config = load_mode_config("chat")?;
    // Session base always; Lead+Workers adds lead-base + capability contracts.
    let mut system_prompt_parts = vec![AGENT_BASE.to_string()];

    config.tool_pool.clear();
    config.system_prompt_base = AGENT_BASE.to_string();

    // Capability path: budget from selected modes only (exclude chat base).
    if caps.rag || caps.search {
        config.budget.max_iterations = 0;
        config.budget.by_user_tier = None;
        config.budget.max_tokens = None;
        config.budget.max_tokens_by_user_tier = None;
        config.budget.no_chunk_grace_tokens = None;
        system_prompt_parts.push(LEAD_BASE.to_string());
    }

    if caps.rag {
        let rag = load_mode_config("rag")?;
        // A1: do not merge native retrieval tool_pool — Worker SaC / host leaf.
        merge_skill_catalog(&mut config.skill_catalog, &rag.skill_catalog);
        add_budget(&mut config, &rag);
        config.inject_retrieval_query = true;
        apply_single_agent_loop_exit(&mut config, &rag);
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
        if let Some(t) = search.temperature {
            config.temperature = Some(t);
        }
        system_prompt_parts.push(CAPABILITY_WEB.to_string());
    }

    // Any retrieval capability: Lead + Workers.
    if caps.rag || caps.search {
        config.retrieve_strategy = agent_loop::r#loop::config::RetrieveStrategy::LeadWorkers;
        // No monomode answer skill mandatory; Lead synthesizes.
        config.skill_catalog.mandatory.synthesis.clear();
        config.tool_pool.clear();
    }

    if caps.is_pure_chat() {
        config.loop_exit.require_evidence = false;
        config.loop_exit.allow_content_early_stop = true;
        config.loop_exit.skip_synthesis_on_direct_answer = true;
        config.synthesis_output.contract = AnswerContractKind::ProseOnly;
        config.inject_retrieval_query = false;
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

    // D9: mandatory retrieve skills from capability set only (knowledge-base /
    // search). Memory is on-demand via skill_request (agent-base pointer).
    // YAML `skill_catalog.mandatory` indirect layer is retired for SaC modes.
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
/// Three-loop switches (`forbid_retrieve_direct_answer`, `verify`,
/// `verify_max_fail_rounds`) are **inherited from capability mode YAML**
/// (`rag` / `search`) and OR-merged across dual capabilities — they must not
/// be wiped by this function (acceptance 2026-08-07).
fn apply_single_agent_loop_exit(config: &mut ModeConfig, capability_yaml: &ModeConfig) {
    config.loop_exit.require_evidence = false;
    config.loop_exit.allow_content_early_stop = false;
    // Inherit / OR-merge three-loop flags from the capability mode being applied.
    config.loop_exit.forbid_retrieve_direct_answer |=
        capability_yaml.loop_exit.forbid_retrieve_direct_answer;
    config.loop_exit.verify |= capability_yaml.loop_exit.verify;
    if capability_yaml.loop_exit.verify_max_fail_rounds > 0 {
        config.loop_exit.verify_max_fail_rounds = config
            .loop_exit
            .verify_max_fail_rounds
            .max(capability_yaml.loop_exit.verify_max_fail_rounds);
    }
    // Three-loop: retrieve never ships final prose; always synthesize (+ judge).
    // Legacy single-path (no three-loop flags) still allows skip-synthesis DirectAnswer.
    config.loop_exit.skip_synthesis_on_direct_answer =
        !(config.loop_exit.forbid_retrieve_direct_answer || config.loop_exit.verify);
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

/// Merge capability budgets into `dst`.
///
/// **Rounds (primary):** dual takes **max** of selected modes (not sum) so
/// product hard cap stays e.g. rag 5, not rag+search=6.
/// **Tokens:** optional; omit/`None` → unlimited. When both set, sum (legacy).
fn add_budget(dst: &mut ModeConfig, src: &ModeConfig) {
    dst.budget.max_iterations = dst
        .budget
        .max_iterations
        .max(src.budget.max_iterations);
    match (&mut dst.budget.by_user_tier, &src.budget.by_user_tier) {
        (Some(dst_map), Some(src_map)) => {
            for (k, v) in src_map {
                dst_map
                    .entry(k.clone())
                    .and_modify(|cur| *cur = (*cur).max(*v))
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
    // Soft pace baseline: take max so dual keeps rag baseline when search is 0.
    dst.budget.baseline_iterations = dst
        .budget
        .baseline_iterations
        .max(src.budget.baseline_iterations);
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
                "prompts/system/lead-base.md".to_string(),
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
        // Retrieve→synthesis from rag YAML (must survive apply_single_agent_loop_exit).
        assert_retrieve_synthesis_enabled(&assembled.config);
        // Budget is max(rag 5, search 1) = 5 (rounds-only; chat base not included)
        assert_eq!(assembled.config.budget.max_iterations, 5);
        assert_eq!(
            assembled.config.budget.max_tokens, None,
            "dual: no token wall"
        );
        // Dual: Lead+Workers (not single-brain SacCodegen).
        assert_eq!(
            assembled.config.retrieve_strategy,
            agent_loop::r#loop::config::RetrieveStrategy::LeadWorkers
        );
        assert!(
            agent_loop::r#loop::config::is_lead_workers_path(&assembled.config),
            "dual must take lead_workers path"
        );

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
        assert_eq!(assembled.config.budget.max_iterations, 5);
        assert_eq!(
            assembled.config.budget.max_tokens, None,
            "rag: rounds-only, no token wall"
        );
        assert_eq!(assembled.config.temperature, Some(0.4));
        assert!(assembled.config.sdk_primitives.contains(&"dense".into()));
        assert!(assembled.config.sdk_primitives.contains(&"grep".into()));
        assert!(!assembled.config.sdk_primitives.contains(&"web".into()));
        assert_eq!(
            assembled.config.retrieve_strategy,
            agent_loop::r#loop::config::RetrieveStrategy::LeadWorkers
        );
        assert!(agent_loop::r#loop::config::is_lead_workers_path(
            &assembled.config
        ));

    }

    #[test]
    fn search_only_budget_is_search() {
        let assembled = assemble_mode(CapabilitySet {
            rag: false,
            search: true,
        })
        .expect("assemble search");
        assert_eq!(assembled.config.budget.max_iterations, 2);
        assert_eq!(
            assembled.config.budget.max_tokens, None,
            "search: no token wall"
        );
        assert_eq!(assembled.config.temperature, Some(0.4));
        assert!(assembled.config.sdk_primitives.contains(&"web".into()));
        assert!(assembled.config.sdk_primitives.contains(&"fetch".into()));
        assert!(
            !assembled.config.sdk_primitives.contains(&"dense".into()),
            "search-only must not mount dense: {:?}",
            assembled.config.sdk_primitives
        );
        assert!(!assembled.config.sdk_primitives.contains(&"grep".into()));
        // Lead+Workers: pack → Lead synthesis (no host direct user bubble).
        assert_eq!(
            assembled.config.retrieve_strategy,
            agent_loop::r#loop::config::RetrieveStrategy::LeadWorkers
        );
        assert!(agent_loop::r#loop::config::is_lead_workers_path(
            &assembled.config
        ));

        assert!(!assembled.config.loop_exit.skip_synthesis_on_direct_answer);
        assert!(assembled.config.loop_exit.forbid_retrieve_direct_answer);
        assert!(!assembled.config.loop_exit.verify);
    }

    #[test]
    fn pure_chat_keeps_chat_budget_and_unified_temp() {
        let assembled = assemble_mode(CapabilitySet::default()).expect("pure chat");
        assert_eq!(assembled.config.budget.max_iterations, 2);
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
                "prompts/system/lead-base.md".to_string(),
                "prompts/capabilities/knowledge-base/contract.md".to_string(),
            ]
        );
        assert_eq!(
            assembled.config.synthesis_output.contract,
            AnswerContractKind::ProseOnly
        );
        assert!(!assembled.config.worker_handoff);
        assert!(!assembled.config.loop_exit.allow_content_early_stop);
        assert_retrieve_synthesis_enabled(&assembled.config);
        assert_eq!(
            assembled.config.retrieve_strategy,
            agent_loop::r#loop::config::RetrieveStrategy::LeadWorkers,
            "W2: rag-only uses Lead+Workers"
        );
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

    /// Product rag/dual: retrieve forbids direct prose; always hand off to synthesis.
    /// Verify is optional (currently off for cost).
    fn assert_retrieve_synthesis_enabled(config: &ModeConfig) {
        assert!(
            config.loop_exit.forbid_retrieve_direct_answer,
            "forbid_retrieve_direct_answer"
        );
        assert!(
            !config.loop_exit.verify,
            "verify off (product cost)"
        );
        assert!(
            !config.loop_exit.skip_synthesis_on_direct_answer,
            "skip_synthesis must be false when retrieve forbids direct answer"
        );
    }

    #[test]
    fn search_only_lead_workers_contract() {
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
        // W3: pack → Lead synthesis; no host DeepSeek user bubble.
        assert_retrieve_synthesis_enabled(&assembled.config);
        assert_eq!(
            assembled.config.retrieve_strategy,
            agent_loop::r#loop::config::RetrieveStrategy::LeadWorkers
        );
        assert!(
            assembled
                .config
                .skill_catalog
                .mandatory
                .synthesis
                .is_empty()
        );
        assert!(
            assembled.config.tool_pool.is_empty(),
            "search single-agent tool_pool empty: {:?}",
            assembled.config.tool_pool
        );
        assert!(assembled.config.sdk_primitives.contains(&"web".into()));
    }

    #[test]
    fn pure_chat_does_not_enable_three_loop() {
        let assembled = assemble_mode(CapabilitySet::default()).expect("chat");
        assert!(!assembled.config.loop_exit.forbid_retrieve_direct_answer);
        assert!(!assembled.config.loop_exit.verify);
        assert!(assembled.config.loop_exit.skip_synthesis_on_direct_answer);
    }
}
