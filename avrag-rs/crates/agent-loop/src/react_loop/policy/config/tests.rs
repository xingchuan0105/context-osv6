use super::{BudgetConfig, ModeConfig, load_mode_config};

#[test]
fn rag_mode_config_deserializes_with_tool_pool_and_clusters() {
    let config = load_mode_config("rag").expect("rag mode should load");
    assert_eq!(config.id, "rag");
    assert!(
        config.tool_pool.is_empty(),
        "RAG retrieve tools are on-demand via memory cluster disclosure"
    );
    let codegen = config
        .skill_catalog
        .cluster_by_id("knowledge-base")
        .expect("codegen cluster");
    assert!(codegen.atomic);
    assert_eq!(codegen.skills, vec!["knowledge-base".to_string()]);
    // P0-1 (2026-07-20): worker synthesis skills unhooked — the final message is
    // the brief's internal_worker_handoff_v1 JSON, not a monomode answer envelope.
    assert!(config.skill_catalog.mandatory.synthesis.is_empty());
    assert_eq!(
        config.synthesis_output.contract,
        super::super::config::AnswerContractKind::ProseOnly
    );
}

#[test]
fn search_mode_config_has_search_cluster() {
    let config = load_mode_config("search").expect("search mode should load");
    assert!(
        config.tool_pool.is_empty(),
        "search Lead+Workers; native tool_pool empty: {:?}",
        config.tool_pool
    );
    assert!(config.skill_catalog.cluster_by_id("search").is_some());
    assert_eq!(
        config.retrieve_strategy,
        super::RetrieveStrategy::LeadWorkers,
        "search-only YAML uses lead_workers (W3)"
    );
    assert!(
        !config.loop_exit.verify,
        "search-only: verify off"
    );
    assert!(
        !config.loop_exit.skip_synthesis_on_direct_answer,
        "search-only: synthesis required"
    );
    assert!(
        config.loop_exit.forbid_retrieve_direct_answer,
        "search-only: no host direct user bubble"
    );
}

#[test]
fn is_lead_workers_path_true_when_strategy_set() {
    use super::{RetrieveStrategy, is_lead_workers_path};
    let mut config = load_mode_config("rag").expect("rag");
    config.retrieve_strategy = RetrieveStrategy::LeadWorkers;
    assert!(is_lead_workers_path(&config));
    config.retrieve_strategy = RetrieveStrategy::SacCodegen;
    assert!(!is_lead_workers_path(&config));
}

#[test]
fn search_yaml_is_lead_workers() {
    use super::{RetrieveStrategy, is_lead_workers_path};
    let config = load_mode_config("search").expect("search");
    assert_eq!(config.retrieve_strategy, RetrieveStrategy::LeadWorkers);
    assert!(is_lead_workers_path(&config));
}

#[test]
fn chat_mode_config_has_only_user_context_tool() {
    let config = load_mode_config("chat").expect("chat mode should load");
    // YAML baseline only: `modes/chat.yaml` lists `user_context`.
    // D11: the effective pool is cleared in app-chat mode_assemble — the
    // pure-chat trio (user_context + calculator + weather_query) is served via
    // sandbox client.* SDK primitives, never as native tools.
    // IP geo / "定位" for pure chat is via `client.user_context`, not a native tool.
    assert_eq!(config.tool_pool, vec!["user_context".to_string()]);
    // P0-2 (2026-07-20): no mandatory synthesis/chat.md — chat-base is the sole
    // pure-chat voice; writing/format stay optional clusters.
    assert!(config.skill_catalog.mandatory.synthesis.is_empty());
}

#[test]
fn skill_catalog_yaml_ids_exist_in_registry() {
    for mode in ["rag", "search", "chat"] {
        let config = load_mode_config(mode).expect("mode should load");
        let registry = agent_tools::progressive::PromptRegistry::standard_cached();
        for cluster in &config.skill_catalog.clusters {
            assert!(
                registry.skill(&cluster.id).is_some(),
                "mode {mode} cluster '{}' missing from registry",
                cluster.id
            );
        }
        for skill in &config.skill_catalog.mandatory.synthesis {
            assert!(
                registry.skill(skill).is_some(),
                "mode {mode} mandatory synthesis '{skill}' missing from registry"
            );
        }
    }
}

#[test]
fn legacy_flat_skill_catalog_deserializes() {
    let yaml = r#"
mode: test
system_prompt_base: prompts/system/agent-base.md
skill_catalog:
  - foo
  - bar
budget:
  max_iterations: 2
"#;
    let mut config: ModeConfig = serde_yaml::from_str(yaml).unwrap();
    config.normalize();
    assert_eq!(config.skill_catalog.flat_skill_ids().len(), 2);
}

#[test]
fn rag_mode_has_mandatory_retrieve_codegen() {
    let config = load_mode_config("rag").expect("rag mode should load");
    assert!(config.inject_retrieval_query);
    // D9: YAML no longer carries `skill_catalog.mandatory` for SaC modes;
    // assemble_mode derives it from the capability set. YAML-level list is empty.
    assert!(
        config.skill_catalog.mandatory.retrieve.is_empty(),
        "rag.yaml must not carry a mandatory retrieve list (D9): {:?}",
        config.skill_catalog.mandatory.retrieve
    );
    assert_eq!(
        super::super::derive_mandatory_retrieve(true, false),
        vec!["knowledge-base".to_string()]
    );
    assert_eq!(
        super::super::derive_mandatory_retrieve(false, true),
        vec!["search".to_string()]
    );
    assert_eq!(
        super::super::derive_mandatory_retrieve(true, true),
        vec!["knowledge-base".to_string(), "search".to_string()]
    );
    assert!(super::super::derive_mandatory_retrieve(false, false).is_empty());
}

#[test]
fn search_mode_injects_retrieval_query() {
    let config = load_mode_config("search").expect("search mode should load");
    assert!(config.inject_retrieval_query);
}

#[test]
fn chat_mode_no_retrieval_query_injection() {
    let config = load_mode_config("chat").expect("chat mode should load");
    assert!(!config.inject_retrieval_query);
}

#[test]
fn budget_config_uses_tier_override_when_present() {
    let mut tiers = std::collections::HashMap::new();
    tiers.insert("free".to_string(), 2);
    tiers.insert("pro".to_string(), 6);
    let cfg = BudgetConfig {
        max_iterations: 4,
        by_user_tier: Some(tiers),
        ..Default::default()
    };
    assert_eq!(
        cfg.resolve_max_iterations(Some(&serde_json::json!("free"))),
        2
    );
    assert_eq!(
        cfg.resolve_max_iterations(Some(&serde_json::json!("PRO"))),
        6
    );
}

#[test]
fn budget_config_falls_back_to_max_iterations_for_unknown_tier() {
    let mut tiers = std::collections::HashMap::new();
    tiers.insert("free".to_string(), 2);
    let cfg = BudgetConfig {
        max_iterations: 4,
        by_user_tier: Some(tiers),
        ..Default::default()
    };
    assert_eq!(
        cfg.resolve_max_iterations(Some(&serde_json::json!("enterprise"))),
        4
    );
}

#[test]
fn budget_config_falls_back_when_no_tier() {
    let cfg = BudgetConfig {
        max_iterations: 4,
        by_user_tier: None,
        ..Default::default()
    };
    assert_eq!(cfg.resolve_max_iterations(None), 4);
}

#[test]
fn budget_config_clamps_to_at_least_one() {
    let cfg = BudgetConfig {
        max_iterations: 0,
        by_user_tier: None,
        ..Default::default()
    };
    assert_eq!(cfg.resolve_max_iterations(None), 1);
}

#[test]
fn budget_config_resolves_token_tier() {
    let mut tiers = std::collections::HashMap::new();
    tiers.insert("free".to_string(), 16_000);
    tiers.insert("pro".to_string(), 28_000);
    let cfg = BudgetConfig {
        max_iterations: 12,
        by_user_tier: None,
        baseline_iterations: 2,
        max_tokens: Some(28_000),
        max_tokens_by_user_tier: Some(tiers),
        no_chunk_grace_tokens: None,
    };
    assert_eq!(
        cfg.resolve_max_tokens(Some(&serde_json::json!("free"))),
        16_000
    );
    assert_eq!(cfg.resolve_max_tokens(None), 28_000);
    // Continue budget = +50% of baseline (not fixed rounds / absolute default).
    assert_eq!(cfg.resolve_continue_token_boost(28_000), 14_000);
    assert_eq!(cfg.resolve_continue_token_boost(16_000), 8_000);
    let with_override = BudgetConfig {
        no_chunk_grace_tokens: Some(10_000),
        ..cfg.clone()
    };
    assert_eq!(with_override.resolve_continue_token_boost(28_000), 10_000);
}


