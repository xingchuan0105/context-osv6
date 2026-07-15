//! Assemble a runtime [`ModeConfig`] from a product [`CapabilitySet`].
//!
//! Wired by `chat::pipeline_steps::dispatch_agent_mode` into AgentRequest metadata
//! (`assembled_mode_config`, `system_prompt_parts`, `capabilities`).

use crate::capabilities::CapabilitySet;
use agent_loop::r#loop::config::{
    AnswerContractKind, ModeConfig, SkillCatalogConfig, load_mode_config,
};
use common::AppError;

const AGENT_BASE: &str = "prompts/orchestrators/agent-base.md";
const CAPABILITY_RAG: &str = "prompts/orchestrators/capability-rag.md";
const CAPABILITY_SEARCH: &str = "prompts/orchestrators/capability-search.md";
const USER_CONTEXT_TOOL: &str = "user_context";

#[derive(Debug, Clone)]
pub struct AssembledMode {
    pub config: ModeConfig,
    /// Prompt file paths to load and join (agent-base + capability manuals).
    pub system_prompt_parts: Vec<String>,
}

/// Build a `ModeConfig` by unioning capability mode YAML on top of chat defaults.
pub fn assemble_mode(caps: CapabilitySet) -> Result<AssembledMode, AppError> {
    // Base: pure-chat loop_exit / skill catalog / budget defaults.
    let mut config = load_mode_config("chat")?;
    let mut system_prompt_parts = vec![AGENT_BASE.to_string()];

    // Always start with user_context in the tool pool (chat YAML is empty).
    config.tool_pool = vec![USER_CONTEXT_TOOL.to_string()];
    config.system_prompt_base = AGENT_BASE.to_string();

    if caps.rag {
        let rag = load_mode_config("rag")?;
        merge_tool_pool(&mut config.tool_pool, &rag.tool_pool);
        merge_skill_catalog(&mut config.skill_catalog, &rag.skill_catalog);
        // Drop chat mandatory synthesis when leaving pure chat (chat.yaml has mandatory chat).
        config
            .skill_catalog
            .mandatory
            .synthesis
            .retain(|s| s != "chat");
        merge_max_budget(&mut config, &rag);
        config.inject_retrieval_query = true;
        config.loop_exit.require_evidence = true;
        config.loop_exit.allow_content_early_stop = false;
        config.loop_exit.skip_synthesis_on_direct_answer = false;
        config.synthesis_output.contract = AnswerContractKind::InternalAnswerUnifiedV1;
        config.auto_fallback = rag.auto_fallback.clone();
        if let Some(t) = rag.temperature {
            config.temperature = Some(t);
        }
        system_prompt_parts.push(CAPABILITY_RAG.to_string());
    }

    if caps.search {
        let search = load_mode_config("search")?;
        merge_tool_pool(&mut config.tool_pool, &search.tool_pool);
        merge_skill_catalog(&mut config.skill_catalog, &search.skill_catalog);
        merge_max_budget(&mut config, &search);
        config.inject_retrieval_query = true;
        config.loop_exit.require_evidence = true;
        config.loop_exit.allow_content_early_stop = false;
        config.loop_exit.skip_synthesis_on_direct_answer = false;
        if caps.rag {
            // Dual: unified contract + single mandatory answer skill.
            config.synthesis_output.contract = AnswerContractKind::InternalAnswerUnifiedV1;
            config.skill_catalog.mandatory.synthesis = vec!["rag-answer".to_string()];
            // Keep rag auto_fallback when dual.
        } else {
            config.synthesis_output.contract = AnswerContractKind::InternalAnswerUnifiedV1;
            config.auto_fallback = search.auto_fallback.clone();
            config.skill_catalog.mandatory.synthesis = vec!["search-answer".to_string()];
        }
        if let Some(t) = search.temperature {
            // Search-only uses search temp; dual keeps rag temp already set.
            if !caps.rag {
                config.temperature = Some(t);
            }
        }
        system_prompt_parts.push(CAPABILITY_SEARCH.to_string());
    }

    if caps.is_pure_chat() {
        // Chat YAML already has these; re-assert for clarity.
        config.loop_exit.require_evidence = false;
        config.loop_exit.allow_content_early_stop = true;
        config.loop_exit.skip_synthesis_on_direct_answer = true;
        config.synthesis_output.contract = AnswerContractKind::ProseOnly;
        config.inject_retrieval_query = false;
        config.auto_fallback = None;
        // tool_pool already only user_context; skill_catalog from chat yaml.
    }

    config.id = caps.agent_type_label().to_string();
    config.system_prompt_base = system_prompt_parts
        .first()
        .cloned()
        .unwrap_or_else(|| AGENT_BASE.to_string());

    Ok(AssembledMode {
        config,
        system_prompt_parts,
    })
}

fn merge_tool_pool(dst: &mut Vec<String>, src: &[String]) {
    for id in src {
        if !dst.iter().any(|x| x == id) {
            dst.push(id.clone());
        }
    }
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

fn merge_max_budget(dst: &mut ModeConfig, src: &ModeConfig) {
    dst.budget.max_iterations = dst.budget.max_iterations.max(src.budget.max_iterations);
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
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_loop::r#loop::config::AnswerContractKind;

    #[test]
    fn pure_chat_has_user_context_only_and_one_prompt_part() {
        let assembled = assemble_mode(CapabilitySet::default()).expect("assemble pure chat");
        assert_eq!(assembled.config.id, "chat");
        assert_eq!(assembled.config.tool_pool, vec!["user_context".to_string()]);
        assert!(!assembled
            .config
            .tool_pool
            .iter()
            .any(|t| t == "web_search"));
        assert_eq!(assembled.system_prompt_parts.len(), 1);
        assert_eq!(
            assembled.system_prompt_parts[0],
            "prompts/orchestrators/agent-base.md"
        );
        assert_eq!(
            assembled.config.synthesis_output.contract,
            AnswerContractKind::ProseOnly
        );
        assert!(!assembled.config.loop_exit.require_evidence);
        assert!(assembled.config.loop_exit.allow_content_early_stop);
        assert!(assembled.config.loop_exit.skip_synthesis_on_direct_answer);
        assert!(!assembled.config.inject_retrieval_query);
        // Chat skill catalog preserved.
        assert!(
            assembled
                .config
                .skill_catalog
                .mandatory
                .synthesis
                .contains(&"chat".to_string())
        );
    }

    #[test]
    fn dual_has_three_prompt_parts_web_search_and_hybrid_contract() {
        let caps = CapabilitySet {
            rag: true,
            search: true,
        };
        let assembled = assemble_mode(caps).expect("assemble dual");
        assert_eq!(assembled.config.id, "rag+search");
        assert_eq!(assembled.system_prompt_parts.len(), 3);
        assert_eq!(
            assembled.system_prompt_parts[0],
            "prompts/orchestrators/agent-base.md"
        );
        assert_eq!(
            assembled.system_prompt_parts[1],
            "prompts/orchestrators/capability-rag.md"
        );
        assert_eq!(
            assembled.system_prompt_parts[2],
            "prompts/orchestrators/capability-search.md"
        );
        assert_eq!(
            assembled.config.skill_catalog.mandatory.synthesis,
            vec!["rag-answer".to_string()],
            "dual must not mandate chat+rag-answer+search-answer together"
        );
        assert!(
            !assembled
                .config
                .skill_catalog
                .mandatory
                .synthesis
                .iter()
                .any(|s| s == "chat" || s == "search-answer")
        );
        assert!(
            assembled
                .config
                .tool_pool
                .iter()
                .any(|t| t == "user_context")
        );
        assert!(
            assembled
                .config
                .tool_pool
                .iter()
                .any(|t| t == "web_search"),
            "dual tool_pool must include web_search: {:?}",
            assembled.config.tool_pool
        );
        assert_eq!(
            assembled.config.synthesis_output.contract,
            AnswerContractKind::InternalAnswerUnifiedV1
        );
        assert!(assembled.config.inject_retrieval_query);
        assert!(assembled.config.loop_exit.require_evidence);
        assert!(!assembled.config.loop_exit.allow_content_early_stop);
        // Budget is max of chat(2) and rag(4)/search(3) → 4
        assert_eq!(assembled.config.budget.max_iterations, 4);
        // Skill catalog union includes codegen (rag) and search cluster.
        assert!(assembled.config.skill_catalog.cluster_by_id("codegen").is_some());
        assert!(assembled.config.skill_catalog.cluster_by_id("search").is_some());
        // auto_fallback from rag when dual
        let fb = assembled.config.auto_fallback.expect("rag fallback");
        assert_eq!(fb.tool_id, "dense_retrieval");
    }

    #[test]
    fn rag_only_contract_and_prompt() {
        let assembled = assemble_mode(CapabilitySet {
            rag: true,
            search: false,
        })
        .expect("assemble rag");
        assert_eq!(assembled.config.id, "rag");
        assert_eq!(assembled.system_prompt_parts.len(), 2);
        assert_eq!(
            assembled.config.synthesis_output.contract,
            AnswerContractKind::InternalAnswerUnifiedV1
        );
        assert!(!assembled
            .config
            .tool_pool
            .iter()
            .any(|t| t == "web_search"));
    }

    #[test]
    fn search_only_contract_and_fallback() {
        let assembled = assemble_mode(CapabilitySet {
            rag: false,
            search: true,
        })
        .expect("assemble search");
        assert_eq!(assembled.config.id, "search");
        assert_eq!(
            assembled.config.synthesis_output.contract,
            AnswerContractKind::InternalAnswerUnifiedV1
        );
        let fb = assembled.config.auto_fallback.expect("search fallback");
        assert_eq!(fb.tool_id, "web_search");
        assert!(assembled
            .config
            .tool_pool
            .iter()
            .any(|t| t == "web_search"));
    }
}
