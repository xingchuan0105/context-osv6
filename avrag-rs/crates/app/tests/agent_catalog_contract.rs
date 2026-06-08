//! Contract tests for capability catalog and strategy schemas (UnifiedAgent era).
//!
//! Replaces deprecated `strategy_*` integration tests that exercised the removed
//! ChatStrategy / RagStrategy / SearchStrategy state machines.

#[test]
fn chat_conversation_history_tools_in_catalog() {
    let catalog = app::agents::progressive::atomic_tool_catalog_cached();
    let tool_names: Vec<&str> = catalog.iter().map(|t| t.spec().name.as_str()).collect();

    assert!(
        tool_names.contains(&"conversation_history_load"),
        "conversation_history_load should be in atomic tool catalog"
    );
    assert!(
        tool_names.contains(&"conversation_history_tag"),
        "conversation_history_tag should be in atomic tool catalog"
    );
}

#[test]
fn static_strategy_schemas_match_capability_registry() {
    let registry = app::agents::capability::CapabilityRegistry::standard();

    let chat = app::agents::capability::chat_schema();
    assert_eq!(registry.strategy("chat").unwrap().states, chat.states);

    let rag = app::agents::capability::rag_schema();
    assert_eq!(registry.strategy("rag").unwrap().states, rag.states);

    let search = app::agents::capability::search_schema();
    assert_eq!(registry.strategy("search").unwrap().states, search.states);
}
