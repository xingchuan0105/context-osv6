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
    // Capability catalog lives in agent-tools (not app::agents after Product App split).
    let registry = agent_tools::capability::CapabilityRegistry::standard();

    let chat = agent_tools::capability::chat_mode_schema();
    assert_eq!(registry.mode("chat").unwrap(), &chat);

    let rag = agent_tools::capability::rag_mode_schema();
    assert_eq!(registry.mode("rag").unwrap(), &rag);

    let search = agent_tools::capability::search_mode_schema();
    assert_eq!(registry.mode("search").unwrap(), &search);
}
