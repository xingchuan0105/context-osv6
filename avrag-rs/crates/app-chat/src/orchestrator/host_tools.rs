//! Orchestrator HostTools — brain/host intercepts (Wave C2).
//!
//! Coordinator LLM tool schemas. **Host-only** names must never appear on
//! `ToolCatalog`. Dual tools (e.g. memory load) may exist on both the
//! orchestrator surface and the worker catalog.

/// Host-**only** intercepts — must not be registered on ToolCatalog.
pub const HOST_ONLY_TOOL_NAMES: &[&str] = &[
    "delegate_rag",
    "delegate_search",
    "evidence_fetch",
    "finish_answer",
];

/// Full set the brain may expose this turn (host-only + dual tools).
pub const HOST_TOOL_NAMES: &[&str] = &[
    "delegate_rag",
    "delegate_search",
    "evidence_fetch",
    "finish_answer",
    "conversation_history_load",
];

pub const DELEGATE_RAG: &str = "delegate_rag";
pub const DELEGATE_SEARCH: &str = "delegate_search";
pub const EVIDENCE_FETCH: &str = "evidence_fetch";
pub const FINISH_ANSWER: &str = "finish_answer";
pub const CONVERSATION_HISTORY_LOAD: &str = "conversation_history_load";

#[cfg(test)]
mod tests {
    use super::*;
    use agent_tools::ToolCatalog;

    #[test]
    fn host_only_tools_are_not_on_tool_catalog() {
        let catalog = ToolCatalog::standard_cached();
        for name in HOST_ONLY_TOOL_NAMES {
            assert!(
                catalog.get(name).is_none(),
                "Host-only tool `{name}` must not be registered on ToolCatalog (Wave C2)"
            );
        }
    }

    #[test]
    fn dual_memory_tool_may_exist_on_catalog() {
        // conversation_history_load is valid on workers via SkillRegistry.
        assert!(
            ToolCatalog::standard_cached()
                .get(CONVERSATION_HISTORY_LOAD)
                .is_some()
        );
    }

    #[test]
    fn host_tool_name_constants_match_list() {
        for n in HOST_ONLY_TOOL_NAMES {
            assert!(HOST_TOOL_NAMES.contains(n));
        }
        assert!(HOST_TOOL_NAMES.contains(&CONVERSATION_HISTORY_LOAD));
    }
}
