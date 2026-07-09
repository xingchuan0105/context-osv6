use super::{AutoFallbackConfig, BudgetConfig, ModeConfig, load_mode_config};

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
        .cluster_by_id("codegen")
        .expect("codegen cluster");
    assert!(codegen.atomic);
    assert_eq!(codegen.skills, vec!["codegen".to_string()]);
    assert!(
        config
            .skill_catalog
            .mandatory
            .synthesis
            .contains(&"rag-answer".to_string())
    );
}

#[test]
fn search_mode_config_has_search_cluster() {
    let config = load_mode_config("search").expect("search mode should load");
    assert!(config.tool_pool.contains(&"web_search".to_string()));
    assert!(config.skill_catalog.cluster_by_id("search").is_some());
}

#[test]
fn chat_mode_config_has_empty_retrieve_tool_pool() {
    let config = load_mode_config("chat").expect("chat mode should load");
    assert!(
        config.tool_pool.is_empty(),
        "chat memory tools are on-demand via memory cluster disclosure"
    );
    assert!(
        config
            .skill_catalog
            .mandatory
            .synthesis
            .contains(&"chat".to_string())
    );
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
system_prompt_base: prompts/orchestrators/chat-system.md
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
    assert!(
        config
            .skill_catalog
            .mandatory
            .retrieve
            .contains(&"codegen".to_string())
    );
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
    };
    assert_eq!(cfg.resolve_max_iterations(None), 4);
}

#[test]
fn budget_config_clamps_to_at_least_one() {
    let cfg = BudgetConfig {
        max_iterations: 0,
        by_user_tier: None,
    };
    assert_eq!(cfg.resolve_max_iterations(None), 1);
}

#[test]
fn auto_fallback_config_deserializes_vertical() {
    let yaml = r#"
enabled: true
tool_id: web_search
top_k: 10
vertical: news
"#;
    let cfg: AutoFallbackConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(cfg.vertical.as_deref(), Some("news"));
}

#[test]
fn auto_fallback_config_default_vertical_none() {
    let yaml = r#"
enabled: true
tool_id: dense_retrieval
top_k: 10
"#;
    let cfg: AutoFallbackConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(cfg.vertical.is_none());
}
