//! Contract tests for capability catalog and mode schemas (UnifiedAgent era).
//!
//! Replaces deprecated `strategy_*` integration tests that exercised the removed
//! ChatStrategy / RagStrategy / SearchStrategy state machines.

// NOTE: chat_conversation_history_tools_in_catalog was removed because
// ADR-0007 moved tool schemas out of PromptRegistry into the memory cluster.
// atomic_tool_catalog_cached() is now intentionally empty.
// conversation_history is still available via the Per-Iteration Context Assembler.

#[test]
fn static_mode_schemas_match_capability_registry() {
    let registry = app::agents::capability::CapabilityRegistry::standard();

    let chat = app::agents::capability::chat_mode_schema();
    assert_eq!(registry.mode("chat").unwrap(), &chat);

    let rag = app::agents::capability::rag_mode_schema();
    assert_eq!(registry.mode("rag").unwrap(), &rag);

    let search = app::agents::capability::search_mode_schema();
    assert_eq!(registry.mode("search").unwrap(), &search);
}
